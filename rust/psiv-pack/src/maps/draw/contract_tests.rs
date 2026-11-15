//! Pixel and priority contracts retained from tests/test_layouts.py and tests/test_render.py.
use super::tests::{layout, patterns};
use super::*;
use crate::maps::layouts::Plane;

#[test]
fn pattern_fields_separate_collision_priority_palette_and_geometry() {
    assert_eq!(tile_index(0xffff), 0x7ff);
    assert_eq!(collision_flag(0xffff), 1);
    assert!(priority(0xffff));
    assert_eq!(vdp_word(0xffff), 0xbfff);
    assert_eq!(vdp_word(0x4123), 0x0123);
    assert_eq!((CHUNK_WORDS, CHUNK_BYTES, CHUNK_PIXELS), (16, 32, 32));
    assert_eq!(OVERLAY_TRANSPARENT_INDICES, [0, 16]);
    assert_eq!(crate::maps::layouts::COLLISION_CELL_PIXELS, 16);
}

#[test]
fn priority_selection_ignores_collision_and_retains_tile_coordinates() {
    let mut words = [0; CHUNK_WORDS];
    words[1] = 0x8003;
    words[5] = 0x8004;
    let chunks = ChunkTable {
        words: vec![[0x4000; CHUNK_WORDS], words, [0x8000; CHUNK_WORDS]],
        blobs: vec![],
    };
    let selected = priority_tiles(&chunks);
    assert!(selected[0].is_empty());
    assert_eq!(selected[1], [(1, 0, 0x8003), (1, 1, 0x8004)]);
    assert_eq!(selected[2].len(), 16);
}

#[test]
fn composition_places_selected_chunks_on_both_axes_and_flips_both_axes() {
    let mut corner = [0; 64];
    corner[0] = 1;
    let patterns = patterns(&[(0, [0; 64]), (1, corner), (2, [2; 64]), (3, [3; 64])]);
    let mut words = [0; CHUNK_WORDS];
    words[0] = 1 | (1 << 11) | (1 << 12);
    let chunks = ChunkTable {
        words: vec![words, [2; CHUNK_WORDS], [3; CHUNK_WORDS]],
        blobs: vec![],
    };
    let l = layout(Plane::Bg, vec![0, 1, 2, 1, 2, 0], 3);
    let pixels = compose_layout(&chunks, &l, &patterns, None, &mut no_prepared_chunks()).unwrap();
    let width = l.width_pixels();
    assert_eq!(pixels.len(), width * l.height_pixels());
    for cy in 0..2 {
        for cx in 0..3 {
            for y in 0..32 {
                for x in 0..32 {
                    let expected = match l.cells[cy * 3 + cx] {
                        0 => u8::from(x == 7 && y == 7),
                        1 => 2,
                        2 => 3,
                        _ => unreachable!(),
                    };
                    assert_eq!(pixels[(cy * 32 + y) * width + cx * 32 + x], expected);
                }
            }
        }
    }
}

#[test]
fn emitted_base_and_overlay_preserve_priority_order_and_transparency() {
    // Both priority planes overlap. A sparse line-1 FG tile wins where lit,
    // and leaves BG visible for colour zero; nonpriority and collision-only
    // tiles must never leak into the overlay.
    let mut sparse = [0; 64];
    sparse[0] = 2;
    let patterns = patterns(&[(1, [3; 64]), (2, sparse), (3, [4; 64])]);
    let mut bg_words = [3; CHUNK_WORDS];
    bg_words[0] = 0x8001;
    let mut fg_words = [0x4003; CHUNK_WORDS];
    fg_words[0] = 0xa002;
    let chunks = ChunkTable {
        words: vec![bg_words, fg_words],
        blobs: vec![],
    };
    let decoded = MapLayout {
        spec: crate::maps::layouts::MapLayoutSpec {
            chunk_blobs: vec![],
            layout_fg: 0,
            layout_bg: 0,
            width_chunks_fg: 1,
            height_chunks_fg: 1,
            width_chunks_bg: 1,
            height_chunks_bg: 1,
            collision_plane: 0,
            tilesets: vec![],
            palette: 0,
            label: "Synthetic",
        },
        chunks,
        fg: layout(Plane::Fg, vec![1], 1),
        bg: layout(Plane::Bg, vec![0], 1),
        collision: crate::maps::layouts::CollisionGrid {
            width: 2,
            height: 2,
            types: vec![0; 4],
        },
        patterns,
    };
    let palette: Vec<_> = (0..48).map(|i| [i, i, i]).collect();
    let base = render_layout(
        &decoded.chunks,
        &decoded.bg,
        &decoded.fg,
        &decoded.patterns,
        &palette,
    )
    .unwrap();
    let (over, counts) = priority_overlay(&decoded, &palette).unwrap();
    let over = over.unwrap();
    let base = png::decode(&base).unwrap();
    let over = png::decode(&over).unwrap();
    assert_eq!((over.width, over.height), (32, 32));
    assert_eq!((base.width, base.height), (32, 32));
    assert_eq!(base.colour, 3);
    assert_eq!(over.colour, 3);
    assert_eq!(base.plte, over.plte);
    assert_eq!(over.plte, palette[..32].as_flattened());
    assert!(base.trns.is_empty());
    assert_eq!(
        over.trns
            .iter()
            .enumerate()
            .filter_map(|(i, &a)| (a == 0).then_some(i))
            .collect::<Vec<_>>(),
        [0, 16]
    );
    assert_eq!(
        (counts.tiles_bg, counts.tiles_fg, counts.opaque_pixels),
        (1, 1, 64)
    );
    for y in 0..32 {
        for x in 0..32 {
            let expected = if x == 0 && y == 0 {
                18
            } else if x < 8 && y < 8 {
                3
            } else {
                0
            };
            assert_eq!(over.pixels[y * 32 + x], expected);
            assert_eq!(
                base.pixels[y * 32 + x],
                if expected == 0 { 4 } else { expected }
            );
        }
    }
    assert!(!over.pixels.contains(&16));
}

#[test]
fn piata_priority_pixels_match_the_tile_footprint_and_share_the_base_palette() {
    let Some(r) = crate::maps::layouts::retail_tests::retail() else {
        return;
    };
    let p = &r.maps[0];
    let palette = crate::maps::layouts::chunk_palette(r.rom.bytes(), p.spec.palette).unwrap();
    let base = render_layout(&p.chunks, &p.bg, &p.fg, &p.patterns, &palette).unwrap();
    let (over, counts) = priority_overlay(p, &palette).unwrap();
    let over = over.unwrap();
    let base = png::decode(&base).unwrap();
    let over = png::decode(&over).unwrap();
    assert_eq!((base.width, base.height), (1024, 1024));
    assert_eq!((over.width, over.height), (base.width, base.height));
    assert_eq!(base.plte, over.plte);
    assert!(base.trns.is_empty());
    assert_eq!(
        over.trns
            .iter()
            .enumerate()
            .filter_map(|(i, &a)| (a == 0).then_some(i))
            .collect::<Vec<_>>(),
        [0, 16]
    );
    assert_eq!(counts.tiles_fg + counts.tiles_bg, 1068);
    assert_eq!(counts.opaque_pixels, 47854);
    assert_eq!(
        over.pixels.iter().filter(|&&p| p != 0).count(),
        counts.opaque_pixels
    );
    assert!(counts.opaque_pixels < over.width * over.height / 10);
    assert!(!over.pixels.contains(&16));
    // Independently read the raw word for each lit pixel. Do not use the
    // priority selector under test to compute the allowed footprint.
    for (index, &pixel) in over.pixels.iter().enumerate().filter(|(_, p)| **p != 0) {
        let (x, y) = (index % over.width, index / over.width);
        assert!(
            [&p.bg, &p.fg].into_iter().any(|l| {
                let id = l.cells[(y / 32) * l.width_chunks + x / 32];
                let word = p.chunks.words[usize::from(id)][((y % 32) / 8) * 4 + (x % 32) / 8];
                word & 0x8000 != 0
            }),
            "pixel {x},{y}: index {pixel} is outside a priority tile"
        );
    }
    let mut prepared = no_prepared_chunks();
    let pixels = compose_layout(&p.chunks, &p.bg, &p.patterns, None, &mut prepared).unwrap();
    let pixels =
        compose_layout(&p.chunks, &p.fg, &p.patterns, Some(pixels), &mut prepared).unwrap();
    assert_eq!(base.pixels, pixels);
    assert!(
        pixels
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 24
    );
}
