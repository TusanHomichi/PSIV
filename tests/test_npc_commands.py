import hashlib
import json
import struct
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.pack import NPC_COMMANDS_NAME, PACK_FORMAT_VERSION, build_pack
from psiv_tools.npc_commands import (
    COLLISION_CELL_PIXELS,
    FACING_NAMES,
    FACING_UNCHANGED,
    FIXED_POINT,
    RECORD_BYTES,
    Command,
    NpcCommandError,
    dispatch_site,
    extract_npc_commands,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

#: The scene op's "one step down", which is what sent this extraction looking.
STEP_DOWN = 2


def command(index: int, raw: str) -> Command:
    from psiv_tools.npc_commands import _decode

    return _decode(index, bytes.fromhex(raw))


class TestRecordFormat(unittest.TestCase):
    """The eight-byte record, on bytes this test writes."""

    def test_a_step_is_distance_over_velocity(self):
        # 16 pixels at 256/256 = 1 pixel a frame is 16 frames, which is what
        # `FieldObj_UpdateStepDuration` counts down.
        down = command(2, "0010000001000000")
        self.assertTrue(down.moves)
        self.assertEqual(down.direction, "down")
        self.assertEqual(down.frames, 16)
        self.assertEqual(down.y_pixels, COLLISION_CELL_PIXELS)
        self.assertEqual(down.facing, 0)

    def test_a_negative_velocity_is_the_other_way(self):
        up = command(1, "00100000ff000400")
        self.assertEqual((up.direction, up.y_velocity, up.frames), ("up", -256, 16))
        left = command(4, "1000ff0000000c00")
        self.assertEqual((left.direction, left.x_velocity), ("left", -256))

    def test_a_zero_record_stands_still_and_keeps_its_facing(self):
        idle = command(0, "000000000000ff00")
        self.assertFalse(idle.moves)
        self.assertIsNone(idle.direction)
        self.assertIsNone(idle.frames)
        self.assertEqual(idle.facing, FACING_UNCHANGED)
        self.assertTrue(idle.to_json()["facing"]["unchanged"])

    def test_the_record_is_eight_bytes_and_the_scale_is_256(self):
        self.assertEqual(RECORD_BYTES, 8)
        self.assertEqual(FIXED_POINT, 256)
        self.assertEqual(sorted(FACING_NAMES), [0x0, 0x4, 0x8, 0xC])

    # --------------------------------------------------------- fail-closed
    def test_a_distance_without_a_velocity_is_refused(self):
        # The duration would never count down: the NPC would walk forever.
        with self.assertRaises(NpcCommandError):
            command(0, "0010000000000000")

    def test_a_velocity_without_a_distance_is_refused(self):
        with self.assertRaises(NpcCommandError):
            command(0, "0000000001000000")

    def test_moving_on_both_axes_is_refused(self):
        with self.assertRaises(NpcCommandError):
            command(0, "1010010001000000")

    def test_a_step_that_is_not_a_whole_number_of_frames_is_refused(self):
        # 16 pixels at 384/256 per frame is 10.67 frames.
        with self.assertRaises(NpcCommandError):
            command(0, "0010000001800000")

    def test_a_facing_that_is_neither_a_direction_nor_the_sentinel(self):
        with self.assertRaises(NpcCommandError):
            command(0, "0000000000000100")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestAgainstTheRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.payload = extract_npc_commands(cls.data)

    def test_the_table_is_named_by_the_routine_that_reads_it(self):
        site, table = dispatch_site(self.data)
        self.assertEqual(site, 0x04A168)
        self.assertEqual(table, 0x04A202)
        self.assertEqual(self.payload["dispatch"]["speed_table"], "0x04A202")
        # `lea (abs).l,a1 / add.w d7,d7 / add.w d7,d7 / movea.l (a1,d7.w),a1`
        self.assertEqual(
            self.data[site:site + 6], b"\x43\xf9" + struct.pack(">I", table)
        )

    def test_three_speeds_of_eleven_commands_each(self):
        self.assertEqual(self.payload["dispatch"]["speed_count"], 3)
        self.assertEqual(self.payload["command_count"], 11)
        speeds = self.payload["speeds"]
        # The tables are flush behind their pointer table and against each
        # other, and the last ends where the next routine begins.
        self.assertEqual(speeds[0]["rom_offset"], "0x04A20E")
        for earlier, later in zip(speeds, speeds[1:]):
            self.assertEqual(earlier["rom_end"], later["rom_offset"])
        self.assertEqual(speeds[-1]["rom_end"], "0x04A316")

    def test_the_three_speeds_are_half_one_and_two_pixels_a_frame(self):
        speeds = self.payload["speeds"]
        self.assertEqual([s["velocity"] for s in speeds], [128, 256, 512])
        self.assertEqual([s["pixels_per_frame"] for s in speeds], [0.5, 1.0, 2.0])
        self.assertEqual([s["frames_per_cell"] for s in speeds], [[32], [16], [8]])
        # The fastest is the party's own speed: `FieldObj_MovementsTbl` stores
        # $0200 and the party takes 8 frames to cross a cell.
        self.assertEqual(speeds[2]["velocity"], 0x200)

    def test_command_two_is_one_step_down_at_every_speed(self):
        # The finding this extraction was sent to explain.
        for speed in self.payload["speeds"]:
            with self.subTest(selector=speed["selector"]):
                down = speed["commands"][STEP_DOWN]
                self.assertEqual(down["direction"], "down")
                self.assertEqual(down["y_cells"], 1)
                self.assertEqual(down["x_pixels"], 0)
                self.assertEqual(down["y_pixels"], COLLISION_CELL_PIXELS)
                self.assertEqual(down["facing"], {"byte": 0, "name": "down",
                                                  "unchanged": False})
                self.assertGreater(down["y_velocity"], 0)

    def test_the_command_set_is_four_directions_and_three_idles(self):
        census = self.payload["census"]
        self.assertEqual(census["standing_still"], [0, 3, 7])
        self.assertEqual(census["duplicate_directions"], {
            "up": [1], "down": [2], "left": [4, 5, 6], "right": [8, 9, 10],
        })
        # Every moving command crosses exactly one collision cell.
        self.assertEqual(census["pixels_per_command"], [COLLISION_CELL_PIXELS])

    def test_the_same_commands_appear_at_every_speed(self):
        # `extract_npc_commands` refuses a ROM where they differ, so this is a
        # restatement for a reader rather than a second check -- but it is the
        # property that lets a consumer treat the id as a direction.
        shapes = [
            [(c["direction"], c["x_pixels"], c["y_pixels"], c["facing"]["byte"])
             for c in speed["commands"]]
            for speed in self.payload["speeds"]
        ]
        self.assertEqual(shapes[0], shapes[1])
        self.assertEqual(shapes[1], shapes[2])

    def test_a_rom_whose_dispatch_moved_is_refused(self):
        broken = bytearray(self.data)
        broken[0x04A168:0x04A170] = b"\x00" * 8
        with self.assertRaises(NpcCommandError):
            dispatch_site(bytes(broken))
        with self.assertRaises(NpcCommandError):
            extract_npc_commands(bytes(broken))

    def test_tables_that_are_not_equal_in_length_are_refused(self):
        broken = bytearray(self.data)
        # Point the third speed table somewhere else entirely.
        broken[0x04A20A:0x04A20E] = struct.pack(">I", 0x04A400)
        with self.assertRaises(NpcCommandError):
            extract_npc_commands(bytes(broken))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestInThePack(unittest.TestCase):
    """The table as the pack emits it, from a one-map build."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(cls.data, cls.root, map_ids=[0x10])

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_the_manifest_points_at_the_npc_command_table(self):
        commands = self.manifest["npc_commands"]
        self.assertEqual(commands["file"], NPC_COMMANDS_NAME)
        blob = (self.root / NPC_COMMANDS_NAME).read_bytes()
        self.assertEqual(commands["sha256"], hashlib.sha256(blob).hexdigest())
        payload = json.loads(blob)
        self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)
        self.assertEqual(payload["kind"], "npc_movement_commands")
        self.assertEqual(commands["command_count"], 11)
        self.assertEqual(commands["speed_count"], 3)
        # Slow, normal and the party's own pace.
        self.assertEqual(commands["frames_per_cell"], [32, 16, 8])
        # The command the scene op that sent us looking uses.
        down = payload["speeds"][1]["commands"][2]
        self.assertEqual((down["direction"], down["y_cells"]), ("down", 1))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """What the clone calls the routines the retail bytes already proved."""

    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")
        cls.constants = (REFERENCE / "ps4.constants.asm").read_text(errors="replace")

    def test_the_routine_and_its_three_tables_are_named(self):
        self.assertIn("FieldObj_NPCMove:", self.source)
        self.assertIn("lea\t(loc_4A202).l, a1", self.source)
        for label in ("loc_4A20E", "loc_4A266", "loc_4A2BE"):
            with self.subTest(label=label):
                self.assertIn(f"dc.l\t{label}", self.source)

    def test_the_record_fields_are_the_struct_offsets_the_clone_defines(self):
        for line in (
            "x_step_constant = $20",
            "y_step_constant = $24",
            "x_step_duration = $28",
            "y_step_duration = $2A",
            "facing_dir = 6",
        ):
            with self.subTest(line=line):
                self.assertIn(line, self.constants)

    def test_the_duration_counts_down_by_the_constant_over_256(self):
        # `asr.l #8` then `sub.w`, which is where `frames` comes from.
        self.assertIn("FieldObj_UpdateStepDuration:", self.source)
        self.assertIn("\tasr.l\t#8, d1\n\tsub.w\td1, x_step_duration(a4)", self.source)

    def test_alys_in_piata_selects_the_middle_speed(self):
        # The NPC scene-lane traced: `moveq #1,d7` before the move call.
        body = self.source.split("FieldObj_NPCAlysPiataMain:")[1].split("; ---")[0]
        self.assertIn("moveq\t#1, d7", body)
        self.assertIn("bsr.w\tFieldObj_NPCMove", body)


if __name__ == "__main__":
    unittest.main()
