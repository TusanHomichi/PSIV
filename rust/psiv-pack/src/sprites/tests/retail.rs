use super::*;
use crate::{decode::Decodes, gfx, maps::layouts::map_palette_rgb};
use sha2::{Digest, Sha256};

/// Independent streamed renderer: read raw six-byte pieces and nibbles at
/// their ROM addresses. Does not call decode_mapping, decode_tile or compose.
fn streamed_pixels(rom: &[u8], art: usize, mapping: usize) -> Vec<u8> {
    let mut out = vec![0; 16 * 32];
    for index in 0..=usize::from(rom[mapping]) {
        let at = mapping + 2 + 6 * index;
        let (x, y) = (rom[at + 4] as i8 as i32, rom[at] as i8 as i32 + 2);
        let word = u16::from_be_bytes([rom[at + 2], rom[at + 3]]);
        let (w, h) = (
            ((rom[at + 1] >> 2) & 3) as usize + 1,
            (rom[at + 1] & 3) as usize + 1,
        );
        for py in 0..h * 8 {
            for px in 0..w * 8 {
                let sx = if word & 0x800 != 0 {
                    w * 8 - 1 - px
                } else {
                    px
                };
                let tile = usize::from(word & 0x7ff) + (sx / 8) * h + py / 8;
                let byte = rom[art + tile * 32 + (py % 8) * 4 + (sx % 8) / 2];
                let value = if sx % 2 == 0 { byte >> 4 } else { byte & 15 };
                let (dx, dy) = (x + px as i32, y + py as i32);
                if value != 0 && (0..16).contains(&dx) && (0..32).contains(&dy) {
                    out[dy as usize * 16 + dx as usize] = value;
                }
            }
        }
    }
    out
}

#[test]
fn party_tables_geometry_cycles_mirrors_timing_and_pixels_match_retail() {
    let Some(r) = retail() else { return };
    let data = r.rom.bytes();
    assert_eq!(
        r.party.iter().map(|p| p.symbol).collect::<Vec<_>>(),
        [
            "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth"
        ]
    );
    for (slot, sprite) in r.party.iter().enumerate() {
        assert_eq!(
            (sprite.slot, sprite.art_offset),
            (slot, 0x290000 + slot * 0x900)
        );
        assert_eq!((PARTY_ART_BYTES, PARTY_ART_TILES), (0x900, 72));
        let long = |at| u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(sprite.art_offset, long(0x5e732 + 4 * slot));
        assert_eq!(sprite.mappings_addr, long(0x5e75e + 4 * slot));
        let routine = r
            .routines
            .iter()
            .find(|p| p.symbol == sprite.symbol)
            .unwrap();
        assert_eq!(routine.art_ptr, Some(sprite.art_offset as u32));
        assert_eq!(routine.mappings_addr, Some(sprite.mappings_addr as u32));
        assert_eq!(routine.sprite_tile_props, Some(0x40));
        assert_eq!(
            sprite.art_sha256,
            crate::png::hex(&Sha256::digest(
                &data[sprite.art_offset..sprite.art_offset + 0x900]
            ))
        );
        let s = &sprite.sheet;
        assert_eq!(
            s.bounds,
            Bounds {
                left: 0,
                top: -2,
                right: 16,
                bottom: 30
            }
        );
        assert_eq!(s.frames.len(), 12);
        assert_eq!(s.sequences.len(), 8);
        assert_eq!(s.palette_line, 2);
        assert_eq!(
            s.mirrors,
            std::collections::BTreeMap::from([(9, 6), (10, 7), (11, 8)])
        );
        for (direction, (facing, name)) in FACINGS.iter().enumerate() {
            let walk = &s.sequences[&format!("walk_{name}")];
            let idle = &s.sequences[&format!("idle_{name}")];
            let base = direction as u8 * 3;
            assert_eq!(walk.frames, [base, base + 1, base, base + 2]);
            assert_eq!(idle.frames, [base]);
            assert_eq!(idle.durations, [walk.durations[0]]);
            let sequence = long(sprite.mappings_addr + usize::from(*facing));
            assert_eq!(walk.sequence_offset, sequence);
            assert_eq!(data[sequence], 4);
            for (n, &frame) in walk.frames.iter().enumerate() {
                assert_eq!(walk.durations[n], u16::from(data[sequence + 1]) + 1);
                assert_eq!(
                    s.frames[usize::from(frame)].pixels,
                    streamed_pixels(data, sprite.art_offset, long(sequence + 2 + n * 4)),
                    "{} {name} frame {n}",
                    sprite.symbol
                );
            }
        }
    }
    assert_eq!(r.party[0].mappings_addr, 0x47e44);
    for (symbol, ticks) in [
        ("Chaz", 11),
        ("Alys", 9),
        ("Hahn", 8),
        ("Rune", 12),
        ("Demi", 5),
        ("Raja", 13),
    ] {
        assert_eq!(
            r.party
                .iter()
                .find(|p| p.symbol == symbol)
                .unwrap()
                .sheet
                .sequences["walk_down"]
                .durations,
            [ticks; 4]
        );
    }
    let left = decode_mapping(data, 0x482a4 + 3 * 8).unwrap();
    let right = decode_mapping(data, 0x482a4 + 9 * 8).unwrap();
    assert_eq!(
        left.pieces[0].tile_word | HFLIP_BIT,
        right.pieces[0].tile_word
    );
}

#[test]
fn palette_binding_and_both_chest_lids_keep_their_retail_geometry() {
    let Some(r) = retail() else { return };
    let rom = r.rom.bytes();
    let palette = pal_init_line_3(rom).unwrap();
    assert_eq!(
        palette,
        gfx::palette_rgb(&gfx::decode_palette(&rom[0x296300..0x296320]).unwrap())
    );
    assert_eq!(
        (palette[0], palette[4], palette[15]),
        ([0, 0, 0], [238, 170, 98], [238; 3])
    );
    let decodes = Decodes::new(rom);
    let normal = chest_sprite(rom, &decodes, &[], false, 0).unwrap();
    let white = chest_sprite(rom, &decodes, &[], true, 0).unwrap();
    for (sheet, bounds, closed, open) in [
        (
            &normal,
            Bounds {
                left: 0,
                top: 8,
                right: 16,
                bottom: 32,
            },
            0x5144a,
            0x51450,
        ),
        (
            &white,
            Bounds {
                left: -4,
                top: 6,
                right: 20,
                bottom: 30,
            },
            0x51456,
            0x5145c,
        ),
    ] {
        assert_eq!(sheet.bounds, bounds);
        assert_eq!(sheet.palette_line, 2);
        assert_eq!(sheet.palette, palette);
        assert_eq!(sheet.sequences["idle_down"].sequence_offset, closed);
        assert_eq!(sheet.sequences["idle_up"].sequence_offset, open);
        assert_eq!(sheet.sequences["idle_down"].durations, [16]);
        assert_ne!(sheet.frames[0], sheet.frames[1]);
    }
    let maps = crate::maps::extract_maps(rom, &decodes).unwrap();
    let map = maps
        .real()
        .find(|m| m.general_var == 0x32 && !m.treasure_chests.entries.is_empty())
        .unwrap();
    let map_palette = map_palette_rgb(rom, map.palette.pointer).unwrap();
    let special = chest_sprite(rom, &decodes, &map_palette, false, 0x32).unwrap();
    assert_eq!(special.palette_line, 1);
    assert_eq!(special.palette, map_palette[16..32]);
    assert_ne!(special.frames[0], normal.frames[0]);
}

#[test]
fn grave_slide_chain_and_walk_speed_records_match_retail() {
    let Some(r) = retail() else { return };
    let rom = r.rom.bytes();
    let wanted = table_sequences(rom, 0x4df1c, 4).unwrap();
    assert_eq!(
        wanted[4..].iter().map(|(_, p)| *p).collect::<Vec<_>>(),
        [0x4e5d0, 0x4e5e6]
    );
    for (n, pointers) in [
        vec![0x4f844, 0x4f884, 0x4f8dc, 0x4f928],
        vec![0x4f864, 0x4f8b0, 0x4f902, 0x4f954],
    ]
    .into_iter()
    .enumerate()
    {
        let sequence = decode_sequence(rom, wanted[n + 4].1).unwrap();
        assert_eq!(
            sequence.frames.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            pointers
        );
        assert_eq!(
            sequence
                .frames
                .iter()
                .map(|(_, d)| u16::from(*d) + 1)
                .collect::<Vec<_>>(),
            [5, 5, 5, 53]
        );
    }
    let timing = serde_json::Value::from(&crate::maps::manifest::step_timing_json(rom).unwrap());
    for (block, (frames, speed)) in [(16, 1), (8, 2), (4, 4)].into_iter().enumerate() {
        assert_eq!(timing["blocks"][block]["frames_per_cell"], frames);
        assert_eq!(timing["blocks"][block]["pixels_per_frame"], speed);
        assert_eq!(
            timing["blocks"][block]["step_constant"],
            format!("0x{:04X}", speed * 256)
        );
    }
    // FieldObj_MovementsTbl and the US mask (the reference fork uses #7).
    assert_eq!(&rom[0x477b8..0x477bc], &[2, 0x41, 0, 3]);
    assert_eq!(&rom[0x47ab8..0x47ac0], &[0, 16, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&rom[0x47aa8..0x47ab0], &[0, 0, 0, 0, 0, 0, 255, 0]);
}
