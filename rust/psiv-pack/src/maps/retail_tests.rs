//! Remaining record and encounter contracts from tests/test_maps.py (#552).
//! The verified local ROM is the input; no extracted fixture is committed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::{Value, json};

use super::*;
use crate::test_support::retail_rom;

pub(super) struct Retail {
    pub rom: crate::Rom,
    pub records: Value,
}

pub(super) fn retail() -> Option<&'static Retail> {
    static RETAIL: OnceLock<Option<Retail>> = OnceLock::new();
    RETAIL
        .get_or_init(|| {
            let rom = retail_rom("map record tests")?;
            let records = Value::from(
                &extract_maps(rom.bytes(), &Decodes::new(rom.bytes()))
                    .unwrap()
                    .to_json(rom.bytes()),
            );
            Some(Retail { rom, records })
        })
        .as_ref()
}

pub(super) fn array(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
pub(super) fn num(value: &Value) -> usize {
    value.as_u64().unwrap() as usize
}
fn hex(value: &Value) -> usize {
    usize::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

#[test]
fn pointer_geometry_sections_raw_bytes_and_nonoverlapping_records() {
    let Some(r) = retail() else { return };
    let doc = &r.records;
    let table = &doc["pointer_table"];
    assert_eq!(table["rom_offset"], "0x100000");
    assert_eq!(table["rom_end_exclusive"], "0x100684");
    assert_eq!(table["count"], 417);
    assert_eq!(table["entry_size"], 4);
    assert_eq!(table["error_trap"], "0x000200");
    assert_eq!(doc["real_map_count"], 361);
    assert_eq!(doc["null_map_count"], 56);
    let maps = array(&doc["maps"]);
    assert_eq!(maps.len(), MAP_COUNT);
    assert_eq!(
        hex(&maps[0]["rom_offset"]),
        FIELD_MAP_PTRS + MAP_COUNT * POINTER_SIZE
    );
    let mut spans = Vec::new();
    for (id, m) in maps.iter().enumerate() {
        assert_eq!(num(&m["id"]), id);
        let null = MAP_SYMBOLS[id].starts_with("Null");
        assert_eq!(m["is_null"], null);
        assert_eq!(
            be32(r.rom.bytes(), FIELD_MAP_PTRS + id * 4) as usize == ERROR_TRAP,
            null
        );
        if null {
            continue;
        }
        let start = hex(&m["rom_offset"]);
        let mut at = start + 2;
        for key in [
            "tilesets",
            "sprites",
            "sprite_data",
            "dimensions",
            "scroll",
            "chunks",
            "map_updates",
            "transitions",
            "transitions_2",
            "objects",
            "treasure_chests",
            "tile_animations",
            "layout",
            "dialogue",
            "interaction_areas",
            "events",
            "palette",
            "flags",
            "map_data_manager",
        ] {
            let section = &m[key];
            assert_eq!(hex(&section["rom_offset"]), at, "map {id} {key}");
            let end = hex(&section["rom_end_exclusive"]);
            assert_eq!(num(&section["size_bytes"]), end - at);
            at = end;
        }
        assert_eq!(hex(&m["rom_end_exclusive"]), at);
        assert_eq!(num(&m["size_bytes"]), at - start);
        assert_eq!(m["raw_hex"], crate::png::hex(&r.rom.bytes()[start..at]));
        spans.push((start, at));
        assert_eq!(m["layout"]["present"], id > 1);
        assert_eq!(m["layout"]["size_bytes"], if id > 1 { 8 } else { 0 });
        let mut pointers = vec![
            hex(&m["palette"]["pointer"]),
            hex(&m["dialogue"]["pointer"]),
        ];
        pointers.extend(array(&m["chunks"]["pointers"]).iter().map(hex));
        for key in ["tilesets", "sprites", "sprite_data"] {
            pointers.extend(
                array(&m[key]["entries"])
                    .iter()
                    .map(|e| hex(&e["source_offset"])),
            );
        }
        if id > 1 {
            pointers.extend([
                hex(&m["layout"]["fg_offset"]),
                hex(&m["layout"]["bg_offset"]),
            ]);
        }
        assert!(pointers.iter().all(|p| *p > 0 && *p < r.rom.bytes().len()));
    }
    spans.sort_unstable();
    assert!(spans.windows(2).all(|w| w[0].1 <= w[1].0));
}

#[test]
fn records_close_the_graph_and_bind_trees_items_interactions_and_totals() {
    let Some(r) = retail() else { return };
    let maps = array(&r.records["maps"]);
    let mut targets = BTreeSet::new();
    let mut routines = BTreeSet::new();
    let mut trees = BTreeSet::new();
    let mut transitions = 0;
    for m in maps.iter().filter(|m| m["is_null"] == false) {
        let tree = num(&m["dialogue"]["tree"]);
        assert!((1..=43).contains(&tree));
        assert_eq!(m["dialogue"]["label"], format!("DialogueTree{tree}"));
        trees.insert(tree);
        for key in ["transitions", "transitions_2"] {
            for t in array(&m[key]["entries"]) {
                let id = num(&t["target"]["id"]);
                assert!(id < maps.len() && maps[id]["is_null"] == false);
                assert_eq!(
                    t["range"]["name"],
                    records::XY_RANGES[num(&t["range"]["id"])]
                );
                targets.insert(id);
                transitions += 1;
            }
        }
        for c in array(&m["treasure_chests"]["entries"]) {
            if c["contents_type"] == "item" {
                assert!((1..=crate::map_symbols::ITEM_SYMBOLS.len()).contains(&num(&c["item_id"])));
            } else {
                assert!(num(&c["meseta"]) > 0);
            }
        }
        routines.extend(
            array(&m["interaction_areas"]["entries"])
                .iter()
                .map(|a| num(&a["interaction_type"])),
        );
    }
    assert!(targets.contains(&0) && targets.contains(&1));
    assert_eq!(transitions, 930);
    assert_eq!(routines.into_iter().collect::<Vec<_>>(), [0, 1, 2, 5]);
    assert_eq!(
        (1..=43).filter(|t| !trees.contains(t)).collect::<Vec<_>>(),
        [6, 27, 28, 29, 30, 31, 32]
    );
    assert_eq!(r.records["dialogue_trees_bound"], json!(trees));
    assert_eq!(
        r.records["totals"],
        json!({"tilesets":1107,"sprite_art":509,"chunk_pointers":438,"transitions":930,"objects":949,"treasure_chests":155,"tile_animations":19,"interaction_areas":649,"record_bytes":59512})
    );
    let area = &maps[0x17]["interaction_areas"]["entries"][0];
    assert_eq!(
        (
            num(&area["x_tile"]),
            num(&area["y_tile"]),
            num(&area["range"]["id"]),
            num(&area["interaction_type"]),
            num(&area["parameter"])
        ),
        (30, 18, 9, 2, 11)
    );
}

#[test]
fn piata_record_and_its_art_boundary() {
    let Some(r) = retail() else { return };
    let p = &r.records["maps"][0x10];
    assert_eq!(p["symbol"], "Piata");
    assert_eq!(p["rom_offset"], "0x11C690");
    assert_eq!(p["rom_end_exclusive"], "0x11C808");
    assert_eq!(p["general_var"], 3);
    assert_eq!(
        p["music"],
        json!({"id":132,"id_hex":"0x84","symbol":"MotabiaTown","changes_music":true})
    );
    for (key, count) in [("tilesets", 3), ("sprites", 7), ("sprite_data", 0)] {
        assert_eq!(p[key]["entry_count"], count);
    }
    for key in [
        "fg_row_size",
        "fg_column_size",
        "bg_row_size",
        "bg_column_size",
    ] {
        assert_eq!(p["dimensions"][key], 31);
    }
    assert_eq!(p["map_updates"]["ids"], json!([2]));
    assert_eq!(p["map_data_manager"]["ids"], json!([]));
    assert_eq!(p["chunks"]["pointers"], json!(["0x122A90", "0x123380"]));
    assert_eq!(p["layout"]["fg_offset"], "0x123710");
    assert_eq!(p["layout"]["bg_offset"], "0x1237B0");
    assert_eq!(p["palette"]["pointer"], "0x123990");
    assert_eq!(
        p["tilesets"]["entries"][0]["source_offset"],
        p["rom_end_exclusive"]
    );
    assert_eq!(
        p["transitions"]["entries"][0]["target"]["symbol"],
        "Motavia"
    );
    let doors: Vec<_> = array(&p["transitions_2"]["entries"])
        .iter()
        .map(|t| t["target"]["symbol"].as_str().unwrap())
        .collect();
    assert_eq!(
        doors,
        [
            "PiataAcademy",
            "PiataDorm",
            "PiataInn",
            "PiataHouse1",
            "PiataItemShop",
            "PiataHouse2"
        ]
    );
}

#[test]
fn encounters_preserve_geometry_modes_grids_and_flag_disagreements() {
    let Some(r) = retail() else { return };
    let e = &r.records["encounters"];
    for (key, value) in [
        ("count", 416),
        ("map_count", 417),
        ("maps_outside_table", 1),
    ] {
        assert_eq!(e["table"][key], value);
    }
    assert_eq!(e["table"]["rom_offset"], "0x008050");
    assert_eq!(e["table"]["rom_end_exclusive"], "0x0081F0");
    assert_eq!(e["formation_group_count"], 68);
    let mut modes = BTreeMap::new();
    for m in array(&e["maps"]) {
        *modes.entry(m["mode"].as_str().unwrap()).or_insert(0) += 1;
        if m["mode"] == "group" {
            assert!(num(&m["group"]) < 68);
        }
    }
    assert_eq!(
        modes,
        BTreeMap::from([
            ("position_grid", 2),
            ("group", 147),
            ("none", 267),
            ("outside_table", 1)
        ])
    );
    assert_eq!(e["maps"][416]["mode"], "outside_table");
    assert_eq!(e["maps"][416]["map_symbol"], "AirCastleSpace");
    assert_eq!(r.records["maps"][416]["flags"]["random_battles"], 0);
    for (i, name, rows, groups, fallback, start, end) in [
        (
            0,
            "motavia",
            64,
            json!([0, 1, 2, 3, 4, 5, 6, 7]),
            2460,
            "0x28501C",
            "0x2853C8",
        ),
        (1, "dezolis", 32, json!([11, 12]), 0, "0x2853CC", "0x28541F"),
    ] {
        let grid = &e["position_grids"][i];
        assert_eq!(e["maps"][i]["mode"], "position_grid");
        assert_eq!(e["maps"][i]["position_grid"], name);
        assert_eq!(e["maps"][i]["value"], i);
        assert_eq!(grid["rom_offset"], start);
        assert_eq!(grid["rom_end_exclusive"], end);
        assert_eq!(grid["decompressed_size"], rows * 64);
        assert_eq!(grid["rows"], rows);
        assert_eq!(grid["columns"], 64);
        assert_eq!(grid["groups_used"], groups);
        assert_eq!(grid["fallback_cells"], fallback);
        assert_eq!(array(&grid["cells"]).len(), rows);
        for row in array(&grid["cells"]) {
            assert_eq!(array(row).len(), 64);
            assert!(array(row).iter().all(|v| num(v) >= 128 || num(v) < 68));
        }
    }
    for v in array(&e["vehicle_groups"]) {
        assert!(num(&v["group"]) < 68);
    }
    let anomalies: Vec<_> = array(&r.records["encounter_anomalies"])
        .iter()
        .map(|a| {
            (
                num(&a["id"]),
                a["mode"].as_str().unwrap(),
                num(&a["random_battles_flag"]),
            )
        })
        .collect();
    assert_eq!(
        anomalies,
        [(0x2d, "none", 1), (0x15b, "group", 0), (0x184, "group", 0)]
    );
    assert!(extract_encounter_binding(r.rom.bytes(), &Decodes::new(r.rom.bytes()), 13).is_err());
}

#[test]
fn damaged_records_and_encounter_table_are_refused() {
    let Some(r) = retail() else { return };
    for at in [FIELD_MAP_PTRS, 0x8050, 0x11C6E8, 0x11C79A] {
        let mut bytes = r.rom.bytes().to_vec();
        bytes[at] ^= 0xff;
        assert!(
            extract_maps(&bytes, &Decodes::new(&bytes)).is_err(),
            "accepted damaged byte at {at:X}"
        );
    }
}
