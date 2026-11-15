//! Atlas contracts use compiled scene operations, including platform builders,
//! rather than parsing Rust source. Scene/ROM choreography guards stay Python.
use super::emission_tests::emission;
use super::retail_tests::{array, num, retail};
use crate::{
    decode::Decodes,
    maps::{draw, extract_maps, layouts, overworld, patches},
    png,
};
use psiv_core::{SCENES, SceneOp};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use patches::scene_tests::cited_rows;

fn uncited(
    writes: &BTreeMap<&str, Vec<(u32, u32, u16)>>,
    rows: &BTreeMap<&str, (usize, BTreeSet<u16>)>,
) -> Vec<String> {
    let mut problems = BTreeSet::new();
    for (scene, writes) in writes {
        for &(_, _, chunk) in writes {
            if !rows
                .get(scene)
                .is_some_and(|(_, chunks)| chunks.contains(&chunk))
            {
                problems.insert(format!("{scene}: chunk ${chunk:02X} has no cited row"));
            }
        }
    }
    problems.into_iter().collect()
}

fn registry() -> BTreeMap<&'static str, Vec<(u32, u32, u16)>> {
    SCENES
        .iter()
        .filter_map(|s| {
            let writes: Vec<_> = s
                .ops
                .iter()
                .flat_map(|op| match op {
                    SceneOp::WriteMapChunks { chunks } => *chunks,
                    _ => &[],
                })
                .copied()
                .collect();
            (!writes.is_empty()).then_some((s.name, writes))
        })
        .collect()
}

fn unresolved(scene: &str, record: &Value, writes: &[(u32, u32, u16)]) -> Vec<String> {
    let tiles = record["patch_tiles"]["tiles"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let indices: BTreeSet<_> = tiles.iter().map(|t| num(&t["index"])).collect();
    writes
        .iter()
        .filter_map(|&(x, y, chunk)| {
            let raw = tiles.iter().any(|t| t["chunk_id"] == chunk);
            let composed = record["overworld_patches"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|p| array(&p["tiles"]))
                .any(|t| {
                    t["collision_chunk_id"] == chunk
                        && t["chunk_x"] == x
                        && t["chunk_y"] == y
                        && indices.contains(&num(&t["patch_tile"]))
                });
            (!raw && !composed).then(|| {
                format!(
                    "{scene}: chunk ${chunk:02X} does not resolve in map ${:03X}",
                    num(&record["id"])
                )
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn every_compiled_scene_write_and_platform_pair_resolves_in_the_emitted_map() {
    let Some(e) = emission() else { return };
    let writes = registry();
    let rows = cited_rows(retail().unwrap().rom.bytes());
    assert_eq!(
        writes.keys().collect::<BTreeSet<_>>(),
        rows.keys().collect()
    );
    for (scene, written) in &writes {
        let (map, expected) = &rows[scene];
        assert_eq!(
            written.iter().map(|(_, _, c)| *c).collect::<BTreeSet<_>>(),
            *expected,
            "{scene}"
        );
        assert_eq!(
            unresolved(scene, &e.maps[map], written),
            Vec::<String>::new()
        );
    }
    // Each of the six compiled platform builders must retain both pairs of
    // start/end writes; the independent retail transcription stays Python.
    assert!(uncited(&writes, &rows).is_empty());
    for (scene, written) in &writes {
        if scene.contains("MovingPlatform") {
            assert_eq!(written.len(), 8, "{scene}");
        }
    }
    // Every tower map with either live door event must carry all table chunks.
    let mut tower_maps = 0;
    let mut written = BTreeSet::new();
    for s in SCENES
        .iter()
        .filter(|s| s.name.starts_with("Event_GaruberkTwDoor"))
    {
        for op in s.ops {
            if let SceneOp::WriteActorMapChunks { chunks, .. } = op {
                written.extend(chunks.iter().map(|(_, _, c)| *c));
            }
        }
    }
    assert_eq!(written, (0x30..0x3f).collect());
    for m in e
        .maps
        .values()
        .filter(|m| (0x199..=0x19f).contains(&num(&m["id"])))
    {
        if array(&m["interaction_areas"])
            .iter()
            .any(|a| a["event_index"] == 0x35 || a["event_index"] == 0x36)
        {
            tower_maps += 1;
            assert!(written.iter().all(|&chunk| {
                array(&m["patch_tiles"]["tiles"])
                    .iter()
                    .any(|t| t["chunk_id"] == chunk)
            }));
        }
    }
    assert!(tower_maps > 0);
}

#[test]
fn missing_rows_chunks_and_wrong_composite_references_are_named() {
    let writes = registry();
    let Some(r) = retail() else { return };
    let rows = cited_rows(r.rom.bytes());
    let mut missing = rows.clone();
    missing.remove("Event_TylerGraveOpening");
    assert_eq!(
        uncited(&writes, &missing),
        ["Event_TylerGraveOpening: chunk $47 has no cited row"]
    );
    let mut missing = rows.clone();
    missing
        .get_mut("Cutscene_PsycoWand")
        .unwrap()
        .1
        .remove(&0x90);
    assert_eq!(
        uncited(&writes, &missing),
        ["Cutscene_PsycoWand: chunk $90 has no cited row"]
    );
    // Raw atlas resolution checks every start/end member, not just one pair.
    let chunks: Vec<_> = (3..11).map(|c| json!({"index":c-3,"chunk_id":c})).collect();
    let mut m = json!({"id":1,"patch_tiles":{"tiles":chunks}});
    let pairs: Vec<_> = (3..11).map(|c| (1, 2, c)).collect();
    assert!(unresolved("platform", &m, &pairs).is_empty());
    for missing in 3..11 {
        let mut broken = m.clone();
        broken["patch_tiles"]["tiles"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["chunk_id"] != missing);
        assert_eq!(
            unresolved("platform", &broken, &pairs),
            [format!(
                "platform: chunk ${missing:02X} does not resolve in map $001"
            )]
        );
    }
    m = json!({"id":0,"patch_tiles":{"tiles":[{"index":7,"chunk_id":null}]},
        "overworld_patches":[{"tiles":[{"collision_chunk_id":3,"chunk_x":1,"chunk_y":2,"patch_tile":7}]}]});
    assert!(unresolved("world", &m, &[(1, 2, 3)]).is_empty());
    for key in ["chunk_x", "chunk_y", "patch_tile", "collision_chunk_id"] {
        let mut broken = m.clone();
        broken["overworld_patches"][0]["tiles"][0][key] = json!(99);
        assert_eq!(unresolved("world", &broken, &[(1, 2, 3)]).len(), 1, "{key}");
    }
    let Some(e) = emission() else { return };
    let mut temple = e.maps[&0x14c].clone();
    temple["patch_tiles"]["tiles"]
        .as_array_mut()
        .unwrap()
        .retain(|t| t["chunk_id"] != 0x57);
    let problems = unresolved(
        "Cutscene_CrashLaanding",
        &temple,
        &writes["Cutscene_CrashLaanding"],
    );
    assert_eq!(
        problems,
        ["Cutscene_CrashLaanding: chunk $57 does not resolve in map $14C"]
    );
    let mut tower = e.maps[&0x19a].clone();
    assert!(unresolved("Garuberk door", &tower, &[(0, 0, 0x3c)]).is_empty());
    tower["patch_tiles"]["tiles"]
        .as_array_mut()
        .unwrap()
        .retain(|t| t["chunk_id"] != 0x3c);
    assert_eq!(
        unresolved("Garuberk door", &tower, &[(0, 0, 0x3c)]),
        ["Garuberk door: chunk $3C does not resolve in map $19A"]
    );
}

#[test]
fn crash_landing_uses_the_temple_bg_stride_and_all_five_animation_frames() {
    let Some(r) = retail() else { return };
    let data = r.rom.bytes();
    let word = |at| crate::m68k::w(data, at).unwrap() as usize;
    assert_eq!(word(0x76822), 0x14c);
    assert_eq!(0x76ae8 + word(0x76ae8), 0x76e00);
    assert_eq!((word(0x76e02), word(0x76e06)), (47, 9));
    assert_eq!(data[0x76e09], 1);
    assert_eq!((word(0x76e24), word(0x76e28), word(0x76e2c)), (1, 64, 65));
    assert_eq!(data[0x76e17] + 1, 6);
    let records = extract_maps(data, &Decodes::new(data)).unwrap();
    let record = records.real().find(|r| r.id == 0x14c).unwrap();
    assert_eq!(record.dimensions.bg_row_size as usize + 1, 64);
    assert_eq!(record.scroll.mode, 1);
    let chunks = patches::scene_patch_chunks(data, record).unwrap();
    assert_eq!(chunks, (0x50..0x60).collect::<Vec<_>>());
    assert!(
        patches::scene_patch_chunks(data, records.real().find(|r| r.id == 1).unwrap())
            .unwrap()
            .is_empty()
    );
    let table = &data[0x76e58..0x76e6c];
    assert_eq!(data[0x76e6c], 0xff);
    let frames: Vec<_> = SCENES
        .iter()
        .find(|s| s.name == "Cutscene_CrashLaanding")
        .unwrap()
        .ops
        .iter()
        .filter_map(|op| {
            if let SceneOp::WriteMapChunks { chunks } = op {
                Some(*chunks)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(frames.len(), 5);
    for (frame, retail) in frames.iter().zip(table.chunks_exact(4)) {
        assert_eq!(
            frame.iter().map(|(x, y, _)| (*x, *y)).collect::<Vec<_>>(),
            [(47, 9), (48, 9), (47, 10), (48, 10)]
        );
        assert_eq!(
            frame.iter().map(|(_, _, c)| *c).collect::<Vec<_>>(),
            retail.iter().map(|&c| u16::from(c)).collect::<Vec<_>>()
        );
    }
    let e = emission().unwrap();
    let m = &e.maps[&0x14c];
    let atlas = &m["patch_tiles"];
    let raw: Vec<_> = array(&atlas["tiles"])
        .iter()
        .filter(|t| !t["chunk_id"].is_null())
        .collect();
    assert_eq!(
        raw.iter().map(|t| num(&t["chunk_id"])).collect::<Vec<_>>(),
        (0x50..0x60).collect::<Vec<_>>()
    );
    assert!(
        array(&atlas["tiles"])[16..]
            .iter()
            .all(|t| t["chunk_id"].is_null())
    );
    let decoded = overworld::decode_map_section(data, &Decodes::new(data), record).unwrap();
    let map = decoded.layout();
    let palette = layouts::chunk_palette(data, map.spec.palette).unwrap();
    // Compare each emitted raw tile with a separate one-chunk map render,
    // including priority, transparency and collision.
    for tile in raw {
        let chunk = num(&tile["chunk_id"]);
        assert_eq!(
            tile["collision"],
            json!(if [0x58, 0x5e, 0x5f].contains(&chunk) {
                vec![8; 4]
            } else {
                vec![0; 4]
            })
        );
        let mut one = map.clone();
        one.bg.width_chunks = 1;
        one.bg.height_chunks = 1;
        one.bg.cells = vec![chunk as u8];
        one.fg.width_chunks = 1;
        one.fg.height_chunks = 1;
        one.fg.cells = vec![0];
        let pixels = draw::compose_layout(
            &one.chunks,
            &one.bg,
            &one.patterns,
            None,
            &mut draw::no_prepared_chunks(),
        )
        .unwrap();
        let base = png::encode_indexed(32, 32, &pixels, &palette[..32], &[]).unwrap();
        let (over, _) = draw::priority_overlay(&one, &palette).unwrap();
        let over = over.unwrap_or_else(|| {
            png::encode_indexed(
                32,
                32,
                &[0; 32 * 32],
                &palette[..32],
                &draw::OVERLAY_TRANSPARENT_INDICES,
            )
            .unwrap()
        });
        for (key, reference) in [("png", base), ("png_over", over)] {
            let expected = png::decode(&reference).unwrap();
            let got = png::decode(&e.patch_images[atlas[key].as_str().unwrap()]).unwrap();
            assert_eq!(got.plte, expected.plte);
            assert_eq!(got.trns, expected.trns);
            for y in 0..32 {
                let start = y * got.width + num(&tile["x"]);
                assert_eq!(
                    got.pixels[start..start + 32],
                    expected.pixels[y * 32..(y + 1) * 32],
                    "chunk {chunk:#x} {key}"
                );
            }
        }
    }
}

#[test]
fn emitted_effects_variants_and_manifest_retain_their_contract() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let records = extract_maps(r.rom.bytes(), &Decodes::new(r.rom.bytes())).unwrap();
    let updates = crate::maps::updates::extract_map_updates(r.rom.bytes(), records.real()).unwrap();
    for record in records.real() {
        assert_eq!(
            e.maps[&record.id]["map_updates"],
            json!(
                updates.per_map[&record.id]
                    .iter()
                    .map(Value::from)
                    .collect::<Vec<_>>()
            ),
            "map {} update order and programs",
            record.id
        );
    }
    for id in [0x24, 0x2c, 0x19a] {
        assert!(e.maps[&id]["map_effects"].is_array());
        assert!(e.maps[&id]["layout_variants"].is_array());
    }
    assert!(!array(&e.maps[&0x24]["map_effects"]).is_empty());
    let map = &e.maps[&0x19a];
    let variants = array(&map["layout_variants"]);
    assert_eq!(variants.len(), 1);
    let v = &variants[0];
    assert_eq!(
        array(&v["planes"])
            .iter()
            .map(|p| p["identical_to_base"].clone())
            .collect::<Vec<_>>(),
        [json!(true), json!(false)]
    );
    assert!(num(&v["differs_from_base_cells"]) > 0);
    assert_eq!(v["collision"]["width_cells"], 96);
    assert_eq!(array(&v["collision"]["rows"]).len(), 96);
    for key in ["width_cells", "height_cells"] {
        assert_eq!(v["collision"][key], map["collision"][key]);
    }
    for key in ["png", "png_over"] {
        assert!(e.files.contains(v[key].as_str().unwrap()));
    }
    let c = &e.fragment["map_effects"];
    assert_eq!(c["slice"], 1);
    assert_eq!(c["evaluated"], "map load only");
    assert_eq!(c["referenced_entries"], 127);
    assert!(array(&c["kinds_emitted"]).contains(&json!("object_despawn")));
    assert!(
        array(&c["maps_with_layout_variants"])
            .iter()
            .any(|m| m["id"] == 0x19a)
    );
    let temple = array(&c["scene_patch_chunks"])
        .iter()
        .find(|m| m["id"] == 0x14c)
        .unwrap();
    assert_eq!(temple["chunks"], json!((0x50..0x60).collect::<Vec<_>>()));
}
