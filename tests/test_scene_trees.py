"""Structural census: a scene's tree ops match the loads in its retail range.

    PYTHONPATH=. python3 -m unittest tests.test_scene_trees -v

`Cutscene_AlysWounded` and `Cutscene_PsycoWand` read Krup Inn F1's tree 5 in
this port where the cartridge loads `DialogueTree6` first, and the scene
faulted the moment its dialogue entry landed on an empty line (runner log H17).
Every one of the four missed loads sat under `if revision>0`, so an op-count
test could not see it: the transcriptions had skipped a conditional class.

This test pins the class, not the instance. It re-derives, from the US retail
image, how many `move.l #<tree>, d0 / jsr (DialogueTreesToRAM).l` calls each
transcribed scene's documented byte range holds, and requires the registry's
`SceneOp::SetDialogueTree` count for that scene to be the same number. The
image is the authority - the clone's `if revision` text is only the map that
found the class (see `docs/scenes/REVISION_AUDIT.md`).

What it covers:

- every scene whose documented range holds a tree load, and
- every registry scene that has a `SetDialogueTree` op, which must therefore
  have a documented range: a scene cannot dodge the census by losing its
  byte-range line.

The negative controls at the end show the census reports a dropped op and is
sensitive to the range it scans, so a green run means agreement rather than a
vacuous pass.
"""
from __future__ import annotations

import re
import unittest
from pathlib import Path

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
ROOT = Path(__file__).resolve().parents[1]
SCENES_DIR = ROOT / "rust" / "psiv-core" / "src" / "scenes"
DOCS_DIR = ROOT / "docs" / "scenes"

# `move.l #<tree>, d0` then `jsr (DialogueTreesToRAM).l` at $53F00.
TREE_LOAD = bytes.fromhex("203c") + b"\x00\x00\x00\x00" + bytes.fromhex("4eb900053f00")

STATIC_RE = re.compile(r"pub static ([A-Z_0-9]+): Scene = Scene \{(.*?)\n\};", re.S)
NAME_RE = re.compile(r'name:\s*"([^"]+)"')
OPS_SYMBOL_RE = re.compile(r"ops:\s*([A-Z_0-9]+)\s*,")
OPS_STATIC_RE = re.compile(
    r"static ([A-Z_0-9]+):\s*&\[SceneOp\]\s*=\s*&(?:\[|\{)(.*?)\n\};", re.S
)
OPS_FN_RE = re.compile(r"fn ([a-z_0-9]+)\(\)\s*->\s*&'static \[SceneOp\]\s*\{(.*?)\n\}", re.S)
SET_TREE_RE = re.compile(r"SetDialogueTree\s*\{")
TITLE_RE = re.compile(r"^#\s+(?:\d+\s*[—–-]\s*)?`([A-Za-z_0-9]+)`", re.M)
RETAIL_BYTES_RE = re.compile(
    r"\*\*Retail bytes:\*\*\s*`\$([0-9A-F]{6})\.\.\$([0-9A-F]{6})`"
)
RETAIL_RANGE_RE = re.compile(
    r"\|\s*Retail range\s*\|\s*\*\*`\$([0-9A-F]{6})`\s*\.\.\s*`\$([0-9A-F]{6})`\*\*"
)
REGISTRY_ROW_RE = re.compile(
    r"\|\s*`([A-Za-z_0-9]+)`\s*\|\s*`\$[0-9A-F]+`\s*\|[^|]*\|\s*"
    r"`\$([0-9A-F]{6})\.\.\$([0-9A-F]{6})`\s*\|"
)


def registry_tree_ops(sources: dict[str, str] | None = None) -> dict[str, int]:
    """Scene name -> `SetDialogueTree` op count, following `ops: X_OPS`."""
    sources = sources if sources is not None else {
        path.name: path.read_text(encoding="utf-8") for path in SCENES_DIR.glob("*.rs")
    }
    bodies: dict[str, str] = {}
    for text in sources.values():
        for name, body in OPS_STATIC_RE.findall(text):
            bodies[name] = body
        for name, body in OPS_FN_RE.findall(text):
            bodies[name] = body
    counts: dict[str, int] = {}
    for text in sources.values():
        for _, body in STATIC_RE.findall(text):
            name = NAME_RE.search(body)
            if name is None:
                continue
            symbol = OPS_SYMBOL_RE.search(body)
            ops = bodies.get(symbol.group(1), "") if symbol else body
            counts[name.group(1)] = len(SET_TREE_RE.findall(ops))
    return counts


def documented_ranges() -> dict[str, list[tuple[int, int]]]:
    """Scene name -> every `(start, end_exclusive)` its docs claim."""
    ranges: dict[str, list[tuple[int, int]]] = {}

    def add(name: str, start: int, end: int) -> None:
        ranges.setdefault(name, []).append((start, end))

    for path in sorted(DOCS_DIR.glob("*.md")):
        text = path.read_text(encoding="utf-8")
        title = TITLE_RE.search(text)
        for match in RETAIL_BYTES_RE.finditer(text):
            if title:
                add(title.group(1), int(match.group(1), 16), int(match.group(2), 16) + 1)
        for match in RETAIL_RANGE_RE.finditer(text):
            if title:
                add(title.group(1), int(match.group(1), 16), int(match.group(2), 16) + 1)
    readme = (DOCS_DIR / "README.md").read_text(encoding="utf-8")
    for match in REGISTRY_ROW_RE.finditer(readme):
        add(match.group(1), int(match.group(2), 16), int(match.group(3), 16) + 1)
    return ranges


def tree_load_offsets(rom: bytes, start: int, end: int) -> list[int]:
    """Offsets of the ten-byte tree-load pattern inside `[start, end)`."""
    hits = []
    for at in range(start, max(start, end - len(TREE_LOAD) + 1)):
        window = rom[at : at + len(TREE_LOAD)]
        if window[:2] == TREE_LOAD[:2] and window[6:] == TREE_LOAD[6:]:
            hits.append(at)
    return hits


def census(rom: bytes, registry: dict[str, int], ranges: dict[str, list[tuple[int, int]]]):
    """Mismatch messages: registry vs image, plus scenes the census cannot see."""
    problems = []
    for name in sorted(registry):
        counts = {len(tree_load_offsets(rom, start, end)) for start, end in ranges.get(name, [])}
        if not counts:
            if registry[name]:
                problems.append(
                    f"{name}: registry has {registry[name]} SetDialogueTree op(s) but no "
                    "documented retail range to check them against"
                )
            continue
        if len(counts) > 1:
            problems.append(f"{name}: documented ranges disagree on the load count {sorted(counts)}")
            continue
        retail = counts.pop()
        if retail != registry[name]:
            problems.append(
                f"{name}: retail bytes load {retail} dialogue tree(s), registry has "
                f"{registry[name]} SetDialogueTree op(s)"
            )
    return problems


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class SceneTreeCensus(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()
        cls.registry = registry_tree_ops()
        cls.ranges = documented_ranges()

    def test_every_scene_matches_the_tree_loads_in_its_retail_bytes(self) -> None:
        problems = census(self.rom, self.registry, self.ranges)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_the_scenes_this_test_was_written_for_are_in_the_census(self) -> None:
        """Guards the guard: the census must cover the H17 scenes themselves."""
        for name, count in [
            ("Cutscene_AlysWounded", 3),
            ("Cutscene_PsycoWand", 3),
            ("Cutscene_ZioDefeated", 1),
        ]:
            self.assertIn(name, self.ranges, f"{name} has no documented retail range")
            self.assertEqual(self.registry.get(name), count, name)


class NegativeControl(unittest.TestCase):
    """The census fails when the port drops an op or the range moves."""

    def setUp(self) -> None:
        if not ROM.exists():
            self.skipTest(f"ROM fixture not present at {ROM}")
        self.rom = ROM.read_bytes()
        self.registry = registry_tree_ops()
        self.ranges = documented_ranges()

    def test_a_dropped_tree_op_is_reported(self) -> None:
        sources = {
            path.name: path.read_text(encoding="utf-8")
            for path in SCENES_DIR.glob("*.rs")
        }
        text = sources["post_rika_cutscenes.rs"]
        # Cutscene_AlysWounded's first tree-6 load, exactly as H17 left it out.
        dropped = text.replace(
            "        SceneOp::SetDialogueTree { rom_addr: TREE_6 },\n", "", 1
        )
        self.assertNotEqual(dropped, text)
        sources["post_rika_cutscenes.rs"] = dropped
        problems = census(self.rom, registry_tree_ops(sources), self.ranges)
        self.assertIn(
            "Cutscene_AlysWounded: retail bytes load 3 dialogue tree(s), registry has "
            "2 SetDialogueTree op(s)",
            problems,
        )

    def test_a_shifted_range_stops_finding_the_load(self) -> None:
        start, end = self.ranges["Cutscene_AlysWounded"][0]
        offsets = tree_load_offsets(self.rom, start, end)
        self.assertEqual(len(offsets), 3)
        # Six bytes past the first `move.l #<tree>, d0` skips that load whole:
        # the scan is positional, not a loose substring match.
        self.assertEqual(len(tree_load_offsets(self.rom, offsets[0] + 6, end)), 2)


if __name__ == "__main__":
    unittest.main()
