"""What the RAM log shows: the fighters, the queue, and one action's effects.

Everything here reads columns the log actually carries, at the width
`oracle/ram_map.json` gives them. The one convention worth stating up front:
HP is a signed word, so a dead fighter reads 65511 for -25, and `Log.signed`
is what turns the rendering back into the number the cartridge holds.
"""
from .errors import FixtureError

#: `loc_266C` clamps every stored damage to this range, so a
#: `Battle_Heal_Damage_List` word outside it was not written by a damage call:
#: `loc_B6A2`'s nine-word clear leaves `$FFFF` over the first four words, which
#: is why a party action reads `$FFFF` in the party's own slots.
DAMAGE_STORED = (1, 999)

#: The roll table's columns, in order. One row per call: `[frame, roll, role,
#: target, pass, round, action]`. `role` is what the call was for - `priority`,
#: `order`, `hit`, `damage`, `ability`, `ability_reroll`, `formation` or
#: `item_drop` - `target` the fighter id a hit or damage call was aimed at (and
#: `null` otherwise), `pass` which `loc_B6A2` pass a hit roll belongs to, and
#: `round`/`action` the round and acting fighter it sits under. How each role
#: is derived is `oracle/fixture/roles.py`'s subject.
ROLL_COLUMNS = ["frame", "roll", "role", "target", "pass", "round", "action"]

PARTY_NAMES = ["level", "hp", "maxhp", "tp", "maxtp", "status", "str",
               "agi", "agi_bat", "dex", "atk", "dfs", "men", "exp"]
PARTY_IDS = [("alys", 1), ("chaz", 2), ("hahn", 3)]
ENEMY_NAMES = ["id", "hp", "maxhp", "status", "agi_bat", "atk", "dfs",
               "str_bat", "men_bat", "dex_bat"]

#: The log maps three named party members and four numbered enemy slots, so
#: the one-based fighter ids that have an HP column are 1..=3 and 6..=9. A
#: vehicle battle's party-side fighter is id 1 and its HP lives in another
#: column entirely (`vehicle_fighter_hp`); `oracle/fixture/vehicle.py` is that
#: section.
HP_COLUMNS = {1: "alys_hp", 2: "chaz_hp", 3: "hahn_hp",
              6: "e1_hp", 7: "e2_hp", 8: "e3_hp", 9: "e4_hp"}


def hp_column(fighter_id):
    """The log's HP column for a one-based fighter id, when it has one."""
    return HP_COLUMNS.get(fighter_id)


def side_of(fighter_id):
    """`cmpi.w #5, d0 / bgt`: ids 1..=5 are the party, 6..=9 the enemies."""
    return "party" if fighter_id <= 5 else "enemy"


def stat_block(log, frame, prefix, names):
    """{field: value} for one fighter's columns at `frame`."""
    return {name: log.num(frame, f"{prefix}_{name}") for name in names}


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
        max_hp = log.num(frame, f"e{slot}_maxhp")
        hp = log.num(frame, f"e{slot}_hp")
        # An empty slot holds zeros; enemy id `$00` is a real enemy (Helex),
        # so occupancy is the record holding together, not the id.
        if not max_hp or not 0 < hp <= max_hp:
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


#: The fighters `Battle_Turn_Order` holds: one id/agility pair per
#: `Obj_Fighters` slot. `Battle_OrderTurns` runs its insertion loop and its
#: agility passes over nine slots (`moveq #8, d7`, ps4.asm:7757 and 7789) and
#: sorts nine entries (ps4.asm:7793-7801), and `oracle/ram_map.json` logs all
#: nine pairs as `turn_00`..`turn_17`. A log written before that window existed
#: carries six pairs, and `turn_order` reads exactly the ones it has.
ORDER_ENTRIES = 9


def turn_order(log, frame):
    """`Battle_Turn_Order` at `frame`: [(fighter id, ordering value)].

    The queue, as far as the log's own columns go: a capture with seven or more
    fighters needs the window past `turn_11`, or the entries it cannot see are
    simply missing from the fixture (`docs/BATTLE_ORACLE_SWEEP.md`).
    """
    order = []
    for index in range(0, 2 * ORDER_ENTRIES, 2):
        if not log.has(f"turn_{index:02d}"):
            break
        fighter = log.num(frame, f"turn_{index:02d}")
        if fighter:
            order.append((fighter, log.num(frame, f"turn_{index + 1:02d}")))
    return order


def round_frames(log, first, last):
    """The frames where `Battle_Turn_Order` is (re)filled: the round starts."""
    starts = []
    columns = [f"turn_{index:02d}" for index in range(2 * ORDER_ENTRIES)]
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        if any(log.changed(frame, column) for column in columns):
            starts.append(frame)
    return starts


def action_windows(log, first, last, cuts=(), roll_frames=()):
    """[(actor, start, end)]: one per turn the turn engine ran.

    `$FFFF4142` holds a one-based fighter index only while the turn engine is
    driving an action; the command phase leaves other numbers in it, so a
    window opens only on a value that names a fighter slot.

    Two things the field alone cannot say, and the caller's `cuts` and
    `roll_frames` are what settle them:

    * **A round's queue build is a boundary.** An actor that acts last in one
      round and first in the next has the same id in the field throughout, and
      its two turns are still two actions: each cut closes the window that is
      open and the next one opens after it (`cuts` are the frames
      `Battle_Turn_Order` is filled on).
    * **The field holds the *previous* actor until the next turn starts.** The
      frames between a queue build and the first action of the round read the
      id of whoever acted last, so a window must not open on that value - the
      Desrt Leach capture's rounds 3-5 read `6` for five frames before the
      vehicle's turn, and an ability byte the enemy had left behind made one of
      them look like a wasted ability. A window opens on that id only when the
      action's own calls show up in it (`roll_frames` are the frames the trace
      drew in), which is also what says a turn that nothing happened in is not
      an action at all.
    """
    cuts = set(cuts)
    rolls = set(roll_frames)
    windows = []
    actor = None
    stale = None
    for frame in range(first, last + 1):
        row = log.by_frame.get(frame)
        if row is None:
            continue
        if frame in cuts:
            if windows:
                windows[-1][2] = min(windows[-1][2], frame - 1)
            actor = None
            stale = log.num(frame, "battle_actor")
            continue
        current = log.num(frame, "battle_actor")
        if not 1 <= current <= 9:
            continue
        if current == actor:
            continue
        if current != stale or frame in rolls:
            if windows:
                windows[-1][2] = min(windows[-1][2], frame - 1)
            windows.append([current, frame, last])
            actor = current
    return [tuple(window) for window in windows]


def enemy_slots(log, frame):
    """The one-based fighter ids of the enemies the log shows at `frame`.

    An occupied slot is one whose record holds together - a maximum and a
    current HP inside it - not one whose id byte is nonzero: enemy id `$00` is
    a real enemy (Helex, formation `$5E`), while an empty slot holds zeros and
    the field's leftovers hold whatever the last map left there (tape 07's
    slot 2 reads 6956 of 7680 before the formation lands, which is why the
    current HP has to be inside the maximum).
    """
    slots = []
    for slot in range(1, 5):
        hp = log.num(frame, f"e{slot}_hp")
        max_hp = log.num(frame, f"e{slot}_maxhp")
        if max_hp and 0 < hp <= max_hp:
            slots.append(slot + 5)
    return slots


def enemies_loaded(log, first, last):
    """The frame the formation was written into RAM: the enemies' first HP.

    Before it the enemy slots hold whatever the field left there - tape 07's
    slot 1 read 12063 over a maximum of 46 - so the test is a full, occupied
    record, not a nonzero byte (`enemy_slots`).
    """
    for frame in range(first, last + 1):
        if frame in log.by_frame and log.num(frame, "enemy_count") > 0 \
                and enemy_slots(log, frame):
            return frame
    raise FixtureError(f"no formation is loaded between {first} and {last}")


def wiped_out(log, slots, first, last, columns=None):
    """The frame the last of `slots` reads zero or less, else None.

    One side's wipe is what decides a battle: the enemies' wipe is a victory
    and the party side's is a defeat, so the frame is the battle's end either
    way and the tape's own tail (the game-over or victory sequence) is not part
    of it.

    `columns` is the fighter-id to HP-column map; a vehicle battle replaces the
    party side's slot 1 with `vehicle_fighter_hp`
    (`oracle/fixture/vehicle.py`), because the members' columns never move
    there.
    """
    columns = HP_COLUMNS if columns is None else columns
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        if all(log.signed(frame, columns[slot]) <= 0 for slot in slots):
            return frame
    return None


def decided_frame(log, enemy_ids, party_ids, first, last, columns=None):
    """The frame the battle's outcome was decided, else None.

    `party_ids` is empty for a vehicle battle, whose party side is the vehicle
    rather than the members: `oracle/fixture/vehicle.py` supplies the frame the
    vehicle is wrecked and this takes whichever side fell first.
    """
    frames = []
    if enemy_ids:
        frames.append(wiped_out(log, enemy_ids, first, last, columns))
    if party_ids:
        frames.append(wiped_out(log, party_ids, first, last, columns))
    seen = [frame for frame in frames if frame is not None]
    return min(seen) if seen else None


def hit_flag_column(fighter_id):
    """The log's `Fighters_Hit_Flags` column for a one-based fighter id.

    The array is indexed by **slot** (`loc_B6A2` writes `-$1(a0,d6.w)` for a
    one-based `d6`), so fighter id 1 reads `hit_00` and enemy slot 1 - id 6 -
    reads `hit_05`.
    """
    return f"hit_{fighter_id - 1:02d}"


def sample_decisive_hits(log, record, decisive_frame):
    """Re-read every target's hit byte at the swing's **decisive** pass.

    `loc_B6A2` presets all nine `Fighters_Hit_Flags` to `$FF` before it rolls
    (`ps4.asm:17493-17498`, `moveq #-1, d0` over nine words) and every pass
    walks the same window, so the byte a swing leaves behind is the one its
    **last** pass wrote - the one `Fighter_TakeDamage` reads
    (`ps4.asm:3569-3571`). [`action_record`] reads the byte at the first frame
    the flags moved, which is the *first* pass's verdict; for a swing whose
    passes arrive in different frames - `loc_AF9C`'s vehicle swing, which draws
    one pass per frame (two for the Ice Digger, three for the Land Rover and
    the Hydrofoil) - the two differ, and the action's own `hit_frame` is not the
    frame its byte came from. This re-reads it at `decisive_frame`, the frame of
    the action's last `loc_B6A2` roll.

    Nothing writes `Fighters_Hit_Flags` between that pass and the action's end -
    the next writer is another action's `loc_B6A2`, which is outside this
    action's window - so the byte is also what the first frame at or after the
    roll holds; `Log.require_complete` has already refused a log with a hole, so
    the roll's own frame is always there to read.
    """
    for target in record["targets"]:
        target["hit"] = log.raw(decisive_frame, hit_flag_column(target["id"]))


def action_record(log, actor, start, end, rolls, occupied, columns=None):
    """One action: who acted, what the log shows, and the rolls it drew.

    Only the acting side's opponents can be its targets - and only the ones
    the formation actually seated, since an empty slot holds zeros that read as
    a corpse - which is what keeps `loc_B6A2`'s blanking of the hit flags
    (`$FF` over every slot, `$FFFF` over the first four damage words) from
    reading as an observation about the actor's own side. `columns` is the
    fighter-id to HP-column map (see `wiped_out`).

    Each target's `hit` here is the byte at the first frame the flags moved,
    the *first* pass's verdict; `assembly` re-reads it at the action's last
    `loc_B6A2` roll with [`sample_decisive_hits`] once the rolls are labelled.
    """
    columns = HP_COLUMNS if columns is None else columns
    hit_frame = next(
        (frame for frame in range(start, end + 1)
         if frame in log.by_frame
         and any(log.changed(frame, hit_flag_column(index))
                 for index in range(0, 10))),
        None)
    opposing = "enemy" if side_of(actor) == "party" else "party"
    targets = []
    for id_ in range(1, 10):
        if side_of(id_) != opposing or id_ not in occupied:
            continue
        flag = hit_flag_column(id_)
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
        hp = log.signed(end, columns[id_])
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
