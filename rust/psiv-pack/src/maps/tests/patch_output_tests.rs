//! Emitted patch output, not scene fidelity (#554, test_map_patches.py).
use super::emission_tests::emission;
use super::retail_tests::{array, num, retail};
use crate::maps::{draw, layouts, overworld, patches};
use crate::{decode::Decodes, png};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn writes(m: &Value) -> impl Iterator<Item = &Value> {
    array(&m["map_effects"])
        .iter()
        .flat_map(|e| array(&e["paths"]))
        .flat_map(|p| array(&p["writes"]))
}
fn cells(rect: &Value) -> impl Iterator<Item = (usize, usize)> {
    let (x, y, w, h) = (
        num(&rect["x"]),
        num(&rect["y"]),
        num(&rect["width"]),
        num(&rect["height"]),
    );
    (y..y + h).flat_map(move |y| (x..x + w).map(move |x| (x, y)))
}

#[test]
fn emitted_writes_resolve_every_chunk_cells_atlas_reference_and_census() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let records = crate::maps::extract_maps(r.rom.bytes(), &Decodes::new(r.rom.bytes())).unwrap();
    let mut map_count = 0;
    let mut atlas_maps = BTreeMap::new();
    let mut write_count = 0;
    let mut door_cells = 0;
    for m in e.maps.values() {
        let written: Vec<_> = writes(m).filter(|w| w["kind"] == "layout_write").collect();
        for w in writes(m).filter(|w| w["kind"] != "layout_write") {
            assert!(w.get("cells").is_none());
        }
        if !written.is_empty() {
            map_count += 1;
        }
        let ids: BTreeSet<_> = written
            .iter()
            .map(|w| num(&w["chunk_id"]))
            .chain(
                patches::scene_patch_chunks(
                    r.rom.bytes(),
                    records.real().find(|r| r.id == num(&m["id"])).unwrap(),
                )
                .unwrap()
                .into_iter()
                .map(usize::from),
            )
            .chain(
                m["map_updates"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|u| u["program"]["kind"] == "conveyor_belts")
                    .flat_map(|u| array(&u["program"]["groups"]))
                    .flat_map(|g| {
                        array(&g["frames_clear"])
                            .iter()
                            .chain(array(&g["frames_set"]).iter())
                    })
                    .map(num),
            )
            .collect();
        let pairs: BTreeSet<_> = written
            .iter()
            .map(|w| (num(&w["fg_chunk_id"]), num(&w["bg_chunk_id"])))
            .chain(
                m["overworld_patches"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|g| array(&g["tiles"]))
                    .map(|t| (num(&t["fg_chunk_id"]), num(&t["bg_chunk_id"]))),
            )
            .chain(
                m["chunk_def_copies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|g| array(&g["tiles"]))
                    .map(|t| (num(&t["fg_chunk_id"]), num(&t["bg_chunk_id"]))),
            )
            .collect();
        // The manifest also lists scene-only raw atlases, while the decoder
        // census above counts only maps with MapDataManager layout writes.
        if !ids.is_empty() {
            atlas_maps.insert(num(&m["id"]), (written.len(), ids.len()));
        }
        let atlas = &m["patch_tiles"];
        if ids.is_empty() && pairs.is_empty() {
            assert!(atlas.is_null());
            continue;
        }
        let tiles = array(&atlas["tiles"]);
        assert_eq!(num(&atlas["count"]), ids.len() + pairs.len());
        assert_eq!(
            tiles
                .iter()
                .filter(|t| !t["chunk_id"].is_null())
                .map(|t| num(&t["chunk_id"]))
                .collect::<BTreeSet<_>>(),
            ids
        );
        assert_eq!(
            tiles
                .iter()
                .filter(|t| t["chunk_id"].is_null())
                .map(|t| (num(&t["fg_chunk_id"]), num(&t["bg_chunk_id"])))
                .collect::<BTreeSet<_>>(),
            pairs
        );
        assert_eq!(atlas["tile_pixels"], 32);
        for key in ["png", "png_over"] {
            if let Some(path) = atlas[key].as_str() {
                let image = &e.patch_images[path];
                assert_eq!(
                    atlas[format!("{key}_sha256")],
                    png::image_sha256(image).unwrap()
                );
                let decoded = png::decode(image).unwrap();
                assert_eq!((decoded.width, decoded.height), (tiles.len() * 32, 32));
            }
        }
        for (i, t) in tiles.iter().enumerate() {
            assert_eq!(num(&t["index"]), i);
            assert_eq!(num(&t["x"]), i * 32);
            assert_eq!(array(&t["collision"]).len(), 4);
            assert!(array(&t["collision"]).iter().all(|v| num(v) < 16));
        }
        for w in written {
            write_count += 1;
            let resolved = array(&w["cells"]);
            assert_eq!(resolved.len(), 4);
            assert_eq!(
                (num(&w["cell_x"]), num(&w["cell_y"])),
                (num(&w["chunk_x"]) * 2, num(&w["chunk_y"]) * 2)
            );
            let t = &tiles[num(&w["patch_tile"])];
            assert!(t["chunk_id"].is_null());
            assert_eq!(t["fg_chunk_id"], w["fg_chunk_id"]);
            assert_eq!(t["bg_chunk_id"], w["bg_chunk_id"]);
            assert_eq!(
                t[format!("{}_chunk_id", w["plane"].as_str().unwrap())],
                w["chunk_id"]
            );
            assert_eq!(
                w["collision_authoritative"],
                w["plane"] == m["collision"]["plane"]
            );
            for (i, c) in resolved.iter().enumerate() {
                assert_eq!(
                    (num(&c["x"]), num(&c["y"])),
                    (num(&w["cell_x"]) + i % 2, num(&w["cell_y"]) + i / 2)
                );
                if w["collision_authoritative"] == true {
                    assert_eq!(c["collision"], t["collision"][i]);
                }
                door_cells += usize::from(c["collision"] == 1);
            }
        }
    }
    assert_eq!(map_count, 14);
    assert!(write_count > 100);
    assert_eq!(door_cells, 6);
    assert!(e.maps[&0xaf]["patch_tiles"].is_null());
    let c = &e.fragment["map_effects"]["layout_write_resolution"];
    assert_eq!(num(&c["map_count"]), atlas_maps.len());
    assert_eq!(array(&c["maps"]).len(), atlas_maps.len());
    for row in array(&c["maps"]) {
        let &(writes, chunks) = atlas_maps.get(&num(&row["id"])).unwrap();
        assert_eq!(num(&row["writes"]), writes);
        assert_eq!(num(&row["chunks"]), chunks);
        assert_eq!(num(&row["cells"]), writes * 4);
    }
    assert_eq!(num(&c["layout_writes"]), write_count);
    assert_eq!(num(&c["cells"]), write_count * 4);
    assert_eq!(
        num(&c["collision_authoritative"]) + num(&c["picture_only"]),
        write_count
    );
    assert_eq!(num(&c["map_change_cells"]), 6);
    assert_eq!(c["cells_per_chunk"], 4);
    assert_eq!(c["atlas_tile_pixels"], 32);
}

#[test]
fn packed_patch_cells_open_the_five_stored_map_doors() {
    let Some(e) = emission() else { return };
    let mut opened = 0;
    for id in [0x24, 0x28, 0x2c, 0xaf] {
        let m = &e.maps[&id];
        let patched: BTreeMap<_, _> = writes(m)
            .filter(|w| w["kind"] == "layout_write")
            .flat_map(|w| array(&w["cells"]))
            .map(|c| ((num(&c["x"]), num(&c["y"])), num(&c["collision"])))
            .collect();
        for w in array(&m["warps"])
            .iter()
            .filter(|w| w["table"] == 2 && !w["rect"].is_null())
        {
            let stored = cells(&w["rect"])
                .filter(|&(x, y)| m["collision"]["rows"][y][x] == 1)
                .count();
            let after = cells(&w["rect"])
                .filter(|&(x, y)| {
                    patched
                        .get(&(x, y))
                        .copied()
                        .unwrap_or_else(|| num(&m["collision"]["rows"][y][x]))
                        == 1
                })
                .count();
            assert!(
                after >= stored.max(1),
                "{} -> {}",
                m["symbol"],
                w["target"]["symbol"]
            );
            opened += usize::from(stored == 0);
        }
    }
    assert_eq!(opened, 5);
    let reported = array(&e.fragment["warps"]["doors_without_map_change_cell"])
        .iter()
        .filter(|d| [0x24, 0x28, 0x2c, 0xaf].contains(&num(&d["id"])))
        .count();
    assert_eq!(reported, opened);
}

#[test]
fn all_overworld_doors_have_their_stored_or_event_opened_map_change_cell() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let decodes = Decodes::new(r.rom.bytes());
    let records = crate::maps::extract_maps(r.rom.bytes(), &decodes).unwrap();
    let mut total = 0;
    let mut seen = BTreeSet::new();
    for record in records.real().take(2) {
        let w = overworld::decode_overworld(r.rom.bytes(), &decodes, record).unwrap();
        assert!(record.transitions.entries.is_empty());
        let m = &e.maps[&record.id];
        let plane = w.layout.collision_layout();
        let all = overworld::apply_patches(plane, &w.patches.iter().collect::<Vec<_>>());
        for door in array(&m["warps"]).iter().filter(|d| d["table"] == 2) {
            total += 1;
            let rect = &door["rect"];
            assert!(
                num(&rect["x"]) + num(&rect["width"]) <= 256
                    && num(&rect["y"]) + num(&rect["height"]) <= 256
            );
            let coords: Vec<_> = cells(rect).collect();
            let original =
                |x, y| layouts::collision_at(&w.layout.chunks, plane, x, y).unwrap() == 1;
            if !coords.iter().any(|&(x, y)| original(x, y)) {
                let opening = overworld::opening_patches(&w, &coords).unwrap();
                assert_eq!(opening.len(), 1);
                let symbol = door["target"]["symbol"].as_str().unwrap();
                let expected = match symbol {
                    "MotaSpaceport" => "EventFlag_MotaSpaceport",
                    "MachineCenter" => "EventFlag_MachineCenter",
                    "TheEdge" => "EventFlag_Reunion",
                    "DezoSpaceport" => "EventFlag_DezoSpaceport",
                    _ => panic!("unexpected shut door {symbol}"),
                };
                assert_eq!(opening[0].event_symbol(), Some(expected));
                seen.insert((record.id, symbol.to_owned()));
            }
            assert!(coords.iter().any(|&(x, y)| original(x, y)
                || layouts::collision_at(&w.layout.chunks, &all, x, y).unwrap() == 1));
        }
    }
    assert_eq!(total, 43);
    assert_eq!(seen.len(), 4);
    let piata = array(&e.maps[&0x10]["warps"])
        .iter()
        .find(|w| w["target"]["id"] == 0)
        .unwrap();
    assert_eq!(piata["destination"], json!({"x_cell":46,"y_cell":143}));
    let back = array(&e.maps[&0]["warps"])
        .iter()
        .find(|w| w["target"]["id"] == 0x10)
        .unwrap();
    assert_eq!(back["rect"], json!({"x":44,"y":140,"width":4,"height":4}));
    assert!(cells(&back["rect"]).all(|(x, y)| e.maps[&0]["collision"]["rows"][y][x] == 1));
}

#[test]
fn palette_copies_only_repaint_registered_npcs_and_do_not_add_placements() {
    let Some(e) = emission() else { return };
    let by_id: BTreeMap<_, _> = array(&e.npcs["sheets"])
        .iter()
        .map(|s| (s["id"].as_str().unwrap(), s))
        .collect();
    let mut found = BTreeSet::new();
    let mut swaps = 0;
    for (&id, m) in &e.maps {
        for path in array(&m["map_effects"])
            .iter()
            .flat_map(|e| array(&e["paths"]))
        {
            for copy in path.get("deferred_effects").map_or(&[][..], array) {
                found.insert(id);
                assert_eq!(copy["resolved_as"], "palette_write");
                assert_eq!(copy["cram_line"], 3);
                assert_eq!(copy["words"], 16);
                assert_eq!(array(&copy["colors"]).len(), 16);
                assert_eq!(copy["affects"]["map_pixels"], 0);
                assert_eq!(copy["affects"]["map_render_identical"], true);
                for swap in array(&copy["affects"]["npc_sheets"]) {
                    swaps += 1;
                    let (from, to) = (swap["from"].as_str().unwrap(), swap["to"].as_str().unwrap());
                    assert_ne!(from, to);
                    assert_eq!(m["npcs"][num(&swap["npc_index"])]["sprite"]["sheet"], from);
                    let (before, after) = (by_id[from], by_id[to]);
                    for key in ["frame_width", "frame_height", "frame_count"] {
                        assert_eq!(before[key], after[key]);
                    }
                    assert_eq!(after["palette"]["cram_line"], 3);
                    assert_ne!(before["palette"]["colors"], after["palette"]["colors"]);
                    assert!(e.npc_files.contains(after["png"].as_str().unwrap()));
                }
                if id == 0x28 {
                    assert_eq!(
                        array(&path["writes"])
                            .iter()
                            .map(|w| w["kind"].as_str().unwrap())
                            .collect::<BTreeSet<_>>(),
                        BTreeSet::from(["object_despawn", "object_x", "object_y"])
                    );
                }
            }
        }
    }
    assert_eq!(found, BTreeSet::from([0x24, 0x28, 0x2c]));
    assert!(swaps > 0);
    let census = &e.fragment["map_effects"]["palette_copies"];
    assert_eq!(census["cram_line"], 3);
    assert_eq!(census["map_tiles_affected"], 0);
    assert_eq!(census["palette_copies"], 3);
    assert!(num(&census["alternate_sheets"]) > 0);
    assert_eq!(num(&census["repainted_npcs"]), swaps);
    let placed: usize = by_id.values().map(|s| num(&s["placements"])).sum();
    assert_eq!(placed, num(&e.fragment["sprites"]["npc_placements"]));
    let objects: usize = e
        .maps
        .values()
        .map(|m| {
            array(&m["npcs"]).len()
                + array(&m["treasure_chests"])
                    .iter()
                    .filter(|c| !c["sprite"].is_null())
                    .count()
        })
        .sum();
    assert_eq!(
        placed + num(&e.fragment["sprites"]["artless_objects"]),
        objects
    );
}

/// Read the actual atlas pixels and compare with a separately rendered patched
/// map crop. This is extraction parity at (42,33), not native scene parity.
#[test]
fn bridge_two_plane_patch_matches_full_map_base_and_priority_crops() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let decodes = Decodes::new(r.rom.bytes());
    let records = crate::maps::extract_maps(r.rom.bytes(), &decodes).unwrap();
    let record = records.real().find(|r| r.id == 0).unwrap();
    let world = overworld::decode_overworld(r.rom.bytes(), &decodes, record).unwrap();
    let m = &e.maps[&0];
    let raw: Vec<_> = array(&m["layout_patches"])
        .iter()
        .filter(|p| p["event_flag"]["id"] == 0x35)
        .collect();
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0]["routine"], "0x053D16");
    let writes = array(&raw[0]["writes"]);
    assert_eq!(writes.len(), 2);
    for (w, (plane, ids)) in writes
        .iter()
        .zip([("fg", json!(["0x00"])), ("bg", json!(["0x48"]))])
    {
        assert_eq!(w["plane"], plane);
        assert_eq!(w["chunk_x"], 42);
        assert_eq!(w["chunk_y"], 33);
        assert_eq!(w["chunk_ids"], ids);
    }
    let group = array(&m["overworld_patches"])
        .iter()
        .find(|g| g["event_flag"] == 0x35)
        .unwrap();
    let [tile] = array(&group["tiles"]) else {
        panic!("one crossing")
    };
    for (key, value) in [
        ("chunk_x", 42),
        ("chunk_y", 33),
        ("fg_chunk_id", 0),
        ("bg_chunk_id", 0x48),
        ("collision_chunk_id", 0x48),
    ] {
        assert_eq!(tile[key], value);
    }
    assert_eq!(tile["collision"], json!([0, 0, 0, 0]));
    let selected: Vec<_> = world
        .patches
        .iter()
        .filter(|p| p.event_flag == 0x35)
        .collect();
    let mut patched = world.layout.clone();
    assert_eq!(
        layouts::collision_at(&patched.chunks, &patched.bg, 84, 66).unwrap(),
        9
    );
    patched.fg = overworld::apply_patches(&patched.fg, &selected);
    patched.bg = overworld::apply_patches(&patched.bg, &selected);
    assert_eq!(
        layouts::collision_at(&patched.chunks, &patched.bg, 84, 66).unwrap(),
        0
    );
    let palette = layouts::chunk_palette(r.rom.bytes(), patched.spec.palette).unwrap();
    let full = draw::render_layout(
        &patched.chunks,
        &patched.bg,
        &patched.fg,
        &patched.patterns,
        &palette,
    )
    .unwrap();
    let (over, _) = draw::priority_overlay(&patched, &palette).unwrap();
    let atlas = &m["patch_tiles"];
    let at = num(&tile["patch_tile"]);
    let entry = &atlas["tiles"][at];
    assert!(entry["chunk_id"].is_null());
    assert_eq!(entry["fg_chunk_id"], 0);
    assert_eq!(entry["bg_chunk_id"], 0x48);
    for (key, full) in [("png", full), ("png_over", over.unwrap())] {
        let full = png::decode(&full).unwrap();
        let small = png::decode(&e.patch_images[atlas[key].as_str().unwrap()]).unwrap();
        assert_eq!(full.plte, small.plte);
        assert_eq!(full.trns, small.trns);
        for y in 0..32 {
            assert_eq!(
                &small.pixels[y * small.width + at * 32..y * small.width + (at + 1) * 32],
                &full.pixels
                    [(33 * 32 + y) * full.width + 42 * 32..(33 * 32 + y) * full.width + 43 * 32]
            );
        }
    }
    let mut bad = world.clone();
    let mut conflict = selected[0].clone();
    conflict.event_flag = 0x36;
    bad.patches.push(conflict);
    assert!(
        patches::resolve_overworld_patches(&world.layout, Some(&bad))
            .unwrap_err()
            .0
            .contains("overlapping flags")
    );
}
