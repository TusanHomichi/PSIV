//! the retired Python battle_pack suite's enemy checks (#549): `battle/enemies.json` as
//! [`build`](super::build) writes it from the local retail ROM.

use std::collections::BTreeSet;

use psiv_data::{AbilitiesFile, EnemiesFile};
use serde_json::json;

use crate::battle::tests::retail;
use crate::test_support::text_pin;

#[test]
fn an_enemy_carries_stats_properties_ai_and_rewards() {
    let Some(retail) = retail() else { return };
    let file: EnemiesFile = retail.typed("enemies");
    assert_eq!((file.count, file.enemies.len()), (153, 153));
    let raw = &retail.file("enemies")["enemies"][0];
    let helex = &file.enemies[0];
    assert_eq!((helex.id, helex.symbol.as_str()), (0, "Helex"));
    assert_eq!(
        text_pin(raw["display_name"].as_str().unwrap()),
        (
            5,
            "7070beb79575c23916ae923334011f05da33da5e9d677dfb0f971648afc03af2".into()
        )
    );
    assert_eq!(raw["hp"], 90);
    assert_eq!(raw["stats"]["strength"], 10);
    assert_eq!(raw["stats"]["attack"], 160);
    // Fourteen element properties, every one a value with its meaning.
    assert_eq!(raw["properties"].as_object().unwrap().len(), 14);
    assert_eq!(
        raw["properties"]["water"],
        json!({"value": 4, "meaning": "very_weak"})
    );
    assert_eq!(
        raw["properties"]["fire"],
        json!({"value": 0, "meaning": "immune"})
    );
    // The AI lists, ids only: the abilities themselves are abilities.json's.
    let ai = raw["ai"].as_object().unwrap();
    let lists: Vec<(&str, usize)> = ai
        .iter()
        .map(|(key, ids)| (key.as_str(), ids.as_array().unwrap().len()))
        .collect();
    assert_eq!(
        lists,
        [
            ("condition_ids", 4),
            ("conditional_ability_ids", 4),
            ("regular_ability_ids", 8)
        ]
    );
    let rewards: BTreeSet<&str> = raw["rewards"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(rewards.is_superset(&BTreeSet::from(["experience", "meseta"])));
}

#[test]
fn the_property_scale_is_five_values() {
    let Some(retail) = retail() else { return };
    let census = &retail.file("enemies")["census"];
    let values = census["property_values"].as_object().unwrap();
    let keys: Vec<&str> = values.keys().map(String::as_str).collect();
    assert_eq!(keys, ["0", "1", "2", "3", "4"]);
    let total: u64 = values.values().map(|n| n.as_u64().unwrap()).sum();
    assert_eq!(total, 153 * 14);
    assert_eq!(census["property_slots"], 14);
}

#[test]
fn every_ai_ability_id_resolves_to_an_enemy_skill() {
    let Some(retail) = retail() else { return };
    let abilities: AbilitiesFile = retail.typed("abilities");
    let skills: BTreeSet<u64> = abilities
        .enemy_skills
        .iter()
        .map(|record| u64::from(record.id))
        .collect();
    let used: BTreeSet<u64> = retail.file("enemies")["census"]["ability_ids_used"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_u64().unwrap())
        .collect();
    assert!(!used.is_empty());
    // Zero is "no ability", not a skill id.
    let dangling: Vec<&u64> = used
        .iter()
        .filter(|id| **id != 0 && !skills.contains(id))
        .collect();
    assert!(dangling.is_empty(), "{dangling:?}");
    // And the census is every id the records name.
    let named: BTreeSet<u64> = retail.file("enemies")["enemies"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|enemy| {
            ["conditional_ability_ids", "regular_ability_ids"]
                .into_iter()
                .flat_map(move |list| enemy["ai"][list].as_array().unwrap().iter())
        })
        .map(|id| id.as_u64().unwrap())
        .collect();
    assert_eq!(named, used);
}
