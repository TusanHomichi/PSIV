//! The field-mode joins: palettes, the party, a map's staged art, vehicles and
//! chests: `psiv_tools/sprites/field.py`, `vehicles.py` and `chest_sprites.py`.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use sha2::{Digest, Sha256};

use super::compose::{
    Bounds, Reading, Sheet, SheetSequence, SpriteCensus, TileSource, build_sheet, compose_frame,
    mapping_box, union_box,
};
use super::objects::FieldObjectRoutine;
use super::records::{
    FACINGS, RAM_BASE, ROM_LIMIT, bounds, decode_mapping, decode_sequence, facing_name, hex6,
    u32_at,
};
use crate::PackError;
use crate::decode::{Call, Decodes};
use crate::gfx::{self, TILE_SIZE};
use crate::maps::layouts::TilePatterns;
use crate::maps::records::MapRecord;
use crate::png::hex;

const CHAR_FIELD_ART_PTRS: usize = 0x05E732;
const CHAR_SPRITE_MAPPINGS_PTRS: usize = 0x05E75E;
pub const PARTY_ART_BYTES: usize = 2304;
pub const PARTY_ART_TILES: usize = PARTY_ART_BYTES / TILE_SIZE;
const PAL_INIT_LINE_3: usize = 0x296300;
pub const PAL_INIT_LINE_3_CRAM_LINE: u8 = 2;
const PALETTE_LINE_SIZE: usize = 32;
const COLORS_PER_LINE: usize = 16;
/// The map palette blob holds CRAM lines 0, 1 and 3, in that order.
pub const MAP_PALETTE_LINE_ORDER: [u8; 3] = [0, 1, 3];
const PARTY_SYMBOLS: [&str; 11] = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth",
];
const STATIC_SEQUENCE: &str = "static";

fn char_field_art_pointers(rom: &[u8]) -> Result<Vec<u32>, PackError> {
    (0..PARTY_SYMBOLS.len())
        .map(|slot| u32_at(rom, CHAR_FIELD_ART_PTRS + slot * 4))
        .collect()
}

fn party_art(rom: &[u8], offset: usize) -> Result<TileSource, PackError> {
    bounds(rom, offset, PARTY_ART_BYTES)?;
    let mut source = TileSource::default();
    source.add(0, &rom[offset..offset + PARTY_ART_BYTES])?;
    Ok(source)
}

fn pal_init_line_3(rom: &[u8]) -> Result<Vec<[u8; 3]>, PackError> {
    bounds(rom, PAL_INIT_LINE_3, PALETTE_LINE_SIZE)?;
    Ok(gfx::palette_rgb(&gfx::decode_palette(
        &rom[PAL_INIT_LINE_3..PAL_INIT_LINE_3 + PALETTE_LINE_SIZE],
    )?))
}

/// `sprite_palette`: the 16 colours a sprite on CRAM line `line` draws with.
/// Line 2 is `Pal_Init_Line_3` on every map, which is why the party looks the
/// same everywhere.
pub fn sprite_palette(
    rom: &[u8],
    map_palette: &[[u8; 3]],
    line: u8,
) -> Result<Vec<[u8; 3]>, PackError> {
    if line == PAL_INIT_LINE_3_CRAM_LINE {
        return pal_init_line_3(rom);
    }
    let Some(position) = MAP_PALETTE_LINE_ORDER.iter().position(|&l| l == line) else {
        return Err(PackError(format!(
            "CRAM line {line} is not one a field sprite can select"
        )));
    };
    let wanted = MAP_PALETTE_LINE_ORDER.len() * COLORS_PER_LINE;
    if map_palette.len() < wanted {
        return Err(PackError(format!(
            "map palette holds {} colours, expected {wanted}",
            map_palette.len()
        )));
    }
    Ok(map_palette[position * COLORS_PER_LINE..(position + 1) * COLORS_PER_LINE].to_vec())
}

/// How a set of sequences reads its art.
struct SheetRequest<'a> {
    source: &'a TileSource,
    reading: Reading,
    palette: &'a [[u8; 3]],
    palette_line: u8,
    required: &'a [&'a str],
}

/// `_sheet_for_sequences`: the named sequences in one sheet. A sequence whose
/// frames name patterns the source lacks is dropped, unless it is required.
fn sheet_for_sequences(
    rom: &[u8],
    wanted: &[(String, usize)],
    request: &SheetRequest,
    mut census: Option<&mut SpriteCensus>,
) -> Result<Sheet, PackError> {
    let mut decoded = Vec::with_capacity(wanted.len());
    for (name, offset) in wanted {
        let sequence = decode_sequence(rom, *offset)?;
        let mappings = sequence
            .frames
            .iter()
            .map(|(mapping, _)| decode_mapping(rom, *mapping))
            .collect::<Result<Vec<_>, _>>()?;
        decoded.push((name, sequence, mappings));
    }
    let bounds: Bounds = union_box(
        decoded
            .iter()
            .flat_map(|(_, _, mappings)| mappings)
            .map(mapping_box),
    )?;
    let mut composed = Vec::with_capacity(decoded.len());
    for (name, sequence, mappings) in decoded {
        let mut frames = Vec::with_capacity(mappings.len());
        let mut missing = Vec::new();
        for mapping in &mappings {
            let (frame, holes) = compose_frame(
                mapping,
                request.source,
                bounds,
                request.reading,
                census.as_deref_mut(),
            );
            missing.extend(holes);
            frames.push(frame);
        }
        if let Some(census) = census.as_deref_mut() {
            census.sequences += 1;
            census.per_frame_duration_sequences += usize::from(sequence.per_frame_durations);
        }
        if !missing.is_empty() {
            if request.required.contains(&name.as_str()) {
                let listed: BTreeSet<u16> = missing.into_iter().collect();
                let shown: Vec<String> = listed.iter().take(8).map(u16::to_string).collect();
                return Err(PackError(format!(
                    "sequence {} ({name}) names patterns [{}] that the art source does not hold",
                    hex6(sequence.rom_offset),
                    shown.join(", ")
                )));
            }
            continue;
        }
        composed.push((name.clone(), sequence, frames));
    }
    if composed.is_empty() {
        return Err(PackError("no sequence in this set composed at all".into()));
    }
    Ok(build_sheet(
        composed,
        bounds,
        request.palette,
        request.palette_line,
    ))
}

fn table_sequences(
    rom: &[u8],
    table: usize,
    facings: usize,
) -> Result<Vec<(String, usize)>, PackError> {
    FACINGS[..facings]
        .iter()
        .map(|&(facing, name)| {
            Ok((
                name.to_owned(),
                u32_at(rom, table + usize::from(facing))? as usize,
            ))
        })
        .collect()
}

const WHOLE_FACINGS: [&str; 4] = ["down", "up", "right", "left"];
const STREAMED: Reading = Reading {
    art_tile: 0,
    tile_props: 0,
    streamed: true,
};

/// One playable character's sheet.
pub struct PartySprite {
    pub slot: usize,
    pub symbol: &'static str,
    pub object_id: usize,
    pub art_offset: usize,
    pub mappings_addr: usize,
    pub sheet: Sheet,
    pub art_sha256: String,
}

/// `party_sprites`: all eleven, with the two tables cross-checked against the
/// routines' own immediates.
pub fn party_sprites(
    rom: &[u8],
    routines: &[FieldObjectRoutine],
) -> Result<Vec<PartySprite>, PackError> {
    let palette = pal_init_line_3(rom)?;
    let mut sprites = Vec::with_capacity(PARTY_SYMBOLS.len());
    for (slot, &symbol) in PARTY_SYMBOLS.iter().enumerate() {
        let art = u32_at(rom, CHAR_FIELD_ART_PTRS + slot * 4)?;
        let table = u32_at(rom, CHAR_SPRITE_MAPPINGS_PTRS + slot * 4)?;
        let Some(routine) = routines.iter().find(|r| r.symbol == symbol) else {
            return Err(PackError(format!("no FieldObj_{symbol} routine")));
        };
        if routine.art_ptr != Some(art) || routine.mappings_addr != Some(table) {
            return Err(PackError(format!(
                "FieldObj_{symbol} sets art {:?} / mappings {:?}, but CharFieldArtPtrs slot {slot} \
                 says {art:#x} / {table:#x}",
                routine.art_ptr, routine.mappings_addr
            )));
        }
        if routine.palette_line() != Some(PAL_INIT_LINE_3_CRAM_LINE) {
            return Err(PackError(format!(
                "FieldObj_{symbol} draws on CRAM line {:?}, not the line \
                 {PAL_INIT_LINE_3_CRAM_LINE} the party is expected to use",
                routine.palette_line()
            )));
        }
        let (art_offset, mappings_addr) = (art as usize, table as usize);
        let source = party_art(rom, art_offset)?;
        if source.len() != PARTY_ART_TILES {
            return Err(PackError(format!(
                "{symbol}'s field art decodes to {} patterns, expected {PARTY_ART_TILES}",
                source.len()
            )));
        }
        let sheet = sheet_for_sequences(
            rom,
            &table_sequences(rom, mappings_addr, FACINGS.len())?,
            &SheetRequest {
                source: &source,
                reading: STREAMED,
                palette: &palette,
                palette_line: PAL_INIT_LINE_3_CRAM_LINE,
                required: &WHOLE_FACINGS,
            },
            None,
        )?;
        sprites.push(PartySprite {
            slot,
            symbol,
            object_id: routine.object_id(),
            art_offset,
            mappings_addr,
            sheet,
            art_sha256: hex(&Sha256::digest(
                &rom[art_offset..art_offset + PARTY_ART_BYTES],
            )),
        });
    }
    Ok(sprites)
}

/// `VEHICLE_RECORDS`: selector, symbol, mapping table, Nemesis art.
const VEHICLE_RECORDS: [(u8, &str, usize, usize); 3] = [
    (1, "LandRover", 0x047EF4, 0x296320),
    (2, "IceDigger", 0x047F04, 0x296A54),
    (3, "Hydrofoil", 0x047F14, 0x2971A4),
];

/// One selector's art and composed field sheet.
pub struct VehicleSprite {
    pub vehicle_index: u8,
    pub symbol: &'static str,
    pub art_offset: usize,
    pub mappings_addr: usize,
    pub compressed_size: usize,
    pub art_size: usize,
    pub sheet: Sheet,
    pub art_sha256: String,
}

/// `vehicle_sprites`: vehicle routines write `$60`, CRAM line 3.
pub fn vehicle_sprites(
    rom: &[u8],
    decodes: &Decodes,
    map_palette: &[[u8; 3]],
) -> Result<Vec<VehicleSprite>, PackError> {
    if map_palette.len() < 48 {
        return Err(PackError(format!(
            "vehicle palette has {} colours; expected the 48-colour map blob",
            map_palette.len()
        )));
    }
    let palette = sprite_palette(rom, map_palette, 3)?;
    let mut sprites = Vec::with_capacity(VEHICLE_RECORDS.len());
    for (vehicle_index, symbol, mappings_addr, art_offset) in VEHICLE_RECORDS {
        bounds(rom, art_offset, 2)?;
        let decoded = decodes.get(Call::Nemesis { offset: art_offset })?;
        let data = decoded.bytes();
        if data.is_empty() || !data.len().is_multiple_of(TILE_SIZE) {
            return Err(PackError(format!(
                "{symbol} Nemesis art at {} decoded to {} bytes, not a non-empty tile stream",
                hex6(art_offset),
                data.len()
            )));
        }
        let mut source = TileSource::default();
        source.add(0, &data)?;
        let sheet = sheet_for_sequences(
            rom,
            &table_sequences(rom, mappings_addr, FACINGS.len())?,
            &SheetRequest {
                source: &source,
                reading: STREAMED,
                palette: &palette,
                palette_line: 3,
                required: &WHOLE_FACINGS,
            },
            None,
        )?;
        sprites.push(VehicleSprite {
            vehicle_index,
            symbol,
            art_offset,
            mappings_addr,
            compressed_size: decoded.consumed(),
            art_size: data.len(),
            sheet,
            art_sha256: hex(&Sha256::digest(&data)),
        });
    }
    Ok(sprites)
}

/// `chest_sprite`: `LoadTreasureChests` stages normal art at `$4DC` and plant or
/// white art at `$4E6`. The `$32` map family selects a second normal chest and
/// CRAM line 1.
pub fn chest_sprite(
    rom: &[u8],
    decodes: &Decodes,
    map_palette: &[[u8; 3]],
    white: bool,
    general_var: u8,
) -> Result<Sheet, PackError> {
    let special = general_var == 0x32 && !white;
    let offset = match (white, special) {
        (true, _) => 0x297674,
        (false, true) => 0x2976FA,
        (false, false) => 0x2975AE,
    };
    let (tile, table): (u16, usize) = if white {
        (0x4E6, 0x51442)
    } else {
        (0x4DC, 0x5143A)
    };
    let line: u8 = if special { 1 } else { 2 };
    let mut source = TileSource::default();
    source.add(
        usize::from(tile),
        &decodes.get(Call::Nemesis { offset })?.bytes(),
    )?;
    sheet_for_sequences(
        rom,
        &table_sequences(rom, table, 2)?,
        &SheetRequest {
            source: &source,
            reading: Reading {
                art_tile: tile,
                tile_props: line << 5,
                streamed: false,
            },
            palette: &sprite_palette(rom, map_palette, line)?,
            palette_line: line,
            required: &["down", "up"],
        },
        None,
    )
}

/// `StagedArt`: everything a map record loads for its sprites to draw out of.
pub struct StagedArt {
    pub vram: TileSource,
    pub ram: HashMap<u32, TileSource>,
}

/// `tile_source_from_patterns`: only filled patterns are real art.
pub fn tile_source_from_patterns(patterns: &TilePatterns) -> TileSource {
    let mut source = TileSource::default();
    for (index, tile) in patterns.tiles.iter().enumerate() {
        if patterns.loaded[index] {
            source.insert(index, *tile);
        }
    }
    source
}

/// `stage_map_art`: replay a record's sprite lists onto the tilesets' bank.
pub fn stage_map_art(
    decodes: &Decodes,
    record: &MapRecord,
    mut vram: TileSource,
) -> Result<StagedArt, PackError> {
    let mut ram = HashMap::new();
    for entry in &record.sprites.entries {
        let decoded = decodes.get(Call::Nemesis {
            offset: entry.source,
        })?;
        if entry.to_ram {
            // loc_51A3E: `lsl.w #5, d0` then `moveq #-1, d1 / move.w d0, d1`.
            let address = RAM_BASE | (u32::from(entry.tile_number) << 5) & 0xFFFF;
            let mut source = TileSource::default();
            source.add(0, &decoded.bytes())?;
            ram.insert(address, source);
        } else {
            vram.add(usize::from(entry.tile_number), &decoded.bytes())?;
        }
    }
    for entry in &record.sprite_data.entries {
        let decoded = decodes.get(Call::Kosinski {
            offset: entry.source,
        })?;
        let data = decoded.bytes();
        if !data.len().is_multiple_of(TILE_SIZE) {
            continue; // Kosinski RAM blobs are not always patterns.
        }
        let mut source = TileSource::default();
        source.add(0, &data)?;
        ram.entry(RAM_BASE | u32::from(entry.word))
            .or_insert(source);
    }
    Ok(StagedArt { vram, ram })
}

/// A placed object resolved to a sheet, or to a reason it has none.
pub struct ObjectSprite {
    pub sheet: Option<Sheet>,
    pub facing_name: Option<String>,
    pub idle_sequence: Option<String>,
    pub walk_sequence: Option<String>,
    pub reason: Option<String>,
    pub missing_patterns: Vec<u16>,
}

fn no_sprite(facing: u8, reason: String, missing_patterns: Vec<u16>) -> ObjectSprite {
    ObjectSprite {
        sheet: None,
        facing_name: facing_name(facing).map(str::to_owned),
        idle_sequence: None,
        walk_sequence: None,
        reason: Some(reason),
        missing_patterns,
    }
}

/// `_streamed_source`: a ROM pointer is accepted only when it is one of the
/// eleven `CharFieldArtPtrs`, because the blobs carry no length of their own. A
/// staged bank comes first.
fn streamed_source<'a>(
    rom: &[u8],
    art_ptr: u32,
    art: &'a StagedArt,
) -> Result<Option<Cow<'a, TileSource>>, PackError> {
    if let Some(source) = art.ram.get(&art_ptr) {
        return Ok(Some(Cow::Borrowed(source)));
    }
    if art_ptr >= ROM_LIMIT || !char_field_art_pointers(rom)?.contains(&art_ptr) {
        return Ok(None);
    }
    Ok(Some(Cow::Owned(party_art(rom, art_ptr as usize)?)))
}

/// `object_sprite`: one placed object's sheet, the way the cartridge draws it.
/// When it draws nothing, the sprite says why, and nothing covers a hole.
#[allow(clippy::too_many_arguments)]
pub fn object_sprite(
    rom: &[u8],
    routine: &FieldObjectRoutine,
    extents: &HashMap<u32, usize>,
    art: &StagedArt,
    map_palette: &[[u8; 3]],
    facing: u8,
    art_tile: u16,
    census: Option<&mut SpriteCensus>,
) -> Result<ObjectSprite, PackError> {
    let symbol = routine.symbol;
    if !routine.builds_sprites {
        return Ok(no_sprite(
            facing,
            format!(
                "FieldObj_{symbol} sets render_flags bit 1, and Field_FillSpriteAttributes \
                 returns on it: the object is a trigger, not art"
            ),
            Vec::new(),
        ));
    }
    let (Some(tile_props), Some(line)) = (routine.sprite_tile_props, routine.palette_line()) else {
        return Ok(no_sprite(
            facing,
            format!(
                "FieldObj_{symbol} never stores sprite tile properties, so it has no palette \
                 line and draws nothing of its own"
            ),
            Vec::new(),
        ));
    };
    let palette = sprite_palette(rom, map_palette, line)?;
    let (source, effective_tile, streamed) = if routine.streams_art {
        let Some(art_ptr) = routine.art_ptr else {
            return Ok(no_sprite(
                facing,
                format!(
                    "FieldObj_{symbol} reaches FieldObj_Animate but stores no art_ptr, so the \
                     art it streams is whatever the object slot held"
                ),
                Vec::new(),
            ));
        };
        let Some(source) = streamed_source(rom, art_ptr, art)? else {
            return Ok(no_sprite(
                facing,
                format!(
                    "FieldObj_{symbol} streams art from 0x{art_ptr:08X}, which this map record \
                     does not load"
                ),
                Vec::new(),
            ));
        };
        (source, 0, true)
    } else {
        (
            Cow::Borrowed(&art.vram),
            routine.art_tile.unwrap_or(art_tile),
            false,
        )
    };
    let reading = Reading {
        art_tile: effective_tile,
        tile_props,
        streamed,
    };

    if let (true, Some(table)) = (routine.animates, routine.mappings_addr) {
        let table = table as usize;
        let owned = extents[&(table as u32)];
        let (name, wanted) = if facing.is_multiple_of(4) && usize::from(facing / 4) < owned {
            (
                facing_name(facing).unwrap_or_default().to_owned(),
                table_sequences(rom, table, owned)?,
            )
        } else {
            // The one AiedoPub object that faces $10 lands here: `facing_dir` is
            // added to `mappings_addr` with no bound, so the cartridge follows
            // the long past the end of this table. Reproduced, not corrected.
            let name = format!("facing_0x{facing:02X}");
            let offset = u32_at(rom, table + usize::from(facing))? as usize;
            (name.clone(), vec![(name, offset)])
        };
        let required = [name.as_str()];
        let request = SheetRequest {
            source: &source,
            reading,
            palette: &palette,
            palette_line: line,
            required: &required,
        };
        return Ok(match sheet_for_sequences(rom, &wanted, &request, census) {
            Ok(sheet) => ObjectSprite {
                sheet: Some(sheet),
                idle_sequence: Some(format!("idle_{name}")),
                walk_sequence: Some(format!("walk_{name}")),
                facing_name: Some(name),
                reason: None,
                missing_patterns: Vec::new(),
            },
            Err(error) => no_sprite(facing, error.0, Vec::new()),
        });
    }

    let Some(mappings) = routine.mappings else {
        return Ok(no_sprite(
            facing,
            format!("FieldObj_{symbol} stores no mapping pointer"),
            Vec::new(),
        ));
    };
    let mapping = decode_mapping(rom, mappings as usize)?;
    let bounds = mapping_box(&mapping);
    let (frame, missing) = compose_frame(&mapping, &source, bounds, reading, census);
    if !missing.is_empty() {
        let listed: BTreeSet<u16> = missing.into_iter().collect();
        return Ok(no_sprite(
            facing,
            format!("FieldObj_{symbol} names patterns this map record does not load"),
            listed.into_iter().collect(),
        ));
    }
    let sequence = SheetSequence {
        frames: vec![0],
        durations: vec![u16::from(routine.mappings_duration.unwrap_or(0)) + 1],
        sequence_offset: mappings as usize,
    };
    Ok(ObjectSprite {
        sheet: Some(Sheet {
            frames: vec![frame],
            sequences: [(STATIC_SEQUENCE.to_owned(), sequence)].into(),
            bounds,
            palette,
            palette_line: line,
            mirrors: Default::default(),
        }),
        facing_name: facing_name(facing).map(str::to_owned),
        idle_sequence: Some(STATIC_SEQUENCE.to_owned()),
        walk_sequence: Some(STATIC_SEQUENCE.to_owned()),
        reason: None,
        missing_patterns: Vec::new(),
    })
}
