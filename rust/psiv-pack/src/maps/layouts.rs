//! Chunks, plane layouts, tile collision and the VRAM image of a map's layout
//! section: the port of `psiv_tools/layouts.py` and `psiv_tools/pack_layouts.py`,
//! which own the retail citations (#216, slice 3).
//!
//! The two world maps store no layout pointers, so [`layout_spec`] refuses
//! them; their paged planes are the overworld port's. The checks that cannot
//! fail in Rust are gone rather than ported: a layout cell is a `u8`, so it
//! cannot name a chunk past [`MAX_CHUNKS`].

use std::collections::BTreeSet;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::draw::{CHUNK_BYTES, CHUNK_PIXELS, CHUNK_TILES, CHUNK_WORDS, TILE_PIXELS, VRAM_TILES};
use super::draw::{collision_flag, tile_index, vdp_word};
use super::records::{MapRecord, hex6};
use crate::PackError;
use crate::decode::{Call, Decoded, Decodes};
use crate::gfx::{self, Color};
use crate::json::Json;
use crate::png::hex;

/// `GetChunkAndCollision` picks a quadrant with bit 1 of the tile coordinate,
/// so one collision cell covers 2x2 tiles.
pub const COLLISION_CELL_TILES: usize = 2;
pub const COLLISION_CELL_PIXELS: usize = COLLISION_CELL_TILES * TILE_PIXELS;
/// Layout cells are single bytes, so a map can name at most 256 chunks.
pub const MAX_CHUNKS: usize = 256;
/// `Map_Layout_FG` to `Map_Layout_BG`.
pub const LAYOUT_RAM_BYTES: usize = 0x1000;
/// Lines 0, 1 and 3: 48 CRAM words.
pub const MAP_PALETTE_BYTES: usize = 96;

/// The name `GetChunkAndCollision`'s annotations give a 4-bit type.
pub fn collision_type_name(value: u8) -> String {
    match value {
        0x0 => "normal".into(),
        0x1 => "map_change".into(),
        0x2 => "recovery".into(),
        0x8 => "solid".into(),
        0x9 => "water".into(),
        0xA => "sand".into(),
        0xB => "ice_block".into(),
        0xC => "shop".into(),
        other => format!("unnamed_{other:X}"),
    }
}

/// `TileCollNormalPtrs`: the types that stop the walker.
pub fn is_blocking(value: u8) -> bool {
    matches!(value, 0x8..=0xC)
}

/// One Kosinski stream, with the provenance a decompressed record lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    pub rom_offset: usize,
    pub compressed_length: usize,
    pub decompressed_length: usize,
    pub sha256: String,
}

impl Blob {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("rom_offset", hex6(self.rom_offset))
            .with("rom_end", hex6(self.rom_offset + self.compressed_length))
            .with("compressed_length", self.compressed_length)
            .with("decompressed_length", self.decompressed_length)
            .with("sha256", self.sha256.as_str())
    }
}

fn decompress_blob(
    rom: &[u8],
    decodes: &Decodes,
    offset: usize,
    what: &str,
) -> Result<(Arc<Decoded>, Blob), PackError> {
    if offset >= rom.len() {
        return Err(PackError(format!(
            "{what} offset 0x{offset:X} is outside the ROM"
        )));
    }
    let decoded = decodes.get(Call::Kosinski { offset })?;
    let consumed = decoded.consumed();
    let blob = Blob {
        rom_offset: offset,
        compressed_length: consumed,
        decompressed_length: decoded.bytes().len(),
        sha256: hex(&Sha256::digest(&rom[offset..offset + consumed])),
    };
    Ok((decoded, blob))
}

/// The concatenated contents of `Chunk_Table` for one map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkTable {
    pub words: Vec<[u16; CHUNK_WORDS]>,
    pub blobs: Vec<Blob>,
}

impl ChunkTable {
    pub fn get(&self, chunk_id: u8) -> Result<&[u16; CHUNK_WORDS], PackError> {
        self.words.get(usize::from(chunk_id)).ok_or_else(|| {
            PackError(format!(
                "Chunk id 0x{chunk_id:02X} is outside the {} chunks this map loads",
                self.words.len()
            ))
        })
    }

    pub fn to_json(&self) -> Json {
        Json::object()
            .with("chunk_count", self.words.len())
            .with("chunk_bytes", CHUNK_BYTES)
            .with("tiles_per_chunk", format!("{CHUNK_TILES}x{CHUNK_TILES}"))
            .with("pixels_per_chunk", format!("{CHUNK_PIXELS}x{CHUNK_PIXELS}"))
            .with(
                "blobs",
                self.blobs.iter().map(Blob::to_json).collect::<Vec<_>>(),
            )
    }
}

/// `decode_chunks`: `offsets` is the list `Map_LoadChunks` walks.
pub fn decode_chunks(
    rom: &[u8],
    decodes: &Decodes,
    offsets: &[usize],
) -> Result<ChunkTable, PackError> {
    if offsets.is_empty() {
        return Err(PackError("A map needs at least one chunk blob".into()));
    }
    let mut raw = Vec::new();
    let mut blobs = Vec::with_capacity(offsets.len());
    for &offset in offsets {
        let (data, blob) = decompress_blob(rom, decodes, offset, "Chunk blob")?;
        raw.extend_from_slice(&data.bytes());
        blobs.push(blob);
    }
    if !raw.len().is_multiple_of(CHUNK_BYTES) {
        return Err(PackError(format!(
            "Chunk data is {} bytes, which is not a whole number of {CHUNK_BYTES}-byte definitions",
            raw.len()
        )));
    }
    let count = raw.len() / CHUNK_BYTES;
    if count > MAX_CHUNKS {
        return Err(PackError(format!(
            "{count} chunk definitions, but layout cells are single bytes so only {MAX_CHUNKS} \
             are reachable"
        )));
    }
    let words = raw
        .as_chunks::<CHUNK_BYTES>()
        .0
        .iter()
        .map(|chunk| {
            let mut words = [0u16; CHUNK_WORDS];
            for (word, &pair) in words.iter_mut().zip(chunk.as_chunks::<2>().0) {
                *word = u16::from_be_bytes(pair);
            }
            words
        })
        .collect();
    Ok(ChunkTable { words, blobs })
}

/// Which plane a layout belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    Fg,
    Bg,
}

impl Plane {
    pub fn name(self) -> &'static str {
        match self {
            Plane::Fg => "fg",
            Plane::Bg => "bg",
        }
    }
}

/// One plane's chunk grid, as decompressed into `Map_Layout_FG`/`BG`.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub plane: Plane,
    pub width_chunks: usize,
    pub height_chunks: usize,
    pub cells: Vec<u8>,
    pub blob: Blob,
    /// Where the blob's length disagreed with the grid in a way the loader
    /// tolerates: `padded` or `short`.
    pub anomaly: Option<Json>,
}

impl Layout {
    /// `SetupChunksFG`: `mulu.w (row_size + 1), d3 / add.w d5, d3`. The caller
    /// stays inside the grid: this is every plane's inner loop.
    pub fn chunk_at(&self, chunk_x: usize, chunk_y: usize) -> u8 {
        self.cells[chunk_y * self.width_chunks + chunk_x]
    }

    pub fn width_pixels(&self) -> usize {
        self.width_chunks * CHUNK_PIXELS
    }

    pub fn height_pixels(&self) -> usize {
        self.height_chunks * CHUNK_PIXELS
    }

    pub fn distinct_chunks(&self) -> BTreeSet<u8> {
        self.cells.iter().copied().collect()
    }

    pub fn to_json(&self) -> Json {
        Json::object()
            .with("plane", self.plane.name())
            .with("width_chunks", self.width_chunks)
            .with("height_chunks", self.height_chunks)
            .with("width_pixels", self.width_pixels())
            .with("height_pixels", self.height_pixels())
            .with("distinct_chunks", self.distinct_chunks().len())
            .with("highest_chunk_id", highest_cell(&self.cells))
            .with("blob", self.blob.to_json())
            .with("sha256", hex(&Sha256::digest(&self.cells)))
    }
}

fn highest_cell(cells: &[u8]) -> u8 {
    cells.iter().copied().max().unwrap_or(0)
}

/// `decode_layout`: the blob a map record points at, fitted to its grid. The
/// loader never looks at how much `KosDecomp` wrote, so a blob may disagree
/// with the grid; a surplus that is not zero still fails, because that means
/// the extent is being read wrong.
pub fn decode_layout(
    rom: &[u8],
    decodes: &Decodes,
    offset: usize,
    width_chunks: usize,
    height_chunks: usize,
    plane: Plane,
) -> Result<Layout, PackError> {
    if width_chunks == 0 || height_chunks == 0 {
        return Err(PackError(format!(
            "Layout dimensions must be positive, got {width_chunks}x{height_chunks}"
        )));
    }
    let expected = width_chunks * height_chunks;
    if expected > LAYOUT_RAM_BYTES {
        return Err(PackError(format!(
            "A {width_chunks}x{height_chunks} layout needs {expected} bytes, more than the \
             {LAYOUT_RAM_BYTES}-byte Map_Layout region holds"
        )));
    }
    let (decoded, blob) = decompress_blob(rom, decodes, offset, "Layout blob")?;
    let data = decoded.bytes();
    let anomaly_of = |kind: &str, effect: String| {
        Json::object()
            .with("plane", plane.name())
            .with("kind", kind)
            .with("blob_offset", hex6(offset))
            .with("blob_bytes", data.len())
            .with("grid_bytes", expected)
            .with("effect", effect)
    };
    let mut anomaly = None;
    let mut cells = data.to_vec();
    if data.len() > expected {
        if data[expected..].iter().any(|&b| b != 0) {
            return Err(PackError(format!(
                "Layout at {} decompresses to {} bytes for a {width_chunks}x{height_chunks} grid \
                 and the surplus is not zero padding; the grid extent is being read wrong",
                hex6(offset),
                data.len()
            )));
        }
        anomaly = Some(anomaly_of(
            "padded",
            "surplus is zero padding no reader can reach".into(),
        ));
        cells.truncate(expected);
    } else if data.len() < expected {
        anomaly = Some(anomaly_of(
            "short",
            format!(
                "{} cells are left holding whatever the previously loaded map wrote to \
                 Map_Layout; zero-filled here",
                expected - data.len()
            ),
        ));
        cells.resize(expected, 0);
    }
    Ok(Layout {
        plane,
        width_chunks,
        height_chunks,
        cells,
        blob,
        anomaly,
    })
}

/// A 4-bit collision type per 16x16-pixel cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollisionGrid {
    pub width: usize,
    pub height: usize,
    pub types: Vec<u8>,
}

impl CollisionGrid {
    /// How many cells hold each type, by type.
    pub fn histogram(&self) -> [usize; 16] {
        let mut counts = [0; 16];
        for &value in &self.types {
            counts[usize::from(value)] += 1;
        }
        counts
    }

    pub fn to_json(&self) -> Json {
        let mut types = Json::object();
        for (value, &cells) in (0u8..).zip(&self.histogram()) {
            if cells != 0 {
                types.insert(
                    format!("0x{value:X}"),
                    Json::object()
                        .with("name", collision_type_name(value))
                        .with("blocking", is_blocking(value))
                        .with("cells", cells),
                );
            }
        }
        Json::object()
            .with("width_cells", self.width)
            .with("height_cells", self.height)
            .with("cell_pixels", COLLISION_CELL_PIXELS)
            .with("types", types)
            .with(
                "walkable_cells",
                self.types.iter().filter(|&&v| !is_blocking(v)).count(),
            )
            .with("sha256", hex(&Sha256::digest(&self.types)))
    }
}

/// `GetChunkAndCollision`: bit 14 of a cell's four tile words, in the order
/// top-left (1), top-right (2), bottom-left (4), bottom-right (8).
fn quadrant_collision(words: &[u16; CHUNK_WORDS], quadrant_x: usize, quadrant_y: usize) -> u8 {
    let base = quadrant_y * CHUNK_TILES * COLLISION_CELL_TILES + quadrant_x * COLLISION_CELL_TILES;
    [0, 1, CHUNK_TILES, CHUNK_TILES + 1]
        .iter()
        .enumerate()
        .fold(0, |value, (bit, step)| {
            value | (collision_flag(words[base + step]) << bit)
        })
}

/// `decode_collision`: which layout to pass is the map's `$FFFFEC24` byte.
pub fn decode_collision(chunks: &ChunkTable, layout: &Layout) -> Result<CollisionGrid, PackError> {
    let width = layout.width_chunks * COLLISION_CELL_TILES;
    let height = layout.height_chunks * COLLISION_CELL_TILES;
    // A chunk's four quadrants, worked out once per chunk id.
    let mut quadrants: [Option<[u8; 4]>; MAX_CHUNKS] = [None; MAX_CHUNKS];
    let mut types = vec![0u8; width * height];
    for chunk_y in 0..layout.height_chunks {
        for chunk_x in 0..layout.width_chunks {
            let chunk_id = layout.chunk_at(chunk_x, chunk_y);
            let cells = match quadrants[usize::from(chunk_id)] {
                Some(cells) => cells,
                None => {
                    let words = chunks.get(chunk_id)?;
                    let cells = [(0, 0), (1, 0), (0, 1), (1, 1)]
                        .map(|(qx, qy)| quadrant_collision(words, qx, qy));
                    quadrants[usize::from(chunk_id)] = Some(cells);
                    cells
                }
            };
            let at = |qx: usize, qy: usize| {
                (chunk_y * COLLISION_CELL_TILES + qy) * width + chunk_x * COLLISION_CELL_TILES + qx
            };
            types[at(0, 0)] = cells[0];
            types[at(1, 0)] = cells[1];
            types[at(0, 1)] = cells[2];
            types[at(1, 1)] = cells[3];
        }
    }
    Ok(CollisionGrid {
        width,
        height,
        types,
    })
}

/// One `loc_519D2` entry: Kosinski patterns landing at a fixed VRAM tile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tileset {
    pub first_tile: usize,
    pub tile_count: usize,
    pub blob: Blob,
}

impl Tileset {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("first_tile", format!("0x{:03X}", self.first_tile))
            .with(
                "end_tile",
                format!("0x{:03X}", self.first_tile + self.tile_count),
            )
            .with("tile_count", self.tile_count)
            .with("blob", self.blob.to_json())
    }
}

/// A VRAM image: 8x8 palette-index tiles, indexed by pattern number. Tiles the
/// map never loads stay blank, and `loaded` says which are real.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilePatterns {
    pub tiles: Vec<[u8; 64]>,
    pub loaded: Vec<bool>,
    pub tilesets: Vec<Tileset>,
}

impl TilePatterns {
    pub fn is_loaded(&self, index: u16) -> bool {
        self.loaded[usize::from(index)]
    }

    pub fn to_json(&self) -> Json {
        Json::object()
            .with(
                "loaded_tiles",
                self.loaded.iter().filter(|&&loaded| loaded).count(),
            )
            .with(
                "tilesets",
                self.tilesets
                    .iter()
                    .map(Tileset::to_json)
                    .collect::<Vec<_>>(),
            )
    }
}

/// `decode_tilesets`: `specs` is the `loc_519D2` list, `(first VRAM tile, ROM
/// offset)`.
pub fn decode_tilesets(
    rom: &[u8],
    decodes: &Decodes,
    specs: &[(usize, usize)],
) -> Result<TilePatterns, PackError> {
    let mut tiles = vec![[0u8; 64]; VRAM_TILES];
    let mut loaded = vec![false; VRAM_TILES];
    let mut tilesets = Vec::with_capacity(specs.len());
    for &(first_tile, offset) in specs {
        if first_tile >= VRAM_TILES {
            return Err(PackError(format!(
                "Tileset VRAM tile 0x{first_tile:X} is outside VRAM"
            )));
        }
        let (decoded, blob) = decompress_blob(rom, decodes, offset, "Tileset")?;
        let data = decoded.bytes();
        if !data.len().is_multiple_of(gfx::TILE_SIZE) {
            return Err(PackError(format!(
                "Tileset at {} decompresses to {} bytes, which is not a whole number of 32-byte \
                 patterns",
                hex6(offset),
                data.len()
            )));
        }
        let count = data.len() / gfx::TILE_SIZE;
        if first_tile + count > VRAM_TILES {
            return Err(PackError(format!(
                "Tileset at {} would run past VRAM: 0x{first_tile:X} + {count} tiles",
                hex6(offset)
            )));
        }
        for (i, pattern) in data.as_chunks::<{ gfx::TILE_SIZE }>().0.iter().enumerate() {
            tiles[first_tile + i] = gfx::decode_tile(pattern)?;
            loaded[first_tile + i] = true;
        }
        tilesets.push(Tileset {
            first_tile,
            tile_count: count,
            blob,
        });
    }
    Ok(TilePatterns {
        tiles,
        loaded,
        tilesets,
    })
}

/// `decode_map_palette`: the 48 CRAM words a map's palette pointer supplies.
pub fn decode_map_palette(rom: &[u8], offset: usize) -> Result<Vec<Color>, PackError> {
    let end = offset + MAP_PALETTE_BYTES;
    if end > rom.len() {
        return Err(PackError(format!(
            "Map palette at {} runs past the end of the ROM",
            hex6(offset)
        )));
    }
    Ok(gfx::decode_palette(&rom[offset..end])?)
}

/// `chunk_palette`: the 32 colours a chunk word can select. Chunk tiles reach
/// CRAM lines 0 and 1 only, because the bit that would select lines 2 and 3 is
/// the collision flag.
pub fn chunk_palette(rom: &[u8], offset: usize) -> Result<Vec<[u8; 3]>, PackError> {
    let mut colours = gfx::palette_rgb(&decode_map_palette(rom, offset)?);
    colours.truncate(32);
    Ok(colours)
}

/// What `GameMode_LoadFieldMap` reads for the layout section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapLayoutSpec {
    pub chunk_blobs: Vec<usize>,
    pub layout_fg: usize,
    pub layout_bg: usize,
    pub width_chunks_fg: usize,
    pub height_chunks_fg: usize,
    pub width_chunks_bg: usize,
    pub height_chunks_bg: usize,
    /// The `$FFFFEC24` byte: 0 reads the FG layout, non-zero the BG layout.
    pub collision_plane: u8,
    pub tilesets: Vec<(usize, usize)>,
    pub palette: usize,
    pub label: &'static str,
}

impl MapLayoutSpec {
    pub fn collision_plane(&self) -> Plane {
        if self.collision_plane != 0 {
            Plane::Bg
        } else {
            Plane::Fg
        }
    }
}

/// `layout_spec`: the pointer-based path only. A world map has no layout
/// section to read, and is refused.
pub fn layout_spec(record: &MapRecord) -> Result<MapLayoutSpec, PackError> {
    let Some((layout_fg, layout_bg)) = record.layout.planes else {
        return Err(PackError(format!(
            "map 0x{:03X} ({}) has no layout section",
            record.id, record.symbol
        )));
    };
    let dimensions = &record.dimensions;
    Ok(MapLayoutSpec {
        chunk_blobs: record.chunks.pointers.clone(),
        layout_fg,
        layout_bg,
        width_chunks_fg: usize::from(dimensions.fg_row_size) + 1,
        height_chunks_fg: usize::from(dimensions.fg_column_size) + 1,
        width_chunks_bg: usize::from(dimensions.bg_row_size) + 1,
        height_chunks_bg: usize::from(dimensions.bg_column_size) + 1,
        collision_plane: record.scroll.mode,
        tilesets: record
            .tilesets
            .entries
            .iter()
            .map(|entry| (usize::from(entry.word), entry.source))
            .collect(),
        palette: record.palette.pointer,
        label: record.symbol,
    })
}

/// The decoded layout section of one field map.
#[derive(Clone, Debug, PartialEq)]
pub struct MapLayout {
    pub spec: MapLayoutSpec,
    pub chunks: ChunkTable,
    pub fg: Layout,
    pub bg: Layout,
    pub collision: CollisionGrid,
    pub patterns: TilePatterns,
}

impl MapLayout {
    pub fn collision_layout(&self) -> &Layout {
        match self.spec.collision_plane() {
            Plane::Bg => &self.bg,
            Plane::Fg => &self.fg,
        }
    }

    pub fn to_json(&self) -> Json {
        Json::object()
            .with("label", self.spec.label)
            .with("collision_plane", self.spec.collision_plane().name())
            .with("collision_plane_byte", self.spec.collision_plane)
            .with("chunks", self.chunks.to_json())
            .with("layout_fg", self.fg.to_json())
            .with("layout_bg", self.bg.to_json())
            .with("collision", self.collision.to_json())
            .with("tilesets", self.patterns.to_json())
            .with("palette_rom_offset", hex6(self.spec.palette))
    }
}

/// `_fill_undefined_chunks`: a layout can name a chunk the map never loads,
/// and the cartridge then reads a `Chunk_Table` slot this map did not write.
/// There is no faithful value, so the pack substitutes the empty definition
/// and records it.
fn fill_undefined_chunks(chunks: &mut ChunkTable, planes: [&Layout; 2]) -> Option<Json> {
    let loaded = chunks.words.len();
    let highest = usize::from(
        planes
            .iter()
            .map(|layout| highest_cell(&layout.cells))
            .max()
            .unwrap_or(0),
    );
    if highest < loaded {
        return None;
    }
    chunks.words.resize(highest + 1, [0; CHUNK_WORDS]);
    let mut cells = Vec::new();
    let mut ids = BTreeSet::new();
    for layout in planes {
        for (index, &value) in layout.cells.iter().enumerate() {
            if usize::from(value) >= loaded {
                let chunk_id = format!("0x{value:02X}");
                ids.insert(chunk_id.clone());
                cells.push(
                    Json::object()
                        .with("plane", layout.plane.name())
                        .with("x", index % layout.width_chunks)
                        .with("y", index / layout.width_chunks)
                        .with("chunk_id", chunk_id),
                );
            }
        }
    }
    Some(
        Json::object()
            .with("kind", "undefined_chunk")
            .with("loaded_chunks", loaded)
            .with("chunk_ids", ids.into_iter().collect::<Vec<_>>())
            .with("cells", cells)
            .with(
                "effect",
                "the cartridge reads a Chunk_Table slot this map never wrote; the pack \
                 substitutes an empty chunk definition",
            ),
    )
}

/// `decode_layout_section`: one map's layout, and the planes' length anomalies.
pub fn decode_layout_section(
    rom: &[u8],
    decodes: &Decodes,
    spec: MapLayoutSpec,
) -> Result<(MapLayout, Vec<Json>), PackError> {
    let mut chunks = decode_chunks(rom, decodes, &spec.chunk_blobs)?;
    let fg = decode_layout(
        rom,
        decodes,
        spec.layout_fg,
        spec.width_chunks_fg,
        spec.height_chunks_fg,
        Plane::Fg,
    )?;
    let bg = decode_layout(
        rom,
        decodes,
        spec.layout_bg,
        spec.width_chunks_bg,
        spec.height_chunks_bg,
        Plane::Bg,
    )?;
    let chunk_anomaly = fill_undefined_chunks(&mut chunks, [&fg, &bg]);
    let anomalies = [fg.anomaly.clone(), bg.anomaly.clone(), chunk_anomaly]
        .into_iter()
        .flatten()
        .collect();
    let collision = decode_collision(
        &chunks,
        match spec.collision_plane() {
            Plane::Bg => &bg,
            Plane::Fg => &fg,
        },
    )?;
    let patterns = decode_tilesets(rom, decodes, &spec.tilesets)?;
    Ok((
        MapLayout {
            spec,
            chunks,
            fg,
            bg,
            collision,
            patterns,
        },
        anomalies,
    ))
}

/// `unloaded_patterns`: VRAM tiles a map's chunks name but its tilesets never
/// fill, sorted.
pub fn unloaded_patterns(decoded: &MapLayout) -> Result<Vec<u16>, PackError> {
    let mut missing = BTreeSet::new();
    for layout in [&decoded.bg, &decoded.fg] {
        for chunk_id in layout.distinct_chunks() {
            for &word in decoded.chunks.get(chunk_id)? {
                let index = tile_index(vdp_word(word));
                if !decoded.patterns.is_loaded(index) {
                    missing.insert(index);
                }
            }
        }
    }
    Ok(missing.into_iter().collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A Kosinski stream of `data` as literals, then the terminator. The
    /// decoder loads the next description field as soon as the last one
    /// empties, before the byte its last bit announces.
    pub(crate) fn kosinski_literals(data: &[u8]) -> Vec<u8> {
        fn push_bit(out: &mut Vec<u8>, field: &mut usize, bits: &mut usize, bit: u8) {
            out[*field + *bits / 8] |= bit << (*bits % 8);
            *bits += 1;
            if *bits == 16 {
                *field = out.len();
                *bits = 0;
                out.extend_from_slice(&[0, 0]);
            }
        }
        let (mut out, mut field, mut bits) = (vec![0, 0], 0, 0);
        for &byte in data {
            push_bit(&mut out, &mut field, &mut bits, 1);
            out.push(byte);
        }
        // 0 1: a full match, whose count of 0 and next byte of 0 end the stream.
        push_bit(&mut out, &mut field, &mut bits, 0);
        push_bit(&mut out, &mut field, &mut bits, 1);
        out.extend_from_slice(&[0x00, 0xF8, 0x00]);
        out
    }

    /// A ROM of `streams` end to end, each stream's offset beside it.
    pub(crate) fn rom_of(streams: &[Vec<u8>]) -> (Vec<u8>, Vec<usize>) {
        let mut rom = Vec::new();
        let mut offsets = Vec::new();
        for stream in streams {
            offsets.push(rom.len());
            rom.extend_from_slice(stream);
        }
        (rom, offsets)
    }

    fn words_bytes(words: &[u16]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_be_bytes()).collect()
    }

    #[test]
    fn the_test_stream_helper_round_trips() {
        for length in [1, 2, 15, 16, 17, 32, 33] {
            let data: Vec<u8> = (0..length).map(|i| (i * 3) as u8).collect();
            let stream = kosinski_literals(&data);
            let (out, consumed) = crate::decode::kosinski::decompress(&stream, 0).unwrap();
            assert_eq!(out, data, "length {length}");
            assert_eq!(consumed, stream.len(), "length {length}");
        }
    }

    #[test]
    fn collision_reads_each_quadrants_four_tiles_in_order() {
        // Top-left quadrant: tile 0 only (1). Top-right: tiles 2 and 3 (1|2).
        // Bottom-left: tile 12 (bit 8 of base 8 + 4 + 0 -> 4). Bottom-right:
        // tile 15 alone (8).
        let mut words = [0u16; CHUNK_WORDS];
        words[0] = 1 << 14;
        words[2] = 1 << 14;
        words[3] = 1 << 14;
        words[12] = 1 << 14;
        words[15] = 1 << 14;
        let chunks = ChunkTable {
            words: vec![words],
            blobs: Vec::new(),
        };
        let layout = Layout {
            plane: Plane::Fg,
            width_chunks: 1,
            height_chunks: 1,
            cells: vec![0],
            blob: Blob {
                rom_offset: 0,
                compressed_length: 0,
                decompressed_length: 0,
                sha256: String::new(),
            },
            anomaly: None,
        };
        let grid = decode_collision(&chunks, &layout).unwrap();
        assert_eq!((grid.width, grid.height), (2, 2));
        assert_eq!(grid.types, [1, 3, 4, 8]);
    }

    #[test]
    fn a_padded_or_short_plane_is_an_anomaly_and_a_dirty_surplus_fails() {
        let (rom, at) = rom_of(&[
            kosinski_literals(&[1, 2, 0, 0, 0, 0]),
            kosinski_literals(&[1, 2]),
            kosinski_literals(&[1, 2, 3, 4, 5]),
        ]);
        let decodes = Decodes::new(&rom);
        let padded = decode_layout(&rom, &decodes, at[0], 2, 2, Plane::Fg).unwrap();
        assert_eq!(padded.cells, [1, 2, 0, 0]);
        let anomaly = padded.anomaly.unwrap();
        assert!(crate::json::dumps(&anomaly, true).contains("\"padded\""));
        let short = decode_layout(&rom, &decodes, at[1], 2, 2, Plane::Bg).unwrap();
        assert_eq!(short.cells, [1, 2, 0, 0]);
        assert!(crate::json::dumps(&short.anomaly.unwrap(), true).contains("2 cells are left"));
        let exact = decode_layout(&rom, &decodes, at[0], 3, 2, Plane::Fg).unwrap();
        assert!(exact.anomaly.is_none());
        let error = decode_layout(&rom, &decodes, at[2], 2, 2, Plane::Fg).unwrap_err();
        assert!(error.0.contains("not zero padding"), "{}", error.0);
        let error = decode_layout(&rom, &decodes, at[0], 65, 64, Plane::Fg).unwrap_err();
        assert!(error.0.contains("Map_Layout region"), "{}", error.0);
    }

    #[test]
    fn a_chunk_past_the_table_gets_the_empty_definition_and_is_recorded() {
        let word = 1u16 << 14;
        let chunk = words_bytes(&[word; CHUNK_WORDS]);
        let (rom, at) = rom_of(&[
            kosinski_literals(&chunk),
            kosinski_literals(&[0, 3]),
            kosinski_literals(&[0, 0]),
            // A 32-byte pattern for tile 1.
            kosinski_literals(&[0x11; 32]),
        ]);
        let decodes = Decodes::new(&rom);
        let spec = MapLayoutSpec {
            chunk_blobs: vec![at[0]],
            layout_fg: at[1],
            layout_bg: at[2],
            width_chunks_fg: 2,
            height_chunks_fg: 1,
            width_chunks_bg: 2,
            height_chunks_bg: 1,
            collision_plane: 0,
            tilesets: vec![(1, at[3])],
            palette: 0,
            label: "Test",
        };
        let (decoded, anomalies) = decode_layout_section(&rom, &decodes, spec).unwrap();
        assert_eq!(decoded.chunks.words.len(), 4);
        assert_eq!(decoded.chunks.words[3], [0; CHUNK_WORDS]);
        let text = crate::json::dumps(&Json::from(anomalies), true);
        assert!(text.contains("\"chunk_id\": \"0x03\""), "{text}");
        assert!(text.contains("\"loaded_chunks\": 1"), "{text}");
        // Tile 0 is named by chunk 0's words (word 1 << 14 strips to tile 0)
        // and nothing loads it.
        assert_eq!(unloaded_patterns(&decoded).unwrap(), [0]);
        // The substituted chunk has no collision bit; the real one has all.
        assert_eq!(&decoded.collision.types[..2], &[0xF, 0xF]);
        assert_eq!(&decoded.collision.types[2..4], &[0, 0]);
    }

    #[test]
    fn the_blob_records_what_the_stream_consumed() {
        let stream = kosinski_literals(&[7, 7]);
        let mut rom = vec![0xAA; 4];
        rom.extend_from_slice(&stream);
        rom.extend_from_slice(&[0xBB; 8]);
        let decodes = Decodes::new(&rom);
        let (_, blob) = decompress_blob(&rom, &decodes, 4, "Test").unwrap();
        assert_eq!(blob.compressed_length, stream.len());
        assert_eq!(blob.decompressed_length, 2);
        assert_eq!(blob.sha256, hex(&Sha256::digest(&stream)));
        let error = decompress_blob(&rom, &decodes, rom.len(), "Test").unwrap_err();
        assert!(error.0.starts_with("Test offset 0x"), "{}", error.0);
    }

    #[test]
    fn blocking_is_the_retail_five_types() {
        let blocking: Vec<u8> = (0..16).filter(|&v| is_blocking(v)).collect();
        assert_eq!(blocking, [8, 9, 0xA, 0xB, 0xC]);
        assert_eq!(collision_type_name(0xB), "ice_block");
        assert_eq!(collision_type_name(0xF), "unnamed_F");
    }
}
