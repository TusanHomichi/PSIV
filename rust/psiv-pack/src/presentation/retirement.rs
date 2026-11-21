//! Emitted presentation contracts from the retired test_presentation_pack suite (#559).
use super::*;
use crate::test_support::{retail_rom, scratch};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

mod palettes;
mod torch;

struct Emission {
    rom: Rom,
    manifest: Value,
    files: BTreeMap<String, Vec<u8>>,
    calls: Vec<crate::decode::Call>,
}
fn emission() -> Option<&'static Emission> {
    static E: OnceLock<Option<Emission>> = OnceLock::new();
    E.get_or_init(|| {
        let rom = retail_rom("presentation records")?;
        let root = scratch("presentation");
        let (_, calls) = crate::decode::observed::during(|| build(&rom, &root).unwrap());
        fn collect(root: &Path, at: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(root, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .replace('\\', "/"),
                        std::fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut files = BTreeMap::new();
        collect(&root, &root, &mut files);
        let manifest = serde_json::from_slice(&files["presentation/panels.json"]).unwrap();
        std::fs::remove_dir_all(root).unwrap();
        Some(Emission {
            rom,
            manifest,
            files,
            calls,
        })
    })
    .as_ref()
}
fn hex(v: &Value) -> usize {
    usize::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}
fn num(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn array(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn image<'a>(e: &'a Emission, record: &Value) -> png::Image<'a> {
    png::decode(&e.files[record["png"].as_str().unwrap()]).unwrap()
}

fn check_panels(rom: &[u8], m: &Value) -> Result<(), String> {
    let records = array(&m["panels"]);
    let expected = panel_ids();
    let ids_vec: Vec<u16> = records.iter().map(|r| num(&r["id"]) as u16).collect();
    let missing = psiv_core::missing_panel_ids(psiv_core::SCENES, &ids_vec);
    if !missing.is_empty() {
        let details: Vec<String> = missing
            .into_iter()
            .map(|(scene, id)| format!("{scene}:0x{id:X}"))
            .collect();
        return Err(format!("missing registry panels: {}", details.join(", ")));
    }
    if records.len() != expected.len() {
        return Err("panel count".into());
    }
    let ids: BTreeSet<_> = ids_vec.into_iter().collect();
    if ids != expected.into_iter().collect() || ids.len() != records.len() {
        return Err("panel identities".into());
    }
    for r in records {
        let at = panel_record_offset(num(&r["id"]) as u16).unwrap();
        let raw = &rom[at..at + PANEL_RECORD_SIZE];
        // Panel record stores dimensions, screen coordinates, and two layers.
        let (cols, rows, x, y) = (
            usize::from(raw[6]),
            usize::from(raw[7]),
            usize::from(raw[8]),
            usize::from(raw[9]),
        );
        if (
            num(&r["columns"]),
            num(&r["rows"]),
            num(&r["x_cells"]),
            num(&r["y_cells"]),
        ) != (cols, rows, x, y)
        {
            return Err("panel layout".into());
        }
        if r["decoded_size"] != json!([cols * 8, rows * 8]) {
            return Err("panel size".into());
        }
        for (tile, mapping) in [(10, 16), (20, 26)] {
            let art = be(raw, tile, 2);
            if art != 0xFFFF {
                let (words, _) =
                    enigma::decompress(rom, be(raw, mapping, 4), art as u16, Some(cols * rows))
                        .unwrap();
                if [0x1A, 0x1B, 0x1F, 0x20, 0x33, 0x34, 0x3B, 0x3C]
                    .contains(&(num(&r["id"]) as u16))
                    && words.len() != cols * rows
                {
                    return Err("panel mapping length".into());
                }
            }
        }
    }
    Ok(())
}

#[test]
fn all_panel_records_keep_retail_layout_both_planes_and_generated_files() {
    let Some(e) = emission() else { return };
    let m = &e.manifest;
    check_panels(e.rom.bytes(), m).unwrap();
    assert_eq!(m["record_table"], "0x07B000");
    assert_eq!(m["record_size"], PANEL_RECORD_SIZE);
    for (id, c, r, x, y) in [
        (0x1A, 9, 13, 29, 5),
        (0x1B, 6, 10, 24, 2),
        (0x1F, 26, 11, 11, 2),
        (0x20, 18, 6, 3, 14),
        (0x33, 13, 17, 23, 2),
        (0x34, 20, 7, 4, 3),
        (0x3B, 13, 15, 23, 3),
        (0x3C, 15, 8, 10, 11),
    ] {
        let p = array(&m["panels"]).iter().find(|p| p["id"] == id).unwrap();
        assert_eq!(
            (
                num(&p["columns"]),
                num(&p["rows"]),
                num(&p["x_cells"]),
                num(&p["y_cells"])
            ),
            (c, r, x, y)
        );
    }
    for p in array(&m["panels"]) {
        image(e, p);
    }
    let mut bad = m.clone();
    bad["panels"][0]["columns"] = json!(1);
    assert_eq!(
        check_panels(e.rom.bytes(), &bad).unwrap_err(),
        "panel layout"
    );
    // Negative control (#413): dropping a registry-referenced panel fails with
    // the offending scene name and panel id.
    let mut dropped = m.clone();
    dropped["panels"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["id"] != 0x10D);
    let err = check_panels(e.rom.bytes(), &dropped).unwrap_err();
    assert!(
        err.contains("missing registry panels") && err.contains("0x10D"),
        "{err}"
    );
}

fn expected_loads() -> BTreeSet<(usize, usize)> {
    [
        (0x1D5DF0, 0x2E6),
        (0x1D628C, 0x179),
        (0x1D642E, 0x191),
        (0x1D6A2E, 0x1FB),
        (0x1D6BC8, 0x20C),
        (0x12951A, 0x3A5),
        (0x1289FA, 0x4A5),
        (0x1D712C, 0x3AF),
        (0x14A1CC, 0x372),
    ]
    .into()
}
fn expected_objects() -> BTreeSet<(usize, usize)> {
    [
        (0x18, 0x26A),
        (0x18, 0x55C),
        (0x194, 0x202),
        (0x194, 0x4A5),
        (0x214, 0x179),
        (0x218, 0x191),
        (0x21C, 0x1FB),
        (0x220, 0x20C),
        (0x188, 0x3A5),
        (0x1FC, 0x2E6),
        (0x338, 0x3AF),
        (0x344, 0x3BA),
        (0x348, 0x3BA),
        (0x34C, 0x372),
        (0x35C, 0x3BA),
        (0x360, 0x3BA),
        (0x364, 0x3BA),
    ]
    .into()
}
fn check_records(m: &Value, files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    if m["coverage"]
        != json!({"panel_records":panel_ids().len(),"load_art_records":9,"temporary_object_keys":17,"generic_portraits":1,"scene_pictures":1})
    {
        return Err("coverage".into());
    }
    let loads: BTreeSet<_> = array(&m["load_art"])
        .iter()
        .map(|r| (hex(&r["source_rom_addr"]), hex(&r["destination_tile"])))
        .collect();
    if loads != expected_loads() || array(&m["load_art"]).len() != 9 {
        return Err("load art identities".into());
    }
    let objects: BTreeSet<_> = array(&m["temporary_objects"])
        .iter()
        .map(|r| (num(&r["object_id"]), num(&r["art_tile"])))
        .collect();
    if objects != expected_objects() || array(&m["temporary_objects"]).len() != 17 {
        return Err("temporary object identities".into());
    }
    for section in [
        "load_art",
        "temporary_objects",
        "portraits",
        "scene_pictures",
    ] {
        for r in array(&m[section]) {
            let path = r["png"].as_str().unwrap();
            if !files.contains_key(path) {
                return Err(format!("missing {path}"));
            }
        }
    }
    Ok(())
}

#[test]
fn opening_scene_pictures_load_art_objects_and_portrait_are_additive() {
    let Some(e) = emission() else { return };
    let m = &e.manifest;
    check_records(m, &e.files).unwrap();
    assert_eq!(
        m["opening_background"],
        json!({"art":"0x1CF1F2","art_consumed":5681,"mapping":"0x1D25BE","mapping_consumed":338,"palette":"0x1D2A3C","screen_y":40,"size":[320,128]})
    );
    assert_eq!(array(&m["scene_pictures"]).len(), 1);
    let g = &m["scene_pictures"][0];
    assert_eq!(g["source_rom_addr"], "0x1DDE0A");
    assert_eq!(g["art"], "0x1DB2C8");
    assert_eq!(
        (num(&g["art_tile"]), num(&g["columns"]), num(&g["rows"])),
        (0x260, 40, 28)
    );
    assert_eq!(g["size"], json!([320, 224]));
    assert_eq!(
        g["scenes"],
        json!(["Event_MeetingLeRoof", "Cutscene_LeRoofAgain"])
    );
    let im = image(e, g);
    assert_eq!((im.colour, im.width, im.height), (3, 320, 224));
    assert!(im.trns.is_empty());
    assert!(im.pixels.iter().all(|p| *p < 16));
    for r in array(&m["load_art"]) {
        assert!(num(&r["tile_count"]) > 0);
    }
    let object = |id, tile| {
        array(&m["temporary_objects"])
            .iter()
            .find(|r| r["object_id"] == id && r["art_tile"] == tile)
            .unwrap()
    };
    assert_eq!(object(0x214, 0x179)["playback_sequence"], "walk_down");
    assert_eq!(object(0x218, 0x191)["playback_sequence"], "walk_up");
    assert_eq!(object(0x218, 0x191)["playback_once"], true);
    for (id, tile, at, words) in [
        (0x218, 0x191, 0x1DE0B8, 16),
        (0x188, 0x3A5, 0x52848, 14),
        (0x194, 0x202, 0x730FC, 16),
    ] {
        let colors =
            gfx::palette_rgb(&gfx::decode_palette(&e.rom.bytes()[at..at + words * 2]).unwrap());
        assert_eq!(
            &array(&object(id, tile)["palette"]["colors"])[..words],
            json!(colors).as_array().unwrap()
        );
    }
    assert_eq!(object(0x188, 0x3A5)["playback_sequence"], "walk_down");
    for (id, tile) in [
        (0x18, 0x26A),
        (0x18, 0x55C),
        (0x194, 0x202),
        (0x344, 0x3BA),
        (0x348, 0x3BA),
        (0x34C, 0x372),
        (0x35C, 0x3BA),
        (0x360, 0x3BA),
        (0x364, 0x3BA),
    ] {
        assert_eq!(object(id, tile)["render_status"], "exact");
    }
    assert!(
        object(0x344, 0x3BA)["sequences"]
            .as_object()
            .unwrap()
            .contains_key("walk_facing_0x10"),
        "Stripper $344 must include facing_dir $10 sequence"
    );
    assert!(
        object(0x34C, 0x372)["sequences"]
            .as_object()
            .unwrap()
            .contains_key("idle_up"),
        "StripClubCustomer $34C must include up/down facing sequences"
    );
    assert_eq!(
        object(0x1FC, 0x2E6)["render_status"],
        "partial_transparent_holes"
    );
    assert_eq!(
        object(0x1FC, 0x2E6)["missing_patterns"],
        json!((0x34B..0x38D).collect::<Vec<_>>())
    );
    for r in array(&m["temporary_objects"]) {
        assert!(num(&r["frame_count"]) > 0);
    }
    let portrait = &m["portraits"][0];
    assert_eq!(portrait["mapping"], "0x2A2B36");
    assert_eq!(portrait["destination_tile"], "0x055C");
    assert_eq!(portrait["size_pixels"], json!([48, 48]));
    for mutation in 0..4 {
        let mut bad = m.clone();
        match mutation {
            0 => {
                bad["temporary_objects"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|r| r["object_id"] != 0x338);
            }
            1 => bad["load_art"][0]["destination_tile"] = json!("0x0000"),
            2 => bad["portraits"][0]["png"] = json!("missing.png"),
            _ => bad["coverage"]["panel_records"] = json!(0),
        }
        assert!(
            check_records(&bad, &e.files).is_err(),
            "mutation {mutation}"
        );
    }
}
