//! The maps section's part of `manifest.json`: what `build_pack`'s map loop
//! gathers for `psiv_tools/pack_manifest.py`, which gives each key's reason.
//! [`map_part`] is one map's share, computed on the map's own thread;
//! [`fragment`] lays every map's share out in map order.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::draw::OVERLAY_TRANSPARENT_INDICES;
use super::layouts::{collision_type_name, unloaded_patterns};
use super::overworld::{MapSection, check_code_sites, opening_patches};
use super::patches::ATLAS_TILE_PIXELS;
use super::pictures::{Pictures, map_stem};
use super::records::{MapRecord, XY_RANGES, map_reference};
use super::{ExtractedMaps, MapEntry};
use crate::json::Json;
use crate::sprites::emit::{FieldSprites, SpritesWritten};
use crate::sprites::field::{MAP_PALETTE_LINE_ORDER, PAL_INIT_LINE_3_CRAM_LINE};
use crate::sprites::objects::{FIELD_OBJECT_COUNT, FIELD_OBJECTS_JMP_TBL};
use crate::sprites::records::FACINGS;
use crate::{PackError, png};

const COLLISION_CELL_PIXELS: usize = 16;
const STANDING_CELL_Y_OFFSET: usize = 1;
const COLLISION_TYPE_SPACE: usize = 16;
const NAMED_COLLISION_TYPES: [u8; 8] = [0x0, 0x1, 0x2, 0x8, 0x9, 0xA, 0xB, 0xC];
const BLOCKING_COLLISION_TYPES: [u8; 5] = [0x8, 0x9, 0xA, 0xB, 0xC];
/// `TRANSITION_TABLES`' table numbers.
const TRANSITION_TABLES: [u8; 2] = [1, 2];
const OVERWORLD_MAP_IDS: [usize; 2] = [0, 1];
const PAL_INIT_LINE_3: usize = 0x296300;
/// `PLANE_BYTES`: fg is 0, bg is 1.
const PLANE_FG: i128 = 0;
const PLANE_BG: i128 = 1;
/// `step_timing`'s table.
const MOVEMENTS_TBL: usize = 0x047AA8;
const MOVEMENT_BLOCKS: usize = 3;
const MOVEMENT_ENTRIES: usize = 16;
const MOVEMENT_ENTRY_SIZE: usize = 8;
const MOVEMENT_NORMAL_BLOCK: usize = 1;

/// `_target`: a map named by id, hex id and symbol.
fn target(record: &MapRecord) -> Json {
    Json::object()
        .with("id", record.id)
        .with("id_hex", format!("0x{:03X}", record.id))
        .with("symbol", record.symbol)
}

/// `{**target, **extra}`: the target's keys first, overwritten in place.
fn with_target(target: &Json, extra: &Json) -> Json {
    let mut out = target.clone();
    crate::json::overlay(&mut out, extra);
    out
}

fn ints(value: &Json, key: &str) -> Result<Vec<i128>, PackError> {
    value.at(&[key])?.items()?.iter().map(Json::int).collect()
}

/// One map's share of the manifest.
pub struct MapPart {
    pub id: usize,
    inventory: Json,
    priority_fg: usize,
    priority_bg: usize,
    opaque_pixels: usize,
    without_overlay: Option<Json>,
    warp_anomalies: Vec<(u8, Json)>,
    layout_anomalies: Vec<Json>,
    overworld: Option<Json>,
    unloaded: Option<Json>,
    collision: [usize; 16],
    dialogue_tree: i128,
    events: Vec<i128>,
    warp_targets: Vec<(i128, Json)>,
    warp_facings: Vec<i128>,
    npc_facings: Vec<i128>,
    npc_interactable: Vec<bool>,
    mute_dialogue: Vec<Json>,
    patch_map: Option<Json>,
    patch_counts: Option<Json>,
    scene_patch: Option<Json>,
    variants: Option<Json>,
}

/// One map's share: from its record, its decoded section, its pictures, the
/// map file as written (with its digest) and the warp anomalies `map_file`
/// found.
pub fn map_part(
    record: &MapRecord,
    section: &MapSection,
    pictures: &Pictures,
    file: &Json,
    json_sha256: String,
    anomalies: Vec<Json>,
) -> Result<MapPart, PackError> {
    let target = target(record);
    let stem = map_stem(record);
    let decoded = section.layout();
    let image = &pictures.files[0].1;
    let over = pictures
        .has_overlay
        .then(|| &pictures.files[1].1)
        .map(|data| png::image_sha256(data))
        .transpose()?;
    let priority_tiles = pictures.priority.tiles_fg + pictures.priority.tiles_bg;
    let dimensions = file.at(&["dimensions"])?;
    let inventory = with_target(
        &target,
        &Json::object()
            .with("json", format!("maps/{stem}.json"))
            .with("png", format!("maps/{stem}.png"))
            .with(
                "png_over",
                pictures
                    .has_overlay
                    .then(|| format!("maps/{stem}_over.png")),
            )
            .with("json_sha256", json_sha256)
            .with("png_sha256", png::image_sha256(image)?)
            .with("png_over_sha256", over)
            .with("priority_tiles", priority_tiles)
            .with("width_cells", dimensions.at(&["width_cells"])?.clone())
            .with("height_cells", dimensions.at(&["height_cells"])?.clone())
            .with("width_pixels", dimensions.at(&["width_pixels"])?.clone())
            .with("height_pixels", dimensions.at(&["height_pixels"])?.clone()),
    );

    let mut warp_anomalies = Vec::with_capacity(anomalies.len());
    for anomaly in anomalies {
        let table = anomaly.at(&["table"])?.int()? as u8;
        let mut entry = with_target(&target, &anomaly);
        if let (Some(world), Some(rect @ Json::Object(_))) =
            (section.overworld(), anomaly.get("rect"))
        {
            // A world-map door with no map-change cell is one an event opens:
            // each patch is applied to the collision plane to find which.
            let field =
                |key: &str| -> Result<usize, PackError> { Ok(rect.at(&[key])?.int()? as usize) };
            let (x, y, width, height) =
                (field("x")?, field("y")?, field("width")?, field("height")?);
            let cells: Vec<(usize, usize)> = (y..y + height)
                .flat_map(|cy| (x..x + width).map(move |cx| (cx, cy)))
                .collect();
            let opened_by: Vec<Json> = opening_patches(world, &cells)?
                .iter()
                .map(|patch| {
                    Json::object()
                        .with("id", patch.event_flag)
                        .with("id_hex", format!("0x{:02X}", patch.event_flag))
                        .with("symbol", patch.event_symbol())
                        .with("routine", format!("0x{:06X}", patch.routine))
                })
                .collect();
            entry.insert("opened_by", opened_by);
        }
        warp_anomalies.push((table, entry));
    }

    let missing = unloaded_patterns(decoded)?;
    let unloaded = (!missing.is_empty())
        .then(|| with_target(&target, &Json::object().with("patterns", missing)));

    let mut warp_targets = Vec::new();
    let mut warp_facings = Vec::new();
    for warp in file.at(&["warps"])?.items()? {
        let warp_target = warp.at(&["target"])?;
        warp_targets.push((warp_target.at(&["id"])?.int()?, warp_target.clone()));
        warp_facings.push(warp.at(&["facing", "id"])?.int()?);
    }
    let mut npc_facings = Vec::new();
    let mut npc_interactable = Vec::new();
    let mut mute_dialogue = Vec::new();
    for npc in file.at(&["npcs"])?.items()? {
        npc_facings.push(npc.at(&["facing", "id"])?.int()?);
        let interactable = npc.at(&["interactable"])? == &Json::Bool(true);
        npc_interactable.push(interactable);
        let dialogue = npc.at(&["dialogue_id"])?;
        if !matches!(dialogue, Json::Null | Json::Int(0)) && !interactable {
            mute_dialogue.push(with_target(
                &target,
                &Json::object()
                    .with("npc_index", npc.at(&["index"])?.clone())
                    .with("symbol", npc.at(&["symbol"])?.clone())
                    .with("dialogue_id", dialogue.clone()),
            ));
        }
    }

    let counts = pictures.patch_counts.to_json();
    let patch_map = (pictures.atlas && pictures.patched_chunks > 0).then(|| {
        with_target(
            &target,
            &Json::object()
                .with("writes", pictures.patch_counts.layout_writes)
                .with("chunks", pictures.patched_chunks)
                .with("cells", pictures.patch_counts.cells)
                .with("has_overlay", pictures.atlas_over),
        )
    });
    Ok(MapPart {
        id: record.id,
        inventory,
        priority_fg: pictures.priority.tiles_fg,
        priority_bg: pictures.priority.tiles_bg,
        opaque_pixels: pictures.priority.opaque_pixels,
        without_overlay: (!pictures.has_overlay).then(|| {
            with_target(
                &target,
                &Json::object().with("priority_tiles", priority_tiles),
            )
        }),
        warp_anomalies,
        layout_anomalies: section
            .anomalies()
            .iter()
            .map(|anomaly| with_target(&target, anomaly))
            .collect(),
        overworld: section.overworld().map(|world| world.to_json()),
        unloaded,
        collision: decoded.collision.histogram(),
        dialogue_tree: file.at(&["dialogue_tree"])?.int()?,
        events: ints(file, "events")?,
        warp_targets,
        warp_facings,
        npc_facings,
        npc_interactable,
        mute_dialogue,
        patch_map,
        patch_counts: pictures.atlas.then_some(counts),
        scene_patch: (!pictures.scene_chunks.is_empty()).then(|| {
            with_target(
                &target,
                &Json::object().with("chunks", pictures.scene_chunks.clone()),
            )
        }),
        variants: (!pictures.variants.is_empty()).then(|| {
            with_target(
                &target,
                &Json::object().with("variants", pictures.variants.len()),
            )
        }),
    })
}

/// Add each integer field of `counts` into `totals`, keeping first-seen order.
fn add_counts(totals: &mut Json, counts: &Json) -> Result<(), PackError> {
    let Json::Object(pairs) = counts else {
        return Err(PackError("a count record is not an object".into()));
    };
    for (key, value) in pairs {
        let sum = totals.get(key).map_or(Ok(0), Json::int)? + value.int()?;
        totals.insert(key.clone(), sum);
    }
    Ok(())
}

fn tally(counts: &BTreeMap<i128, i128>) -> Json {
    Json::Object(
        counts
            .iter()
            .map(|(key, value)| (key.to_string(), Json::from(*value)))
            .collect(),
    )
}

/// `step_timing_json`: what one cell of walking costs, per
/// `FieldObj_Step_Offset` block (`psiv_tools/sprites/field.py`).
pub fn step_timing_json(rom: &[u8]) -> Result<Json, PackError> {
    let mut blocks = Vec::with_capacity(MOVEMENT_BLOCKS);
    let mut normal = None;
    for block in 0..MOVEMENT_BLOCKS {
        let mut speeds = BTreeSet::new();
        for entry in 0..MOVEMENT_ENTRIES {
            let base = MOVEMENTS_TBL + (block * MOVEMENT_ENTRIES + entry) * MOVEMENT_ENTRY_SIZE;
            let bytes = rom.get(base..base + 6).ok_or_else(|| {
                PackError(format!("movement entry at 0x{base:06X} runs past the ROM"))
            })?;
            let durations = [bytes[0], bytes[1]];
            let steps = [
                i16::from_be_bytes([bytes[2], bytes[3]]),
                i16::from_be_bytes([bytes[4], bytes[5]]),
            ];
            for (duration, step) in durations.into_iter().zip(steps) {
                if duration == 0 || step == 0 {
                    continue;
                }
                let word = u32::from(step.unsigned_abs());
                let total = u32::from(duration) << 8;
                if total % word != 0 || word % 256 != 0 {
                    return Err(PackError(format!(
                        "movement block {block} entry {entry} does not divide into whole \
                         frames: duration 0x{total:X}, step 0x{word:X}"
                    )));
                }
                speeds.insert((total / word, word));
            }
        }
        if speeds.len() != 1 {
            return Err(PackError(format!(
                "movement block {block} holds {} distinct speeds; the table is not three \
                 uniform blocks",
                speeds.len()
            )));
        }
        let (frames, word) = speeds.into_iter().next().unwrap_or_default();
        if (word / 256) * frames != COLLISION_CELL_PIXELS as u32 {
            return Err(PackError(format!(
                "movement block {block} covers {} pixels per step, not the \
                 {COLLISION_CELL_PIXELS}-pixel collision cell",
                (word / 256) * frames
            )));
        }
        if block == MOVEMENT_NORMAL_BLOCK {
            normal = Some(frames);
        }
        blocks.push(
            Json::object()
                .with("block", block)
                .with("step_constant", format!("0x{word:04X}"))
                .with("pixels_per_frame", word / 256)
                .with("frames_per_cell", frames),
        );
    }
    Ok(Json::object()
        .with("table", format!("0x{MOVEMENTS_TBL:06X}"))
        .with("selector", "FieldObj_Step_Offset")
        .with("cell_pixels", COLLISION_CELL_PIXELS)
        .with("normal_block", MOVEMENT_NORMAL_BLOCK)
        .with("frames_per_cell", normal)
        .with("blocks", blocks)
        .with(
            "note",
            "Animation is not locked to the step: FieldObj_Animate free-runs on its own \
             per-sequence duration and FieldObj_Move only resets it to frame 0 when the object \
             stops moving.",
        ))
}

/// Everything the maps section knows besides the maps: the records, the
/// sprite work, what the sprite writer wrote, and the effect and update
/// censuses.
pub struct Inputs<'a> {
    pub maps: &'a ExtractedMaps,
    pub field: &'a FieldSprites,
    pub sprites: SpritesWritten,
    pub effects_census: &'a Json,
    pub updates_census: &'a Json,
    pub shops: Json,
}

/// The maps section's manifest keys, from every map's part (in any order).
pub fn fragment(
    rom: &[u8],
    root: &Path,
    mut parts: Vec<MapPart>,
    inputs: Inputs,
) -> Result<Json, PackError> {
    parts.sort_by_key(|part| part.id);
    let mut census: BTreeMap<&str, BTreeMap<i128, i128>> = [
        "collision_types",
        "npc_facing_bytes",
        "warp_facing_bytes",
        "dialogue_trees",
        "sprite_palette_lines",
        "priority_tiles",
        "event_ids",
        "npc_interactable",
    ]
    .into_iter()
    .map(|key| (key, BTreeMap::new()))
    .collect();
    let mut count = |key: &str, value: i128, by: i128| {
        if let Some(counts) = census.get_mut(key) {
            *counts.entry(value).or_insert(0) += by;
        }
    };
    let mut inventory = Vec::new();
    let mut without_overlay = Vec::new();
    let mut warp_anomalies = Vec::new();
    let mut layout_anomalies = Vec::new();
    let mut overworlds = Vec::new();
    let mut unloaded = Vec::new();
    let mut warp_targets: BTreeMap<i128, Json> = BTreeMap::new();
    let mut mute_dialogue = Vec::new();
    let mut patch_maps = Vec::new();
    let mut patch_totals = Json::object();
    let mut scene_patch_maps = Vec::new();
    let mut variant_maps = Vec::new();
    let (mut priority_tiles, mut opaque_pixels, mut warp_count) = (0, 0, 0);
    for part in parts {
        for (plane, placed) in [(PLANE_FG, part.priority_fg), (PLANE_BG, part.priority_bg)] {
            if placed > 0 {
                count("priority_tiles", plane, placed as i128);
            }
        }
        priority_tiles += part.priority_fg + part.priority_bg;
        opaque_pixels += part.opaque_pixels;
        without_overlay.extend(part.without_overlay);
        warp_anomalies.extend(part.warp_anomalies);
        layout_anomalies.extend(part.layout_anomalies);
        overworlds.extend(part.overworld);
        unloaded.extend(part.unloaded);
        warp_count += part.warp_targets.len();
        for (value, cells) in part.collision.iter().enumerate() {
            if *cells > 0 {
                count("collision_types", value as i128, *cells as i128);
            }
        }
        count("dialogue_trees", part.dialogue_tree, 1);
        for event in part.events {
            count("event_ids", event, 1);
        }
        for ((id, warp_target), facing) in part.warp_targets.into_iter().zip(part.warp_facings) {
            warp_targets.entry(id).or_insert(warp_target);
            count("warp_facing_bytes", facing, 1);
        }
        for facing in part.npc_facings {
            count("npc_facing_bytes", facing, 1);
        }
        for interactable in part.npc_interactable {
            count("npc_interactable", i128::from(interactable), 1);
        }
        mute_dialogue.extend(part.mute_dialogue);
        if let Some(counts) = &part.patch_counts {
            add_counts(&mut patch_totals, counts)?;
        }
        patch_maps.extend(part.patch_map);
        scene_patch_maps.extend(part.scene_patch);
        variant_maps.extend(part.variants);
        inventory.push(part.inventory);
    }
    let mut placed = 0;
    for &(line, placements) in &inputs.sprites.npc_lines {
        count("sprite_palette_lines", i128::from(line), placements as i128);
        placed += placements;
    }
    let packed: BTreeSet<i128> = inventory
        .iter()
        .map(|entry| entry.at(&["id"]).and_then(Json::int))
        .collect::<Result<_, _>>()?;

    let mut skipped = Vec::new();
    for entry in &inputs.maps.entries {
        if let MapEntry::Null { id, .. } = entry {
            skipped.push(map_reference(*id).with(
                "reason",
                "PtrMap_Null placeholder; the table entry points at ErrorTrap",
            ));
        }
    }

    // The sprite work's artless objects and palette copies, map by map.
    let mut artless = Vec::new();
    let mut palette_totals = Json::object();
    let records: BTreeMap<usize, &MapRecord> = inputs
        .maps
        .real()
        .map(|record| (record.id, record))
        .collect();
    for map in &inputs.field.maps {
        let target = records
            .get(&map.id)
            .map(|record| target(record))
            .ok_or_else(|| {
                PackError(format!(
                    "sprites for map 0x{:03X}, which has no record",
                    map.id
                ))
            })?;
        for entry in &map.artless {
            artless.push(with_target(&target, entry));
        }
        add_counts(
            &mut palette_totals,
            &Json::object()
                .with("palette_copies", map.palette_counts[0])
                .with("alternate_sheets", map.palette_counts[1])
                .with("repainted_npcs", map.palette_counts[2]),
        )?;
    }

    let routines = &inputs.field.routines;
    let [party, npcs, vehicles] = &inputs.sprites.files;
    let (_, sprite_bytes) = crate::png_census(root, "sprites")?;
    let sprites = Json::object()
        .with("party", party.0)
        .with("party_sha256", party.1.as_str())
        .with("party_sheet_count", party.2)
        .with("npcs", npcs.0)
        .with("npcs_sha256", npcs.1.as_str())
        .with("npc_sheet_count", npcs.2)
        .with("vehicles", vehicles.0)
        .with("vehicles_sha256", vehicles.1.as_str())
        .with("vehicle_sheet_count", vehicles.2)
        .with(
            "vehicle_palette_source",
            "each selected map palette, CRAM line 3; base is first selected map",
        )
        .with("npc_placements", placed)
        .with("artless_objects", artless.len())
        .with(
            "interaction",
            Json::object()
                .with("render_flags_bit", 3u8)
                .with(
                    "types_interactable",
                    routines.iter().filter(|r| r.interactable).count(),
                )
                .with(
                    "types_not_interactable",
                    routines.iter().filter(|r| !r.interactable).count(),
                )
                .with(
                    "types_changing_at_runtime",
                    routines
                        .iter()
                        .filter(|r| r.interactable_changes_at_runtime)
                        .map(|r| r.symbol)
                        .collect::<Vec<_>>(),
                )
                .with("dialogue_id_but_not_interactable", mute_dialogue),
        )
        .with("census", inputs.field.census.to_json())
        .with("artless", artless)
        .with("bytes", sprite_bytes)
        .with(
            "field_objects",
            Json::object()
                .with("table", format!("0x{FIELD_OBJECTS_JMP_TBL:06X}"))
                .with("count", FIELD_OBJECT_COUNT)
                .with("stride", 4u8),
        )
        .with(
            "palette",
            Json::object()
                .with(
                    "selector",
                    "$13(a4), OR-ed into the pattern word's high byte",
                )
                .with(
                    "cram_lines",
                    Json::object()
                        .with("0x00", 0u8)
                        .with("0x20", 1u8)
                        .with("0x40", 2u8)
                        .with("0x60", 3u8),
                )
                .with("map_palette_lines", MAP_PALETTE_LINE_ORDER.to_vec())
                .with("fixed_line", PAL_INIT_LINE_3_CRAM_LINE)
                .with("fixed_line_source", "Pal_Init_Line_3")
                .with("fixed_line_rom_offset", format!("0x{PAL_INIT_LINE_3:06X}"))
                .with("party_line", PAL_INIT_LINE_3_CRAM_LINE),
        )
        .with(
            "facings",
            Json::Object(
                FACINGS
                    .iter()
                    .map(|&(value, name)| (value.to_string(), Json::from(name)))
                    .collect(),
            ),
        )
        .with("walk", step_timing_json(rom)?)
        .with(
            "note",
            "Frame durations are game frames, one per Field_RunObjects call. idle_<dir> is \
             frame 0 of the direction's sequence, which is where FieldObj_Move parks a stopped \
             object; walk_<dir> is the whole cycle.",
        );

    let mut without_cell = Json::object();
    for table in TRANSITION_TABLES {
        without_cell.insert(
            format!("table_{table}"),
            warp_anomalies.iter().filter(|(t, _)| *t == table).count(),
        );
    }
    let doors: Vec<Json> = warp_anomalies
        .into_iter()
        .filter(|(table, _)| *table == 2)
        .map(|(_, anomaly)| anomaly)
        .collect();
    let mut map_effects = inputs.effects_census.clone();
    let patch_map_count = patch_maps.len();
    map_effects.insert("scene_patch_chunks", scene_patch_maps);
    let mut resolution = patch_totals;
    resolution.insert("maps", patch_maps);
    resolution.insert("map_count", patch_map_count);
    resolution.insert("atlas_tile_pixels", ATLAS_TILE_PIXELS);
    resolution.insert("cells_per_chunk", 4u8);
    resolution.insert(
        "note",
        "Each layout_write carries `cells` (the 2x2 collision cells the written chunk imposes) \
         and `patch_tile` (an index into that map's patch_tiles atlas).",
    );
    map_effects.insert("layout_write_resolution", resolution);
    let mut copies = palette_totals;
    copies.insert("cram_line", 3u8);
    copies.insert("map_tiles_affected", 0u8);
    copies.insert(
        "note",
        "Chunk words carry one palette bit, so map tiles reach CRAM lines 0 and 1. A copy to \
         line 3 repaints the field objects whose routine stores $60; the alternates are \
         ordinary sheets named per NPC on the path's deferred_effects.",
    );
    map_effects.insert("palette_copies", copies);
    map_effects.insert("maps_with_layout_variants", variant_maps);
    map_effects.insert("slice", 1u8);
    map_effects.insert(
        "kinds_emitted",
        vec![
            "object_despawn",
            "object_rewrite",
            "object_dialogue",
            "object_x",
            "object_y",
            "layout_write",
            "layout_replace",
        ],
    );
    map_effects.insert(
        "kinds_deferred",
        "palette, scroll, chunk-table rewrites, alternate colours and flag writes are later \
         slices; a path that performs one carries it in `deferred` rather than dropping it",
    );

    let maps_with_overlay = inventory
        .iter()
        .filter(|entry| entry.get("png_over").is_some_and(|v| *v != Json::Null))
        .count();
    let census_json = Json::Object(
        census
            .iter()
            .map(|(key, counts)| ((*key).to_owned(), tally(counts)))
            .collect(),
    );
    let unpacked: Vec<Json> = warp_targets
        .into_iter()
        .filter(|(id, _)| !packed.contains(id))
        .map(|(_, target)| target)
        .collect();
    Ok(Json::object()
        .with(
            "collision",
            Json::object()
                .with("cell_pixels", COLLISION_CELL_PIXELS)
                .with("type_space", COLLISION_TYPE_SPACE)
                .with(
                    "type_names",
                    Json::Object(
                        NAMED_COLLISION_TYPES
                            .iter()
                            .map(|&value| {
                                (format!("0x{value:X}"), Json::from(collision_type_name(value)))
                            })
                            .collect(),
                    ),
                )
                .with("named_types_are_not_the_valid_set", true)
                .with("blocking_types", BLOCKING_COLLISION_TYPES.to_vec()),
        )
        .with(
            "warps",
            Json::object()
                .with("rect_units", "collision cells")
                .with("standing_cell_y_offset", STANDING_CELL_Y_OFFSET)
                .with("count", warp_count)
                .with(
                    "xy_ranges",
                    Json::Object(
                        XY_RANGES
                            .iter()
                            .enumerate()
                            .map(|(value, name)| (value.to_string(), Json::from(*name)))
                            .collect(),
                    ),
                )
                .with("without_map_change_cell", without_cell)
                .with("doors_without_map_change_cell", doors),
        )
        .with("sprites", sprites)
        .with("map_count", inventory.len())
        .with("map_updates", inputs.updates_census.clone())
        .with("map_effects", map_effects)
        .with(
            "overlays",
            Json::object()
                .with("file_suffix", "_over.png")
                .with("priority_bit", 15u8)
                .with(
                    "draw_order",
                    vec![
                        "high-priority sprites",
                        "<id>_<symbol>_over.png",
                        "field sprites",
                        "<id>_<symbol>.png",
                    ],
                )
                .with(
                    "transparent_palette_indices",
                    OVERLAY_TRANSPARENT_INDICES.to_vec(),
                )
                .with(
                    "composited",
                    "plane B first, plane A over it, colour 0 transparent",
                )
                .with("maps_with_overlay", maps_with_overlay)
                .with("maps_without_overlay", without_overlay.len())
                .with("priority_tiles", priority_tiles)
                .with("opaque_pixels", opaque_pixels)
                .with("without_overlay", without_overlay)
                .with(
                    "note",
                    "Bit 15 survives ChunkTilesToBuffer's #$BFFF mask, so it is real VDP data: \
                     a map tile carrying it draws above an ordinary sprite. A field object whose \
                     flag byte has bit 4 set is a high-priority sprite (Field_FillSpriteAttributes' \
                     bset #7) and belongs above the overlay; the party never is.",
                ),
        )
        .with("maps", inventory)
        .with(
            "overworld",
            Json::object()
                .with("map_ids", OVERWORLD_MAP_IDS.to_vec())
                .with("selector", "Field_Map_Index & $FFFE == 0")
                .with(
                    "note",
                    "loc_539E2/loc_53A04 read no layout pointer for these two; both planes \
                     stream 1KB pages of raw chunk ids from fixed tables into a four-page rolling \
                     window. Nothing on this path is compressed. layout_patches on the map \
                     record are the event-gated writes the page loader performs; MapDataManager \
                     effects are not decoded for any map, including MapDataMan_Dezolis, which \
                     rewrites Dezolis' chunk definitions once EventFlag_DarkForce2 is set.",
                )
                .with("code", check_code_sites(rom)?)
                .with("maps", overworlds),
        )
        .with("skipped", skipped)
        .with("unpacked_warp_targets", unpacked)
        .with("census", census_json)
        .with(
            "unloaded_patterns",
            Json::object()
                .with(
                    "note",
                    "VRAM tiles a map's chunks name that its loc_519D2 tileset list does not \
                     fill. Some are filled by the record's sprite art lists, which the pack does \
                     not stage; the rest are VRAM no part of the record writes. Either way the \
                     render leaves those pixels showing whatever is underneath.",
                )
                .with("maps", unloaded),
        )
        .with("layout_anomalies", layout_anomalies)
        .with("shops", inputs.shops))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A movement table whose three blocks step 1, 2 and 4 pixels a frame.
    fn table() -> Vec<u8> {
        let mut rom = vec![0u8; MOVEMENTS_TBL + MOVEMENT_BLOCKS * MOVEMENT_ENTRIES * 8];
        for block in 0..MOVEMENT_BLOCKS {
            let speed = 1u16 << block;
            for entry in 0..MOVEMENT_ENTRIES {
                let at = MOVEMENTS_TBL + (block * MOVEMENT_ENTRIES + entry) * 8;
                // `(duration << 8) / step` frames, `step / 256` pixels each:
                // a duration of 16 walks the 16-pixel cell at every speed.
                rom[at] = 16;
                rom[at + 1] = 16;
                rom[at + 2..at + 4].copy_from_slice(&(speed << 8).to_be_bytes());
                rom[at + 4..at + 6].copy_from_slice(&(-((speed << 8) as i16)).to_be_bytes());
            }
        }
        rom
    }

    #[test]
    fn each_movement_block_walks_one_cell_at_its_own_speed() {
        let json = serde_json::Value::from(&step_timing_json(&table()).unwrap());
        assert_eq!(json["frames_per_cell"], 8, "the normal block");
        assert_eq!(json["blocks"][0]["frames_per_cell"], 16);
        assert_eq!(json["blocks"][2]["pixels_per_frame"], 4);
        assert_eq!(json["blocks"][1]["step_constant"], "0x0200");
    }

    #[test]
    fn a_block_with_two_speeds_or_a_short_cell_is_refused() {
        let mut rom = table();
        let at = MOVEMENTS_TBL + 8;
        rom[at + 2..at + 4].copy_from_slice(&0x0400u16.to_be_bytes());
        rom[at] = 4;
        let error = step_timing_json(&rom).unwrap_err();
        assert!(error.0.contains("2 distinct speeds"), "{}", error.0);
        let mut rom = table();
        for entry in 0..MOVEMENT_ENTRIES {
            let at = MOVEMENTS_TBL + entry * 8;
            rom[at] = 8;
            rom[at + 1] = 8;
        }
        let error = step_timing_json(&rom).unwrap_err();
        assert!(error.0.contains("not the 16-pixel"), "{}", error.0);
    }
}
