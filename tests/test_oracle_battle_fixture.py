"""Tests for `oracle.fixture`, the replay-fixture extractor, and its CLI.

The inputs are hand-built rows: a trace and a RAM log written from a few
sentences about what happened, so the extractor's reading of them can be
checked without the emulator. The rows carry the two seed half-words and the
HV word a call returned, so the tests can build the *same* call twice - once
with the roll column the cartridge's `sub.w (RNG_Seed).w, d0` computes (the
longword's high half, what the host writes now) and once with the low half the
host wrote before it was fixed - and insist the extractor takes the first and
refuses the second.
"""
import json
import os
import tempfile
import unittest

from oracle import fixture as bf

M16 = 0xFFFF

#: The columns the extractor reads, with the hex/dec rendering
#: `oracle/ram_map.json` gives them. The tests carry their own copy so a
#: change in the map has to be deliberate in both places.
RAM_MAP = {"fields": [
    {"name": name, "hex": name in HEX}
    for name in (
        "frame", "battle_actor", "battle_priority", "battle_exp_total",
        "battle_meseta_total", "enemy_count", "enemy_ambush_chance",
        "enemy_run_chance", "item_drop_rate", "dropped_item",
        *[f"turn_{index:02d}" for index in range(18)],
        "chaz_level", "chaz_hp", "chaz_maxhp", "chaz_tp", "chaz_maxtp",
        "chaz_status", "chaz_str", "chaz_agi", "chaz_agi_bat", "chaz_dex",
        "chaz_atk", "chaz_dfs", "chaz_men", "chaz_exp",
        "alys_level", "alys_hp", "alys_maxhp", "alys_tp", "alys_maxtp",
        "alys_status", "alys_str", "alys_agi", "alys_agi_bat", "alys_dex",
        "alys_atk", "alys_dfs", "alys_men", "alys_exp",
        "hahn_level", "hahn_hp", "hahn_maxhp", "hahn_tp", "hahn_maxtp",
        "hahn_status", "hahn_str", "hahn_agi", "hahn_agi_bat", "hahn_dex",
        "hahn_atk", "hahn_dfs", "hahn_men", "hahn_exp",
    )
    for HEX in [("battle_priority", *[f"turn_{index:02d}"
                                      for index in range(18)])]
] + [
    {"name": f"{prefix}_{suffix}", "hex": suffix in ("id", "status")}
    for prefix in ("e1", "e2", "e3", "e4")
    for suffix in ("id", "hp", "maxhp", "status", "str_bat", "men_bat",
                   "dex_bat", "agi_bat", "atk", "dfs")
] + [
    {"name": f"hit_{index:02d}", "hex": True} for index in range(10)
] + [
    {"name": f"dmg_{index:02d}", "hex": False} for index in range(10)
]}

TRACE_HEADER = ("frame,call_index_in_frame,pc,hv,frame_count,seed_before,"
                "roll,seed_after")


def trace_row(frame, index, hv, frame_count, seed_before, roll):
    """One trace row, with the roll column's text supplied by the caller."""
    return {
        "frame": str(frame),
        "call_index_in_frame": str(index),
        "pc": "0423A2",
        "hv": f"{hv:04X}",
        "frame_count": str(frame_count),
        "seed_before": f"{seed_before:08X}",
        "roll": f"{roll:04X}",
        "seed_after": f"{seed_before:08X}",
    }


def cartridge_roll(hv, frame_count, seed_before):
    """The roll `sub.w (RNG_Seed).w, d0` leaves: the longword's high half."""
    return (hv + frame_count - ((seed_before >> 16) & M16)) & M16


def trace_low_word_roll(hv, frame_count, seed_before):
    """What the trace's own roll column computes: the low half."""
    return (hv + frame_count - (seed_before & M16)) & M16


class Row:
    """A RAM-log row with sensible defaults for everything not being tested."""

    def __init__(self, **overrides):
        defaults = {
            "frame": 1, "battle_actor": "0", "battle_priority": "00",
            "battle_exp_total": "0", "battle_meseta_total": "0",
            "enemy_count": "0", "enemy_ambush_chance": "0",
            "enemy_run_chance": "0", "item_drop_rate": "0",
            "dropped_item": "0",
            **{f"turn_{index:02d}": "0000" for index in range(18)},
            **{f"hit_{index:02d}": "FF" for index in range(10)},
            **{f"dmg_{index:02d}": "0" for index in range(10)},
            **{f"e{slot}_{suffix}": "0"
               for slot in range(1, 5)
               for suffix in ("id", "hp", "maxhp", "status", "str_bat",
                              "men_bat", "dex_bat", "agi_bat", "atk", "dfs")},
            **{f"{who}_{suffix}": value
               for who in ("alys", "chaz", "hahn")
               for suffix, value in (
                   ("level", "7"), ("hp", "53"), ("maxhp", "53"),
                   ("tp", "40"), ("maxtp", "40"), ("status", "0"),
                   ("str", "12"), ("agi", "15"), ("agi_bat", "15"),
                   ("dex", "13"), ("atk", "13"), ("dfs", "18"), ("men", "12"),
                   ("exp", "0"))},
        }
        defaults.update({key: str(value) for key, value in overrides.items()})
        self.values = defaults

    def __getitem__(self, key):
        return self.values[key]


class LogBuilder:
    """A RAM log assembled frame by frame, carrying the last row forward."""

    def __init__(self):
        self.rows = []
        self.last = Row()

    def frame(self, number, **overrides):
        merged = dict(self.last.values)
        merged["frame"] = str(number)
        merged.update({key: str(value) for key, value in overrides.items()})
        self.last = Row(**merged)
        self.rows.append(self.last)
        return self.last


class FixtureTest(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.mkdtemp(prefix="psiv-battle-fixture-")
        self.addCleanup(lambda: None)

    def load(self, rows, header=None, ram_map=None):
        """A Log over hand-built rows; the default carries every column.

        `ram_map` is for a test that needs a column this file's map does not
        carry, e.g. the forced captures' ability and vehicle cells
        (`tests/test_oracle_battle_fixture_forced.py`)."""
        header = header or list(Row().values)
        ram_map = ram_map or RAM_MAP
        path = os.path.join(self.tmpdir, "log.csv")
        with open(path, "w") as handle:
            handle.write("# provenance\n")
            handle.write(",".join(header) + "\n")
            for row in rows:
                handle.write(",".join(row[column] for column in header) + "\n")
        map_path = os.path.join(self.tmpdir, "ram_map.json")
        with open(map_path, "w") as handle:
            json.dump(ram_map, handle)
        return bf.Log(bf.load_rows(path), bf.load_ram_map(map_path))


class RollDerivation(FixtureTest):
    def test_the_cartridge_roll_subtracts_the_seed_longwords_high_half(self):
        hv = 0x2292
        frame_count = 0x708F
        seed = 0x21E817F3
        self.assertEqual(
            bf.roll_high_word(trace_row(29711, 0, hv, frame_count, seed, 0)),
            (0x2292 + 0x708F - 0x21E8) & M16,
        )
        # The pre-fix column is the other half: the two differ by the seed's
        # halves, which is a per-frame constant.
        self.assertEqual(bf.roll_low_word(trace_row(29711, 0, hv, frame_count, seed, 0)),
                         0x7B2E)
        self.assertNotEqual(bf.roll_high_word(trace_row(29711, 0, hv, frame_count, seed, 0)),
                            0x7B2E)

    def test_the_roll_column_report_counts_the_rows_that_agree(self):
        seed = 0x21E817F3
        high = [trace_row(10, index, 0x2292, 0x708F, seed,
                          cartridge_roll(0x2292, 0x708F, seed))
                for index in range(2)]
        self.assertEqual(bf.roll_column_report(high),
                         {"agrees": 2, "subtracts_low_word": 0, "neither": 0})

    def test_a_low_word_roll_column_is_rejected_with_its_frame_and_call(self):
        seed = 0x21E817F3
        low = [trace_row(10, index, 0x2292, 0x708F, seed,
                         trace_low_word_roll(0x2292, 0x708F, seed))
               for index in range(3)]
        with self.assertRaises(bf.FixtureError) as caught:
            bf.roll_column_report(low)
        self.assertIn("f10 call 0", str(caught.exception))
        self.assertIn("low-half subtraction", str(caught.exception))

    def test_a_roll_column_that_is_neither_derivation_is_rejected(self):
        row = trace_row(10, 0, 0x2292, 0x708F, 0x21E817F3, 0x0000)
        with self.assertRaises(bf.FixtureError) as caught:
            bf.roll_column_report([row])
        self.assertIn("f10 call 0", str(caught.exception))
        self.assertIn("neither", str(caught.exception))

    def test_the_report_names_the_first_row_that_is_not_the_cartridges(self):
        # Only the second call is wrong, and the message has to say so.
        seed = 0x21E817F3
        rows = [trace_row(10, 0, 0x2292, 0x708F, seed,
                          cartridge_roll(0x2292, 0x708F, seed)),
                trace_row(10, 1, 0x23F3, 0x708F, seed,
                          trace_low_word_roll(0x23F3, 0x708F, seed))]
        with self.assertRaises(bf.FixtureError) as caught:
            bf.roll_column_report(rows)
        self.assertIn("f10 call 1", str(caught.exception))

    def test_rolls_are_kept_in_frame_order_and_grouped_by_frame(self):
        seed = 0x21E817F3
        rows = [trace_row(10, 0, 0x2292, 0x708F, seed,
                          cartridge_roll(0x2292, 0x708F, seed)),
                trace_row(10, 1, 0x23F3, 0x708F, seed,
                          cartridge_roll(0x23F3, 0x708F, seed)),
                trace_row(12, 0, 0x1000, 0x7091, seed,
                          cartridge_roll(0x1000, 0x7091, seed))]
        rolls = bf.rolls_in_window(rows, 9, 11)
        self.assertEqual([frame for frame, _ in rolls], [10, 10])
        self.assertEqual(bf.group_by_frame(rolls),
                         [(10, [roll for _, roll in rolls])])


class Segmentation(FixtureTest):
    """The extractor's reading of the RAM log's own shape."""

    def test_an_action_opens_on_a_fighter_id_and_ignores_menu_state(self):
        # The command phase leaves non-fighter numbers in $FFFF4142, and the
        # turn engine writes a queue entry - even one it then skips because the
        # fighter is down - before it tests anything (`ps4.asm:8033-8043`), so
        # what opens a window is the action's own calls: frames 2-3 read the
        # fighter's id but drew nothing, and are not a turn.
        rows = [Row(frame=1, battle_actor="12037"), Row(frame=2, battle_actor="1"),
                Row(frame=3, battle_actor="1"), Row(frame=4, battle_actor="0"),
                Row(frame=5, battle_actor="7"), Row(frame=6, battle_actor="9")]
        log = self.load(rows)
        self.assertEqual(bf.action_windows(log, 1, 6, roll_frames=[3, 6]),
                         [(1, 3, 5), (9, 6, 6)])

    def test_a_round_opens_where_the_turn_order_is_filled(self):
        log = self.load([Row(frame=1), Row(frame=2, turn_00="0001"),
                         Row(frame=3, turn_00="0001"), Row(frame=4, turn_00="0002")])
        # Frame 1 is the log's first row, so nothing moved into it.
        self.assertEqual(bf.round_frames(log, 1, 4), [2, 4])

    def test_the_formation_check_needs_a_whole_enemy_record(self):
        # Tape 07's field RAM reads 12063 HP over a maximum of 46 before the
        # formation is written; the enemy count and a full record settle it.
        stale = [Row(frame=1, e1_id="7963", e1_hp="12063", e1_maxhp="46"),
                 Row(frame=2, e1_id="7963", e1_hp="12063", e1_maxhp="46")]
        loaded = [Row(frame=3, e1_id="000A", e1_hp="25", e1_maxhp="25",
                      enemy_count="2")]
        log = self.load(stale + loaded)
        self.assertEqual(bf.enemy_slots(log, 3), [6])
        # One record where the count says two: the load is half done, and a
        # fixture started here would fight a formation the cartridge did not.
        with self.assertRaises(bf.FixtureError):
            bf.enemies_loaded(log, 1, 3)

    def test_a_half_written_formation_is_not_the_start_frame(self):
        """The load writes one slot per frame; `enemy_count` is the tell.

        The sweep's formation `$13`: f24821 holds one SandNewt record over a
        count of 2, f24822 holds both. Starting at f24821 seated one enemy, and
        the port's round was then a different battle from the cartridge's.
        """
        log = self.load([Row(frame=1, e1_id="0038", e1_hp="41", e1_maxhp="41",
                             enemy_count="2"),
                         Row(frame=2, e1_id="0038", e1_hp="41", e1_maxhp="41",
                             e2_id="0038", e2_hp="41", e2_maxhp="41",
                             enemy_count="2")])
        self.assertEqual(bf.enemy_slots(log, 1), [6])
        self.assertEqual(bf.enemy_slots(log, 2), [6, 7])
        self.assertEqual(bf.enemies_loaded(log, 1, 2), 2)

    def test_a_battle_with_no_formation_is_rejected(self):
        log = self.load([Row(frame=1)])
        with self.assertRaises(bf.FixtureError):
            bf.enemies_loaded(log, 1, 1)


class Observations(FixtureTest):
    def observation(self, rows):
        log = self.load(rows)
        return log, bf.action_record(log, 1, 30207, 30208,
                                     [(30207, 16), (30207, 32)],
                                     {1, 2, 3, 6, 7})

    def test_a_slot_is_a_target_when_the_pass_resolved_it(self):
        log, record = self.observation([
            Row(frame=30206, e1_hp="25", e2_hp="25", dmg_05="0", dmg_06="0"),
            # The hit pass presets every flag and then writes the window's
            # verdicts; the damage routine fills the damage list; the HP moves
            # later, in the animation.
            Row(frame=30207, battle_actor="1", hit_05="00", hit_06="00",
                dmg_05="12", dmg_06="10", e1_hp="25", e2_hp="25"),
            Row(frame=30208, battle_actor="1", hit_05="00", hit_06="00",
                e1_hp="13", e2_hp="15"),
        ])
        self.assertEqual([target["id"] for target in record["targets"]], [6, 7])
        self.assertEqual([target["hit"] for target in record["targets"]],
                         ["00", "00"])
        self.assertEqual([target["damage"] for target in record["targets"]], [12, 10])
        self.assertEqual([target["hp_after"] for target in record["targets"]], [13, 15])
        self.assertEqual([target["died"] for target in record["targets"]],
                         [False, False])
        self.assertEqual(record["roll_count"], 48)

    def test_a_slot_the_pass_left_at_ff_is_not_a_target(self):
        """A preset that rewrites a previous action's verdict is not a target.

        The sweep's formation `$10`: the round's last actor's window reaches
        the frames where the battle scratch above `$FFFF4150` is reused, and the
        bytes that land there are not verdicts at all. A slot whose flag reads
        `$FF` at the action's own pass is unresolved, whatever moves later.
        """
        log, record = self.observation([
            Row(frame=30206, e1_hp="25", e2_hp="25"),
            Row(frame=30207, battle_actor="1", hit_05="00", dmg_05="12",
                e1_hp="25", e2_hp="25"),
            # The scratch's own write, after the action's pass: three bytes
            # `loc_B6A2` never writes, and `$FF` again for the slot it hit.
            Row(frame=30208, battle_actor="1541", hit_02="03", hit_03="06",
                hit_05="FF", e1_hp="13", e2_hp="25"),
        ])
        self.assertEqual([target["id"] for target in record["targets"]], [6])
        self.assertEqual(record["targets"][0]["hit"], "00")
        self.assertEqual(record["verdict_frame"], 30207)

    def test_the_actors_own_side_is_never_a_target(self):
        # `loc_B6A2` blanks every slot, so a party slot reading $FF (and its
        # damage word reading $FFFF) is the clear, not an observation.
        log, record = self.observation([
            Row(frame=30206, hit_05="FF", hit_06="FF", dmg_05="65535", dmg_06="0"),
            Row(frame=30207, battle_actor="1", dmg_05="65535", dmg_06="10"),
            Row(frame=30208, battle_actor="1", e2_hp="15"),
        ])
        self.assertEqual([target["id"] for target in record["targets"]], [7])

    def test_a_negative_hp_is_reported_as_the_cartridge_stores_it(self):
        log, record = self.observation([
            Row(frame=30206, dmg_06="0"),
            Row(frame=30207, battle_actor="1", hit_06="00", dmg_06="15"),
            Row(frame=30208, battle_actor="1", e2_hp="65535"),
        ])
        target = record["targets"][0]
        self.assertEqual(target["damage"], 15)
        self.assertEqual(target["hp_after"], -1)
        self.assertTrue(target["died"])


class FixtureAssembly(FixtureTest):
    """The whole extractor on a battle it can be read by hand.

    Ten frames, four of them with calls: the encounter's formation draw before
    the enemy records exist, the battle's priority draw after them, the queue
    build, and one action whose two calls are the pair a single-target hit pass
    makes. The damage routine's sixteen-call runs are left out: the extractor's
    role labels are the Python tests' subject, and the Rust replay is where
    they are checked against the cartridge.
    """

    HV, FRAME_COUNT, SEED = 0x2292, 0x708F, 0x21E817F3

    def roll_for(self, frame):
        return cartridge_roll(self.HV + frame, self.FRAME_COUNT, self.SEED)

    def row(self, frame, index):
        return trace_row(frame, index, self.HV + frame, self.FRAME_COUNT,
                         self.SEED,
                         cartridge_roll(self.HV + frame, self.FRAME_COUNT,
                                        self.SEED))

    def build(self, trace_roll=None, column=None):
        """The fixture for the hand-built battle.

        `trace_roll` forces every row's roll column to one value (a trace that
        carries neither derivation); `column` builds the column with another
        function, e.g. `trace_low_word_roll` for what the host wrote before it
        was fixed."""
        trace = []
        for frame in (10, 11, 12, 13):
            count = {10: 1, 11: 1, 12: 13, 13: 2}[frame]
            for index in range(count):
                row = self.row(frame, index)
                if trace_roll is not None:
                    row["roll"] = f"{trace_roll:04X}"
                elif column is not None:
                    row["roll"] = f"{column(self.HV + frame, self.FRAME_COUNT,
                                            self.SEED):04X}"
                trace.append(row)
        log_rows = [
            Row(frame=9),
            Row(frame=10, e1_id="7963", e1_hp="12063", e1_maxhp="46"),
            Row(frame=11, e1_id="000A", e1_hp="25", e1_maxhp="25",
                e2_id="000A", e2_hp="25", e2_maxhp="25", enemy_count="2",
                enemy_ambush_chance="15", enemy_run_chance="5",
                item_drop_rate="0", battle_priority="00"),
            Row(frame=12, turn_00="0001", turn_01="0014", turn_02="0002",
                turn_03="000B"),
            Row(frame=13, battle_actor="1", hit_05="00", dmg_05="12",
                e1_hp="13", turn_00="0001", turn_01="0014",
                turn_02="0002", turn_03="000B", battle_exp_total="12",
                battle_meseta_total="3"),
        ]
        log = self.load(log_rows)
        fixture = bf.build_fixture(trace, log, {**RAM_MAP}, 9, 13,
                                   {"tape": "t", "core": "c", "patch": "p",
                                    "trace": "x", "trace_sha256": "0" * 64,
                                    "log_sha256": "1" * 64})
        return fixture

    @staticmethod
    def table(table):
        """A roll table's rows as dicts, keyed by the file's own columns."""
        return [dict(zip(table["columns"], row)) for row in table["rows"]]

    def test_the_battle_stream_starts_at_the_priority_draw(self):
        fixture = self.build()
        rolls = self.table(fixture["rolls"])
        self.assertEqual(rolls[0]["role"], "priority")
        self.assertEqual(rolls[0]["frame"], 11)
        self.assertEqual(fixture["rolls"]["columns"],
                         ["frame", "roll", "role", "target", "pass", "round",
                          "action"])
        outside = self.table(fixture["outside_rolls"])
        self.assertEqual([row["frame"] for row in outside], [10])
        self.assertEqual([row["role"] for row in outside], ["formation"])
        # One call per frame: 1 + 1 + 13 + 2. The extractor checks every row's
        # roll column against the cartridge's derivation and reports them all
        # as agreeing: nothing is accepted on the strength of the column
        # alone, and the pre-fix low-half column never reaches a fixture.
        self.assertEqual(fixture["provenance"]["roll_column"],
                         {"agrees": 17, "subtracts_low_word": 0, "neither": 0})
        self.assertEqual(fixture["provenance"]["battle_roll_count"], 16)
        self.assertEqual(fixture["provenance"]["outside_roll_count"], 1)
        self.assertEqual(fixture["provenance"]["roll_count"], 17)

    def test_the_start_state_is_the_frame_the_formation_was_written(self):
        fixture = self.build()
        self.assertEqual(fixture["provenance"]["start_frame"], 11)
        self.assertEqual(fixture["provenance"]["round_frames"], [12])
        self.assertEqual(fixture["formation"]["enemies"][0]["id"], 6)
        self.assertEqual(fixture["formation"]["enemies"][1]["id"], 7)
        self.assertEqual(fixture["formation"]["enemies"][0]["hp"], 25)
        self.assertEqual(fixture["formation"]["priority"], 0)
        self.assertEqual(fixture["party"][0]["id"], 1)
        self.assertEqual(fixture["party"][0]["hp"], 53)

    def test_the_round_carries_its_order_commands_and_action(self):
        fixture = self.build()
        round_ = fixture["rounds"][0]
        self.assertEqual(round_["order"], [1, 2])
        self.assertEqual(round_["ordering"], [20, 11])
        self.assertEqual(round_["order_frame"], 12)
        self.assertEqual([entry["command"] for entry in round_["commands"]],
                         ["attack", "attack"])
        action = round_["actions"][0]
        self.assertEqual(action["actor"], 1)
        self.assertEqual(action["hit_frame"], 13)
        self.assertEqual(action["roll_count"], 2)
        self.assertEqual([target["id"] for target in action["targets"]], [6])
        self.assertEqual(action["targets"][0]["damage"], 12)
        self.assertEqual(action["targets"][0]["hp_after"], 13)

    def test_a_trace_whose_roll_column_is_neither_derivation_is_rejected(self):
        with self.assertRaises(bf.FixtureError):
            self.build(trace_roll=0x1234)

    def test_a_trace_with_the_pre_fix_low_word_column_is_rejected(self):
        # Lane O1's capture is exactly this trace: seeds, PCs, counter and
        # chain all right, every roll column shifted by a per-frame constant.
        with self.assertRaises(bf.FixtureError) as caught:
            self.build(column=trace_low_word_roll)
        self.assertIn("f10 call 0", str(caught.exception))
        self.assertIn("low-half subtraction", str(caught.exception))

    def test_a_log_with_a_hole_is_rejected(self):
        log = self.load([Row(frame=1), Row(frame=3)])
        with self.assertRaises(bf.FixtureError) as caught:
            bf.build_fixture([trace_row(1, 0, 0x1000, 1, 0x21E817F3, 0)],
                             log, {**RAM_MAP}, 1, 3,
                             {"tape": "t", "core": "c", "patch": "p",
                              "trace": "x", "trace_sha256": "0" * 64,
                              "log_sha256": "1" * 64})
        self.assertIn("missing", str(caught.exception))


class DecisiveHitByte(FixtureTest):
    """The frame an action's per-target `hit` byte is read at.

    `loc_B6A2` presets all nine `Fighters_Hit_Flags` to `$FF` before it rolls
    (`ps4.asm:17493-17498`) and every pass walks the same window, so the byte a
    swing leaves behind is the one its **last** pass wrote - the byte
    `Fighter_TakeDamage` reads (`ps4.asm:3569-3571`). A swing whose passes all
    arrive in one frame leaves that byte at the frame the flags first moved; a
    swing whose passes arrive in *different* frames leaves the first pass's
    there, and the decisive pass's only later, so the fixture reads the last
    pass's frame. Rounds 3 and 6 of the `$53` Desrt Leach capture are the case
    the second test stands in for: a `$01` first pass over a swing whose third
    pass read `$00`, and a damage number that only a normal hit produces.
    """

    HV, FRAME_COUNT, SEED = 0x2292, 0x708F, 0x21E817F3

    def row(self, frame, index):
        return trace_row(frame, index, self.HV + frame, self.FRAME_COUNT,
                         self.SEED,
                         cartridge_roll(self.HV + frame, self.FRAME_COUNT,
                                        self.SEED))

    def swing(self, frames):
        """The fixture for one swing against one enemy.

        `frames` is `(frame, calls, flag)` per frame the swing drew in: how
        many of the trace's calls that frame held - one per pass it ran - and
        what the target's `hit_05` reads from that frame on. The frames follow
        the formation and the queue build, which the `FixtureAssembly` battle
        above supplies the shape of.
        """
        trace = []
        for frame, calls, _ in [(10, 1, None), (11, 1, None), (12, 13, None),
                                *frames]:
            for index in range(calls):
                trace.append(self.row(frame, index))
        rows = [
            Row(frame=9),
            Row(frame=10, e1_id="7963", e1_hp="12063", e1_maxhp="46"),
            Row(frame=11, e1_id="000A", e1_hp="25", e1_maxhp="25",
                e2_id="000A", e2_hp="25", e2_maxhp="25", enemy_count="2",
                enemy_ambush_chance="15", enemy_run_chance="5",
                item_drop_rate="0", battle_priority="00"),
            Row(frame=12, turn_00="0001", turn_01="0014", turn_02="0002",
                turn_03="000B", battle_actor="0"),
        ]
        for index, (frame, _, flag) in enumerate(frames):
            rows.append(Row(frame=frame, battle_actor="1", hit_05=flag,
                            turn_00="0001", turn_01="0014", turn_02="0002",
                            turn_03="000B", e1_hp="13",
                            battle_exp_total="12" if index else "0",
                            battle_meseta_total="3" if index else "0"))
        log = self.load(rows)
        return bf.build_fixture(trace, log, {**RAM_MAP}, 9, frames[-1][0],
                                {"tape": "t", "core": "c", "patch": "p",
                                 "trace": "x", "trace_sha256": "0" * 64,
                                 "log_sha256": "1" * 64})

    @staticmethod
    def action(fixture):
        action = fixture["rounds"][0]["actions"][0]
        return action, action["targets"][0]

    @staticmethod
    def passes(fixture):
        """The `pass` column of the action's hit rolls, in the trace's order."""
        table = fixture["rolls"]
        rows = [dict(zip(table["columns"], row)) for row in table["rows"]]
        return [row["pass"] for row in rows if row["role"] == "hit"]

    def test_a_single_frame_swing_reads_the_byte_where_the_flags_moved(self):
        # Both of the swing's passes ran in f13 - the shape every character's
        # attack has, and Alys's and Kyra's two - so the byte the flags first
        # moved to *is* the decisive pass's, and the fixture reads it there.
        fixture = self.swing([(13, 2, "01")])
        action, target = self.action(fixture)
        self.assertEqual(action["hit_frame"], 13)
        self.assertEqual(target["hit"], "01")
        self.assertEqual(self.passes(fixture), [1, 2])

    def test_a_swing_whose_passes_span_frames_reads_the_last_passs_byte(self):
        fixture = self.swing([(13, 1, "01"), (14, 1, "00"), (15, 1, "00")])
        action, target = self.action(fixture)
        # The flags first moved at f13, to the *first* pass's critical - what
        # this fixture used to record. The swing's byte is the third pass's.
        self.assertEqual(action["hit_frame"], 13)
        self.assertEqual(target["hit"], "00")
        self.assertEqual(self.passes(fixture), [1, 2, 3])

    def test_a_swing_whose_last_pass_missed_resolves_nothing(self):
        # A miss writes `$FF` over the slot (`loc_B704`, `ps4.asm:17528-17530`),
        # so a swing whose first pass hit and whose second missed resolves
        # nothing: the slot is read at the decisive pass's frame and left out of
        # the target list, which is what the port's own swing has to match.
        fixture = self.swing([(13, 1, "00"), (14, 1, "FF")])
        action = fixture["rounds"][0]["actions"][0]
        self.assertEqual(action["hit_frame"], 13)
        self.assertEqual(action["verdict_frame"], 14)
        self.assertEqual(action["targets"], [])
        self.assertEqual(self.passes(fixture), [1, 2])


if __name__ == "__main__":
    unittest.main()
