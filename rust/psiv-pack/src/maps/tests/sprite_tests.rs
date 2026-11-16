//! Emitted sprite contract from the retired test_pack_sprites suite and the retired test_chest_sprites suite.
//! Reuses the full map emission: references and counts span every real map.
use super::emission_tests::emission;
use super::retail_tests::{array, num};
use crate::png;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn sheet_contract(sheet: &Value, image: &[u8]) {
    let p = png::decode(image).unwrap();
    assert_eq!(p.colour, 3);
    assert_eq!(p.plte.len(), 48);
    assert_eq!(p.trns, [0]);
    assert_eq!(
        p.plte,
        array(&sheet["palette"]["colors"])
            .iter()
            .flat_map(|c| array(c).iter().map(|v| num(v) as u8))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        (p.width, p.height),
        (
            num(&sheet["frame_width"]) * num(&sheet["frame_count"]),
            num(&sheet["frame_height"])
        )
    );
    assert_eq!(sheet["png_sha256"], png::image_sha256(image).unwrap());
    for sequence in sheet["sequences"].as_object().unwrap().values() {
        assert!(!array(&sequence["frames"]).is_empty());
        for frame in array(&sequence["frames"]) {
            assert!(num(&frame["index"]) < num(&sheet["frame_count"]));
            assert!(num(&frame["duration_ticks"]) > 0);
        }
    }
}

fn reference_contract(reference: &Value, sheets: &BTreeMap<&str, &Value>) {
    assert_eq!(reference["sheets"], "sprites/npcs.json");
    let sheet = sheets[reference["sheet"].as_str().unwrap()];
    for key in ["idle_sequence", "walk_sequence"] {
        assert!(
            sheet["sequences"]
                .get(reference[key].as_str().unwrap())
                .is_some()
        );
    }
}

fn alternates(value: &Value, seen: &mut BTreeSet<String>) {
    match value {
        Value::Object(o) => {
            if let Some(entries) = o.get("npc_sheets") {
                for entry in array(entries) {
                    for key in ["from", "to"] {
                        seen.insert(entry[key].as_str().unwrap().to_owned());
                    }
                }
            }
            for child in o.values() {
                alternates(child, seen);
            }
        }
        Value::Array(a) => {
            for child in a {
                alternates(child, seen);
            }
        }
        _ => {}
    }
}

#[test]
fn emitted_party_vehicles_pngs_sequences_and_origins() {
    let Some(e) = emission() else { return };
    assert_eq!(e.party["format_version"], psiv_data::PACK_FORMAT_VERSION);
    assert_eq!(e.vehicles["format_version"], psiv_data::PACK_FORMAT_VERSION);
    let r = crate::sprites::tests::retail().unwrap();
    assert_eq!(array(&e.party["sheets"]).len(), 11);
    for (entry, sprite) in array(&e.party["sheets"]).iter().zip(&r.party) {
        assert_eq!(entry["id"], sprite.symbol);
        assert_eq!(entry["png"], format!("sprites/party/{}.png", sprite.symbol));
        assert_eq!(
            (
                num(&entry["frame_width"]),
                num(&entry["frame_height"]),
                num(&entry["frame_count"])
            ),
            (16, 32, 12)
        );
        assert_eq!(
            (
                entry["origin_x"].as_i64().unwrap(),
                entry["origin_y"].as_i64().unwrap()
            ),
            (0, 2)
        );
        assert_eq!(entry["palette"]["cram_line"], 2);
        assert_eq!(entry["palette"]["source"], "Pal_Init_Line_3");
        assert_eq!(entry["art"]["tile_count"], 72);
        assert_eq!(entry["sequences"].as_object().unwrap().len(), 8);
        for (name, sequence) in &sprite.sheet.sequences {
            assert_eq!(
                entry["sequences"][name]["frames"],
                json!(
                    sequence
                        .frames
                        .iter()
                        .zip(&sequence.durations)
                        .map(|(i, d)| json!({"index":i,"duration_ticks":d}))
                        .collect::<Vec<_>>()
                )
            );
        }
        let image = &e.sprite_images[entry["png"].as_str().unwrap()];
        sheet_contract(entry, image);
        assert_eq!(png::decode(image).unwrap().pixels, sprite.sheet.strip().2);
    }
    assert_eq!(
        array(&e.vehicles["sheets"])
            .iter()
            .map(|s| num(&s["vehicle_index"]))
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    for s in array(&e.vehicles["sheets"]).iter().chain(
        array(&e.vehicles["palette_variants"])
            .iter()
            .flat_map(|v| array(&v["sheets"])),
    ) {
        assert_eq!(
            s["png"],
            format!("sprites/vehicles/{}.png", s["id"].as_str().unwrap())
        );
        assert_eq!((num(&s["frame_width"]), num(&s["frame_height"])), (40, 32));
        assert_eq!(s["palette"]["cram_line"], 3);
        assert_eq!(s["art"]["compression"], "nemesis");
        sheet_contract(s, &e.sprite_images[s["png"].as_str().unwrap()]);
    }
    for s in array(&e.npcs["sheets"]) {
        assert_eq!(
            s["png"],
            format!("sprites/npcs/{}.png", s["id"].as_str().unwrap())
        );
        sheet_contract(s, &e.sprite_images[s["png"].as_str().unwrap()]);
    }
}

#[test]
fn every_placed_or_alternate_sheet_resolves_and_placement_and_interaction_censuses_balance() {
    let Some(e) = emission() else { return };
    let sheets: BTreeMap<_, _> = array(&e.npcs["sheets"])
        .iter()
        .map(|s| (s["id"].as_str().unwrap(), s))
        .collect();
    assert_eq!(num(&e.npcs["sheet_count"]), sheets.len());
    let types: BTreeMap<_, _> = array(&e.npcs["field_objects"]["types"])
        .iter()
        .map(|t| (num(&t["object_id"]), t))
        .collect();
    assert_eq!(types.len(), 222);
    assert_eq!(e.npcs["field_objects"]["render_flags_bit"], 3);
    assert_eq!(e.npcs["field_objects"]["count"], 222);
    assert_eq!(
        e.npcs["field_objects"]["instruction_forms"],
        json!(["bset #3, $2(a4)", "bclr #3, $2(a4)"])
    );
    assert_eq!(
        array(&e.npcs["field_objects"]["tested_by"])
            .iter()
            .map(|t| t["routine"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Interaction_ChkObjects", "FieldObj_DoObjCollision"]
    );
    let mut seen = BTreeSet::new();
    let mut placed = BTreeMap::<&str, usize>::new();
    let (mut npc_count, mut artless, mut objects) = (0, 0, 0);
    let mut interaction = BTreeMap::<String, usize>::new();
    for map in e.maps.values() {
        for npc in array(&map["npcs"]) {
            npc_count += 1;
            objects += 1;
            let t = types[&num(&npc["object_id"])];
            assert!(npc["interactable"].is_boolean());
            assert_eq!(npc["interactable"], t["interactable"]);
            assert!(!t["source"].as_str().unwrap().is_empty());
            *interaction
                .entry(
                    if npc["interactable"].as_bool().unwrap() {
                        "1"
                    } else {
                        "0"
                    }
                    .into(),
                )
                .or_default() += 1;
            if npc["sprite"].is_null() {
                assert!(!npc["sprite_reason"].as_str().unwrap().is_empty());
                artless += 1;
            } else {
                assert!(npc["sprite_reason"].is_null());
                reference_contract(&npc["sprite"], &sheets);
                let id = npc["sprite"]["sheet"].as_str().unwrap();
                *placed.entry(id).or_default() += 1;
                seen.insert(id.to_owned());
            }
            if ["Xanafalgue", "Igglanova"].contains(&npc["symbol"].as_str().unwrap()) {
                assert_eq!(npc["interactable"], false);
            }
            if npc["symbol"] == "InvisibleBlock" {
                assert_eq!(npc["interactable"], true);
                assert!(npc["sprite"].is_null());
            }
        }
        for chest in array(&map["treasure_chests"]) {
            objects += 1;
            reference_contract(&chest["sprite"], &sheets);
            let id = chest["sprite"]["sheet"].as_str().unwrap();
            for name in ["idle_down", "idle_up"] {
                assert!(sheets[id]["sequences"].get(name).is_some());
            }
            *placed.entry(id).or_default() += 1;
            seen.insert(id.to_owned());
        }
        alternates(&map["map_effects"], &mut seen);
    }
    assert_eq!(seen, sheets.keys().map(|s| s.to_string()).collect());
    for (&id, sheet) in &sheets {
        assert_eq!(
            num(&sheet["placements"]),
            placed.get(id).copied().unwrap_or(0),
            "{id}"
        );
    }
    let sprites = &e.fragment["sprites"];
    assert_eq!(
        placed.values().sum::<usize>(),
        num(&sprites["npc_placements"])
    );
    assert_eq!(artless, num(&sprites["artless_objects"]));
    assert_eq!(placed.values().sum::<usize>() + artless, objects);
    assert_eq!(e.fragment["census"]["npc_interactable"], json!(interaction));
    assert_eq!(interaction.values().sum::<usize>(), npc_count);
    assert_eq!(sprites["interaction"]["render_flags_bit"], 3);
    assert_eq!(num(&sprites["interaction"]["types_interactable"]), 104);
    assert_eq!(num(&sprites["interaction"]["types_not_interactable"]), 118);
    assert_eq!(
        sprites["interaction"]["types_changing_at_runtime"],
        json!(["FellowPenguin"])
    );
}

#[test]
fn exceptional_facing_story_chest_and_manifest_bindings() {
    let Some(e) = emission() else { return };
    let pub_map = e.maps.values().find(|m| m["symbol"] == "AiedoPub").unwrap();
    let odd: Vec<_> = array(&pub_map["npcs"])
        .iter()
        .filter(|n| n["facing"]["name"].is_null())
        .collect();
    assert_eq!(odd.len(), 1);
    assert_eq!(odd[0]["index"], 2);
    assert_eq!(odd[0]["symbol"], "NPCType30");
    assert_eq!(odd[0]["facing"], json!({"id":16,"name":null}));
    assert_eq!(odd[0]["sprite"]["facing"], "facing_0x10");
    assert_eq!(odd[0]["sprite"]["idle_sequence"], "idle_facing_0x10");
    let sheet = array(&e.npcs["sheets"])
        .iter()
        .find(|s| s["id"] == odd[0]["sprite"]["sheet"])
        .unwrap();
    assert_eq!(
        sheet["sequences"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["idle_facing_0x10", "walk_facing_0x10"]
    );
    let alshline = array(&e.maps[&0x4a]["treasure_chests"])
        .iter()
        .find(|c| c["chest_flag"] == 8)
        .unwrap();
    assert_eq!(alshline["item_id"], 141);
    assert_eq!(alshline["item_symbol"], "Alshline");
    assert_eq!(
        (num(&alshline["x_cell"]), num(&alshline["y_cell"])),
        (40, 32)
    );
    let s = &e.fragment["sprites"];
    assert_eq!(
        s["palette"]["cram_lines"],
        json!({"0x00":0,"0x20":1,"0x40":2,"0x60":3})
    );
    assert_eq!(s["palette"]["map_palette_lines"], json!([0, 1, 3]));
    for key in ["fixed_line", "party_line"] {
        assert_eq!(s["palette"][key], 2);
    }
    assert_eq!(s["palette"]["fixed_line_rom_offset"], "0x296300");
    assert_eq!(s["walk"]["frames_per_cell"], 8);
    assert_eq!(s["walk"]["normal_block"], 1);
    assert_eq!(
        array(&s["walk"]["blocks"])
            .iter()
            .map(|b| num(&b["frames_per_cell"]))
            .collect::<Vec<_>>(),
        [16, 8, 4]
    );
}

#[test]
fn broken_output_references_frame_indices_and_image_metadata_are_rejected() {
    let Some(e) = emission() else { return };
    let sheets: BTreeMap<_, _> = array(&e.npcs["sheets"])
        .iter()
        .map(|s| (s["id"].as_str().unwrap(), s))
        .collect();
    let reference = &e.maps[&0x10]["npcs"][0]["sprite"];
    reference_contract(reference, &sheets);
    for key in ["sheets", "sheet", "idle_sequence", "walk_sequence"] {
        let mut bad = reference.clone();
        bad[key] = json!("missing");
        assert!(
            std::panic::catch_unwind(|| reference_contract(&bad, &sheets)).is_err(),
            "{key}"
        );
    }
    let sheet = &e.party["sheets"][0];
    let image = &e.sprite_images[sheet["png"].as_str().unwrap()];
    for key in ["index", "duration_ticks"] {
        let mut bad = sheet.clone();
        bad["sequences"]["walk_down"]["frames"][0][key] =
            json!(if key == "index" { 12 } else { 0 });
        assert!(std::panic::catch_unwind(|| sheet_contract(&bad, image)).is_err());
    }
    let mut bad = sheet.clone();
    bad["png_sha256"] = json!("wrong");
    assert!(std::panic::catch_unwind(|| sheet_contract(&bad, image)).is_err());
}

#[test]
fn camp_npc_resolves_to_its_content_named_line_three_sheet() {
    let Some(e) = emission() else { return };
    let r = crate::sprites::tests::retail().unwrap();
    let rom = r.rom.bytes();
    let decodes = crate::decode::Decodes::new(rom);
    let records = crate::maps::extract_maps(rom, &decodes).unwrap();
    let record = records.real().find(|m| m.id == 0x13).unwrap();
    let specs: Vec<_> = record
        .tilesets
        .entries
        .iter()
        .map(|p| (usize::from(p.word), p.source))
        .collect();
    let patterns = crate::maps::layouts::decode_tilesets(rom, &decodes, &specs).unwrap();
    let art = crate::sprites::field::stage_map_art(
        &decodes,
        record,
        crate::sprites::field::tile_source_from_patterns(&patterns),
    )
    .unwrap();
    let palette = crate::maps::layouts::map_palette_rgb(rom, record.palette.pointer).unwrap();
    let object = &record.objects.entries[0].record;
    let routine = r
        .routines
        .iter()
        .find(|p| p.object_id() == usize::from(object.object_id))
        .unwrap();
    let expected = crate::sprites::field::object_sprite(
        rom,
        routine,
        &crate::sprites::objects::facing_table_extents(&r.routines),
        &art,
        &palette,
        object.facing_dir,
        object.art_tile,
        None,
    )
    .unwrap()
    .sheet
    .unwrap();
    // Derive the name locally instead of committing the old ROM-derived hash.
    let id = format!("{}_{}", routine.symbol, &expected.identity()[..8]);
    let top = &e.maps[&0x13]["npcs"][0];
    assert_eq!(top["index"], 0);
    assert_eq!(top["sprite"]["sheet"], id);
    let sheet = array(&e.npcs["sheets"])
        .iter()
        .find(|s| s["id"] == id)
        .unwrap();
    assert_eq!(sheet["palette"]["cram_line"], 3);
    assert_eq!(sheet["palette"]["colors"], json!(palette[32..48]));
    assert_eq!(sheet["palette"]["colors"][13], json!([98, 68, 172]));
    assert_eq!(
        png::decode(&e.sprite_images[sheet["png"].as_str().unwrap()])
            .unwrap()
            .pixels,
        expected.strip().2
    );
}

#[test]
fn camp_receipt_plane_a_keeps_its_status_word() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/states/camp_root_idle_vdp_7675.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local camp frame-7675 state absent: status word check skipped");
            return;
        }
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    };
    let state: Value = serde_json::from_slice(&bytes).unwrap();
    let plane = state["regions"]["plane_a"]["bytes_hex"].as_str().unwrap();
    let (x, y) = (28 + 11, (4 + 29) % 32);
    let offset = (y * 64 + x) * 2;
    assert_eq!(
        u16::from_str_radix(&plane[offset * 2..offset * 2 + 4], 16).unwrap(),
        0xc6f9
    );
}
