//! Character extraction and local disassembly oracles from
//! tests/test_battle_art.py (#550). No cartridge payload or digest is stored.

use std::collections::BTreeSet;

use super::*;
use crate::battle::image_tests::{Output, assert_png, palette, plane};
use crate::test_support::retail_rom;

#[test]
fn plc_blocks_poses_art_extents_and_the_wren_overrun() {
    let Some(rom) = retail_rom("character pose census") else {
        return;
    };
    let rom = rom.bytes();
    let records = records(rom).unwrap();
    let extents = extents(rom, &records).unwrap();
    assert_eq!((records.len(), extents.len()), (11, 13));
    assert_eq!(records[0].plc, 0x8274);
    assert_eq!(
        (COLUMNS, ROWS, SLOT_WORDS, BASE_WORD),
        (6, 6, 36, 0x8000 | (3 << 13))
    );
    let mut poses = 0;
    let mut oversized = Vec::new();
    let mut overallocated = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let end = records.get(index + 1).map_or(0x836A, |r| r.plc);
        assert_eq!(
            end - record.plc,
            2 + 4 * (record.art.len() + record.poses.len())
        );
        assert_eq!(
            record.art.len(),
            if [6, 7].contains(&index) { 2 } else { 1 }
        );
        assert_eq!(record.poses[0].art, record.art[0]);
        let largest = record.art.iter().map(|a| extents[a] * 32).max().unwrap();
        assert!(usize::from(record.vram_bytes) >= largest);
        if usize::from(record.vram_bytes) != largest {
            overallocated.push(index);
        }
        for (pose, p) in record.poses.iter().enumerate() {
            let (words, _) = pose_words(rom, p).unwrap();
            assert!(highest(&words) < 64);
            assert!(words.len() >= 36);
            if words.len() != 36 {
                oversized.push((index, pose, words.len()));
            }
            poses += 1;
        }
    }
    assert_eq!(poses, 43);
    assert_eq!(overallocated, [0]);
    assert_eq!(oversized, [(7, 3, 42)]);
    let distinct: BTreeSet<_> = extents
        .iter()
        .map(|(&at, &n)| &rom[at..at + n * 32])
        .collect();
    assert_eq!(distinct.len(), 13);
}

#[test]
fn every_pose_png_uses_the_first_36_cells_and_on_foot_palette() {
    let Some(rom) = retail_rom("character image content") else {
        return;
    };
    let rom = rom.bytes();
    let root = Output::new("character-images");
    let emitted = serde_json::Value::from(&write(rom, &root.0).unwrap());
    assert_eq!(emitted["characters"].as_array().unwrap().len(), 11);
    assert_eq!(emitted["pose_count"], 43);
    assert_eq!(emitted["palette"]["cram_line"], 3);
    let records = records(rom).unwrap();
    let extents = extents(rom, &records).unwrap();
    let mut colors = vec![[0; 3]];
    colors.extend(palette(rom, 0x76A4, 15));
    for vehicle in 0..4 {
        let mut expected = vec![[0; 3]];
        expected.extend(palette(rom, 0x76A4 + 30 * vehicle, 15));
        assert_eq!(rgb(&cram_line(rom, vehicle).unwrap()).unwrap(), expected);
        assert_eq!(
            emitted["palette"]["variants"][vehicle]["colors"],
            serde_json::json!(expected)
        );
    }
    assert_eq!(verify_ui_colors(rom).unwrap().len(), 3);
    for (record, output) in records
        .iter()
        .zip(emitted["characters"].as_array().unwrap())
    {
        assert_eq!(output["pose_count"], record.poses.len());
        assert_eq!(
            output["poses"].as_array().unwrap().len(),
            record.poses.len()
        );
        for (number, (pose, out)) in record
            .poses
            .iter()
            .zip(output["poses"].as_array().unwrap())
            .enumerate()
        {
            let (words, _) = pose_words(rom, pose).unwrap();
            assert_eq!(out["pose"], number);
            assert_eq!(out["label"], POSE_LABELS[record.index][number]);
            assert_eq!(out["label_source"], LABEL_SOURCE);
            assert_eq!(out["cell_count"], words.len());
            let raw = &rom[pose.art..pose.art + extents[&pose.art] * 32];
            let pixels = plane(&words[..36], 6, raw, 0);
            let bytes = std::fs::read(root.0.join(out["png"].as_str().unwrap())).unwrap();
            assert_png(&bytes, (48, 48), &pixels, &colors);
            assert_eq!(out["png_sha256"], png::image_sha256(&bytes).unwrap());
        }
    }
}

/// A present but incomplete/unreadable reference must fail, just as a
/// present but unusable ROM does. Missing optional reference skips explicitly.
fn reference() -> Option<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm");
    match root.try_exists() {
        Ok(false) => {
            eprintln!("disassembly absent: character art oracle skipped");
            None
        }
        Ok(true) => Some(root),
        Err(e) => panic!("cannot inspect reference: {e}"),
    }
}

fn find_unique(rom: &[u8], bytes: &[u8]) -> usize {
    assert!(!bytes.is_empty());
    let matches: Vec<_> = rom
        .windows(bytes.len())
        .enumerate()
        .filter_map(|(i, b)| (b == bytes).then_some(i))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "reference payload must occur once in retail ROM"
    );
    matches[0]
}

#[test]
fn local_disassembly_names_every_pose_and_bounds_every_art_blob() {
    let Some(rom) = retail_rom("character disassembly oracle") else {
        return;
    };
    let Some(reference) = reference() else { return };
    let rom = rom.bytes();
    let records = records(rom).unwrap();
    let extents = extents(rom, &records).unwrap();
    let mut names = BTreeMap::new();
    for path in std::fs::read_dir(reference.join("plane mappings/characters")).unwrap() {
        let path = path.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        let name = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(" Enigma", "");
        assert!(names.insert(find_unique(rom, &bytes), name).is_none());
    }
    assert_eq!(names.len(), 43);
    let mut actions = BTreeSet::new();
    for record in &records {
        for (index, pose) in record.poses.iter().enumerate() {
            let name = &names[&pose.mapping];
            let action = name
                .strip_prefix(&format!("{} ", CHARACTERS[record.index]))
                .unwrap();
            assert_eq!(action.replace(' ', ""), POSE_LABELS[record.index][index]);
            if index == 0 {
                assert_eq!(action, "Idle");
            }
            actions.insert(action);
        }
    }
    assert_eq!(
        actions,
        BTreeSet::from([
            "Idle",
            "Cast",
            "Cast 2",
            "Arms Raised",
            "Arms Raised 2",
            "Charge",
            "Charge Magic",
            "Load Gun",
            "Fire Gun",
            "Slasher",
            "Two Slashers",
            "Throw Slasher",
            "Throw Two Slashers"
        ])
    );
    let mut found = BTreeSet::new();
    for path in std::fs::read_dir(reference.join("graphics/characters")).unwrap() {
        let path = path.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap();
        if !name.ends_with("Long Range") && !["Demi Gun", "Wren Gun"].contains(&name) {
            continue;
        }
        let bytes = std::fs::read(path).unwrap();
        let at = find_unique(rom, &bytes);
        assert_eq!(extents[&at] * 32, bytes.len());
        assert!(found.insert(at));
    }
    assert_eq!(found, extents.keys().copied().collect());
}
