//! `battle/art/enemy_overlays.json` and the replacement frames: the port of
//! `psiv_tools/battle_enemy_overlays.py`, whose module docs give the reasons.
//! `EnemySpriteMappingsOffs` lists, per enemy, the descriptors that copy
//! animated Art #2 tiles over a run of the body's Art #3 bank. A frame is the
//! whole body redrawn with one descriptor's tiles replaced, because a
//! replacement tile's colour zero clears the body pixel under it. Pointers
//! with bit 31 set are decoded but not drawn: the init routine disables
//! their sprite slot.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::enemies::{Record, Source, bank, bounds, mapping, records};
use super::{compose, enemy_palette, safe};
use crate::json::Json;
use crate::{PackError, png};

/// `ENEMY_SPRITE_MAPPING_TABLE`.
const TABLE: usize = 0x010FCE;
const ENTRY_COUNT: usize = 153;
const DATA_START: usize = 0x011100;
/// Each pinned region is held by its length and SHA-256, not its bytes: the
/// source carries no cartridge content, and the provenance below reports the
/// verified bytes as the ROM holds them.
const TABLE_PIN: (usize, &str) = (
    4,
    "d9f6c61dee63a66347c722a426f939a835a39ca8ccc55eac6a01c90783d8091d",
);
/// `EnemySprites_Animate`, pinned by its first opcodes.
const ANIMATE: usize = 0x040F66;
const ANIMATE_PIN: (usize, &str) = (
    12,
    "b0755648ab3345540abc2326f36cccd1b4e58a97ae328199e6641a8d65364416",
);
/// `Mappings_ZoranBult`, which the tenth entry must reach.
const ZORAN: usize = 0x01190C;
const ZORAN_PIN: (usize, &str) = (
    20,
    "79ec0f5fdadedb652be1147e7a7b93a356722481f307174e81cc19d1d6f619a9",
);
const PNG_DIRECTORY: &str = "battle/art/enemy_overlays";
const DISABLED: u32 = 0x8000_0000;

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn hex(bytes: &[u8]) -> String {
    crate::png::hex(bytes)
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn slice<'a>(rom: &'a [u8], at: usize, len: usize, what: &str) -> Result<&'a [u8], PackError> {
    rom.get(at..at + len).ok_or_else(|| {
        PackError(format!(
            "{what} at 0x{at:06X} (+{len}) runs past the end of the ROM"
        ))
    })
}

fn word(rom: &[u8], at: usize, what: &str) -> Result<u16, PackError> {
    let b = slice(rom, at, 2, what)?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

fn long(rom: &[u8], at: usize, what: &str) -> Result<u32, PackError> {
    let b = slice(rom, at, 4, what)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// A pinned region of the ROM, refused unless its digest matches.
fn pinned<'a>(
    rom: &'a [u8],
    at: usize,
    (len, digest): (usize, &str),
    what: &str,
    refusal: &str,
) -> Result<&'a [u8], PackError> {
    let bytes = slice(rom, at, len, what)?;
    if sha256(bytes) != digest {
        return refuse(format!("{refusal} ({})", hex(bytes)));
    }
    Ok(bytes)
}

/// `verify_enemy_overlay_sources`: the table, ZoranBult's entry and the
/// animation routine, pinned before anything is decoded.
fn provenance(rom: &[u8]) -> Result<Json, PackError> {
    let first = pinned(
        rom,
        TABLE,
        TABLE_PIN,
        "EnemySpriteMappingsOffs",
        &format!("EnemySpriteMappingsOffs at 0x{TABLE:06X}: retail signature changed"),
    )?;
    let zoran = TABLE + usize::from(word(rom, TABLE + 10 * 2, "ZoranBult table entry")?);
    if zoran != ZORAN {
        return refuse(format!(
            "EnemySpriteMappingsOffs ZoranBult entry points to 0x{zoran:06X}, expected \
             0x{ZORAN:06X}"
        ));
    }
    let descriptors = pinned(
        rom,
        ZORAN,
        ZORAN_PIN,
        "Mappings_ZoranBult",
        "Mappings_ZoranBult: retail descriptor signature changed",
    )?;
    let animate = pinned(
        rom,
        ANIMATE,
        ANIMATE_PIN,
        "EnemySprites_Animate",
        "EnemySprites_Animate: retail opcode signature changed",
    )?;
    Ok(Json::object()
        .with("rom_sha256", sha256(rom))
        .with("table_signature", hex(first))
        .with(
            "table_signature_sha256",
            sha256(slice(rom, TABLE, 8, "EnemySpriteMappingsOffs")?),
        )
        .with("zoran_mapping_signature", hex(descriptors))
        .with(
            "animation_routine",
            Json::object()
                .with("label", "EnemySprites_Animate")
                .with("rom_offset", hex6(ANIMATE))
                .with("signature", hex(animate))
                .with("signature_sha256", sha256(animate)),
        ))
}

/// One descriptor's sequence (`_decode_sequence`).
struct Sequence {
    pointer: usize,
    data: usize,
    compact: bool,
    cadence: Option<u8>,
    sources: Vec<u8>,
    durations: Vec<u8>,
}

fn sequence(rom: &[u8], raw: u32, label: &str) -> Result<Sequence, PackError> {
    let pointer = (raw & 0x7FFF_FFFF) as usize;
    if pointer & 1 != 0 {
        return refuse(format!(
            "{label}: sequence wrapper 0x{pointer:06X} is not even"
        ));
    }
    let data = long(rom, pointer, &format!("{label} sequence wrapper"))? as usize;
    let first = slice(rom, data, 1, &format!("{label} sequence"))?[0];
    if first & 0x80 != 0 {
        let count = usize::from(first & 0x7F);
        if count == 0 {
            return refuse(format!("{label}: compact sequence has zero frames"));
        }
        let durations = slice(rom, data + 1, count, &format!("{label} durations"))?.to_vec();
        let sources = slice(rom, data + 1 + count, count, &format!("{label} sources"))?.to_vec();
        Ok(Sequence {
            pointer,
            data,
            compact: true,
            cadence: None,
            sources,
            durations,
        })
    } else {
        let count = usize::from(first);
        if count == 0 {
            return refuse(format!("{label}: ordinary sequence has zero frames"));
        }
        let cadence = slice(rom, data + 1, 1, &format!("{label} cadence"))?[0];
        let sources = slice(rom, data + 2, count, &format!("{label} sources"))?.to_vec();
        Ok(Sequence {
            pointer,
            data,
            compact: false,
            cadence: Some(cadence),
            durations: vec![cadence; count],
            sources,
        })
    }
}

/// `_placements`: the body rectangles whose cells name the descriptor's
/// destination run, in any of the four flips.
fn placements(
    words: &[u16],
    columns: usize,
    rows: usize,
    destination: usize,
    count: usize,
) -> Vec<Json> {
    let mut found = Vec::new();
    for height in 1..=rows {
        if !count.is_multiple_of(height) {
            continue;
        }
        let width = count / height;
        if width > columns {
            continue;
        }
        for y in 0..=rows - height {
            for x in 0..=columns - width {
                for flip_v in [false, true] {
                    for flip_h in [false, true] {
                        let mut indices = Vec::new();
                        let matched = (0..height).all(|row| {
                            (0..width).all(|column| {
                                let sx = if flip_h { width - 1 - column } else { column };
                                let sy = if flip_v { height - 1 - row } else { row };
                                let index = (y + row) * columns + x + column;
                                let cell = words[index];
                                let ok = usize::from(cell & 0x07FF)
                                    == destination + sy * width + sx
                                    && (cell & 0x0800 != 0) == flip_h
                                    && (cell & 0x1000 != 0) == flip_v;
                                if ok {
                                    indices.push(index);
                                }
                                ok
                            })
                        });
                        if matched {
                            found.push(
                                Json::object()
                                    .with("offset_cells", vec![x, y])
                                    .with("offset_pixels", vec![x * 8, y * 8])
                                    .with("size_cells", vec![width, height])
                                    .with("size_pixels", vec![width * 8, height * 8])
                                    .with("flip_h", flip_h)
                                    .with("flip_v", flip_v)
                                    .with("cell_indices", indices),
                            );
                        }
                    }
                }
            }
        }
    }
    found
}

/// One decoded descriptor (`OverlayPiece`).
struct Piece {
    index: usize,
    descriptor: usize,
    destination: usize,
    count: usize,
    raw: u32,
    sequence: Sequence,
    placements: Vec<Json>,
    source_range_valid: bool,
}

impl Piece {
    fn enabled(&self) -> bool {
        self.raw & DISABLED == 0
    }

    fn initial(&self) -> u8 {
        self.sequence.sources[0]
    }

    /// `runtime_order`: frames 1.. then 0, as the animator advances first.
    fn runtime_order(&self) -> Vec<usize> {
        let n = self.sequence.sources.len();
        if n <= 1 {
            vec![0]
        } else {
            (1..n).chain([0]).collect()
        }
    }

    fn metadata(&self, art2: &Json) -> Json {
        let s = &self.sequence;
        let order = self.runtime_order();
        let first = self.placements.first();
        let from_first = |key: &str| {
            first
                .and_then(|p| p.get(key))
                .cloned()
                .unwrap_or(Json::Null)
        };
        let cells: Vec<Json> = self
            .placements
            .iter()
            .flat_map(|p| match p.get("cell_indices") {
                Some(Json::Array(items)) => items.clone(),
                _ => Vec::new(),
            })
            .collect();
        Json::object()
            .with("piece", self.index)
            .with("descriptor_rom_offset", hex6(self.descriptor))
            .with("enabled", self.enabled())
            .with("pointer_flag_bit31", !self.enabled())
            .with("pointer_rom_value", format!("0x{:08X}", self.raw))
            .with("pointer", hex6(s.pointer))
            .with("sequence_data", hex6(s.data))
            .with("destination_tile", self.destination)
            .with("tile_count", self.count)
            .with("destination_tile_end", self.destination + self.count - 1)
            .with("tile_source", art2.clone())
            .with("tile_source_addressing", "Art #2 local pattern offsets")
            .with(
                "palette",
                Json::object()
                    .with("source", "enemy CRAM line")
                    .with("cram_lines", vec![1u8, 2])
                    .with("pixel_indices", "line-relative 0-15"),
            )
            .with(
                "sequence_format",
                if s.compact {
                    "compact_per_frame_duration"
                } else {
                    "constant_cadence"
                },
            )
            .with("frame_count", s.sources.len())
            .with("cadence", s.cadence)
            .with("cadence_ticks", s.cadence.map(|c| u16::from(c) + 1))
            .with("durations", s.durations.clone())
            .with(
                "duration_ticks",
                s.durations
                    .iter()
                    .map(|&d| u16::from(d) + 1)
                    .collect::<Vec<_>>(),
            )
            .with("source_patterns", s.sources.clone())
            .with("initial_source_pattern", self.initial())
            .with("runtime_order", order.clone())
            .with(
                "runtime_source_patterns",
                order.iter().map(|&i| s.sources[i]).collect::<Vec<_>>(),
            )
            .with(
                "runtime_durations",
                order.iter().map(|&i| s.durations[i]).collect::<Vec<_>>(),
            )
            .with("source_range_valid", self.source_range_valid)
            .with(
                "placement_status",
                if self.placements.is_empty() {
                    "not_in_body_mapping"
                } else {
                    "body_mapping_rectangle"
                },
            )
            .with("offset_pixels", from_first("offset_pixels"))
            .with("size_pixels", from_first("size_pixels"))
            .with("flip_h", from_first("flip_h"))
            .with("flip_v", from_first("flip_v"))
            .with("placements", self.placements.clone())
            .with("covered_body_cells", cells)
    }
}

/// One enemy's decode (`DecodedEnemyOverlay`).
struct Enemy {
    id: usize,
    symbol: &'static str,
    columns: usize,
    words: Vec<u16>,
    tiles: Vec<[u8; 64]>,
    art2: (usize, usize),
    art2_json: Json,
    holes: Vec<usize>,
    pieces: Vec<Piece>,
    mapping: usize,
    block: usize,
}

impl Enemy {
    fn art2_tile(&self, index: usize) -> Option<&[u8; 64]> {
        (index < self.art2.1).then(|| &self.tiles[self.art2.0 + index])
    }
}

/// A descriptor: its offset, destination tile, tile count and raw pointer.
type Descriptor = (usize, u8, u8, u32);

/// `_block_for_enemy`: the block and its six-byte descriptors.
fn block(rom: &[u8], id: usize) -> Result<(usize, Vec<Descriptor>), PackError> {
    let block = TABLE + usize::from(word(rom, TABLE + id * 2, "enemy sprite table entry")?);
    if block < DATA_START || block & 1 != 0 {
        return refuse(format!(
            "enemy 0x{id:02X}: EnemySpriteMappingsOffs entry reaches invalid block 0x{block:06X}"
        ));
    }
    let count = usize::from(word(rom, block, "enemy sprite descriptor count")?) + 1;
    let mut cursor = block + 2;
    let mut descriptors = Vec::new();
    for index in 0..count {
        let raw = slice(
            rom,
            cursor,
            6,
            &format!("enemy 0x{id:02X} descriptor {index}"),
        )?;
        descriptors.push((
            cursor,
            raw[0],
            raw[1],
            u32::from_be_bytes([raw[2], raw[3], raw[4], raw[5]]),
        ));
        cursor += 6;
    }
    Ok((block, descriptors))
}

fn art2_json(source: &Source) -> Json {
    Json::object()
        .with("field", format!("+0x{:02X}", 2 + 4 * source.field))
        .with("rom_offset", hex6(source.offset))
        .with("first_pattern", source.first_pattern)
        .with("tile_count", source.tile_count)
}

/// `decode_enemy_overlay_records`: every enemy, plus the decoded end.
fn decode(rom: &[u8], records: &[Record]) -> Result<(Vec<Enemy>, usize), PackError> {
    let bounds = bounds(records, rom.len());
    let mut max_end = DATA_START;
    let mut out = Vec::new();
    for record in records {
        let id = record.id;
        let (tiles, sources) = bank(rom, record, &bounds)?;
        let (words, _, columns) = mapping(rom, record)?;
        let source = &sources[1];
        let art2 = (source.first_pattern, usize::from(source.tile_count));
        let holes = words
            .iter()
            .enumerate()
            .filter_map(|(index, &w)| {
                let tile = usize::from(w & 0x07FF);
                (tile != 0 && tiles.get(tile) == Some(&[0; 64])).then_some(index)
            })
            .collect();
        let (block, descriptors) = block(rom, id)?;
        max_end = max_end.max(block + 2 + 6 * descriptors.len());
        let mut pieces = Vec::new();
        for (index, (descriptor, destination, count, raw)) in descriptors.into_iter().enumerate() {
            let label = format!("enemy 0x{id:02X} piece {index}");
            let sequence = sequence(rom, raw, &label)?;
            let n = sequence.sources.len();
            max_end = max_end.max(
                sequence.data
                    + 1
                    + if sequence.cadence.is_none() {
                        n * 2
                    } else {
                        1 + n
                    },
            );
            let (destination, count) = (usize::from(destination), usize::from(count));
            let valid = sequence
                .sources
                .iter()
                .all(|&s| usize::from(s) + count <= art2.1);
            if raw & DISABLED == 0 && !valid {
                return refuse(format!(
                    "{label}: active Art #2 source range exceeds its {}-tile blob",
                    art2.1
                ));
            }
            if destination + count > tiles.len() {
                return refuse(format!(
                    "{label}: destination {destination}+{count} exceeds its {}-tile bank",
                    tiles.len()
                ));
            }
            pieces.push(Piece {
                index,
                descriptor,
                destination,
                count,
                raw,
                placements: placements(&words, columns, words.len() / columns, destination, count),
                sequence,
                source_range_valid: valid,
            });
        }
        out.push(Enemy {
            id,
            symbol: record.symbol,
            columns,
            words,
            tiles,
            art2,
            art2_json: art2_json(source),
            holes,
            pieces,
            mapping: record.mapping,
            block,
        });
    }
    Ok((out, max_end))
}

/// The enabled pieces whose destination run holds `tile`.
fn covering(enemy: &Enemy, tile: usize) -> impl Iterator<Item = &Piece> {
    enemy
        .pieces
        .iter()
        .filter(move |p| p.enabled() && p.destination <= tile && tile < p.destination + p.count)
}

/// `_coverage`.
fn coverage(enemies: &[Enemy]) -> Json {
    let mut covered_ids = Vec::new();
    let mut all_frame_ids = Vec::new();
    let (mut total_covered, mut total_all_frame, mut remaining) = (0, 0, 0);
    for enemy in enemies {
        let (mut covered, mut all_frame) = (0, 0);
        for &cell in &enemy.holes {
            let tile = usize::from(enemy.words[cell] & 0x07FF);
            let pieces: Vec<&Piece> = covering(enemy, tile).collect();
            if pieces.is_empty() {
                remaining += 1;
                continue;
            }
            covered += 1;
            let every_frame = pieces.iter().any(|piece| {
                let offset = tile - piece.destination;
                let values: Vec<&[u8; 64]> = piece
                    .sequence
                    .sources
                    .iter()
                    .chain([&piece.initial()])
                    .filter_map(|&s| enemy.art2_tile(usize::from(s) + offset))
                    .collect();
                !values.is_empty() && values.iter().all(|v| **v != [0; 64])
            });
            if every_frame {
                all_frame += 1;
            }
        }
        total_covered += covered;
        total_all_frame += all_frame;
        if !enemy.holes.is_empty() && covered == enemy.holes.len() {
            covered_ids.push(enemy.id);
        }
        if !enemy.holes.is_empty() && all_frame == enemy.holes.len() {
            all_frame_ids.push(enemy.id);
        }
    }
    let holed: Vec<usize> = enemies
        .iter()
        .filter(|e| !e.holes.is_empty())
        .map(|e| e.id)
        .collect();
    Json::object()
        .with("holed_enemy_count", holed.len())
        .with("holed_enemy_ids", holed)
        .with("holed_enemies_fully_covered_by_destination", covered_ids)
        .with(
            "holed_enemies_nonblank_in_every_decoded_frame",
            all_frame_ids,
        )
        .with(
            "total_body_holes",
            enemies.iter().map(|e| e.holes.len()).sum::<usize>(),
        )
        .with("total_holes_covered_by_destination", total_covered)
        .with(
            "total_holes_nonblank_in_every_decoded_frame",
            total_all_frame,
        )
        .with("remaining_holes_by_destination", remaining)
}

/// `render_overlay_piece_frame`: the body with one state's tiles in place.
fn frame(rom: &[u8], enemy: &Enemy, piece: &Piece, source: u8) -> Result<Vec<u8>, PackError> {
    let source = usize::from(source);
    if source + piece.count > enemy.art2.1 {
        return refuse(format!(
            "enemy 0x{:02X} piece {}: source frame {source}+{} is outside Art #2",
            enemy.id, piece.index, piece.count
        ));
    }
    let mut tiles = enemy.tiles.clone();
    for i in 0..piece.count {
        tiles[piece.destination + i] = enemy.tiles[enemy.art2.0 + source + i];
    }
    let (width, height, pixels) = compose(&enemy.words, enemy.columns, &tiles, 0)?;
    Ok(png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        &enemy_palette(rom, enemy.id)?,
        &[0],
    )?)
}

/// `emit_enemy_overlays`: write every enabled piece's frames and return the
/// index.
pub fn write(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let mut provenance = provenance(rom)?;
    let records = records(rom)?;
    let (enemies, max_end) = decode(rom, &records)?;
    provenance.insert(
        "table",
        Json::object()
            .with("label", "EnemySpriteMappingsOffs")
            .with("rom_offset", hex6(TABLE))
            .with("entry_count", ENTRY_COUNT)
            .with("entry_size", 2u8)
            .with("data_start", hex6(DATA_START))
            .with("decoded_data_end_exclusive", hex6(max_end)),
    );
    provenance.insert(
        "descriptor_format",
        "dc.w descriptor_count_minus_1; dc.b destination_tile, tile_count; dc.l sequence_pointer",
    );
    provenance.insert("pointer_flag", "bit31 disables the Enemy_Sprites RAM slot");
    provenance.insert(
        "source_art",
        "ArtNem enemy field +0x06 (Art #2), 32-byte tiles",
    );
    provenance.insert(
        "destination_art",
        "Art #3 staging bank, 32-byte tile offsets",
    );
    provenance.insert(
        "animation_decode",
        Json::object()
            .with("ordinary", "count, cadence, source_pattern[count]")
            .with(
                "compact",
                "0x80|count, duration[count], source_pattern[count]",
            )
            .with(
                "initial_frame",
                "loc_7C56 startup source; EnemySprites_Animate advances frame then wraps",
            )
            .with("timer_ticks", "stored_duration + 1 update calls"),
    );
    let mut entries = Vec::new();
    for enemy in &enemies {
        let mut pieces = Vec::new();
        for piece in &enemy.pieces {
            let mut meta = piece.metadata(&enemy.art2_json);
            if !piece.enabled() {
                meta.insert("initial_png", Json::Null);
                meta.insert("frames", Vec::<Json>::new());
                pieces.push(meta);
                continue;
            }
            let prefix = format!(
                "{PNG_DIRECTORY}/{:03}_{}_piece{:02}",
                enemy.id,
                safe(enemy.symbol),
                piece.index
            );
            let initial = frame(rom, enemy, piece, piece.initial())?;
            let initial_path = format!("{prefix}_initial.png");
            crate::write_file(root, &initial_path, &initial)?;
            meta.insert("initial_png", initial_path);
            meta.insert("initial_png_sha256", png::image_sha256(&initial)?);
            let mut frames = Vec::new();
            for (number, &source) in piece.sequence.sources.iter().enumerate() {
                let image = frame(rom, enemy, piece, source)?;
                let path = format!("{prefix}_frame{number:02}.png");
                crate::write_file(root, &path, &image)?;
                let duration = piece.sequence.durations[number];
                frames.push(
                    Json::object()
                        .with("frame", number)
                        .with("source_pattern", source)
                        .with("duration", duration)
                        .with("duration_ticks", u16::from(duration) + 1)
                        .with("png", path)
                        .with("png_sha256", png::image_sha256(&image)?),
                );
            }
            meta.insert("frames", frames);
            pieces.push(meta);
        }
        entries.push(
            Json::object()
                .with("id", enemy.id)
                .with("symbol", enemy.symbol)
                .with("mapping_rom_offset", hex6(enemy.mapping))
                .with("mapping_block_rom_offset", hex6(enemy.block))
                .with("piece_count", enemy.pieces.len())
                .with(
                    "enabled_piece_count",
                    enemy.pieces.iter().filter(|p| p.enabled()).count(),
                )
                .with("body_holes", enemy.holes.len())
                .with("body_hole_cells", enemy.holes.clone())
                .with("pieces", pieces),
        );
    }
    let blocks: BTreeSet<usize> = enemies.iter().map(|e| e.block).collect();
    Ok(Json::object()
        .with("format_version", crate::battle::FORMAT_VERSION)
        .with("kind", "battle_enemy_overlay_art")
        .with(
            "table",
            provenance.get("table").cloned().unwrap_or(Json::Null),
        )
        .with("provenance", provenance)
        .with(
            "piece_count",
            enemies.iter().map(|e| e.pieces.len()).sum::<usize>(),
        )
        .with(
            "enabled_piece_count",
            enemies
                .iter()
                .flat_map(|e| &e.pieces)
                .filter(|p| p.enabled())
                .count(),
        )
        .with("distinct_mapping_blocks", blocks.len())
        .with("coverage", coverage(&enemies))
        .with("enemies", entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pinned_region_must_match_its_digest() {
        let rom = [1u8, 2, 3, 4];
        let pin = (
            2,
            "a12871fee210fb8619291eaea194581cbd2531e4b23759d225f6806923f63222",
        );
        assert_eq!(pinned(&rom, 0, pin, "x", "changed").unwrap(), [1, 2]);
        let error = pinned(&rom, 1, pin, "x", "changed").err().unwrap();
        assert_eq!(error.0, "changed (0203)");
        assert!(pinned(&rom, 3, pin, "x", "changed").is_err());
    }

    #[test]
    fn a_placement_finds_the_run_in_any_flip() {
        // A 4x1 body: tiles 5,6 then 6,5 mirrored.
        let words = [5u16, 6, 0x0800 | 6, 0x0800 | 5];
        let found = placements(&words, 4, 1, 5, 2);
        let found: Vec<serde_json::Value> = found.iter().map(serde_json::Value::from).collect();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0]["offset_cells"], serde_json::json!([0, 0]));
        assert_eq!(found[1]["flip_h"], true);
        assert_eq!(found[1]["cell_indices"], serde_json::json!([2, 3]));
    }

    #[test]
    fn a_compact_sequence_carries_its_own_durations() {
        let mut rom = vec![0u8; 0x40];
        rom[0x10..0x14].copy_from_slice(&0x20u32.to_be_bytes());
        rom[0x20..0x25].copy_from_slice(&[0x82, 3, 7, 1, 2]);
        let s = sequence(&rom, 0x8000_0010, "test").unwrap();
        assert!(s.compact && s.cadence.is_none());
        assert_eq!((s.durations, s.sources), (vec![3, 7], vec![1, 2]));
        rom[0x20..0x24].copy_from_slice(&[2, 5, 4, 9]);
        let s = sequence(&rom, 0x10, "test").unwrap();
        assert_eq!(
            (s.cadence, s.durations, s.sources),
            (Some(5), vec![5, 5], vec![4, 9])
        );
        rom[0x20] = 0x80;
        assert!(sequence(&rom, 0x10, "test").is_err());
        assert!(sequence(&rom, 0x11, "test").is_err());
    }

    #[test]
    fn a_vertically_flipped_run_is_found_bottom_up() {
        // A 1x2 body: tile 6 above tile 5, both v-flipped.
        let words = [0x1000 | 6u16, 0x1000 | 5];
        let found = placements(&words, 1, 2, 5, 2);
        let found: Vec<serde_json::Value> = found.iter().map(serde_json::Value::from).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0]["flip_v"].clone(), found[0]["flip_h"].clone()),
            (true.into(), false.into())
        );
        assert_eq!(found[0]["cell_indices"], serde_json::json!([0, 1]));
    }
}
