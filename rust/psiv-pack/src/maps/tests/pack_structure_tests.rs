//! File/report contracts from the remainder of the retired test_pack_maps suite.

use super::emission_tests::{Emission, emission};
use super::retail_tests::{array, num, retail};
use serde_json::{Value, json};

fn named<'a>(e: &'a Emission, symbol: &str) -> &'a Value {
    e.maps.values().find(|m| m["symbol"] == symbol).unwrap()
}
fn anomalies<'a>(e: &'a Emission, symbol: &str) -> Vec<&'a Value> {
    array(&e.fragment["layout_anomalies"])
        .iter()
        .filter(|a| a["symbol"] == symbol)
        .collect()
}
fn assert_image_size(m: &Value, size: (usize, usize, u8)) {
    assert_eq!(size.2, 3);
    assert_eq!(
        (size.0, size.1),
        (num(&m["width_pixels"]), num(&m["height_pixels"]))
    );
}

#[test]
fn image_files_dimensions_no_overlay_and_draw_order_metadata() {
    let Some(e) = emission() else { return };
    for entry in array(&e.fragment["maps"]) {
        let size = e.images[entry["png"].as_str().unwrap()];
        assert_image_size(entry, size);
        let d = &e.maps[&num(&entry["id"])]["dimensions"];
        assert_image_size(d, size);
        assert_eq!(
            (size.0, size.1),
            (num(&d["width_cells"]) * 16, num(&d["height_cells"]) * 16)
        );
    }
    let edge = named(e, "TheEdge");
    let entry = array(&e.fragment["maps"])
        .iter()
        .find(|m| m["symbol"] == "TheEdge")
        .unwrap();
    assert!(entry["png_over"].is_null());
    assert!(entry["png_over_sha256"].is_null());
    assert_eq!(entry["priority_tiles"], 0);
    assert!(edge.as_object().unwrap().contains_key("png_over"));
    assert!(edge["png_over"].is_null());
    let updates = edge["map_update_images"].as_object().unwrap();
    assert_eq!(
        updates.keys().collect::<Vec<_>>(),
        [entry["png"].as_str().unwrap()]
    );
    let mut expected = std::collections::BTreeSet::from([
        "maps/100_TheEdge.json".to_owned(),
        "maps/100_TheEdge.png".to_owned(),
    ]);
    expected.extend(updates.values().map(|v| v.as_str().unwrap().to_owned()));
    assert_eq!(
        e.files
            .iter()
            .filter(|p| p.starts_with("maps/100_TheEdge"))
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    let over = &e.fragment["overlays"];
    assert_eq!(over["priority_bit"], 15);
    assert_eq!(over["file_suffix"], "_over.png");
    assert_eq!(over["transparent_palette_indices"], json!([0, 16]));
    assert_eq!(
        num(&over["maps_with_overlay"]) + num(&over["maps_without_overlay"]),
        e.maps.len()
    );
    assert!(
        array(&over["without_overlay"])
            .iter()
            .any(|m| m["symbol"] == "TheEdge")
    );
    let tiles: usize = array(&e.fragment["maps"])
        .iter()
        .map(|m| num(&m["priority_tiles"]))
        .sum();
    assert_eq!(num(&over["priority_tiles"]), tiles);
    assert_eq!(
        e.fragment["census"]["priority_tiles"]
            .as_object()
            .unwrap()
            .values()
            .map(num)
            .sum::<usize>(),
        tiles
    );
    let mut bad = array(&e.fragment["maps"])[0].clone();
    bad["width_pixels"] = json!(1);
    let size = e.images[bad["png"].as_str().unwrap()];
    assert!(std::panic::catch_unwind(|| assert_image_size(&bad, size)).is_err());
}

#[test]
fn piatas_written_image_is_the_two_plane_composition() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let rom = r.rom.bytes();
    let decodes = crate::decode::Decodes::new(rom);
    let records = super::extract_maps(rom, &decodes).unwrap();
    let p = records.real().find(|m| m.id == 0x10).unwrap();
    let spec = super::layouts::layout_spec(p).unwrap();
    assert_eq!(spec.chunk_blobs, [0x122a90, 0x123380]);
    assert_eq!((spec.layout_fg, spec.layout_bg), (0x123710, 0x1237b0));
    assert_eq!(
        (
            spec.width_chunks_fg,
            spec.height_chunks_fg,
            spec.width_chunks_bg,
            spec.height_chunks_bg
        ),
        (32, 32, 32, 32)
    );
    assert_eq!(
        spec.tilesets,
        [(0x10, 0x11c808), (0x111, 0x11de68), (0x213, 0x11f4a8)]
    );
    assert_eq!(spec.palette, 0x123990);
    assert_eq!(spec.label, "Piata");
    assert_eq!(spec.collision_plane, 1);
    for id in [0, 1] {
        let world = records.real().find(|m| m.id == id).unwrap();
        assert!(super::layouts::layout_spec(world).is_err());
        let world = super::overworld::decode_map_section(rom, &decodes, world).unwrap();
        assert!(world.overworld().is_some());
        assert_eq!(
            (
                world.layout().collision.width,
                world.layout().collision.height
            ),
            (256, 256)
        );
    }
    let section = super::overworld::decode_map_section(rom, &decodes, p).unwrap();
    assert!(section.overworld().is_none());
    let d = section.layout();
    let mut prepared = super::draw::no_prepared_chunks();
    let ground =
        super::draw::compose_layout(&d.chunks, &d.bg, &d.patterns, None, &mut prepared).unwrap();
    let composed = super::draw::compose_layout(
        &d.chunks,
        &d.fg,
        &d.patterns,
        Some(ground.clone()),
        &mut prepared,
    )
    .unwrap();
    assert_ne!(ground, composed);
    let png = crate::png::decode(&e.piata_png).unwrap();
    assert_eq!(png.pixels, composed);
    let palette = super::layouts::map_palette_rgb(rom, d.spec.palette).unwrap();
    assert_eq!(
        e.piata_png,
        super::draw::render_layout(&d.chunks, &d.bg, &d.fg, &d.patterns, &palette).unwrap()
    );
}

#[test]
fn emitted_layout_anomalies_keep_padding_short_planes_and_undefined_cells() {
    let Some(e) = emission() else { return };
    let academy = named(e, "PiataAcademy");
    assert_eq!(
        (
            num(&academy["dimensions"]["width_cells"]),
            num(&academy["dimensions"]["height_cells"])
        ),
        (64, 32)
    );
    assert_eq!(
        (
            num(&academy["dimensions"]["width_pixels"]),
            num(&academy["dimensions"]["height_pixels"])
        ),
        (1024, 512)
    );
    let padded = anomalies(e, "PiataAcademy");
    assert_eq!(padded.len(), 2);
    assert_eq!(
        padded
            .iter()
            .map(|a| a["plane"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["fg", "bg"]
    );
    for a in padded {
        assert_eq!(a["kind"], "padded");
        assert_eq!(a["blob_bytes"], 1024);
        assert_eq!(a["grid_bytes"], 512);
    }
    let short = anomalies(e, "ClimCenter_F2");
    assert_eq!(short.len(), 1);
    assert_eq!(short[0]["kind"], "short");
    assert_eq!(short[0]["plane"], "bg");
    assert_eq!(short[0]["blob_bytes"], 1024);
    assert_eq!(short[0]["grid_bytes"], 2304);
    assert_eq!(named(e, "ClimCenter_F2")["collision"]["plane"], "fg");
    let stray = anomalies(e, "InnerSanctuary_B1");
    assert_eq!(stray.len(), 1);
    assert_eq!(stray[0]["kind"], "undefined_chunk");
    assert_eq!(stray[0]["loaded_chunks"], 128);
    assert_eq!(stray[0]["chunk_ids"], json!(["0xFF"]));
    assert_eq!(
        stray[0]["cells"],
        json!([{"plane":"bg","x":26,"y":28,"chunk_id":"0xFF"}])
    );
    let rows = &named(e, "InnerSanctuary_B1")["collision"]["rows"];
    for y in [56, 57] {
        for x in [52, 53] {
            assert_eq!(rows[y][x], 0);
        }
    }
}

#[test]
fn five_manager_doors_and_six_event_doors_are_reported_in_order() {
    let Some(e) = emission() else { return };
    let doors = array(&e.fragment["warps"]["doors_without_map_change_cell"]);
    let manager: Vec<_> = doors
        .iter()
        .filter(|d| d["symbol"] == "Zema" || d["symbol"] == "BirthValley_B1")
        .map(|d| {
            (
                d["symbol"].as_str().unwrap(),
                d["target"]["symbol"].as_str().unwrap(),
                d["rect"].clone(),
            )
        })
        .collect();
    assert_eq!(
        manager,
        vec![
            (
                "Zema",
                "ZemaHouse1",
                json!({"x":44,"y":23,"width":1,"height":1})
            ),
            (
                "Zema",
                "ZemaWeaponShop",
                json!({"x":26,"y":31,"width":1,"height":1})
            ),
            (
                "Zema",
                "ZemaInn",
                json!({"x":36,"y":31,"width":1,"height":1})
            ),
            (
                "Zema",
                "ZemaItemShop",
                json!({"x":42,"y":43,"width":1,"height":1})
            ),
            (
                "BirthValley_B1",
                "BioPlant",
                json!({"x":22,"y":15,"width":2,"height":1})
            )
        ]
    );
    let world: Vec<_> = doors
        .iter()
        .filter(|d| d["id"] == 0 || d["id"] == 1)
        .map(|d| {
            (
                d["target"]["symbol"].as_str().unwrap(),
                array(&d["opened_by"])
                    .iter()
                    .map(|f| f["symbol"].as_str().unwrap())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        world,
        vec![
            ("MachineCenter", vec!["EventFlag_MachineCenter"]),
            ("MotaSpaceport", vec!["EventFlag_MotaSpaceport"]),
            ("TheEdge", vec!["EventFlag_Reunion"]),
            ("TheEdge", vec!["EventFlag_Reunion"]),
            ("TheEdge", vec!["EventFlag_Reunion"]),
            ("DezoSpaceport", vec!["EventFlag_DezoSpaceport"])
        ]
    );
}

#[test]
fn world_map_files_layout_patches_collision_census_and_page_alias_reports() {
    let Some(e) = emission() else { return };
    for (id, symbol, warps, patches) in [(0, "Motavia", 29, 9), (1, "Dezolis", 14, 3)] {
        let m = &e.maps[&id];
        assert_eq!(m["symbol"], symbol);
        assert_eq!(
            m["dimensions"],
            json!({"cell_pixels":16,"height_cells":256,"height_chunks":128,"height_pixels":4096,"width_cells":256,"width_chunks":128,"width_pixels":4096})
        );
        assert_eq!(m["collision"]["plane"], "bg");
        assert_eq!(array(&m["collision"]["rows"]).len(), 256);
        assert!(
            array(&m["collision"]["rows"])
                .iter()
                .all(|r| array(r).len() == 256)
        );
        assert!(array(&m["npcs"]).is_empty());
        assert!(array(&m["treasure_chests"]).is_empty());
        assert_eq!(array(&m["warps"]).len(), warps);
        assert!(array(&m["warps"]).iter().all(|w| w["table"] == 2));
        assert_eq!(array(&m["layout_patches"]).len(), patches);
    }
    let piata = &e.maps[&0x10];
    assert!(piata.get("layout_patches").is_none());
    let patch = array(&e.maps[&0]["layout_patches"])
        .iter()
        .find(|p| p["event_flag"]["symbol"] == "EventFlag_MotaSpaceport")
        .unwrap();
    assert_eq!(
        patch["writes"],
        json!([{"plane":"bg","chunk_x":26,"chunk_y":45,"cell_x":52,"cell_y":90,"chunk_ids":["0x3F"],"displacement":0}])
    );
    let world = &e.fragment["overworld"];
    assert_eq!(world["map_ids"], json!([0, 1]));
    assert_eq!(
        array(&world["maps"])
            .iter()
            .map(|m| num(&m["map_id"]))
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(world["maps"][0]["compression"].is_null());
    assert_eq!(world["code"]["page_tables"]["fg"]["motavia"], "0x107DC2");
    assert_eq!(world["code"]["page_tables"]["bg"]["dezolis"], "0x1175C4");
    assert_eq!(e.fragment["census"]["collision_types"]["10"], 632);
    assert_eq!(e.fragment["census"]["collision_types"]["11"], 1704);
    for code in [10, 11] {
        assert!(super::layouts::is_blocking(code));
        assert!(
            e.fragment["collision"]["type_names"]
                .get(format!("0x{code:X}"))
                .is_some()
        );
    }
    let aliases = anomalies(e, "Dezolis");
    assert_eq!(aliases.len(), 2);
    assert_eq!(
        aliases
            .iter()
            .map(|a| a["plane"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["fg", "bg"]
    );
    for a in aliases {
        assert_eq!(a["kind"], "aliased_pages");
        assert_eq!(a["distinct_pages"], 8);
        assert_eq!(array(&a["aliased_pages"]).len(), 8);
    }
    let entrance = array(&e.maps[&0]["warps"])
        .iter()
        .find(|w| w["target"]["id"] == 0x10)
        .unwrap();
    assert_eq!(
        entrance["rect"],
        json!({"x":44,"y":140,"width":4,"height":4})
    );
    assert_eq!(e.maps[&0]["collision"]["rows"][143][46], 1);
}
