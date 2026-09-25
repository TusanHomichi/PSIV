//! Map transitions: the ROM's two transition tables, one record of each.
//!
//! A record stores a source coordinate and an `XYRangeJmpTbl` index rather than an
//! area, so the packer resolves both into the half-open cell rectangle the runtime
//! tests, clipped to the map.

use super::cell::{Cell, CellPos, CellRect};
use super::facing::Facing;
use super::reference::{MapRef, RangeRef};
use serde::{Deserialize, Serialize};

/// Which transition table a warp came from.
///
/// `RunMapTransitions` walks one or the other depending on the collision type
/// the player is standing on, so the table decides when a warp is even
/// considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum TransitionTable {
    /// `Map_Transition_Data_Addr`, walked by `MapTransTile_Normal` for every
    /// standing collision type except map-change, solid, ice and shop: map
    /// edges, cave mouths, doormats.
    Normal,
    /// `Map_Transition_Data_2_Addr`, walked by `MapTransTile_MapChange`,
    /// reached only from a standing collision type of 1 whose previously
    /// occupied cell was not also type 1. These are doorways.
    MapChange,
}

impl TryFrom<u8> for TransitionTable {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(TransitionTable::Normal),
            2 => Ok(TransitionTable::MapChange),
            other => Err(format!(
                "transition table {other} does not exist; the ROM has tables 1 and 2"
            )),
        }
    }
}

impl From<TransitionTable> for u8 {
    fn from(value: TransitionTable) -> u8 {
        match value {
            TransitionTable::Normal => 1,
            TransitionTable::MapChange => 2,
        }
    }
}

/// A map transition.
///
/// The ROM stores a source coordinate and an `XYRangeJmpTbl` index, not a
/// rectangle; the packer transcribes the fifteen routines into [`Warp::rect`]
/// and clips it to the map, because five of them are open-ended half-planes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warp {
    /// Position in this map's warp list.
    pub index: u32,
    /// Which table the record came from.
    pub table: TransitionTable,
    /// The collision the player must be standing on for this table to be
    /// walked: `any_walkable_tile` or `map_change_tile`.
    pub trigger: String,
    /// Where the record sits in the ROM, for tracing back.
    pub record_offset: String,
    /// The `XYRangeJmpTbl` entry that produced [`Warp::rect`].
    pub range: RangeRef,
    /// The record's coordinate, raw and resolved.
    pub source: WarpSource,
    /// The trigger area in cells, clipped to the map.
    ///
    /// `None` when the range produces no area at all -- `Null` never fires, and
    /// a clipped half-plane can fall entirely outside the map. Such a warp is
    /// dead data, not an error; the manifest counts them.
    pub rect: Option<CellRect>,
    /// The map this transition leads to.
    pub target: MapRef,
    /// Where the player lands on the target map, in that map's cells.
    pub destination: Cell,
    /// Which way the player faces on arrival.
    pub facing: Facing,
    /// The record's `character_alignment` byte; not decoded yet.
    pub character_alignment: u8,
}

/// A transition's source coordinate, raw and resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarpSource {
    /// The raw X byte from the record.
    pub x_byte: u32,
    /// The raw Y byte from the record, one row above the cell it means.
    pub y_byte: u32,
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells, with the standing-cell shift applied.
    pub y_cell: u32,
}

impl WarpSource {
    /// The occupied cell this record is talking about.
    pub const fn pos(self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_tables_are_the_roms_two() {
        assert_eq!(
            serde_json::from_str::<TransitionTable>("1").unwrap(),
            TransitionTable::Normal
        );
        assert_eq!(
            serde_json::from_str::<TransitionTable>("2").unwrap(),
            TransitionTable::MapChange
        );
        let err = serde_json::from_str::<TransitionTable>("3")
            .unwrap_err()
            .to_string();
        assert!(err.contains("tables 1 and 2"), "{err}");
        assert_eq!(
            serde_json::to_string(&TransitionTable::MapChange).unwrap(),
            "2"
        );
    }
}
