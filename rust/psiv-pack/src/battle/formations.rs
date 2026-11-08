//! `battle/formations.json`: the port of `psiv_tools/formations.py` and
//! `build_formations` in `psiv_tools/battle_pack.py`, which own the retail
//! citations. Formation records are `$FF`-terminated runs in four Kosinski
//! blocks (ids `$80` apart) and a boss block; two more kinds sit uncompressed
//! in battle-object code. The encounter groups, map bindings and overworld
//! grids come from [`crate::maps::encounters`].
//!
//! The file is `psiv_data::FormationsFile`. What the game doesn't read (each
//! record's block and group masks, an enemy's groups, the sources, the
//! encounter table, the vehicle groups and the census) has no typed home yet
//! and is laid over it.

use std::collections::BTreeSet;

use psiv_data::{
    EncounterGroup, EncounterGroups, Formation, FormationEnemy, FormationsFile, InlineFormation,
    MapBinding, PositionGrid,
};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use super::{FORMAT_VERSION, range, serialize};
use crate::PackError;
use crate::decode::{Call, Decodes};
use crate::json::{Json, overlay};
use crate::maps::encounters::extract_encounter_binding;
use crate::png::hex;

const HEADER_SIZE: usize = 7;
const TERMINATOR: u8 = 0xFF;
const CANNOT_RUN_THRESHOLD: u8 = 0xF0;
const INDEX_GROUP_SIZE: usize = 0x40;
const INDEX_ENTRIES_PER_GROUP: usize = 32;
const BLOCK_RECORDS_MAX: usize = 0x80;

struct Block {
    name: &'static str,
    label: &'static str,
    start: usize,
    end: usize,
    id_base: Option<u16>,
}

/// The four normal blocks, end-exclusive; the decompressor must consume each
/// range exactly.
const BLOCKS: [Block; 4] = [
    Block {
        name: "block_1",
        label: "Battle_FormationData1",
        start: 0x283E6C,
        end: 0x2842AE,
        id_base: Some(0x000),
    },
    Block {
        name: "block_2",
        label: "Battle_FormationData2",
        start: 0x2842BC,
        end: 0x284713,
        id_base: Some(0x080),
    },
    Block {
        name: "block_3",
        label: "Battle_FormationData3",
        start: 0x28471C,
        end: 0x284B7E,
        id_base: Some(0x100),
    },
    Block {
        name: "block_4",
        label: "Battle_FormationData4",
        start: 0x284B8C,
        end: 0x284F7C,
        id_base: Some(0x180),
    },
];
const BOSS_BLOCK: Block = Block {
    name: "boss",
    label: "Battle_BossFormationData",
    start: 0x284F7C,
    end: 0x285012,
    id_base: None,
};
const INDEX_BLOCK: Block = Block {
    name: "formation_indexes",
    label: "Battle_FormationIndexes",
    start: 0x2836EC,
    end: 0x283E67,
    id_base: None,
};

/// Records an object copies over `Enemy_Formation_Data` mid-battle, each found
/// by the one `lea (<offset>).l, a0` that loads it: (label, offset, loader).
const INLINE: [(&str, usize, &str); 5] = [
    ("loc_1A2F4", 0x1A2F4, "BattleObj_Fusion"),
    ("loc_23D00", 0x23D00, "loc_23C84"),
    ("loc_1308C", 0x1308C, "BattleObj_TarantellaCombine"),
    ("loc_1987E", 0x1987E, "BattleObj_SlugFission"),
    ("loc_224E8", 0x224E8, "loc_223C0"),
];
/// The longest inline record: the header and four enemies.
const INLINE_MAX: usize = HEADER_SIZE + 4 * 2 + 1;

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn typed<T: DeserializeOwned>(value: &Json, what: &str) -> Result<T, PackError> {
    serde_json::from_value(serde_json::Value::from(value))
        .map_err(|error| PackError(format!("the reader's type refuses {what}: {error}")))
}

/// One decoded record: the typed formation and what the game doesn't read.
struct Record {
    formation: Formation,
    research: Json,
}

fn mask_slots(mask: u8) -> Vec<u8> {
    (0..4u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| bit + 1)
        .collect()
}

/// `parse_formation_record`.
fn parse_record(record: &[u8]) -> Result<Record, PackError> {
    if record.len() < HEADER_SIZE + 3 || record.last() != Some(&TERMINATOR) {
        return Err(PackError(format!(
            "Malformed formation record: {}",
            hex(record)
        )));
    }
    let body = &record[HEADER_SIZE..record.len() - 1];
    if !body.len().is_multiple_of(2) {
        return Err(PackError(format!(
            "Formation record has a dangling enemy byte: {}",
            hex(record)
        )));
    }
    let (group_1, group_2) = (record[5], record[6]);
    let mut enemies = Vec::new();
    let mut groups_json = Vec::new();
    for (index, pair) in body.as_chunks::<2>().0.iter().enumerate() {
        let slot = index as u8 + 1;
        let bit = 1 << index;
        let groups: Vec<u8> = [(1u8, group_1), (2, group_2)]
            .into_iter()
            .filter(|(_, mask)| mask & bit != 0)
            .map(|(n, _)| n)
            .collect();
        enemies.push(FormationEnemy {
            slot,
            enemy_id: u16::from(pair[0]),
            position: pair[1],
        });
        groups_json.push(Json::object().with("groups", groups));
    }
    let declared = record[4];
    let formation = Formation {
        id: None,
        event_battle_index: None,
        ambush_chance: record[0],
        run_chance: record[1],
        can_run: record[1] < CANNOT_RUN_THRESHOLD,
        drop_rate: record[2],
        drop_item: (record[3] != 0).then_some(record[3]),
        count: declared,
        count_matches_entries: usize::from(declared) == enemies.len(),
        enemies,
    };
    let research = Json::object()
        .with("group_1_mask", format!("0x{group_1:02X}"))
        .with("group_2_mask", format!("0x{group_2:02X}"))
        .with("group_1_slots", mask_slots(group_1))
        .with("group_2_slots", mask_slots(group_2))
        .with("enemies", groups_json);
    Ok(Record {
        formation,
        research,
    })
}

/// `split_formation_records`: past each header, whole pairs to the `$FF`, so
/// a `$FF` position byte is never taken for a terminator.
fn split(block: &[u8]) -> Result<Vec<&[u8]>, PackError> {
    let mut records = Vec::new();
    let mut start = 0;
    while start < block.len() {
        let mut cursor = start + HEADER_SIZE;
        while cursor < block.len() && block[cursor] != TERMINATOR {
            cursor += 2;
        }
        if cursor >= block.len() {
            return Err(PackError(format!(
                "Unterminated formation record at offset 0x{start:X} of a {}-byte block",
                block.len()
            )));
        }
        records.push(&block[start..=cursor]);
        start = cursor + 1;
    }
    Ok(records)
}

/// Decompress a block, holding it to its documented range; its source record.
fn decompress(rom: &[u8], decodes: &Decodes, block: &Block) -> Result<(Vec<u8>, Json), PackError> {
    let decoded = decodes.get(Call::Kosinski {
        offset: block.start,
    })?;
    let expected = block.end - block.start;
    if decoded.consumed() != expected {
        return Err(PackError(format!(
            "{} at {}: decompressor consumed {} bytes, but the documented range is {expected} \
             bytes",
            block.label,
            hex6(block.start),
            decoded.consumed()
        )));
    }
    let bytes = decoded.bytes().into_owned();
    let compressed = &rom[block.start..block.end];
    let source = Json::object()
        .with("label", block.label)
        .with("rom_offset", hex6(block.start))
        .with("rom_end_exclusive", hex6(block.end))
        .with("compression", "kosinski")
        .with("compressed_size", compressed.len())
        .with("compressed_sha256", hex(&Sha256::digest(compressed)))
        .with("decompressed_size", bytes.len());
    Ok((bytes, source))
}

/// `parse_formation_block`, held to the game's own byte-by-byte `$FF` count.
fn parse_block(rom: &[u8], decodes: &Decodes, block: &Block) -> Result<Vec<Record>, PackError> {
    let (bytes, _) = decompress(rom, decodes, block)?;
    let records = split(&bytes)?;
    let terminators = bytes.iter().filter(|&&b| b == TERMINATOR).count();
    if terminators != records.len() {
        return Err(PackError(format!(
            "{}: structural split found {} records but the block contains {terminators} $FF \
             bytes; the game's terminator scan would not agree with this parse",
            block.label,
            records.len()
        )));
    }
    records
        .into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let mut record = parse_record(raw)?;
            match block.id_base {
                Some(base) => record.formation.id = Some(base + index as u16),
                None => record.formation.event_battle_index = Some(index as u16),
            }
            record.research.insert("block", block.name);
            record.research.insert("index_in_block", index);
            Ok(record)
        })
        .collect()
}

/// `extract_inline_formations`: each accepted only when one `lea` loads it.
fn inline(rom: &[u8]) -> Result<Vec<(InlineFormation, Json)>, PackError> {
    INLINE
        .iter()
        .map(|&(label, offset, loaded_by)| {
            let loads = crate::m68k::find_all(rom, &format!("41f9{offset:08x}")).len();
            if loads != 1 {
                return Err(PackError(format!(
                    "{label} at {}: expected one `lea` that loads it, found {loads}",
                    hex6(offset)
                )));
            }
            let window = rom
                .get(offset..offset + INLINE_MAX)
                .ok_or_else(|| PackError(format!("{label} runs past the end of the ROM")))?;
            let mut cursor = HEADER_SIZE;
            while cursor < window.len() && window[cursor] != TERMINATOR {
                cursor += 2;
            }
            if cursor >= window.len() {
                return Err(PackError(format!(
                    "{label}: no terminator within {INLINE_MAX} bytes"
                )));
            }
            let record = parse_record(&window[..=cursor])?;
            Ok((
                InlineFormation {
                    label: label.to_owned(),
                    rom_offset: hex6(offset),
                    loaded_by: loaded_by.to_owned(),
                    formation: record.formation,
                },
                record.research,
            ))
        })
        .collect()
}

/// `extract_formation_indexes`: 32 big-endian formation ids per 64-byte group.
fn groups(
    rom: &[u8],
    decodes: &Decodes,
    known: &BTreeSet<u16>,
) -> Result<(Vec<EncounterGroup>, Json), PackError> {
    let (bytes, source) = decompress(rom, decodes, &INDEX_BLOCK)?;
    if !bytes.len().is_multiple_of(INDEX_GROUP_SIZE) {
        return Err(PackError(format!(
            "Battle_FormationIndexes decompressed to {} bytes, which is not a multiple of the \
             {INDEX_GROUP_SIZE}-byte group size",
            bytes.len()
        )));
    }
    let groups: Vec<EncounterGroup> = bytes
        .as_chunks::<INDEX_GROUP_SIZE>()
        .0
        .iter()
        .enumerate()
        .map(|(group, chunk)| EncounterGroup {
            group: group as u32,
            formation_ids: chunk
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect(),
        })
        .collect();
    let unresolved: BTreeSet<u16> = groups
        .iter()
        .flat_map(|g| g.formation_ids.iter().copied())
        .filter(|id| !known.contains(id))
        .collect();
    if !unresolved.is_empty() {
        let listed: Vec<String> = unresolved.iter().map(|id| format!("0x{id:03X}")).collect();
        return Err(PackError(format!(
            "Battle_FormationIndexes references formation ids that do not exist: {}",
            listed.join(", ")
        )));
    }
    Ok((groups, source))
}

/// Pick `keys` from an object, in that order.
fn project(value: &Json, keys: &[&str]) -> Json {
    keys.iter().fold(Json::object(), |out, key| {
        out.with(*key, value.get(key).cloned().unwrap_or(Json::Null))
    })
}

/// The whole file.
pub fn build(rom: &[u8], decodes: &Decodes) -> Result<Json, PackError> {
    let mut normal = Vec::new();
    for block in &BLOCKS {
        let records = parse_block(rom, decodes, block)?;
        if records.len() > BLOCK_RECORDS_MAX {
            return Err(PackError(format!(
                "{} holds {} records; the game's id dispatch allows at most 0x80 per block",
                block.label,
                records.len()
            )));
        }
        normal.extend(records);
    }
    let bosses = parse_block(rom, decodes, &BOSS_BLOCK)?;
    let inline = inline(rom)?;
    let known: BTreeSet<u16> = normal.iter().filter_map(|r| r.formation.id).collect();
    let (groups, index_source) = groups(rom, decodes, &known)?;
    let encounters = extract_encounter_binding(rom, decodes, groups.len())?;

    let bindings: Vec<MapBinding> = encounters
        .maps
        .iter()
        .map(|binding| typed(&binding.json, "a map binding"))
        .collect::<Result<_, _>>()?;
    let grid_json = match encounters.json.get("position_grids") {
        Some(Json::Array(grids)) => grids.clone(),
        _ => Vec::new(),
    };
    let grid_keys = [
        "name",
        "label",
        "map",
        "rom_offset",
        "columns",
        "rows",
        "cell_size_pixels",
        "indexing",
        "groups_used",
        "fallback_group",
        "fallback_cells",
        "cells",
    ];
    let grids_research: Vec<Json> = grid_json.iter().map(|g| project(g, &grid_keys)).collect();
    let grids: Vec<PositionGrid> = grids_research
        .iter()
        .map(|grid| typed(grid, "a position grid"))
        .collect::<Result<_, _>>()?;

    let both = || normal.iter().chain(&bosses);
    let mut mismatched = Vec::new();
    for record in both().filter(|r| !r.formation.count_matches_entries) {
        let id = record.formation.id.ok_or_else(|| {
            PackError(format!(
                "{} declares a count its entries disagree with",
                record.formation.key()
            ))
        })?;
        mismatched.push(id);
    }
    let referenced: BTreeSet<u16> = groups
        .iter()
        .flat_map(|g| g.formation_ids.iter().copied())
        .collect();
    let table = encounters.json.get("table").cloned().unwrap_or(Json::Null);
    let census = Json::object()
        .with("count_mismatches", mismatched)
        .with(
            "enemies_per_formation",
            range(both().map(|r| r.formation.enemies.len() as u64)),
        )
        .with(
            "formations_with_a_drop",
            both().filter(|r| r.formation.drop_item.is_some()).count(),
        )
        .with(
            "unrunnable_formations",
            both().filter(|r| !r.formation.can_run).count(),
        )
        .with("formation_ids_referenced", referenced.len())
        .with(
            "maps_with_encounters",
            encounters.maps.iter().filter(|b| b.mode != "none").count(),
        )
        .with(
            "table_covers_maps",
            table.get("count").cloned().unwrap_or(Json::Null),
        )
        .with(
            "map_ids",
            table.get("map_count").cloned().unwrap_or(Json::Null),
        )
        .with(
            "maps_outside_table",
            table
                .get("maps_outside_table")
                .cloned()
                .unwrap_or(Json::Null),
        );

    let file = FormationsFile {
        format_version: FORMAT_VERSION,
        formation_count: normal.len() as u32,
        boss_formation_count: bosses.len() as u32,
        formations: normal.iter().map(|r| r.formation.clone()).collect(),
        boss_formations: bosses.iter().map(|r| r.formation.clone()).collect(),
        inline_formations: inline.iter().map(|(f, _)| f.clone()).collect(),
        encounter_groups: Some(EncounterGroups {
            group_count: groups.len() as u32,
            entries_per_group: INDEX_ENTRIES_PER_GROUP as u32,
            groups,
        }),
        map_bindings: bindings,
        position_grids: grids,
    };
    let mut out = serialize(&file, "the formations")?;
    let research = Json::object()
        .with("kind", "battle_formations")
        .with(
            "formations",
            normal
                .iter()
                .map(|r| r.research.clone())
                .collect::<Vec<_>>(),
        )
        .with(
            "boss_formations",
            bosses
                .iter()
                .map(|r| r.research.clone())
                .collect::<Vec<_>>(),
        )
        .with(
            "inline_formations",
            inline.into_iter().map(|(_, j)| j).collect::<Vec<_>>(),
        )
        .with(
            "encounter_groups",
            Json::object()
                .with("source", index_source)
                .with("group_size_bytes", INDEX_GROUP_SIZE),
        )
        .with("position_grids", grids_research)
        .with(
            "vehicle_groups",
            encounters
                .json
                .get("vehicle_groups")
                .cloned()
                .unwrap_or(Json::Null),
        )
        .with("table", table)
        .with("census", census);
    overlay(&mut out, &research);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_reads_its_header_slots_and_groups() {
        let raw = [
            0x10, 0xF0, 8, 0, 3, 0b011, 0b100, 1, 14, 1, 26, 9, 30, TERMINATOR,
        ];
        let record = parse_record(&raw).unwrap();
        let f = &record.formation;
        assert!(!f.can_run, "$F0 refuses a run");
        assert_eq!(f.drop_item, None);
        assert!(f.count_matches_entries);
        assert_eq!(f.enemies[2].slot, 3);
        assert_eq!(f.enemies[2].enemy_id, 9);
        assert_eq!(mask_slots(0b011), [1, 2]);
        let groups = serde_json::Value::from(&record.research);
        assert_eq!(groups["enemies"][2]["groups"], serde_json::json!([2]));
        assert_eq!(groups["enemies"][0]["groups"], serde_json::json!([1]));
    }

    #[test]
    fn a_short_count_or_a_dangling_byte_is_seen() {
        let raw = [0, 0, 0, 5, 4, 1, 0, 1, 2, 3, 4, 5, 6, TERMINATOR];
        let f = parse_record(&raw).unwrap().formation;
        assert!(!f.count_matches_entries, "declares 4, lists 3");
        assert_eq!(f.drop_item, Some(5));
        assert!(parse_record(&[0, 0, 0, 0, 1, 1, 0, 1, 2, 3, TERMINATOR]).is_err());
    }

    #[test]
    fn the_split_steps_over_a_position_byte_of_ff() {
        let block = [
            0, 0, 0, 0, 1, 1, 0, 7, TERMINATOR, TERMINATOR, // record 1, position $FF
            0, 0, 0, 0, 1, 1, 0, 8, 2, TERMINATOR,
        ];
        let records = split(&block).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].len(), 10);
        assert!(split(&block[..block.len() - 1]).is_err(), "unterminated");
    }
}
