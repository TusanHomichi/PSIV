"""`psiv_tools.warps`: `XYRangeJmpTbl` transcribed into trigger rectangles.

None of this needs the ROM. The rectangles are the fifteen routines' own
arithmetic, so they are checkable against the disassembly by reading, and the
ROM-backed half -- that retail records select only nine of the fifteen, and
that every doorway's rectangle covers a map-change cell -- lives in
`test_pack.py` where the pack is.
"""

import unittest

from psiv_tools.layouts import COLLISION_CELL_PIXELS
from psiv_tools.warps import (
    STANDING_CELL_Y_OFFSET,
    XY_RANGE_NAMES,
    PackError,
    Rect,
    warp_rect,
    xy_range_name,
)


class TestStandingCellShift(unittest.TestCase):
    def test_the_shift_is_one_cell_down(self):
        # `GetChunkAndCollision` does `addi.w #$10,d6` before shifting Y down to
        # a cell, and $10 is one 16-pixel cell.
        self.assertEqual(STANDING_CELL_Y_OFFSET, 1)
        self.assertEqual(warp_rect(0x3, 5, 5, 64, 64).y, 6)


class TestRect(unittest.TestCase):
    def test_cells_are_half_open(self):
        self.assertEqual(
            list(Rect(2, 3, 2, 2).cells()), [(2, 3), (3, 3), (2, 4), (3, 4)]
        )

    def test_an_empty_rect_is_not_a_rect(self):
        with self.assertRaises(PackError):
            Rect(0, 0, 0, 4)
        with self.assertRaises(PackError):
            Rect(0, 0, 4, -1)


class TestWarpRect(unittest.TestCase):
    """`XYRangeJmpTbl`, entry by entry.

    Sizes are the `addi.w` immediates the routines add to the record's
    coordinate before comparing; the accept window is `d0 <= player < d0 + w`,
    so a rectangle is half-open and the immediate is its width in pixels.
    """

    GRID = (64, 64)

    def rect(self, range_id, x=4, y=4):
        return warp_rect(range_id, x, y, *self.GRID)

    def test_every_jump_table_entry_has_a_name(self):
        self.assertEqual(sorted(XY_RANGE_NAMES), list(range(15)))
        self.assertEqual(xy_range_name(0x9), "XPlus20_YPlus10")
        with self.assertRaises(PackError):
            xy_range_name(0xF)

    def test_box_ranges_are_their_addi_immediates(self):
        for range_id, (name, pixels_x, pixels_y) in {
            0x1: ("XYPlus40", 0x40, 0x40),
            0x2: ("XYPlus20", 0x20, 0x20),
            0x9: ("XPlus20_YPlus10", 0x20, 0x10),
            0xA: ("XPlus10_YPlus60", 0x10, 0x60),
            0xB: ("XPlus40_YPlus20", 0x40, 0x20),
            0xC: ("XPlus10_YPlus20", 0x10, 0x20),
            0xD: ("XPlus60_YPlus10", 0x60, 0x10),
            0xE: ("XPlus40_YPlus10", 0x40, 0x10),
        }.items():
            with self.subTest(range=name):
                self.assertEqual(xy_range_name(range_id), name)
                rect = self.rect(range_id)
                self.assertEqual(
                    (rect.width, rect.height),
                    (pixels_x // COLLISION_CELL_PIXELS, pixels_y // COLLISION_CELL_PIXELS),
                )

    def test_a_rect_starts_at_the_record_coordinate_one_row_down(self):
        # GetChunkAndCollision adds $10 to Y before it derives a cell, so the
        # cell a character at curr_y_pos occupies is one row below curr_y_pos/16.
        rect = warp_rect(0x9, 31, 6, *self.GRID)
        self.assertEqual(rect.to_json(), {"x": 31, "y": 7, "width": 2, "height": 1})
        self.assertEqual(STANDING_CELL_Y_OFFSET, 1)

    def test_exact_is_a_single_cell(self):
        self.assertEqual(
            self.rect(0x3, 22, 20).to_json(), {"x": 22, "y": 21, "width": 1, "height": 1}
        )

    def test_null_never_fires(self):
        self.assertIsNone(self.rect(0x0))

    def test_half_plane_ranges_run_to_the_map_edge(self):
        # XLower/YLower accept a player at or before the coordinate, XHigher/
        # YHigher at or after it; the other bound is the grid.
        self.assertEqual(
            self.rect(0x4, 10, 20).to_json(), {"x": 0, "y": 0, "width": 11, "height": 64}
        )
        self.assertEqual(
            self.rect(0x5, 10, 20).to_json(), {"x": 10, "y": 0, "width": 54, "height": 64}
        )
        self.assertEqual(
            self.rect(0x6, 10, 20).to_json(), {"x": 0, "y": 0, "width": 64, "height": 22}
        )
        self.assertEqual(
            self.rect(0x7, 10, 20).to_json(), {"x": 0, "y": 21, "width": 64, "height": 43}
        )

    def test_xy_lower_with_player_y_keeps_its_2a0_floor(self):
        # `cmpi.w #$2A0,d3 / bls` returns before either coordinate is looked at,
        # so the rectangle starts at the first cell a standing player can be in
        # with curr_y_pos above $2A0: ($2A0 + $10) / 16, plus the standing shift.
        rect = warp_rect(0x8, 20, 60, 128, 128)
        self.assertEqual(rect.to_json(), {"x": 0, "y": 44, "width": 21, "height": 18})
        self.assertIsNone(warp_rect(0x8, 20, 10, 128, 128))

    def test_rects_are_clipped_to_the_map(self):
        self.assertEqual(
            warp_rect(0x1, 62, 62, 64, 64).to_json(),
            {"x": 62, "y": 63, "width": 2, "height": 1},
        )
        self.assertIsNone(warp_rect(0x1, 70, 70, 64, 64))

    def test_an_index_past_the_jump_table_is_rejected(self):
        with self.assertRaises(PackError):
            warp_rect(0xF, 0, 0, 64, 64)
        with self.assertRaises(PackError):
            warp_rect(0x9, 0, 0, 0, 64)
