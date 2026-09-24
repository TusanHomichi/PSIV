"""Assembling one fixture: the start state, the stream, and what it did.

The fixture is the smallest set of numbers a replay needs - the battle's start
state, the cartridge's rolls in the order it drew them, the commands the party
issued, and what the cartridge's RAM says each action did - and every number in
it is transcribed from the two logs. Nothing is fitted to `psiv-core`, so a
replay that disagrees with a fixture is a finding about the port.

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

# What the log can and cannot decide

Fields the log decides outright: the party's and enemies' live stats, HP, TP
and status; the turn order each round; `Battle_Priority`; every roll's frame;
every `Fighters_Hit_Flags` byte and `Battle_Heal_Damage_List` word as they are
written; the experience and meseta totals; the drop rate; an enemy's executed
ability id (`eN_ability`); and, in a vehicle battle, the party side's HP.

Fields it does not, recorded here as `"undetermined"` notes:

* **Command identity.** The log carries menu cursors (`battle_main_option`,
  `battle_char_comd_idx`), not the command each member chose. Every party
  action observed in these battles is a physical attack (damage lands on enemy
  slots with no status or TP movement), and the tapes hold C through the
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
* **The ability's class.** The log names the id an enemy executed and what it
  left behind - damage on one slot, on every slot, or nothing at all - but not
  the arm that produced it; `kind` is that reading and nothing more
  (`oracle/fixture/enemies.py`).
"""
from . import enemies as enemy_readings, roles, vehicle as vehicles
from .errors import FixtureError
from .observations import (ROLL_COLUMNS, action_record, action_windows,
                           battle_start, decided_frame, enemies_loaded,
                           round_frames, side_of, turn_order)
from .rolls import (DAMAGE_RUN, HIT_NOT_TARGETED, group_by_frame,
                    roll_column_report, rolls_in_window)


def build_fixture(trace_rows, log, ram_map, first, last, meta):
    """The whole fixture, from the two logs and the extraction's metadata."""
    log.require_complete(first, last)
    roll_column = roll_column_report(trace_rows)
    frames = group_by_frame(rolls_in_window(trace_rows, first, last))
    if not frames:
        raise FixtureError(f"the trace has no rolls in {first}..{last}")
    start_frame = enemies_loaded(log, first, last)
    party, enemies = battle_start(log, start_frame)
    vehicle = vehicles.vehicle_of(log, start_frame)
    if vehicle is not None:
        # The members' columns are the field's: nothing loads them in a
        # vehicle battle, and `oracle/fixture/vehicle.py` carries the party
        # side instead.
        party = []
    columns = vehicles.hp_columns(vehicle)
    starts = round_frames(log, start_frame, last)
    if not starts:
        raise FixtureError("Battle_Turn_Order never changes: no round was found")
    # The opening draw - `loc_B62A`'s, between the formation load and the first
    # round. Its frame is what says what `Battle_Priority` ended up as: the cell
    # is written *by* that draw, so the frame the formation was loaded on still
    # holds whatever the field left there (`$00` for an ambushed battle, which
    # is not what the battle ran with).
    opening = [frame for frame, _ in frames if start_frame <= frame < starts[0]]
    if len(opening) > 1:
        raise FixtureError(
            f"the trace holds {len(opening)} call frames between the formation "
            f"load at f{start_frame} and the first round at f{starts[0]}: the "
            f"battle opens on at most one draw")
    # A battle fought mounted draws no opening roll at all: the Desrt Leach
    # capture's only call before round 1 is the formation draw, and
    # `Battle_Priority` stays `$00` (Normal) throughout. The port's own
    # `Battle::start_vehicle` still draws one, which is a divergence the
    # fixture records rather than papers over.
    priority_frame = opening[0] if opening else None

    def round_of(frame):
        found = [index for index, round_frame in enumerate(starts, start=1)
                 if round_frame <= frame]
        return found[-1] if found else 0

    # Every fighter the formation seated, enemies included: an empty enemy
    # slot holds zeros, which would otherwise read as a corpse. A vehicle
    # battle's party side is its fighter alone.
    occupied = {entry["id"] for entry in party} | {entry["id"] for entry in enemies}
    if vehicle is not None:
        occupied = {vehicle["fighter_id"]} | {entry["id"] for entry in enemies}

    # A round's order pass ends the previous round: an action cannot own the
    # frames that belong to the next round's queue build, and an actor that
    # acts last in one round and first in the next is two actions, not one.
    windows = action_windows(log, start_frame, last, cuts=starts,
                             roll_frames=[frame for frame, _ in frames])
    if not windows:
        raise FixtureError(f"no action starts between {start_frame} and {last}")

    # The battle ends when one side is wiped out - the party side's wipe is a
    # defeat - so the frames after that belong to the game-over or victory
    # sequence, not to an action.
    party_side = ([vehicle["fighter_id"]] if vehicle is not None
                  else [entry["id"] for entry in party])
    decided = decided_frame(log, [entry["id"] for entry in enemies],
                            party_side, windows[-1][1], last, columns)
    action_end = decided if decided is not None else last
    windows[-1] = (windows[-1][0], windows[-1][1],
                   min(windows[-1][2], action_end))

    beyond = [frame for frame, _ in frames if frame > action_end]
    if beyond and not _won(log, enemies, action_end, columns):
        # A won battle is followed by the item drop's draw, which the fixture
        # keeps outside the battle's stream. A battle that was not won has no
        # such call: anything drawn after it is the game-over sequence, and it
        # is not this extractor's to label.
        raise FixtureError(
            f"the trace draws at f{beyond[0]} after the battle was decided at "
            f"f{action_end} and no enemy was defeated: the game-over sequence "
            f"is not a battle action")

    # The actions, with the observations that label the rolls. A round owns
    # the windows that start inside it.
    animation_pass = roles.animation_hit_pass_actors(party)
    records = []
    for actor, start, end in windows:
        action_rolls = [(frame, len(values)) for frame, values in frames
                        if start <= frame <= end]
        record = action_record(log, actor, start, end, action_rolls, occupied,
                              columns)
        record["round"] = round_of(start)
        record["living_opponents"] = [
            id_ for id_ in sorted(occupied)
            if side_of(id_) != side_of(actor)
            and log.signed(start, columns[id_]) > 0]
        record["animation_pass"] = animation_pass
        record["passes_done"] = 0
        if side_of(actor) == "enemy":
            reading = enemy_readings.ability_used(log, actor - 5, start, end)
            record["ability"] = reading["ability"]
            record["ability_frame"] = reading["ability_frame"]
            record["ability_written"] = reading["written"]
        else:
            # The command the member chose is not in the log; every party
            # action here is a physical attack and the tape holds C
            # (`docs/BATTLE_ORACLE_REPLAY.md`).
            record["ability"] = 0
            record["ability_frame"] = None
            record["ability_written"] = False
        record["kind"] = (enemy_readings.kind_of(
            record["ability"], roles.resolved_targets(record))
                          if side_of(actor) == "enemy" else "attack")
        records.append(record)

    # Roles, in the trace's own order: the priority draw, each round's order
    # pass, each action's calls, and - outside the battle proper - the
    # encounter's formation draw and the post-victory item drop draw.
    labelled = []
    won = _won(log, enemies, action_end, columns)
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
                labels = roles.label_calls(record, frame, len(values))
                # A call site's pass numbers follow the action's earlier
                # frames: one frame holds a whole number of passes.
                record["passes_done"] = max(
                    record["passes_done"],
                    max((pass_number for _, _, pass_number in labels),
                        default=0))
                labelled.append((frame, values, labels,
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
            written = {key: value for key, value in record.items()
                       if key not in ("animation_pass", "passes_done")}
            written["rolls"] = [[frame, count]
                                for frame, count in record["rolls"]]
            actions.append(written)
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

    party_side_dead = [id_ for id_ in party_side
                       if log.signed(action_end, columns[id_]) <= 0]
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
            "priority_frame": priority_frame,
            "action_end_frame": action_end,
            "decided_frame": decided,
            "rewards_frame": rewards_frame,
            "round_frames": starts,
            "roll_frames": [frame for frame, _ in frames],
            "roll_count": sum(len(values) for _, values in frames),
            "battle_roll_count": len(rolls),
            "outside_roll_count": len(outside),
            "outside_roll_summary": [
                f"{row[2]} at frame {row[0]}" for row in outside],
            "vehicle_battle": vehicle is not None,
            "roll_convention": (
                "roll = (hv + frame_count - the word at $FFFFEF0C) & $FFFF; "
                "the subtrahend is the seed longword's high half, which is "
                "what a 68000 reads at (RNG_Seed).w, and every row of the "
                "trace's own roll column is checked against it"),
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
                "an enemy action's `kind` is the log's reading of the ability "
                "id and what it left behind (damage, or nothing), not the "
                "routine the cartridge ran: an ability whose effect no slot "
                "shows is `wasted` whether it spent the turn by the FloatMine "
                "fall-through or by resolving against a target the log cannot "
                "see",
            ] + ([] if vehicle is None else
                 ["a vehicle battle's fields: " + "; ".join(
                     vehicle["undetermined"])]),
        },
        "formation": {
            "ambush_chance": log.num(start_frame, "enemy_ambush_chance"),
            "run_chance": log.num(start_frame, "enemy_run_chance"),
            "drop_rate": log.num(start_frame, "item_drop_rate"),
            "drop_item": log.num(start_frame, "dropped_item"),
            "enemy_count": log.num(start_frame, "enemy_count"),
            "priority": log.num(priority_frame if priority_frame is not None
                                else start_frame, "battle_priority"),
            "enemies": enemies,
        },
        "party": party,
        # The party slots whose attack route runs the close-range animation's
        # hit pass as well as Character_Attack's (Alys and Kyra, by the name
        # the fixture's party list carries); empty for a vehicle battle, whose
        # fighter is neither.
        "animation_hit_pass_ids": sorted(animation_pass),
        **({} if vehicle is None else {"vehicle": vehicle}),
        "rolls": {"columns": ROLL_COLUMNS, "rows": rolls},
        "outside_rolls": {"columns": ROLL_COLUMNS, "rows": outside},
        "rounds": rounds,
        "outcome": {
            "victory": won,
            "defeat": bool(party_side) and len(party_side_dead) == len(party_side),
            "dead_enemy_ids": [entry["id"] for entry in enemies if
                               log.signed(action_end,
                                          columns[entry["id"]]) <= 0],
            "dead_party_ids": party_side_dead,
            "experience_total": log.num(rewards_frame, "battle_exp_total"),
            "meseta": log.num(rewards_frame, "battle_meseta_total"),
            "end_frame": action_end,
            # The older fixtures' name for the same frame; `end_frame` is the
            # one to read, because a defeat's end is not a victory.
            "victory_frame": action_end,
        },
    }


def _won(log, enemies, frame, columns):
    """Whether every enemy the formation seated is down at `frame`."""
    return bool(enemies) and all(
        log.signed(frame, columns[entry["id"]]) <= 0 for entry in enemies)
