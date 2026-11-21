//! `Chunk_Table` 16-word chunk-definition copies (`#676`):
//!
//! - `Event_EclipseTorchUsed` (`$07041E` before dialogue `$2A`, repeated at
//!   `$070466` after dialogue `$2A`, `ps4.asm:149699-149718`) copies 16 words
//!   from `$FFFF6100` (chunk `$08`) to `$FFFF6960` (chunk `$4B`) via `trap #1`.
//! - `MapDataMan_Dezolis` (`loc_525AC`, `$0525AC`, `ps4.asm:108843-108851`)
//!   applies the identical 16-word copy when `EventFlag_EclipseTorch` (`$9C`)
//!   is set.
//!
//! Because this mutates a chunk definition rather than a layout byte, every
//! occurrence of chunk `$4B` on Dezolis (`map 1`) takes chunk `$08`'s collision
//! and tile art while neighboring chunk definitions remain untouched.

use std::collections::BTreeSet;

use super::layouts::{ChunkTable, MapLayout, Plane, chunk_collision_cells};
use super::records::MapRecord;
use crate::PackError;
use crate::json::Json;
use crate::m68k::{l, w};

const CHUNK_TABLE_BASE: u32 = 0xFFFF_6000;
const CHUNK_STRIDE_BYTES: u32 = 0x20;
const DEZOLIS_MAP_ID: usize = 1;
const MAP_DATA_MAN_DEZOLIS_ECLIPSE: usize = 0x0525AC;
const EVENT_ECLIPSE_TORCH_COPY_1: usize = 0x07041E;
const EVENT_ECLIPSE_TORCH_COPY_2: usize = 0x070466;
const EVENT_FLAGS_TEST_REL: u16 = 0x5072;
const EVENT_FLAGS_SET: u32 = 0x0005_7666;
const FIELD_UPDATE_OBJECTS: u32 = 0x0004_47D8;
const REFRESH_PLANE_BG: u32 = 0x0005_40FA;
const DMA_PLANE_B_VINT: u32 = 0x0004_16A6;

/// Verified retail specification of one 16-word `Chunk_Table` copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkCopySpec {
    pub source_addr: u32,
    pub dest_addr: u32,
    pub words: u16,
    pub source_chunk: u8,
    pub dest_chunk: u8,
    pub event_flag: u16,
}

fn parse_trap1_copy(rom: &[u8], at: usize, label: &str) -> Result<(u32, u32, u16), PackError> {
    // `lea (src).l, a0` (`41F9`), `lea (dst).l, a1` (`43F9`),
    // `move.w #(words-1), d7` (`3E3C`), `trap #1` (`4E41`).
    if w(rom, at)? != 0x41F9
        || w(rom, at + 6)? != 0x43F9
        || w(rom, at + 12)? != 0x3E3C
        || w(rom, at + 16)? != 0x4E41
    {
        return Err(PackError(format!(
            "{label} at {at:#08x}: expected lea/lea/move.w/trap #1"
        )));
    }
    let src = l(rom, at + 2)?;
    let dst = l(rom, at + 8)?;
    let words = w(rom, at + 14)?
        .checked_add(1)
        .ok_or_else(|| PackError(format!("{label}: word count overflow")))?;
    Ok((src, dst, words))
}

fn chunk_index(addr: u32, label: &str) -> Result<u8, PackError> {
    let offset = addr.checked_sub(CHUNK_TABLE_BASE).ok_or_else(|| {
        PackError(format!(
            "{label}: address {addr:#010x} is below Chunk_Table ({CHUNK_TABLE_BASE:#010x})"
        ))
    })?;
    if offset % CHUNK_STRIDE_BYTES != 0 {
        return Err(PackError(format!(
            "{label}: address {addr:#010x} is not 32-byte chunk aligned"
        )));
    }
    let id = offset / CHUNK_STRIDE_BYTES;
    u8::try_from(id).map_err(|_| {
        PackError(format!(
            "{label}: chunk index {id:#x} exceeds 256-chunk table"
        ))
    })
}

/// Pin US retail instructions at `MapDataMan_Dezolis` (`loc_525AC`, `$0525AC`)
/// and both `Event_EclipseTorchUsed` sites (`$07041E`, `$070466`).
pub fn extract_eclipse_torch_copy(rom: &[u8]) -> Result<ChunkCopySpec, PackError> {
    // `MapDataMan_Dezolis` (`loc_525AC`, `$0525AC`):
    // `move.w #$9C, d0 / bsr.w EventFlags_Test / beq.s loc_525C8 / <trap #1 copy>`.
    if w(rom, MAP_DATA_MAN_DEZOLIS_ECLIPSE)? != 0x303C
        || w(rom, MAP_DATA_MAN_DEZOLIS_ECLIPSE + 4)? != 0x6100
        || w(rom, MAP_DATA_MAN_DEZOLIS_ECLIPSE + 6)? != EVENT_FLAGS_TEST_REL
        || w(rom, MAP_DATA_MAN_DEZOLIS_ECLIPSE + 8)? != 0x6712
    {
        return Err(PackError(
            "MapDataMan_Dezolis loc_525AC flag guard moved".into(),
        ));
    }
    let event_flag = w(rom, MAP_DATA_MAN_DEZOLIS_ECLIPSE + 2)?;
    let load_copy = parse_trap1_copy(
        rom,
        MAP_DATA_MAN_DEZOLIS_ECLIPSE + 10,
        "MapDataMan_Dezolis loc_525AC",
    )?;
    let scene_copy_1 = parse_trap1_copy(
        rom,
        EVENT_ECLIPSE_TORCH_COPY_1,
        "Event_EclipseTorchUsed $07041E",
    )?;
    // Verify subsequent Field_UpdateObjects -> RefreshPlaneBG -> DMAPlane_B_VInt ordering.
    if w(rom, EVENT_ECLIPSE_TORCH_COPY_1 + 18)? != 0x4EB9
        || l(rom, EVENT_ECLIPSE_TORCH_COPY_1 + 20)? != FIELD_UPDATE_OBJECTS
        || w(rom, EVENT_ECLIPSE_TORCH_COPY_1 + 24)? != 0x4EB9
        || l(rom, EVENT_ECLIPSE_TORCH_COPY_1 + 26)? != REFRESH_PLANE_BG
        || w(rom, EVENT_ECLIPSE_TORCH_COPY_1 + 30)? != 0x4EB9
        || l(rom, EVENT_ECLIPSE_TORCH_COPY_1 + 32)? != DMA_PLANE_B_VINT
    {
        return Err(PackError(
            "Event_EclipseTorchUsed $070430..$070441 refresh/DMA sequence moved".into(),
        ));
    }
    let scene_copy_2 = parse_trap1_copy(
        rom,
        EVENT_ECLIPSE_TORCH_COPY_2,
        "Event_EclipseTorchUsed $070466",
    )?;
    if w(rom, EVENT_ECLIPSE_TORCH_COPY_2 + 18)? != 0x103C
        || w(rom, EVENT_ECLIPSE_TORCH_COPY_2 + 20)? != event_flag
        || w(rom, EVENT_ECLIPSE_TORCH_COPY_2 + 22)? != 0x4EF9
        || l(rom, EVENT_ECLIPSE_TORCH_COPY_2 + 24)? != EVENT_FLAGS_SET
    {
        return Err(PackError(
            "Event_EclipseTorchUsed $070478 EventFlag_EclipseTorch set moved".into(),
        ));
    }
    if load_copy != scene_copy_1 || load_copy != scene_copy_2 {
        return Err(PackError(format!(
            "Eclipse Torch chunk copies disagree: load={load_copy:?}, scene1={scene_copy_1:?}, scene2={scene_copy_2:?}"
        )));
    }
    let (source_addr, dest_addr, words) = load_copy;
    if words != 16 {
        return Err(PackError(format!(
            "Eclipse Torch chunk copy expected 16 words, got {words}"
        )));
    }
    let source_chunk = chunk_index(source_addr, "Eclipse Torch source")?;
    let dest_chunk = chunk_index(dest_addr, "Eclipse Torch destination")?;
    Ok(ChunkCopySpec {
        source_addr,
        dest_addr,
        words,
        source_chunk,
        dest_chunk,
        event_flag,
    })
}

/// Apply an exact 16-word chunk-definition copy to a cloned `ChunkTable`,
/// leaving every other chunk definition byte-for-byte unchanged.
pub fn copy_chunk_definition(
    chunks: &ChunkTable,
    source_chunk: u8,
    dest_chunk: u8,
) -> Result<ChunkTable, PackError> {
    let source_words = *chunks.get(source_chunk)?;
    let _ = chunks.get(dest_chunk)?;
    let mut mutated = chunks.clone();
    mutated.words[usize::from(dest_chunk)] = source_words;
    Ok(mutated)
}

/// Resolved chunk-definition copy JSON records and required `(fg, bg)` patch
/// atlas pairs.
pub type ChunkDefCopyResolution = (Vec<Json>, Vec<(u8, u8)>);

/// Resolve Dezolis's Eclipse Torch chunk-definition copy (`$08 -> $4B`) to
/// every affected `(chunk_x, chunk_y)` cell, its post-copy collision cells, and
/// the composed `(fg, bg)` pairs `patch_atlas` needs to render.
pub fn resolve_chunk_def_copies(
    rom: &[u8],
    record: &MapRecord,
    decoded: &MapLayout,
) -> Result<ChunkDefCopyResolution, PackError> {
    if record.id != DEZOLIS_MAP_ID {
        return Ok((Vec::new(), Vec::new()));
    }
    let spec = extract_eclipse_torch_copy(rom)?;
    let mutated = copy_chunk_definition(&decoded.chunks, spec.source_chunk, spec.dest_chunk)?;
    let (width, height) = (decoded.bg.width_chunks, decoded.bg.height_chunks);
    let mut tiles = Vec::new();
    let mut pairs = BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            let fg = decoded.fg.chunk_at(x, y);
            let bg = decoded.bg.chunk_at(x, y);
            if fg != spec.dest_chunk && bg != spec.dest_chunk {
                continue;
            }
            let fg_after = if fg == spec.dest_chunk {
                spec.source_chunk
            } else {
                fg
            };
            let bg_after = if bg == spec.dest_chunk {
                spec.source_chunk
            } else {
                bg
            };
            let collision_id = if decoded.spec.collision_plane() == Plane::Bg {
                bg
            } else {
                fg
            };
            let collision = chunk_collision_cells(mutated.get(collision_id)?);
            tiles.push(
                Json::object()
                    .with("chunk_x", x)
                    .with("chunk_y", y)
                    .with("fg_chunk_id", fg_after)
                    .with("bg_chunk_id", bg_after)
                    .with("collision_chunk_id", collision_id)
                    .with("collision", collision.to_vec()),
            );
            pairs.insert((fg_after, bg_after));
        }
    }
    if tiles.is_empty() {
        return Err(PackError(format!(
            "Dezolis has no occurrences of chunk {:#04x}",
            spec.dest_chunk
        )));
    }
    let copy_json = Json::object()
        .with("source_chunk", spec.source_chunk)
        .with("dest_chunk", spec.dest_chunk)
        .with("event_flag", spec.event_flag)
        .with("tiles", tiles);
    Ok((vec![copy_json], pairs.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::Decodes;
    use crate::maps::extract_maps;
    use crate::maps::overworld::decode_map_section;

    #[test]
    fn eclipse_torch_copies_exact_sixteen_words_and_resolves_all_eleven_dezolis_trees() {
        let Some(rom) = crate::test_support::retail_rom("eclipse torch chunk copy") else {
            return;
        };
        let data = rom.bytes();
        let spec = extract_eclipse_torch_copy(data).unwrap();
        assert_eq!(spec.source_addr, 0xFFFF_6100);
        assert_eq!(spec.dest_addr, 0xFFFF_6960);
        assert_eq!(spec.words, 16);
        assert_eq!(spec.source_chunk, 0x08);
        assert_eq!(spec.dest_chunk, 0x4B);
        assert_eq!(spec.event_flag, 0x9C);

        let decodes = Decodes::new(data);
        let maps = extract_maps(data, &decodes).unwrap();
        let dezolis = maps.real().find(|m| m.id == 1).unwrap();
        let section = decode_map_section(data, &decodes, dezolis).unwrap();
        let decoded = section.layout();

        // Mutating Chunk_Table changes ONLY chunk $4B to match chunk $08's 16 words.
        let mutated = copy_chunk_definition(&decoded.chunks, 0x08, 0x4B).unwrap();
        assert_eq!(mutated.words[0x4B], decoded.chunks.words[0x08]);
        assert_ne!(decoded.chunks.words[0x4B], decoded.chunks.words[0x08]);
        for id in 0..decoded.chunks.words.len() {
            if id != 0x4B {
                assert_eq!(
                    mutated.words[id], decoded.chunks.words[id],
                    "neighboring chunk {id:#04x} must stay unchanged"
                );
            }
        }
        // Chunk $4B carries the dark-tree base visuals while sharing walkable
        // [0, 0, 0, 0] collision with snow chunk $08 (entry is gated by the
        // Eclipse Torch trigger until $9C is set).
        assert_eq!(
            chunk_collision_cells(&mutated.words[0x4B]),
            chunk_collision_cells(&decoded.chunks.words[0x08])
        );
        assert_eq!(chunk_collision_cells(&mutated.words[0x4B]), [0, 0, 0, 0]);

        let (copies, pairs) = resolve_chunk_def_copies(data, dezolis, decoded).unwrap();
        assert_eq!(copies.len(), 1);
        let val = serde_json::Value::from(&copies[0]);
        let tiles = val["tiles"].as_array().unwrap();
        assert_eq!(tiles.len(), 11, "all 11 Dezolis Garuberk tree chunks");
        for tile in tiles {
            assert_eq!(tile["collision"], serde_json::json!([0, 0, 0, 0]));
            assert_eq!(tile["bg_chunk_id"], 0x08);
            assert_eq!(tile["collision_chunk_id"], 0x4B);
        }
        assert!(!pairs.is_empty());

        // Unrelated map control: Motavia (0) and Piata (0x10) emit no chunk copies.
        let motavia = maps.real().find(|m| m.id == 0).unwrap();
        let mota_sec = decode_map_section(data, &decodes, motavia).unwrap();
        let (mota_copies, mota_pairs) =
            resolve_chunk_def_copies(data, motavia, mota_sec.layout()).unwrap();
        assert!(mota_copies.is_empty() && mota_pairs.is_empty());

        // Negative controls: altering word count, post-dialogue copy, or load flag fails.
        let mut bad_count = data.to_vec();
        bad_count[EVENT_ECLIPSE_TORCH_COPY_1 + 14..EVENT_ECLIPSE_TORCH_COPY_1 + 16]
            .copy_from_slice(&0x000Eu16.to_be_bytes());
        assert!(extract_eclipse_torch_copy(&bad_count).is_err());

        let mut bad_repeat = data.to_vec();
        bad_repeat[EVENT_ECLIPSE_TORCH_COPY_2 + 8..EVENT_ECLIPSE_TORCH_COPY_2 + 12]
            .copy_from_slice(&0xFFFF_6980u32.to_be_bytes());
        assert!(extract_eclipse_torch_copy(&bad_repeat).is_err());

        let mut bad_flag = data.to_vec();
        bad_flag[MAP_DATA_MAN_DEZOLIS_ECLIPSE + 2..MAP_DATA_MAN_DEZOLIS_ECLIPSE + 4]
            .copy_from_slice(&0x009Bu16.to_be_bytes());
        assert!(extract_eclipse_torch_copy(&bad_flag).is_err());
    }
}
