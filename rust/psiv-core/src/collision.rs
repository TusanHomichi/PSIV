//! The 4-bit collision grid.
//!
//! Ground truth (README.md / docs/source-notes/formats.md, "Map layouts and collision"):
//! bit 14 of each pattern-name word is a collision flag, and the four flags of
//! a 2x2-tile cell form a 4-bit collision type per 16-pixel cell. Types are
//! 0 normal, 1 map change, 2 recovery, 8 solid, 9 water, $A sand, $B ice,
//! $C shop. `TileCollNormalPtrs` routes 8/9/$A/$B to `TileColl_Solid` and $C to
//! `TileColl_Shop`, both of which are `moveq #1,d2 / rts`; every other value
//! routes to `TileColl_Empty`. So the blocking set is exactly {8,9,A,B,C} and
//! the unnamed values 3-7 / $D-$F are walkable if they ever appear.

use crate::error::MapError;
use crate::geom::Cell;

/// The highest value a 4-bit collision cell can hold.
pub const MAX_COLLISION_VALUE: u8 = 0x0F;

/// A decoded collision type.
///
/// [`CollisionType::Unnamed`] carries any 4-bit value the disassembly does not
/// name. Those exist in the encoding and walk like normal ground; the runtime
/// preserves them rather than folding them into `Normal` so a bridge or a test
/// can still see what the cartridge actually stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionType {
    /// `0` - ordinary walkable ground.
    Normal,
    /// `1` - map change. Does **not** block: the walker steps onto the cell
    /// and the transition fires on arrival.
    MapChange,
    /// `2` - recovery point. Walkable.
    Recovery,
    /// `8` - solid.
    Solid,
    /// `9` - water.
    Water,
    /// `$A` - sand.
    Sand,
    /// `$B` - ice block.
    Ice,
    /// `$C` - shop counter.
    Shop,
    /// A 4-bit value the disassembly does not name (`3`-`7`, `$D`-`$F`).
    Unnamed(u8),
}

impl CollisionType {
    /// Decodes a raw 4-bit cell value.
    ///
    /// Values above `0x0F` cannot come out of the 4-flag encoding; they are
    /// rejected at grid construction, so this saturates them into `Unnamed`
    /// rather than panicking.
    #[must_use]
    pub const fn from_raw(raw: u8) -> CollisionType {
        match raw {
            0x0 => CollisionType::Normal,
            0x1 => CollisionType::MapChange,
            0x2 => CollisionType::Recovery,
            0x8 => CollisionType::Solid,
            0x9 => CollisionType::Water,
            0xA => CollisionType::Sand,
            0xB => CollisionType::Ice,
            0xC => CollisionType::Shop,
            other => CollisionType::Unnamed(other),
        }
    }

    /// The raw 4-bit value this type came from.
    #[must_use]
    pub const fn to_raw(self) -> u8 {
        match self {
            CollisionType::Normal => 0x0,
            CollisionType::MapChange => 0x1,
            CollisionType::Recovery => 0x2,
            CollisionType::Solid => 0x8,
            CollisionType::Water => 0x9,
            CollisionType::Sand => 0xA,
            CollisionType::Ice => 0xB,
            CollisionType::Shop => 0xC,
            CollisionType::Unnamed(raw) => raw,
        }
    }

    /// Whether the type stops the walker. Exactly `{8, 9, $A, $B, $C}`.
    #[must_use]
    pub const fn is_blocking(self) -> bool {
        matches!(
            self,
            CollisionType::Solid
                | CollisionType::Water
                | CollisionType::Sand
                | CollisionType::Ice
                | CollisionType::Shop
        )
    }

    /// Whether stepping onto this cell should fire a map transition.
    #[must_use]
    pub const fn is_map_change(self) -> bool {
        matches!(self, CollisionType::MapChange)
    }
}

/// A map's collision grid: one 4-bit type per 16-pixel cell, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionGrid {
    width: u16,
    height: u16,
    cells: Vec<u8>,
}

impl CollisionGrid {
    /// Builds a grid from row-major 4-bit cell values.
    ///
    /// # Errors
    ///
    /// Returns [`MapError::EmptyGrid`] for a zero dimension,
    /// [`MapError::GridSizeMismatch`] when `cells.len() != width * height`, and
    /// [`MapError::CollisionValueOutOfRange`] for any value above `0x0F`.
    pub fn new(width: u16, height: u16, cells: Vec<u8>) -> Result<CollisionGrid, MapError> {
        if width == 0 || height == 0 {
            return Err(MapError::EmptyGrid { width, height });
        }
        let expected = usize::from(width) * usize::from(height);
        if cells.len() != expected {
            return Err(MapError::GridSizeMismatch {
                width,
                height,
                expected,
                found: cells.len(),
            });
        }
        if let Some(index) = cells.iter().position(|&v| v > MAX_COLLISION_VALUE) {
            return Err(MapError::CollisionValueOutOfRange {
                index,
                value: cells[index],
            });
        }
        Ok(CollisionGrid {
            width,
            height,
            cells,
        })
    }

    /// Builds a grid of a single repeated collision value.
    ///
    /// # Errors
    ///
    /// Same conditions as [`CollisionGrid::new`].
    pub fn filled(width: u16, height: u16, value: u8) -> Result<CollisionGrid, MapError> {
        let len = usize::from(width) * usize::from(height);
        CollisionGrid::new(width, height, vec![value; len])
    }

    /// Width in cells.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Height in cells.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Row-major cell values, for renderers and debug tooling.
    #[must_use]
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    /// Whether `cell` is inside the grid.
    #[must_use]
    pub const fn contains(&self, cell: Cell) -> bool {
        cell.x < self.width && cell.y < self.height
    }

    /// The raw 4-bit value at `cell`, or `None` when out of bounds.
    #[must_use]
    pub fn raw_at(&self, cell: Cell) -> Option<u8> {
        if !self.contains(cell) {
            return None;
        }
        let index = usize::from(cell.y) * usize::from(self.width) + usize::from(cell.x);
        self.cells.get(index).copied()
    }

    /// The decoded type at `cell`, or `None` when out of bounds.
    #[must_use]
    pub fn type_at(&self, cell: Cell) -> Option<CollisionType> {
        self.raw_at(cell).map(CollisionType::from_raw)
    }

    /// Whether `cell` stops the walker. Out of bounds counts as blocking:
    /// the field walker never leaves its map's grid.
    #[must_use]
    pub fn is_blocking(&self, cell: Cell) -> bool {
        self.type_at(cell).is_none_or(CollisionType::is_blocking)
    }

    /// Overwrites one cell. Test and tooling helper; the runtime never mutates
    /// collision data mid-play.
    ///
    /// # Errors
    ///
    /// [`MapError::CollisionValueOutOfRange`] for a value above `0x0F`, or
    /// [`MapError::CellOutOfBounds`] when `cell` is outside the grid.
    pub fn set(&mut self, cell: Cell, value: u8) -> Result<(), MapError> {
        if value > MAX_COLLISION_VALUE {
            return Err(MapError::CollisionValueOutOfRange { index: 0, value });
        }
        if !self.contains(cell) {
            return Err(MapError::CellOutOfBounds {
                cell,
                width: self.width,
                height: self.height,
            });
        }
        let index = usize::from(cell.y) * usize::from(self.width) + usize::from(cell.x);
        self.cells[index] = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blocking_set_is_exactly_the_documented_five() {
        // solid, water, sand, ice, shop -- and nothing else in the 4-bit range.
        const BLOCKING_VALUES: [u8; 5] = [0x8, 0x9, 0xA, 0xB, 0xC];
        for raw in 0..=MAX_COLLISION_VALUE {
            let blocking = CollisionType::from_raw(raw).is_blocking();
            assert_eq!(
                blocking,
                BLOCKING_VALUES.contains(&raw),
                "collision type {raw:#X}"
            );
        }
    }

    #[test]
    fn raw_round_trips_through_the_decoded_type() {
        for raw in 0..=MAX_COLLISION_VALUE {
            assert_eq!(CollisionType::from_raw(raw).to_raw(), raw);
        }
    }

    #[test]
    fn map_change_is_not_blocking() {
        assert!(!CollisionType::MapChange.is_blocking());
        assert!(CollisionType::MapChange.is_map_change());
    }

    #[test]
    fn grid_rejects_malformed_input() {
        assert!(matches!(
            CollisionGrid::new(0, 4, vec![]),
            Err(MapError::EmptyGrid { .. })
        ));
        assert!(matches!(
            CollisionGrid::new(2, 2, vec![0; 3]),
            Err(MapError::GridSizeMismatch { .. })
        ));
        assert!(matches!(
            CollisionGrid::new(2, 2, vec![0, 0, 0, 0x10]),
            Err(MapError::CollisionValueOutOfRange { index: 3, .. })
        ));
    }

    #[test]
    fn grid_indexes_row_major() {
        let grid = CollisionGrid::new(3, 2, vec![0, 1, 2, 8, 9, 0xC]).unwrap();
        assert_eq!(grid.raw_at(Cell::new(0, 0)), Some(0));
        assert_eq!(grid.raw_at(Cell::new(2, 0)), Some(2));
        assert_eq!(grid.raw_at(Cell::new(0, 1)), Some(8));
        assert_eq!(grid.raw_at(Cell::new(2, 1)), Some(0xC));
        assert_eq!(grid.raw_at(Cell::new(3, 0)), None);
        assert_eq!(grid.raw_at(Cell::new(0, 2)), None);
    }

    #[test]
    fn out_of_bounds_blocks() {
        let grid = CollisionGrid::filled(2, 2, 0).unwrap();
        assert!(!grid.is_blocking(Cell::new(1, 1)));
        assert!(grid.is_blocking(Cell::new(2, 1)));
    }
}
