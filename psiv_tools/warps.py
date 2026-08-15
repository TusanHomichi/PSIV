"""Warp trigger rectangles: `XYRangeJmpTbl`, transcribed.

A transition record stores an `XYRangeJmpTbl` index, not a rectangle.
`DoMapTransitionData` loads the record's two coordinate bytes into d0/d1 scaled
by 16, the party leader's `curr_x_pos`/`curr_y_pos` into d2/d3, and jumps into
the table; the routine there answers "is the player inside this transition's
area". The fifteen routines are transcribed below into rectangles.

Two details decide what those rectangles mean, and both are checkable against
the cartridge rather than assumed:

* `DoMapTransitionData` returns immediately when either step counter is
  non-zero, so the comparison only ever runs with the character standing still,
  i.e. exactly on the 16-pixel grid. Every bound the routines test is itself a
  multiple of 16, so a pixel rectangle converts to a cell rectangle with
  nothing left over.
* `GetChunkAndCollision` adds `#$10` to Y before it derives a cell
  (`addi.w #$10,d6`), so the cell a character *occupies* is one row below
  `curr_y_pos // 16`. Record coordinates are `curr_*_pos` values, so every Y a
  map record stores -- transition source and destination, object and chest
  placement -- is one row above the cell it is talking about. That shift is
  `STANDING_CELL_Y_OFFSET`, and it applies to more than warps, which is why it
  lives here with the reasoning rather than at its first use.

So the rectangles are in **collision-grid cells**, the same coordinate space as
`collision.rows` and the same space `docs/RUNTIME_DESIGN.md` puts logical
position in: a warp fires when the player's occupied cell is inside `rect`. The
pack keeps the raw record bytes in `source.x_byte` / `source.y_byte` so the
shift stays re-derivable.

Rectangles are clipped to the map. Five of the routines are open-ended
(`XLower`, `XHigher`, `YLower`, `YHigher`, `XYLowerWithPlayerY` test one bound
and let the other run to infinity), which is only a rectangle at all because
the player cannot leave the grid.

This module is the pack's lowest layer: it knows the cartridge's trigger
geometry and nothing about files, records or emission. `psiv_tools.pack`
re-exports everything here, which is where a consumer should reach for it.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterator

from .layouts import COLLISION_CELL_PIXELS


class PackError(ValueError):
    """Anything the pack cannot emit honestly.

    Defined here because `warps` is the bottom of the pack's own dependency
    chain and `render` and `pack_layouts` both raise it; `psiv_tools.pack`
    re-exports it, so there is exactly one class and `except PackError` catches
    it wherever it was raised.
    """


#: `GetChunkAndCollision`: `addi.w #$10,d6` before the shift down to a cell.
STANDING_CELL_Y_OFFSET = 1


# ---------------------------------------------------------------------------
# XYRangeJmpTbl
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Rect:
    """A half-open rectangle of collision cells."""

    x: int
    y: int
    width: int
    height: int

    def __post_init__(self) -> None:
        if self.width <= 0 or self.height <= 0:
            raise PackError(
                f"Rect at ({self.x}, {self.y}) is {self.width}x{self.height}; an "
                "empty trigger area is reported as no rectangle at all"
            )

    def cells(self) -> Iterator[tuple[int, int]]:
        for y in range(self.y, self.y + self.height):
            for x in range(self.x, self.x + self.width):
                yield x, y

    def to_json(self) -> dict[str, int]:
        return {"x": self.x, "y": self.y, "width": self.width, "height": self.height}


#: The eight `XYRangeJmpTbl` routines that build a box out of the record's
#: coordinate: `d6 = d0 + w`, `d7 = d1 + h`, then four comparisons that accept
#: `d0 <= player_x < d6` and `d1 <= player_y < d7`. Sizes are in pixels, as the
#: `addi.w` immediates spell them.
_BOX_RANGES: dict[int, tuple[str, int, int]] = {
    0x1: ("XYPlus40", 0x40, 0x40),
    0x2: ("XYPlus20", 0x20, 0x20),
    0x9: ("XPlus20_YPlus10", 0x20, 0x10),
    0xA: ("XPlus10_YPlus60", 0x10, 0x60),
    0xB: ("XPlus40_YPlus20", 0x40, 0x20),
    0xC: ("XPlus10_YPlus20", 0x10, 0x20),
    0xD: ("XPlus60_YPlus10", 0x60, 0x10),
    0xE: ("XPlus40_YPlus10", 0x40, 0x10),
}

#: The seven that do not: a point test, four half-plane tests, the one that
#: also gates on the player being past a fixed Y, and the one that never fires.
_OTHER_RANGES: dict[int, str] = {
    0x0: "Null",
    0x3: "XYExact",
    0x4: "XLower",
    0x5: "XHigher",
    0x6: "YLower",
    0x7: "YHigher",
    0x8: "XYLowerWithPlayerY",
}

XY_RANGE_NAMES: dict[int, str] = {
    **{value: name for value, (name, _, _) in _BOX_RANGES.items()},
    **_OTHER_RANGES,
}

#: `XYRange_XYLowerWithPlayerY` starts `cmpi.w #$2A0,d3 / bls` -- it does
#: nothing at all unless the player's Y is strictly greater than $2A0. On the
#: 16-pixel grid that means `curr_y_pos >= $2B0`, one cell further down again
#: once the standing-cell shift is applied.
_PLAYER_Y_FLOOR_PIXELS = 0x2A0
_PLAYER_Y_FLOOR_CELL = (
    _PLAYER_Y_FLOOR_PIXELS // COLLISION_CELL_PIXELS + 1 + STANDING_CELL_Y_OFFSET
)


def xy_range_name(value: int) -> str:
    if value not in XY_RANGE_NAMES:
        raise PackError(f"XYRange index {value} is outside the 15-entry jump table")
    return XY_RANGE_NAMES[value]


def warp_rect(
    range_id: int, x_byte: int, y_byte: int, width_cells: int, height_cells: int
) -> Rect | None:
    """The cells a transition record covers, clipped to its map.

    `x_byte`/`y_byte` are the record's two coordinate bytes as stored. The
    result is in collision-grid cells, so `y_byte` has already been shifted by
    `STANDING_CELL_Y_OFFSET`. `None` means the transition can never fire, which
    is `XYRange_Null` or a rectangle that lies entirely off the map.
    """
    if width_cells <= 0 or height_cells <= 0:
        raise PackError(f"Map is {width_cells}x{height_cells} cells")
    x = x_byte
    y = y_byte + STANDING_CELL_Y_OFFSET
    #: Half-open bounds before clipping. The open-ended routines are written
    #: with the map's own edge as the missing bound, which is only legitimate
    #: because the player can never stand outside the grid.
    if range_id in _BOX_RANGES:
        _, pixels_x, pixels_y = _BOX_RANGES[range_id]
        bounds = (x, y, x + pixels_x // COLLISION_CELL_PIXELS, y + pixels_y // COLLISION_CELL_PIXELS)
    elif range_id == 0x0:
        # `moveq #0,d7 / rts` -- never inside.
        return None
    elif range_id == 0x3:
        # Both coordinates compared with `bne`: one cell, exactly.
        bounds = (x, y, x + 1, y + 1)
    elif range_id == 0x4:
        # `cmp.w d2,d0 / bcs` fails only when the target X is below the
        # player's, so everything from the left edge up to and including x.
        bounds = (0, 0, x + 1, height_cells)
    elif range_id == 0x5:
        # `bhi` fails when the target X is above the player's: x and rightwards.
        bounds = (x, 0, width_cells, height_cells)
    elif range_id == 0x6:
        bounds = (0, 0, width_cells, y + 1)
    elif range_id == 0x7:
        bounds = (0, y, width_cells, height_cells)
    elif range_id == 0x8:
        # X and Y both "lower", plus the $2A0 floor on the player's own Y.
        bounds = (0, _PLAYER_Y_FLOOR_CELL, x + 1, y + 1)
    else:
        raise PackError(f"XYRange index {range_id} is outside the 15-entry jump table")

    x0, y0 = max(bounds[0], 0), max(bounds[1], 0)
    x1, y1 = min(bounds[2], width_cells), min(bounds[3], height_cells)
    if x1 <= x0 or y1 <= y0:
        return None
    return Rect(x0, y0, x1 - x0, y1 - y0)
