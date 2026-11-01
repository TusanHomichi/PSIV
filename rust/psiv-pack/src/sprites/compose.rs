//! Turning mapping records into pixels: `psiv_tools/sprites/compose.py`. Mega
//! Drive sprite behaviour (column-major patterns, whole-piece flips, colour 0
//! transparent), and folding a facing's frames into a deduplicated sheet.

use std::collections::{BTreeMap, HashMap};

use sha2::{Digest, Sha256};

use super::records::{AnimationSequence, HFLIP_BIT, Mapping, Piece, TILE_INDEX_MASK, VFLIP_BIT};
use crate::PackError;
use crate::gfx::{self, TILE_SIZE};
use crate::json::Json;
use crate::png::hex;

/// A bank of 8x8 palette-index tiles with holes in it: VRAM is the model, and
/// an unfilled pattern is a hole rather than a guess.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TileSource {
    tiles: HashMap<usize, [u8; 64]>,
}

impl TileSource {
    pub fn add(&mut self, first: usize, data: &[u8]) -> Result<(), PackError> {
        if !data.len().is_multiple_of(TILE_SIZE) {
            return Err(PackError(format!(
                "art at tile {first} is {} bytes, not a whole number of patterns",
                data.len()
            )));
        }
        for (index, pattern) in data.as_chunks::<TILE_SIZE>().0.iter().enumerate() {
            self.tiles.insert(first + index, gfx::decode_tile(pattern)?);
        }
        Ok(())
    }

    pub fn insert(&mut self, index: usize, tile: [u8; 64]) {
        self.tiles.insert(index, tile);
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

/// One composed frame, palette-indexed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn width(&self) -> usize {
        (self.right - self.left) as usize
    }

    pub fn height(&self) -> usize {
        (self.bottom - self.top) as usize
    }
}

/// `mapping_box`.
pub fn mapping_box(mapping: &Mapping) -> Bounds {
    let edges = |piece: &Piece| {
        let (x, y) = (i32::from(piece.x), i32::from(piece.y));
        Bounds {
            left: x,
            top: y,
            right: x + piece.width_tiles() as i32 * 8,
            bottom: y + piece.height_tiles() as i32 * 8,
        }
    };
    let first = edges(&mapping.pieces[0]);
    mapping.pieces[1..].iter().map(edges).fold(first, union)
}

fn union(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }
}

/// `union_box`.
pub fn union_box(boxes: impl IntoIterator<Item = Bounds>) -> Result<Bounds, PackError> {
    boxes
        .into_iter()
        .reduce(union)
        .ok_or_else(|| PackError("cannot take the union of no boxes".into()))
}

/// `effective_tile_word`: `Field_FillSpriteAttributes`' arithmetic. High bytes
/// add, `$13(a4)` is OR-ed in, then the low bytes add and carry back.
pub fn effective_tile_word(art_tile: u16, tile_word: u16, tile_props: u8) -> u16 {
    let mut high = ((art_tile >> 8) + (tile_word >> 8)) as u8 | tile_props;
    let low = (art_tile & 0xFF) + (tile_word & 0xFF);
    if low > 0xFF {
        high = high.wrapping_add(1);
    }
    (u16::from(high) << 8) | (low & 0xFF)
}

/// Counters for where retail data is narrower than the format allows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpriteCensus {
    pub frames: usize,
    pub pieces: usize,
    pub hflip_pieces: usize,
    pub vflip_pieces: usize,
    pub vflip_pieces_streamed: usize,
    pub tile_word_carries: usize,
    pub nonzero_mapping_header_bytes: usize,
    pub sequences: usize,
    pub per_frame_duration_sequences: usize,
}

impl SpriteCensus {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("frames", self.frames)
            .with("pieces", self.pieces)
            .with("hflip_pieces", self.hflip_pieces)
            .with("vflip_pieces", self.vflip_pieces)
            .with("vflip_pieces_streamed", self.vflip_pieces_streamed)
            .with("tile_word_carries", self.tile_word_carries)
            .with(
                "nonzero_mapping_header_bytes",
                self.nonzero_mapping_header_bytes,
            )
            .with("sequences", self.sequences)
            .with(
                "per_frame_duration_sequences",
                self.per_frame_duration_sequences,
            )
    }
}

/// How a frame reads its pattern words.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub art_tile: u16,
    pub tile_props: u8,
    /// `loc_44C5E`: the word indexes the object's own art blob and only the
    /// H-flip bit is honoured. Otherwise it is added to `art_tile` and names
    /// VRAM, with both flips.
    pub streamed: bool,
}

/// `compose_frame`: one mapping drawn into `bounds`, with any missing tiles.
pub fn compose_frame(
    mapping: &Mapping,
    source: &TileSource,
    bounds: Bounds,
    reading: Reading,
    mut census: Option<&mut SpriteCensus>,
) -> (Frame, Vec<u16>) {
    let (width, height) = (bounds.width(), bounds.height());
    let mut pixels = vec![0u8; width * height];
    let mut missing = Vec::new();
    if let Some(census) = census.as_deref_mut() {
        census.frames += 1;
        census.pieces += mapping.pieces.len();
        if mapping.header_byte != 0 {
            census.nonzero_mapping_header_bytes += 1;
        }
    }
    for piece in &mapping.pieces {
        let word = if reading.streamed {
            piece.tile_word
        } else {
            effective_tile_word(reading.art_tile, piece.tile_word, reading.tile_props)
        };
        let hflip = word & HFLIP_BIT != 0;
        let vflip = !reading.streamed && word & VFLIP_BIT != 0;
        if let Some(census) = census.as_deref_mut() {
            if reading.streamed {
                if piece.tile_word & VFLIP_BIT != 0 {
                    census.vflip_pieces_streamed += 1;
                }
            } else if (reading.art_tile & 0xFF) + (piece.tile_word & 0xFF) > 0xFF {
                census.tile_word_carries += 1;
            }
            census.hflip_pieces += usize::from(hflip);
            census.vflip_pieces += usize::from(vflip);
        }
        blit_piece(
            &mut pixels,
            bounds,
            piece,
            word & TILE_INDEX_MASK,
            source,
            (hflip, vflip),
            &mut missing,
        );
    }
    (Frame { pixels }, missing)
}

/// `_blit_piece`: patterns run down each column before moving right, and a flip
/// mirrors the whole piece rather than each tile.
fn blit_piece(
    pixels: &mut [u8],
    bounds: Bounds,
    piece: &Piece,
    first_tile: u16,
    source: &TileSource,
    (hflip, vflip): (bool, bool),
    missing: &mut Vec<u16>,
) {
    let (width, height) = (bounds.width() as i32, bounds.height() as i32);
    let (across, down) = (piece.width_tiles(), piece.height_tiles());
    let base_x = i32::from(piece.x) - bounds.left;
    let base_y = i32::from(piece.y) - bounds.top;
    for column in 0..across {
        for row in 0..down {
            let index = (usize::from(first_tile) + column * down + row) as u16 & TILE_INDEX_MASK;
            let Some(tile) = source.tiles.get(&usize::from(index)) else {
                missing.push(index);
                continue;
            };
            let target_column = if hflip { across - 1 - column } else { column } as i32;
            let target_row = if vflip { down - 1 - row } else { row } as i32;
            for y in 0..8usize {
                let source_y = if vflip { 7 - y } else { y };
                let out_y = base_y + target_row * 8 + y as i32;
                if !(0..height).contains(&out_y) {
                    continue;
                }
                for x in 0..8usize {
                    let source_x = if hflip { 7 - x } else { x };
                    let value = tile[source_y * 8 + source_x];
                    if value == 0 {
                        continue;
                    }
                    let out_x = base_x + target_column * 8 + x as i32;
                    if (0..width).contains(&out_x) {
                        pixels[(out_y * width + out_x) as usize] = value;
                    }
                }
            }
        }
    }
}

/// One facing's animation, as frame indices into a sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetSequence {
    pub frames: Vec<u8>,
    pub durations: Vec<u16>,
    pub sequence_offset: usize,
}

/// A set of distinct frames plus the sequences that index them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub frames: Vec<Frame>,
    pub sequences: BTreeMap<String, SheetSequence>,
    pub bounds: Bounds,
    pub palette: Vec<[u8; 3]>,
    pub palette_line: u8,
    pub mirrors: BTreeMap<usize, usize>,
}

impl Sheet {
    /// `identity`: same pixels, geometry, palette and timing, same sheet.
    pub fn identity(&self) -> String {
        let mut digest = Sha256::new();
        for edge in [
            self.bounds.left,
            self.bounds.top,
            self.bounds.right,
            self.bounds.bottom,
        ] {
            digest.update(edge.to_be_bytes());
        }
        digest.update([self.palette_line]);
        for colour in &self.palette {
            digest.update(colour);
        }
        for frame in &self.frames {
            digest.update(&frame.pixels);
        }
        for (name, sequence) in &self.sequences {
            digest.update(name.as_bytes());
            digest.update(&sequence.frames);
            for duration in &sequence.durations {
                digest.update(duration.to_be_bytes());
            }
        }
        hex(&digest.finalize())
    }

    /// `strip`: the frames left to right in one row.
    pub fn strip(&self) -> (usize, usize, Vec<u8>) {
        let (frame_width, height) = (self.bounds.width(), self.bounds.height());
        let width = frame_width * self.frames.len();
        let mut pixels = vec![0u8; width * height];
        for (index, frame) in self.frames.iter().enumerate() {
            for y in 0..height {
                let start = y * width + index * frame_width;
                pixels[start..start + frame_width]
                    .copy_from_slice(&frame.pixels[y * frame_width..(y + 1) * frame_width]);
            }
        }
        (width, height, pixels)
    }
}

/// `(name, sequence, frames)`, in emission order.
pub type Composed = (String, AnimationSequence, Vec<Frame>);

/// `build_sheet`: identical frames collapse, each entry becomes a `walk_<name>`
/// of the whole cycle and an `idle_<name>` of its first frame, and a frame that
/// is another's exact reflection is recorded in `mirrors`.
pub fn build_sheet(
    composed: Vec<Composed>,
    bounds: Bounds,
    palette: &[[u8; 3]],
    palette_line: u8,
) -> Sheet {
    let mut frames: Vec<Frame> = Vec::new();
    let mut by_pixels: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut sequences = BTreeMap::new();
    for (name, sequence, composed_frames) in composed {
        let mut indices = Vec::with_capacity(composed_frames.len());
        for frame in composed_frames {
            let index = *by_pixels.entry(frame.pixels.clone()).or_insert_with(|| {
                frames.push(frame);
                frames.len() - 1
            });
            indices.push(index as u8);
        }
        let durations: Vec<u16> = sequence
            .frames
            .iter()
            .map(|(_, duration)| u16::from(*duration) + 1)
            .collect();
        sequences.insert(
            format!("idle_{name}"),
            SheetSequence {
                frames: vec![indices[0]],
                durations: vec![durations[0]],
                sequence_offset: sequence.rom_offset,
            },
        );
        sequences.insert(
            format!("walk_{name}"),
            SheetSequence {
                frames: indices,
                durations,
                sequence_offset: sequence.rom_offset,
            },
        );
    }
    let frame_width = bounds.width();
    let mut mirrors = BTreeMap::new();
    for (index, frame) in frames.iter().enumerate() {
        let mut reflected = frame.pixels.clone();
        for row in reflected.chunks_mut(frame_width.max(1)) {
            row.reverse();
        }
        if let Some(other) = frames[..index].iter().position(|f| f.pixels == reflected) {
            mirrors.insert(index, other);
        }
    }
    Sheet {
        frames,
        sequences,
        bounds,
        palette: palette.to_vec(),
        palette_line,
        mirrors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(x: i8, y: i8, size_byte: u8, tile_word: u16) -> Piece {
        Piece {
            y,
            size_byte,
            tile_word,
            x,
        }
    }

    fn numbered() -> TileSource {
        // Tile n is solid colour n + 1.
        let mut source = TileSource::default();
        for n in 0..8u8 {
            source.insert(usize::from(n), [n + 1; 64]);
        }
        source
    }

    const NAMED: Reading = Reading {
        art_tile: 0,
        tile_props: 0,
        streamed: false,
    };

    #[test]
    fn the_carry_goes_back_into_the_high_byte() {
        assert_eq!(effective_tile_word(0x0123, 0x0456, 0x20), 0x2579);
        assert_eq!(effective_tile_word(0x00FF, 0x0001, 0), 0x0100);
        // The OR comes before the carry, so the carry can flip a palette bit.
        assert_eq!(effective_tile_word(0x00FF, 0x1F01, 0x20), 0x4000);
    }

    #[test]
    fn patterns_run_down_each_column_and_a_flip_mirrors_the_whole_piece() {
        let mapping = |word| Mapping {
            header_byte: 0,
            pieces: vec![piece(0, 0, 0b0101, word)],
        };
        let bounds = mapping_box(&mapping(0));
        assert_eq!((bounds.width(), bounds.height()), (16, 16));
        let corners = |word| {
            let (frame, missing) = compose_frame(&mapping(word), &numbered(), bounds, NAMED, None);
            assert!(missing.is_empty());
            let at = |x: usize, y: usize| frame.pixels[y * 16 + x];
            [at(0, 0), at(0, 8), at(8, 0), at(8, 8)]
        };
        assert_eq!(corners(0), [1, 2, 3, 4]);
        assert_eq!(corners(HFLIP_BIT), [3, 4, 1, 2]);
        assert_eq!(corners(VFLIP_BIT), [2, 1, 4, 3]);
    }

    #[test]
    fn a_streamed_word_ignores_the_v_flip_and_art_tile() {
        let mapping = Mapping {
            header_byte: 0,
            pieces: vec![piece(0, 0, 0b0001, VFLIP_BIT)],
        };
        let reading = Reading {
            art_tile: 5,
            tile_props: 0,
            streamed: true,
        };
        let mut census = SpriteCensus::default();
        let bounds = mapping_box(&mapping);
        let (frame, _) = compose_frame(&mapping, &numbered(), bounds, reading, Some(&mut census));
        assert_eq!((frame.pixels[0], frame.pixels[8 * 8]), (1, 2));
        assert_eq!((census.vflip_pieces, census.vflip_pieces_streamed), (0, 1));
    }

    #[test]
    fn a_hole_is_reported_and_colour_0_shows_through() {
        let mapping = Mapping {
            header_byte: 3,
            pieces: vec![piece(0, 0, 0, 1), piece(0, 0, 0, 0x7FF)],
        };
        let mut source = TileSource::default();
        source.insert(1, [0; 64]);
        let mut census = SpriteCensus::default();
        let bounds = mapping_box(&mapping);
        let (frame, missing) = compose_frame(&mapping, &source, bounds, NAMED, Some(&mut census));
        assert_eq!(missing, [0x7FF]);
        assert!(frame.pixels.iter().all(|&p| p == 0));
        assert_eq!(census.nonzero_mapping_header_bytes, 1);
    }

    #[test]
    fn identical_frames_collapse_and_a_reflection_is_a_mirror() {
        let bounds = Bounds {
            left: 0,
            top: 0,
            right: 2,
            bottom: 1,
        };
        let frame = |a, b| Frame { pixels: vec![a, b] };
        let sequence = AnimationSequence {
            rom_offset: 9,
            per_frame_durations: true,
            frames: vec![(0, 0), (0, 4), (0, 1)],
        };
        let sheet = build_sheet(
            vec![(
                "down".into(),
                sequence,
                vec![frame(1, 2), frame(1, 2), frame(2, 1)],
            )],
            bounds,
            &[[0; 3]; 16],
            2,
        );
        assert_eq!(sheet.frames.len(), 2);
        assert_eq!(sheet.mirrors, BTreeMap::from([(1, 0)]));
        assert_eq!(sheet.sequences["walk_down"].frames, [0, 0, 1]);
        assert_eq!(sheet.sequences["walk_down"].durations, [1, 5, 2]);
        assert_eq!(sheet.sequences["idle_down"].frames, [0]);
        assert_eq!(sheet.strip(), (4, 1, vec![1, 2, 2, 1]));
    }

    #[test]
    fn the_identity_covers_the_palette_and_the_timing() {
        let bounds = Bounds {
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
        };
        let sequence = |duration| AnimationSequence {
            rom_offset: 0,
            per_frame_durations: false,
            frames: vec![(0, duration)],
        };
        let sheet = |palette: [u8; 3], duration| {
            build_sheet(
                vec![(
                    "down".into(),
                    sequence(duration),
                    vec![Frame { pixels: vec![1] }],
                )],
                bounds,
                &[palette],
                0,
            )
            .identity()
        };
        assert_eq!(sheet([1, 2, 3], 0), sheet([1, 2, 3], 0));
        assert_ne!(sheet([1, 2, 3], 0), sheet([1, 2, 4], 0));
        assert_ne!(sheet([1, 2, 3], 0), sheet([1, 2, 3], 1));
    }
}
