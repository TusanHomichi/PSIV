//! Making a `layout_write` something a runtime can apply: the port of
//! `psiv_tools/map_patches.py`, which owns the retail citations (#216, slice 3).
//! A write resolves to the collision cells it changes and a tile in the map's
//! patch atlas, drawn with the map's own tileset and palette.
//!
//! `SCENE_CHUNK_WRITES` holds only the rows that add
//! atlas chunks (`via` `"scene"`): Python's others are for its own guard.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::draw::{
    CHUNK_PIXELS, CHUNK_TILES, OVERLAY_TRANSPARENT_INDICES, prepare_chunk, priority,
};
use super::effects::anchors::routine_address;
use super::effects::{field, field_mut, int, items, text};
use super::layouts::{ChunkTable, MapLayout, Plane, TilePatterns, chunk_collision_cells};
use super::overworld::Overworld;
use super::records::MapRecord;
use crate::PackError;
use crate::gfx;
use crate::json::Json;
use crate::png;
use crate::sprites::compose::SpriteCensus;
use crate::sprites::emit::{NpcMetadata, SheetRegistry};
use crate::sprites::field::{MAP_PALETTE_LINE_ORDER, StagedArt, object_sprite};
use crate::sprites::objects::FieldObjectRoutine;

/// The atlas is one row of 32x32 tiles.
pub const ATLAS_TILE_PIXELS: usize = CHUNK_PIXELS;
const GARUBERK_DOOR_OPENING_1: u8 = 0x03;
const GARUBERK_DOOR_OPENING_2: u8 = 0x04;
const GARUBERK_DOOR_TABLES: [(usize, usize); 4] =
    [(0x6F4D4, 28), (0x6F588, 28), (0x6F714, 16), (0x6F894, 16)];
const GARUBERK_DOOR_CHECKS: [(usize, &str); 2] = [(0x6F478, "0C110038"), (0x6F52E, "0C110038")];

/// One transcribed scene's live layout writes on one map that add atlas chunks.
struct SceneChunkWrite {
    scene: &'static str,
    map_id: usize,
    chunks: &'static [u8],
    table: Option<(usize, usize)>,
    checks: &'static [(usize, &'static str)],
}

const SCENE_CHUNK_WRITES: &[SceneChunkWrite] = &[
    SceneChunkWrite {
        scene: "Cutscene_CrashLaanding",
        map_id: 0x14C,
        chunks: &[],
        table: Some((0x76E58, 21)),
        checks: &[],
    },
    SceneChunkWrite {
        scene: "Event_SilenceTmDoorOpening",
        map_id: 0x0F0,
        chunks: &[
            0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61,
            0x62, 0x63, 0x64, 0x65, 0x66, 0x67,
        ],
        table: None,
        checks: &[
            (0x6FC34, "12811342000114831544"),
            (
                0x6FC5C,
                "0654555E5F06565760610658596263065A5B6465065C5D6667FF",
            ),
        ],
    },
    SceneChunkWrite {
        scene: "Event_TylerGraveOpening",
        map_id: 0x120,
        chunks: &[0x47],
        table: None,
        checks: &[(0x6FCF4, "12BC0047")],
    },
    SceneChunkWrite {
        scene: "Event_VahFortMovingPlatform1",
        map_id: 0x0CA,
        chunks: &[0xC0, 0xC1, 0xC2, 0xC3, 0xCC, 0xCD],
        table: None,
        checks: &[
            (0x6C5EC, "02C00220C2C302C002C0C0C1"),
            (0x6C5F8, "020002C002C0CCCDFE0002C00220CCCD"),
        ],
    },
    SceneChunkWrite {
        scene: "Event_VahFortMovingPlatform2",
        map_id: 0x0CA,
        chunks: &[0xC0, 0xC1, 0xCC, 0xCD],
        table: None,
        checks: &[
            (0x6C77C, "02C003A0C0C102C00300C0C1"),
            (0x6C788, "FE0002C00300CCCD020002C003A0CCCD"),
        ],
    },
    SceneChunkWrite {
        scene: "Event_WpnPlntMovingPlatform1",
        map_id: 0x0C5,
        chunks: &[0xC0, 0xC1, 0xC2, 0xC3, 0xCC, 0xCD],
        table: None,
        checks: &[
            (0x6C90C, "01C002A0C2C301C00380C0C1"),
            (0x6C918, "020001C00380CCCDFE0001C002A0CCCD"),
        ],
    },
    SceneChunkWrite {
        scene: "Event_WpnPlntMovingPlatform2",
        map_id: 0x0C5,
        chunks: &[0xC0, 0xC1, 0xC2, 0xC3, 0xCC, 0xCD],
        table: None,
        checks: &[
            (0x6CA9C, "02200160C2C302200240C0C1"),
            (0x6CAA8, "020002200240CCCDFE0002200160CCCD"),
        ],
    },
    SceneChunkWrite {
        scene: "Event_WpnPlntMovingPlatform3",
        map_id: 0x0C5,
        chunks: &[0xC0, 0xC1, 0xC2, 0xC3, 0xCC, 0xCD],
        table: None,
        checks: &[
            (0x6CC2C, "03800160C2C303800240C0C1"),
            (0x6CC38, "020003800240CCCDFE0003800160CCCD"),
        ],
    },
    SceneChunkWrite {
        scene: "Event_WpnPlntMovingPlatform4",
        map_id: 0x0C5,
        chunks: &[0xC0, 0xC1, 0xC2, 0xC3, 0xCC, 0xCD],
        table: None,
        checks: &[
            (0x6CDB8, "03E002A0C2C303E00380C0C1"),
            (0x6CDC4, "020003E00380CCCDFE0003E002A0CCCD"),
        ],
    },
];

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

/// The bytes at `offset` are the hex `expected`.
fn pinned(rom: &[u8], offset: usize, expected: &str) -> bool {
    let length = expected.len() / 2;
    rom.get(offset..offset + length)
        .is_some_and(|bytes| png::hex(bytes).eq_ignore_ascii_case(expected))
}

/// `chunk_ids`: what a row writes, with every pinned byte checked.
fn row_chunks(rom: &[u8], row: &SceneChunkWrite) -> Result<BTreeSet<u8>, PackError> {
    for &(offset, expected) in row.checks {
        if !pinned(rom, offset, expected) {
            return refuse(format!("{}: the write at ${offset:06X} moved", row.scene));
        }
    }
    let mut found: BTreeSet<u8> = row.chunks.iter().copied().collect();
    if let Some((offset, length)) = row.table {
        let table = rom.get(offset..offset + length);
        let Some((ids, [0xFF])) = table.map(|t| t.split_at(length - 1)) else {
            return refuse(format!(
                "{}: the chunk table at ${offset:06X} is incomplete",
                row.scene
            ));
        };
        if ids.contains(&0xFF) {
            return refuse(format!(
                "{}: the chunk table at ${offset:06X} is incomplete",
                row.scene
            ));
        }
        found.extend(ids);
    }
    Ok(found)
}

/// `garuberk_door_chunks`: every chunk the four door tables write.
fn garuberk_door_chunks(rom: &[u8]) -> Result<BTreeSet<u8>, PackError> {
    for (offset, expected) in GARUBERK_DOOR_CHECKS {
        if !pinned(rom, offset, expected) {
            return refuse(format!("Garuberk door guard at ${offset:06X} moved"));
        }
    }
    let mut chunks = BTreeSet::new();
    for (offset, length) in GARUBERK_DOOR_TABLES {
        let Some(table) = rom.get(offset..offset + length) else {
            return refuse(format!(
                "Garuberk door table at ${offset:06X} is incomplete"
            ));
        };
        if table[length - 1] != 0xFF || !(length - 1).is_multiple_of(3) {
            return refuse(format!(
                "Garuberk door table at ${offset:06X} is incomplete"
            ));
        }
        for row in table[..length - 1].as_chunks::<3>().0 {
            chunks.extend(&row[1..3]);
        }
    }
    Ok(chunks)
}

/// `scene_patch_chunks`: the original animation chunks this map's live scene
/// writes need, sorted.
pub fn scene_patch_chunks(rom: &[u8], record: &MapRecord) -> Result<Vec<u8>, PackError> {
    let events: BTreeSet<u8> = record
        .interaction_areas
        .entries
        .iter()
        .filter(|area| area.record.interaction_type == 2)
        .map(|area| area.record.parameter)
        .collect();
    let mut chunks = BTreeSet::new();
    if events.contains(&8) {
        // Event_BioPlantDoorOpening: delay, upper chunk, lower chunk; FF.
        match rom.get(0x6B9A4..0x6B9AE) {
            Some(table) if table[9] == 0xFF => {
                chunks.extend([1, 2, 4, 5, 7, 8].map(|i| table[i]));
            }
            _ => return refuse("BioPlant door animation table is incomplete".into()),
        }
    }
    if events.contains(&0x13) {
        // Opening and closing tables: CPU delay, chunk; FF. $4F is the original
        // closed chunk checked by Event_ElevatorDoorOpening.
        chunks.insert(0x4F);
        for offset in [0x6C334, 0x6C46E] {
            match rom.get(offset..offset + 9) {
                Some(table) if table[8] == 0xFF => {
                    chunks.extend(table[1..8].iter().step_by(2));
                }
                _ => return refuse("elevator door animation table is incomplete".into()),
            }
        }
    }
    if events.contains(&GARUBERK_DOOR_OPENING_1) || events.contains(&GARUBERK_DOOR_OPENING_2) {
        chunks.extend(garuberk_door_chunks(rom)?);
    }
    for row in SCENE_CHUNK_WRITES
        .iter()
        .filter(|row| row.map_id == record.id)
    {
        chunks.extend(row_chunks(rom, row)?);
    }
    Ok(chunks.into_iter().collect())
}

/// Census counts from [`resolve_map_effects`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PatchCounts {
    pub layout_writes: usize,
    pub collision_authoritative: usize,
    pub picture_only: usize,
    pub cells: usize,
    pub map_change_cells: usize,
}

impl PatchCounts {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("layout_writes", self.layout_writes)
            .with("collision_authoritative", self.collision_authoritative)
            .with("picture_only", self.picture_only)
            .with("cells", self.cells)
            .with("map_change_cells", self.map_change_cells)
    }
}

/// `resolve_map_effects`: fold resolved cells into a map's effect list, and
/// collect its patched chunks.
pub fn resolve_map_effects(
    decoded: &MapLayout,
    effects: &[Json],
) -> Result<(Vec<Json>, Vec<u8>, PatchCounts), PackError> {
    let plane = decoded.spec.collision_plane().name();
    let mut chunk_ids = Vec::new();
    let mut counts = PatchCounts::default();
    let mut resolved_effects = Vec::with_capacity(effects.len());
    for effect in effects {
        let mut paths = Vec::new();
        for path in items(effect, "paths") {
            let mut writes = Vec::new();
            for write in items(path, "writes") {
                if text(write, "kind") != "layout_write" {
                    writes.push(write.clone());
                    continue;
                }
                counts.layout_writes += 1;
                let (Some(chunk_id), Some(cell_x), Some(cell_y)) = (
                    int(write, "chunk_id"),
                    int(write, "cell_x"),
                    int(write, "cell_y"),
                ) else {
                    return refuse(format!(
                        "layout_write at {} has no chunk id or no cell",
                        text(write, "at")
                    ));
                };
                let Some(words) = usize::try_from(chunk_id)
                    .ok()
                    .and_then(|id| decoded.chunks.words.get(id))
                else {
                    return refuse(format!(
                        "layout_write names chunk 0x{chunk_id:02X}, outside the {} this map loads",
                        decoded.chunks.words.len()
                    ));
                };
                let values = chunk_collision_cells(words);
                let cells: Vec<Json> = [(0, 0), (1, 0), (0, 1), (1, 1)]
                    .into_iter()
                    .zip(values)
                    .map(|((dx, dy), collision)| {
                        Json::object()
                            .with("x", Json::Int(cell_x + dx))
                            .with("y", Json::Int(cell_y + dy))
                            .with("collision", collision)
                    })
                    .collect();
                let authoritative = text(write, "plane") == plane;
                if authoritative {
                    counts.collision_authoritative += 1;
                } else {
                    counts.picture_only += 1;
                }
                counts.cells += cells.len();
                counts.map_change_cells += values.iter().filter(|&&v| v == 1).count();
                chunk_ids.push(chunk_id as u8);
                writes.push(
                    write
                        .clone()
                        .with("cells", cells)
                        .with("collision_authoritative", authoritative)
                        .with("collision_plane", plane),
                );
            }
            paths.push(path.clone().with("writes", writes));
        }
        resolved_effects.push(effect.clone().with("paths", paths));
    }
    Ok((resolved_effects, chunk_ids, counts))
}

/// The tile groups by flag, and the FG/BG pairs they need composed.
pub type OverworldPatches = (Vec<Json>, Vec<(u8, u8)>);
type FlagCells = BTreeMap<(usize, usize), [u8; 2]>;
/// The base PNG, the overlay PNG if any, and the tile entries.
pub type PatchAtlas = (Vec<u8>, Option<Vec<u8>>, Vec<Json>);

/// `resolve_overworld_patches`: the paged loader's hooks as tiles by flag, and
/// the FG/BG pairs they need composed. Distinct flags must not touch one chunk,
/// or one precomposed tile would hide an unmodelled combination.
pub fn resolve_overworld_patches(
    decoded: &MapLayout,
    overworld: Option<&Overworld>,
) -> Result<OverworldPatches, PackError> {
    let Some(overworld) = overworld else {
        return Ok((Vec::new(), Vec::new()));
    };
    let (width, height) = (decoded.bg.width_chunks, decoded.bg.height_chunks);
    if (decoded.fg.width_chunks, decoded.fg.height_chunks) != (width, height) {
        return refuse("overworld planes have different dimensions".into());
    }
    // Flags in first-seen order, each with its cells in coordinate order.
    let mut by_flag: Vec<(u16, FlagCells)> = Vec::new();
    let mut owner: BTreeMap<(usize, usize), u16> = BTreeMap::new();
    for patch in &overworld.patches {
        let flag = patch.event_flag;
        if flag >= 512 {
            return refuse(format!(
                "overworld event flag {flag} is outside the event bank"
            ));
        }
        let at = match by_flag.iter().position(|(seen, _)| *seen == flag) {
            Some(at) => at,
            None => {
                by_flag.push((flag, BTreeMap::new()));
                by_flag.len() - 1
            }
        };
        for write in &patch.writes {
            if write.chunk_ids.is_empty() {
                return refuse("overworld hook contains an empty chunk write".into());
            }
            for (step, &chunk_id) in write.chunk_ids.iter().enumerate() {
                let (x, y) = (write.chunk_x + step, write.chunk_y);
                if x >= width || y >= height {
                    return refuse(format!(
                        "overworld patch chunk ({x},{y}) is outside {width}x{height}"
                    ));
                }
                if usize::from(chunk_id) >= decoded.chunks.words.len() {
                    return refuse(format!(
                        "overworld patch chunk {chunk_id:#04x} is not loaded"
                    ));
                }
                let previous = *owner.entry((x, y)).or_insert(flag);
                if previous != flag {
                    return refuse(format!(
                        "overworld chunk ({x},{y}) has overlapping flags {previous:#x}/{flag:#x}"
                    ));
                }
                let pair = by_flag[at]
                    .1
                    .entry((x, y))
                    .or_insert_with(|| [decoded.fg.chunk_at(x, y), decoded.bg.chunk_at(x, y)]);
                pair[usize::from(write.plane == Plane::Bg)] = chunk_id;
            }
        }
    }
    let collision_plane = usize::from(decoded.spec.collision_plane() == Plane::Bg);
    let mut groups = Vec::with_capacity(by_flag.len());
    let mut pairs = BTreeSet::new();
    for (flag, cells) in by_flag {
        let mut tiles = Vec::with_capacity(cells.len());
        for ((x, y), pair) in cells {
            let collision_id = pair[collision_plane];
            let collision = chunk_collision_cells(decoded.chunks.get(collision_id)?);
            tiles.push(
                Json::object()
                    .with("chunk_x", x)
                    .with("chunk_y", y)
                    .with("fg_chunk_id", pair[0])
                    .with("bg_chunk_id", pair[1])
                    .with("collision_chunk_id", collision_id)
                    .with("collision", collision.to_vec()),
            );
            pairs.insert((pair[0], pair[1]));
        }
        groups.push(Json::object().with("event_flag", flag).with("tiles", tiles));
    }
    Ok((groups, pairs.into_iter().collect()))
}

type Tile = Box<[u8; CHUNK_PIXELS * CHUNK_PIXELS]>;

/// `compose_chunk`: one chunk definition drawn, colour 0 of either line as 0,
/// and how many tiles it placed. `priority_only` keeps the above-sprites tiles.
fn compose_chunk(
    chunks: &ChunkTable,
    patterns: &TilePatterns,
    chunk_id: u8,
    priority_only: bool,
) -> Result<(Tile, usize), PackError> {
    let words = chunks.get(chunk_id)?;
    let entries: Vec<(usize, usize, u16)> = words
        .iter()
        .enumerate()
        .filter(|&(_, &word)| !priority_only || priority(word))
        .map(|(i, &word)| (i % CHUNK_TILES, i / CHUNK_TILES, word))
        .collect();
    let placed = entries
        .iter()
        .filter(|&&(_, _, word)| patterns.is_loaded(super::draw::tile_index(word)))
        .count();
    let mut pixels = prepare_chunk(entries, patterns);
    for pixel in pixels.iter_mut() {
        if *pixel & 0x0F == 0 {
            *pixel = 0;
        }
    }
    Ok((pixels, placed))
}

fn over(fg: &Tile, bg: &Tile) -> Tile {
    let mut out = bg.clone();
    for (pixel, &value) in out.iter_mut().zip(fg.iter()) {
        if value & 0x0F != 0 {
            *pixel = value;
        }
    }
    out
}

fn atlas(tiles: &[Tile], palette: &[[u8; 3]], transparent: &[u8]) -> Result<Vec<u8>, PackError> {
    let width = ATLAS_TILE_PIXELS * tiles.len();
    let mut pixels = vec![0u8; width * ATLAS_TILE_PIXELS];
    for (index, tile) in tiles.iter().enumerate() {
        for (y, row) in tile.as_chunks::<ATLAS_TILE_PIXELS>().0.iter().enumerate() {
            let start = y * width + index * ATLAS_TILE_PIXELS;
            pixels[start..start + ATLAS_TILE_PIXELS].copy_from_slice(row);
        }
    }
    Ok(png::encode_indexed(
        width as u32,
        ATLAS_TILE_PIXELS as u32,
        &pixels,
        palette,
        transparent,
    )?)
}

/// `patch_atlas`: raw chunk tiles, then composed overworld pairs, in one row.
/// The overlay is `None` when no raw tile has priority pixels and there are no
/// composites; composites always get one, so a changed chunk can clear the
/// baked map's old above-sprites pixels.
pub fn patch_atlas(
    decoded: &MapLayout,
    chunk_ids: &BTreeSet<u8>,
    palette: &[[u8; 3]],
    composite_pairs: &[(u8, u8)],
) -> Result<PatchAtlas, PackError> {
    if chunk_ids.is_empty() && composite_pairs.is_empty() {
        return refuse("cannot build an atlas from no chunks".into());
    }
    let (chunks, patterns) = (&decoded.chunks, &decoded.patterns);
    let mut base_tiles = Vec::new();
    let mut over_tiles = Vec::new();
    let mut entries = Vec::new();
    let mut priority_pixels = 0;
    let entry = |index: usize, placed: usize, opaque: usize, collision_id: u8| {
        Ok::<Json, PackError>(
            Json::object()
                .with("index", index)
                .with("x", index * ATLAS_TILE_PIXELS)
                .with("priority_tiles", placed)
                .with("priority_pixels", opaque)
                .with(
                    "collision",
                    chunk_collision_cells(chunks.get(collision_id)?).to_vec(),
                ),
        )
    };
    for &chunk_id in chunk_ids {
        let (base, _) = compose_chunk(chunks, patterns, chunk_id, false)?;
        let (above, placed) = compose_chunk(chunks, patterns, chunk_id, true)?;
        let opaque = above.iter().filter(|&&value| value != 0).count();
        priority_pixels += opaque;
        base_tiles.push(base);
        over_tiles.push(above);
        entries.push(entry(entries.len(), placed, opaque, chunk_id)?.with("chunk_id", chunk_id));
    }
    for &(fg_id, bg_id) in composite_pairs {
        let (fg, _) = compose_chunk(chunks, patterns, fg_id, false)?;
        let (bg, _) = compose_chunk(chunks, patterns, bg_id, false)?;
        let (fg_over, fg_count) = compose_chunk(chunks, patterns, fg_id, true)?;
        let (bg_over, bg_count) = compose_chunk(chunks, patterns, bg_id, true)?;
        let above = over(&fg_over, &bg_over);
        let opaque = above.iter().filter(|&&value| value & 0x0F != 0).count();
        priority_pixels += opaque;
        base_tiles.push(over(&fg, &bg));
        over_tiles.push(above);
        let collision_id = if decoded.spec.collision_plane() == Plane::Fg {
            fg_id
        } else {
            bg_id
        };
        entries.push(
            entry(entries.len(), bg_count + fg_count, opaque, collision_id)?
                .with("chunk_id", Json::Null)
                .with("fg_chunk_id", fg_id)
                .with("bg_chunk_id", bg_id),
        );
    }
    let base_png = atlas(&base_tiles, palette, &[])?;
    let over_png = if priority_pixels != 0 || !composite_pairs.is_empty() {
        Some(atlas(&over_tiles, palette, &OVERLAY_TRANSPARENT_INDICES)?)
    } else {
        None
    };
    Ok((base_png, over_png, entries))
}

/// `index_writes`: point each resolved write at its atlas tile.
pub fn index_writes(effects: &mut [Json], entries: &[Json]) -> Result<(), PackError> {
    let tile_of: BTreeMap<i128, i128> = entries
        .iter()
        .filter_map(|entry| Some((int(entry, "chunk_id")?, int(entry, "index")?)))
        .collect();
    for effect in effects {
        let Some(Json::Array(paths)) = field_mut(effect, "paths") else {
            continue;
        };
        for path in paths {
            let Some(Json::Array(writes)) = field_mut(path, "writes") else {
                continue;
            };
            for write in writes {
                if text(write, "kind") != "layout_write" {
                    continue;
                }
                let tile = int(write, "chunk_id").and_then(|id| tile_of.get(&id));
                let Some(&tile) = tile else {
                    return refuse(format!(
                        "no atlas tile for the write at {}",
                        text(write, "at")
                    ));
                };
                write.insert("patch_tile", Json::Int(tile));
            }
        }
    }
    Ok(())
}

/// `index_overworld_patches`: bind composed page-hook tiles.
pub fn index_overworld_patches(patches: &mut [Json], entries: &[Json]) -> Result<(), PackError> {
    let tile_of: BTreeMap<(i128, i128), i128> = entries
        .iter()
        .filter(|entry| field(entry, "chunk_id") == Some(&Json::Null))
        .filter_map(|entry| {
            Some((
                (int(entry, "fg_chunk_id")?, int(entry, "bg_chunk_id")?),
                int(entry, "index")?,
            ))
        })
        .collect();
    for patch in patches {
        let Some(Json::Array(tiles)) = field_mut(patch, "tiles") else {
            continue;
        };
        for tile in tiles {
            let pair = int(tile, "fg_chunk_id").zip(int(tile, "bg_chunk_id"));
            let Some(&index) = pair.and_then(|pair| tile_of.get(&pair)) else {
                return refuse("no atlas tile for an overworld patch".into());
            };
            tile.insert("patch_tile", Json::Int(index));
        }
    }
    Ok(())
}

/// `atlas_json`: the atlas as a map record carries it.
pub fn atlas_json(
    entries: Vec<Json>,
    base_name: &str,
    base_image: &[u8],
    over_name: Option<&str>,
    over_image: Option<&[u8]>,
) -> Result<Json, PackError> {
    Ok(Json::object()
        .with("png", base_name)
        .with("png_sha256", png::image_sha256(base_image)?)
        .with("png_over", over_name)
        .with(
            "png_over_sha256",
            over_image.map(png::image_sha256).transpose()?,
        )
        .with("tile_pixels", ATLAS_TILE_PIXELS)
        .with("count", entries.len())
        .with(
            "note",
            "Blit tile `index` at (chunk_x * 32, chunk_y * 32) over the baked map PNG; the \
             overlay tile of the same index goes above sprites.",
        )
        .with("tiles", entries))
}

const PALETTE_TABLE_BUFFER: usize = 0xFB00;
const CRAM_LINE_BYTES: usize = 32;
const PALETTE_COPY_SEARCH: usize = 0x80;

/// A decoded `lea src,a0 / lea dst,a1 / move.w #n,d7 / move.l (a0)+,(a1)+ / dbf`.
struct PaletteCopy {
    at: usize,
    source: usize,
    offset: usize,
    words: usize,
}

/// `decode_palette_copy`: the bulk copy in the first bytes of `routine`, if any.
/// A copy that isn't one whole CRAM line on a line boundary is refused, because
/// everything claimed about what it affects rests on that.
fn decode_palette_copy(rom: &[u8], routine: usize) -> Result<Option<PaletteCopy>, PackError> {
    const LENGTH: usize = 20;
    let end = (routine + PALETTE_COPY_SEARCH).min(rom.len());
    let found = rom.get(routine..end).and_then(|span| {
        span.windows(LENGTH).position(|w| {
            w[..2] == [0x41, 0xF9]
                && w[6..8] == [0x43, 0xF8]
                && w[10..12] == [0x3E, 0x3C]
                && w[14..20] == [0x22, 0xD8, 0x51, 0xCF, 0xFF, 0xFC]
        })
    });
    let Some(position) = found else {
        return Ok(None);
    };
    let at = routine + position;
    let word =
        |offset: usize| usize::from(u16::from_be_bytes([rom[at + offset], rom[at + offset + 1]]));
    let source = word(2) << 16 | word(4);
    let destination = word(8);
    let words = (word(12) + 1) * 2;
    if destination < PALETTE_TABLE_BUFFER
        || !(destination - PALETTE_TABLE_BUFFER).is_multiple_of(CRAM_LINE_BYTES)
        || words != 16
    {
        return refuse(format!(
            "the bulk copy at 0x{routine:06X} writes {words} words to 0xFFFF{destination:04X},              which is not one whole CRAM line of 0xfb00"
        ));
    }
    Ok(Some(PaletteCopy {
        at,
        source,
        offset: destination - PALETTE_TABLE_BUFFER,
        words,
    }))
}

/// `resolve_palette_effects`: attach each palette copy to the sprite sheets it
/// repaints. The effect hangs on the path, because two of the three copies sit
/// on paths whose only writes are `object_despawn`. An affected NPC is resolved
/// again under the post-copy palette and registered like any sheet. Returns the
/// counts: copies, alternate sheets, repainted NPCs.
#[allow(clippy::too_many_arguments)]
pub fn resolve_palette_effects(
    rom: &[u8],
    record: &MapRecord,
    art: &StagedArt,
    effects: &mut [Json],
    npcs: &[NpcMetadata],
    routines: &[FieldObjectRoutine],
    extents: &HashMap<u32, usize>,
    registry: &mut SheetRegistry,
    census: &mut SpriteCensus,
    map_palette: &[[u8; 3]],
    dispatch: usize,
) -> Result<[usize; 3], PackError> {
    let mut counts = [0; 3];
    for effect in effects {
        let entry = int(effect, "entry");
        let Some(Json::Array(paths)) = field_mut(effect, "paths") else {
            continue;
        };
        for path in paths {
            let copies = items(path, "deferred")
                .iter()
                .any(|note| note == &Json::from("postincrement bulk copy"));
            if !copies {
                continue;
            }
            let index = entry.unwrap_or(0) as usize;
            let routine = routine_address(rom, dispatch, index)?;
            let Some(copy) = decode_palette_copy(rom, routine)? else {
                continue;
            };
            counts[0] += 1;
            let raw = rom
                .get(copy.source..copy.source + CRAM_LINE_BYTES)
                .unwrap_or(&[]);
            let colors = gfx::decode_palette(raw)?;
            let rgb = gfx::palette_rgb(&colors);
            let line = copy.offset / CRAM_LINE_BYTES;
            let Some(position) = MAP_PALETTE_LINE_ORDER
                .iter()
                .position(|&l| usize::from(l) == line)
            else {
                return refuse(format!("{line} is not in a map palette blob"));
            };
            let mut after = map_palette.to_vec();
            after.splice(position * 16..(position + 1) * 16, rgb.iter().copied());
            let mut repainted = Vec::new();
            let mut alternates = BTreeSet::new();
            for (npc_index, (object, meta)) in record.objects.entries.iter().zip(npcs).enumerate() {
                let object = &object.record;
                let found = routines
                    .iter()
                    .find(|r| r.object_id() == usize::from(object.object_id));
                let Some(routine) = found else { continue };
                let Some(from) = &meta.sheet else { continue };
                if routine.palette_line().map(usize::from) != Some(line) {
                    continue;
                }
                let alternate = object_sprite(
                    rom,
                    routine,
                    extents,
                    art,
                    &after,
                    object.facing_dir,
                    object.art_tile,
                    Some(census),
                )?;
                let Some(sheet) = alternate.sheet else {
                    continue;
                };
                let symbol = object.symbol();
                let to = registry.register(sheet, symbol.unwrap_or("FieldObj"), false)?;
                if to == *from {
                    continue;
                }
                alternates.insert(to.clone());
                repainted.push(
                    Json::object()
                        .with("npc_index", npc_index)
                        .with("symbol", symbol)
                        .with("from", from.as_str())
                        .with("to", to),
                );
            }
            counts[2] += repainted.len();
            counts[1] += alternates.len();
            path.insert(
                "deferred_effects",
                vec![
                    Json::object()
                        .with("instruction", "postincrement bulk copy")
                        .with("resolved_as", "palette_write")
                        .with("at", format!("0x{:06X}", copy.at))
                        .with("source", format!("0x{:06X}", copy.source))
                        .with(
                            "destination",
                            format!("Palette_Table_Buffer+0x{:02X}", copy.offset),
                        )
                        .with("cram_line", line)
                        .with("words", copy.words)
                        .with("installs_chunk_definitions", false)
                        .with("colors", rgb.iter().map(|c| c.to_vec()).collect::<Vec<_>>())
                        .with(
                            "words_hex",
                            colors
                                .iter()
                                .map(|c| format!("0x{:04X}", c.raw))
                                .collect::<Vec<_>>(),
                        )
                        .with(
                            "affects",
                            Json::object()
                                .with("map_pixels", 0)
                                .with("map_render_identical", true)
                                .with(
                                    "reason",
                                    "chunk tiles reach CRAM lines 0 and 1 only; bit 14 is the \
                                     collision flag and never reaches the VDP",
                                )
                                .with("npc_sheets", repainted),
                        ),
                ],
            );
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests;
