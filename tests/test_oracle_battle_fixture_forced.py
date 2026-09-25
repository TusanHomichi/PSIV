"""Tests for the extractor's forced-capture readings.

The hand-built rows come from `tests/test_oracle_battle_fixture.py`, which is
where the roll derivation and the party-battle extraction are pinned; this file
adds the columns and shapes a *forced* capture brought with it: an enemy's
executed ability, a vehicle battle's party side, a defeat, and the action
windows a round boundary cuts.

Nothing here starts the emulator. `tests/test_oracle_force_battle.py` pins the
capture side - the tape, the patches, the report - and
`tests/test_oracle_force_battle_provenance.py` the provenance lines a capture's
log opens with.
"""
import unittest

from oracle import fixture as fx
from tests.test_oracle_battle_fixture import (FixtureTest, LogBuilder, RAM_MAP,
                                             Row, cartridge_roll, trace_row)

#: The base map's fields, plus the ones the forced captures read: the four
#: enemy ability bytes (`enemy` group), `Vehicle_Index` and the saved record
#: (`vehicle` group), and the vehicle fighter's HP (`battle` group).
FORCED_MAP = {"fields": RAM_MAP["fields"] + [
    {"name": f"e{slot}_ability", "hex": True} for slot in range(1, 5)
] + [
    {"name": "vehicle_index", "hex": True},
    {"name": "vehicle_fighter_hp", "hex": False},
    {"name": "vehicle_land_hp", "hex": False},
    {"name": "vehicle_land_max_hp", "hex": False},
    {"name": "vehicle_land_skill_mask", "hex": False},
    {"name": "vehicle_land_skill1_current", "hex": False},
    {"name": "vehicle_land_skill1_max", "hex": False},
]}


class ForcedRow(Row):
    """A log row that also carries the forced captures' columns."""

    def __init__(self, **overrides):
        defaults = {f"e{slot}_ability": "00" for slot in range(1, 5)}
        defaults.update({
            "vehicle_index": "0000", "vehicle_fighter_hp": "0",
            "vehicle_land_hp": "0", "vehicle_land_max_hp": "0",
            "vehicle_land_skill_mask": "0",
            "vehicle_land_skill1_current": "0",
            "vehicle_land_skill1_max": "0",
        })
        defaults.update(overrides)
        super().__init__(**defaults)


class ForcedLog(LogBuilder):
    """The base log builder, starting from a [`ForcedRow`]."""

    def __init__(self):
        super().__init__()
        self.last = ForcedRow()


class ForcedFixture(FixtureTest):
    """The base fixture test, with the forced captures' columns."""

    HV, FRAME_COUNT, SEED = 0x2292, 0x708F, 0x21E817F3

    def log(self, rows):
        header = [entry["name"] for entry in FORCED_MAP["fields"]]
        return self.load(rows, header, ram_map=FORCED_MAP)

    def trace(self, frames):
        """Trace rows for {frame: call count}, rolls the cartridge's own."""
        rows = []
        for frame, count in frames.items():
            for index in range(count):
                hv = self.HV + frame
                rows.append(trace_row(frame, index, hv, self.FRAME_COUNT,
                                      self.SEED,
                                      cartridge_roll(hv, self.FRAME_COUNT,
                                                     self.SEED)))
        return rows

    def build(self, rows, frames, first, last, meta=None):
        return fx.build_fixture(self.trace(frames), self.log(rows),
                                dict(FORCED_MAP), first, last,
                                meta or {"tape": "t", "core": "c",
                                         "patch": "p", "trace": "x",
                                         "trace_sha256": "0" * 64,
                                         "log_sha256": "1" * 64})


class VehicleSection(ForcedFixture):
    def hand_built(self, **overrides):
        """Four frames: the formation load, the opening draw, a round, then an
        enemy action. `overrides` are applied to every frame."""
        builder = ForcedLog()
        builder.frame(20, enemy_count=0, **overrides)
        builder.frame(21, enemy_count=1, e1_id=81, e1_hp=1040, e1_maxhp=1040,
                      **overrides)
        builder.frame(22, turn_00=6, turn_01=20, **overrides)
        builder.frame(23, battle_actor=6, hit_00="00", dmg_00=57,
                      alys_hp=683, e1_hp=1040, **overrides)
        return builder.rows

    def test_a_vehicle_battle_is_read_from_the_index_and_the_saved_record(self):
        rows = self.hand_built(vehicle_index=1, vehicle_fighter_hp=740,
                               vehicle_land_hp=740, vehicle_land_max_hp=740,
                               vehicle_land_skill_mask=3,
                               vehicle_land_skill1_current=8,
                               vehicle_land_skill1_max=8)
        fixture = self.build(rows, {20: 1, 21: 1, 22: 13, 23: 17}, 20, 23)
        vehicle = fixture["vehicle"]
        self.assertEqual(vehicle["index"], 1)
        self.assertEqual(vehicle["name"], "Land Rover")
        self.assertEqual(vehicle["hp"], 740)
        # The saved record's own HP is the fighter's at the battle's first
        # frame, which is what makes its maximum the battle copy's too.
        self.assertEqual(vehicle["max_hp"], 740)
        self.assertEqual(vehicle["saved_record"]["skill_mask"], 3)
        self.assertEqual(vehicle["saved_record"]["skill1_current"], 8)
        self.assertEqual(vehicle["fighter_id"], fx.VEHICLE_FIGHTER_ID)
        # The members are the field's, so the fixture carries none of them.
        self.assertEqual(fixture["party"], [])
        self.assertTrue(fixture["provenance"]["vehicle_battle"])
        self.assertEqual(fixture["provenance"]["priority_frame"], 21)

    def test_the_fighters_hp_column_is_the_vehicles_own(self):
        rows = self.hand_built(vehicle_index=1)
        vehicle = fx.vehicle_of(self.log(rows), 21)
        self.assertIsNotNone(vehicle)
        columns = fx.hp_columns(vehicle)
        self.assertEqual(columns[1], "vehicle_fighter_hp")
        self.assertEqual(fx.hp_columns()[1], "alys_hp")

    def test_a_battle_on_foot_carries_no_vehicle_section(self):
        rows = self.hand_built(vehicle_index=0)
        fixture = self.build(rows, {20: 1, 21: 1, 22: 13, 23: 17}, 20, 23)
        self.assertNotIn("vehicle", fixture)
        self.assertFalse(fixture["provenance"]["vehicle_battle"])
        self.assertEqual([entry["id"] for entry in fixture["party"]], [1, 2, 3])

    def test_a_vehicle_battle_draws_no_opening_roll(self):
        # The Desrt Leach capture's only call before its first round is the
        # formation draw: its priority frame is null, and the value the log
        # holds is what the battle ran with.
        rows = self.hand_built(vehicle_index=1)
        fixture = self.build(rows, {20: 1, 22: 13, 23: 17}, 20, 23)
        self.assertIsNone(fixture["provenance"]["priority_frame"])
        self.assertEqual(fixture["formation"]["priority"], 0)


class EnemyAbilities(ForcedFixture):
    def hand_built(self, ability="02", **overrides):
        """An enemy's turn: one ability call, then sixteen damage draws."""
        builder = ForcedLog()
        builder.frame(20, enemy_count=1, e1_id=0, e1_hp=90, e1_maxhp=90,
                      **overrides)
        builder.frame(21, turn_00=6, turn_01=20, turn_02=1, turn_03=9,
                      **overrides)
        builder.frame(22, battle_actor=6, e1_ability=ability, hit_00="00",
                      dmg_00=78, alys_hp=0, **overrides)
        builder.frame(23, **overrides)
        return builder.rows

    def test_an_ability_action_is_the_byte_the_enemy_wrote(self):
        fixture = self.build(self.hand_built(), {20: 1, 21: 14, 22: 1, 23: 16},
                             20, 23)
        action = fixture["rounds"][0]["actions"][0]
        self.assertEqual(action["kind"], "ability")
        self.assertEqual(action["ability"], 2)
        self.assertEqual(action["ability_frame"], 22)
        self.assertTrue(action["ability_written"])
        # The ability's own arm never runs loc_B6A2's hit pass: the calls are
        # the ability roll and then the damage run.
        roles = [roll[2] for roll in fixture["rolls"]["rows"]
                 if roll[6] == 6]
        self.assertEqual(roles, ["ability"] + ["damage"] * 16)

    def test_the_ability_roll_that_must_re_roll_is_labelled_as_one(self):
        builder = ForcedLog()
        builder.frame(20, enemy_count=1, e1_id=0, e1_hp=90, e1_maxhp=90)
        builder.frame(21, turn_00=6, turn_01=20, turn_02=1, turn_03=9)
        builder.frame(22, battle_actor=6, e1_ability="02", hit_00="00",
                      dmg_00=78, alys_hp=0)
        builder.frame(23, alys_hp=0)
        # Two calls at the ability's frame: the draw of zero (the word is zero
        # when a battle loads, `ps4.asm:9992-9994`) and the re-roll.
        fixture = self.build(builder.rows, {20: 1, 21: 14, 22: 2, 23: 16},
                             20, 23)
        roles = [roll[2] for roll in fixture["rolls"]["rows"] if roll[6] == 6]
        self.assertEqual(roles[:2], ["ability", "ability_reroll"])

    def test_an_ability_that_leaves_nothing_behind_is_a_wasted_turn(self):
        # The FloatMine carriers' `$07`/`$17` spend the turn with no effect
        # (`loc_10406`, `ps4.asm:22781`): the byte moves and no slot does.
        builder = ForcedLog()
        builder.frame(20, enemy_count=1, e1_id=50, e1_hp=60, e1_maxhp=60)
        builder.frame(21, turn_00=6, turn_01=20, turn_02=1, turn_03=9)
        builder.frame(22, battle_actor=6, e1_ability="07")
        builder.frame(23)
        fixture = self.build(builder.rows, {20: 1, 21: 14, 22: 1, 23: 1},
                             20, 23)
        action = fixture["rounds"][0]["actions"][0]
        self.assertEqual(action["kind"], "wasted")
        self.assertEqual(action["ability"], 7)
        self.assertEqual(action["targets"], [])
        self.assertEqual(fixture["rolls"]["rows"][-2][2], "ability")

    def test_a_basic_attack_carries_no_ability(self):
        # The roll landed on one of the six empty slots: the byte reads zero
        # and the turn is the ordinary swing - `Enemy_Attack`'s ability roll,
        # then `loc_B6A2`'s hit pass, then the damage run.
        rows = self.hand_built(ability="00")
        fixture = self.build(rows, {20: 1, 21: 14, 22: 2, 23: 16}, 20, 23)
        action = fixture["rounds"][0]["actions"][0]
        self.assertEqual(action["kind"], "attack")
        self.assertEqual(action["ability"], 0)


class ActionWindows(ForcedFixture):
    """The frames a turn's window covers, and what must not open one."""

    def rows_for(self, actor_by_frame, round_frames=(), **overrides):
        builder = ForcedLog()
        for frame in sorted(actor_by_frame):
            overrides_here = dict(overrides.get(frame, {}))
            builder.frame(frame, battle_actor=actor_by_frame[frame],
                          **overrides_here)
        return builder.rows

    def test_a_round_boundary_splits_an_actor_that_acts_on_both_sides(self):
        # Frames 1-2: the vehicle's turn. Frame 3 is the next round's queue
        # build, and the vehicle acts again from frame 5 - the same id
        # throughout, and still two actions.
        rows = self.rows_for({1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1})
        windows = fx.action_windows(self.log(rows), 1, 6, cuts=[3],
                                    roll_frames=[1, 5])
        self.assertEqual(windows, [(1, 1, 2), (1, 5, 6)])

    def test_a_frame_the_queue_wrote_and_nothing_drew_is_no_window(self):
        """A skipped turn reads as the actor's id for the rest of the round.

        `loc_576A` writes the entry into `$FFFF4142` and only then tests the
        fighter's status, so an enemy that fell before its turn arrived leaves
        its id there until the next queue build - with no call behind it. That
        is the sweep's no-swing cluster: the port has no turn there either.
        """
        rows = self.rows_for({1: 1, 2: 7, 3: 7, 4: 7})
        windows = fx.action_windows(self.log(rows), 1, 4, roll_frames=[1])
        self.assertEqual(windows, [(1, 1, 4)])

    def test_a_stale_actor_id_opens_no_window_without_calls_of_its_own(self):
        # Frames 3-5 still read the *previous* round's last actor, and the
        # turn engine wrote it before testing whether the fighter could act
        # (`ps4.asm:8033-8043`): with nothing drawn there, they are the round's
        # tail. The next action is the other fighter's, and its own call is
        # what says so.
        rows = self.rows_for({1: 6, 2: 6, 3: 6, 4: 6, 5: 6, 6: 1})
        windows = fx.action_windows(self.log(rows), 1, 6, cuts=[3],
                                    roll_frames=[1, 6])
        self.assertEqual(windows, [(6, 1, 2), (1, 6, 6)])


class Outcome(ForcedFixture):
    def defeat_rows(self, extra=()):
        builder = ForcedLog()
        builder.frame(20, enemy_count=2, e1_id=0, e1_hp=90, e1_maxhp=90,
                      e2_id=0, e2_hp=90, e2_maxhp=90)
        builder.frame(21, turn_00=1, turn_01=20, turn_02=6, turn_03=9)
        builder.frame(22, battle_actor=1, hit_05="00", dmg_05=30, e1_hp=60)
        builder.frame(23, battle_actor=6, hit_00="00", dmg_00=53, alys_hp=0,
                      hit_01="00", dmg_01=25, chaz_hp=0)
        builder.frame(24, hit_02="00", dmg_02=21, hahn_hp=0)
        for frame in extra:
            builder.frame(frame, battle_actor=0)
        return builder.rows

    def test_a_party_wipe_is_a_defeat_with_its_own_frame(self):
        rows = self.defeat_rows()
        fixture = self.build(rows, {20: 1, 21: 13, 22: 1, 23: 1, 24: 17},
                             20, 24)
        outcome = fixture["outcome"]
        self.assertFalse(outcome["victory"])
        self.assertTrue(outcome["defeat"])
        self.assertEqual(outcome["dead_party_ids"], [1, 2, 3])
        self.assertEqual(outcome["end_frame"], 24)
        self.assertEqual(outcome["experience_total"], 0)

    def test_a_battle_that_was_not_won_may_not_draw_after_it_ends(self):
        rows = self.defeat_rows(extra=(25,))
        with self.assertRaises(fx.FixtureError) as raised:
            self.build(rows, {20: 1, 21: 13, 22: 1, 23: 1, 24: 17, 25: 16},
                       20, 25)
        self.assertIn("after the battle was decided", str(raised.exception))


class TruncatedCapture(ForcedFixture):
    """`--max-rounds` and `--hp-patch`: the two readings a sweep needs.

    The sweep's captures stop at a round boundary and start from a party the
    capture itself patched to 999 HP (`oracle/force/durable.py`), so a fixture
    has to say both: how many rounds it holds and that the battle was still
    running at the end, and that its start state's HP is the capture's rather
    than the tape's.
    """

    def build(self, rows, frames, first, last, meta=None, **extra):
        return fx.build_fixture(
            self.trace(frames), self.log(rows), dict(FORCED_MAP), first, last,
            meta or {"tape": "t", "core": "c", "patch": "p", "trace": "x",
                     "trace_sha256": "0" * 64, "log_sha256": "1" * 64},
            **extra)

    def hand_built(self, rounds=3, party=None):
        """`rounds` rounds: each opens with a queue build, then the party's
        swing at slot 1 and the enemy's reply at Alys."""
        party = party or {}
        builder = ForcedLog()
        builder.frame(20, enemy_count=0, **party)
        builder.frame(21, enemy_count=2, e1_id=10, e1_hp=25, e1_maxhp=25,
                      e2_id=10, e2_hp=25, e2_maxhp=25, **party)
        frame = 22
        for number in range(1, rounds + 1):
            builder.frame(frame, turn_00=number, turn_01=20, turn_02=1,
                          turn_03=9, **party)
            builder.frame(frame + 1, battle_actor=1, hit_05="00", dmg_05=3,
                          e1_hp=25 - number, **party)
            builder.frame(frame + 2, battle_actor=6, hit_00="00", dmg_00=2,
                          **party)
            frame += 3
        return builder.rows, frame

    def frames_for(self, rounds):
        """The trace: one draw before the battle, one for the queue build, and
        a hit plus sixteen damage draws per action."""
        frames = {20: 1, 21: 1}
        frame = 22
        for _ in range(rounds):
            frames[frame] = 13
            frames[frame + 1] = 17
            frames[frame + 2] = 17
            frame += 3
        return frames

    def test_a_capped_capture_holds_the_rounds_it_reached_and_says_so(self):
        rows, last = self.hand_built(rounds=3)
        fixture = self.build(rows, self.frames_for(3), 20, last - 1, None,
                             max_rounds=1)
        outcome = fixture["outcome"]
        self.assertTrue(outcome["truncated"])
        self.assertEqual(outcome["rounds_captured"], 1)
        self.assertFalse(outcome["victory"])
        self.assertFalse(outcome["defeat"])
        self.assertEqual([round_["round"] for round_ in fixture["rounds"]], [1])
        # Round 2's queue build is at f25; the fixture stops at f24, and the
        # rolls drawn after it are not part of the capture.
        self.assertEqual(fixture["provenance"]["battle_frames"], [20, 24])
        self.assertEqual(fixture["provenance"]["round_frames"], [22])
        self.assertEqual({row[0] for row in fixture["rolls"]["rows"]},
                         {21, 22, 23, 24})

    def test_a_cap_the_log_never_reaches_captures_every_round(self):
        rows, last = self.hand_built(rounds=2)
        fixture = self.build(rows, self.frames_for(2), 20, last - 1, None,
                             max_rounds=5)
        outcome = fixture["outcome"]
        self.assertFalse(outcome["truncated"])
        self.assertEqual(outcome["rounds_captured"], 2)
        self.assertEqual(len(fixture["rounds"]), 2)

    def test_the_patch_records_which_fighters_read_it(self):
        patched = {"alys_hp": 999, "alys_maxhp": 999, "chaz_hp": 999,
                   "chaz_maxhp": 999, "hahn_hp": 999, "hahn_maxhp": 999}
        rows, last = self.hand_built(rounds=1, party=patched)
        note = self.build(rows, self.frames_for(1), 20, last - 1, None,
                          hp_patch=999)["provenance"]["hp_patch"]
        self.assertEqual(note["hp"], 999)
        self.assertEqual(note["members"], ["alys", "chaz", "hahn"])
        self.assertFalse(note["vehicle"])

    def test_a_member_the_tape_left_down_is_not_in_the_patch(self):
        patched = {"alys_hp": 999, "alys_maxhp": 999, "chaz_hp": 999,
                   "chaz_maxhp": 999, "hahn_hp": 0, "hahn_maxhp": 21}
        rows, last = self.hand_built(rounds=1, party=patched)
        note = self.build(rows, self.frames_for(1), 20, last - 1, None,
                          hp_patch=999)["provenance"]["hp_patch"]
        self.assertEqual(note["members"], ["alys", "chaz"])

    def test_a_capture_without_the_patch_says_nothing_about_one(self):
        rows, last = self.hand_built(rounds=1)
        fixture = self.build(rows, self.frames_for(1), 20, last - 1)
        self.assertNotIn("hp_patch", fixture["provenance"])


if __name__ == "__main__":
    unittest.main()


class Commands(ForcedFixture):
    """The commanded target, when the capture logs the command cells.

    `Character_Command_Data` is what the command phase wrote, and the one record
    of what a member's swing was *aimed* at: the cartridge moves
    `Current_Target_Index` off a commanded enemy that has fallen
    (`ps4.asm:8345-8409`), so the cell is what tells a swing that kept its aim
    from one the retarget scan re-aimed (`docs/BATTLE_ORACLE_SWEEP.md`, the
    retarget cluster). A capture carries it only from the point `bcmd` joined
    `oracle/force/runs.py`'s `GROUPS`.
    """

    COMMAND_MAP = {"fields": FORCED_MAP["fields"] + [
        {"name": "cmd0_target", "hex": False},
    ]}

    def test_the_commanded_target_is_recorded_when_the_log_carries_it(self):
        # 6 is enemy slot 1; 65535 is `$FFFF`, the -1 a whole-side attack
        # writes (`ps4.asm:8464`), which the fixture keeps as the sign it is.
        log = self.load([ForcedRow(frame=1, cmd0_target="6"),
                         ForcedRow(frame=2, cmd0_target="65535")],
                        header=[entry["name"]
                                for entry in self.COMMAND_MAP["fields"]],
                        ram_map=self.COMMAND_MAP)
        self.assertEqual(fx.command_entry(log, 1, 1),
                         {"id": 1, "command": "attack", "target": 6})
        self.assertEqual(fx.command_entry(log, 1, 2),
                         {"id": 1, "command": "attack", "target": -1})

    def test_a_log_without_the_command_cells_names_no_target(self):
        log = self.load([ForcedRow(frame=1)])
        self.assertEqual(fx.command_entry(log, 1, 1),
                         {"id": 1, "command": "attack"})


class ActionEffects(ForcedFixture):
    """What an action moved, read off every fighter - either side.

    An enemy ability need not touch its opponents: the sweep's TechUser casts
    RES on itself, and the status arms leave their mark in a status byte rather
    than a damage word, which is what decides whether the log calls the turn an
    ability at all (`oracle/fixture/enemies.py`'s `kind_of`).
    """

    def action(self, rows, frames):
        fixture = self.build(rows, frames, 20, 23)
        return fixture["rounds"][0]["actions"][0]

    def test_a_self_heal_is_an_ability_and_shows_on_the_casters_own_cell(self):
        # 99 TechUser's `$45` RES: the enemy's own HP rises by the damage run's
        # value, and no opposing slot moves at all.
        builder = ForcedLog()
        builder.frame(20, enemy_count=1, e1_id=99, e1_hp=38, e1_maxhp=80)
        builder.frame(21, turn_00=6, turn_01=20, turn_02=1, turn_03=9)
        builder.frame(22, battle_actor=6, e1_ability="45")
        builder.frame(23, e1_hp=75)
        action = self.action(builder.rows, {20: 1, 21: 14, 22: 1, 23: 16})
        self.assertEqual(action["kind"], "ability")
        self.assertEqual(action["ability"], 0x45)
        self.assertEqual(action["targets"], [])
        self.assertEqual(action["effect"]["hp"], [[6, 38, 75]])

    def test_a_status_that_moves_is_an_ability(self):
        # 32 Caterpillr's `$11` POISON: the effect is a status byte, and every
        # damage word stays where it was.
        builder = ForcedLog()
        builder.frame(20, enemy_count=1, e1_id=32, e1_hp=195, e1_maxhp=195)
        builder.frame(21, turn_00=7, turn_01=20, turn_02=1, turn_03=9)
        builder.frame(22, battle_actor=7, e2_ability="11", hahn_status="0",
                      hit_02="00")
        builder.frame(23, hahn_status="1", hit_02="00")
        action = self.action(builder.rows, {20: 1, 21: 14, 22: 1, 23: 1})
        self.assertEqual(action["kind"], "ability")
        self.assertEqual(action["effect"]["status"], [[3, 0, 1]])

    def test_the_death_bit_is_a_death_and_not_a_status_effect(self):
        # `Battle_KillFighter` sets StatusDead (`$04`) when a HP cell goes
        # non-positive; the port reports that as a `Died` event, so a fixture
        # that called the bit an effect would ask it for a status the log never
        # had.
        builder = ForcedLog()
        builder.frame(20, enemy_count=1, e1_id=15, e1_hp=261, e1_maxhp=261)
        builder.frame(21, turn_00=6, turn_01=20, turn_02=1, turn_03=9)
        builder.frame(22, battle_actor=6, alys_hp=900, alys_status="0")
        builder.frame(23, alys_hp=0, alys_status="04")
        action = self.action(builder.rows, {20: 1, 21: 14, 22: 2, 23: 16})
        self.assertEqual(action["effect"]["status"], [])
        self.assertEqual(action["effect"]["hp"], [[1, 900, 0]])

    def test_the_pass_frame_follows_the_last_pass_a_swing_drew(self):
        # A vehicle swing draws one pass per frame (`loc_AF9C`): the bytes the
        # later frames leave are the ones `Fighter_TakeDamage` reads.
        rows = [ForcedRow(frame=1, battle_actor="1", hit_05="01"),
                ForcedRow(frame=2, battle_actor="1", hit_05="00"),
                ForcedRow(frame=3, battle_actor="1", hit_05="00")]
        log = self.log(rows)
        self.assertEqual(fx.pass_frame(log, 1, 3, [1, 2]), 2)

    def test_a_frame_that_drew_nothing_is_not_a_pass(self):
        # The battle scratch above `$FFFF4100` is reused when a round runs out
        # of actors, and the bytes it leaves are not verdicts.
        rows = [ForcedRow(frame=1, battle_actor="1", hit_05="00"),
                ForcedRow(frame=2, battle_actor="1541", hit_02="03",
                          hit_03="06", hit_05="FF"),
                ForcedRow(frame=3, battle_actor="1541", hit_02="03",
                          hit_03="06", hit_05="FF")]
        log = self.log(rows)
        self.assertEqual(fx.pass_frame(log, 1, 3, [1]), 1)
