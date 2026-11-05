"""Guard: every scene layout write resolves to a chunk the pack's atlas carries.

    PYTHONPATH=. python3 -m unittest tests.test_scene_chunk_atlas -v

`Cutscene_CrashLanding` wrote a Raja Temple chunk the pack did not carry, and
the runtime refused it (`scene chunk atlas is absent`, #67); `Event_Tyler
GraveOpening` did the same on Tyler. Both were found by a route halting. The
runtime resolves a `SceneOp::WriteMapChunks` against the *current map's* atlas
(`write_scene_map_chunks`): a raw tile with that chunk id, or, on the
overworlds, the composed tile a paged hook carries at the same coordinate.

This test closes the class from both ends:

1. every `WriteMapChunks` in the scene registry has a row in
   `psiv_tools.map_patches.SCENE_CHUNK_WRITES` (scene, map, chunk, citation),
   so a newly transcribed write cannot go uncited; and
2. every row's chunks resolve in the pack for that map, by the rule the runtime
   applies. It reads `PSIV_RUNTIME_PACK`, else the repo's `runtime-pack`, and
   skips with a message when there is none. A pack built before a row was added
   fails here by design: rebuild it (`docs/DEVELOPMENT.md`).

The negative controls remove a row, and remove a chunk from a pack's atlas, and
show each failure names the scene and the chunk.

The Rust pack builder (`rust/psiv-pack/src/maps/patches.rs`) carries its own
copy of the rows that add atlas chunks (`via` `"scene"`). `RustBuilderRows`
holds the two to the same scenes, maps, chunks, tables and pinned bytes without
the ROM, so a row added to one builder only fails here rather than in the full
gate's pack parity (the Wreckage row did, #333).
"""
from __future__ import annotations

import json
import os
import re
import unittest
from pathlib import Path

from psiv_tools import map_patches
from tests.test_scene_trees import (
    NAME_RE,
    OPS_FN_RE,
    OPS_STATIC_RE,
    OPS_SYMBOL_RE,
    SCENES_DIR,
    STATIC_RE,
)

ROOT = Path(__file__).resolve().parents[1]
RUST_PATCHES = ROOT / "rust" / "psiv-pack" / "src" / "maps" / "patches.rs"
ROM = ROOT / "Phantasy Star IV (USA).md"
PACK = Path(os.environ.get("PSIV_RUNTIME_PACK", ROOT / "runtime-pack"))

WRITE_RE = re.compile(r"WriteMapChunks\s*\{\s*chunks:\s*&\[(.*?)\]\s*,?\s*\}", re.S)
TUPLE_RE = re.compile(r"\(\s*(0x[0-9A-Fa-f]+|\d+)\s*,\s*(0x[0-9A-Fa-f]+|\d+)\s*,\s*(0x[0-9A-Fa-f]+|\d+)\s*\)")


RUST_TABLE_RE = re.compile(
    r"const SCENE_CHUNK_WRITES: &\[SceneChunkWrite\] = &\[(.*?)\n\];", re.S
)
RUST_ROW_RE = re.compile(
    r"SceneChunkWrite\s*\{\s*scene:\s*\"([^\"]+)\",\s*map_id:\s*(0x[0-9A-Fa-f]+|\d+),\s*"
    r"chunks:\s*&\[([^\]]*)\],\s*table:\s*(None|Some\(\(\s*(0x[0-9A-Fa-f]+|\d+),\s*(0x[0-9A-Fa-f]+|\d+)\s*\)\)),\s*"
    r"checks:\s*&\[(.*?)\],\s*\}",
    re.S,
)
RUST_CHECK_RE = re.compile(r"\(\s*(0x[0-9A-Fa-f]+|\d+),\s*\"([0-9A-Fa-f]*)\",?\s*\)")

Row = tuple[str, int, tuple[int, ...], "tuple[int, int] | None", tuple[tuple[int, str], ...]]


def rust_rows(text: str | None = None) -> list[Row]:
    """The Rust builder's `SCENE_CHUNK_WRITES`, sorted; every row must parse."""
    text = text if text is not None else RUST_PATCHES.read_text(encoding="utf-8")
    table = RUST_TABLE_RE.search(text)
    if table is None:
        raise ValueError(f"no SCENE_CHUNK_WRITES table in {RUST_PATCHES}")
    body = table.group(1)
    rows: list[Row] = []
    for match in RUST_ROW_RE.finditer(body):
        scene, map_id, chunks, _, t_offset, t_length, checks = match.groups()
        rows.append(
            (
                scene,
                int(map_id, 0),
                tuple(sorted(int(c, 0) for c in chunks.replace(",", " ").split())),
                (int(t_offset, 0), int(t_length, 0)) if t_offset else None,
                tuple((int(o, 0), h.upper()) for o, h in RUST_CHECK_RE.findall(checks)),
            )
        )
    if len(rows) != body.count("SceneChunkWrite {"):
        raise ValueError("a Rust SCENE_CHUNK_WRITES row does not parse")
    return sorted(rows)


def python_rows(
    rows: tuple[map_patches.SceneChunkWrite, ...] = map_patches.SCENE_CHUNK_WRITES,
) -> list[Row]:
    """Python's rows that add atlas chunks, in `rust_rows`' shape."""
    return sorted(
        (
            row.scene,
            row.map_id,
            tuple(sorted(row.chunks)),
            row.table,
            tuple((o, h.upper()) for o, h in row.checks),
        )
        for row in rows
        if row.via == "scene"
    )


def registry_writes(sources: dict[str, str] | None = None) -> dict[str, list[tuple[int, int, int]]]:
    """Scene name -> every `(chunk x, chunk y, chunk id)` its `WriteMapChunks` ops write."""
    sources = sources if sources is not None else {
        path.name: path.read_text(encoding="utf-8") for path in SCENES_DIR.glob("*.rs")
    }
    bodies: dict[str, str] = {}
    for text in sources.values():
        for name, body in OPS_STATIC_RE.findall(text):
            bodies[name] = body
        for name, body in OPS_FN_RE.findall(text):
            bodies[name] = body
    found: dict[str, list[tuple[int, int, int]]] = {}
    for text in sources.values():
        for _, body in STATIC_RE.findall(text):
            name = NAME_RE.search(body)
            if name is None:
                continue
            symbol = OPS_SYMBOL_RE.search(body)
            ops = bodies.get(symbol.group(1), "") if symbol else body
            writes = [
                tuple(int(v, 0) for v in match)
                for block in WRITE_RE.findall(ops)
                for match in TUPLE_RE.findall(block)
            ]
            if writes:
                found[name.group(1)] = writes
    return found


def uncited_writes(
    writes: dict[str, list[tuple[int, int, int]]],
    rows: tuple[map_patches.SceneChunkWrite, ...] = map_patches.SCENE_CHUNK_WRITES,
    rom: bytes | None = None,
) -> list[str]:
    """Registry writes with no row, naming the scene and the chunk."""
    problems = []
    for scene, tuples in sorted(writes.items()):
        mine = [row for row in rows if row.scene == scene]
        covered: set[int] = set()
        for row in mine:
            covered |= set(row.chunks) if rom is None else map_patches.chunk_ids(rom, row)
        for x, y, chunk in tuples:
            if chunk not in covered:
                problems.append(
                    f"{scene} writes chunk ${chunk:02X} at ({x},{y}) and "
                    "SCENE_CHUNK_WRITES has no row for it"
                )
    return problems


def atlas_resolves(record: dict, chunk: int, writes: list[tuple[int, int, int]]) -> bool:
    """The runtime's rule (`write_scene_map_chunks`): a raw tile, or a composed one."""
    atlas = record.get("patch_tiles")
    if not atlas:
        return False
    if any(tile.get("chunk_id") == chunk for tile in atlas["tiles"]):
        return True
    coords = {(x, y) for x, y, c in writes if c == chunk}
    by_index = {tile["index"] for tile in atlas["tiles"]}
    for patch in record.get("overworld_patches") or ():
        for entry in patch["tiles"]:
            if (
                entry["collision_chunk_id"] == chunk
                and (entry["chunk_x"], entry["chunk_y"]) in coords
                and entry["patch_tile"] in by_index
            ):
                return True
    return False


def unresolved(
    writes: dict[str, list[tuple[int, int, int]]],
    pack: Path,
    rows: tuple[map_patches.SceneChunkWrite, ...] = map_patches.SCENE_CHUNK_WRITES,
    rom: bytes | None = None,
    maps: dict[int, dict] | None = None,
) -> list[str]:
    problems = []
    for row in rows:
        if row.scene not in writes:
            problems.append(f"{row.scene} has a SCENE_CHUNK_WRITES row but no WriteMapChunks in the registry")
            continue
        if maps is not None and row.map_id in maps:
            record = maps[row.map_id]
        else:
            found = list(pack.glob(f"maps/{row.map_id:03X}_*.json"))
            found = [p for p in found if not p.name.endswith("_indices.json")]
            if not found:
                problems.append(f"{row.scene}: the pack has no map ${row.map_id:03X}")
                continue
            record = json.loads(found[0].read_text(encoding="utf-8"))
        for chunk in sorted(set(row.chunks) if rom is None else map_patches.chunk_ids(rom, row)):
            if not atlas_resolves(record, chunk, writes[row.scene]):
                problems.append(
                    f"{row.scene}: chunk ${chunk:02X} does not resolve in the atlas of map "
                    f"${row.map_id:03X} ({record.get('symbol')}); rebuild the pack"
                )
    return problems


class RustBuilderRows(unittest.TestCase):
    """Both builders add the same atlas chunks; no ROM needed."""

    def test_the_rust_builder_has_python_s_scene_rows(self) -> None:
        self.assertEqual(rust_rows(), python_rows())

    def test_the_wreckage_row_is_in_both(self) -> None:
        scenes = {row[0] for row in rust_rows()}
        self.assertIn("Event_WreckageEngine", scenes)
        self.assertIn("Event_WreckageEngine", {row[0] for row in python_rows()})

    # Negative controls: each way the Rust copy can drift is seen.
    def _drifted(self, old: str, new: str) -> list[Row]:
        text = RUST_PATCHES.read_text(encoding="utf-8")
        self.assertEqual(text.count(old), 1, old)
        return rust_rows(text.replace(old, new))

    def test_a_row_missing_from_rust_is_seen(self) -> None:
        text = RUST_PATCHES.read_text(encoding="utf-8")
        start = text.index('    SceneChunkWrite {\n        scene: "Event_WreckageEngine"')
        end = text.index("    },\n", start) + len("    },\n")
        self.assertNotEqual(rust_rows(text[:start] + text[end:]), python_rows())

    def test_a_different_chunk_map_table_or_pin_is_seen(self) -> None:
        for old, new in (
            ("chunks: &[0xA6]", "chunks: &[0xA7]"),
            ("map_id: 0x0B6", "map_id: 0x0B7"),
            ("table: Some((0x76E58, 21))", "table: Some((0x76E58, 22))"),
            ('(0x6DD2A, "13BC00A67020")', '(0x6DD2A, "13BC00A67021")'),
            ('(0x6DD24, "137C00A60020"),\n', ""),
        ):
            with self.subTest(old=old):
                self.assertNotEqual(self._drifted(old, new), python_rows())

    def test_a_row_that_does_not_parse_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            self._drifted("table: None,\n        checks: &[\n            (0x6DD1A", "table: Nope,\n        checks: &[\n            (0x6DD1A")

    def test_python_s_other_rows_are_not_rust_s(self) -> None:
        # `via` other than "scene" are Python's own guard rows.
        self.assertNotIn("Cutscene_PsycoWand", {row[0] for row in rust_rows()})


class WreckagePins(unittest.TestCase):
    """`chunk_ids` refuses a change to any of the Wreckage's four `move.b` sites."""

    def test_each_pinned_write_is_checked(self) -> None:
        row = next(r for r in map_patches.SCENE_CHUNK_WRITES if r.scene == "Event_WreckageEngine")
        self.assertEqual(len(row.checks), 4)
        end = max(offset + len(hex_) // 2 for offset, hex_ in row.checks)
        image = bytearray(end)
        for offset, hex_ in row.checks:
            image[offset : offset + len(hex_) // 2] = bytes.fromhex(hex_)
        self.assertEqual(map_patches.chunk_ids(bytes(image), row), {0xA6})
        for offset, _ in row.checks:
            for delta in (0, 3):  # the opcode, and the chunk it stores
                with self.subTest(offset=hex(offset), byte=delta):
                    moved = bytearray(image)
                    moved[offset + delta] ^= 1
                    with self.assertRaisesRegex(map_patches.MapPatchError, f"\\${offset:06X} moved"):
                        map_patches.chunk_ids(bytes(moved), row)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class SceneChunkAtlas(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.writes = registry_writes()
        cls.rom = ROM.read_bytes()

    def test_the_registry_has_the_writes_the_table_expects(self) -> None:
        self.assertEqual(
            sorted(self.writes),
            sorted({row.scene for row in map_patches.SCENE_CHUNK_WRITES}),
        )

    def test_every_registry_write_has_a_cited_row(self) -> None:
        problems = uncited_writes(self.writes, rom=self.rom)
        self.assertEqual(problems, [], "\n".join(problems))

    def test_every_row_matches_the_image(self) -> None:
        for row in map_patches.SCENE_CHUNK_WRITES:
            self.assertTrue(map_patches.chunk_ids(self.rom, row), row.scene)

    @unittest.skipUnless(
        (PACK / "manifest.json").is_file(),
        f"no runtime pack at {PACK} (set PSIV_RUNTIME_PACK)",
    )
    def test_every_write_resolves_in_the_pack(self) -> None:
        problems = unresolved(self.writes, PACK, rom=self.rom)
        self.assertEqual(problems, [], "\n".join(problems))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class NegativeControl(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.writes = registry_writes()
        cls.rom = ROM.read_bytes()

    def test_a_missing_row_names_the_scene_and_the_chunk(self) -> None:
        rows = tuple(r for r in map_patches.SCENE_CHUNK_WRITES if r.scene != "Event_TylerGraveOpening")
        problems = uncited_writes(self.writes, rows, self.rom)
        self.assertEqual(
            problems,
            ["Event_TylerGraveOpening writes chunk $47 at (10,12) and SCENE_CHUNK_WRITES has no row for it"],
        )

    def test_a_row_missing_one_chunk_names_it(self) -> None:
        rows = tuple(
            map_patches.SceneChunkWrite(
                scene=r.scene, map_id=r.map_id, citation=r.citation, chunks=(0x8F,), via=r.via
            )
            if r.scene == "Cutscene_PsycoWand"
            else r
            for r in map_patches.SCENE_CHUNK_WRITES
        )
        problems = uncited_writes(self.writes, rows, self.rom)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("Cutscene_PsycoWand writes chunk $90", problems[0])

    @unittest.skipUnless(
        (PACK / "manifest.json").is_file(),
        f"no runtime pack at {PACK} (set PSIV_RUNTIME_PACK)",
    )
    def test_a_pack_atlas_without_the_chunk_names_the_scene_and_the_chunk(self) -> None:
        rows = tuple(r for r in map_patches.SCENE_CHUNK_WRITES if r.scene == "Cutscene_CrashLaanding")
        record = json.loads(next(PACK.glob("maps/14C_*.json")).read_text(encoding="utf-8"))
        full = unresolved(self.writes, PACK, rows, self.rom)
        # (the guard for a different scene set is vacuous here: the rows name
        # only the crash landing, so the registry check does not trip)
        self.assertEqual(
            [p for p in full if "Cutscene_CrashLaanding" in p and "does not resolve" in p], []
        )
        record["patch_tiles"]["tiles"] = [
            t for t in record["patch_tiles"]["tiles"] if t.get("chunk_id") != 0x57
        ]
        problems = unresolved(self.writes, PACK, rows, self.rom, maps={0x14C: record})
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("Cutscene_CrashLaanding: chunk $57 does not resolve", problems[0])

    def test_a_moved_write_fails_the_build(self) -> None:
        rom = bytearray(self.rom)
        rom[0x6FCF4 + 3] = 0x48  # the chunk the Tyler write stores
        row = next(r for r in map_patches.SCENE_CHUNK_WRITES if r.scene == "Event_TylerGraveOpening")
        with self.assertRaises(map_patches.MapPatchError):
            map_patches.chunk_ids(bytes(rom), row)


if __name__ == "__main__":
    unittest.main()
