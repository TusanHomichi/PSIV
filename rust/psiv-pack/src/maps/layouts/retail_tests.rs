//! Local retail checks: no cartridge images, text or digest fixtures (#553).

use super::*;
use crate::maps::{
    extract_maps,
    records::{FIELD_MAP_PTRS, be32},
};
use std::sync::OnceLock;

pub(in crate::maps) struct Retail {
    pub rom: crate::Rom,
    pub maps: Vec<MapLayout>,
    pub records: Vec<MapRecord>,
}

pub(in crate::maps) fn retail() -> Option<&'static Retail> {
    static INPUT: OnceLock<Option<Retail>> = OnceLock::new();
    INPUT
        .get_or_init(|| {
            let rom = crate::test_support::retail_rom("layout and drawing tests")?;
            let decodes = Decodes::new(rom.bytes());
            let records = extract_maps(rom.bytes(), &decodes).unwrap();
            let records: Vec<_> = [0x10, 0x1b, 0x92]
                .into_iter()
                .map(|id| records.real().find(|r| r.id == id).unwrap().clone())
                .collect();
            let maps = records
                .iter()
                .map(|record| {
                    decode_layout_section(rom.bytes(), &decodes, layout_spec(record).unwrap())
                        .unwrap()
                        .0
                })
                .collect();
            Some(Retail { rom, maps, records })
        })
        .as_ref()
}

#[test]
fn hand_walked_maps_keep_dimensions_loaded_chunks_collision_and_palette_contracts() {
    let Some(r) = retail() else { return };
    for (m, (id, pointer, chunks, side, plane)) in r.maps.iter().zip([
        (0x10, 0x11c690, 183, 32, Plane::Bg),
        (0x1b, 0x130084, 256, 32, Plane::Bg),
        (0x92, 0x164ed6, 256, 48, Plane::Fg),
    ]) {
        assert_eq!(
            be32(r.rom.bytes(), FIELD_MAP_PTRS + id * 4) as usize,
            pointer
        );
        assert_eq!(m.chunks.words.len(), chunks);
        assert_eq!((m.fg.width_chunks, m.fg.height_chunks), (side, side));
        assert_eq!((m.bg.width_chunks, m.bg.height_chunks), (side, side));
        assert_eq!(
            (m.collision.width, m.collision.height),
            (side * 2, side * 2)
        );
        assert_eq!(m.spec.collision_plane(), plane);
        assert_eq!(m.spec.collision_plane, u8::from(plane == Plane::Bg));
        // Independent nibble oracle from the four raw high-byte flags, with
        // explicit weights rather than the decoder's quadrant helper.
        let l = m.collision_layout();
        for y in 0..m.collision.height {
            for x in 0..m.collision.width {
                let id = l.cells[(y / 2) * l.width_chunks + x / 2];
                let words = &m.chunks.words[usize::from(id)];
                let at = (y % 2) * 8 + (x % 2) * 2;
                let flag = |i: usize| u8::from(words[i] & 0x4000 != 0);
                let expected = flag(at) + 2 * flag(at + 1) + 4 * flag(at + 4) + 8 * flag(at + 5);
                assert_eq!(m.collision.types[y * m.collision.width + x], expected);
            }
        }
        for l in [&m.fg, &m.bg] {
            assert!(l.cells.iter().all(|&id| usize::from(id) < chunks));
        }
        for words in &m.chunks.words {
            for &word in words {
                assert_eq!(vdp_word(word) & (1 << 14), 0);
                assert!(((vdp_word(word) >> 13) & 3) <= 1);
            }
        }
        let colours = decode_map_palette(r.rom.bytes(), m.spec.palette).unwrap();
        assert_eq!(colours.len(), MAP_PALETTE_BYTES / 2);
        assert!(colours.iter().all(|c| c.raw & !gfx::CRAM_DEFINED_MASK == 0));
        let palette = chunk_palette(r.rom.bytes(), m.spec.palette).unwrap();
        assert_eq!(palette.len(), 32);
        assert_eq!(palette[0], [0, 0, 0]);
    }
}

#[test]
fn piata_blob_boundaries_and_tilesets_reach_the_first_sprite_without_gaps() {
    let Some(r) = retail() else { return };
    let p = &r.maps[0];
    for (blob, (offset, consumed, output)) in p
        .chunks
        .blobs
        .iter()
        .zip([(0x122a90, 0x8e1, 4544), (0x123380, 0x381, 1312)])
    {
        assert_eq!(
            (
                blob.rom_offset,
                blob.compressed_length,
                blob.decompressed_length
            ),
            (offset, consumed, output)
        );
        let end = offset + consumed;
        assert!(r.rom.bytes()[end..(end + 15) & !15].iter().all(|&b| b == 0));
    }
    for (l, (offset, consumed, next)) in [&p.fg, &p.bg].into_iter().zip([
        (0x123710, 0x9d, 0x1237b0),
        (0x1237b0, 0x1d8, p.spec.palette),
    ]) {
        assert_eq!(
            (l.blob.rom_offset, l.blob.compressed_length),
            (offset, consumed)
        );
        assert!((offset + consumed..offset + consumed + 16).contains(&next));
    }
    let sets = &p.patterns.tilesets;
    assert_eq!(
        sets.iter()
            .map(|s| (s.first_tile, s.tile_count))
            .collect::<Vec<_>>(),
        [(0x10, 257), (0x111, 258), (0x213, 189)]
    );
    for pair in sets.windows(2) {
        assert_eq!(pair[0].first_tile + pair[0].tile_count, pair[1].first_tile);
    }
    let end = sets.last().unwrap().first_tile + sets.last().unwrap().tile_count;
    assert_eq!(end, 0x2d0);
    assert_eq!(
        usize::from(crate::m68k::w(r.rom.bytes(), 0x11c6a6).unwrap()),
        end
    );
    assert!(unloaded_patterns(p).unwrap().is_empty());
}

#[test]
fn collision_plane_choice_exposes_the_cave_and_piata_doors_and_shop_counter() {
    let Some(r) = retail() else { return };
    let cave = &r.maps[2];
    let wrong = decode_collision(&cave.chunks, &cave.bg).unwrap();
    let cells = cave.collision.types.len();
    assert_ne!(wrong.types, cave.collision.types);
    assert!(
        cave.collision
            .types
            .iter()
            .filter(|&&v| is_blocking(v))
            .count()
            < cells / 4
    );
    assert!(wrong.types.iter().filter(|&&v| is_blocking(v)).count() > cells * 3 / 4);
    let p = &r.maps[0];
    let record = &r.records[0];
    assert_eq!(record.transitions_2.entries.len(), 6);
    // The record's door coordinates are in 16px cells. The walker probes one
    // cell below; the Academy's range spans two cells (loc_45B0A).
    for t in &record.transitions_2.entries {
        assert_eq!(
            collision_at(
                &p.chunks,
                p.collision_layout(),
                usize::from(t.record.source_x_tile),
                usize::from(t.record.source_y_tile) + 1
            )
            .unwrap(),
            1
        );
    }
    assert_eq!(p.collision.histogram()[1], 7);
    for x in [0x1f, 0x20] {
        assert_eq!(
            collision_at(&p.chunks, p.collision_layout(), x, 7).unwrap(),
            1
        );
    }
    // loc_68394's shop table entry, read locally rather than copied as bytes.
    let shop = &r.maps[1];
    let at = 0x683f4;
    assert_eq!(crate::m68k::w(r.rom.bytes(), at).unwrap(), 0x1b);
    let x = usize::from(crate::m68k::w(r.rom.bytes(), at + 2).unwrap()) / COLLISION_CELL_PIXELS;
    let y = usize::from(crate::m68k::w(r.rom.bytes(), at + 4).unwrap()) / COLLISION_CELL_PIXELS;
    assert_eq!(
        collision_at(&shop.chunks, shop.collision_layout(), x, y + 1).unwrap(),
        0xc
    );
    assert_eq!(collision_type_name(0xc), "shop");
    assert!(is_blocking(0xc));
}
