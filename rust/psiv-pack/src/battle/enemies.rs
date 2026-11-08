//! `battle/enemies.json`: the port of `extract_enemies` in
//! `psiv_tools/core.py` and `build_enemies` in `psiv_tools/battle_pack.py`.
//! `Battle_EnemyData` holds 153 48-byte records: hit points, the basic
//! attack's two bytes, stats, the fourteen property slots, the AI lists and the
//! rewards.
//!
//! The file is `psiv_data::EnemiesFile`. Each record's offset, the property
//! meanings, the source and the census have no typed home yet and are laid
//! over it.

use std::collections::{BTreeMap, BTreeSet};

use psiv_data::{EnemiesFile, Enemy, EnemyAi, EnemyAttack, EnemyStats, Property, Rewards};

use super::names::{
    Display, PROPERTIES, basic_attack_status, display, element, enemy_symbols, named_byte,
    property_level,
};
use super::{FORMAT_VERSION, range, serialize};
use crate::PackError;
use crate::json::{Json, overlay};

/// `TABLES["enemies"]`.
const ENEMIES: usize = 0x2816BC;
const RECORD_BYTES: usize = 48;

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// One 48-byte record.
fn enemy(id: u16, symbol: &str, record: &[u8]) -> Enemy {
    let be16 = |at: usize| u16::from_be_bytes([record[at], record[at + 1]]);
    let properties = PROPERTIES
        .iter()
        .zip(&record[12..26])
        .map(|(&name, &value)| (name.to_owned(), Property { value }))
        .collect();
    Enemy {
        id,
        symbol: symbol.to_owned(),
        display_name: None,
        hp: be16(0),
        stats: EnemyStats {
            strength: record[4],
            mental: record[5],
            agility: record[6],
            dexterity: record[7],
            attack: be16(8),
            defense: u16::from(record[10]),
            magic_defense: u16::from(record[11]),
        },
        attack: EnemyAttack {
            element: named_byte(record[2], element),
            status_effect: named_byte(record[3], basic_attack_status),
        },
        properties,
        ai: EnemyAi {
            condition_ids: record[28..32].to_vec(),
            conditional_ability_ids: record[32..36].to_vec(),
            regular_ability_ids: record[36..44].to_vec(),
        },
        rewards: Rewards {
            experience: be16(44),
            meseta: be16(46),
        },
    }
}

/// What the records carry beside the schema: the offset and each property's
/// meaning.
fn research(offset: usize, enemy: &Enemy) -> Json {
    let mut properties = Json::object();
    for (name, property) in &enemy.properties {
        properties.insert(
            name.as_str(),
            Json::object().with("meaning", property_level(property.value)),
        );
    }
    Json::object()
        .with("rom_offset", hex6(offset))
        .with("properties", properties)
}

fn census(enemies: &[Enemy]) -> Json {
    let mut values: BTreeMap<u8, usize> = BTreeMap::new();
    for enemy in enemies {
        for property in enemy.properties.values() {
            *values.entry(property.value).or_default() += 1;
        }
    }
    let mut property_values = Json::object();
    for (value, count) in values {
        property_values.insert(value.to_string(), count);
    }
    let slots: BTreeSet<&String> = enemies.iter().flat_map(|e| e.properties.keys()).collect();
    let used: BTreeSet<u8> = enemies
        .iter()
        .flat_map(|e| {
            e.ai.conditional_ability_ids
                .iter()
                .chain(&e.ai.regular_ability_ids)
                .copied()
        })
        .collect();
    Json::object()
        .with("property_values", property_values)
        .with("property_slots", slots.len())
        .with("hp", range(enemies.iter().map(|e| u64::from(e.hp))))
        .with(
            "experience",
            range(enemies.iter().map(|e| u64::from(e.rewards.experience))),
        )
        .with(
            "meseta",
            range(enemies.iter().map(|e| u64::from(e.rewards.meseta))),
        )
        .with(
            "with_conditional_ai",
            enemies
                .iter()
                .filter(|e| e.ai.conditional_ability_ids.iter().any(|&id| id != 0))
                .count(),
        )
        .with("ability_ids_used", used.into_iter().collect::<Vec<_>>())
}

/// The whole file.
pub fn build(rom: &[u8], names: &Display) -> Result<Json, PackError> {
    let mut enemies = Vec::new();
    let mut extra = Vec::new();
    for (id, symbol) in enemy_symbols().into_iter().enumerate() {
        let offset = ENEMIES + id * RECORD_BYTES;
        let record = rom
            .get(offset..offset + RECORD_BYTES)
            .ok_or_else(|| PackError(format!("Enemy record {id} runs past end of ROM")))?;
        let mut record = enemy(id as u16, symbol, record);
        record.display_name = display(names, record.id);
        extra.push(research(offset, &record));
        enemies.push(record);
    }
    let properties: BTreeSet<String> = enemies
        .iter()
        .flat_map(|e| e.properties.keys().cloned())
        .collect();
    let census = census(&enemies);
    let file = EnemiesFile {
        format_version: FORMAT_VERSION,
        count: enemies.len() as u32,
        properties: properties.into_iter().collect(),
        enemies,
    };
    let mut out = serialize(&file, "the enemies")?;
    let over = Json::object()
        .with("kind", "battle_enemies")
        .with(
            "source",
            Json::object()
                .with("label", "Battle_EnemyData")
                .with("rom_offset", hex6(ENEMIES))
                .with("record_bytes", RECORD_BYTES),
        )
        .with("enemies", extra)
        .with("census", census);
    overlay(&mut out, &over);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> [u8; RECORD_BYTES] {
        let mut raw = [0u8; RECORD_BYTES];
        raw[0..2].copy_from_slice(&1234u16.to_be_bytes());
        raw[2] = 3;
        raw[3] = 0x1C;
        raw[4] = 40;
        raw[8..10].copy_from_slice(&0x0102u16.to_be_bytes());
        raw[10] = 200;
        raw[12] = 0;
        raw[25] = 4;
        raw[28] = 9;
        raw[35] = 7;
        raw[43] = 5;
        raw[44..46].copy_from_slice(&500u16.to_be_bytes());
        raw[46..48].copy_from_slice(&65535u16.to_be_bytes());
        raw
    }

    #[test]
    fn a_record_reads_its_words_bytes_and_slots() {
        let enemy = enemy(3, "Seeker", &record());
        assert_eq!(enemy.hp, 1234);
        assert_eq!(enemy.stats.strength, 40);
        assert_eq!(enemy.stats.attack, 0x0102);
        assert_eq!(enemy.stats.defense, 200);
        assert_eq!(enemy.attack.element.name.as_deref(), Some("fire"));
        assert_eq!(enemy.attack.status_effect.name.as_deref(), Some("paralyze"));
        assert_eq!(enemy.properties["physical"].value, 0);
        assert_eq!(enemy.properties["destroy"].value, 4);
        assert_eq!(enemy.properties.len(), 14);
        assert_eq!(enemy.ai.condition_ids, [9, 0, 0, 0]);
        assert_eq!(enemy.ai.conditional_ability_ids, [0, 0, 0, 7]);
        assert_eq!(enemy.ai.regular_ability_ids[7], 5);
        assert_eq!(
            (enemy.rewards.experience, enemy.rewards.meseta),
            (500, 65535)
        );
    }

    #[test]
    fn an_unknown_status_byte_has_no_name() {
        let mut raw = record();
        raw[3] = 0x1D;
        let enemy = enemy(0, "Helex", &raw);
        assert_eq!(enemy.attack.status_effect.id, 0x1D);
        assert!(enemy.attack.status_effect.name.is_none());
    }

    #[test]
    fn the_census_counts_values_and_conditional_ai() {
        let a = enemy(0, "A", &record());
        let mut raw = record();
        raw[32..36].fill(0);
        raw[36..44].fill(0);
        let b = enemy(1, "B", &raw);
        let census = serde_json::Value::from(&census(&[a, b]));
        assert_eq!(census["with_conditional_ai"], 1);
        assert_eq!(census["ability_ids_used"], serde_json::json!([0, 5, 7]));
        assert_eq!(census["property_values"]["0"], 26);
        assert_eq!(census["property_values"]["4"], 2);
        assert_eq!(census["hp"]["min"], 1234);
    }
}
