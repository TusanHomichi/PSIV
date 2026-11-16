//! Retail patch extraction checks; original data stays local (#554).
use super::*;
use crate::decode::Decodes;
use crate::maps::{extract_maps, layouts, overworld::decode_map_section};

#[test]
fn every_scene_row_checks_its_pins_and_complete_table() {
    let Some(rom) = crate::test_support::retail_rom("scene atlas rows") else {
        return;
    };
    let data = rom.bytes();
    for row in SCENE_CHUNK_WRITES {
        assert!(!row_chunks(data, row).unwrap().is_empty(), "{}", row.scene);
        for &(at, _) in row.checks {
            let mut moved = data.to_vec();
            moved[at] ^= 1;
            let error = row_chunks(&moved, row).unwrap_err().0;
            assert!(
                error.contains(row.scene) && error.contains("moved"),
                "{error}"
            );
        }
    }
    let crash = SCENE_CHUNK_WRITES
        .iter()
        .find(|r| r.scene == "Cutscene_CrashLaanding")
        .unwrap();
    let (table, length) = crash.table.unwrap();
    for (at, byte) in [(table, 0xff), (table + length - 1, 0)] {
        let mut broken = data.to_vec();
        broken[at] = byte;
        assert!(
            row_chunks(&broken, crash)
                .unwrap_err()
                .0
                .contains("incomplete")
        );
    }
    assert!(
        row_chunks(&data[..table + length - 1], crash)
            .unwrap_err()
            .0
            .contains("incomplete")
    );
    assert_eq!(garuberk_door_chunks(data).unwrap(), (0x30..0x3f).collect());
    let mut broken = data.to_vec();
    broken[0x6f47b] ^= 1;
    assert!(
        garuberk_door_chunks(&broken)
            .unwrap_err()
            .0
            .contains("guard")
    );
    broken.copy_from_slice(data);
    broken[0x6f714 + 15] = 3;
    assert!(
        garuberk_door_chunks(&broken)
            .unwrap_err()
            .0
            .contains("incomplete")
    );
}

#[test]
fn deferred_copies_are_line_three_and_use_the_maps_original_chunks() {
    let Some(rom) = crate::test_support::retail_rom("map patch extraction") else {
        return;
    };
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let maps = extract_maps(data, &decodes).unwrap();
    for (routine, source) in [
        (0x51e4c, 0x133e20),
        (0x5203e, 0x12f570),
        (0x520a4, 0x12f570),
    ] {
        let copy = decode_palette_copy(data, routine).unwrap().unwrap();
        assert_eq!((copy.source, copy.offset, copy.words), (source, 3 * 32, 16));
        assert_eq!(&data[copy.at..copy.at + 2], &0x41f9u16.to_be_bytes());
        assert_eq!(data[copy.at + 8..copy.at + 10], 0xfb60u16.to_be_bytes());
        assert_eq!(data[copy.at + 12..copy.at + 14], 7u16.to_be_bytes());
        assert_eq!(
            gfx::decode_palette(&data[source..source + 32])
                .unwrap()
                .len(),
            16
        );
    }
    let record = maps.real().find(|r| r.id == 0x24).unwrap();
    let section = decode_map_section(data, &decodes, record).unwrap();
    let map = section.layout();
    assert_eq!(map.chunks.words.len(), 182);
    assert_eq!(
        chunk_collision_cells(map.chunks.get(0x5a).unwrap()),
        [8, 8, 1, 8]
    );
    assert_eq!(chunk_collision_cells(map.chunks.get(0x56).unwrap()), [8; 4]);
    assert!(map.chunks.get(0x59).is_ok());
    assert_eq!(record.palette.pointer + 96, 0x133e20);
    // Map words address lines 0/1 after bit 14 is removed. A line-3
    // copy therefore cannot alter any map pixel or install chunk definitions.
    let mut before = layouts::map_palette_rgb(data, record.palette.pointer).unwrap();
    let mut after = before.clone();
    let colors = gfx::decode_palette(&data[0x133e20..0x133e40]).unwrap();
    after[32..48].copy_from_slice(&gfx::palette_rgb(&colors));
    assert_ne!(before[32..48], after[32..48]);
    for &word in map.chunks.words.iter().flatten() {
        assert!((super::super::draw::vdp_word(word) >> 13) & 3 < 2);
    }
    before.truncate(32);
    after.truncate(32);
    assert_eq!(before, after);
    assert_eq!(map.spec.collision_plane(), Plane::Bg);
    let zio = maps.real().find(|r| r.id == 0xaf).unwrap();
    assert_eq!(
        layouts::layout_spec(zio).unwrap().collision_plane(),
        Plane::Fg
    );
    let effects = super::super::effects::extract_map_effects(data, maps.real()).unwrap();
    let (resolved, _, counts) = resolve_map_effects(map, &effects.per_map[&0x24]).unwrap();
    let doors: Vec<_> = resolved
        .iter()
        .flat_map(|e| items(e, "paths"))
        .flat_map(|p| items(p, "writes"))
        .filter(|w| int(w, "chunk_id") == Some(0x5a))
        .collect();
    assert_eq!(doors.len(), 4);
    assert_eq!(counts.picture_only, 1);
    for w in doors {
        assert_eq!(text(w, "plane"), "bg");
        assert_eq!(field(w, "collision_authoritative"), Some(&Json::Bool(true)));
    }
}

#[test]
fn scene_chunk_tables_preserve_animation_collision_and_map_ownership() {
    let Some(rom) = crate::test_support::retail_rom("scene patch chunks") else {
        return;
    };
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let maps = extract_maps(data, &decodes).unwrap();
    for (symbol, expected) in [
        ("BirthValley_B1", vec![0x29, 0x2a, 0x42, 0x43, 0x44, 0x45]),
        ("BioPlant_Part3", vec![0x4f, 0x50, 0x51, 0x52, 0x53]),
        ("Tyler", vec![0x47]),
        ("RajaTemple", (0x50..0x60).collect()),
        ("Zema", vec![]),
    ] {
        let record = maps.real().find(|r| r.symbol == symbol).unwrap();
        assert_eq!(
            scene_patch_chunks(data, record).unwrap(),
            expected,
            "{symbol}"
        );
        let section = decode_map_section(data, &decodes, record).unwrap();
        let map = section.layout();
        for id in expected {
            assert!(map.chunks.get(id).is_ok());
        }
        if symbol == "BirthValley_B1" {
            assert_eq!(
                chunk_collision_cells(map.chunks.get(0x29).unwrap()),
                [8, 8, 1, 1]
            );
        }
        if symbol == "BioPlant_Part3" {
            for id in 0x4f..0x53 {
                assert_eq!(chunk_collision_cells(map.chunks.get(id).unwrap()), [8; 4]);
            }
            assert_eq!(
                chunk_collision_cells(map.chunks.get(0x53).unwrap()),
                [8, 8, 1, 1]
            );
        }
        if symbol == "Tyler" {
            let mut moved = data.to_vec();
            moved[0x6fcf7] = 0x48;
            assert!(
                scene_patch_chunks(&moved, record)
                    .unwrap_err()
                    .0
                    .contains("Event_TylerGraveOpening: the write at $06FCF4 moved")
            );
        }
    }
    assert!(
        scene_patch_chunks(data, maps.real().find(|r| r.id == 0).unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn aiedo_stamp_keeps_foreground_and_exposes_changed_background_pixels() {
    let Some(rom) = crate::test_support::retail_rom("Aiedo patch composition") else {
        return;
    };
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let maps = extract_maps(data, &decodes).unwrap();
    let section = decode_map_section(
        data,
        &decodes,
        maps.real().find(|r| r.symbol == "Aiedo").unwrap(),
    )
    .unwrap();
    let m = section.layout();
    assert_eq!(m.spec.collision_plane(), Plane::Bg);
    for (x, y) in [(22, 37), (25, 37)] {
        let fg = m.fg.chunk_at(x, y);
        let bg = m.bg.chunk_at(x, y);
        assert_ne!(bg, 6);
        let (front, _) = compose_chunk(&m.chunks, &m.patterns, fg, false).unwrap();
        let (back, _) = compose_chunk(&m.chunks, &m.patterns, bg, false).unwrap();
        let (bare, _) = compose_chunk(&m.chunks, &m.patterns, 6, false).unwrap();
        let baked = over(&front, &back);
        let stamped = over(&front, &bare);
        assert!(
            front
                .iter()
                .zip(stamped.iter())
                .all(|(&f, &s)| f & 15 == 0 || f == s)
        );
        assert_eq!(
            baked
                .iter()
                .zip(stamped.iter())
                .filter(|(a, b)| a != b)
                .count(),
            38
        );
        assert!(
            front
                .iter()
                .zip(bare.iter())
                .any(|(&f, &b)| f & 15 != 0 && b != f)
        );
    }
}
