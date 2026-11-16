"""Numeric title layout and runtime-pack front-door contracts."""

import re
import unittest
from pathlib import Path

from oracle.decode_layout import TITLE_ELEMENTS, decode_title_summary



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






def _png_size(data):
    # IHDR starts after the 8-byte signature and 8-byte length/type prefix.
    return tuple(int.from_bytes(data[offset:offset + 4], "big") for offset in (16, 20))
