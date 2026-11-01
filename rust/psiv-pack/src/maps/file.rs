//! One map's pack file, `maps/<id>_<symbol>.json`: `psiv_tools/pack_map_json.py`
//! and `map_updates_render.bind_updates`.
//!
//! The file is a `psiv_data::MapRecord`, which owns the schema the game reads.
//! Warps, interaction areas, NPCs, chests and the map's scalars are built as its
//! types. What comes from an extractor's JSON (effects, updates, patches,
//! variants, scroll) is parsed into them, so a shape the reader refuses fails
//! here. What the game doesn't read has no typed home yet, and is laid over the
//! serialized record ([`overlay`]): the research fields, and the integer
//! collision rows the Python builder writes where the type writes hex.

use psiv_data::{
    Cell, CellRect, Collision, ContentsType, Dimensions, Direction, Facing, InteractionArea,
    InteractionFlagType, InteractionSource, MapId, MapRecord as MapFile, MapRef, Npc, RangeRef,
    SpriteFacing, SpriteRef, TransitionTable, Treasure, VehicleBattleLayout, Warp, WarpSource,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::effects::{field, text};
use super::layouts::{COLLISION_CELL_PIXELS, CollisionGrid, MapLayout};
use super::overworld::MapSection;
use super::pictures::Pictures;
use super::records::{MapRecord, hex6, map_reference};
use crate::PackError;
use crate::json::{Json, overlay};
use crate::m68k::w;
use crate::map_symbols::{ITEM_SYMBOLS, MAP_SYMBOLS};
use crate::sprites::emit::{MapSprites, NPC_NAME, NpcMetadata};
use crate::sprites::records::FACINGS;
use crate::warps::{Rect, STANDING_CELL_Y_OFFSET, interaction_rect, warp_rect, xy_range_name};

/// `PACK_FORMAT_VERSION`.
const FORMAT_VERSION: u32 = 2;
const MAP_CHANGE_COLLISION_TYPE: u8 = 0x1;
/// `Pal_Init_Line_3`, which `bind_updates` splices in as CRAM line 2.
const PAL_INIT_LINE_3: usize = 0x296300;
/// `Interaction_EventIndexes`: `Interaction_GetEvent` indexes these 32 words
/// with an area's byte 9. Their high bit means what it means in `Event_Index`.
const INTERACTION_EVENT_INDEXES: usize = 0x064F22;
const INTERACTION_EVENT_COUNT: usize = 32;

fn typed<T: DeserializeOwned>(value: &Json, what: &str) -> Result<T, PackError> {
    serde_json::from_value(Value::from(value))
        .map_err(|error| PackError(format!("the reader's type refuses {what}: {error}")))
}

fn facing(id: u8) -> Facing {
    let name = FACINGS
        .iter()
        .position(|(value, _)| *value == id)
        .map(|index| {
            [
                Direction::Down,
                Direction::Up,
                Direction::Right,
                Direction::Left,
            ][index]
        });
    Facing { id, name }
}

fn range_ref(id: u16) -> Result<RangeRef, PackError> {
    Ok(RangeRef {
        id: id as u8,
        name: xy_range_name(id)?.to_owned(),
    })
}

fn cell_rect(rect: Option<Rect>) -> Option<CellRect> {
    rect.map(|rect| CellRect {
        x: rect.x as u32,
        y: rect.y as u32,
        width: rect.width as u32,
        height: rect.height as u32,
    })
}

fn standing(y: u32) -> u32 {
    y.saturating_add_signed(STANDING_CELL_Y_OFFSET)
}

/// `_warps`: both transition tables, and the transitions whose rectangle covers
/// no map-change cell.
pub fn warps(
    record: &MapRecord,
    grid: &CollisionGrid,
) -> Result<(Vec<Warp>, Vec<Json>), PackError> {
    let mut out = Vec::new();
    let mut anomalies = Vec::new();
    for (entries, table, trigger) in [
        (
            &record.transitions.entries,
            TransitionTable::Normal,
            "any_walkable_tile",
        ),
        (
            &record.transitions_2.entries,
            TransitionTable::MapChange,
            "map_change_tile",
        ),
    ] {
        for entry in entries {
            let source = &entry.record;
            let (width, height) = (grid.width as i32, grid.height as i32);
            let rect = warp_rect(
                source.range,
                i32::from(source.source_x_tile),
                i32::from(source.source_y_tile),
                width,
                height,
            )?;
            let covered = rect.is_some_and(|rect| {
                rect.cells().any(|(x, y)| {
                    grid.types[y as usize * grid.width + x as usize] == MAP_CHANGE_COLLISION_TYPE
                })
            });
            let index = out.len() as u32;
            let target = usize::from(source.target);
            let (x, y) = (
                u32::from(source.source_x_tile),
                u32::from(source.source_y_tile),
            );
            let warp = Warp {
                index,
                table,
                trigger: trigger.to_owned(),
                record_offset: hex6(entry.rom_offset),
                range: range_ref(source.range)?,
                source: WarpSource {
                    x_byte: x,
                    y_byte: y,
                    x_cell: x,
                    y_cell: standing(y),
                },
                rect: cell_rect(rect),
                target: MapRef {
                    id: MapId(source.target),
                    symbol: MAP_SYMBOLS.get(target).map(|s| (*s).to_owned()),
                },
                destination: Cell {
                    x_cell: u32::from(source.destination_x_tile),
                    y_cell: standing(u32::from(source.destination_y_tile)),
                },
                facing: facing(source.facing_dir),
                character_alignment: source.character_alignment,
            };
            if !covered {
                anomalies.push(
                    Json::object()
                        .with("warp_index", index)
                        .with("table", u8::from(table))
                        .with(
                            "range",
                            Json::object()
                                .with("id", source.range)
                                .with("name", xy_range_name(source.range)?),
                        )
                        .with("record_offset", hex6(entry.rom_offset))
                        .with("rect", rect.map(Rect::to_json))
                        .with("target", map_reference(target)),
                );
            }
            out.push(warp);
        }
    }
    Ok((out, anomalies))
}

/// One word of `Interaction_EventIndexes`, read from the ROM.
fn interaction_event_index(rom: &[u8], parameter: u8) -> Result<u16, PackError> {
    let index = usize::from(parameter);
    if index >= INTERACTION_EVENT_COUNT {
        return Err(PackError(format!(
            "it indexes Interaction_EventIndexes with ${parameter:02X}, outside the \
             {INTERACTION_EVENT_COUNT}-entry table"
        )));
    }
    Ok(w(rom, INTERACTION_EVENT_INDEXES + index * 2)?)
}

/// `_interaction_areas`: type 2 records also carry the indexed event word.
fn interaction_areas(
    rom: &[u8],
    record: &MapRecord,
    doc: &Json,
    grid: &CollisionGrid,
) -> Result<Vec<InteractionArea>, PackError> {
    let listed = field(doc, "interaction_areas")
        .map_or(&[][..], |list| super::effects::items(list, "entries"));
    let mut out = Vec::with_capacity(record.interaction_areas.entries.len());
    for (index, entry) in record.interaction_areas.entries.iter().enumerate() {
        let area = &entry.record;
        let rect = interaction_rect(
            area.range,
            i32::from(area.x_tile),
            i32::from(area.y_tile),
            grid.width as i32,
            grid.height as i32,
        )?;
        let event_index = if area.interaction_type == 2 {
            interaction_event_index(rom, area.parameter)
                .map_err(|error| {
                    PackError(format!(
                        "interaction area at {}: {}",
                        hex6(entry.rom_offset),
                        error.0
                    ))
                })
                .map(Some)?
        } else {
            None
        };
        let flag_type = listed.get(index).and_then(|json| field(json, "flag_type"));
        let Some(flag_type) = flag_type else {
            return Err(PackError(format!(
                "interaction area {index} of map 0x{:03X} has no flag type",
                record.id
            )));
        };
        let flag_type: InteractionFlagType = typed(flag_type, "an interaction flag type")?;
        let (x, y) = (u32::from(area.x_tile), u32::from(area.y_tile));
        out.push(InteractionArea {
            index: index as u32,
            record_offset: hex6(entry.rom_offset),
            range: range_ref(area.range)?,
            source: InteractionSource {
                x_byte: x,
                y_byte: y,
                x_cell: x / 2,
                y_cell: standing(y / 2),
            },
            rect: cell_rect(rect),
            flag_type,
            flag: area.flag,
            interaction_type: area.interaction_type,
            parameter: area.parameter,
            event_index,
        });
    }
    Ok(out)
}

fn sprite_ref(reference: &Json) -> Result<SpriteRef, PackError> {
    Ok(SpriteRef {
        sheet: text(reference, "sheet").to_owned(),
        sheets: NPC_NAME.to_owned(),
        facing: match text(reference, "facing") {
            "down" => Some(SpriteFacing::Down),
            "up" => Some(SpriteFacing::Up),
            "left" => Some(SpriteFacing::Left),
            "right" => Some(SpriteFacing::Right),
            "" => None,
            _ => Some(SpriteFacing::Unknown),
        },
        idle_sequence: text(reference, "idle_sequence").to_owned(),
        walk_sequence: text(reference, "walk_sequence").to_owned(),
    })
}

/// `_npcs`: object coordinates are words scaled by 8, and the cell is the floor
/// of that plus the standing-cell shift.
fn npcs(record: &MapRecord, metadata: &[NpcMetadata]) -> Result<Vec<Npc>, PackError> {
    let mut out = Vec::with_capacity(metadata.len());
    for (index, (entry, meta)) in record.objects.entries.iter().zip(metadata).enumerate() {
        let object = &entry.record;
        let (x, y) = (u32::from(object.x_tile) * 8, u32::from(object.y_tile) * 8);
        let cell = COLLISION_CELL_PIXELS as u32;
        out.push(Npc {
            index: index as u32,
            record_offset: hex6(entry.rom_offset),
            object_id: object.object_id,
            symbol: object.symbol().map(str::to_owned),
            x_pixels: x,
            y_pixels: y,
            x_cell: x / cell,
            y_cell: standing(y / cell),
            facing: facing(object.facing_dir),
            dialogue_id: u16::from(object.dialogue_id),
            sprite: meta.sprite.as_ref().map(sprite_ref).transpose()?,
            sprite_reason: meta.sprite_reason.clone(),
            art_tile: u32::from(object.art_tile),
            interactable: meta.interactable,
            camera_bypass: meta.camera_bypass,
        });
    }
    Ok(out)
}

/// `_treasure_chests`: byte 1 selects how byte 3 reads, an item id or a count of
/// hundreds of meseta.
fn treasure_chests(record: &MapRecord, references: &[Json]) -> Result<Vec<Treasure>, PackError> {
    let mut out = Vec::with_capacity(references.len());
    for (index, (entry, reference)) in record
        .treasure_chests
        .entries
        .iter()
        .zip(references)
        .enumerate()
    {
        let chest = &entry.record;
        let item_id = (!chest.is_meseta).then_some(u16::from(chest.contents));
        let item_symbol = match item_id {
            Some(id) => match usize::from(id)
                .checked_sub(1)
                .and_then(|i| ITEM_SYMBOLS.get(i))
            {
                Some(symbol) => Some((*symbol).to_owned()),
                None => {
                    return Err(PackError(format!(
                        "treasure chest at {} names item {id}, outside the {}-entry item table",
                        hex6(entry.rom_offset),
                        ITEM_SYMBOLS.len()
                    )));
                }
            },
            None => None,
        };
        let (x, y) = (u32::from(chest.x_tile), u32::from(chest.y_tile));
        out.push(Treasure {
            index: index as u32,
            record_offset: hex6(entry.rom_offset),
            x_cell: x,
            y_cell: standing(y),
            x_pixels: x * 16,
            y_pixels: y * 16,
            white_chest: chest.white_chest,
            object_symbol: Some(
                if chest.white_chest {
                    "WhiteTreasureChest"
                } else {
                    "TreasureChest"
                }
                .to_owned(),
            ),
            sprite: Some(sprite_ref(reference)?),
            contents_type: if chest.is_meseta {
                ContentsType::Meseta
            } else {
                ContentsType::Item
            },
            item_id,
            item_symbol,
            meseta: chest.is_meseta.then_some(u32::from(chest.contents) * 100),
            chest_flag: u16::from(chest.chest_flag),
        });
    }
    Ok(out)
}

fn int_rows(cells: &[u8], width: usize) -> Vec<Json> {
    cells
        .chunks_exact(width)
        .map(|row| Json::from(row.to_vec()))
        .collect()
}

/// `bind_updates`' palette: the map's 48 words with line 2 spliced in.
fn update_palette(rom: &[u8], pointer: usize) -> Result<Vec<u16>, PackError> {
    let mut words = Vec::with_capacity(64);
    for i in 0..48 {
        if i == 32 {
            for j in 0..16 {
                words.push(w(rom, PAL_INIT_LINE_3 + j * 2)?);
            }
        }
        words.push(w(rom, pointer + i * 2)?);
    }
    Ok(words)
}

/// The file, and the warp anomalies the manifest reports.
pub fn map_file(
    rom: &[u8],
    record: &MapRecord,
    section: &MapSection,
    pictures: &Pictures,
    sprites: &MapSprites,
    programs: &[Json],
) -> Result<(Json, Vec<Json>), PackError> {
    let decoded: &MapLayout = section.layout();
    let grid = &decoded.collision;
    let layout = decoded.collision_layout();
    if (grid.width, grid.height) != (layout.width_chunks * 2, layout.height_chunks * 2) {
        return Err(PackError(format!(
            "map 0x{:03X}: collision grid is {}x{} cells but its plane is {}x{} chunks",
            record.id, grid.width, grid.height, layout.width_chunks, layout.height_chunks
        )));
    }
    let doc = record.to_json(rom);
    let part = |key: &str| -> Result<&Json, PackError> {
        field(&doc, key).ok_or_else(|| PackError(format!("the record has no {key}")))
    };
    let (warps, anomalies) = warps(record, grid)?;
    let stem = super::pictures::map_stem(record);
    let collision_rows = int_rows(&grid.types, grid.width);
    let collision_json = Json::object()
        .with("plane", decoded.spec.collision_plane().name())
        .with("plane_byte", decoded.spec.collision_plane)
        .with("width_cells", grid.width)
        .with("height_cells", grid.height)
        .with("rows", collision_rows.clone());
    let collision: Collision = typed(&collision_json, "the collision grid")?;
    let vehicle_rows = int_rows(&layout.cells, layout.width_chunks);
    let vehicle_json = Json::object()
        .with("plane", layout.plane.name())
        .with("width_chunks", layout.width_chunks)
        .with("height_chunks", layout.height_chunks)
        .with("rows", vehicle_rows);
    let vehicle_battle: VehicleBattleLayout = typed(&vehicle_json, "the vehicle battle layout")?;
    let overworld = section.overworld();
    let layout_patches: Vec<Json> = overworld.map_or(Vec::new(), |world| {
        world.patches.iter().map(|patch| patch.to_json()).collect()
    });
    let png = format!("maps/{stem}.png");
    let has_over = pictures
        .files
        .iter()
        .any(|(name, _)| *name == format!("maps/{stem}_over.png"));
    let map_effects_json = Json::from(pictures.map_effects.clone());
    let variants_json = Json::from(pictures.variants.clone());
    let updates_json = Json::from(programs.to_vec());
    let map_update_images = pictures
        .update_images
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let file = MapFile {
        format_version: FORMAT_VERSION,
        id: MapId(record.id as u16),
        symbol: Some(record.symbol.to_owned()),
        png: png.clone(),
        png_over: has_over.then(|| format!("maps/{stem}_over.png")),
        dimensions: Dimensions {
            width_cells: grid.width as u32,
            height_cells: grid.height as u32,
            width_chunks: layout.width_chunks as u32,
            height_chunks: layout.height_chunks as u32,
            width_pixels: layout.width_pixels() as u32,
            height_pixels: layout.height_pixels() as u32,
            cell_pixels: COLLISION_CELL_PIXELS as u32,
        },
        collision,
        scroll: typed(part("scroll")?, "the scroll section")?,
        music: typed(part("music")?, "the music")?,
        flags: typed(part("flags")?, "the flags")?,
        dialogue_tree: record.dialogue.tree as u8,
        events: record.events.ids.clone(),
        warps,
        interaction_areas: interaction_areas(rom, record, &doc, grid)?,
        npcs: npcs(record, &sprites.npcs)?,
        treasure_chests: treasure_chests(record, &sprites.chests)?,
        map_effects: typed(&map_effects_json, "the map effects")?,
        map_updates: Some(typed(&updates_json, "the map updates")?),
        map_update_palette: Some(update_palette(rom, record.palette.pointer)?),
        map_update_images,
        layout_variants: typed(&variants_json, "the layout variants")?,
        vehicle_battle: Some(vehicle_battle),
        patch_tiles: pictures
            .patch_tiles
            .as_ref()
            .map(|tiles| typed(tiles, "the patch tiles"))
            .transpose()?,
        layout_patches: typed(&Json::from(layout_patches.clone()), "the layout patches")?,
        overworld_patches: overworld
            .map(|_| {
                typed(
                    &Json::from(pictures.overworld_patches.clone()),
                    "the overworld patches",
                )
            })
            .transpose()?,
    };
    let value = serde_json::to_value(&file)
        .map_err(|error| PackError(format!("cannot serialize map {:#x}: {error}", record.id)))?;
    let mut out = Json::from(&value);

    // What the game doesn't read, and what the Python builder writes differently.
    let warp_targets: Vec<Json> = file
        .warps
        .iter()
        .map(|warp| {
            let target = usize::from(warp.target.id.0);
            Json::object().with(
                "target",
                Json::object().with("id_hex", text(&map_reference(target), "id_hex")),
            )
        })
        .collect();
    let npc_sprites: Vec<Json> = sprites
        .npcs
        .iter()
        .map(|meta| Json::object().with("sprite", meta.sprite.clone()))
        .collect();
    let chest_sprites: Vec<Json> = sprites
        .chests
        .iter()
        .map(|reference| Json::object().with("sprite", reference.clone()))
        .collect();
    let research = Json::object()
        .with("id_hex", text(&map_reference(record.id), "id_hex"))
        .with("record_offset", hex6(record.pointer))
        .with("collision", Json::object().with("rows", collision_rows))
        .with("scroll", part("scroll")?.clone())
        .with("warps", warp_targets)
        .with("npcs", npc_sprites)
        .with("treasure_chests", chest_sprites);
    overlay(&mut out, &research);
    // These came as an extractor's JSON and the reader's type parsed them, so
    // they go out as they came: the type would add a null for each field a
    // write of another kind doesn't have.
    out.insert("map_effects", map_effects_json);
    out.insert("map_updates", updates_json);
    out.insert("layout_variants", variants_json);
    if let Some(tiles) = &pictures.patch_tiles {
        out.insert("patch_tiles", tiles.clone());
    }
    Ok((out, anomalies))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_facing_is_named_only_when_it_is_one_of_the_four() {
        assert_eq!(facing(8).name, Some(Direction::Right));
        assert_eq!(facing(0xC).name, Some(Direction::Left));
        assert_eq!(facing(0x10).name, None);
        assert_eq!(facing(0x10).id, 0x10);
    }

    #[test]
    fn an_event_index_keeps_its_high_bit_and_the_parameter_is_bounded() {
        let mut rom = vec![0u8; INTERACTION_EVENT_INDEXES + 64];
        rom[INTERACTION_EVENT_INDEXES + 62] = 0x80;
        rom[INTERACTION_EVENT_INDEXES + 63] = 0x1E;
        assert_eq!(interaction_event_index(&rom, 31).unwrap(), 0x801E);
        assert_eq!(interaction_event_index(&rom, 0).unwrap(), 0);
        let error = interaction_event_index(&rom, 32).unwrap_err();
        assert!(
            error.0.contains("outside the 32-entry table"),
            "{}",
            error.0
        );
    }

    #[test]
    fn the_standing_cell_is_one_row_below() {
        assert_eq!(standing(0), 1);
    }

    #[test]
    fn the_update_palette_splices_line_2_at_word_32() {
        let mut rom = vec![0u8; PAL_INIT_LINE_3 + 32];
        for i in 0..16 {
            rom[PAL_INIT_LINE_3 + i * 2 + 1] = 0xA0 + i as u8;
        }
        for i in 0..48 {
            rom[i * 2 + 1] = i as u8;
        }
        let words = update_palette(&rom, 0).unwrap();
        assert_eq!(words.len(), 64);
        assert_eq!(
            (words[31], words[32], words[47], words[48]),
            (31, 0xA0, 0xAF, 32)
        );
    }

    #[test]
    fn a_shape_the_reader_refuses_fails_by_name() {
        let error = typed::<Vec<Treasure>>(&Json::from("nope"), "the chests").unwrap_err();
        assert!(
            error.0.starts_with("the reader's type refuses the chests"),
            "{}",
            error.0
        );
    }

    #[test]
    fn a_sprite_facing_past_the_four_is_unknown_to_the_type() {
        let reference = Json::object()
            .with("sheet", "S")
            .with("facing", "facing_0x10")
            .with("idle_sequence", "idle_facing_0x10")
            .with("walk_sequence", "walk_facing_0x10");
        assert_eq!(
            sprite_ref(&reference).unwrap().facing,
            Some(SpriteFacing::Unknown)
        );
        assert_eq!(
            sprite_ref(&reference.with("facing", Json::Null))
                .unwrap()
                .facing,
            None
        );
    }
}
