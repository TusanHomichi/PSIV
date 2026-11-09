"""The cutscene-return reload table (`rust/psiv-core/src/scenes/field_reload.rs`).

    PYTHONPATH=. python3 -m unittest tests.test_oracle_battle_fixture_reload -v

Two groups. The table cases need only the repository: the Rust table parses,
is sorted, and names exactly the maps the pack has; the maps whose row is a
first-load proxy are listed. The oracle cases re-measure forced fixtures;
they do not replay the organic captures and skip with a
message when the ROM, the emulation core or the pack is absent.
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from oracle import reload_frames

ROOT = Path(__file__).resolve().parent.parent
TABLE = ROOT / "rust/psiv-core/src/scenes/field_reload.rs"
ROM = ROOT / "Phantasy Star IV (USA).md"
CORE = ROOT / "oracle/core/genesis_plus_gx_libretro.so"
PACK = ROOT / "runtime-pack/manifest.json"

#: The two flight landings (tape 35) and the principal's office (tape 07).
ANCHORS = {0x18D: (52, 52), 0xBF: (42, 42), 0x14: (54, 55)}
# These are forced fixture observations, before any organic table override.
FIXTURE_ANCHORS = {0x18D: (52, 53), 0xBF: (42, 43), 0x14: (54, 55)}


def table_rows() -> dict[int, tuple[int, int]]:
    text = TABLE.read_text()
    body = text[text.index("static ROWS"):]
    body = body[: body.index("];")]
    return {int(m.group(1), 16): (int(m.group(2)), int(m.group(3)))
            for m in re.finditer(r"\((0x[0-9a-fA-F]+), (\d+), (\d+)\)", body)}


def anchor_mismatches(rows: dict[int, tuple[int, int]]) -> list[int]:
    return [m for m, expected in ANCHORS.items() if rows.get(m) != expected]


def proxy_maps() -> set[int]:
    text = TABLE.read_text()
    body = text[text.index("PROXY_MAPS"):]
    body = body[body.index("// BEGIN GENERATED PROXY"):body.index("// END GENERATED PROXY")]
    return {int(m, 16) for m in re.findall(r"0x[0-9a-fA-F]+", body)}


class TableCase(unittest.TestCase):
    def test_the_table_parses_and_is_sorted_and_includes_the_fade(self):
        rows = table_rows()
        self.assertGreater(len(rows), 300)
        self.assertEqual(list(rows), sorted(rows))
        self.assertTrue(all(16 <= plain <= music < 400 for plain, music in rows.values()))

    def test_the_anchors_are_the_organic_counts(self):
        rows = table_rows()
        self.assertEqual(anchor_mismatches(rows), [])

    def test_negative_control_a_doctored_anchor_is_caught(self):
        rows = dict(table_rows())
        rows[0x14] = (53, 55)
        self.assertEqual(anchor_mismatches(rows), [0x14])
        del rows[0x18D]
        self.assertEqual(anchor_mismatches(rows), [0x18D, 0x14])

    def test_every_pack_map_has_a_row_and_the_proxies_are_named(self):
        if not PACK.is_file():
            self.skipTest("runtime pack absent")
        manifest = json.loads(PACK.read_text())
        packed = {entry["id"] for entry in manifest["maps"]}
        rows = set(table_rows())
        self.assertEqual(packed - rows, set(), "maps with no row")
        self.assertEqual(rows - packed, set(), "rows for maps the pack lacks")
        self.assertTrue(proxy_maps() <= rows)
        self.assertTrue(proxy_maps().isdisjoint(ANCHORS), "an anchor is organic, not a proxy")


class MethodCase(unittest.TestCase):
    @staticmethod
    def log(modes):
        return [{"frame": str(7200 + i), "game_mode": f"{mode:04X}"}
                for i, mode in enumerate(modes)]

    def test_the_reload_is_the_run_of_mode_8_rows_from_the_cutscene_frame(self):
        rows = self.log([8, 8, 8, 12, 12])
        self.assertEqual(reload_frames.reload_rows(rows), 3)
        # A run that began before the cutscene frame is not counted.
        early = [{"frame": "7100", "game_mode": "0008"}] + rows
        self.assertEqual(reload_frames.reload_rows(early), 3)

    def test_negative_control_a_run_that_never_reloads_or_never_returns_is_rejected(self):
        with self.assertRaises(ValueError):
            reload_frames.reload_rows(self.log([12, 12, 12]))
        with self.assertRaises(ValueError):
            reload_frames.reload_rows(self.log([8, 8, 8]))

    def test_attempts_retain_distinct_replayable_evidence_even_when_they_fail(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "oracle").mkdir()
            (root / "oracle/ram_map.tsv").write_text("ram map")
            for name in ("rom", "core", "manifest.json", "host", "source.tape"):
                (root / name).write_text(name)
            out = root / "results"
            out.mkdir()

            def run(command, **kwargs):
                log = Path(command[command.index("--out") + 1])
                log.write_text("frame,game_mode\n7000,0008\n7001,000C\n"
                               "7200,0008\n7201,0008\n7202,000C\n")
                return subprocess.CompletedProcess(command, 0, b"host output")

            with (patch.object(reload_frames, "ROOT", root),
                  patch.object(reload_frames, "ROM", "rom"),
                  patch.object(reload_frames, "CORE", "core"),
                  patch.object(reload_frames, "PACK", root),
                  patch.object(reload_frames.host_binary, "ensure", return_value=root / "host"),
                  patch.object(reload_frames.subprocess, "run", side_effect=run) as runner):
                for at in (None, (1, 2), (3, 4), None):
                    self.assertEqual(reload_frames.run_one(0x14, 1, root / "source.tape", out,
                                                         at=at), 2)
                self.assertEqual(reload_frames.run_one(0x14, 1, root / "source.tape", out,
                                                     first_load=True), 1)
                runner.side_effect = None
                runner.return_value = subprocess.CompletedProcess([], 7, b"failed host")
                with self.assertRaisesRegex(RuntimeError, "oracle exited 7"):
                    reload_frames.run_one(0x14, 1, root / "source.tape", out)
                runner.side_effect = lambda command, **kwargs: subprocess.CompletedProcess(
                    command, 0, b"no log produced")
                with self.assertRaises(FileNotFoundError):
                    reload_frames.run_one(0x14, 1, root / "source.tape", out)

            attempts = list(out.iterdir())
            self.assertEqual(len(attempts), 7)
            statuses = []
            for attempt in attempts:
                receipt = json.loads((attempt / "receipt.json").read_text())
                statuses.append(receipt["exit_status"])
                self.assertEqual((attempt / "input.tape").read_bytes(), b"source.tape")
                self.assertEqual(receipt["input_sha256"]["tape"],
                                 hashlib.sha256(b"source.tape").hexdigest())
                self.assertEqual(Path(receipt["command"][receipt["command"].index("--tape") + 1]),
                                 attempt / "input.tape")
                self.assertTrue((attempt / "stdout.log").is_file())
                if (attempt / "stdout.log").read_bytes() == b"host output":
                    self.assertTrue((attempt / "frames.csv").is_file())
            self.assertEqual(sorted(statuses), [0, 0, 0, 0, 0, 0, 7])


@unittest.skipUnless(shutil.which("gcc"), "PC trace transport probe needs gcc")
class PcTraceCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scratch = tempfile.TemporaryDirectory()
        cls.root = Path(cls.scratch.name)
        cls.binary = cls.root / "probe"
        subprocess.run(["gcc", "-Wall", "-Wextra", "-Werror", "-rdynamic",
                        "-I", str(ROOT / "oracle/host"),
                        str(ROOT / "tests/fixtures/pc_trace_probe.c"),
                        str(ROOT / "oracle/host/pc_trace.c"), "-ldl",
                        "-o", str(cls.binary)], check=True, capture_output=True)

    @classmethod
    def tearDownClass(cls):
        cls.scratch.cleanup()

    def run_probe(self, mode="capture", watch="051790", start="3", path=None):
        path = str(path if path is not None else self.root / f"{mode}.csv")
        return subprocess.run([str(self.binary), mode, path, watch, start],
                              capture_output=True, text=True)

    def test_trace_records_instruction_cycles_only_from_requested_frame(self):
        result = self.run_probe()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([line for line in (self.root / "capture.csv").read_text().splitlines()
                          if not line.startswith("#")],
                         ["frame,pc,cycles", "3,051790,100", "3,051790,200"])

    def test_incomplete_captures_fail_instead_of_looking_exact(self):
        for mode in ("drop", "badget", "missing"):
            with self.subTest(mode=mode):
                result = self.run_probe(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(result.stderr.strip())

    def test_invalid_trace_configuration_is_rejected(self):
        for watch, start, path in [("-", "3", None), ("051791", "3", None),
                                   ("1000000", "3", None), ("junk", "3", None),
                                   ("051790", "0", None), ("051790", "-1", None),
                                   ("051790", "3", "-")]:
            with self.subTest(watch=watch, start=start, path=path):
                self.assertNotEqual(self.run_probe("config", watch, start, path).returncode, 0)
        self.assertNotEqual(self.run_probe("short").returncode, 0)


class OracleCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        missing = [p.name for p in (ROM, CORE, PACK) if not p.exists()]
        if missing:
            raise unittest.SkipTest(f"local inputs absent: {', '.join(missing)}")

    def test_the_forced_fixture_counts_and_flag_effects(self):
        rows = table_rows()
        out = ROOT / "build/reload-tests" / str(time.time_ns())
        out.mkdir(parents=True)
        print(f"reload fixture evidence: {out}", flush=True)
        tape = reload_frames.reload_tape(out)
        for map_id in ANCHORS:
            music = reload_frames.raw_music(map_id)
            same = reload_frames.run_one(map_id, music, tape, out)
            other = reload_frames.run_one(map_id, (music + 1) & 0xFF, tape, out)
            self.assertEqual((same, other), FIXTURE_ANCHORS[map_id],
                             f"{map_id:#x} forced fixture (not organic)")
        # Bit 7 of Map_Load_Flags skips Pal_FadeIn's 16 frames.
        map_id = 0x14
        music = reload_frames.raw_music(map_id)
        skipped = reload_frames.run_one(map_id, music, tape, out, bits=0x80)
        self.assertEqual(skipped, rows[map_id][0] - 16)
        # Bit 0 keeps the music branch from running at all.
        bit0 = reload_frames.run_one(map_id, (music + 1) & 0xFF, tape, out, bits=0x01)
        self.assertEqual(bit0, rows[map_id][0])


if __name__ == "__main__":
    unittest.main()
