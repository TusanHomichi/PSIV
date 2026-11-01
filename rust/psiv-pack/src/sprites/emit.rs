//! Writing `sprites/`: `psiv_tools/sprites/emit.py`, `vehicle_pack.py`, and the
//! part of `build_pack`'s map loop that registers sheets. A sheet is keyed by
//! content, so one NPC sheet serves whole towns and a palette makes a new sheet.
//! Registration is in map order, because the first object to ask names the file.
//!
//! The files are `psiv_data::sprites` types, with what the game doesn't read in
//! each type's `research` map: it has no typed home yet.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use psiv_data::{Sequence, SequenceFrame, Sheet as SheetRecord, SheetFile, VehiclePaletteVariant};
use serde_json::{Map, Value, json};

use super::compose::{Sheet, SpriteCensus};
use super::field::{
    ObjectSprite, PAL_INIT_LINE_3_CRAM_LINE, PARTY_ART_BYTES, PARTY_ART_TILES, StagedArt,
    VehicleSprite, chest_sprite, object_sprite, party_sprites, stage_map_art,
    tile_source_from_patterns, vehicle_sprites,
};
use super::objects::{
    FIELD_OBJ_DO_OBJ_COLLISION, FIELD_OBJECTS_JMP_TBL, FieldObjectRoutine, INTERACTION_CHK_OBJECTS,
    RENDER_FLAG_CAMERA_BYPASS, RENDER_FLAG_INTERACTABLE, facing_table_extents, scan_field_objects,
};
use crate::decode::Decodes;
use crate::json::Json;
use crate::maps::effects::{Anchors, MapEffects};
use crate::maps::layouts::{decode_tilesets, map_palette_rgb};
use crate::maps::patches::resolve_palette_effects;
use crate::maps::pictures::MapInputs;
use crate::maps::records::MapRecord;
use crate::{PackError, png};

const PARTY_DIRECTORY: &str = "sprites/party";
const NPC_DIRECTORY: &str = "sprites/npcs";
const VEHICLE_DIRECTORY: &str = "sprites/vehicles";
const PARTY_NAME: &str = "sprites/party.json";
pub const NPC_NAME: &str = "sprites/npcs.json";
const VEHICLE_NAME: &str = "sprites/vehicles.json";
/// `PACK_FORMAT_VERSION`.
const FORMAT_VERSION: u32 = 2;
const SPRITE_TRANSPARENT_INDEX: u8 = 0;
const SHEET_ID_LENGTH: usize = 8;

/// `sheet_png`: frames left to right, the object's CRAM line baked in.
fn sheet_png(sheet: &Sheet) -> Result<Vec<u8>, PackError> {
    let (width, height, pixels) = sheet.strip();
    if sheet.palette.len() != 16 {
        return Err(PackError(format!(
            "sprite sheet palette holds {} colours, not 16",
            sheet.palette.len()
        )));
    }
    Ok(png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        &sheet.palette,
        &[SPRITE_TRANSPARENT_INDEX],
    )?)
}

/// `sheet_json`: the geometry, timing and palette a renderer needs. `research`
/// comes in holding what the caller adds.
fn sheet_record(
    sheet: &Sheet,
    id: String,
    path: String,
    image: &[u8],
    mut research: Map<String, Value>,
) -> Result<SheetRecord, PackError> {
    research.insert("png_sha256".into(), png::image_sha256(image)?.into());
    research.insert(
        "palette".into(),
        json!({
            "cram_line": sheet.palette_line,
            "source": if sheet.palette_line == PAL_INIT_LINE_3_CRAM_LINE {
                "Pal_Init_Line_3"
            } else {
                "map palette blob"
            },
            "transparent_index": SPRITE_TRANSPARENT_INDEX,
            "colors": sheet.palette,
        }),
    );
    research.insert(
        "mirrors".into(),
        Value::Object(
            sheet
                .mirrors
                .iter()
                .map(|(index, other)| (index.to_string(), Value::from(*other)))
                .collect(),
        ),
    );
    let sequences = sheet
        .sequences
        .iter()
        .map(|(name, sequence)| {
            let frames = sequence
                .frames
                .iter()
                .zip(&sequence.durations)
                .map(|(&index, &duration)| SequenceFrame {
                    index: u32::from(index),
                    duration_ticks: u32::from(duration),
                })
                .collect();
            let record = Sequence {
                frames,
                looping: true,
                sequence_offset: Some(format!("0x{:06X}", sequence.sequence_offset)),
            };
            (name.clone(), record)
        })
        .collect();
    Ok(SheetRecord {
        id,
        png: path,
        frame_width: sheet.bounds.width() as u32,
        frame_height: sheet.bounds.height() as u32,
        frame_count: sheet.frames.len() as u32,
        origin_x: -sheet.bounds.left,
        origin_y: -sheet.bounds.top,
        sequences,
        research,
    })
}

fn safe(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `SheetRegistry`: deduplicates sheets across maps and names their files.
#[derive(Default)]
pub struct SheetRegistry {
    by_identity: HashMap<String, String>,
    sheets: BTreeMap<String, Sheet>,
    users: BTreeMap<String, usize>,
}

impl SheetRegistry {
    /// `register`: `placed` is false for a sheet nothing stands on, an alternate
    /// a palette copy switches to.
    pub fn register(
        &mut self,
        sheet: Sheet,
        symbol: &str,
        placed: bool,
    ) -> Result<String, PackError> {
        let identity = sheet.identity();
        let name = match self.by_identity.get(&identity) {
            Some(name) => name.clone(),
            None => {
                let name = format!("{}_{}", safe(symbol), &identity[..SHEET_ID_LENGTH]);
                if self.sheets.contains_key(&name) {
                    return Err(PackError(format!(
                        "two different sheets both want the name '{name}'; the \
                         {SHEET_ID_LENGTH}-character content hash is not enough"
                    )));
                }
                self.by_identity.insert(identity, name.clone());
                self.sheets.insert(name.clone(), sheet);
                self.users.insert(name.clone(), 0);
                name
            }
        };
        if placed && let Some(users) = self.users.get_mut(&name) {
            *users += 1;
        }
        Ok(name)
    }

    fn emit(&self, root: &Path) -> Result<Vec<SheetRecord>, PackError> {
        let mut entries = Vec::with_capacity(self.sheets.len());
        for (name, sheet) in &self.sheets {
            let image = sheet_png(sheet)?;
            let path = format!("{NPC_DIRECTORY}/{name}.png");
            crate::write_file(root, &path, &image)?;
            let mut research = Map::new();
            research.insert("placements".into(), self.users[name].into());
            entries.push(sheet_record(sheet, name.clone(), path, &image, research)?);
        }
        Ok(entries)
    }
}

/// `NpcMetadata`: what the pack knows about one placed object beyond its record.
#[derive(Clone, Debug, PartialEq)]
pub struct NpcMetadata {
    pub sprite: Option<Json>,
    pub sheet: Option<String>,
    pub sprite_reason: Option<String>,
    pub interactable: bool,
    pub camera_bypass: bool,
}

impl NpcMetadata {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("sprite", self.sprite.clone())
            .with("sprite_reason", self.sprite_reason.as_deref())
            .with("interactable", self.interactable)
            .with("camera_bypass", self.camera_bypass)
    }
}

fn sprite_reference(
    sheet: &str,
    facing: Option<&str>,
    idle: Option<&str>,
    walk: Option<&str>,
) -> Json {
    Json::object()
        .with("sheets", NPC_NAME)
        .with("sheet", sheet)
        .with("facing", facing)
        .with("idle_sequence", idle)
        .with("walk_sequence", walk)
}

/// `resolve_map_sprites`: every object of one map to a sheet, or to a reason.
#[allow(clippy::too_many_arguments)]
pub fn resolve_map_sprites(
    rom: &[u8],
    record: &MapRecord,
    art: &StagedArt,
    palette: &[[u8; 3]],
    routines: &[FieldObjectRoutine],
    extents: &HashMap<u32, usize>,
    registry: &mut SheetRegistry,
    census: &mut SpriteCensus,
) -> Result<(Vec<NpcMetadata>, Vec<Json>), PackError> {
    let mut references = Vec::with_capacity(record.objects.entries.len());
    let mut artless = Vec::new();
    for (index, entry) in record.objects.entries.iter().enumerate() {
        let object = &entry.record;
        let found = routines
            .iter()
            .find(|r| r.object_id() == usize::from(object.object_id));
        let Some(routine) = found else {
            return Err(PackError(format!(
                "map 0x{:03X} object {index} names field object 0x{:X}, outside \
                 FieldObjectsJmpTbl",
                record.id, object.object_id
            )));
        };
        let ObjectSprite {
            sheet,
            facing_name,
            idle_sequence,
            walk_sequence,
            reason,
            missing_patterns,
        } = object_sprite(
            rom,
            routine,
            extents,
            art,
            palette,
            object.facing_dir,
            object.art_tile,
            Some(census),
        )?;
        let Some(sheet) = sheet else {
            artless.push(
                Json::object()
                    .with("npc_index", index)
                    .with("object_id", object.object_id)
                    .with("symbol", object.symbol())
                    .with("reason", reason.as_deref())
                    .with("missing_patterns", missing_patterns),
            );
            references.push(NpcMetadata {
                sprite: None,
                sheet: None,
                sprite_reason: reason,
                interactable: routine.interactable,
                camera_bypass: routine.camera_bypass,
            });
            continue;
        };
        let sheet_id = registry.register(sheet, object.symbol().unwrap_or("FieldObj"), true)?;
        references.push(NpcMetadata {
            sprite: Some(sprite_reference(
                &sheet_id,
                facing_name.as_deref(),
                idle_sequence.as_deref(),
                walk_sequence.as_deref(),
            )),
            sheet: Some(sheet_id),
            sprite_reason: None,
            interactable: routine.interactable,
            camera_bypass: routine.camera_bypass,
        });
    }
    Ok((references, artless))
}

/// `bind_chest_sprites`: the reference each chest carries, registering its sheet.
pub fn chest_references(
    rom: &[u8],
    decodes: &Decodes,
    record: &MapRecord,
    palette: &[[u8; 3]],
    registry: &mut SheetRegistry,
) -> Result<Vec<Json>, PackError> {
    let mut references = Vec::with_capacity(record.treasure_chests.entries.len());
    for entry in &record.treasure_chests.entries {
        let white = entry.record.white_chest;
        let sheet = chest_sprite(rom, decodes, palette, white, record.general_var)?;
        let symbol = if white {
            "WhiteTreasureChest"
        } else {
            "TreasureChest"
        };
        let name = registry.register(sheet, symbol, true)?;
        references.push(sprite_reference(
            &name,
            Some("down"),
            Some("idle_down"),
            Some("walk_down"),
        ));
    }
    Ok(references)
}

fn emit_party(
    root: &Path,
    rom: &[u8],
    routines: &[FieldObjectRoutine],
) -> Result<Vec<SheetRecord>, PackError> {
    let mut entries = Vec::new();
    for sprite in party_sprites(rom, routines)? {
        let image = sheet_png(&sprite.sheet)?;
        let path = format!("{PARTY_DIRECTORY}/{}.png", safe(sprite.symbol));
        crate::write_file(root, &path, &image)?;
        let research = json!({
            "slot": sprite.slot,
            "symbol": sprite.symbol,
            "object_id": sprite.object_id,
            "art": {
                "rom_offset": format!("0x{:06X}", sprite.art_offset),
                "size_bytes": PARTY_ART_BYTES,
                "tile_count": PARTY_ART_TILES,
                "compression": null,
                "sha256": sprite.art_sha256,
            },
            "mappings_table": format!("0x{:06X}", sprite.mappings_addr),
        });
        let Value::Object(research) = research else {
            unreachable!("json! of an object");
        };
        entries.push(sheet_record(
            &sprite.sheet,
            sprite.symbol.to_owned(),
            path,
            &image,
            research,
        )?);
    }
    Ok(entries)
}

fn emit_vehicles(
    root: &Path,
    vehicles: &[VehicleSprite],
    suffix: &str,
) -> Result<Vec<SheetRecord>, PackError> {
    let mut entries = Vec::with_capacity(vehicles.len());
    for sprite in vehicles {
        let image = sheet_png(&sprite.sheet)?;
        let label = format!("{}{suffix}", sprite.symbol);
        let path = format!("{VEHICLE_DIRECTORY}/{}.png", safe(&label));
        crate::write_file(root, &path, &image)?;
        let research = json!({
            "vehicle_index": sprite.vehicle_index,
            "symbol": sprite.symbol,
            "art": {
                "rom_offset": format!("0x{:06X}", sprite.art_offset),
                "size_bytes": sprite.art_size,
                "compressed_size_bytes": sprite.compressed_size,
                "tile_count": sprite.art_size / 32,
                "compression": "nemesis",
                "sha256": sprite.art_sha256,
            },
            "mappings_table": format!("0x{:06X}", sprite.mappings_addr),
        });
        let Value::Object(research) = research else {
            unreachable!("json! of an object");
        };
        entries.push(sheet_record(&sprite.sheet, label, path, &image, research)?);
    }
    Ok(entries)
}

/// `emit_vehicle_index`: the base sheets and every distinct selected-map palette.
fn vehicle_file(
    rom: &[u8],
    decodes: &Decodes,
    root: &Path,
    base_palette: &[[u8; 3]],
    variants: &[(Vec<[u8; 3]>, Vec<u16>)],
) -> Result<SheetFile, PackError> {
    let entries = emit_vehicles(root, &vehicle_sprites(rom, decodes, base_palette)?, "")?;
    let base_key = &base_palette[32..48];
    let mut palette_variants = Vec::with_capacity(variants.len());
    for (number, (key, map_ids)) in variants.iter().enumerate() {
        let (sheets, sheet_ids): (Vec<SheetRecord>, Vec<String>) = if key == base_key {
            (Vec::new(), entries.iter().map(|e| e.id.clone()).collect())
        } else {
            let mut palette = vec![[0u8; 3]; 32];
            palette.extend_from_slice(key);
            let sheets = emit_vehicles(
                root,
                &vehicle_sprites(rom, decodes, &palette)?,
                &format!("_pal{number:02}"),
            )?;
            let ids = sheets.iter().map(|e| e.id.clone()).collect();
            (sheets, ids)
        };
        palette_variants.push(VehiclePaletteVariant {
            map_ids: map_ids.clone(),
            sheet_ids,
            sheets,
        });
    }
    let mut research = Map::new();
    research.insert(
        "palette_source".into(),
        "each selected map palette, CRAM line 3; base is first selected map".into(),
    );
    Ok(SheetFile {
        format_version: FORMAT_VERSION,
        kind: Some("field_vehicles".into()),
        sheet_count: entries.len() as u32,
        sheets: entries,
        palette_variants,
        research,
    })
}

/// `field_objects_json`: `render_flags` bits per object type, with provenance
/// stated once.
fn field_objects(routines: &[FieldObjectRoutine], placements: &HashMap<usize, usize>) -> Value {
    let types: Vec<Value> = routines
        .iter()
        .map(|routine| {
            json!({
                "object_id": routine.object_id(),
                "symbol": routine.symbol,
                "routine_offset": format!("0x{:06X}", routine.rom_offset),
                "interactable": routine.interactable,
                "source": routine.interactable_source,
                "changes_at_runtime": routine.interactable_changes_at_runtime,
                "camera_bypass": routine.camera_bypass,
                "camera_bypass_source": routine.camera_bypass_source,
                "camera_bypass_changes_at_runtime": routine.camera_bypass_changes_at_runtime,
                "placements": placements.get(&routine.object_id()).copied().unwrap_or(0),
            })
        })
        .collect();
    json!({
        "render_flags_bit": RENDER_FLAG_INTERACTABLE,
        "camera_render_flags_bit": RENDER_FLAG_CAMERA_BYPASS,
        "table": format!("0x{FIELD_OBJECTS_JMP_TBL:06X}"),
        "count": types.len(),
        "tested_by": [
            {"routine": "Interaction_ChkObjects",
             "rom_offset": format!("0x{INTERACTION_CHK_OBJECTS:06X}"),
             "effect": "the talk probe skips the object"},
            {"routine": "FieldObj_DoObjCollision",
             "rom_offset": format!("0x{FIELD_OBJ_DO_OBJ_COLLISION:06X}"),
             "effect": "the object does not block the walker"},
        ],
        "instruction_forms": ["bset #3, $2(a4)", "bclr #3, $2(a4)"],
        "unwritten_bit_is_clear_because":
            "GameMode_LoadFieldMap zero-fills $400 longwords from Field_Objects_Memory via \
             trap #0 before LoadMapObjects parses the record, so a routine that writes neither \
             leaves the bit clear",
        "types": types,
    })
}

/// One map's sprite work, for the probe and, later, the map file.
pub struct MapSprites {
    pub id: usize,
    pub npcs: Vec<NpcMetadata>,
    pub artless: Vec<Json>,
    pub chests: Vec<Json>,
    /// The map's effect list with `deferred_effects` on the palette paths.
    pub effects: Vec<Json>,
    pub palette_counts: [usize; 3],
}

/// Everything the map loop leaves for the index files.
pub struct FieldSprites {
    pub routines: Vec<FieldObjectRoutine>,
    pub registry: SheetRegistry,
    pub census: SpriteCensus,
    pub vehicle_palette: Option<Vec<[u8; 3]>>,
    pub vehicle_variants: Vec<(Vec<[u8; 3]>, Vec<u16>)>,
    pub placements: HashMap<usize, usize>,
    pub maps: Vec<MapSprites>,
}

/// The part of `build_pack`'s loop that registers sheets, map by map.
pub fn field_sprites(
    rom: &[u8],
    decodes: &Decodes,
    inputs: &MapInputs,
) -> Result<FieldSprites, PackError> {
    let routines = scan_field_objects(rom)?;
    let extents = facing_table_extents(&routines);
    let dispatch = Anchors::read(rom)?.table;
    let mut registry = SheetRegistry::default();
    let mut census = SpriteCensus::default();
    let mut vehicle_palette = None;
    let mut vehicle_variants: Vec<(Vec<[u8; 3]>, Vec<u16>)> = Vec::new();
    let mut placements: HashMap<usize, usize> = HashMap::new();
    let mut maps = Vec::new();
    for record in inputs.maps.real() {
        let specs: Vec<(usize, usize)> = record
            .tilesets
            .entries
            .iter()
            .map(|entry| (usize::from(entry.word), entry.source))
            .collect();
        let patterns = decode_tilesets(rom, decodes, &specs)?;
        let art = stage_map_art(decodes, record, tile_source_from_patterns(&patterns))?;
        let palette = map_palette_rgb(rom, record.palette.pointer)?;
        let (npcs, artless) = resolve_map_sprites(
            rom,
            record,
            &art,
            &palette,
            &routines,
            &extents,
            &mut registry,
            &mut census,
        )?;
        let key = palette[32..48].to_vec();
        match vehicle_variants.iter_mut().find(|(seen, _)| *seen == key) {
            Some((_, ids)) => ids.push(record.id as u16),
            None => vehicle_variants.push((key, vec![record.id as u16])),
        }
        if vehicle_palette.is_none() {
            vehicle_palette = Some(palette.clone());
        }
        let mut effects = effects_of(&inputs.effects, record);
        let palette_counts = resolve_palette_effects(
            rom,
            record,
            &art,
            &mut effects,
            &npcs,
            &routines,
            &extents,
            &mut registry,
            &mut census,
            &palette,
            dispatch,
        )?;
        let chests = chest_references(rom, decodes, record, &palette, &mut registry)?;
        for entry in &record.objects.entries {
            *placements
                .entry(usize::from(entry.record.object_id))
                .or_default() += 1;
        }
        maps.push(MapSprites {
            id: record.id,
            npcs,
            artless,
            chests,
            effects,
            palette_counts,
        });
    }
    Ok(FieldSprites {
        routines,
        registry,
        census,
        vehicle_palette,
        vehicle_variants,
        placements,
        maps,
    })
}

fn effects_of(effects: &MapEffects, record: &MapRecord) -> Vec<Json> {
    effects.per_map.get(&record.id).cloned().unwrap_or_default()
}

/// A sheet index file, as the Python builder writes it.
fn write_sheet_file(root: &Path, name: &str, file: &SheetFile) -> Result<(), PackError> {
    let value = serde_json::to_value(file)
        .map_err(|error| PackError(format!("cannot serialize {name}: {error}")))?;
    crate::write_json(root, name, &Json::from(&value)).map(drop)
}

/// Every sheet and the three index files.
pub fn write_sprites(
    data: &[u8],
    decodes: &Decodes,
    root: &Path,
    field: &FieldSprites,
) -> Result<(), PackError> {
    let party = emit_party(root, data, &field.routines)?;
    write_sheet_file(
        root,
        PARTY_NAME,
        &SheetFile {
            format_version: FORMAT_VERSION,
            kind: Some("field_party".into()),
            sheet_count: party.len() as u32,
            sheets: party,
            palette_variants: Vec::new(),
            research: Map::new(),
        },
    )?;
    let npcs = field.registry.emit(root)?;
    let mut research = Map::new();
    research.insert(
        "field_objects".into(),
        field_objects(&field.routines, &field.placements),
    );
    write_sheet_file(
        root,
        NPC_NAME,
        &SheetFile {
            format_version: FORMAT_VERSION,
            kind: Some("field_npcs".into()),
            sheet_count: npcs.len() as u32,
            sheets: npcs,
            palette_variants: Vec::new(),
            research,
        },
    )?;
    let Some(base) = &field.vehicle_palette else {
        return Err(PackError(
            "cannot extract vehicle sheets from an empty map selection".into(),
        ));
    };
    let vehicles = vehicle_file(data, decodes, root, base, &field.vehicle_variants)?;
    write_sheet_file(root, VEHICLE_NAME, &vehicles)
}

#[cfg(test)]
mod tests {
    use super::super::compose::{Bounds, Frame};
    use super::*;

    fn sheet(pixel: u8) -> Sheet {
        Sheet {
            frames: vec![Frame {
                pixels: vec![pixel],
            }],
            sequences: BTreeMap::new(),
            bounds: Bounds {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            },
            palette: vec![[0; 3]; 16],
            palette_line: 2,
            mirrors: BTreeMap::new(),
        }
    }

    #[test]
    fn the_registry_keys_by_content_and_counts_only_placements() {
        let mut registry = SheetRegistry::default();
        let first = registry.register(sheet(1), "A B", true).unwrap();
        assert!(first.starts_with("A_B_"));
        assert_eq!(registry.register(sheet(1), "Other", true).unwrap(), first);
        assert_eq!(registry.register(sheet(1), "Other", false).unwrap(), first);
        assert_eq!(registry.users[&first], 2);
        let second = registry.register(sheet(2), "A B", false).unwrap();
        assert_ne!(first, second);
        assert_eq!(registry.users[&second], 0);
    }

    #[test]
    fn a_sheet_png_needs_a_whole_palette_line() {
        let mut short = sheet(1);
        short.palette.pop();
        assert!(
            sheet_png(&short)
                .unwrap_err()
                .0
                .contains("holds 15 colours")
        );
    }
}
