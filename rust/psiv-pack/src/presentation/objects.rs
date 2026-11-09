//! The scene art beside the panels: every `LoadArt` site's preview, the
//! objects scenes build out of it, and the shopkeeper portrait
//! (`_emit_load_art`, `_emit_temporary_objects` and `_emit_generic_portrait`
//! in `psiv_tools/presentation_pack.py`, which give the reasons).
//!
//! `panels.json` keeps Python's dict order, so each record here is built in
//! the order its Python dict is: the spec's keys first, then what is added.

use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::decode::{Decodes, nemesis};
use crate::json::Json;
use crate::maps::ExtractedMaps;
use crate::maps::layouts::{decode_tilesets, map_palette_rgb};
use crate::maps::records::MapRecord;
use crate::sprites::compose::{Reading, Sheet, TileSource, compose_frame, mapping_box};
use crate::sprites::emit::sheet_png;
use crate::sprites::field::{
    PAL_INIT_LINE_3_CRAM_LINE, SheetRequest, StagedArt, object_sprite, pal_init_line_3,
    sheet_for_sequences, sprite_palette, stage_map_art, table_sequences, tile_source_from_patterns,
};
use crate::sprites::objects::FieldObjectRoutine;
use crate::sprites::records::{decode_mapping, decode_sequence};
use crate::{PackError, gfx, png};

/// `RAW_FIELD_ART_BYTES`: a party member's field art, which has no length
/// of its own.
const RAW_FIELD_ART_BYTES: usize = 2304;
/// `_write_art_preview`'s CRAM line.
const PREVIEW_PALETTE_LINE: u8 = 3;
const SPRITE_TRANSPARENT_INDEX: u8 = 0;
/// The VRAM patterns `_partial_sheet` fills.
const VRAM_PATTERNS: usize = 0x800;

/// `LOAD_ART_RECORDS`: every `LoadArt` site in the transcribed scenes.
struct LoadArt {
    scene: &'static str,
    source: usize,
    destination_tile: u16,
    map_id: usize,
    object_ids: &'static [usize],
}

const LOAD_ART_RECORDS: [LoadArt; 7] = [
    LoadArt {
        scene: "Event_PsycoWandChest",
        source: 0x1D5DF0,
        destination_tile: 0x02E6,
        map_id: 0x087,
        object_ids: &[0x01FC],
    },
    LoadArt {
        scene: "Event_RuneFlaeli",
        source: 0x1D628C,
        destination_tile: 0x0179,
        map_id: 0x19F,
        object_ids: &[0x0214],
    },
    LoadArt {
        scene: "Event_RuneFlaeli",
        source: 0x1D642E,
        destination_tile: 0x0191,
        map_id: 0x19F,
        object_ids: &[],
    },
    LoadArt {
        scene: "Event_RuneFlaeli",
        source: 0x1D6A2E,
        destination_tile: 0x01FB,
        map_id: 0x19F,
        object_ids: &[],
    },
    LoadArt {
        scene: "Event_RuneFlaeli",
        source: 0x1D6BC8,
        destination_tile: 0x020C,
        map_id: 0x19F,
        object_ids: &[],
    },
    LoadArt {
        scene: "Event_Alshline",
        source: 0x12951A,
        destination_tile: 0x03A5,
        map_id: 0x024,
        object_ids: &[0x0188],
    },
    LoadArt {
        scene: "MeetingRika",
        source: 0x1289FA,
        destination_tile: 0x04A5,
        map_id: 0x024,
        object_ids: &[0x0194],
    },
];

/// How a temporary object's art reaches it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    /// Streamed from work RAM, where the scene copied the ROM blob.
    RawFieldArt,
    /// The preceding `LoadArt` upload.
    Nemesis,
}

/// `playback_sequence` (and `playback_once`), and whether the Python spec
/// names it before `art_tile`, as Igglanova's does.
struct Playback {
    sequence: &'static str,
    once: bool,
    before_art_tile: bool,
}

/// `TEMPORARY_OBJECT_RECORDS`: keyed by the literal `(object id, art tile)`
/// pair a scene passes to `ObjectAnimation`.
struct Temporary {
    scene: &'static str,
    object_id: usize,
    art_tile: u16,
    symbol: &'static str,
    playback: Option<Playback>,
    source: usize,
    kind: SourceKind,
    map_id: usize,
    load_art: bool,
}

const fn temporary(
    scene: &'static str,
    (object_id, art_tile): (usize, u16),
    symbol: &'static str,
    playback: Option<Playback>,
    (source, kind): (usize, SourceKind),
    map_id: usize,
) -> Temporary {
    Temporary {
        scene,
        object_id,
        art_tile,
        symbol,
        playback,
        source,
        kind,
        map_id,
        load_art: matches!(kind, SourceKind::Nemesis),
    }
}

const fn plays(sequence: &'static str, once: bool, before_art_tile: bool) -> Option<Playback> {
    Some(Playback {
        sequence,
        once,
        before_art_tile,
    })
}

const TEMPORARY_OBJECT_RECORDS: [Temporary; 9] = [
    temporary(
        "MeetingRika",
        (0x0018, 0x026A),
        "Rika",
        None,
        (0x292D00, SourceKind::RawFieldArt),
        0x0AC,
    ),
    temporary(
        "MeetingRika",
        (0x0018, 0x055C),
        "Rika",
        None,
        (0x292D00, SourceKind::RawFieldArt),
        0x0AC,
    ),
    temporary(
        "MeetingRika",
        (0x0194, 0x04A5),
        "Holt",
        None,
        (0x1289FA, SourceKind::Nemesis),
        0x024,
    ),
    temporary(
        "Event_RuneFlaeli",
        (0x0214, 0x0179),
        "RuneFlaeli",
        plays("walk_down", true, false),
        (0x1D628C, SourceKind::Nemesis),
        0x0D8,
    ),
    temporary(
        "Event_RuneFlaeli",
        (0x0218, 0x0191),
        "Flaeli",
        plays("walk_up", true, false),
        (0x1D642E, SourceKind::Nemesis),
        0x0D8,
    ),
    temporary(
        "Event_RuneFlaeli",
        (0x021C, 0x01FB),
        "BlastedRock",
        None,
        (0x1D6A2E, SourceKind::Nemesis),
        0x0D8,
    ),
    temporary(
        "Event_RuneFlaeli",
        (0x0220, 0x020C),
        "RockShock",
        None,
        (0x1D6BC8, SourceKind::Nemesis),
        0x0D8,
    ),
    temporary(
        "Event_Alshline",
        (0x0188, 0x03A5),
        "Igglanova",
        plays("walk_down", false, true),
        (0x12951A, SourceKind::Nemesis),
        0x024,
    ),
    temporary(
        "Event_PsycoWandChest",
        (0x01FC, 0x02E6),
        "ChestBarrierSplinter",
        None,
        (0x1D5DF0, SourceKind::Nemesis),
        0x087,
    ),
];

/// `Event_RuneFlaeli` uploads all four blobs before it builds an object.
const RUNE_FLAELI_UPLOADS: [(usize, usize); 4] = [
    (0x1D628C, 0x179),
    (0x1D642E, 0x191),
    (0x1D6A2E, 0x1FB),
    (0x1D6BC8, 0x20C),
];
/// ...and replaces CRAM line 3 with sixteen words from here.
const RUNE_FLAELI_LINE_3: usize = 0x1DE0B8;
/// `MapDataMan_PiataBasementB2_Palettes`: fourteen CRAM line-3 words
/// `Cutscene_Alshline` loads after the monster art.
const ALSHLINE_LINE_3: usize = 0x52848;
const ALSHLINE_LINE_3_WORDS: usize = 14;
const IGGLANOVA: usize = 0x0188;
const CHEST_BARRIER_SPLINTER: usize = 0x01FC;

const SHOPKEEPER_PORTRAIT_ART: usize = 0x29DE1E;
const SHOPKEEPER_PORTRAIT_MAPPING: usize = 0x2A2B36;
const SHOPKEEPER_PORTRAIT_TILE: u16 = 0x055C;
const SHOPKEEPER_PORTRAIT_TILES: usize = 36;
const SHOPKEEPER_PORTRAIT_PALETTE: usize = 0x296300;

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn sha256(data: &[u8]) -> String {
    png::hex(&Sha256::digest(data))
}

/// What the scene objects read: the map records, the field-object routines
/// and their facing tables.
pub struct Inputs<'a> {
    pub decodes: &'a Decodes<'a>,
    pub maps: &'a ExtractedMaps,
    pub routines: HashMap<usize, FieldObjectRoutine>,
    pub extents: HashMap<u32, usize>,
}

impl Inputs<'_> {
    fn map(&self, id: usize) -> Option<&MapRecord> {
        self.maps.real().find(|record| record.id == id)
    }

    fn routine(&self, id: usize) -> Result<&FieldObjectRoutine, PackError> {
        self.routines
            .get(&id)
            .ok_or_else(|| PackError(format!("no field-object routine 0x{id:04X}")))
    }
}

fn palette_words(rom: &[u8], at: usize, words: usize) -> Result<Vec<[u8; 3]>, PackError> {
    let raw = rom
        .get(at..at + words * 2)
        .ok_or_else(|| PackError(format!("palette at 0x{at:06X} runs past the ROM")))?;
    Ok(gfx::palette_rgb(&gfx::decode_palette(raw)?))
}

/// `_write_art_preview`: the blob's patterns, sixteen across, on CRAM line 3
/// of the map live at the site (`Pal_Init_Line_3` without one).
fn preview(
    rom: &[u8],
    inputs: &Inputs,
    spec: &LoadArt,
    raw: &[u8],
    root: &Path,
    path: &str,
) -> Result<Json, PackError> {
    let tiles = gfx::decode_tiles(raw)?;
    let columns = tiles.len().clamp(1, 16);
    let (width, height, pixels) = gfx::compose_sheet(&tiles, columns)?;
    let palette = match inputs.map(spec.map_id) {
        Some(record) => sprite_palette(
            rom,
            &map_palette_rgb(rom, record.palette.pointer)?,
            PREVIEW_PALETTE_LINE,
        )?,
        None => pal_init_line_3(rom)?,
    };
    let image = png::encode_indexed(width as u32, height as u32, &pixels, &palette, &[0])?;
    crate::write_file(root, path, &image)?;
    let symbols: Vec<&str> = spec
        .object_ids
        .iter()
        .filter_map(|id| inputs.routines.get(id).map(|routine| routine.symbol))
        .collect();
    Ok(Json::object()
        .with("png", path)
        .with("png_sha256", png::image_sha256(&image)?)
        .with("size_pixels", vec![width, height])
        .with("tile_count", tiles.len())
        .with("preview_palette_line", PREVIEW_PALETTE_LINE)
        .with("object_symbols", symbols))
}

/// `_emit_load_art`.
pub fn load_art(rom: &[u8], root: &Path, inputs: &Inputs) -> Result<Vec<Json>, PackError> {
    let mut records = Vec::with_capacity(LOAD_ART_RECORDS.len());
    for spec in &LOAD_ART_RECORDS {
        let (raw, consumed) = nemesis::decompress(rom, spec.source)?;
        let path = format!(
            "presentation/load_art/load_art_{:03x}.png",
            spec.destination_tile
        );
        let shown = preview(rom, inputs, spec, &raw, root, &path)?;
        let mut record = Json::object()
            .with("scene", spec.scene)
            .with("source_rom_addr", hex6(spec.source))
            .with(
                "destination_tile",
                format!("0x{:04X}", spec.destination_tile),
            )
            .with("map_id", format!("0x{:03X}", spec.map_id))
            .with("object_ids", spec.object_ids.to_vec())
            .with("compressed_size", consumed)
            .with("decompressed_size", raw.len())
            .with("compression", "nemesis")
            .with("decompressed_sha256", sha256(&raw));
        crate::json::overlay(&mut record, &shown);
        records.push(record);
    }
    Ok(records)
}

/// `_art_blob`: the bytes, the length read, and the compression.
fn art_blob(rom: &[u8], spec: &Temporary) -> Result<(Vec<u8>, usize, &'static str), PackError> {
    match spec.kind {
        SourceKind::RawFieldArt => {
            let raw = rom
                .get(spec.source..spec.source + RAW_FIELD_ART_BYTES)
                .ok_or_else(|| {
                    PackError(format!(
                        "raw field art at 0x{:06X} is truncated",
                        spec.source
                    ))
                })?;
            Ok((raw.to_vec(), raw.len(), "none"))
        }
        SourceKind::Nemesis => {
            let (raw, consumed) = nemesis::decompress(rom, spec.source)?;
            Ok((raw, consumed, "nemesis"))
        }
    }
}

/// `_missing_patterns_for_sequences`: every VRAM pattern a sequence maps
/// outside the staged bank, sorted.
fn missing_patterns(
    rom: &[u8],
    wanted: &[(String, usize)],
    source: &TileSource,
    reading: Reading,
) -> Result<Vec<u16>, PackError> {
    let mut missing = Vec::new();
    for (_, offset) in wanted {
        for (mapping, _) in decode_sequence(rom, *offset)?.frames {
            let mapping = decode_mapping(rom, mapping)?;
            let (_, holes) = compose_frame(&mapping, source, mapping_box(&mapping), reading, None);
            missing.extend(holes);
        }
    }
    missing.sort_unstable();
    missing.dedup();
    Ok(missing)
}

/// `_partial_sheet`: the chest splinter's sheet with the patterns its
/// mappings name outside the upload drawn transparent.
fn partial_sheet(
    rom: &[u8],
    wanted: &[(String, usize)],
    art: &StagedArt,
    reading: Reading,
    palette: &[[u8; 3]],
    palette_line: u8,
) -> Result<Sheet, PackError> {
    let mut filled = TileSource::default();
    for index in 0..VRAM_PATTERNS {
        filled.insert(index, art.vram.tile(index).copied().unwrap_or([0; 64]));
    }
    let required: Vec<&str> = wanted.iter().map(|(name, _)| name.as_str()).collect();
    sheet_for_sequences(
        rom,
        wanted,
        &SheetRequest {
            source: &filled,
            reading,
            palette,
            palette_line,
            required: &required,
        },
        None,
    )
}

/// `sheet_json`, in its own key order.
fn sheet_json(sheet: &Sheet, path: &str, image: &[u8]) -> Result<Json, PackError> {
    let source = if sheet.palette_line == PAL_INIT_LINE_3_CRAM_LINE {
        "Pal_Init_Line_3"
    } else {
        "map palette blob"
    };
    let colours: Vec<Json> = sheet.palette.iter().map(|c| c.to_vec().into()).collect();
    let mirrors = Json::Object(
        sheet
            .mirrors
            .iter()
            .map(|(index, other)| (index.to_string(), Json::from(*other)))
            .collect(),
    );
    let sequences = Json::Object(
        sheet
            .sequences
            .iter()
            .map(|(name, sequence)| {
                let frames: Vec<Json> = sequence
                    .frames
                    .iter()
                    .zip(&sequence.durations)
                    .map(|(&index, &duration)| {
                        Json::object()
                            .with("index", index)
                            .with("duration_ticks", duration)
                    })
                    .collect();
                let record = Json::object()
                    .with("frames", frames)
                    .with("loop", true)
                    .with("sequence_offset", hex6(sequence.sequence_offset));
                (name.clone(), record)
            })
            .collect(),
    );
    Ok(Json::object()
        .with("png", path)
        .with("png_sha256", png::image_sha256(image)?)
        .with("frame_width", sheet.bounds.width())
        .with("frame_height", sheet.bounds.height())
        .with("frame_count", sheet.frames.len())
        .with("origin_x", -sheet.bounds.left)
        .with("origin_y", -sheet.bounds.top)
        .with(
            "palette",
            Json::object()
                .with("cram_line", sheet.palette_line)
                .with("source", source)
                .with("transparent_index", SPRITE_TRANSPARENT_INDEX)
                .with("colors", colours),
        )
        .with("mirrors", mirrors)
        .with("sequences", sequences))
}

/// The spec's own keys, in `TEMPORARY_OBJECT_RECORDS`' order, with
/// `source_kind` dropped as `_emit_temporary_objects` drops it.
fn spec_json(spec: &Temporary) -> Json {
    let mut record = Json::object()
        .with("scene", spec.scene)
        .with("object_id", spec.object_id);
    let playback = |record: Json, playback: &Playback| {
        let record = record.with("playback_sequence", playback.sequence);
        if playback.once {
            record.with("playback_once", true)
        } else {
            record
        }
    };
    if let Some(p) = spec.playback.as_ref().filter(|p| p.before_art_tile) {
        record = playback(record, p);
    }
    record = record
        .with("art_tile", spec.art_tile)
        .with("symbol", spec.symbol);
    if let Some(p) = spec.playback.as_ref().filter(|p| !p.before_art_tile) {
        record = playback(record, p);
    }
    let load_art = spec.load_art.then(|| {
        Json::object()
            .with("source_rom_addr", hex6(spec.source))
            .with("destination_tile", format!("0x{:04X}", spec.art_tile))
    });
    record
        .with("source_rom_addr", hex6(spec.source))
        .with("map_id", format!("0x{:03X}", spec.map_id))
        .with("load_art", load_art)
}

/// `_emit_temporary_objects`.
pub fn temporary_objects(rom: &[u8], root: &Path, inputs: &Inputs) -> Result<Vec<Json>, PackError> {
    let mut records = Vec::with_capacity(TEMPORARY_OBJECT_RECORDS.len());
    for spec in &TEMPORARY_OBJECT_RECORDS {
        let routine = inputs.routine(spec.object_id)?;
        let (raw, consumed, compression) = art_blob(rom, spec)?;
        let map = inputs
            .map(spec.map_id)
            .ok_or_else(|| PackError(format!("map 0x{:03X} has no record", spec.map_id)))?;
        let specs: Vec<(usize, usize)> = map
            .tilesets
            .entries
            .iter()
            .map(|entry| (usize::from(entry.word), entry.source))
            .collect();
        let patterns = decode_tilesets(rom, inputs.decodes, &specs)?;
        let mut art = stage_map_art(inputs.decodes, map, tile_source_from_patterns(&patterns))?;
        match spec.kind {
            SourceKind::RawFieldArt => {
                let mut source = TileSource::default();
                source.add(0, &raw)?;
                art.ram.insert(spec.source as u32, source);
            }
            SourceKind::Nemesis => art.vram.add(usize::from(spec.art_tile), &raw)?,
        }
        let mut palette = map_palette_rgb(rom, map.palette.pointer)?;
        let mut palette_source = None;
        if spec.scene == "Event_RuneFlaeli" {
            // The map palette blob holds lines 0, 1 and 3; line 2 is the
            // party's own.
            for (address, tile) in RUNE_FLAELI_UPLOADS {
                let (loaded, _) = nemesis::decompress(rom, address)?;
                art.vram.add(tile, &loaded)?;
            }
            palette.splice(32..48, palette_words(rom, RUNE_FLAELI_LINE_3, 16)?);
            palette_source = Some("Event_RuneFlaeli CRAM line 3 at 0x1DE0B8");
        } else if spec.object_id == IGGLANOVA {
            // The last two words stay the map's.
            palette.splice(
                32..32 + ALSHLINE_LINE_3_WORDS,
                palette_words(rom, ALSHLINE_LINE_3, ALSHLINE_LINE_3_WORDS)?,
            );
            palette_source = Some(
                "Alshline Igglanova CRAM line 3: fourteen words at 0x052848, two retained map words",
            );
        }
        let rendered = object_sprite(
            rom,
            routine,
            &inputs.extents,
            &art,
            &palette,
            0,
            spec.art_tile,
            None,
        )?;
        let mut status = "exact";
        let mut missing = Vec::new();
        let mut sheet = rendered.sheet;
        if sheet.is_none()
            && spec.object_id == CHEST_BARRIER_SPLINTER
            && let Some(table) = routine.mappings_addr
        {
            let wanted = table_sequences(rom, table as usize, inputs.extents[&table])?;
            let line = routine.palette_line().unwrap_or(0);
            let reading = Reading {
                art_tile: spec.art_tile,
                tile_props: routine.sprite_tile_props.unwrap_or(0),
                streamed: false,
            };
            missing = missing_patterns(rom, &wanted, &art.vram, reading)?;
            let colours = sprite_palette(rom, &palette, line)?;
            sheet = Some(partial_sheet(rom, &wanted, &art, reading, &colours, line)?);
            status = "partial_transparent_holes";
        }
        let Some(sheet) = sheet else {
            return Err(PackError(format!(
                "temporary object 0x{:04X} did not render: {}",
                spec.object_id,
                rendered.reason.unwrap_or_default()
            )));
        };
        let image = sheet_png(&sheet)?;
        let path = format!(
            "presentation/temporary_objects/temporary_{:04x}_{:03x}.png",
            spec.object_id, spec.art_tile
        );
        crate::write_file(root, &path, &image)?;
        let mut record = spec_json(spec)
            .with("source_compression", compression)
            .with("source_size", raw.len())
            .with("source_consumed", consumed)
            .with("render_status", status)
            .with("missing_patterns", missing);
        crate::json::overlay(&mut record, &sheet_json(&sheet, &path, &image)?);
        let rune_line = spec.scene == "Event_RuneFlaeli" && sheet.palette_line == 3;
        if let Some(source) = palette_source.filter(|_| rune_line || spec.object_id == IGGLANOVA)
            && let Json::Object(pairs) = &mut record
            && let Some((_, palette)) = pairs.iter_mut().find(|(key, _)| key == "palette")
        {
            palette.insert("source", source);
        }
        records.push(record);
    }
    Ok(records)
}

/// `_emit_generic_portrait`: the shopkeeper's 36 patterns, six across.
pub fn portrait(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let (raw, consumed) = nemesis::decompress(rom, SHOPKEEPER_PORTRAIT_ART)?;
    let tiles = gfx::decode_tiles(&raw)?;
    if tiles.len() != SHOPKEEPER_PORTRAIT_TILES {
        return Err(PackError(format!(
            "shopkeeper portrait decoded {} tiles, expected {SHOPKEEPER_PORTRAIT_TILES}",
            tiles.len()
        )));
    }
    let (width, height, pixels) = gfx::compose_sheet(&tiles, 6)?;
    let image = png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        &pal_init_line_3(rom)?,
        &[0],
    )?;
    let path = "presentation/portraits/shopkeeper_2.png";
    crate::write_file(root, path, &image)?;
    Ok(Json::object()
        .with("id", "shopkeeper_2")
        .with("png", path)
        .with("png_sha256", png::image_sha256(&image)?)
        .with("size_pixels", vec![width, height])
        .with("art", hex6(SHOPKEEPER_PORTRAIT_ART))
        .with("art_consumed", consumed)
        .with("mapping", hex6(SHOPKEEPER_PORTRAIT_MAPPING))
        .with(
            "destination_tile",
            format!("0x{SHOPKEEPER_PORTRAIT_TILE:04X}"),
        )
        .with("tile_count", tiles.len())
        .with("palette", hex6(SHOPKEEPER_PORTRAIT_PALETTE)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(record: &Json) -> Vec<&str> {
        match record {
            Json::Object(pairs) => pairs.iter().map(|(key, _)| key.as_str()).collect(),
            _ => panic!("not an object"),
        }
    }

    #[test]
    fn a_temporary_record_keeps_its_python_spec_order() {
        let find = |symbol| {
            TEMPORARY_OBJECT_RECORDS
                .iter()
                .find(|spec| spec.symbol == symbol)
                .unwrap()
        };
        let common = ["source_rom_addr", "map_id", "load_art"];
        assert_eq!(
            keys(&spec_json(find("Igglanova"))),
            [
                [
                    "scene",
                    "object_id",
                    "playback_sequence",
                    "art_tile",
                    "symbol"
                ]
                .as_slice(),
                &common
            ]
            .concat()
        );
        assert_eq!(
            keys(&spec_json(find("Flaeli"))),
            [
                [
                    "scene",
                    "object_id",
                    "art_tile",
                    "symbol",
                    "playback_sequence",
                    "playback_once"
                ]
                .as_slice(),
                &common
            ]
            .concat()
        );
        let rika = spec_json(find("Rika"));
        assert_eq!(rika.get("load_art"), Some(&Json::Null));
        assert_eq!(
            keys(&rika),
            [
                ["scene", "object_id", "art_tile", "symbol"].as_slice(),
                &common
            ]
            .concat()
        );
    }
}
