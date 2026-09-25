//! `LoadTreasureChests` entries: where a chest stands and what it holds.
//!
//! The record's type byte selects how its value byte is read, so exactly one of
//! `item_id` and `meseta` is set and `contents_type` says which.

use super::cell::CellPos;
use super::object::SpriteRef;
use serde::{Deserialize, Serialize};

/// What a chest holds. Byte 1 of the chest record selects between the two, and
/// byte 3 is read as whichever was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentsType {
    /// Byte 3 is an inventory item id.
    Item,
    /// Byte 3 is a meseta count, in hundreds.
    Meseta,
}

/// A treasure chest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Treasure {
    /// Position in this map's chest list.
    pub index: u32,
    /// Where the record sits in the ROM.
    pub record_offset: String,
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells, with the standing-cell shift applied.
    pub y_cell: u32,
    /// X in pixels.
    pub x_pixels: u32,
    /// Y in pixels.
    pub y_pixels: u32,
    /// Whether this is one of the white chests.
    pub white_chest: bool,
    /// The chest object's symbol.
    #[serde(default)]
    pub object_symbol: Option<String>,
    /// Original closed/open lid art in the shared field-object sheet index.
    #[serde(default)]
    pub sprite: Option<SpriteRef>,
    /// Which of [`Treasure::item_id`] and [`Treasure::meseta`] is meaningful.
    pub contents_type: ContentsType,
    /// Set when `contents_type` is [`ContentsType::Item`].
    #[serde(default)]
    pub item_id: Option<u16>,
    /// The item's symbol, when it holds an item.
    #[serde(default)]
    pub item_symbol: Option<String>,
    /// Set when `contents_type` is [`ContentsType::Meseta`]. Already multiplied
    /// out: the ROM stores hundreds.
    #[serde(default)]
    pub meseta: Option<u32>,
    /// The save flag that remembers this chest was opened.
    pub chest_flag: u16,
}

impl Treasure {
    /// The cell the chest occupies.
    pub const fn pos(&self) -> CellPos {
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
    fn contents_type_uses_the_packers_spelling() {
        let item: ContentsType = serde_json::from_str("\"item\"").unwrap();
        let meseta: ContentsType = serde_json::from_str("\"meseta\"").unwrap();
        assert_eq!(item, ContentsType::Item);
        assert_eq!(meseta, ContentsType::Meseta);
        assert!(serde_json::from_str::<ContentsType>("\"gold\"").is_err());
    }
}
