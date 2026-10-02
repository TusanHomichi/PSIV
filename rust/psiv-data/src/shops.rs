//! Shop counters, stock lists and inn rates: the pack's `shops.json`.
//!
//! The cartridge binds a shop to the shopkeeper's position (`docs/camp/SHOPS.md`,
//! "Counters"): `Shop_FindCounter` (`0x065D12`) scans the eight-byte rows of
//! `ShopCounters` for the map and the position of the object the party talked
//! to. A row is a counter, an inn or a shop; a shop names one of the
//! `ShopInventories` lists, an inn one of the eighteen rate words.
//!
//! This module is the schema only. The rules that use it — what a counter
//! opens, what a night costs — live in `psiv-runtime`.

use std::path::Path;

use serde::Deserialize;

use crate::DataError;

/// The pack's shop file, relative to the pack directory.
pub const SHOPS_FILE: &str = "shops.json";

/// A shopkeeper portrait the counters point at by ROM art address.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopPortrait {
    /// The ROM address of the portrait's art, as the counters spell it.
    pub art: String,
    /// The extracted PNG, relative to the pack directory.
    pub png: String,
}

/// One row of the cartridge's counter table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopCounter {
    /// The row's index in `ShopCounters`.
    pub id: usize,
    /// Whether the row is reachable in the game. Three Tonoe rows are dead.
    #[serde(default = "live_by_default")]
    pub live: bool,
    /// The map the counter stands on.
    pub map_id: u16,
    /// `"inn"` or `"shop"`.
    pub kind: String,
    /// The shopkeeper's cell.
    pub x_cell: u16,
    /// The shopkeeper's cell.
    pub y_cell: u16,
    /// The inn this row is, for an inn.
    #[serde(default)]
    pub inn_index: Option<usize>,
    /// The `ShopInventories` list this row sells, for a shop.
    #[serde(default)]
    pub shop_inventory_index: Option<usize>,
    /// The shopkeeper's portrait art address.
    pub portrait: String,
    /// The shop's greeting selector.
    #[serde(default)]
    pub greeting: Option<ShopGreeting>,
}

fn live_by_default() -> bool {
    true
}

impl ShopCounter {
    /// Whether this row is an inn.
    #[must_use]
    pub fn is_inn(&self) -> bool {
        self.kind == "inn"
    }
}

/// A shop's greeting word: which noun names its trade.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopGreeting {
    /// `0` weapons, `1` armor, anything else items.
    #[serde(default)]
    pub trade_fragment: u8,
}

/// One item a shop sells.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopItem {
    /// The `InventoryData` index.
    pub item_id: u8,
    /// The record's price word, spent unchanged (`ps4.asm:135153`).
    pub buy_price: u32,
    /// The cartridge's name for the item.
    pub display_name: String,
    /// The disassembly's symbol.
    pub symbol: String,
}

/// One `ShopInventories` list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopInventory {
    /// The list's index.
    pub index: usize,
    /// The items it sells, in menu order. Stock is unlimited.
    pub items: Vec<ShopItem>,
}

/// One inn's rate word.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InnRecord {
    /// The rate's index; also the inn selector (`Inn_Index`).
    pub index: usize,
    /// Meseta per occupied party slot.
    pub rate_per_character: u32,
}

/// The decoded `shops.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopData {
    #[serde(default)]
    portraits: Vec<ShopPortrait>,
    counters: Vec<ShopCounter>,
    inventories: Vec<ShopInventory>,
    inns: Vec<InnRecord>,
}

impl ShopData {
    pub(crate) fn load(pack_dir: &Path) -> Result<ShopData, DataError> {
        let path = pack_dir.join(SHOPS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))
    }

    /// The shopkeeper portraits, by art address.
    #[must_use]
    pub fn portraits(&self) -> &[ShopPortrait] {
        &self.portraits
    }

    /// The pack path of the portrait for a counter's art address.
    ///
    /// `0x29FC66` is the alternate-greeting baker the portrait table's own row
    /// may not carry; its extracted image is `dialogue/portraits/12_Baker.png`.
    #[must_use]
    pub fn portrait_png(&self, art: &str) -> Option<&str> {
        self.portraits
            .iter()
            .find(|portrait| portrait.art == art)
            .map(|portrait| portrait.png.as_str())
            .or_else(|| (art == "0x29FC66").then_some("dialogue/portraits/12_Baker.png"))
    }

    /// Every counter row, in table order.
    #[must_use]
    pub fn counters(&self) -> &[ShopCounter] {
        &self.counters
    }

    /// The live counter whose shopkeeper stands on `(x, y)` of `map_id`.
    #[must_use]
    pub fn counter_at(&self, map_id: u16, x: u16, y: u16) -> Option<&ShopCounter> {
        self.counters.iter().find(|counter| {
            counter.live && counter.map_id == map_id && counter.x_cell == x && counter.y_cell == y
        })
    }

    /// The live counter with row id `index`, or failing that the row at that
    /// position in the table. The debug selector's lookup.
    #[must_use]
    pub fn counter_index(&self, index: usize) -> Option<&ShopCounter> {
        self.counters
            .iter()
            .find(|counter| counter.live && counter.id == index)
            .or_else(|| self.counters.get(index))
            .filter(|counter| counter.live)
    }

    /// The stock list with this index.
    #[must_use]
    pub fn inventory(&self, index: usize) -> Option<&ShopInventory> {
        self.inventories
            .iter()
            .find(|inventory| inventory.index == index)
    }

    /// The inn rate with this index.
    #[must_use]
    pub fn inn(&self, index: usize) -> Option<&InnRecord> {
        self.inns.iter().find(|inn| inn.index == index)
    }

    /// Every item every stock list sells, in table order.
    pub fn inventory_items(&self) -> impl Iterator<Item = &ShopItem> {
        self.inventories
            .iter()
            .flat_map(|inventory| inventory.items.iter())
    }

    /// The first listed buy price of an item, in table order.
    #[must_use]
    pub fn listed_price(&self, item: u8) -> Option<u32> {
        self.inventory_items()
            .find(|candidate| candidate.item_id == item)
            .map(|candidate| candidate.buy_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ShopData {
        serde_json::from_value(serde_json::json!({
            "portraits": [{"art": "0xAA", "png": "shops/portraits/aa.png"}],
            "counters": [
                {"id": 0, "map_id": 25, "kind": "inn", "x_cell": 41, "y_cell": 30,
                 "inn_index": 0, "portrait": "0xAA"},
                {"id": 1, "live": false, "map_id": 25, "kind": "shop", "x_cell": 1, "y_cell": 2,
                 "shop_inventory_index": 0, "portrait": "0xAA"},
                {"id": 2, "map_id": 26, "kind": "shop", "x_cell": 3, "y_cell": 4,
                 "shop_inventory_index": 0, "portrait": "0xAA",
                 "greeting": {"trade_fragment": 1}}
            ],
            "inventories": [{"index": 0, "items": [
                {"item_id": 125, "buy_price": 20, "display_name": "MONOMATE", "symbol": "Monomate"}
            ]}],
            "inns": [{"index": 0, "rate_per_character": 5}]
        }))
        .unwrap()
    }

    #[test]
    fn a_counter_is_found_by_map_and_cell_and_dead_rows_never_match() {
        let data = sample();
        assert_eq!(data.counter_at(25, 41, 30).map(|c| c.id), Some(0));
        assert!(data.counter_at(25, 41, 31).is_none(), "wrong cell");
        assert!(data.counter_at(99, 41, 30).is_none(), "wrong map");
        assert!(data.counter_at(25, 1, 2).is_none(), "a dead row");
        assert!(data.counter_index(1).is_none(), "a dead row by index");
        assert_eq!(data.counter_index(2).map(|c| c.map_id), Some(26));
    }

    #[test]
    fn stock_prices_and_rates_resolve_through_their_indexes() {
        let data = sample();
        assert_eq!(data.inventory(0).unwrap().items[0].buy_price, 20);
        assert_eq!(data.listed_price(125), Some(20));
        assert_eq!(data.listed_price(1), None);
        assert_eq!(data.inn(0).unwrap().rate_per_character, 5);
        assert_eq!(data.portrait_png("0xAA"), Some("shops/portraits/aa.png"));
        assert_eq!(
            data.portrait_png("0x29FC66"),
            Some("dialogue/portraits/12_Baker.png")
        );
        assert!(data.counters()[0].is_inn());
    }
}
