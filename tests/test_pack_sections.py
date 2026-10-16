"""Map-independent pack sections reused across builds in one process (#130)."""

import shutil
import tempfile
import unittest
from pathlib import Path

from psiv_tools import pack_sections


class Emitter:
    """A stand-in section: its files and value are a function of the ROM bytes."""

    def __init__(self, rom: bytes):
        self.rom = rom
        self.runs = 0

    def __call__(self, root: Path) -> dict:
        self.runs += 1
        (root / "part" / "empty").mkdir(parents=True)
        (root / "part" / "data.bin").write_bytes(self.rom)
        (root / "top.json").write_text(f"{self.rom.hex()}\n")
        return {"size": len(self.rom), "items": [self.rom[0]]}


def tree(root: Path) -> dict[str, bytes | None]:
    return {
        path.relative_to(root).as_posix(): (None if path.is_dir() else path.read_bytes())
        for path in sorted(root.rglob("*"))
    }


class TestSections(unittest.TestCase):
    def setUp(self):
        # The store is process-wide: keep what earlier pack builds in this
        # process put there, so the suite's later builds still reuse it.
        kept = dict(pack_sections._store)
        pack_sections.clear()
        self.addCleanup(pack_sections._store.update, kept)
        self.addCleanup(pack_sections.clear)
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)

    def build(self, rom: bytes, emitter: Emitter, name: str):
        root = self.tmp / name
        root.mkdir()
        result = pack_sections.for_rom(rom).emit(root, ("demo",), emitter)
        return root, result

    def test_a_second_build_replays_the_first_byte_for_byte(self):
        rom = b"\x07ROM-A"
        emitter = Emitter(rom)
        first_root, first = self.build(rom, emitter, "one")
        second_root, second = self.build(rom, emitter, "two")
        self.assertEqual(emitter.runs, 1)
        self.assertEqual(tree(first_root), tree(second_root))
        self.assertIn("part/empty", tree(second_root), "empty directories replay too")
        self.assertEqual(first, second)

    def test_the_replayed_value_is_a_copy(self):
        rom = b"\x07ROM-A"
        emitter = Emitter(rom)
        _, first = self.build(rom, emitter, "one")
        first["items"].append(99)
        _, second = self.build(rom, emitter, "two")
        second["items"].append(98)
        _, third = self.build(rom, emitter, "three")
        self.assertEqual(third["items"], [rom[0]])

    def test_two_roms_never_share_a_section(self):
        rom_a = b"\x07ROM-A"
        rom_b = b"\x07ROM-B"  # same length, one byte apart
        a, b = Emitter(rom_a), Emitter(rom_b)
        root_a, _ = self.build(rom_a, a, "a")
        root_b, _ = self.build(rom_b, b, "b")
        self.assertEqual((a.runs, b.runs), (1, 1))
        self.assertEqual((root_b / "part" / "data.bin").read_bytes(), rom_b)
        self.assertNotEqual(tree(root_a), tree(root_b))
        # Going back to the first ROM replays the first ROM's files.
        again, _ = self.build(rom_a, a, "a2")
        self.assertEqual(a.runs, 1)
        self.assertEqual(tree(again), tree(root_a))

    def test_fresh_runs_every_emitter_and_keeps_nothing(self):
        rom = b"\x07ROM-A"
        emitter = Emitter(rom)
        with pack_sections.fresh():
            first_root, _ = self.build(rom, emitter, "one")
            second_root, _ = self.build(rom, emitter, "two")
        self.assertEqual(emitter.runs, 2)
        self.assertEqual(tree(first_root), tree(second_root))
        self.build(rom, emitter, "three")
        self.assertEqual(emitter.runs, 3, "fresh() left nothing behind to reuse")

    def test_only_the_latest_roms_are_kept(self):
        emitters = [Emitter(bytes([i]) * 4) for i in range(pack_sections.ROM_LIMIT + 1)]
        for i, emitter in enumerate(emitters):
            self.build(emitter.rom, emitter, f"r{i}")
        self.build(emitters[0].rom, emitters[0], "again")
        self.assertEqual(emitters[0].runs, 2, "the oldest ROM was evicted")
        self.build(emitters[-1].rom, emitters[-1], "latest")
        self.assertEqual(emitters[-1].runs, 1)

    def test_a_failing_section_is_not_kept(self):
        rom = b"\x07ROM-A"
        runs = []

        def broken(root: Path):
            runs.append(root)
            (root / "half.json").write_text("{")
            raise ValueError("bad ROM")

        target = self.tmp / "out"
        target.mkdir()
        for _ in range(2):
            with self.assertRaises(ValueError):
                pack_sections.for_rom(rom).emit(target, ("broken",), broken)
        self.assertEqual(len(runs), 2)
        self.assertEqual(list(target.iterdir()), [], "nothing reaches the pack")

    def test_values_are_computed_once_and_copied(self):
        rom = b"\x07ROM-A"
        computed = []

        def compute():
            computed.append(1)
            return {"list": [1]}

        sections = pack_sections.for_rom(rom)
        first = sections.value(("v",), compute)
        first["list"].append(2)
        self.assertEqual(pack_sections.for_rom(rom).value(("v",), compute), {"list": [1]})
        self.assertEqual(len(computed), 1)
        self.assertEqual(pack_sections.for_rom(b"other").value(("v",), compute), {"list": [1]})
        self.assertEqual(len(computed), 2)


if __name__ == "__main__":
    unittest.main()
