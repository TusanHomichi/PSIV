//! `Interaction_ChkMapAreas` entries: the areas a map checks as the player moves.
//!
//! The ROM stores an 8-pixel source coordinate and an `XYRangeJmpTbl` index; the pack
//! resolves both into the same 16-pixel rectangle the runtime uses, and keeps the
//! handler's flag bank, flag id and parameter beside them.

use super::cell::CellRect;
use super::reference::RangeRef;
use serde::{Deserialize, Serialize};

/// One map interaction area.
///
/// The ROM stores an 8-pixel source coordinate and an `XYRangeJmpTbl` index.
/// The pack resolves both into the same 16-pixel collision-cell rectangle the
/// runtime uses for landing checks, while retaining the raw bytes for audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionArea {
    /// Position in this map's interaction-area list.
    pub index: u32,
    /// ROM record offset.
    pub record_offset: String,
    /// The `XYRangeJmpTbl` entry that produced [`InteractionArea::rect`].
    pub range: RangeRef,
    /// The raw coordinate and its standing-cell resolution.
    pub source: InteractionSource,
    /// The area in collision cells, clipped to the map.
    pub rect: Option<CellRect>,
    /// Which flag bank the interaction handler uses for its record flag.
    pub flag_type: InteractionFlagType,
    /// Handler-specific flag id.
    pub flag: u8,
    /// Index into the retail interaction-handler table.
    pub interaction_type: u8,
    /// Handler-specific byte 9.
    pub parameter: u8,
    /// `Interaction_GetEvent`'s resolved Event_Index, when this is type 2.
    #[serde(default)]
    pub event_index: Option<u16>,
}

/// An interaction area's raw 8-pixel source coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionSource {
    /// Raw X word from the record, in 8-pixel units.
    pub x_byte: u32,
    /// Raw Y word from the record, in 8-pixel units and one row above the
    /// occupied collision cell after resolution.
    pub y_byte: u32,
    /// Collision-grid column after dividing the raw X by two.
    pub x_cell: u32,
    /// Collision-grid row after dividing raw Y by two and applying the
    /// standing-cell shift.
    pub y_cell: u32,
}

/// The flag-bank selector stored in an interaction record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionFlagType {
    /// Raw byte 6.
    pub id: u8,
    /// Extractor's name, when the byte is one of the retail selectors.
    #[serde(default)]
    pub name: Option<String>,
}
