//! Synthetic layout contracts retained from tests/test_layouts.py (#553).

use super::tests::{kosinski_literals, rom_of};
use super::*;

pub(super) fn layout(cells: Vec<u8>, width: usize, plane: Plane) -> Layout {
    Layout {
        plane,
        height_chunks: cells.len() / width,
        width_chunks: width,
        cells,
        blob: Blob {
            rom_offset: 0,
            compressed_length: 0,
            decompressed_length: 0,
            sha256: String::new(),
        },
        anomaly: None,
    }
}

#[test]
fn chunks_concatenate_big_endian_definitions_and_refuse_invalid_inputs() {
    let first: Vec<_> = [1u16; CHUNK_WORDS * 2]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    let second: Vec<_> = [0x1234u16; CHUNK_WORDS]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    let (rom, at) = rom_of(&[kosinski_literals(&first), kosinski_literals(&second)]);
    let chunks = decode_chunks(&rom, &Decodes::new(&rom), &at).unwrap();
    assert_eq!(chunks.words.len(), 3);
    assert_eq!(chunks.words[2], [0x1234; CHUNK_WORDS]);
    assert_eq!(
        chunks
            .blobs
            .iter()
            .map(|b| b.rom_offset)
            .collect::<Vec<_>>(),
        at
    );
    assert_eq!(chunks.blobs[0].decompressed_length, 2 * CHUNK_BYTES);
    assert!(chunks.get(3).is_err());
    assert!(decode_chunks(&rom, &Decodes::new(&rom), &[]).is_err());
    for (bytes, message) in [
        (CHUNK_BYTES + 2, "whole number"),
        (CHUNK_BYTES * (MAX_CHUNKS + 1), "256"),
    ] {
        let rom = kosinski_literals(&vec![0; bytes]);
        assert!(
            decode_chunks(&rom, &Decodes::new(&rom), &[0])
                .unwrap_err()
                .0
                .contains(message)
        );
    }
}

#[test]
fn a_rectangular_layout_uses_its_width_as_stride() {
    let cells: Vec<_> = (0..48).collect();
    let rom = kosinski_literals(&cells);
    let l = decode_layout(&rom, &Decodes::new(&rom), 0, 8, 6, Plane::Bg).unwrap();
    assert_eq!(l.plane, Plane::Bg);
    assert_eq!((l.width_pixels(), l.height_pixels()), (256, 192));
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(l.chunk_at(x, y), (y * 8 + x) as u8);
        }
    }
    for (w, h) in [(0, 1), (1, 0)] {
        assert!(decode_layout(&rom, &Decodes::new(&rom), 0, w, h, Plane::Fg).is_err());
    }
}

#[test]
fn collision_uses_only_bit_14_and_all_four_weights_in_every_quadrant() {
    // loc_45B0A: top-left, top-right, bottom-left, bottom-right contribute
    // 1, 2, 4, 8. All other pattern-word bits are video data.
    for value in 0..16u8 {
        for qy in 0..2 {
            for qx in 0..2 {
                let mut words = [0xBFFF; CHUNK_WORDS];
                let base = qy * 8 + qx * 2;
                for (bit, step) in [0, 1, 4, 5].into_iter().enumerate() {
                    if value & (1 << bit) != 0 {
                        words[base + step] |= 1 << 14;
                    }
                }
                let cells = chunk_collision_cells(&words);
                let mut expected = [0; 4];
                expected[qy * 2 + qx] = value;
                assert_eq!(cells, expected, "nibble {value}, quadrant {qx},{qy}");
            }
        }
    }
}

#[test]
fn collision_grid_selects_chunks_in_row_order_and_agrees_with_cell_queries() {
    let mut solid = [0; CHUNK_WORDS];
    // Set only the bottom-right tile of each quadrant: collision type 8.
    for i in [5, 7, 13, 15] {
        solid[i] = 1 << 14;
    }
    let chunks = ChunkTable {
        words: vec![[0; CHUNK_WORDS], solid],
        blobs: vec![],
    };
    let l = layout(vec![0, 1, 1, 0, 0, 1], 3, Plane::Fg);
    let grid = decode_collision(&chunks, &l).unwrap();
    assert_eq!((grid.width, grid.height), (6, 4));
    assert_eq!(grid.histogram()[0], 12);
    assert_eq!(grid.histogram()[8], 12);
    for y in 0..4 {
        for x in 0..6 {
            let expected = if l.cells[(y / 2) * 3 + x / 2] == 0 {
                0
            } else {
                8
            };
            assert_eq!(grid.types[y * 6 + x], expected);
            assert_eq!(collision_at(&chunks, &l, x, y).unwrap(), expected);
            assert_eq!(is_blocking(expected), expected == 8);
        }
    }
    assert!(collision_at(&chunks, &l, 6, 0).is_err());
    assert!(collision_at(&chunks, &l, 0, 4).is_err());
    assert!(decode_collision(&chunks, &layout(vec![2], 1, Plane::Fg)).is_err());
    assert_eq!(collision_type_name(1), "map_change");
    assert!(!is_blocking(1));
    assert_eq!(collision_type_name(7), "unnamed_7");
}

#[test]
fn record_dimensions_add_one_and_collision_plane_is_zero_vs_nonzero() {
    let Some(rom) = crate::test_support::retail_rom("layout spec tests") else {
        return;
    };
    let maps = super::super::extract_maps(rom.bytes(), &Decodes::new(rom.bytes())).unwrap();
    let mut record = maps.real().find(|r| r.id == 0x10).unwrap().clone();
    record.dimensions.fg_row_size = 0x1f;
    record.dimensions.fg_column_size = 0x0f;
    record.dimensions.bg_row_size = 0x7f;
    record.dimensions.bg_column_size = 0;
    let spec = layout_spec(&record).unwrap();
    assert_eq!(
        (
            spec.width_chunks_fg,
            spec.height_chunks_fg,
            spec.width_chunks_bg,
            spec.height_chunks_bg
        ),
        (32, 16, 128, 1)
    );
    for (byte, plane) in [
        (0, Plane::Fg),
        (1, Plane::Bg),
        (2, Plane::Bg),
        (255, Plane::Bg),
    ] {
        let mut spec = spec.clone();
        spec.collision_plane = byte;
        assert_eq!(spec.collision_plane(), plane);
    }
    assert!(layout_spec(maps.real().find(|r| r.id == 0).unwrap()).is_err());
}
