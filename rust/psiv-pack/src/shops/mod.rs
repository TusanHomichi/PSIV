//! `shops.json` and `shops/portraits/`: the port of `psiv_tools/shop_pack.py`,
//! which owns the reasons the file is one table and not a per-map section
//! (#216, slice 5).
//!
//! The file is `psiv_data::ShopData`: counters, inventories, inns and
//! portraits as the runtime reads them. The rules, the census, the source
//! block and each record's provenance (offsets, raw bytes, the shopkeeper
//! binding, sell prices) have no typed home yet and are laid over it.
//!
//! A counter names its shopkeeper by position, so this needs every map's
//! objects and collision grid: the `maps` section builds it once those are
//! written ([`ShopMap`]).

mod flow;
mod portraits;
mod tables;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use psiv_data::{
    CollisionGrid, InnRecord, Npc, ShopCounter, ShopData, ShopGreeting, ShopGroup, ShopInventory,
    ShopItem, ShopLines,
};

use crate::json::{Json, overlay};
use crate::text::{decode_string, extract_names};
use crate::{PackError, Rom};

use flow::{Flow, MAX_PARTY, PRICE_OFFSET, read_flow, sell_price};
use tables::{INN_GROUP, Inventory, Location, SHOP_COUNT, item_symbol};

const FORMAT_VERSION: u32 = 2;
const FILE: &str = "shops.json";
/// The farewell a shopkeeper says when Cancel leaves the BUY/SELL menu
/// (`Win_ShopBuySell`'s `loc_65014`, `ps4.asm:135009-135045`): `loc_682A4`'s
/// four pointers, picked by the text variant in `$FFFFE3EE`.
const FAREWELL_TABLE: usize = 0x0682A4;
const FAREWELL_VARIANTS: usize = 4;
/// The innkeeper's, on Cancel or NO at the bill (`Win_RestConfirm`'s
/// `loc_660B6`, `ps4.asm:136345-136366`): one line per inn text variant.
const INN_FAREWELLS: [usize; 2] = [0x2AC18E, 0x2AC24E];
/// `TABLES["items"]`: 160 records of 22 bytes.
const ITEMS: usize = 0x2A8E28;
const ITEM_BYTES: usize = 22;
const ITEM_COUNT: usize = 160;
const ITEM_TYPE_OFFSET: usize = 10;
/// `ITEM_TYPES`, by the record's type byte.
const ITEM_TYPES: [&str; 11] = [
    "none_or_unused",
    "one_handed_single_target_weapon",
    "one_handed_multi_target_weapon",
    "two_handed_single_target_weapon",
    "two_handed_multi_target_weapon",
    "shield",
    "headwear_or_ring",
    "body_equipment",
    "disposable_item",
    "plot_item",
    "field_only_item",
];

/// What a counter needs from one written map.
pub struct ShopMap {
    pub id: u16,
    pub npcs: Vec<Npc>,
    pub collision: CollisionGrid,
}

struct Price {
    buy: u16,
    sell: u32,
    kind: u8,
}

/// Every item's two prices, from the record word the shop code reads.
fn prices(rom: &[u8], divisor: u32) -> Result<Vec<Price>, PackError> {
    (0..ITEM_COUNT)
        .map(|index| {
            let offset = ITEMS + index * ITEM_BYTES;
            let record = rom.get(offset..offset + ITEM_BYTES).ok_or_else(|| {
                PackError(format!("Item record {} runs past end of ROM", index + 1))
            })?;
            let buy = u16::from_be_bytes([record[PRICE_OFFSET], record[PRICE_OFFSET + 1]]);
            Ok(Price {
                buy,
                sell: sell_price(buy, divisor),
                kind: record[ITEM_TYPE_OFFSET],
            })
        })
        .collect()
}

fn price(prices: &[Price], item: u8) -> &Price {
    &prices[usize::from(item) - 1]
}

fn item_names(rom: &[u8]) -> Result<HashMap<u64, String>, PackError> {
    let names = serde_json::Value::from(&extract_names(rom)?);
    Ok(names["item_names"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| Some((entry["id"].as_u64()?, entry["name"].as_str()?.to_owned())))
        .collect())
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn type_json(kind: u8) -> Json {
    Json::object()
        .with("id", kind)
        .with("name", ITEM_TYPES.get(usize::from(kind)).copied())
}

/// A counter's standing cell: `GetChunkAndCollision` adds `$10` to Y.
fn cell(x: u16, y: u16) -> (u16, u16) {
    (x >> 4, (y + 0x10) >> 4)
}

fn serialize<T: serde::Serialize>(value: &T, what: &str) -> Result<Json, PackError> {
    serde_json::to_value(value)
        .map(|value| Json::from(&value))
        .map_err(|error| PackError(format!("cannot serialize {what}: {error}")))
}

struct Counter<'a> {
    row: &'a Location,
    record: ShopCounter,
    research: Json,
    collision: Option<u8>,
    bound: bool,
}

fn counter<'a>(
    id: usize,
    row: &'a Location,
    objects: &HashMap<(u16, u32, u32), &Npc>,
    maps: &BTreeMap<u16, &ShopMap>,
    flow: &Flow,
    greetings: &[u16],
) -> Result<Counter<'a>, PackError> {
    let is_inn = row.is_inn();
    let index = row.index();
    let (x_cell, y_cell) = cell(row.x, row.y);
    let art = &flow.portrait_groups[usize::from(!is_inn)];
    let portrait = art.get(index).ok_or_else(|| {
        PackError(format!(
            "counter {id} selects portrait {index}, past the group's {}",
            art.len()
        ))
    })?;
    let keeper = objects.get(&(row.map_id, u32::from(row.x), u32::from(row.y)));
    let collision = maps.get(&row.map_id).and_then(|map| {
        map.collision
            .at(u32::from(x_cell), u32::from(y_cell))
            .map(|code| code.code())
    });
    let greeting = (!is_inn).then(|| greetings[index]);
    let record = ShopCounter {
        id,
        live: (row.x, row.y) != (0, 0),
        map_id: row.map_id,
        kind: if is_inn { "inn" } else { "shop" }.to_owned(),
        x_cell,
        y_cell,
        inn_index: is_inn.then_some(index),
        shop_inventory_index: row.shop_inventory_index(),
        portrait: format!("0x{portrait:06X}"),
        group: ShopGroup {
            id: row.group_id(),
            name: row.group_name().map(str::to_owned),
        },
        greeting: greeting.map(|word| ShopGreeting {
            index,
            trade_fragment: (word & 0xFF) as u8,
            text_variant: (word >> 8) as u8,
        }),
    };
    let shopkeeper = match keeper {
        None => Json::Null,
        Some(npc) => Json::object()
            .with("object_index", npc.index)
            .with("object_id", npc.object_id)
            .with("symbol", npc.symbol.clone())
            .with("facing", serialize(&npc.facing, "a facing")?)
            .with("dialogue_id", npc.dialogue_id)
            .with("x_cell", npc.x_cell)
            .with("y_cell", npc.y_cell),
    };
    let mut research = Json::object()
        .with("rom_offset", hex6(row.rom_offset))
        .with("raw_hex", crate::png::hex(&row.raw))
        .with("map_symbol", row.map_symbol())
        .with("x_pixels", row.x)
        .with("y_pixels", row.y)
        .with("selector", format!("0x{:04X}", row.selector))
        .with(
            "group",
            Json::object()
                .with("id", row.group_id())
                .with("name", row.group_name()),
        )
        .with("index", index)
        .with("collision", collision)
        .with("shopkeeper", shopkeeper);
    if let Some(word) = greeting {
        research.insert(
            "greeting",
            Json::object()
                .with("index", index)
                .with("rom_offset", hex6(flow.greetings_table + index * 2))
                .with("raw_hex", format!("{word:04x}"))
                .with("text_variant", word >> 8),
        );
    }
    Ok(Counter {
        row,
        bound: keeper.is_some(),
        record,
        research,
        collision,
    })
}

/// The whole file and the portraits beside it, under `root`.
/// Returns `emit_shops`' manifest fragment.
pub fn build(rom: &Rom, root: &Path, maps: &[ShopMap]) -> Result<Json, PackError> {
    let data = rom.bytes();
    let flow = read_flow(rom)?;
    let lists = tables::inventories(data)?;
    let rows = tables::locations(data, lists.len())?;
    let prices = prices(data, flow.sell_divisor)?;
    let names = item_names(data)?;
    let maps_by_id: BTreeMap<u16, &ShopMap> = maps.iter().map(|map| (map.id, map)).collect();
    let mut objects: HashMap<(u16, u32, u32), &Npc> = HashMap::new();
    for map in maps {
        for npc in &map.npcs {
            objects
                .entry((map.id, npc.x_pixels, npc.y_pixels))
                .or_insert(npc);
        }
    }

    let inn_indexes: BTreeSet<usize> = rows
        .iter()
        .filter(|row| row.group_id() == INN_GROUP)
        .map(Location::index)
        .collect();
    if inn_indexes.iter().copied().ne(0..inn_indexes.len()) {
        return Err(PackError(format!(
            "the location table binds inn indexes {inn_indexes:?}, which is not the rate \
             table's index space"
        )));
    }
    let rates: Vec<u16> = (0..inn_indexes.len())
        .map(|index| crate::m68k::w(data, flow.inn_table + index * 2))
        .collect::<Result<_, _>>()?;
    let greetings: Vec<u16> = (0..SHOP_COUNT)
        .map(|index| crate::m68k::w(data, flow.greetings_table + index * 2))
        .collect::<Result<_, _>>()?;

    let counters = rows
        .iter()
        .enumerate()
        .map(|(id, row)| counter(id, row, &objects, &maps_by_id, &flow, &greetings))
        .collect::<Result<Vec<_>, _>>()?;
    let mut bound: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for c in counters.iter().filter(|c| c.record.live) {
        if let Some(index) = c.record.shop_inventory_index {
            bound.entry(index).or_default().push(c.record.id);
        }
    }
    let (inventories, inventory_research) = inventories(&lists, &prices, &names, &bound)?;
    let (inns, inn_research) = inns(&rates, &counters, &flow)?;
    let portraits = portraits::write(data, root)?;

    let census = census(
        &counters,
        &lists,
        &inventory_research,
        &rates,
        &prices,
        &flow,
    );
    let unbound = census_unbound(&counters);
    if !unbound.is_empty() {
        return Err(PackError(format!(
            "these live counters name a position no map object occupies, so the shopkeeper \
             binding does not hold: {unbound:?}"
        )));
    }

    let typed = ShopData {
        portraits: portraits.iter().map(|p| p.record.clone()).collect(),
        counters: counters.iter().map(|c| c.record.clone()).collect(),
        inventories,
        inns,
        lines: lines(rom)?,
    };
    let mut out = serialize(&typed, "the shops")?;
    let research = Json::object()
        .with("format_version", FORMAT_VERSION)
        .with("kind", "shops")
        .with("counter_count", counters.len())
        .with("inventory_count", lists.len())
        .with("inn_count", rates.len())
        .with("source", source(&flow))
        .with("rules", flow.json.clone())
        .with(
            "counters",
            counters
                .iter()
                .map(|c| c.research.clone())
                .collect::<Vec<_>>(),
        )
        .with("inventories", inventory_research)
        .with("inns", inn_research)
        .with("census", census)
        .with(
            "portraits",
            portraits
                .into_iter()
                .map(|p| p.research)
                .collect::<Vec<_>>(),
        );
    overlay(&mut out, &research);
    let sha = crate::write_json(root, FILE, &out)?;
    let prices = out.at(&["rules", "prices"])?;
    Ok(Json::object()
        .with("file", FILE)
        .with("sha256", sha)
        .with("counters", out.at(&["counter_count"])?.clone())
        .with("inventories", out.at(&["inventory_count"])?.clone())
        .with("inns", out.at(&["inn_count"])?.clone())
        .with(
            "prices",
            Json::object()
                .with("offset", prices.at(&["price_offset"])?.clone())
                .with("sell_divisor", prices.at(&["sell", "divisor"])?.clone())
                .with(
                    "unlimited_stock",
                    prices.at(&["stock", "unlimited"])?.clone(),
                ),
        )
        .with("census", out.at(&["census"])?.clone()))
}

/// One `RunText` line in the dialogue charset, up to its `$FF`.
fn window_line(data: &[u8], address: usize) -> Result<String, PackError> {
    let tail = data
        .get(address..)
        .ok_or_else(|| PackError(format!("line at 0x{address:06X} is past the ROM")))?;
    let end = tail
        .iter()
        .position(|&byte| byte == 0xFF)
        .ok_or_else(|| PackError(format!("line at 0x{address:06X} has no terminator")))?;
    let decoded = decode_string(&tail[..end], "dialogue", 1)
        .map_err(|error| PackError(format!("line at 0x{address:06X}: {error}")))?;
    match field(&decoded, "text") {
        Some(Json::Str(text)) => Ok(text.clone()),
        _ => Err(PackError(format!(
            "line at 0x{address:06X} decoded no text"
        ))),
    }
}

/// The lines a shop or inn window says as the party leaves.
fn lines(rom: &Rom) -> Result<ShopLines, PackError> {
    let data = rom.bytes();
    let farewell = (0..FAREWELL_VARIANTS)
        .map(|index| {
            let address = crate::m68k::l(data, FAREWELL_TABLE + 4 * index)? as usize;
            window_line(data, address)
        })
        .collect::<Result<_, _>>()?;
    let inn_farewell = INN_FAREWELLS
        .iter()
        .map(|&address| window_line(data, address))
        .collect::<Result<_, _>>()?;
    Ok(ShopLines {
        farewell,
        inn_farewell,
    })
}

fn source(flow: &Flow) -> Json {
    Json::object()
        .with(
            "locations",
            Json::object()
                .with("label", tables::LOCATIONS_LABEL)
                .with("rom_offset", hex6(tables::LOCATIONS_START))
                .with("entry_bytes", tables::LOCATION_ENTRY_BYTES)
                .with("terminator", "0xFFFF"),
        )
        .with(
            "inventories",
            Json::object()
                .with("label", tables::INVENTORIES_LABEL)
                .with("rom_offset", hex6(tables::INVENTORIES_START))
                .with("rom_end_exclusive", hex6(tables::INVENTORIES_END))
                .with("terminator", "0xFF")
                .with("indexing", tables::INDEXING),
        )
        .with(
            "inn_rates",
            Json::object()
                .with("label", "loc_68112")
                .with("rom_offset", hex6(flow.inn_table))
                .with("entry_bytes", 2u8),
        )
        .with(
            "greetings",
            Json::object()
                .with("label", "loc_68136")
                .with("rom_offset", hex6(flow.greetings_table))
                .with("entry_bytes", 2u8),
        )
        .with(
            "portraits",
            Json::object()
                .with("label", "ShopPortraitGroupsPtrs")
                .with("rom_offset", hex6(flow.portrait_pointers)),
        )
}

fn inventories(
    lists: &[Inventory],
    prices: &[Price],
    names: &HashMap<u64, String>,
    bound: &BTreeMap<usize, Vec<usize>>,
) -> Result<(Vec<ShopInventory>, Vec<Json>), PackError> {
    let mut typed = Vec::new();
    let mut research = Vec::new();
    for list in lists {
        let mut items = Vec::new();
        let mut extra = Vec::new();
        for &item in &list.items {
            let p = price(prices, item);
            let display_name = names.get(&u64::from(item)).cloned().ok_or_else(|| {
                PackError(format!(
                    "shop {} sells item {item}, which has no name",
                    list.index
                ))
            })?;
            items.push(ShopItem {
                item_id: item,
                buy_price: u32::from(p.buy),
                display_name,
                symbol: item_symbol(item).expect("checked by the table").to_owned(),
            });
            extra.push(
                Json::object()
                    .with("type", type_json(p.kind))
                    .with("sell_price", p.sell),
            );
        }
        let by = bound.get(&list.index).cloned().unwrap_or_default();
        research.push(
            Json::object()
                .with("rom_offset", hex6(list.rom_offset))
                .with("raw_hex", crate::png::hex(&list.raw))
                .with("item_count", list.items.len())
                .with("items", extra)
                .with("reachable", !by.is_empty())
                .with("bound_by", by),
        );
        typed.push(ShopInventory {
            index: list.index,
            items,
        });
    }
    Ok((typed, research))
}

fn inns(
    rates: &[u16],
    counters: &[Counter],
    flow: &Flow,
) -> Result<(Vec<InnRecord>, Vec<Json>), PackError> {
    let mut typed = Vec::new();
    let mut research = Vec::new();
    for (index, &word) in rates.iter().enumerate() {
        let rate = u32::from(word & 0xFF);
        let portrait = flow.portrait_groups[0]
            .get(index)
            .ok_or_else(|| PackError(format!("inn {index} has no innkeeper portrait")))?;
        let at: Vec<&Counter> = counters
            .iter()
            .filter(|c| c.record.inn_index == Some(index))
            .collect();
        let costs = (1..=MAX_PARTY).fold(Json::object(), |out, size| {
            out.with(size.to_string(), rate * size)
        });
        let info = Json::object()
            .with("rom_offset", hex6(flow.inn_table + index * 2))
            .with("raw_hex", format!("{word:04x}"))
            .with("text_variant", word >> 8)
            .with("portrait", format!("0x{portrait:06X}"))
            .with("cost_by_party_size", costs)
            .with(
                "bound_by",
                at.iter().map(|c| c.record.id).collect::<Vec<_>>(),
            )
            .with(
                "maps",
                at.iter().map(|c| c.row.map_symbol()).collect::<Vec<_>>(),
            );
        typed.push(InnRecord {
            index,
            rate_per_character: rate,
            text_variant: (word >> 8) as u8,
        });
        research.push(info);
    }
    Ok((typed, research))
}

fn census_unbound(counters: &[Counter]) -> Vec<usize> {
    counters
        .iter()
        .filter(|c| c.record.live && !c.bound)
        .map(|c| c.record.id)
        .collect()
}

fn census(
    counters: &[Counter],
    lists: &[Inventory],
    inventories: &[Json],
    rates: &[u16],
    prices: &[Price],
    flow: &Flow,
) -> Json {
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for c in counters {
        *groups.entry(c.row.group_id().to_string()).or_default() += 1;
    }
    let sold: BTreeSet<u8> = lists.iter().flat_map(|l| l.items.iter().copied()).collect();
    let unreachable: Vec<usize> = lists
        .iter()
        .zip(inventories)
        .filter(|(_, json)| matches!(field(json, "reachable"), Some(Json::Bool(false))))
        .map(|(list, _)| list.index)
        .collect();
    let sizes = lists.iter().map(|l| l.items.len());
    let sold_prices = sold.iter().map(|&i| price(prices, i).buy);
    let inn_rates = rates.iter().map(|w| w & 0xFF);
    let mut collision: BTreeMap<String, usize> = BTreeMap::new();
    for code in counters.iter().filter_map(|c| c.collision) {
        *collision.entry(code.to_string()).or_default() += 1;
    }
    let range =
        |min: Option<u64>, max: Option<u64>| Json::object().with("min", min).with("max", max);
    let distinct_portraits: BTreeSet<u32> =
        flow.portrait_groups.iter().flatten().copied().collect();
    let as_object = |map: BTreeMap<String, usize>| {
        map.into_iter()
            .fold(Json::object(), |out, (key, count)| out.with(key, count))
    };
    Json::object()
        .with("counters_by_group", as_object(groups))
        .with(
            "live_counters",
            counters.iter().filter(|c| c.record.live).count(),
        )
        .with(
            "dead_counters",
            counters
                .iter()
                .filter(|c| !c.record.live)
                .map(|c| {
                    Json::object()
                        .with("id", c.record.id)
                        .with("map_symbol", c.row.map_symbol())
                        .with("selector", format!("0x{:04X}", c.row.selector))
                        .with("rom_offset", hex6(c.row.rom_offset))
                })
                .collect::<Vec<_>>(),
        )
        .with(
            "counters_bound_to_an_object",
            counters.iter().filter(|c| c.bound).count(),
        )
        .with("live_counters_without_an_object", census_unbound(counters))
        .with("unreachable_inventories", unreachable)
        .with(
            "inventory_sizes",
            range(
                sizes.clone().min().map(|n| n as u64),
                sizes.max().map(|n| n as u64),
            ),
        )
        .with("distinct_items_sold", sold.len())
        .with(
            "items_never_sold",
            (1..=ITEM_COUNT as u8)
                .filter(|item| !sold.contains(item))
                .collect::<Vec<_>>(),
        )
        .with(
            "items_sold_with_no_price",
            sold.iter()
                .copied()
                .filter(|&item| price(prices, item).buy == 0)
                .collect::<Vec<_>>(),
        )
        .with(
            "price_range",
            range(
                sold_prices.clone().min().map(u64::from),
                sold_prices.max().map(u64::from),
            ),
        )
        .with(
            "inn_rates",
            range(
                inn_rates.clone().min().map(u64::from),
                inn_rates.max().map(u64::from),
            ),
        )
        .with(
            "inns_with_alternate_text",
            rates
                .iter()
                .enumerate()
                .filter(|(_, w)| *w >> 8 != 0)
                .map(|(index, _)| index)
                .collect::<Vec<_>>(),
        )
        .with("distinct_portraits", distinct_portraits.len())
        .with(
            "maps_with_a_counter",
            counters
                .iter()
                .map(|c| c.row.map_id)
                .collect::<BTreeSet<_>>()
                .len(),
        )
        .with("counter_collision", as_object(collision))
}

fn field<'a>(json: &'a Json, key: &str) -> Option<&'a Json> {
    match json {
        Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counter_stands_on_the_cell_its_y_is_shifted_into() {
        assert_eq!(cell(656, 464), (41, 30));
        assert_eq!(cell(0, 0), (0, 1));
    }

    #[test]
    fn an_item_type_outside_the_table_has_no_name() {
        let named = serde_json::Value::from(&type_json(8));
        assert_eq!(named["name"], "disposable_item");
        assert!(serde_json::Value::from(&type_json(11))["name"].is_null());
    }
}
