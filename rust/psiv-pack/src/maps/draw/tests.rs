use super::*;
use crate::maps::layouts::{Blob, Plane};

pub(super) fn patterns(tiles: &[(usize, [u8; 64])]) -> TilePatterns {
    let mut all = TilePatterns {
        tiles: vec![[0; 64]; VRAM_TILES],
        loaded: vec![false; VRAM_TILES],
        tilesets: Vec::new(),
    };
    for &(index, tile) in tiles {
        all.tiles[index] = tile;
        all.loaded[index] = true;
    }
    all
}

fn numbered_tile() -> [u8; 64] {
    // Row y, column x holds (3*y + x) % 15 + 1: no colour 0, no symmetry
    // across the flips the test reads.
    let mut tile = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            tile[y * 8 + x] = ((y * 3 + x) % 15 + 1) as u8;
        }
    }
    tile
}

pub(super) fn layout(plane: Plane, cells: Vec<u8>, width_chunks: usize) -> Layout {
    let height_chunks = cells.len() / width_chunks;
    Layout {
        plane,
        width_chunks,
        height_chunks,
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
fn a_word_draws_flipped_on_its_palette_line() {
    let tile = numbered_tile();
    let patterns = patterns(&[(5, tile)]);
    let flip_x: u16 = 1 << 11;
    let flip_y: u16 = 1 << 12;
    let line_1: u16 = 1 << 13;
    let collision: u16 = 1 << 14;
    let drawn = prepare_chunk(
        [
            (0, 0, 5),
            (1, 0, 5 | flip_x),
            (2, 0, 5 | flip_y),
            (3, 3, 5 | line_1 | collision),
        ],
        &patterns,
    );
    let at = |tile_x: usize, tile_y: usize, x: usize, y: usize| {
        drawn[(tile_y * 8 + y) * CHUNK_PIXELS + tile_x * 8 + x]
    };
    for y in 0..8 {
        for x in 0..8 {
            let colour = tile[y * 8 + x];
            assert_eq!(at(0, 0, x, y), colour);
            assert_eq!(at(1, 0, 7 - x, y), colour);
            assert_eq!(at(2, 0, x, 7 - y), colour);
            // The collision bit is not a palette bit.
            assert_eq!(at(3, 3, x, y), colour + 16);
            // Nothing was drawn at this tile.
            assert_eq!(at(1, 1, x, y), 0);
        }
    }
}

#[test]
fn an_unloaded_pattern_draws_nothing() {
    let drawn = prepare_chunk([(0, 0, 9)], &patterns(&[]));
    assert!(drawn.iter().all(|&p| p == 0));
}

#[test]
fn plane_a_leaves_colour_0_of_either_line_transparent() {
    // Chunk 0 is solid colour 3; chunk 1 has one lit pixel and colour
    // zero elsewhere, tested on each reachable palette line.
    let mut sparse = [0u8; 64];
    sparse[0] = 2;
    for line in [0, 1u16] {
        let patterns = patterns(&[(1, [3; 64]), (2, sparse)]);
        let chunks = ChunkTable {
            words: vec![[1; CHUNK_WORDS], [2 | (line << 13); CHUNK_WORDS]],
            blobs: Vec::new(),
        };
        let mut prepared = no_prepared_chunks();
        let bg = layout(Plane::Bg, vec![0], 1);
        let fg = layout(Plane::Fg, vec![1], 1);
        let base = compose_layout(&chunks, &bg, &patterns, None, &mut prepared).unwrap();
        assert!(base.iter().all(|&p| p == 3));
        let both = compose_layout(&chunks, &fg, &patterns, Some(base), &mut prepared).unwrap();
        assert_eq!(both[0], 2 + line as u8 * 16);
        assert_eq!(both[1], 3, "colour 0 shows plane B through");
        // Drawn opaque, the same chunk keeps its palette-line colour zero.
        let mut fresh = no_prepared_chunks();
        let alone = compose_layout(&chunks, &fg, &patterns, None, &mut fresh).unwrap();
        assert_eq!((alone[0], alone[1]), (2 + line as u8 * 16, line as u8 * 16));
        let error =
            compose_layout(&chunks, &fg, &patterns, Some(vec![0; 3]), &mut fresh).unwrap_err();
        assert!(error.0.starts_with("Base image is 3 bytes"), "{}", error.0);
    }
}

#[test]
fn the_overlay_holds_only_priority_tiles_and_is_none_without_them() {
    let tile = numbered_tile();
    let patterns = patterns(&[(1, tile)]);
    let priority_bit: u16 = 1 << 15;
    let mut with_priority = [1u16; CHUNK_WORDS];
    with_priority[0] = 1 | priority_bit;
    let spec = crate::maps::layouts::MapLayoutSpec {
        chunk_blobs: Vec::new(),
        layout_fg: 0,
        layout_bg: 0,
        width_chunks_fg: 1,
        height_chunks_fg: 1,
        width_chunks_bg: 1,
        height_chunks_bg: 1,
        collision_plane: 0,
        tilesets: Vec::new(),
        palette: 0,
        label: "Test",
    };
    let chunks = ChunkTable {
        words: vec![with_priority],
        blobs: Vec::new(),
    };
    let grid = crate::maps::layouts::CollisionGrid {
        width: 2,
        height: 2,
        types: vec![0; 4],
    };
    let mut decoded = MapLayout {
        spec,
        chunks,
        fg: layout(Plane::Fg, vec![0], 1),
        bg: layout(Plane::Bg, vec![0], 1),
        collision: grid,
        patterns,
    };
    let palette: Vec<[u8; 3]> = (0..48).map(|i| [i, i, i]).collect();
    let (image, counts) = priority_overlay(&decoded, &palette).unwrap();
    // One tile of the chunk, once per plane, every pixel opaque.
    assert_eq!(
        counts,
        PriorityCounts {
            tiles_fg: 1,
            tiles_bg: 1,
            opaque_pixels: 64,
            chunks_with_priority_tiles: 1,
        }
    );
    assert!(image.is_some());

    // No priority bit anywhere: no file.
    decoded.chunks.words[0][0] = 1;
    let (image, counts) = priority_overlay(&decoded, &palette).unwrap();
    assert!(image.is_none());
    assert_eq!(counts.opaque_pixels, 0);

    // A priority tile the map never loaded is counted but draws nothing.
    decoded.chunks.words[0][0] = 7 | priority_bit;
    let (image, counts) = priority_overlay(&decoded, &palette).unwrap();
    assert!(image.is_none());
    assert_eq!((counts.tiles_fg, counts.opaque_pixels), (1, 0));

    // Planes of different sizes cannot line up.
    decoded.fg = layout(Plane::Fg, vec![0, 0], 2);
    let error = priority_overlay(&decoded, &palette).unwrap_err();
    assert!(error.0.starts_with("map planes are"), "{}", error.0);
}

#[test]
fn the_render_refuses_planes_that_do_not_line_up() {
    let chunks = ChunkTable {
        words: vec![[0; CHUNK_WORDS]],
        blobs: Vec::new(),
    };
    let error = render_layout(
        &chunks,
        &layout(Plane::Bg, vec![0], 1),
        &layout(Plane::Fg, vec![0, 0], 2),
        &patterns(&[]),
        &[[0; 3]; 32],
    )
    .unwrap_err();
    assert!(error.0.starts_with("Overlay plane is 2x1"), "{}", error.0);
}
