//! tests/test_battle_pack.py's ability checks (#549): the effect table read
//! from the local retail ROM and `battle/abilities.json` as
//! [`build`](super::super::build) writes it.

use std::collections::BTreeSet;

use psiv_data::AbilitiesFile;
use serde_json::Value;

use super::*;
use crate::battle::tests::retail;
use crate::test_support::text_pin;

/// `docs/battle/BATTLE_RULES.md` section 11 finding 2.
const BLACK_WAVE_ID: u16 = 112;
const BLACK_WAVE_EFFECT: u8 = 0x2C;
const BLACK_WAVE_TARGET: &str = "0x00B033";
const KINDS: [&str; 4] = ["techniques", "skills", "enemy_skills", "item_effects"];

#[test]
fn the_effect_table_bounds_itself() {
    let Some(rom) = crate::test_support::retail_rom("ability effect table") else {
        return;
    };
    // Each entry is an offset from the table base, so the table ends where
    // its lowest target begins: AbilityEffect_None's `rts`. effect_count
    // also refuses any entry that lands back inside the table.
    let rom = rom.bytes();
    assert_eq!(effect_count(rom).unwrap(), 44);
    assert_eq!(EFFECT_NONE - EFFECTS, 44 * 2);
    let mut broken = rom.to_vec();
    broken[EFFECT_NONE..EFFECT_NONE + 2].fill(0);
    assert!(effect_count(&broken).is_err());
}

fn kind<'a>(raw: &'a Value, kind: &str) -> &'a [Value] {
    raw[kind].as_array().unwrap()
}

#[test]
fn black_wave_is_flagged_rather_than_dispatched() {
    let Some(retail) = retail() else { return };
    let raw = retail.file("abilities");
    let black_wave = kind(raw, "enemy_skills")
        .iter()
        .find(|record| record["id"] == BLACK_WAVE_ID)
        .unwrap();
    // Effect $2C is one past the 44-entry table. The dispatcher has no
    // bound, so the word where entry 44 would be, added to the base, is
    // where the cartridge would jump: an odd address, so an address error.
    assert_eq!(black_wave["effect_id"], BLACK_WAVE_EFFECT);
    assert_eq!(black_wave["effect_out_of_range"], true);
    assert_eq!(black_wave["effect_dispatch_target"], BLACK_WAVE_TARGET);
    assert_eq!(
        u32::from_str_radix(&BLACK_WAVE_TARGET[2..], 16).unwrap() % 2,
        1
    );
    assert_eq!(
        text_pin(black_wave["display_name"].as_str().unwrap()),
        (
            10,
            "a9b565e963ab74d043c4511d6c49f4b68cbc352817755c838761a14b23446300".into()
        )
    );
    // And it is the only one in the whole set.
    let out_of_range = raw["census"]["effect_out_of_range"].as_array().unwrap();
    assert_eq!(out_of_range.len(), 1);
    assert_eq!(out_of_range[0]["id"], BLACK_WAVE_ID);
    let flagged: Vec<&Value> = KINDS
        .iter()
        .flat_map(|k| kind(raw, k))
        .filter(|record| record["effect_out_of_range"] == true)
        .map(|record| &record["id"])
        .collect();
    assert_eq!(flagged, [&Value::from(BLACK_WAVE_ID)]);
    assert_eq!(raw["effects"]["bounds_checked"], false);
}

#[test]
fn the_four_kinds_share_one_record_shape_and_add_their_own_fields() {
    let Some(retail) = retail() else { return };
    let raw = retail.file("abilities");
    assert_eq!(raw["record_bytes"], 8);
    let file: AbilitiesFile = retail.typed("abilities");
    for kind_name in KINDS {
        for record in kind(raw, kind_name) {
            assert_eq!(record["kind"], kind_name);
            for field in [
                "effect_id",
                "power_or_hit_chance",
                "resistance_stat",
                "element",
                "effect_out_of_range",
            ] {
                assert!(record.get(field).is_some(), "{kind_name} {field}");
            }
        }
    }
    // Only techniques cost TP; only skills declare a weapon requirement.
    assert!(file.techniques.iter().all(|r| r.tp_cost.is_some()));
    assert!(file.skills.iter().all(|r| r.requires_weapon.is_some()));
    assert!(
        kind(raw, "skills")
            .iter()
            .all(|r| r.get("relevant_stat").is_some())
    );
    assert!(
        kind(raw, "enemy_skills")
            .iter()
            .all(|r| r.get("relevant_stat").is_some())
    );
    let foi = &file.techniques[0];
    assert_eq!((foi.name.as_deref(), foi.tp_cost), (Some("Foi"), Some(3)));
    assert_eq!(
        text_pin(foi.display_name.as_deref().unwrap()),
        (
            3,
            "9cebb192fc6d6a41b0086f9b98e06cdab8b48d3fd3271ed03b36599c96ef477a".into()
        )
    );
    let element = foi.element.as_ref().unwrap();
    assert_eq!((element.id, element.name.as_deref()), (3, Some("fire")));
}

#[test]
fn item_battle_records_are_the_same_eight_bytes() {
    let Some(retail) = retail() else { return };
    let file: AbilitiesFile = retail.typed("abilities");
    // An item's battle behaviour is the effect block embedded in its 22-byte
    // inventory record, read as the damage pipeline reads an ability.
    assert_eq!(file.item_effects.len(), 160);
    let dagger = &file.item_effects[0];
    assert_eq!((dagger.id, dagger.symbol.as_deref()), (1, Some("Dagger")));
    assert_eq!(
        text_pin(dagger.display_name.as_deref().unwrap()),
        (
            6,
            "b9b5b5d0625c22ec01eca1882b14d55be64e5896bdb6b986e4ddf778cc63fdc8".into()
        )
    );
    assert!(dagger.battle_object_or_graphic_id.is_some());
    assert!(!dagger.effect_out_of_range);
}

#[test]
fn the_effect_census_separates_used_from_unused() {
    let Some(retail) = retail() else { return };
    let raw = retail.file("abilities");
    let census = &raw["census"];
    let ids = |key: &str| -> BTreeSet<u64> {
        census[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap())
            .collect()
    };
    let (used, unused) = (ids("effect_ids_used"), ids("effect_ids_in_table_unused"));
    // The out-of-range id is used, because it is; the unused list is bounded
    // by the table, so the two deliberately do not add up.
    assert!(used.contains(&u64::from(BLACK_WAVE_EFFECT)));
    assert!(!unused.contains(&u64::from(BLACK_WAVE_EFFECT)));
    assert!(used.is_disjoint(&unused));
    let count = raw["effects"]["count"].as_u64().unwrap();
    assert!(unused.iter().all(|&id| id < count));
    let every: BTreeSet<u64> = KINDS
        .iter()
        .flat_map(|k| kind(raw, k))
        .map(|record| record["effect_id"].as_u64().unwrap())
        .collect();
    assert_eq!(used, every);
}
