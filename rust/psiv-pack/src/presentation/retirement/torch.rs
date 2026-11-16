//! Every Eclipse Torch mapping and counter, drawn again from the local ROM.
use super::*;
use crate::sprites::{
    compose::{Reading, TileSource, compose_frame, mapping_box},
    records::{decode_mapping, decode_sequence},
};

fn check_torch(e: &Emission, manifest: &Value) -> Result<(), String> {
    let sheet = array(&manifest["temporary_objects"])
        .iter()
        .find(|r| r["object_id"] == 0x338 && r["art_tile"] == 0x3AF)
        .ok_or("missing Eclipse Torch sheet")?;
    let data = e.rom.bytes();
    let setup: Vec<_> = [0x41F9u16, 0x001D, 0x712C, 0x303C, 0x03AF]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    assert_eq!(&data[0x702CE..0x702CE + setup.len()], setup);
    assert_eq!(sheet["scene"], "Event_EclipseTorchUsed");
    assert_eq!(sheet["map_id"], "0x001");
    assert_eq!(sheet["source_rom_addr"], "0x1D712C");
    assert_eq!(sheet["source_compression"], "nemesis");
    assert_eq!(
        sheet["load_art"],
        json!({"source_rom_addr":"0x1D712C","destination_tile":"0x03AF"})
    );
    assert_eq!(sheet["render_status"], "exact");
    assert_eq!(sheet["missing_patterns"], json!([]));
    for (key, value) in [
        ("frame_width", 32),
        ("frame_height", 32),
        ("frame_count", 4),
        ("origin_x", 0),
        ("origin_y", 0),
    ] {
        if sheet[key] != value {
            return Err(format!("torch {key}"));
        }
    }
    assert_eq!(
        sheet["palette"],
        json!({"cram_line":0,"source":"map palette blob","transparent_index":0,"colors":gfx::palette_rgb(&gfx::decode_palette(&data[0x119604..0x119624]).unwrap())})
    );
    let (raw, consumed) = nemesis::decompress(data, 0x1D712C).unwrap();
    assert_eq!(sheet["source_size"], raw.len());
    assert_eq!(sheet["source_consumed"], consumed);
    let mut source = TileSource::default();
    source.add(0x3AF, &raw).unwrap();
    let im = image(e, sheet);
    assert_eq!((im.colour, im.width, im.height), (3, 128, 32));
    assert_eq!(im.trns, [0]);
    let mappings = [0x5132A, 0x51332, 0x5133A, 0x51342];
    for (slot, (name, offset, delay)) in [
        ("down", 0x50396, 7),
        ("up", 0x503A8, 11),
        ("right", 0x503BA, 15),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(be(data, 0x502C2 + slot * 4, 4), offset);
        let sequence = decode_sequence(data, offset).unwrap();
        assert_eq!(sequence.frames, mappings.map(|m| (m, delay)));
        let packed = &sheet["sequences"][format!("walk_{name}")];
        if packed["sequence_offset"] != format!("0x{offset:06X}") {
            return Err("torch sequence address".into());
        }
        if packed["frames"]
            != json!(
                (0..4)
                    .map(|i| json!({"index":i,"duration_ticks":delay+1}))
                    .collect::<Vec<_>>()
            )
        {
            return Err("torch sequence frames".into());
        }
        for (frame, mapping) in array(&packed["frames"]).iter().zip(mappings) {
            let mapping = decode_mapping(data, mapping).unwrap();
            let (rendered, missing) = compose_frame(
                &mapping,
                &source,
                mapping_box(&mapping),
                Reading {
                    art_tile: 0x3AF,
                    tile_props: 0,
                    streamed: false,
                },
                None,
            );
            assert!(missing.is_empty());
            let left = num(&frame["index"]) * 32;
            let crop: Vec<u8> = (0..32)
                .flat_map(|y| {
                    im.pixels[y * 128 + left..y * 128 + left + 32]
                        .iter()
                        .copied()
                })
                .collect();
            assert_eq!(crop, rendered.pixels);
        }
    }
    Ok(())
}

#[test]
fn torch_sheet_preserves_every_mapping_and_duration_and_rejects_mutations() {
    let Some(e) = emission() else { return };
    check_torch(e, &e.manifest).unwrap();
    let mut bad = e.manifest.clone();
    bad["temporary_objects"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["object_id"] != 0x338);
    assert_eq!(
        check_torch(e, &bad).unwrap_err(),
        "missing Eclipse Torch sheet"
    );
    for field in ["index", "duration_ticks"] {
        let mut bad = e.manifest.clone();
        let sheet = bad["temporary_objects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["object_id"] == 0x338)
            .unwrap();
        sheet["sequences"]["walk_down"]["frames"][0][field] = json!(99);
        assert_eq!(check_torch(e, &bad).unwrap_err(), "torch sequence frames");
    }
    let calls: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/decode_census.json")).unwrap();
    let torch = json!({"decoder":"nemesis","offset":0x1D712C});
    let covered = |calls: &[Value]| calls.contains(&torch);
    assert!(covered(array(&calls["calls"])));
    let missing: Vec<_> = array(&calls["calls"])
        .iter()
        .filter(|r| **r != torch)
        .cloned()
        .collect();
    assert!(!covered(&missing));
}

#[test]
fn decoder_census_covers_every_observed_presentation_call_and_detects_omissions() {
    use crate::decode::Call;
    let Some(e) = emission() else { return };
    let census: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/decode_census.json")).unwrap();
    let observed: Vec<Value> = e.calls.iter().map(|call| match call {
        Call::Nemesis {offset} => json!({"decoder":"nemesis","offset":offset}),
        Call::Kosinski {offset} => json!({"decoder":"kosinski","offset":offset}),
        Call::Enigma {offset,base_tile,max_words} => json!({"decoder":"enigma","offset":offset,"base_tile":base_tile,"max_words":max_words}),
    }).collect();
    assert!(!observed.is_empty());
    let missing = |calls: &[Value]| {
        observed
            .iter()
            .filter(|v| !calls.contains(v))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(missing(array(&census["calls"])), Vec::<Value>::new());
    let without_one: Vec<_> = array(&census["calls"])
        .iter()
        .filter(|v| **v != observed[0])
        .cloned()
        .collect();
    assert_eq!(missing(&without_one), vec![observed[0].clone()]);
}
