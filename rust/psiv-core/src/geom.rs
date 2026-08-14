//! Grid geometry: the 16-pixel collision cell, rectangles over it, and the
//! four cardinal directions the field walker understands.
//!
//! Everything here is integer-only by construction. There is no diagonal
//! direction because the original has none: `GetChunkAndCollision` resolves a
//! single 16-pixel cell per step and the walker commits to one axis at a time
//! (see `docs/RUNTIME_DESIGN.md`, "Fidelity spine").

/// A cardinal direction, used both for facing and for movement.
///
/// Screen convention: `y` grows downward, matching the row-major layout of
/// [`CollisionGrid`](crate::CollisionGrid) rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// Toward lower `y`.
    Up,
    /// Toward higher `y`.
    Down,
    /// Toward lower `x`.
    Left,
    /// Toward higher `x`.
    Right,
}

impl Direction {
    /// All four directions, in a fixed order (useful for exhaustive tests).
    pub const ALL: [Direction; 4] = [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ];

    /// The per-step cell delta, as `(dx, dy)`.
    #[must_use]
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    /// The opposite direction.
    #[must_use]
    pub const fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

/// A position on the 16-pixel collision grid.
///
/// Cell coordinates are unsigned because the cartridge stores them as unsigned
/// tile bytes/words. Stepping off the top or left edge is represented by
/// [`Cell::neighbor`] returning `None`, never by wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Cell {
    /// Column, in 16-pixel cells.
    pub x: u16,
    /// Row, in 16-pixel cells.
    pub y: u16,
}

impl Cell {
    /// Constructs a cell at `(x, y)`.
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Cell {
        Cell { x, y }
    }

    /// The adjacent cell in `dir`, or `None` when that would leave the
    /// non-negative quadrant. Never wraps.
    #[must_use]
    pub fn neighbor(self, dir: Direction) -> Option<Cell> {
        let (dx, dy) = dir.delta();
        // `as i16` is exact for the -1..=1 deltas produced by `Direction`.
        let x = self.x.checked_add_signed(dx as i16)?;
        let y = self.y.checked_add_signed(dy as i16)?;
        Some(Cell { x, y })
    }
}

/// A half-open rectangle of cells: `[x, x + width) x [y, y + height)`.
///
/// Warp source areas arrive as one of these. The cartridge stores a point plus
/// an `XYRangeJmpTbl` selector; turning that selector into a rectangle is the
/// data layer's job, so the core only ever sees the resolved rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellRect {
    /// Left edge, inclusive.
    pub x: u16,
    /// Top edge, inclusive.
    pub y: u16,
    /// Width in cells; always at least 1 in a validated map.
    pub width: u16,
    /// Height in cells; always at least 1 in a validated map.
    pub height: u16,
}

impl CellRect {
    /// Constructs a rectangle from its origin and size.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> CellRect {
        CellRect {
            x,
            y,
            width,
            height,
        }
    }

    /// A one-cell rectangle covering `cell` (the `XYExact` range).
    #[must_use]
    pub const fn single(cell: Cell) -> CellRect {
        CellRect::new(cell.x, cell.y, 1, 1)
    }

    /// Whether `cell` lies inside the rectangle.
    #[must_use]
    pub fn contains(&self, cell: Cell) -> bool {
        let x = u32::from(cell.x);
        let y = u32::from(cell.y);
        x >= u32::from(self.x)
            && x < u32::from(self.x) + u32::from(self.width)
            && y >= u32::from(self.y)
            && y < u32::from(self.y) + u32::from(self.height)
    }

    /// The exclusive right edge, widened so it cannot overflow.
    #[must_use]
    pub const fn right(&self) -> u32 {
        self.x as u32 + self.width as u32
    }

    /// The exclusive bottom edge, widened so it cannot overflow.
    #[must_use]
    pub const fn bottom(&self) -> u32 {
        self.y as u32 + self.height as u32
    }

    /// Whether the rectangle is degenerate (zero cells).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbor_does_not_wrap_at_the_origin() {
        let origin = Cell::new(0, 0);
        assert_eq!(origin.neighbor(Direction::Up), None);
        assert_eq!(origin.neighbor(Direction::Left), None);
        assert_eq!(origin.neighbor(Direction::Down), Some(Cell::new(0, 1)));
        assert_eq!(origin.neighbor(Direction::Right), Some(Cell::new(1, 0)));
    }

    #[test]
    fn neighbor_does_not_wrap_at_the_far_edge() {
        let far = Cell::new(u16::MAX, u16::MAX);
        assert_eq!(far.neighbor(Direction::Down), None);
        assert_eq!(far.neighbor(Direction::Right), None);
    }

    #[test]
    fn opposite_is_an_involution() {
        for dir in Direction::ALL {
            assert_eq!(dir.opposite().opposite(), dir);
        }
    }

    #[test]
    fn rect_contains_is_half_open() {
        let rect = CellRect::new(2, 3, 2, 1);
        assert!(rect.contains(Cell::new(2, 3)));
        assert!(rect.contains(Cell::new(3, 3)));
        assert!(!rect.contains(Cell::new(4, 3)));
        assert!(!rect.contains(Cell::new(2, 4)));
    }

    #[test]
    fn single_cell_rect_contains_only_that_cell() {
        let rect = CellRect::single(Cell::new(5, 5));
        assert!(rect.contains(Cell::new(5, 5)));
        for dir in Direction::ALL {
            let neighbor = Cell::new(5, 5).neighbor(dir).unwrap();
            assert!(!rect.contains(neighbor));
        }
    }
}
