//! tests/test_battle_records.py's `TestTheRecords`, the equipment half
//! (#549): `battle/equipment.json` as [`build`](super::build) writes it from
//! the local retail ROM.

use std::collections::BTreeSet;

use psiv_data::EquipmentFile;
use serde_json::{Value, json};

use crate::battle::rules::BONUS_FIELDS;
use crate::battle::tests::retail;
use crate::test_support::text_pin;

/// The one inventory record carrying an equip mask its type can never reach.
const NOTHING_ITEM_ID: u64 = 118;

fn files() -> Option<(&'static Value, EquipmentFile)> {
    let retail = retail()?;
    Some((retail.file("equipment"), retail.typed("equipment")))
}

fn item(raw: &Value, id: u64) -> &Value {
    raw["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("no item {id}"))
}

#[test]
fn a_hundred_and_sixty_items_in_id_order() {
    let Some((_, file)) = files() else { return };
    assert_eq!((file.count, file.items.len()), (160, 160));
    let ids: Vec<u8> = file.items.iter().map(|item| item.id).collect();
    assert_eq!(ids, (1..=160).collect::<Vec<u8>>());
}

#[test]
fn the_type_byte_decides_slot_weapon_and_multi_target() {
    let Some((raw, file)) = files() else { return };
    let knife = file.item(2).unwrap();
    assert_eq!(knife.symbol, "HuntKnife");
    assert_eq!(
        text_pin(knife.display_name.as_deref().unwrap()),
        (
            10,
            "51b3fb3e17fee52478207939eecf1dba0d16dd95b231a779f97fa4eea3bc5b09".into()
        )
    );
    assert_eq!(
        (knife.kind.id, knife.kind.name.as_str()),
        (1, "one_handed_single_target_weapon")
    );
    assert_eq!(item(raw, 2)["slot"], "right_hand");
    assert!(knife.is_weapon && !knife.multi_target);
    assert_eq!(knife.bonuses.attack, 5);
    assert_eq!(
        item(raw, 2)["element"],
        json!({"id": 1, "name": "physical", "role": "attack_element"})
    );

    // The boomerang is the one-handed multi-target case.
    let boomerang = file.item(3).unwrap();
    assert_eq!(boomerang.symbol, "Boomerang");
    assert_eq!(
        text_pin(boomerang.display_name.as_deref().unwrap()),
        (
            9,
            "144e675907eee0fba99e52d3f73ae5e2cdf589baff395a473d5f33cd6410c9bb".into()
        )
    );
    assert_eq!(boomerang.kind.id, 2);
    assert!(boomerang.multi_target && !boomerang.two_handed);
}

#[test]
fn every_item_resolves_to_its_type_rule() {
    let Some((raw, file)) = files() else { return };
    for (typed, json) in file.items.iter().zip(raw["items"].as_array().unwrap()) {
        let id = typed.id;
        let rule = file
            .kind(typed.kind.id)
            .unwrap_or_else(|| panic!("item {id}: type {} has no rule", typed.kind.id));
        assert_eq!(json["equippable"], rule.equippable, "{id}");
        assert_eq!(rule.equippable, !json["slot"].is_null(), "{id}");
        assert_eq!(json["slot"], json!(rule.slot), "{id}");
        assert_eq!(
            (typed.is_weapon, typed.multi_target, typed.two_handed),
            (rule.is_weapon, rule.multi_target, rule.two_handed),
            "{id}"
        );
        assert!(!typed.multi_target || typed.is_weapon, "{id}");
        assert_eq!(typed.element.role, rule.element_role, "{id}");
    }
}

#[test]
fn the_equip_mask_only_names_real_characters() {
    let Some((raw, _)) = files() else { return };
    for item in raw["items"].as_array().unwrap() {
        let id = &item["id"];
        let mask = u16::from_str_radix(
            item["equippable_by_mask"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();
        assert_eq!(mask >> 11, 0, "{id}");
        let owners: Vec<u64> = item["equippable_by"]
            .as_array()
            .unwrap()
            .iter()
            .map(|who| who["character_id"].as_u64().unwrap())
            .collect();
        let bits: Vec<u64> = (0..11).filter(|bit| mask & (1 << bit) != 0).collect();
        assert_eq!(owners, bits, "{id}");
    }
}

#[test]
fn the_census_names_the_unreachable_mask_and_the_unused_type() {
    let Some((raw, file)) = files() else { return };
    let census = &raw["census"];
    let unreachable: Vec<&Value> = census["equip_mask_on_unequippable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| &entry["id"])
        .collect();
    assert_eq!(unreachable, [&json!(NOTHING_ITEM_ID)]);
    let nothing = item(raw, NOTHING_ITEM_ID);
    assert_eq!(nothing["equippable"], false);
    assert!(nothing["type"]["id"].as_u64() > census["max_equippable_type"].as_u64());

    // No item declares the unused type zero.
    assert_eq!(census["types_declared_unused"], json!([0]));
    let present: BTreeSet<u8> = file.items.iter().map(|item| item.kind.id).collect();
    assert!(!present.contains(&0));
    assert!(
        !census["types_present"]
            .as_array()
            .unwrap()
            .contains(&json!(0))
    );
    assert_eq!(census["equippable"], 132);
}

#[test]
fn the_bonus_fields_are_the_ones_the_passes_name() {
    let Some((raw, _)) = files() else { return };
    let named: BTreeSet<&str> = BONUS_FIELDS.iter().map(|(_, name)| *name).collect();
    for item in raw["items"].as_array().unwrap() {
        let fields: BTreeSet<&str> = item["bonuses"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(fields, named, "{}", item["id"]);
    }
}
