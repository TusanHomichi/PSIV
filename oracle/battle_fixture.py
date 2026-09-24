#!/usr/bin/env python3
"""Turn a tape's RNG trace and RAM log into a replay fixture for psiv-core.

    python3 oracle/battle_fixture.py --trace build/tape07_rolls.csv \
                                     --log   build/tape07_battle.csv \
                                     --out   rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json

The fixture is the smallest set of numbers a replay needs: the battle's start
state, the cartridge's rolls in the order it drew them, the commands the party
issued, and what the cartridge's RAM says each action did. Every number is
transcribed from the two logs below; nothing is fitted to psiv-core, so a
replay that disagrees with the fixture is a finding about the port.

# The two inputs

`--trace` is the CSV `psiv_oracle --rng-trace` writes: one row per call of
`UpdateRNGSeed2` (`ps4.asm:86097`), with the HV word the read returned, the
frame counter and the seed longword around each call.

`--log` is the RAM log from the same run (`--groups core,battle,bhit,enemy,
chars,rng`). Its columns are read through `oracle/ram_map.json`, which is the
single source of truth for their width, address and hex/dec rendering.

# How a roll is derived, and why not from the trace's own roll column

`UpdateRNGSeed2` is:

    30 2D 00 08     move.w  $8(a5), d0        ; d0 = VDP HV counter
    D0 78 EF 1C     add.w   (Main_Frame_Count).w, d0
    90 78 EF 0C     sub.w   (RNG_Seed).w, d0   ; d0 = the roll
    E6 F8 EF 0C     ror     (RNG_Seed).w
    4E 75           rts

`(RNG_Seed).w` at `$FFFFEF0C` is the **high half** of the longword stored
there: a 68000 word operand at that address reads bytes `$EF0C`/`$EF0D`, and
`RNG_Seed` is a longword. So the roll is

    roll = (hv + frame_count - high_word(seed)) & $FFFF

The trace's own `roll` column instead subtracts the *low* half (`$FFFFEF0E`),
which is the word `ror` never touches and no instruction reads as the
subtrahend - while the same file's `seed_after` column does rotate the high
half. The two halves differ by a per-frame constant, so that column is a
per-frame-constant shift of the cartridge's rolls, not the rolls.

This module therefore derives every roll from the trace's raw `hv`,
`frame_count` and `seed_before` columns, which are not in doubt (the seed
chain and the HV reads were verified by `oracle/rng_trace.py check`), and
*reports* how the `roll` column relates to that derivation:

    "roll_column": {"agrees": n, "subtracts_low_word": n}

Both conventions are recomputed, so a trace that carries either one is
accepted and a trace that carries neither is rejected rather than replayed.
The reading is settled by the cartridge, not by preference: the high-word
derivation reproduces the battle's turn order (`turn_XX` in the log) and all
six of its damage values exactly, and the low-word one reproduces none of
them. See `docs/BATTLE_ORACLE_REPLAY.md`.

# What the log can and cannot decide

Fields the log decides outright: the party's and enemies' live stats, HP, TP
and status; the turn order each round; `Battle_Priority`; every roll's frame;
every `Fighters_Hit_Flags` byte and `Battle_Heal_Damage_List` word as they are
written; the experience and meseta totals; the drop rate.

Fields it does not, recorded here as `"undetermined"` notes:

* **Command identity.** The log carries menu cursors (`battle_main_option`,
  `battle_char_comd_idx`), not the command each member chose. Every party
  action observed in this battle is a physical attack (damage lands on enemy
  slots with no status or TP movement), and the tape holds C through the
  command phase, so the fixture says `attack` for each member and says so.
* **A miss versus an untargeted slot.** `Fighters_Hit_Flags` is `$FF` for
  both, so a slot whose flag is `$FF` and whose damage word did not move is
  reported as `"ff"` rather than guessed.
* **A damage word rewritten to the same value.** A slot's damage is only
  visible as a change; an action that rewrote the previous value is
  indistinguishable from one that skipped the slot.
* **Frame of the HP write.** `Battle_Heal_Damage_List` is filled by the
  damage routine and applied later, inside the animation state machine, so
  the fixtures' `hp_after` is the HP at the *end* of the action.

# Segmentation

An action is the run of frames between two changes of `battle_actor`
(`$FFFF4142`). A round begins at the frame where `Battle_Turn_Order`
(`turn_00`, `$FFFFEFB0`) changes - the order pass - and its actions follow.
Runs of exactly sixteen calls are `Battle_CalculateDamage`
(`ps4.asm:17381`: `moveq #$F,d7` with `dbf`), so a frame's non-damage calls
are whatever precedes them: the order pass's nine jitter draws plus four
`Enemy_TargetCharacter` draws, an action's hit or ability rolls, or - before
the first round and after the last - the encounter's formation draw and the
post-battle item drop draw, which the fixture keeps out of the battle's own
stream because no battle routine consumes them.
"""
import argparse
import csv
import hashlib
import json
import os
import sys

M16 = 0xFFFF

#: The frame `--battle-*` defaults bracket: the first basement battle of tape
#: 07, as `oracle/README.md` "Battle ground truth" records it.
BATTLE_FIRST = 24794
BATTLE_LAST = 30428

#: `Battle_CalculateDamage`'s loop bound, and therefore the length of every
#: damage run in the trace (`moveq #$F, d7` + `dbf`).
DAMAGE_RUN = 16

#: `Fighters_Hit_Flags`'s "not targeted or missed" byte.
HIT_NOT_TARGETED = "FF"


class FixtureError(Exception):
    """The inputs do not describe a battle this module can hand over."""


# --------------------------------------------------------------------------
# Log plumbing
# --------------------------------------------------------------------------

def load_rows(path):
    """A CSV from the oracle as row dicts, minus its provenance comments."""
    with open(path) as handle:
        return list(csv.DictReader(
            line for line in handle if not line.startswith("#")))


def header_lines(path):
    """The `#` provenance lines an oracle CSV opens with."""
    with open(path) as handle:
        return [line.rstrip("\n") for line in handle
                if line.startswith("#")]


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def load_ram_map(path):
    """{name: field} from `oracle/ram_map.json`."""
    with open(path) as handle:
        fields = json.load(handle)["fields"]
    return {field["name"]: field for field in fields}


class Log:
    """The RAM log, with each column read at the width the map gives it."""

    def __init__(self, rows, ram_map):
        self.rows = rows
        self.map = ram_map
        self.by_frame = {}
        for row in rows:
            self.by_frame[int(row["frame"])] = row

    def raw(self, frame, name):
        """The column's text, as the host wrote it."""
        row = self.by_frame.get(frame)
        if row is None:
            raise FixtureError(f"the log has no frame {frame}")
        if name not in row:
            raise FixtureError(f"the log has no column {name!r}; rerun the "
                               f"oracle with the group that carries it")
        return row[name]

    def num(self, frame, name):
        """The column as an integer, honouring its hex/dec rendering."""
        text = self.raw(frame, name)
        return int(text, 16) if self.map[name].get("hex") else int(text)

    def signed(self, frame, name):
        """A column the cartridge stores as a signed word (HP goes below 0)."""
        value = self.num(frame, name)
        return value - 0x10000 if value > 0x7FFF else value

    def changed(self, frame, name):
        """Whether the column differs from the *previous frame in the log*.

        A missing previous frame is not a change: the caller is looking for the
        frame something moved, and with a hole in the log there is nothing to
        compare against. `require_complete` is what keeps that from hiding a
        real move.
        """
        previous = self.by_frame.get(frame - 1)
        if previous is None:
            return False
        return previous.get(name) != self.by_frame[frame].get(name)

    def require_complete(self, first, last):
        """Insist every frame between `first` and `last` has a row."""
        missing = [frame for frame in range(first, last + 1)
                   if frame not in self.by_frame]
        if missing:
            raise FixtureError(
                f"the log is missing {len(missing)} frame(s) between {first} "
                f"and {last} (first: {missing[0]}); a fixture needs a row for "
                f"every frame, because a change is read against the frame "
                f"before it")

# --------------------------------------------------------------------------
# Rolls
# --------------------------------------------------------------------------

def roll_high_word(row):
    """The cartridge's roll: `sub.w (RNG_Seed).w, d0` reads $FFFFEF0C."""
    seed = int(row["seed_before"], 16)
    return (int(row["hv"], 16) + int(row["frame_count"])
            - ((seed >> 16) & M16)) & M16


def roll_low_word(row):
    """The subtraction the trace's own `roll` column performs ($FFFFEF0E)."""
    seed = int(row["seed_before"], 16)
    return (int(row["hv"], 16) + int(row["frame_count"])
            - (seed & M16)) & M16


def roll_column_report(rows):
    """How the trace's `roll` column relates to both derivations."""
    report = {"agrees": 0, "subtracts_low_word": 0, "neither": 0}
    for row in rows:
        logged = int(row["roll"], 16)
        if logged == roll_high_word(row):
            report["agrees"] += 1
        elif logged == roll_low_word(row):
            report["subtracts_low_word"] += 1
        else:
            report["neither"] += 1
    if report["neither"]:
        raise FixtureError(
            f"{report['neither']} trace row(s) carry a roll that is neither "
            f"(hv + frame_count - high word) nor (hv + frame_count - low word); "
            f"the trace does not describe UpdateRNGSeed2")
    return report


def rolls_in_window(rows, first, last):
    """[(frame, roll)] for the trace rows inside the battle's frame window."""
    return [(int(row["frame"]), roll_high_word(row)) for row in rows
            if first <= int(row["frame"]) <= last]


def group_by_frame(rolls):
    """[(frame, [roll, ...])] in frame order, frames ascending and unique."""
    frames = []
    for frame, _ in rolls:
        if not frames or frames[-1] != frame:
            frames.append(frame)
    return [(frame, [roll for f, roll in rolls if f == frame]) for frame in frames]


# --------------------------------------------------------------------------
# Observations
# --------------------------------------------------------------------------

def stat_block(log, frame, prefix, names):
    """{field: value} for one fighter's columns at `frame`."""
    return {name: log.num(frame, f"{prefix}_{name}") for name in names}


PARTY_NAMES = ["level", "hp", "maxhp", "tp", "maxtp", "status", "str",
               "agi", "agi_bat", "dex", "atk", "dfs", "men", "exp"]
PARTY_IDS = [("alys", 1), ("chaz", 2), ("hahn", 3)]
ENEMY_NAMES = ["id", "hp", "maxhp", "status", "agi_bat", "atk", "dfs",
               "str_bat", "men_bat", "dex_bat"]


def battle_start(log, frame):
    """The party's and enemies' live state at the battle's first frame."""
    party = []
    for prefix, index in PARTY_IDS:
        block = stat_block(log, frame, prefix, PARTY_NAMES)
        party.append({
            "id": index,
            "name": prefix.upper(),
            "level": block["level"],
            "hp": block["hp"],
            "max_hp": block["maxhp"],
            "tp": block["tp"],
            "max_tp": block["maxtp"],
            "status": block["status"],
            "strength": block["str"],
            "mental": block["men"],
            "agility": block["agi_bat"],
            "dexterity": block["dex"],
            "attack": block["atk"],
            "defence": block["dfs"],
            "experience": block["exp"],
        })
    enemies = []
    for slot in range(1, 5):
        enemy_id = log.num(frame, f"e{slot}_id")
        max_hp = log.num(frame, f"e{slot}_maxhp")
        if not enemy_id or not max_hp:
            continue
        block = stat_block(log, frame, f"e{slot}", ENEMY_NAMES)
        enemies.append({
            "formation_slot": slot,
            "id": slot + 5,
            "enemy_id": block["id"],
            "hp": block["hp"],
            "max_hp": block["maxhp"],
            "status": block["status"],
            "strength": block["str_bat"],
            "mental": block["men_bat"],
            "agility": block["agi_bat"],
            "dexterity": block["dex_bat"],
            "attack": block["atk"],
            "defence": block["dfs"],
        })
    return party, enemies


def turn_order(log, frame):
    """`Battle_Turn_Order` at `frame`: [(fighter id, ordering value)]."""
    order = []
    for index in range(0, 12, 2):
        fighter = log.num(frame, f"turn_{index:02d}")
        if fighter:
            order.append((fighter, log.num(frame, f"turn_{index + 1:02d}")))
    return order


def round_frames(log, first, last):
    """The frames where `Battle_Turn_Order` is (re)filled: the round starts."""
    starts = []
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        if any(log.changed(frame, f"turn_{i:02d}") for i in range(0, 12)):
            starts.append(frame)
    return starts


def action_windows(log, first, last):
    """[(actor, start, end)]: one per change of `battle_actor`.

    `$FFFF4142` holds a one-based fighter index only while the turn engine is
    driving an action; the command phase leaves other numbers in it, so a
    window opens only on a value that names a fighter slot.
    """
    windows = []
    actor = None
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        current = log.num(frame, "battle_actor")
        if not 1 <= current <= 9:
            continue
        if current != actor:
            if windows:
                windows[-1][2] = frame - 1
            windows.append([current, frame, last])
            actor = current
    return [tuple(window) for window in windows]



#: The roll table's columns, in order. One row per call: `[frame, roll, role,
#: target, pass, round, action]`. `role` is what the call was for - `priority`,
#: `order`, `hit`, `damage`, `ability`, `ability_reroll`, `formation` or
#: `item_drop` - `target` the fighter id a hit or damage call was aimed at (and
#: `null` otherwise), `pass` which `loc_B6A2` pass a hit roll belongs to, and
#: `round`/`action` the round and acting fighter it sits under. How each role is
#: derived is `assign_roles`'s subject, in `build_fixture`.
ROLL_COLUMNS = ["frame", "roll", "role", "target", "pass", "round", "action"]

#: `loc_266C` clamps every stored damage to this range, so a
#: `Battle_Heal_Damage_List` word outside it was not written by a damage call:
#: `loc_B6A2`'s nine-word clear leaves `$FFFF` over the first four words, which
#: is why a party action reads `$FFFF` in the party's own slots.
DAMAGE_STORED = (1, 999)


def side_of(fighter_id):
    """`cmpi.w #5, d0 / bgt`: ids 1..=5 are the party, 6..=9 the enemies."""
    return "party" if fighter_id <= 5 else "enemy"


def action_record(log, actor, start, end, rolls, occupied):
    """One action: who acted, what the log shows, and the rolls it drew.

    Only the acting side's opponents can be its targets - and only the ones
    the formation actually seated, since an empty slot holds zeros that read as
    a corpse - which is what keeps `loc_B6A2`'s blanking of the hit flags
    (`$FF` over every slot, `$FFFF` over the first four damage words) from
    reading as an observation about the actor's own side.
    """
    hit_frame = next(
        (frame for frame in range(start, end + 1)
         if frame in log.by_frame
         and any(log.changed(frame, f"hit_{i:02d}") for i in range(0, 10))),
        None)
    opposing = "enemy" if side_of(actor) == "party" else "party"
    targets = []
    for id_ in range(1, 10):
        if side_of(id_) != opposing or id_ not in occupied:
            continue
        flag = f"hit_{id_ - 1:02d}"
        damage = f"dmg_{id_ - 1:02d}"

        def stored(frame):
            value = log.signed(frame, damage)
            return value if DAMAGE_STORED[0] <= value <= DAMAGE_STORED[1] else None

        moved_flag = any(log.changed(frame, flag)
                         for frame in range(start, end + 1)
                         if frame in log.by_frame)
        damage_frame = next(
            (frame for frame in range(start, end + 1)
             if frame in log.by_frame and stored(frame) is not None
             and stored(frame - 1) != stored(frame)), None)
        if not moved_flag and damage_frame is None:
            continue
        hp = log.signed(end, hp_column(id_))
        targets.append({
            "id": id_,
            "hit": log.raw(hit_frame or start, flag),
            "damage": stored(damage_frame) if damage_frame else None,
            "damage_frame": damage_frame,
            "hp_after": hp,
            "died": hp <= 0,
        })
    return {
        "actor": actor,
        "start_frame": start,
        "end_frame": end,
        "hit_frame": hit_frame,
        "rolls": [list(entry) for entry in rolls],
        "roll_count": sum(count for _, count in rolls),
        "targets": targets,
    }


#: The log maps three named party members and four numbered enemy slots, so
#: the one-based fighter ids that have an HP column are 1..=3 and 6..=9.
HP_COLUMNS = {1: "alys_hp", 2: "chaz_hp", 3: "hahn_hp",
              6: "e1_hp", 7: "e2_hp", 8: "e3_hp", 9: "e4_hp"}


def hp_column(fighter_id):
    """The log's HP column for a one-based fighter id, when it has one."""
    return HP_COLUMNS.get(fighter_id)


def enemy_slots(log, frame):
    """The one-based fighter ids of the enemies the log shows at `frame`."""
    slots = []
    for slot in range(1, 5):
        enemy_id = log.num(frame, f"e{slot}_id")
        hp = log.num(frame, f"e{slot}_hp")
        max_hp = log.num(frame, f"e{slot}_maxhp")
        if enemy_id and max_hp and 0 < hp <= max_hp:
            slots.append(slot + 5)
    return slots


def enemies_loaded(log, first, last):
    """The frame the formation was written into RAM: the enemies' first HP.

    Before it the enemy slots hold whatever the field left there - tape 07's
    read 12063 over a maximum of 46 - so the test is a full, occupied record,
    not a nonzero byte.
    """
    for frame in range(first, last + 1):
        if frame in log.by_frame and log.num(frame, "enemy_count") > 0 \
                and enemy_slots(log, frame):
            return frame
    raise FixtureError(f"no formation is loaded between {first} and {last}")


def wiped_out(log, slots, first, last):
    """The frame the last of `slots` reads zero or less, else `last`."""
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        if all(log.signed(frame, hp_column(slot)) <= 0 for slot in slots):
            return frame
    return last


def compact_leaf_arrays(text):
    """Join every array that holds only scalars onto one line.

    `json.dump(indent=1)` puts each element of each array on its own line, which
    turns a 134-row table into a thousand-line file. The data is untouched -
    only the layout of a leaf array changes, and a leaf array is exactly the
    shape a table's rows have. Strings are tracked so a bracket inside one is
    text and not nesting.
    """
    lines, out, index = text.split("\n"), [], 0
    while index < len(lines):
        line = lines[index]
        out.append(line)
        if line.rstrip().endswith("["):
            depth, block, in_string, escaped = 1, [], False, False
            for offset in range(index + 1, len(lines)):
                candidate = lines[offset]
                for character in candidate:
                    if in_string:
                        if escaped:
                            escaped = False
                        elif character == "\\":
                            escaped = True
                        elif character == '"':
                            in_string = False
                    elif character == '"':
                        in_string = True
                    elif character == "[":
                        depth += 1
                    elif character == "]":
                        depth -= 1
                block.append(candidate)
                if depth == 0:
                    break
            body = [entry.strip() for entry in block[:-1]]
            if body and not any(entry.startswith(("{", "[")) for entry in body):
                joined = ", ".join(entry.rstrip(",") for entry in body)
                # The closing line carries the member's own comma, if any.
                tail = block[-1].strip()[1:]
                out[-1] = f"{line.rstrip()} {joined} ]{tail}"
                index += len(block) + 1
                continue
        index += 1
    return "\n".join(out)


def build_fixture(trace_rows, log, ram_map, first, last, meta):
    """The whole fixture, from the two logs and the extraction's metadata."""
    log.require_complete(first, last)
    roll_column = roll_column_report(trace_rows)
    frames = group_by_frame(rolls_in_window(trace_rows, first, last))
    if not frames:
        raise FixtureError(f"the trace has no rolls in {first}..{last}")
    start_frame = enemies_loaded(log, first, last)
    party, enemies = battle_start(log, start_frame)
    starts = round_frames(log, start_frame, last)
    if not starts:
        raise FixtureError("Battle_Turn_Order never changes: no round was found")

    def round_of(frame):
        found = [index for index, round_frame in enumerate(starts, start=1)
                 if round_frame <= frame]
        return found[-1] if found else 0

    # Every fighter the formation seated, enemies included: an empty enemy
    # slot holds zeros, which would otherwise read as a corpse.
    occupied = {entry["id"] for entry in party} | {entry["id"] for entry in enemies}

    windows = []
    for actor, start, end in action_windows(log, start_frame, last):
        # A round's order pass ends the previous round: an action cannot own
        # the frames that belong to the next round's queue build. The last
        # action ends where the party has finished wiping the field out.
        next_round = next((frame for frame in starts if frame > start), None)
        if next_round is not None:
            end = min(end, next_round - 1)
        windows.append((actor, start, end))
    action_end = wiped_out(log, [entry["id"] for entry in enemies],
                           windows[-1][1], last)
    windows[-1] = (windows[-1][0], windows[-1][1],
                   min(windows[-1][2], action_end))

    # The actions, with the observations that label the rolls. A round owns
    # the windows that start inside it.
    records = []
    for actor, start, end in windows:
        action_rolls = [(frame, len(values)) for frame, values in frames
                        if start <= frame <= end]
        record = action_record(log, actor, start, end, action_rolls, occupied)
        record["round"] = round_of(start)
        record["living_opponents"] = [
            id_ for id_ in sorted(occupied)
            if side_of(id_) != side_of(actor)
            and log.signed(start, hp_column(id_)) > 0]
        records.append(record)

    # Roles, in the trace's own order: the priority draw, each round's order
    # pass, each action's calls, and - outside the battle proper - the
    # encounter's formation draw and the post-victory item drop draw.
    def label(record, frame, values):
        """What each of one action's calls was for.

        The trace says how many calls a frame held, not what they were for, so
        the roles come from the cartridge's own structure plus what the RAM log
        then shows:

        * a frame of exactly sixteen calls - or of several runs of sixteen - is
          `Battle_CalculateDamage` (`ps4.asm:17381`: `moveq #$F, d7` with
          `dbf`), one run per target, and the runs follow the acting side's
          fighters in slot order;
        * a party action's other calls are `loc_B6A2`'s hit pass
          (`ps4.asm:17492`), one `Battle_CalculateChances` per target the swing
          covers, and `AlysKyraAttack_Init` (`ps4.asm:13975`) runs that pass a
          second time for the swing it animates;
        * an enemy's calls are `Enemy_Attack`'s ability roll (`ps4.asm:19146`),
          repeated while it equals `$FFFFEEA8`, and then the hit pass.

        A count that matches none of those shapes is labelled `action` with no
        target, so a reader can see that nothing was claimed about it.
        """
        calls = [entry for entry in record["rolls"] if entry[0] == frame]
        count = calls[0][1] if calls else 0
        targets = [target["id"] for target in record["targets"]
                   if target["hit"] != HIT_NOT_TARGETED or target["damage"]]
        living = record["living_opponents"]
        if count and count % DAMAGE_RUN == 0:
            # The damage runs follow the acting side's fighters in slot order,
            # so the k-th run belongs to the k-th target the action resolved.
            # One frame can hold several: a two-enemy swing runs the routine
            # once per target, in the same frame.
            seen = sum(other[1] for other in record["rolls"]
                       if other[0] < frame) // DAMAGE_RUN
            labels = []
            for index in range(count // DAMAGE_RUN):
                position = seen + index
                labels += [("damage",
                            targets[position] if position < len(targets) else None,
                            0)] * DAMAGE_RUN
            return labels
        if record["actor"] <= 5:
            # loc_B6A2 rolls once per target the swing covers - every living
            # opponent for a multi-target weapon, the cursor's target for a
            # single-target one - and `AlysKyraAttack_Init` runs the pass a
            # second time for the swing it animates. The targets are the ones
            # the log then shows that action resolving.
            covered = targets if targets else living
            if count == len(covered) or count == 2 * len(covered):
                return [("hit", covered[index % len(covered)],
                         1 + index // len(covered)) for index in range(count)]
            return [("action", None, 0)] * len(values)
        # Enemy_Attack: the ability roll, repeated while it equals the word the
        # routine compares against, then the hit pass.
        labels = [("ability", None, 0)]
        labels += [("ability_reroll", None, 0)] * (count - 2)
        labels += [("hit", targets[0] if targets else None, 1)]
        return labels[:count] if count else []

    labelled = []
    for frame, values in frames:
        if frame < starts[0]:
            role = "formation" if frame < start_frame else "priority"
            labelled.append((frame, values, [(role, None, 0)] * len(values), 0, 0))
            continue
        if frame > action_end:
            labelled.append((frame, values,
                             [("item_drop", None, 0)] * len(values), 0, 0))
            continue
        for record in records:
            if record["start_frame"] <= frame <= record["end_frame"]:
                labelled.append((frame, values, label(record, frame, values),
                                 record["round"], record["actor"]))
                break
        else:
            labelled.append((frame, values,
                             [("order", None, 0)] * len(values),
                             round_of(frame), 0))

    rolls, outside = [], []
    for frame, values, labels, number, actor in labelled:
        target = outside if labels[0][0] in ("formation", "item_drop") else rolls
        for roll, (role, hit, pass_number) in zip(values, labels):
            target.append([frame, roll, role, hit, pass_number, number, actor])

    rounds = []
    for number, round_frame in enumerate(starts, start=1):
        actions = []
        for record in records:
            if record["round"] != number:
                continue
            record["rolls"] = [[frame, count] for frame, count in record["rolls"]]
            actions.append(record)
        order = turn_order(log, round_frame)
        rounds.append({
            "round": number,
            "order_frame": round_frame,
            "order": [fighter for fighter, _ in order],
            "ordering": [value for _, value in order],
            "commands": [{"id": fighter, "command": "attack"}
                         for fighter, _ in order if fighter <= 3],
            "roll_count": sum(len(values) for _, values, _, n, _
                              in labelled if n == number),
            "actions": actions,
        })
    if any(not round_["actions"] for round_ in rounds):
        raise FixtureError("a round has no action: the windows and the queue "
                           "fills disagree")

    rewards_frame = next(
        (frame for frame in range(action_end, last + 1)
         if frame in log.by_frame
         and log.num(frame, "battle_exp_total")
         == log.num(last, "battle_exp_total")), action_end)
    return {
        "format_version": 1,
        "provenance": {
            **meta,
            "battle_frames": [first, last],
            "start_frame": start_frame,
            "action_end_frame": action_end,
            "rewards_frame": rewards_frame,
            "round_frames": starts,
            "roll_frames": [frame for frame, _ in frames],
            "roll_count": sum(len(values) for _, values in frames),
            "battle_roll_count": len(rolls),
            "outside_roll_count": len(outside),
            "outside_roll_summary": [
                f"{row[2]} at frame {row[0]}" for row in outside],
            "roll_convention": (
                "roll = (hv + frame_count - the word at $FFFFEF0C) & $FFFF; "
                "the subtrahend is the seed longword's high half, which is "
                "what a 68000 reads at (RNG_Seed).w"),
            "roll_column": roll_column,
            "damage_run": DAMAGE_RUN,
            "undetermined": [
                "command identity: the log has menu cursors, not the chosen "
                "commands; every party action here is a physical attack and "
                "the tape holds C, so each is recorded as `attack`",
                "a miss and an untargeted slot both read $FF in "
                "Fighters_Hit_Flags, so a $FF slot with an unmoved damage "
                "word is reported raw rather than guessed",
                "a damage word rewritten to the value it already held is "
                "invisible, so `damage_frame: null` means unobserved, not "
                "unwritten",
                "hp is sampled once a frame and retail subtracts inside the "
                "animation state machine, so a slot's HP reading is the value "
                "at the end of its action's window",
            ],
        },
        "formation": {
            "ambush_chance": log.num(start_frame, "enemy_ambush_chance"),
            "run_chance": log.num(start_frame, "enemy_run_chance"),
            "drop_rate": log.num(start_frame, "item_drop_rate"),
            "drop_item": log.num(start_frame, "dropped_item"),
            "enemy_count": log.num(start_frame, "enemy_count"),
            "priority": log.num(start_frame, "battle_priority"),
            "enemies": enemies,
        },
        "party": party,
        "rolls": {"columns": ROLL_COLUMNS, "rows": rolls},
        "outside_rolls": {"columns": ROLL_COLUMNS, "rows": outside},
        "rounds": rounds,
        "outcome": {
            "victory": bool(enemies) and all(
                log.signed(action_end, hp_column(entry["id"])) <= 0
                for entry in enemies),
            "dead_enemy_ids": [entry["id"] for entry in enemies if
                               log.signed(action_end,
                                          hp_column(entry["id"])) <= 0],
            "experience_total": log.num(rewards_frame, "battle_exp_total"),
            "meseta": log.num(rewards_frame, "battle_meseta_total"),
            "victory_frame": action_end,
        },
    }


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------

def default_ram_map():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)),
                        "ram_map.json")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--trace", required=True,
                        help="CSV from psiv_oracle --rng-trace")
    parser.add_argument("--log", required=True,
                        help="RAM log CSV from the same run")
    parser.add_argument("--out", required=True, help="fixture JSON to write")
    parser.add_argument("--ram-map", default=default_ram_map())
    parser.add_argument("--tape", default="oracle/tapes/07_first_battle.tape")
    parser.add_argument("--core", default="Genesis Plus GX 2d7131c "
                                         "(libretro/Genesis-Plus-GX)")
    parser.add_argument("--patch", default="oracle/patches/0001-rng-hv-trace.patch")
    parser.add_argument("--battle-first", type=int, default=BATTLE_FIRST)
    parser.add_argument("--battle-last", type=int, default=BATTLE_LAST)
    arguments = parser.parse_args(argv)

    trace_rows = load_rows(arguments.trace)
    log = Log(load_rows(arguments.log), load_ram_map(arguments.ram_map))
    meta = {
        "tape": arguments.tape,
        "core": arguments.core,
        "patch": arguments.patch,
        "trace": os.path.basename(arguments.trace),
        "trace_sha256": sha256(arguments.trace),
        "log_sha256": sha256(arguments.log),
        "trace_header": header_lines(arguments.trace),
        "log_header": header_lines(arguments.log),
    }
    fixture = build_fixture(trace_rows, log, load_ram_map(arguments.ram_map),
                            arguments.battle_first, arguments.battle_last, meta)
    with open(arguments.out, "w") as handle:
        handle.write(compact_leaf_arrays(
            json.dumps(fixture, indent=1, sort_keys=False)))
        handle.write("\n")
    # Re-read what was written: the layout pass must not have changed the data.
    with open(arguments.out) as handle:
        if json.load(handle) != fixture:
            raise FixtureError(f"{arguments.out} does not read back as the "
                               f"fixture that was built")

    print(f"wrote {arguments.out}")
    print(f"  battle frames   {fixture['provenance']['battle_frames']}")
    print(f"  rounds          {len(fixture['rounds'])} "
          f"at {fixture['provenance']['round_frames']}")
    print(f"  rolls           {len(fixture['rolls']['rows'])} kept, "
          f"{fixture['provenance']['roll_column']}")
    for round_ in fixture["rounds"]:
        print(f"    round {round_['round']}: order {round_['order']} "
              f"({round_['roll_count']} rolls) "
              f"{len(round_['actions'])} action(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
