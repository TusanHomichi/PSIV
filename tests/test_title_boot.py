"""Numeric title layout and runtime-pack front-door contracts."""

import builtins
import json
import os
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from oracle.decode_layout import TITLE_ELEMENTS, decode_title_summary
from psiv_tools import m68k, title_pack

try:
    from test_pack_fixture import PackFixtureCase
except ImportError:  # pragma: no cover
    from tests.test_pack_fixture import PackFixtureCase


def _plane(fill=0):
    words = [[f"0x{fill:04X}" for _ in range(40)] for _ in range(28)]
    return {
        "visible_words": words,
        "visible_nonzero_cells": 0,
        "chrome_rectangles": [],
        "text_runs": [],
    }


def _put(plane, x, y, width, height, base, palette=0):
    count = 0
    for row in range(height):
        for column in range(width):
            pattern = base + ((row * width + column) % max(1, width))
            plane["visible_words"][y + row][x + column] = f"0x{(palette << 13) | pattern:04X}"
            count += 1
    plane["visible_nonzero_cells"] += count


class TestTitleDecoderContract(unittest.TestCase):
    def test_decoded_title_rectangles_are_the_retail_numbers(self):
        by_name = {element["name"]: element for element in TITLE_ELEMENTS}
        self.assertEqual(by_name["sega_logo"]["cell_rect"], {"x": 12, "y": 11, "width": 17, "height": 5})
        self.assertEqual(by_name["title_logo"]["cell_rect"], {"x": 11, "y": 3, "width": 17, "height": 13})
        self.assertEqual(by_name["subtitle"]["cell_rect"], {"x": 5, "y": 17, "width": 29, "height": 3})
        self.assertEqual(by_name["press_start"]["pattern_range"], [0xD8, 0xE2])
        self.assertEqual(by_name["copyright"]["pattern_range"], [0xCA, 0xD6])

    def test_final_title_summary_reports_background_and_palette_numbers(self):
        plane_a = _plane()
        _put(plane_a, 11, 3, 17, 13, 0x10)
        _put(plane_a, 5, 17, 29, 3, 0x92)
        _put(plane_a, 11, 22, 18, 1, 0xD8, 2)
        _put(plane_a, 12, 25, 17, 1, 0xCA, 1)
        plane_b = _plane()
        _put(plane_b, 0, 0, 40, 28, 0xE3, 3)
        cram = {"lines": [{"line": line, "words": ["0x0000"] * 16} for line in range(4)]}
        summary = decode_title_summary(plane_a, plane_b, cram, 500)
        self.assertEqual(summary["phase"], "title")
        self.assertEqual(summary["background"]["observed"]["nonzero_cells"], 1120)
        self.assertEqual(summary["background"]["mapping"]["base_tile"], "0x0E3")
        self.assertEqual(summary["elements"]["title_logo"]["observed"]["nonzero_cells"], 221)
        self.assertEqual(summary["elements"]["press_start"]["observed"]["palette_lines"], [2])
        self.assertEqual(summary["palette_cycle"]["lines"]["2"], [0] * 16)


class TestTitlePack(PackFixtureCase):
    def test_manifest_emits_title_assets_additively(self):
        title = self.manifest["title"]
        self.assertEqual(title["layout"]["path"], "title/layout.json")
        self.assertEqual(title["background"]["png"], "title/background.png")
        self.assertEqual(
            title["background_transfer"]["png"],
            "title/titlebarbgtoppart.png",
        )
        layout = json.loads((self.root / title["layout"]["path"]).read_text())
        self.assertEqual(layout["screen"], {"width_pixels": 320, "height_pixels": 224})
        self.assertEqual(layout["background"]["placements"][0]["x_pixels"], 0)
        self.assertEqual(layout["background"]["placements"][-1]["x_pixels"], 112)
        self.assertEqual(layout["background_transfer"]["size_pixels"], [64, 224])
        self.assertEqual(
            layout["palette_cycle"]["frames"],
            [25, 50, 75, 100, 125, 150, 175, 200, 300, 400, 401, 450, 500, 550, 600, 650],
        )
        replay = self.root / "title" / "replay" / "frame_650"
        self.assertTrue((replay / "background.png").is_file())
        self.assertTrue((replay / "title_logo.png").is_file())
        self.assertTrue((self.root / title["background_transfer"]["png"]).is_file())
        for name, size in {
            "sega_logo": (136, 40),
            "title_logo": (136, 104),
            "subtitle": (232, 24),
            "press_start": (144, 8),
            "copyright": (136, 8),
        }.items():
            with self.subTest(asset=name):
                path = self.root / title["assets"][name]["png"]
                self.assertTrue(path.is_file())
                self.assertEqual(_png_size(path.read_bytes()), size)


REPO = Path(__file__).resolve().parents[1]
ROM = REPO / "Phantasy Star IV (USA).md"
TAPE = REPO / "oracle" / "tapes" / "25_title_boot.tape"
# Oracle evidence only: tape 25's captured Palette_Table_Buffer words. It is an
# ignored local capture, so the comparison skips on a clone without it.
CAPTURE = REPO / "oracle" / "layouts" / "title" / "palette_cycle.json"


def _title_rs_main_frame_offset():
    source = (REPO / "rust" / "psiv-godot" / "src" / "title.rs").read_text(encoding="utf-8")
    match = re.search(r"const TITLE_MAIN_FRAME_OFFSET: u32 = (\d+);", source)
    if match is None:
        raise AssertionError("title.rs no longer declares TITLE_MAIN_FRAME_OFFSET")
    return int(match.group(1))


def _tape_start_frames(path):
    """1-based frames on which a tape holds START (`count buttons label`)."""
    frame = 0
    held = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        count, buttons = line.split()[:2]
        for _ in range(int(count)):
            frame += 1
            if "S" in buttons:
                held.append(frame)
    return held


class TestTitleCycleInputs(unittest.TestCase):
    def test_start_presses_are_tape_25s(self):
        expected = [frame for span in title_pack.TAPE25_START_HELD for frame in span]
        self.assertEqual(_tape_start_frames(TAPE), expected)

    def test_sampled_frames_fall_inside_tape_25(self):
        frames = title_pack.PALETTE_CYCLE_FRAMES
        self.assertEqual(list(frames), sorted(set(frames)))
        tape_length = sum(
            int(line.split()[0])
            for line in TAPE.read_text(encoding="utf-8").splitlines()
            if line.split("#", 1)[0].strip()
        )
        self.assertLessEqual(frames[-1], tape_length)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTitleCycleFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = ROM.read_bytes()

    def _capture(self):
        if not CAPTURE.is_file():
            self.skipTest(
                f"tape 25 CRAM capture not present at {CAPTURE}; "
                "the ROM-derived cycle cannot be compared on this clone"
            )
        return json.loads(CAPTURE.read_text(encoding="utf-8"))

    def _mismatches(self, derived, capture):
        """Frames whose 64 words or changed-entry list differ from tape 25."""
        return sorted(
            (frame for frame in capture["frames"]
             if derived["frames"].get(frame) != capture["frames"][frame]),
            key=int,
        )

    def _patched(self, edits):
        rom = bytearray(self.rom)
        for at, value in edits.items():
            rom[at:at + len(value)] = value
        return bytes(rom)

    def test_derived_cycle_matches_the_tape_25_capture(self):
        capture = self._capture()
        derived = title_pack.derive_palette_cycle(self.rom)
        self.assertEqual(sorted(derived["frames"], key=int), sorted(capture["frames"], key=int))
        self.assertEqual(self._mismatches(derived, capture), [])
        self.assertEqual(
            (derived["line_count"], derived["colors_per_line"]),
            (capture["line_count"], capture["colors_per_line"]),
        )

    def test_shape_without_the_capture(self):
        derived = title_pack.derive_palette_cycle(self.rom)
        self.assertEqual(
            [int(frame) for frame in derived["frames"]], list(title_pack.PALETTE_CYCLE_FRAMES)
        )
        first = derived["frames"]["25"]
        self.assertEqual(first["changed_entries_from_previous"], [])
        self.assertEqual(sorted(first["lines"]), ["0", "1", "2", "3"])
        for frame in derived["frames"].values():
            for words in frame["lines"].values():
                self.assertEqual(len(words), 16)
                self.assertTrue(all(0 <= word <= 0xFFFF for word in words))

    # Negative controls: each perturbs one input the derivation reads and must
    # break the comparison at the frames that input governs. The expected
    # frames are where tape 25's capture changes for that reason, not values
    # recomputed by the code under test.
    def test_control_sega_cycle_period(self):
        # cmpi.w #5 -> #6 at $41116: every logo step lands later.
        capture = self._capture()
        derived = title_pack.derive_palette_cycle(self._patched({0x41116: b"\x00\x06"}))
        # Frame 50 still lands on the same step; 200 is past the last one.
        self.assertEqual(self._mismatches(derived, capture), ["75", "100", "125", "150", "175"])

    def test_control_sega_shine_table(self):
        # loc_411C0's second byte: the shine sweep's leading colour at frame
        # 100, which also changes frame 125's changed-entry list.
        capture = self._capture()
        derived = title_pack.derive_palette_cycle(self._patched({0x411C1: b"\x02"}))
        self.assertEqual(self._mismatches(derived, capture), ["100", "125"])

    def test_control_title_palette(self):
        # Pal_TitleScreen line 3, entry 1: the settled title from frame 300 on.
        capture = self._capture()
        derived = title_pack.derive_palette_cycle(self._patched({0x1D2A1E: b"\x00\x02"}))
        self.assertEqual(
            self._mismatches(derived, capture),
            ["300", "400", "401", "450", "500", "550", "600", "650"],
        )

    def test_control_press_start_table(self):
        # Every PressStartButtonCyclingPal word: the prompt at 550 and 600.
        capture = self._capture()
        # (frame 500's changed-entry list then differs too).
        table = self._patched({0x42CE6 + 2 * index: b"\x00\x02" for index in range(32)})
        derived = title_pack.derive_palette_cycle(table)
        self.assertEqual(self._mismatches(derived, capture), ["500", "550", "600"])

    def test_control_menu_palette(self):
        # Pal_Init_Line_3's third word: the menu line at frame 650.
        capture = self._capture()
        derived = title_pack.derive_palette_cycle(self._patched({0x296304: b"\x00\x02"}))
        self.assertEqual(self._mismatches(derived, capture), ["650"])

    def test_control_text_white(self):
        # loc_427DA's move.w #$EEE: the text entries START whitens at 401.
        capture = self._capture()
        derived = title_pack.derive_palette_cycle(self._patched({0x427E8: b"\x0C\xCC"}))
        self.assertEqual(
            self._mismatches(derived, capture),
            ["401", "450", "500", "550", "600", "650"],
        )

    def test_control_boot_lag(self):
        # One extra power-on lag frame: the Sega fade-in reads one step behind.
        capture = self._capture()
        lag = {name: dict(steps) for name, steps in title_pack.TITLE_LAG_FRAMES.items()}
        lag["GameMode_Sega"][0] += 1
        derived = title_pack.derive_palette_cycle(self.rom, lag=lag)
        self.assertEqual(self._mismatches(derived, capture), ["25", "50"])

    def test_control_sampled_frame(self):
        # Sampling one frame late: frame 26 is not frame 25's fade step.
        capture = self._capture()
        frames = tuple(frame + 1 for frame in title_pack.PALETTE_CYCLE_FRAMES)
        shifted = title_pack.derive_palette_cycle(self.rom, frames)
        shifted["frames"] = {
            str(int(frame) - 1): words for frame, words in shifted["frames"].items()
        }
        self.assertEqual(
            self._mismatches(shifted, capture),
            ["25", "50", "75", "100", "125", "400", "401", "600"],
        )

    def test_control_start_press(self):
        # START one frame later: frame 401 still shows the black text entries.
        capture = self._capture()
        held = (range(402, 406), title_pack.TAPE25_START_HELD[1])
        derived = title_pack.derive_palette_cycle(self.rom, start_held=held)
        self.assertEqual(self._mismatches(derived, capture), ["401", "450"])

    def _prompt_offsets(self, lag=None):
        boot = title_pack.run_title_boot(self.rom, lag=lag)
        first = boot.entries["TitleRoutine_PressStartButton"]
        last = boot.entries["loc_42C0E"]
        return {boot.vint_frames[k] - k for k in range(first + 1, last + 1)}

    def test_prompt_main_frame_offset_matches_title_rs(self):
        # The capture samples the prompt too sparsely to pin the title's lag
        # frames to one frame, so the prompt's tape-frame minus
        # Main_Frame_Count is checked against title.rs's calibrated
        # TITLE_MAIN_FRAME_OFFSET, read from that file.
        self.assertEqual(self._prompt_offsets(), {_title_rs_main_frame_offset()})

    def test_control_title_lag(self):
        # One extra lag frame in the title's art load: the capture still
        # matches (see above), the prompt offset does not.
        lag = {name: dict(steps) for name, steps in title_pack.TITLE_LAG_FRAMES.items()}
        lag["TitleRoutine_FadingText"][0] += 1
        self.assertEqual(self._prompt_offsets(lag), {_title_rs_main_frame_offset() + 1})

    def test_control_wrong_instruction_fails_closed(self):
        # A different opcode where DoSegaPaletteCycling's cmpi.w sits.
        with self.assertRaises(m68k.DecodeError):
            title_pack.derive_palette_cycle(self._patched({0x41114: b"\x0C\x78"}))

    def test_unmodelled_timeout_fails_closed(self):
        # Without the second START the prompt times out into a routine the
        # model does not follow; it must say so rather than guess.
        held = (title_pack.TAPE25_START_HELD[0],)
        with self.assertRaises(title_pack.TitleCycleError):
            title_pack.derive_palette_cycle(self.rom, start_held=held)

    def test_emit_title_opens_no_oracle_path(self):
        # The pack build reads the ROM, not oracle captures (#214). Paths are
        # compared unresolved: oracle/layouts is often a symlinked input.
        opened = []
        real_open = builtins.open

        def recording_open(file, *args, **kwargs):
            if isinstance(file, (str, Path)):
                opened.append(os.path.abspath(file))
            return real_open(file, *args, **kwargs)

        with tempfile.TemporaryDirectory() as out, \
                mock.patch("builtins.open", recording_open), \
                mock.patch("io.open", recording_open):
            title_pack.emit_title(self.rom, out)
        oracle = os.path.join(os.path.abspath(REPO), "oracle") + os.sep
        self.assertTrue(opened, "the recorder saw no file opens")
        self.assertEqual([path for path in opened if path.startswith(oracle)], [])


def _png_size(data):
    # IHDR starts after the 8-byte signature and 8-byte length/type prefix.
    return tuple(int.from_bytes(data[offset:offset + 4], "big") for offset in (16, 20))
