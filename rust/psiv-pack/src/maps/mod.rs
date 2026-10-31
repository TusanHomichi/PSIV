//! Field maps: the pointer table, the record grammar and encounter binding,
//! the port of `psiv_tools/maps/` (#216, slice 3). [`extract_maps`] walks all
//! 417 pointer-table entries. [`layouts`] decodes a record's layout section,
//! [`overworld`] the two world maps' paged planes, and [`draw`] renders
//! either. No claimed section writes these yet, so the gate
//! holds them to Python as the `maps` and `layouts` probes
//! (`examples/probe.rs`).

pub mod draw;
pub mod encounters;
pub mod layouts;
pub mod overworld;
pub mod records;

use std::collections::BTreeSet;

use crate::PackError;
use crate::decode::Decodes;
use crate::json::Json;
use crate::map_symbols::MAP_SYMBOLS;
use crate::text::dialogue_tree_specs;
use encounters::{extract_encounter_binding, formation_group_count};
use records::{
    ERROR_TRAP, FIELD_MAP_PTRS, MAP_COUNT, MapRecord, POINTER_SIZE, be32, check_pointer_table,
    hex6, map_reference, walk_map,
};

/// A `FieldMapPtrs` entry: a real map or a `PtrMap_Null*` placeholder.
#[derive(Clone, Debug, PartialEq)]
pub enum MapEntry {
    Null { id: usize, pointer: usize },
    Real(Box<MapRecord>),
}

/// What [`extract_maps`] returns.
pub struct ExtractedMaps {
    pub entries: Vec<MapEntry>,
    pub pointer_table: Json,
    pub encounters: Json,
    pub encounter_anomalies: Vec<Json>,
}

impl ExtractedMaps {
    pub fn real(&self) -> impl Iterator<Item = &MapRecord> {
        self.entries.iter().filter_map(|entry| match entry {
            MapEntry::Real(record) => Some(record.as_ref()),
            MapEntry::Null { .. } => None,
        })
    }

    /// The document `extract_maps` returns.
    pub fn to_json(&self, data: &[u8]) -> Json {
        let real: Vec<&MapRecord> = self.real().collect();
        let sum =
            |value: &dyn Fn(&MapRecord) -> usize| -> usize { real.iter().map(|m| value(m)).sum() };
        let trees: BTreeSet<usize> = real.iter().map(|m| m.dialogue.tree).collect();
        let maps: Vec<Json> = self
            .entries
            .iter()
            .map(|entry| match entry {
                MapEntry::Real(record) => record.to_json(data),
                MapEntry::Null { id, pointer } => {
                    let at = FIELD_MAP_PTRS + id * POINTER_SIZE;
                    map_reference(*id)
                        .with("is_null", true)
                        .with("pointer_offset", hex6(at))
                        .with("pointer", hex6(*pointer))
                        .with("raw_hex", crate::png::hex(&data[at..at + POINTER_SIZE]))
                }
            })
            .collect();
        Json::object()
            .with("pointer_table", self.pointer_table.clone())
            .with("map_count", MAP_COUNT)
            .with("real_map_count", real.len())
            .with("null_map_count", MAP_COUNT - real.len())
            .with(
                "totals",
                Json::object()
                    .with("tilesets", sum(&|m| m.tilesets.entries.len()))
                    .with("sprite_art", sum(&|m| m.sprites.entries.len()))
                    .with("chunk_pointers", sum(&|m| m.chunks.pointers.len()))
                    .with(
                        "transitions",
                        sum(&|m| m.transitions.entries.len() + m.transitions_2.entries.len()),
                    )
                    .with("objects", sum(&|m| m.objects.entries.len()))
                    .with("treasure_chests", sum(&|m| m.treasure_chests.entries.len()))
                    .with("tile_animations", sum(&|m| m.tile_animations.entries.len()))
                    .with(
                        "interaction_areas",
                        sum(&|m| m.interaction_areas.entries.len()),
                    )
                    .with("record_bytes", sum(&|m| m.end - m.pointer)),
            )
            .with(
                "dialogue_trees_bound",
                trees.into_iter().collect::<Vec<_>>(),
            )
            .with("encounter_anomalies", self.encounter_anomalies.clone())
            .with("encounters", self.encounters.clone())
            .with("maps", maps)
    }
}

/// Walk every entry of `FieldMapPtrs`, decoding the full record grammar.
pub fn extract_maps(data: &[u8], decodes: &Decodes) -> Result<ExtractedMaps, PackError> {
    let pointer_table = check_pointer_table(data)?;
    let trees: Vec<(usize, usize)> = dialogue_tree_specs(data, decodes)?
        .iter()
        .map(|spec| (spec.start, spec.tree))
        .collect();

    let mut entries = Vec::with_capacity(MAP_COUNT);
    for (map_id, symbol) in MAP_SYMBOLS.iter().enumerate() {
        let pointer = be32(data, FIELD_MAP_PTRS + map_id * POINTER_SIZE) as usize;
        let is_null = symbol.starts_with("Null");
        if pointer == ERROR_TRAP {
            if !is_null {
                return Err(PackError(format!(
                    "map 0x{map_id:03X} ({symbol}) points at ErrorTrap but its MapID_* symbol \
                     is not a PtrMap_Null* placeholder"
                )));
            }
            entries.push(MapEntry::Null {
                id: map_id,
                pointer,
            });
            continue;
        }
        if is_null {
            return Err(PackError(format!(
                "map 0x{map_id:03X} ({symbol}) is a PtrMap_Null* placeholder but points at {} \
                 instead of ErrorTrap",
                hex6(pointer)
            )));
        }
        entries.push(MapEntry::Real(Box::new(walk_map(
            data, map_id, pointer, &trees,
        )?)));
    }

    for entry in &entries {
        let MapEntry::Real(record) = entry else {
            continue;
        };
        for (section, list) in [
            ("transitions", &record.transitions),
            ("transitions_2", &record.transitions_2),
        ] {
            for transition in &list.entries {
                let target = usize::from(transition.record.target);
                let real = MAP_SYMBOLS
                    .get(target)
                    .is_some_and(|symbol| !symbol.starts_with("Null"));
                if !real {
                    return Err(PackError(format!(
                        "map 0x{:03X} ({}) has a {section} record at {} targeting map \
                         0x{target:03X}, which is not a real map",
                        record.id,
                        record.symbol,
                        hex6(transition.rom_offset)
                    )));
                }
            }
        }
    }

    let group_count = formation_group_count(decodes)?;
    let encounters = extract_encounter_binding(data, decodes, group_count)?;
    let mut encounter_anomalies = Vec::new();
    for entry in &mut entries {
        let MapEntry::Real(record) = entry else {
            continue;
        };
        let binding = &encounters.maps[record.id];
        let flag = record.flags.random_battles;
        let rolls = flag != 0;
        // The two halves of the encounter system live apart and nothing in
        // the game reconciles them.
        let consistent = if binding.mode == "outside_table" {
            !rolls
        } else {
            rolls == (binding.mode != "none")
        };
        if !consistent {
            encounter_anomalies.push(
                map_reference(record.id)
                    .with("mode", binding.mode)
                    .with("value", binding.value)
                    .with("random_battles_flag", flag)
                    .with(
                        "effect",
                        if rolls {
                            "random battles are enabled but the map names no encounter group, \
                             so Battle_SetupEnemyData would index past the formation-index table"
                        } else {
                            "an encounter group is assigned but random battles are off"
                        },
                    ),
            );
        }
        record.encounters = binding
            .json
            .clone()
            .with("random_battles_flag", flag)
            .with("consistent", consistent);
    }

    Ok(ExtractedMaps {
        entries,
        pointer_table,
        encounters: encounters.json,
        encounter_anomalies,
    })
}
