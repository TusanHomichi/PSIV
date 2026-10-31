//! Which formation group a map rolls from: `psiv_tools/maps/encounters.py`,
//! which owns the retail citations (`Battle_SetupEnemyData`).

use std::collections::BTreeSet;

use super::records::{MAP_COUNT, hex6, map_reference};
use crate::PackError;
use crate::decode::{Call, Decodes};
use crate::json::Json;
use crate::map_symbols::MAP_SYMBOLS;

/// `Battle_EnemyFormationIndexes`: one byte per map id, one short of the map
/// space, read with no bounds check.
const TABLE: usize = 0x008050;
const TABLE_SIZE: usize = 416;
const SIGNATURE: &str = "00013cffffffffffffffffffffffffffffffffffff0e0f0fffffffffffffffff\
                         ffffffffffffffffffffff1011ffffffffffffffffffffffffffffffffffffff";
/// Values 0 and 1 mean a world map, not a group.
const WORLD_MAP_SENTINEL_MAX: u8 = 1;
const NO_ENCOUNTERS: u8 = 0xFF;
const POSITION_GRID_COLUMNS: usize = 0x40;

struct GridSpec {
    name: &'static str,
    label: &'static str,
    map_id: usize,
    start: usize,
    end: usize,
}

const POSITION_GRIDS: [GridSpec; 2] = [
    GridSpec {
        name: "motavia",
        label: "Battle_MotaFormationGroupIndexes",
        map_id: 0,
        start: 0x28501C,
        end: 0x2853C8,
    },
    GridSpec {
        name: "dezolis",
        label: "Battle_DezoFormationGroupIndexes",
        map_id: 1,
        start: 0x2853CC,
        end: 0x28541F,
    },
];

/// `Battle_SetupEnemyData`'s vehicle branch: (world, condition, group).
const VEHICLE_GROUPS: [(&str, &str, u8); 4] = [
    ("dezolis", "any vehicle on Dezolis", 0x0D),
    ("motavia", "Mota_Battle_BG_Index == 1", 0x09),
    ("motavia", "Mota_Battle_BG_Index == 3", 0x0A),
    ("motavia", "any other Mota_Battle_BG_Index", 0x08),
];

/// `Battle_FormationIndexes`, the groups `extract_formation_indexes` splits.
const INDEX_BLOCK_LABEL: &str = "Battle_FormationIndexes";
const INDEX_BLOCK_START: usize = 0x2836EC;
const INDEX_BLOCK_END: usize = 0x283E67;
const INDEX_GROUP_SIZE: usize = 0x40;

fn kosinski(decodes: &Decodes, offset: usize) -> Result<(Vec<u8>, usize), PackError> {
    let decoded = decodes.get(Call::Kosinski { offset })?;
    Ok((decoded.bytes().into_owned(), decoded.consumed()))
}

/// How many groups `Battle_FormationIndexes` holds.
pub fn formation_group_count(decodes: &Decodes) -> Result<usize, PackError> {
    let (block, consumed) = kosinski(decodes, INDEX_BLOCK_START)?;
    let expected = INDEX_BLOCK_END - INDEX_BLOCK_START;
    if consumed != expected {
        return Err(PackError(format!(
            "{INDEX_BLOCK_LABEL} at {}: decompressor consumed {consumed} bytes, but the \
             documented range is {expected} bytes",
            hex6(INDEX_BLOCK_START)
        )));
    }
    if !block.len().is_multiple_of(INDEX_GROUP_SIZE) {
        return Err(PackError(format!(
            "{INDEX_BLOCK_LABEL} decompressed to {} bytes, which is not a multiple of the \
             {INDEX_GROUP_SIZE}-byte group size",
            block.len()
        )));
    }
    Ok(block.len() / INDEX_GROUP_SIZE)
}

fn check_table(data: &[u8]) -> Result<&[u8], PackError> {
    let signature: Vec<u8> = (0..SIGNATURE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&SIGNATURE[i..i + 2], 16).expect("hex signature"))
        .collect();
    let actual = data.get(TABLE..TABLE + signature.len());
    if actual != Some(signature.as_slice()) {
        return Err(PackError(format!(
            "Battle_EnemyFormationIndexes signature mismatch at {}: expected {SIGNATURE}, got {}",
            hex6(TABLE),
            crate::png::hex(actual.unwrap_or_default())
        )));
    }
    if crate::m68k::find_all(data, SIGNATURE).len() != 1 {
        return Err(PackError(
            "Battle_EnemyFormationIndexes signature is not unique in this ROM; the offset would \
             not be self-verifying"
                .to_owned(),
        ));
    }
    Ok(&data[TABLE..TABLE + TABLE_SIZE])
}

fn decode_position_grid(
    decodes: &Decodes,
    spec: &GridSpec,
    group_count: usize,
) -> Result<(Json, Vec<u8>), PackError> {
    let (grid, consumed) = kosinski(decodes, spec.start)?;
    let expected = spec.end - spec.start;
    if consumed != expected {
        return Err(PackError(format!(
            "{} at {}: decompressor consumed {consumed} bytes, but the documented range is \
             {expected} bytes",
            spec.label,
            hex6(spec.start)
        )));
    }
    if !grid.len().is_multiple_of(POSITION_GRID_COLUMNS) {
        return Err(PackError(format!(
            "{} decompresses to {} bytes, which is not a whole number of \
             {POSITION_GRID_COLUMNS}-column rows",
            spec.label,
            grid.len()
        )));
    }
    let mut groups = BTreeSet::new();
    let mut fallback_cells = 0usize;
    for &value in &grid {
        if value & 0x80 != 0 {
            // `bpl` fails, so Battle_SetupEnemyData substitutes group 0.
            fallback_cells += 1;
            continue;
        }
        if usize::from(value) >= group_count {
            return Err(PackError(format!(
                "{} names encounter group {value}, but only {group_count} groups exist in \
                 Battle_FormationIndexes",
                spec.label
            )));
        }
        groups.insert(value);
    }
    let groups: Vec<u8> = groups.into_iter().collect();
    let cells: Vec<Json> = grid
        .chunks(POSITION_GRID_COLUMNS)
        .map(|row| row.to_vec().into())
        .collect();
    Ok((
        Json::object()
            .with("name", spec.name)
            .with("label", spec.label)
            .with("map", map_reference(spec.map_id))
            .with("rom_offset", hex6(spec.start))
            .with("rom_end_exclusive", hex6(spec.end))
            .with("compression", "kosinski")
            .with("compressed_size", expected)
            .with("decompressed_size", grid.len())
            .with("columns", POSITION_GRID_COLUMNS)
            .with("rows", grid.len() / POSITION_GRID_COLUMNS)
            .with("cell_size_pixels", 0x40)
            .with("indexing", "(character_y >> 6) * 64 + (character_x >> 6)")
            .with("fallback_cells", fallback_cells)
            .with("fallback_group", 0)
            .with("groups_used", groups.clone())
            .with("cells", cells),
        groups,
    ))
}

/// One map's binding: its JSON, its mode and its table byte.
pub struct Binding {
    pub json: Json,
    pub mode: &'static str,
    pub value: Option<u8>,
}

pub struct Encounters {
    pub json: Json,
    pub maps: Vec<Binding>,
}

/// `extract_encounter_binding`.
pub fn extract_encounter_binding(
    data: &[u8],
    decodes: &Decodes,
    group_count: usize,
) -> Result<Encounters, PackError> {
    let table = check_table(data)?;
    debug_assert_eq!(MAP_SYMBOLS.len(), MAP_COUNT);
    let mut grids = Vec::new();
    let mut groups_used = Vec::new();
    for spec in &POSITION_GRIDS {
        let (grid, used) = decode_position_grid(decodes, spec, group_count)?;
        grids.push(grid);
        groups_used.push(used);
    }
    for (_, condition, group) in VEHICLE_GROUPS {
        if usize::from(group) >= group_count {
            return Err(PackError(format!(
                "the vehicle encounter group {group} for {condition} is outside the \
                 {group_count} known groups"
            )));
        }
    }

    let mut maps = Vec::new();
    for (map_id, symbol) in MAP_SYMBOLS.iter().copied().enumerate() {
        let record = Json::object()
            .with("map_id", map_id)
            .with("map_id_hex", format!("0x{map_id:03X}"))
            .with("map_symbol", symbol);
        let Some(&value) = table.get(map_id) else {
            maps.push(Binding {
                json: record
                    .with("in_table", false)
                    .with("rom_offset", Json::Null)
                    .with("value", Json::Null)
                    .with("mode", "outside_table")
                    .with("group", Json::Null),
                mode: "outside_table",
                value: None,
            });
            continue;
        };
        let record = record
            .with("in_table", true)
            .with("rom_offset", hex6(TABLE + map_id))
            .with("value", value);
        let (json, mode) = if value == NO_ENCOUNTERS {
            (
                record.with("mode", "none").with("group", Json::Null),
                "none",
            )
        } else if value <= WORLD_MAP_SENTINEL_MAX {
            let Some(at) = POSITION_GRIDS.iter().position(|g| g.map_id == map_id) else {
                return Err(PackError(format!(
                    "map 0x{map_id:03X} ({symbol}) has world-map encounter sentinel {value} but \
                     is not one of the two maps with a position grid"
                )));
            };
            let name = POSITION_GRIDS[at].name;
            let vehicles: Vec<u8> = VEHICLE_GROUPS
                .iter()
                .filter(|(world, _, _)| *world == name)
                .map(|(_, _, group)| *group)
                .collect();
            (
                record
                    .with("mode", "position_grid")
                    .with("group", Json::Null)
                    .with("position_grid", name)
                    .with("groups_available", groups_used[at].clone())
                    .with("vehicle_groups", vehicles),
                "position_grid",
            )
        } else {
            if usize::from(value) >= group_count {
                return Err(PackError(format!(
                    "map 0x{map_id:03X} ({symbol}) names encounter group {value}, but only \
                     {group_count} groups exist"
                )));
            }
            (record.with("mode", "group").with("group", value), "group")
        };
        maps.push(Binding {
            json,
            mode,
            value: Some(value),
        });
    }

    let vehicle_groups: Vec<Json> = VEHICLE_GROUPS
        .iter()
        .map(|(world, condition, group)| {
            Json::object()
                .with("world", *world)
                .with("condition", *condition)
                .with("group", *group)
        })
        .collect();
    let json = Json::object()
        .with(
            "table",
            Json::object()
                .with("label", "Battle_EnemyFormationIndexes")
                .with("rom_offset", hex6(TABLE))
                .with("rom_end_exclusive", hex6(TABLE + table.len()))
                .with("entry_size", 1)
                .with("count", table.len())
                .with("map_count", MAP_COUNT)
                .with("signature_hex", SIGNATURE)
                .with("indexing", "Field_Map_Index, one byte, no bounds check")
                .with("no_encounter_value", format!("0x{NO_ENCOUNTERS:02X}"))
                .with("world_map_sentinel_max", WORLD_MAP_SENTINEL_MAX)
                .with("maps_outside_table", MAP_COUNT - table.len()),
        )
        .with("formation_group_count", group_count)
        .with("vehicle_groups", vehicle_groups)
        .with("position_grids", grids)
        .with(
            "maps",
            maps.iter().map(|b| b.json.clone()).collect::<Vec<_>>(),
        );
    Ok(Encounters { json, maps })
}
