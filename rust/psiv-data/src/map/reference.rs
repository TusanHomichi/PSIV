//! What a map record names outside itself: another map, and an `XYRangeJmpTbl`
//! routine.
//!
//! Both are stored as the index the cartridge holds plus the disassembly's name for
//! it, so a record reads without the tables and a message can print either.

use crate::ids::MapId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An `XYRangeJmpTbl` entry, by index and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RangeRef {
    /// Index into `XYRangeJmpTbl`, `0..15`.
    pub id: u8,
    /// The disassembly's name for it, such as `XYExact`.
    pub name: String,
}

/// A reference to another map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MapRef {
    /// The referenced map's id.
    pub id: MapId,
    /// Its `MapID_*` symbol, when it has one.
    #[serde(default)]
    pub symbol: Option<String>,
}

impl MapRef {
    /// The symbol, or the id in hex. For messages.
    pub fn label(&self) -> String {
        match &self.symbol {
            Some(symbol) => symbol.clone(),
            None => self.id.to_string(),
        }
    }
}
