//! Construction errors.
//!
//! Every one of these is a *data* problem — a bad pack, a bad bridge, a bad
//! test fixture. The core reports them; it never panics on them, because a
//! runtime that dies on one bad map record is useless for the archaeology this
//! project is doing.

use core::fmt;

use crate::geom::{Cell, CellRect};
use crate::map::{MapId, NpcId, SubCellOffset};
use crate::state::FlagBank;

/// Why a [`CollisionGrid`](crate::CollisionGrid), [`FieldMap`](crate::FieldMap)
/// or [`FieldState`](crate::FieldState) could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MapError {
    /// A grid dimension was zero.
    EmptyGrid {
        /// Requested width in cells.
        width: u16,
        /// Requested height in cells.
        height: u16,
    },
    /// The cell vector length did not match `width * height`.
    GridSizeMismatch {
        /// Grid width in cells.
        width: u16,
        /// Grid height in cells.
        height: u16,
        /// `width * height`.
        expected: usize,
        /// How many values were supplied.
        found: usize,
    },
    /// A cell value did not fit in the 4-bit collision encoding.
    CollisionValueOutOfRange {
        /// Row-major index of the offending cell.
        index: usize,
        /// The value found.
        value: u8,
    },
    /// A cell lay outside the grid.
    CellOutOfBounds {
        /// The offending cell.
        cell: Cell,
        /// Grid width in cells.
        width: u16,
        /// Grid height in cells.
        height: u16,
    },
    /// A warp's source rectangle covered no cells.
    EmptyWarpRect {
        /// Index of the warp in the map's warp list.
        warp_index: usize,
    },
    /// A warp's source rectangle ran past the edge of the grid.
    WarpRectOutOfBounds {
        /// Index of the warp in the map's warp list.
        warp_index: usize,
        /// The offending rectangle.
        rect: CellRect,
        /// Grid width in cells.
        width: u16,
        /// Grid height in cells.
        height: u16,
    },
    /// A warp's source rectangle was bigger than a wrapping map, so it would
    /// cover some cells more than once.
    WarpRectLargerThanMap {
        /// Index of the warp in the map's warp list.
        warp_index: usize,
        /// The offending rectangle.
        rect: CellRect,
        /// Grid width in cells.
        width: u16,
        /// Grid height in cells.
        height: u16,
    },
    /// An NPC stood outside the grid.
    NpcOutOfBounds {
        /// The offending NPC.
        npc: NpcId,
        /// Where it was placed.
        cell: Cell,
        /// Grid width in cells.
        width: u16,
        /// Grid height in cells.
        height: u16,
    },
    /// An NPC's sub-cell offset had a component of 16 or more, which would put
    /// it in a different cell than the one it claims.
    NpcOffsetOutOfRange {
        /// The offending NPC.
        npc: NpcId,
        /// The offset supplied.
        offset: SubCellOffset,
    },
    /// The party was placed outside the grid.
    PartyOutOfBounds {
        /// The map it was placed on.
        map: MapId,
        /// Where it was placed.
        cell: Cell,
        /// Grid width in cells.
        width: u16,
        /// Grid height in cells.
        height: u16,
    },
    /// A step duration of zero frames was requested.
    ZeroStepFrames,
    /// A flag id was past the end of its bank.
    FlagOutOfRange {
        /// Which bank.
        bank: FlagBank,
        /// The id asked for.
        id: u16,
        /// How many flags that bank holds.
        capacity: u16,
    },
    /// A party slot index was past the end of the slot array.
    PartySlotOutOfRange {
        /// The slot asked for.
        slot: usize,
        /// How many slots exist.
        slots: usize,
    },
    /// More party members were requested than the cartridge has slots for.
    TooManyPartyMembers {
        /// Members requested, leader included.
        requested: usize,
        /// The cartridge's cap.
        max: usize,
    },
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::EmptyGrid { width, height } => {
                write!(f, "collision grid has a zero dimension: {width}x{height}")
            }
            MapError::GridSizeMismatch {
                width,
                height,
                expected,
                found,
            } => write!(
                f,
                "collision grid {width}x{height} needs {expected} cells, got {found}"
            ),
            MapError::CollisionValueOutOfRange { index, value } => write!(
                f,
                "collision value {value:#04X} at index {index} does not fit in 4 bits"
            ),
            MapError::CellOutOfBounds {
                cell,
                width,
                height,
            } => write!(
                f,
                "cell ({}, {}) is outside a {width}x{height} grid",
                cell.x, cell.y
            ),
            MapError::EmptyWarpRect { warp_index } => {
                write!(f, "warp {warp_index} has an empty source rectangle")
            }
            MapError::WarpRectOutOfBounds {
                warp_index,
                rect,
                width,
                height,
            } => write!(
                f,
                "warp {warp_index} source rect ({}, {}) {}x{} runs past a {width}x{height} grid",
                rect.x, rect.y, rect.width, rect.height
            ),
            MapError::WarpRectLargerThanMap {
                warp_index,
                rect,
                width,
                height,
            } => write!(
                f,
                "warp {warp_index} source rect {}x{} is larger than the {width}x{height} world it wraps in",
                rect.width, rect.height
            ),
            MapError::NpcOutOfBounds {
                npc,
                cell,
                width,
                height,
            } => write!(
                f,
                "npc {} at ({}, {}) is outside a {width}x{height} grid",
                npc.0, cell.x, cell.y
            ),
            MapError::NpcOffsetOutOfRange { npc, offset } => write!(
                f,
                "npc {} has sub-cell offset ({}, {}); both must be under 16",
                npc.0, offset.x, offset.y
            ),
            MapError::PartyOutOfBounds {
                map,
                cell,
                width,
                height,
            } => write!(
                f,
                "party start ({}, {}) is outside map {}'s {width}x{height} grid",
                cell.x, cell.y, map.0
            ),
            MapError::ZeroStepFrames => write!(f, "a step must last at least one frame"),
            MapError::FlagOutOfRange { bank, id, capacity } => write!(
                f,
                "flag id {id} is outside the {bank:?} bank's {capacity} flags"
            ),
            MapError::PartySlotOutOfRange { slot, slots } => {
                write!(f, "party slot {slot} does not exist; there are {slots}")
            }
            MapError::TooManyPartyMembers { requested, max } => write!(
                f,
                "a party of {requested} exceeds the cartridge's {max} field slots"
            ),
        }
    }
}

impl core::error::Error for MapError {}
