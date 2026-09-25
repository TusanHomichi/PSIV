//! The collision-cell coordinate space: positions, rectangles and a map's size.
//!
//! Logical position lives on this grid and nowhere else in the record. A cell is
//! `COLLISION_CELL_PIXELS` pixels; `Cell` is a position as the pack spells it, with
//! the standing-cell shift the packer applied already in `y_cell`.

use serde::{Deserialize, Serialize};

/// One collision cell is 16x16 pixels: `GetChunkAndCollision` picks a quadrant
/// with bit 1 of the tile coordinate, so a cell covers 2x2 eight-pixel tiles.
pub const COLLISION_CELL_PIXELS: u32 = 16;

/// Map size, in cells, chunks and pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    /// Width in collision cells. This is the coordinate space of everything
    /// else in the record.
    pub width_cells: u32,
    /// Height in collision cells.
    pub height_cells: u32,
    /// Width in chunks; a chunk is 4x4 tiles, so 2x2 cells.
    pub width_chunks: u32,
    /// Height in chunks.
    pub height_chunks: u32,
    /// Width in pixels, for the renderer.
    pub width_pixels: u32,
    /// Height in pixels.
    pub height_pixels: u32,
    /// Always [`COLLISION_CELL_PIXELS`]; carried so the pack is readable on its
    /// own and so a drift in the packer is caught rather than assumed away.
    pub cell_pixels: u32,
}

/// A position on the collision-cell grid.
///
/// Not deserialised directly -- the pack spells its coordinates `x_cell` and
/// `y_cell` at each site, so that no number in the JSON is ambiguous about its
/// unit. This is the type they are read into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CellPos {
    /// Column, in cells, from the left edge.
    pub x: u32,
    /// Row, in cells, from the top edge.
    pub y: u32,
}

impl CellPos {
    /// A position from a column and a row.
    pub const fn new(x: u32, y: u32) -> CellPos {
        CellPos { x, y }
    }
}

/// A cell position as the pack spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells, already carrying the standing-cell shift.
    pub y_cell: u32,
}

impl Cell {
    /// As a [`CellPos`].
    pub const fn pos(self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

/// A half-open rectangle of collision cells: `x..x + width`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRect {
    /// Leftmost column, in cells.
    pub x: u32,
    /// Topmost row, in cells.
    pub y: u32,
    /// Width in cells. The packer never emits zero; it emits no rectangle.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
}

impl CellRect {
    /// Does this rectangle cover a cell?
    pub fn contains(&self, pos: CellPos) -> bool {
        pos.x >= self.x
            && pos.y >= self.y
            && pos.x < self.x.saturating_add(self.width)
            && pos.y < self.y.saturating_add(self.height)
    }

    /// One past the last cell on each axis.
    pub fn end(&self) -> CellPos {
        CellPos::new(
            self.x.saturating_add(self.width),
            self.y.saturating_add(self.height),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_are_half_open() {
        let rect = CellRect {
            x: 2,
            y: 3,
            width: 2,
            height: 1,
        };
        assert!(rect.contains(CellPos::new(2, 3)));
        assert!(rect.contains(CellPos::new(3, 3)));
        assert!(!rect.contains(CellPos::new(4, 3)));
        assert!(!rect.contains(CellPos::new(2, 4)));
        assert!(!rect.contains(CellPos::new(1, 3)));
        assert_eq!(rect.end(), CellPos::new(4, 4));
    }

    #[test]
    fn an_empty_rect_contains_nothing() {
        let rect = CellRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        assert!(!rect.contains(CellPos::new(0, 0)));
    }
}
