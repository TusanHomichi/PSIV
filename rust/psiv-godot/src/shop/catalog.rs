//! The shop tables decoded from the runtime pack's `shops.json`.
//!
//! Counters, inventories, inn rates and item display names are pack data; this
//! module decodes them and answers lookup questions about them. It holds no
//! cursor, no window and no transaction.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::portraits;

#[derive(Clone, Debug, Deserialize)]
struct ShopsFile {
    #[serde(default)]
    portraits: Vec<portraits::Portrait>,
    counters: Vec<ShopCounter>,
    inventories: Vec<ShopInventory>,
    inns: Vec<InnRecord>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ShopCounter {
    id: usize,
    #[serde(default = "default_live")]
    live: bool,
    map_id: u16,
    kind: String,
    x_cell: u16,
    y_cell: u16,
    #[serde(default)]
    pub(super) inn_index: Option<usize>,
    #[serde(default)]
    pub(super) shop_inventory_index: Option<usize>,
    pub(super) portrait: String,
    #[serde(default)]
    pub(super) greeting: Option<GreetingSelector>,
}

impl ShopCounter {
    pub(super) fn is_inn(&self) -> bool {
        self.kind == "inn"
    }
}

fn default_live() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct GreetingSelector {
    #[serde(default)]
    pub(super) trade_fragment: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ShopInventory {
    index: usize,
    pub(super) items: Vec<ShopItem>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ShopItem {
    pub(super) item_id: u8,
    pub(super) buy_price: u32,
    pub(super) display_name: String,
    pub(super) symbol: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct InnRecord {
    index: usize,
    pub(super) rate_per_character: u32,
}

#[derive(Clone, Debug)]
pub(super) struct ShopCatalog {
    pub(super) portraits: HashMap<String, String>,
    counters: Vec<ShopCounter>,
    pub(super) inventories: Vec<ShopInventory>,
    inns: Vec<InnRecord>,
    names: HashMap<u8, String>,
}

impl ShopCatalog {
    pub(super) fn load(pack_dir: &str) -> Result<ShopCatalog, String> {
        let path = Path::new(pack_dir).join("shops.json");
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let file: ShopsFile = serde_json::from_str(&text)
            .map_err(|error| format!("could not decode {}: {error}", path.display()))?;
        let mut names = HashMap::new();
        for inventory in &file.inventories {
            for item in &inventory.items {
                names
                    .entry(item.item_id)
                    .or_insert_with(|| item.display_name.clone());
            }
        }
        Ok(ShopCatalog {
            portraits: file.portraits.into_iter().map(|p| (p.art, p.png)).collect(),
            counters: file.counters,
            inventories: file.inventories,
            inns: file.inns,
            names,
        })
    }

    pub(super) fn counter_at(&self, map_id: u16, x: u16, y: u16) -> Option<ShopCounter> {
        self.counters
            .iter()
            .find(|counter| {
                counter.live
                    && counter.map_id == map_id
                    && counter.x_cell == x
                    && counter.y_cell == y
            })
            .cloned()
    }

    pub(super) fn counter_index(&self, index: usize) -> Option<ShopCounter> {
        self.counters
            .iter()
            .find(|counter| counter.live && counter.id == index)
            .or_else(|| self.counters.get(index))
            .filter(|counter| counter.live)
            .cloned()
    }

    pub(super) fn inventory(&self, index: usize) -> Option<&ShopInventory> {
        self.inventories
            .iter()
            .find(|inventory| inventory.index == index)
    }

    pub(super) fn inn(&self, index: usize) -> Option<&InnRecord> {
        self.inns.iter().find(|inn| inn.index == index)
    }

    pub(super) fn item_name(&self, item: u8) -> String {
        self.names
            .get(&item)
            .cloned()
            .unwrap_or_else(|| format!("ITEM {item}"))
    }
}
