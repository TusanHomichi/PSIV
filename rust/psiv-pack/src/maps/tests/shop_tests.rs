//! Emitted-shop contracts from [retired test_shop_flow](https://github.com/TusanHomichi/PSIV/blob/902868b517fd6b8eb0aee15b5dd500623cc7954c/tests/test_shop_flow.py) and
//! the retired test_shop_portraits suite (#560), sharing the existing map emission.
use super::emission_tests::emission;
use super::retail_tests::{array, num, retail};
use crate::{json::Json, png};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[test]
fn shop_file_counts_manifest_prices_rates_and_census_match_the_cartridge() {
    let Some(e) = emission() else { return };
    let data = retail().unwrap().rom.bytes();
    let s = &e.shops;
    assert_eq!(s["kind"], "shops");
    assert_eq!(s["format_version"], crate::PACK_FORMAT_VERSION);
    for (count, records, expected) in [
        ("counter_count", "counters", 68),
        ("inventory_count", "inventories", 49),
        ("inn_count", "inns", 18),
    ] {
        assert_eq!(s[count], expected);
        assert_eq!(array(&s[records]).len(), expected);
        assert_eq!(e.fragment["shops"][records], expected);
    }
    let fragment = &e.fragment["shops"];
    assert_eq!(fragment["file"], "shops.json");
    assert_eq!(fragment["sha256"], png::hex(&Sha256::digest(&e.shop_bytes)));
    assert_eq!(fragment["prices"]["sell_divisor"], 2);
    assert_eq!(fragment["prices"]["unlimited_stock"], true);
    assert_eq!(fragment["census"], s["census"]);
    assert!(e.shop_bytes.ends_with(b"\n"));
    assert!(
        crate::json::pack_bytes(&Json::from(s)) == e.shop_bytes,
        "shop JSON must be sorted, indented and newline terminated"
    );
    for inventory in array(&s["inventories"]) {
        let index = num(&inventory["index"]);
        let bound: Vec<_> = array(&s["counters"])
            .iter()
            .filter(|c| c["live"] == true && c["shop_inventory_index"] == index)
            .map(|c| c["id"].clone())
            .collect();
        assert_eq!(inventory["bound_by"], json!(bound));
        assert_eq!(inventory["reachable"], !bound.is_empty());
        for item in array(&inventory["items"]) {
            let at = 0x2a8e28 + (num(&item["item_id"]) - 1) * 22 + 0x14;
            let buy = usize::from(u16::from_be_bytes(data[at..at + 2].try_into().unwrap()));
            assert!(buy > 0);
            assert_eq!(item["buy_price"], buy);
            assert_eq!(item["sell_price"], buy / 2);
            assert!(!item["display_name"].as_str().unwrap().is_empty());
        }
    }
    for (index, inn) in array(&s["inns"]).iter().enumerate() {
        assert_eq!(inn["index"], index);
        assert_eq!(inn["rate_per_character"], data[0x68112 + 2 * index + 1]);
        assert_eq!(inn["text_variant"], data[0x68112 + 2 * index]);
        let costs = inn["cost_by_party_size"].as_object().unwrap();
        assert_eq!(costs.len(), 5);
        for size in 1..=5 {
            assert_eq!(
                costs[&size.to_string()],
                num(&inn["rate_per_character"]) * size
            );
        }
        let bound: Vec<_> = array(&s["counters"])
            .iter()
            .filter(|c| c["inn_index"] == index)
            .map(|c| c["id"].clone())
            .collect();
        assert!(!bound.is_empty());
        assert_eq!(inn["bound_by"], json!(bound));
    }
    assert_eq!(s["census"]["unreachable_inventories"], json!([9, 10, 11]));
    assert_eq!(
        array(&s["census"]["dead_counters"])
            .iter()
            .map(|c| num(&c["id"]))
            .collect::<Vec<_>>(),
        [21, 22, 23]
    );
    for id in [21, 22, 23] {
        let c = &s["counters"][id];
        assert_eq!(c["live"], false);
        assert_eq!(c["x_pixels"], 0);
        assert_eq!(c["y_pixels"], 0);
        assert_eq!(c["map_symbol"], "Tonoe");
    }
}

/// Independent selector oracle: group 2 reuses group 1, while inns index
/// group 0. Read the art pointer directly from the local retail tables.
fn selected_portrait(data: &[u8], counter: &Value) -> String {
    let selector = u16::from_str_radix(
        counter["selector"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();
    let base = if selector >> 8 == 0 { 0x68006 } else { 0x6804e };
    let at = base + usize::from(selector & 0xff) * 4;
    format!(
        "0x{:06X}",
        u32::from_be_bytes(data[at..at + 4].try_into().unwrap())
    )
}

#[test]
fn every_counter_binds_by_position_and_selects_its_group_portrait() {
    let Some(e) = emission() else { return };
    let data = retail().unwrap().rom.bytes();
    let mut spread = std::collections::BTreeMap::<String, usize>::new();
    let mut zema = Vec::new();
    let portraits: BTreeSet<_> = array(&e.shops["portraits"])
        .iter()
        .map(|p| p["art"].as_str().unwrap())
        .collect();
    for c in array(&e.shops["counters"]) {
        assert_eq!(c["portrait"], selected_portrait(data, c));
        assert!(portraits.contains(c["portrait"].as_str().unwrap()));
        assert_eq!(num(&c["x_cell"]), num(&c["x_pixels"]) >> 4);
        assert_eq!(num(&c["y_cell"]), (num(&c["y_pixels"]) + 16) >> 4);
        let m = &e.maps[&num(&c["map_id"])];
        if c["live"] == true {
            let npc = &array(&m["npcs"])[num(&c["shopkeeper"]["object_index"])];
            for field in ["x_cell", "y_cell"] {
                assert_eq!(c[field], npc[field]);
            }
            assert_eq!(c["shopkeeper"]["object_id"], npc["object_id"]);
            assert_eq!(c["shopkeeper"]["dialogue_id"], npc["dialogue_id"]);
        }
        if !c["collision"].is_null() {
            *spread.entry(num(&c["collision"]).to_string()).or_default() += 1;
        }
        if c["map_symbol"].as_str().unwrap().starts_with("Zema") {
            zema.push((
                c["map_symbol"].as_str().unwrap(),
                c["kind"].as_str().unwrap(),
            ));
            if c["kind"] == "inn" {
                assert_eq!(
                    e.shops["inns"][num(&c["inn_index"])]["rate_per_character"],
                    20
                );
            }
        }
    }
    assert!(spread["0"] > 0 && spread["12"] > 0);
    assert_eq!(json!(spread), e.shops["census"]["counter_collision"]);
    assert_eq!(e.shops["census"]["live_counters"], 65);
    assert_eq!(e.shops["census"]["counters_bound_to_an_object"], 65);
    assert_eq!(
        e.shops["census"]["live_counters_without_an_object"],
        json!([])
    );
    zema.sort();
    assert_eq!(
        zema,
        [
            ("ZemaInn", "inn"),
            ("ZemaItemShop", "shop"),
            ("ZemaWeaponShop", "shop"),
            ("ZemaWeaponShop", "shop")
        ]
    );
}

#[test]
fn shop_emission_is_repeatable_and_missing_map_bindings_are_refused() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let maps: Vec<_> = e
        .maps
        .values()
        .map(|m| {
            let m: psiv_data::MapRecord = serde_json::from_value(m.clone()).unwrap();
            crate::shops::ShopMap {
                id: m.id.0,
                npcs: m.npcs,
                collision: m.collision.grid,
            }
        })
        .collect();
    let root = crate::test_support::scratch("shops-repeat");
    let fragment = crate::shops::build(&r.rom, &root, &maps).unwrap();
    assert_eq!(Value::from(&fragment), e.fragment["shops"]);
    assert_eq!(
        std::fs::read(root.join("shops.json")).unwrap(),
        e.shop_bytes
    );
    for (name, expected) in &e.shop_images {
        assert_eq!(
            png::image_sha256(&std::fs::read(root.join(name)).unwrap()).unwrap(),
            png::image_sha256(expected).unwrap()
        );
    }
    let filtered: Vec<_> = maps
        .into_iter()
        .filter(|m| [0x19, 0x1b, 0x26, 0x27, 0x2a, 0x41].contains(&m.id))
        .collect();
    for maps in [filtered.as_slice(), &[]] {
        let error = crate::shops::build(&r.rom, &root, maps).unwrap_err();
        assert!(
            error.0.contains("shopkeeper binding does not hold"),
            "{error}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
