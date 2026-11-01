//! How a chunk's pattern-name words become pixels, and the two map renders
//! made of them: the port of `psiv_tools/chunk_draw.py`, the rendering half of
//! `layouts.py` and `psiv_tools/render.py`, which own the retail citations
//! (#216, slice 3).
//!
//! A pixel is `palette_line * 16 + colour_index`. A tile's indices come from
//! `gfx::decode_tile`, so they are below 16, and a pixel's low nybble says
//! whether its colour is 0: that is the transparency test of a compositing
//! draw, with no separate mask.

use super::layouts::{ChunkTable, Layout, MAX_CHUNKS, MapLayout, TilePatterns};
use crate::PackError;
use crate::json::Json;
use crate::png;

/// `ChunkTilesToBuffer`'s four cells across and four rows down.
pub const CHUNK_TILES: usize = 4;
pub const CHUNK_WORDS: usize = CHUNK_TILES * CHUNK_TILES;
pub const CHUNK_BYTES: usize = CHUNK_WORDS * 2;
pub const TILE_PIXELS: usize = 8;
pub const CHUNK_PIXELS: usize = CHUNK_TILES * TILE_PIXELS;
/// The 11-bit pattern index.
pub const VRAM_TILES: usize = 0x800;

const TILE_INDEX_MASK: u16 = 0x07FF;
const H_FLIP_BIT: u16 = 11;
const V_FLIP_BIT: u16 = 12;
const PALETTE_BIT: u16 = 13;
const COLLISION_BIT: u16 = 14;
const PRIORITY_BIT: u16 = 15;
/// `ChunkTilesToBuffer` / `ChunkTilesToVRAM`: the collision bit never reaches
/// the VDP.
const VDP_WORD_MASK: u16 = 0xBFFF;
/// Pixel values are `palette_line * 16 + colour_index`, so colour 0 of CRAM
/// lines 0 and 1 are indices 0 and 16.
pub const OVERLAY_TRANSPARENT_INDICES: [u8; 2] = [0, 16];

pub fn tile_index(word: u16) -> u16 {
    word & TILE_INDEX_MASK
}

pub fn collision_flag(word: u16) -> u8 {
    ((word >> COLLISION_BIT) & 1) as u8
}

pub fn vdp_word(word: u16) -> u16 {
    word & VDP_WORD_MASK
}

pub fn priority(word: u16) -> bool {
    word >> PRIORITY_BIT & 1 != 0
}

/// One chunk's tiles drawn into a 32x32 block: every pixel of a loaded tile at
/// `palette_line * 16 + colour_index`, colour 0 included, and 0 where no tile
/// was drawn.
pub type PreparedChunk = [u8; CHUNK_PIXELS * CHUNK_PIXELS];

/// Chunk drawings by chunk id, for the planes of one map: they index the same
/// chunk table and patterns.
pub type Prepared = Vec<Option<Box<PreparedChunk>>>;

pub fn no_prepared_chunks() -> Prepared {
    vec![None; MAX_CHUNKS]
}

/// `prepare_chunk`: draw `(tile_x, tile_y, word)` entries the way the VDP draws
/// each word. A pattern the map never loaded draws nothing.
pub fn prepare_chunk(
    entries: impl IntoIterator<Item = (usize, usize, u16)>,
    patterns: &TilePatterns,
) -> Box<PreparedChunk> {
    let mut pixels: Box<PreparedChunk> = Box::new([0; CHUNK_PIXELS * CHUNK_PIXELS]);
    for (tile_x, tile_y, raw) in entries {
        let word = vdp_word(raw);
        let index = tile_index(word);
        if !patterns.is_loaded(index) {
            continue;
        }
        let tile = &patterns.tiles[usize::from(index)];
        let shift = ((word >> PALETTE_BIT) & 1) as u8 * 16;
        let flip_x = word >> H_FLIP_BIT & 1 != 0;
        let flip_y = word >> V_FLIP_BIT & 1 != 0;
        for y in 0..TILE_PIXELS {
            let source_y = if flip_y { TILE_PIXELS - 1 - y } else { y };
            let row = &tile[source_y * TILE_PIXELS..(source_y + 1) * TILE_PIXELS];
            let start = (tile_y * TILE_PIXELS + y) * CHUNK_PIXELS + tile_x * TILE_PIXELS;
            for (x, out) in pixels[start..start + TILE_PIXELS].iter_mut().enumerate() {
                let source_x = if flip_x { TILE_PIXELS - 1 - x } else { x };
                *out = row[source_x] + shift;
            }
        }
    }
    pixels
}

/// `overlay_chunk`: composite `chunk` into the plane cell; colour 0 shows
/// through.
fn overlay_chunk(
    pixels: &mut [u8],
    width: usize,
    chunk_x: usize,
    chunk_y: usize,
    chunk: &PreparedChunk,
) {
    let origin = chunk_y * CHUNK_PIXELS * width + chunk_x * CHUNK_PIXELS;
    for (y, row) in chunk.as_chunks::<CHUNK_PIXELS>().0.iter().enumerate() {
        let start = origin + y * width;
        for (out, &pixel) in pixels[start..start + CHUNK_PIXELS].iter_mut().zip(row) {
            if pixel & 0x0F != 0 {
                *out = pixel;
            }
        }
    }
}

fn whole_chunk(
    chunks: &ChunkTable,
    chunk_id: u8,
    patterns: &TilePatterns,
) -> Result<Box<PreparedChunk>, PackError> {
    let words = chunks.get(chunk_id)?;
    Ok(prepare_chunk(
        words
            .iter()
            .enumerate()
            .map(|(i, &word)| (i % CHUNK_TILES, i / CHUNK_TILES, word)),
        patterns,
    ))
}

/// `compose_layout`: draw one plane into an index buffer; with `base` given,
/// composite over it the way the VDP overlays plane A on plane B.
pub fn compose_layout(
    chunks: &ChunkTable,
    layout: &Layout,
    patterns: &TilePatterns,
    base: Option<Vec<u8>>,
    prepared: &mut Prepared,
) -> Result<Vec<u8>, PackError> {
    let (width, height) = (layout.width_pixels(), layout.height_pixels());
    if let Some(base) = &base
        && base.len() != width * height
    {
        return Err(PackError(format!(
            "Base image is {} bytes, expected {} for a {width}x{height} plane",
            base.len(),
            width * height
        )));
    }
    let compositing = base.is_some();
    let mut pixels = base.unwrap_or_else(|| vec![0; width * height]);
    for chunk_y in 0..layout.height_chunks {
        for chunk_x in 0..layout.width_chunks {
            let chunk_id = layout.chunk_at(chunk_x, chunk_y);
            let slot = &mut prepared[usize::from(chunk_id)];
            if slot.is_none() {
                *slot = Some(whole_chunk(chunks, chunk_id, patterns)?);
            }
            let Some(chunk) = slot.as_deref() else {
                continue;
            };
            if compositing {
                overlay_chunk(&mut pixels, width, chunk_x, chunk_y, chunk);
            } else {
                let origin = chunk_y * CHUNK_PIXELS * width + chunk_x * CHUNK_PIXELS;
                for (y, row) in chunk.as_chunks::<CHUNK_PIXELS>().0.iter().enumerate() {
                    let start = origin + y * width;
                    pixels[start..start + CHUNK_PIXELS].copy_from_slice(row);
                }
            }
        }
    }
    Ok(pixels)
}

fn line_colours(palette: &[[u8; 3]]) -> &[[u8; 3]] {
    &palette[..palette.len().min(32)]
}

/// `render_layout`: plane `layout` with plane `overlay` composited over it, as
/// an indexed PNG. Plane A is always in front of plane B, so pass the BG as
/// `layout` and the FG as `overlay`.
pub fn render_layout(
    chunks: &ChunkTable,
    layout: &Layout,
    overlay: &Layout,
    patterns: &TilePatterns,
    palette: &[[u8; 3]],
) -> Result<Vec<u8>, PackError> {
    let mut prepared = no_prepared_chunks();
    let pixels = compose_layout(chunks, layout, patterns, None, &mut prepared)?;
    if (overlay.width_chunks, overlay.height_chunks) != (layout.width_chunks, layout.height_chunks)
    {
        return Err(PackError(format!(
            "Overlay plane is {}x{} chunks but the base plane is {}x{}",
            overlay.width_chunks, overlay.height_chunks, layout.width_chunks, layout.height_chunks
        )));
    }
    let pixels = compose_layout(chunks, overlay, patterns, Some(pixels), &mut prepared)?;
    Ok(png::encode_indexed(
        layout.width_pixels() as u32,
        layout.height_pixels() as u32,
        &pixels,
        line_colours(palette),
        &[],
    )?)
}

/// What went into a priority overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriorityCounts {
    pub tiles_fg: usize,
    pub tiles_bg: usize,
    pub opaque_pixels: usize,
    pub chunks_with_priority_tiles: usize,
}

impl PriorityCounts {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with(
                "tiles",
                Json::object()
                    .with("fg", self.tiles_fg)
                    .with("bg", self.tiles_bg),
            )
            .with("opaque_pixels", self.opaque_pixels)
            .with(
                "chunks_with_priority_tiles",
                self.chunks_with_priority_tiles,
            )
    }
}

type PriorityTiles = Vec<(usize, usize, u16)>;

/// `priority_tiles`: each chunk definition's priority tiles, empty when it has
/// none.
fn priority_tiles(chunks: &ChunkTable) -> Vec<PriorityTiles> {
    chunks
        .words
        .iter()
        .map(|words| {
            words
                .iter()
                .enumerate()
                .filter(|&(_, &word)| priority(word))
                .map(|(i, &word)| (i % CHUNK_TILES, i / CHUNK_TILES, word))
                .collect()
        })
        .collect()
}

/// `_draw_priority_plane`: draw one plane's priority tiles, returning how many
/// it placed. A plane not drawing a priority tile draws nothing at all.
fn draw_priority_plane(
    layout: &Layout,
    tiles_by_chunk: &[PriorityTiles],
    patterns: &TilePatterns,
    pixels: &mut [u8],
    prepared: &mut Prepared,
) -> usize {
    let width = layout.width_pixels();
    let mut placed = 0;
    for chunk_y in 0..layout.height_chunks {
        for chunk_x in 0..layout.width_chunks {
            let chunk_id = usize::from(layout.chunk_at(chunk_x, chunk_y));
            let Some(tiles) = tiles_by_chunk.get(chunk_id).filter(|t| !t.is_empty()) else {
                continue;
            };
            placed += tiles.len();
            let chunk = prepared[chunk_id]
                .get_or_insert_with(|| prepare_chunk(tiles.iter().copied(), patterns));
            overlay_chunk(pixels, width, chunk_x, chunk_y, chunk);
        }
    }
    placed
}

/// `priority_overlay`: the above-sprites layer of one map as a PNG, `None`
/// when the map has no priority pixels, plus what went into it. A renderer
/// draws base, then sprites, then this.
pub fn priority_overlay(
    decoded: &MapLayout,
    palette: &[[u8; 3]],
) -> Result<(Option<Vec<u8>>, PriorityCounts), PackError> {
    let (width, height) = (decoded.bg.width_pixels(), decoded.bg.height_pixels());
    if (decoded.fg.width_pixels(), decoded.fg.height_pixels()) != (width, height) {
        return Err(PackError(format!(
            "map planes are {}x{} and {width}x{height}; an overlay has to line up with the base \
             render",
            decoded.fg.width_pixels(),
            decoded.fg.height_pixels()
        )));
    }
    let tiles_by_chunk = priority_tiles(&decoded.chunks);
    let mut pixels = vec![0u8; width * height];
    // Both planes draw a chunk's same priority tiles, so they share drawings.
    let mut prepared = no_prepared_chunks();
    // Plane B first, plane A over it, as the base render and the VDP do.
    let tiles_bg = draw_priority_plane(
        &decoded.bg,
        &tiles_by_chunk,
        &decoded.patterns,
        &mut pixels,
        &mut prepared,
    );
    let tiles_fg = draw_priority_plane(
        &decoded.fg,
        &tiles_by_chunk,
        &decoded.patterns,
        &mut pixels,
        &mut prepared,
    );
    let counts = PriorityCounts {
        tiles_fg,
        tiles_bg,
        opaque_pixels: pixels.iter().filter(|&&p| p != 0).count(),
        chunks_with_priority_tiles: tiles_by_chunk.iter().filter(|t| !t.is_empty()).count(),
    };
    if counts.opaque_pixels == 0 {
        return Ok((None, counts));
    }
    let image = png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        line_colours(palette),
        &OVERLAY_TRANSPARENT_INDICES,
    )?;
    Ok((Some(image), counts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::layouts::{Blob, Plane};

    fn patterns(tiles: &[(usize, [u8; 64])]) -> TilePatterns {
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
        // Row y, column x holds (y + x) % 15 + 1: no colour 0, no symmetry
        // across the flips the test reads.
        let mut tile = [0u8; 64];
        for y in 0..8 {
            for x in 0..8 {
                tile[y * 8 + x] = ((y * 3 + x) % 15 + 1) as u8;
            }
        }
        tile
    }

    fn layout(plane: Plane, cells: Vec<u8>, width_chunks: usize) -> Layout {
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
        // Chunk 0 is solid colour 3; chunk 1 is colour 0 on line 1 (16) with
        // one pixel of colour 2 on line 1.
        let mut sparse = [0u8; 64];
        sparse[0] = 2;
        let patterns = patterns(&[(1, [3; 64]), (2, sparse)]);
        let chunks = ChunkTable {
            words: vec![[1; CHUNK_WORDS], [2 | (1 << 13); CHUNK_WORDS]],
            blobs: Vec::new(),
        };
        let mut prepared = no_prepared_chunks();
        let bg = layout(Plane::Bg, vec![0], 1);
        let fg = layout(Plane::Fg, vec![1], 1);
        let base = compose_layout(&chunks, &bg, &patterns, None, &mut prepared).unwrap();
        assert!(base.iter().all(|&p| p == 3));
        let both = compose_layout(&chunks, &fg, &patterns, Some(base), &mut prepared).unwrap();
        assert_eq!(both[0], 18);
        assert_eq!(both[1], 3, "colour 0 of line 1 shows plane B through");
        // Drawn opaque, the same chunk keeps its line-1 colour 0.
        let mut fresh = no_prepared_chunks();
        let alone = compose_layout(&chunks, &fg, &patterns, None, &mut fresh).unwrap();
        assert_eq!((alone[0], alone[1]), (18, 16));
        let error =
            compose_layout(&chunks, &fg, &patterns, Some(vec![0; 3]), &mut fresh).unwrap_err();
        assert!(error.0.starts_with("Base image is 3 bytes"), "{}", error.0);
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
}
