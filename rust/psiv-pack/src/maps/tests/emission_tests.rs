//! Emitted map structure from tests/test_pack_maps.py (#552).
//! Related file and report checks share this emission in pack_structure_tests.

use super::retail_tests::{array, num, retail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub(super) struct Emission {
    pub fragment: Value,
    pub maps: BTreeMap<usize, Value>,
    pub images: BTreeMap<String, (usize, usize, u8)>,
    pub files: std::collections::BTreeSet<String>,
    pub piata_png: Vec<u8>,
    pub shops: Value,
}

pub(super) fn emission() -> Option<&'static Emission> {
    static EMISSION: OnceLock<Option<Emission>> = OnceLock::new();
    EMISSION
        .get_or_init(|| {
            let r = retail()?;
            let root = crate::test_support::scratch("map-emission");
            std::fs::create_dir_all(&root).unwrap();
            let result = super::pictures::build(&r.rom, &root);
            let fragment = Value::from(&result.unwrap());
            let maps = array(&fragment["maps"])
                .iter()
                .map(|entry| {
                    let bytes = std::fs::read(root.join(entry["json"].as_str().unwrap())).unwrap();
                    (num(&entry["id"]), serde_json::from_slice(&bytes).unwrap())
                })
                .collect();
            let mut images = BTreeMap::new();
            for entry in array(&fragment["maps"]) {
                for key in ["png", "png_over"] {
                    if let Some(path) = entry[key].as_str() {
                        let bytes = std::fs::read(root.join(path)).unwrap();
                        let png = crate::png::decode(&bytes).unwrap();
                        images.insert(path.to_owned(), (png.width, png.height, png.colour));
                        if key == "png" {
                            assert!(png.trns.is_empty());
                            let mut at = 8;
                            let mut kinds = Vec::new();
                            while at < bytes.len() {
                                let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
                                    as usize;
                                kinds.push(bytes[at + 4..at + 8].to_vec());
                                at += len + 12;
                            }
                            assert_eq!(kinds, [b"IHDR", b"PLTE", b"IDAT", b"IEND"]);
                        }
                    }
                }
            }
            let files = std::fs::read_dir(root.join("maps"))
                .unwrap()
                .map(|p| format!("maps/{}", p.unwrap().file_name().to_str().unwrap()))
                .collect();
            let piata_png = std::fs::read(root.join("maps/010_Piata.png")).unwrap();
            let shops =
                serde_json::from_slice(&std::fs::read(root.join("shops.json")).unwrap()).unwrap();
            std::fs::remove_dir_all(root).unwrap();
            Some(Emission {
                fragment,
                maps,
                images,
                files,
                piata_png,
                shops,
            })
        })
        .as_ref()
}

/// Compare independently decoded records with what is actually written, so
/// changing a valid JSON field cannot silently change map semantics.
fn assert_record_fields(record: &Value, emitted: &Value) {
    assert_eq!(emitted["music"]["id"], record["music"]["id"]);
    assert_eq!(emitted["music"]["symbol"], record["music"]["symbol"]);
    assert_eq!(emitted["dialogue_tree"], record["dialogue"]["tree"]);
    assert_eq!(emitted["events"], record["events"]["ids"]);
    assert!(array(&emitted["events"]).iter().all(|v| num(v) < 128));
    for key in ["poison", "random_battles", "town_teleport"] {
        assert_eq!(emitted["flags"][key], record["flags"][key]);
    }
    assert_eq!(
        emitted["flags"]["dungeon_teleport_index"],
        record["flags"]["dungeon_teleport_index"]
    );
}

#[test]
fn every_emitted_record_keeps_events_music_flags_and_graph_structure() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    assert_eq!(e.fragment["map_count"], 361);
    assert_eq!(e.maps.len(), 361);
    assert!(array(&e.fragment["unpacked_warp_targets"]).is_empty());
    assert_eq!(e.fragment["warps"]["rect_units"], "collision cells");
    assert_eq!(
        num(&e.fragment["warps"]["count"]),
        e.maps
            .values()
            .map(|m| array(&m["warps"]).len())
            .sum::<usize>()
    );
    let fixture_ids = [0x10, 0x1b, 0x92];
    assert!(
        array(&e.fragment["warps"]["doors_without_map_change_cell"])
            .iter()
            .all(|d| !fixture_ids.contains(&num(&d["id"])))
    );
    assert_eq!(
        fixture_ids
            .iter()
            .flat_map(|id| array(&e.maps[id]["warps"]))
            .filter(|w| w["table"] == 1)
            .count(),
        4
    );
    for (&id, m) in &e.maps {
        assert_record_fields(&r.records["maps"][id], m);
        let typed: psiv_data::MapRecord = serde_json::from_value(m.clone()).unwrap();
        assert_eq!(typed.id.0 as usize, id);
        assert_eq!(typed.format_version, psiv_data::PACK_FORMAT_VERSION);
        for warp in array(&m["warps"]) {
            assert!(e.maps.contains_key(&num(&warp["target"]["id"])));
        }
    }
    assert_eq!(e.maps[&0x10]["events"], json!([0]));
}

#[test]
fn piata_collision_doorways_exit_and_npcs_keep_coordinate_spaces() {
    let Some(e) = emission() else { return };
    let p = &e.maps[&0x10];
    let c = &p["collision"];
    assert_eq!(c["plane"], "bg");
    assert_eq!(c["plane_byte"], 1);
    assert_eq!(c["width_cells"], 64);
    assert_eq!(c["height_cells"], 64);
    assert_eq!(
        p["dimensions"],
        json!({"cell_pixels":16,"height_cells":64,"height_chunks":32,"height_pixels":1024,"width_cells":64,"width_chunks":32,"width_pixels":1024})
    );
    assert_eq!(array(&c["rows"]).len(), 64);
    for row in array(&c["rows"]) {
        assert_eq!(array(row).len(), 64);
        assert!(array(row).iter().all(|v| num(v) <= 15));
    }
    let warps = array(&p["warps"]);
    let door = warps.iter().find(|w| w["target"]["id"] == 0x11).unwrap();
    assert_eq!(door["table"], 2);
    assert_eq!(door["trigger"], "map_change_tile");
    assert_eq!(door["range"], json!({"id":9,"name":"XPlus20_YPlus10"}));
    assert_eq!(door["rect"], json!({"x":31,"y":7,"width":2,"height":1}));
    assert_eq!(
        door["source"],
        json!({"x_byte":31,"y_byte":6,"x_cell":31,"y_cell":7})
    );
    assert_eq!(door["destination"], json!({"x_cell":31,"y_cell":19}));
    assert_eq!(door["facing"], json!({"id":4,"name":"up"}));
    assert_eq!(door["target"]["symbol"], "PiataAcademy");
    let exit = warps.iter().find(|w| w["target"]["id"] == 0).unwrap();
    assert_eq!(exit["table"], 1);
    assert_eq!(exit["trigger"], "any_walkable_tile");
    assert_eq!(exit["range"], json!({"id":7,"name":"YHigher"}));
    assert_eq!(exit["rect"], json!({"x":0,"y":48,"width":64,"height":16}));
    assert_eq!(exit["destination"], json!({"x_cell":46,"y_cell":143}));
    let doors: Vec<_> = warps.iter().filter(|w| w["table"] == 2).collect();
    assert_eq!(
        doors
            .iter()
            .map(|w| num(&w["target"]["id"]))
            .collect::<Vec<_>>(),
        [0x11, 0x18, 0x19, 0x1a, 0x1b, 0x1c]
    );
    let mut covered = std::collections::BTreeSet::new();
    for w in doors {
        let rect = &w["rect"];
        let mut hits = 0;
        for y in num(&rect["y"])..num(&rect["y"]) + num(&rect["height"]) {
            for x in num(&rect["x"])..num(&rect["x"]) + num(&rect["width"]) {
                if c["rows"][y][x] == 1 {
                    covered.insert((x, y));
                    hits += 1;
                }
            }
        }
        assert!(hits > 0);
    }
    let everywhere: std::collections::BTreeSet<_> = array(&c["rows"])
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            array(row)
                .iter()
                .enumerate()
                .filter(|(_, v)| **v == 1)
                .map(move |(x, _)| (x, y))
        })
        .collect();
    assert_eq!(covered, everywhere);
    assert_eq!(covered.len(), 7);
    let npcs = array(&p["npcs"]);
    assert_eq!(npcs.len(), 10);
    assert!(npcs.iter().all(|n| n["symbol"].is_string()));
    let npc = &npcs[0];
    assert_eq!(npc["symbol"], "NPCType6");
    assert_eq!(npc["dialogue_id"], 83);
    assert_eq!(npc["facing"], json!({"id":4,"name":"up"}));
    for (key, value) in [
        ("x_pixels", 480),
        ("y_pixels", 736),
        ("x_cell", 30),
        ("y_cell", 47),
    ] {
        assert_eq!(npc[key], value);
    }
}

#[test]
fn island_cave_chest_and_flags() {
    let Some(e) = emission() else { return };
    let cave = e
        .maps
        .values()
        .find(|m| m["symbol"] == "IslandCave")
        .unwrap();
    assert_eq!(
        cave["flags"],
        json!({"poison":1,"random_battles":1,"town_teleport":0,"dungeon_teleport_index":24})
    );
    let chests = array(&cave["treasure_chests"]);
    assert_eq!(chests.len(), 1);
    let c = &chests[0];
    assert_eq!(c["contents_type"], "item");
    assert_eq!(c["item_id"], 133);
    assert_eq!(c["item_symbol"], "Escapipe");
    assert!(c["meseta"].is_null());
    for (key, value) in [
        ("x_cell", 82),
        ("y_cell", 73),
        ("x_pixels", 1312),
        ("y_pixels", 1152),
    ] {
        assert_eq!(c[key], value);
    }
}

#[test]
fn incorrect_emitted_fields_fail_the_record_contract() {
    let Some(e) = emission() else { return };
    let record = &retail().unwrap().records["maps"][0x10];
    for path in [
        vec!["music", "id"],
        vec!["dialogue_tree"],
        vec!["events"],
        vec!["flags", "poison"],
    ] {
        let mut bad = e.maps[&0x10].clone();
        let mut field = &mut bad;
        for key in &path {
            field = &mut field[*key];
        }
        *field = if path == ["events"] {
            json!([127, 0])
        } else {
            json!(255)
        };
        assert!(
            std::panic::catch_unwind(|| assert_record_fields(record, &bad)).is_err(),
            "accepted incorrect {path:?}"
        );
    }
}

#[test]
fn shop_locations_and_interaction_event_words_match_the_map_records() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let mut piata_shops = std::collections::BTreeSet::new();
    let doors = array(&e.maps[&0x10]["warps"]);
    for counter in array(&e.shops["counters"]) {
        let id = num(&counter["map_id"]);
        let m = &e.maps[&id];
        let symbol = m["symbol"].as_str().unwrap();
        if symbol.starts_with("Piata") {
            piata_shops.insert(symbol);
            assert!(doors.iter().any(|d| num(&d["target"]["id"]) == id));
        }
    }
    assert_eq!(
        piata_shops,
        std::collections::BTreeSet::from(["PiataInn", "PiataItemShop"])
    );
    // Interaction_EventIndexes is a word table, including parameters $1B/$1C.
    for parameter in [0x1b, 0x1c] {
        assert_eq!(
            crate::m68k::w(r.rom.bytes(), 0x64f22 + parameter * 2).unwrap() as usize,
            parameter
        );
    }
    let area = &e.maps[&0x17]["interaction_areas"][0];
    assert_eq!(area["parameter"], 11);
    assert_eq!(area["event_index"], 0x6b);
    for m in e.maps.values() {
        for area in array(&m["interaction_areas"]) {
            if area["interaction_type"] == 2 {
                let parameter = num(&area["parameter"]);
                assert!(parameter < 32);
                assert_eq!(
                    num(&area["event_index"]),
                    usize::from(crate::m68k::w(r.rom.bytes(), 0x64f22 + parameter * 2).unwrap())
                );
            }
        }
    }
}
