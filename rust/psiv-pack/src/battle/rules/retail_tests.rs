//! tests/test_battle_records.py's `TestTheRules` (#549): the five routines,
//! decoded out of the local retail ROM rather than transcribed. Each value
//! asserted is an address, an offset, a count or a type id the Python test
//! already pinned.

use std::collections::{BTreeMap, BTreeSet};

use super::equip::read_equip_rules;
use super::mod_stats::read_mod_stats;
use super::*;

fn retail(what: &str) -> Option<Vec<u8>> {
    crate::test_support::retail_rom(what).map(|rom| rom.bytes().to_vec())
}

#[test]
fn every_signature_occurs_exactly_as_often_as_declared() {
    let Some(rom) = retail("rules signatures") else {
        return;
    };
    for (label, signature, expected) in SIGNATURES {
        assert_eq!(
            crate::m68k::find_all(&rom, signature).len(),
            expected,
            "{label}"
        );
        assert_eq!(sites(&rom, label).unwrap().len(), expected, "{label}");
    }
}

#[test]
fn a_rom_that_lost_or_doubled_a_signature_is_refused() {
    let Some(rom) = retail("rules refusals") else {
        return;
    };
    let site = site(&rom, "UpdateCharModStats").unwrap();
    let mut lost = rom.clone();
    lost[site..site + 2].copy_from_slice(&[0x4E, 0x71]);
    let error = read_mod_stats(&lost).err().unwrap();
    assert!(error.0.contains("UpdateCharModStats"), "{}", error.0);
    assert!(read_rules(&lost).is_err());

    // A second copy anywhere makes the site ambiguous.
    let mut doubled = rom.clone();
    let signature: Vec<u8> = (0..SIGNATURES[1].1.len() / 2)
        .map(|i| u8::from_str_radix(&SIGNATURES[1].1[i * 2..i * 2 + 2], 16).unwrap())
        .collect();
    doubled[0x200..0x200 + signature.len()].copy_from_slice(&signature);
    assert!(read_mod_stats(&doubled).is_err());
    assert!(read_char_init(&doubled).is_err(), "it calls the same site");
}

// InitializeCharStats

#[test]
fn the_record_layout_comes_out_of_the_routine() {
    let Some(rom) = retail("character record layout") else {
        return;
    };
    let init = read_char_init(&rom).unwrap();
    assert_eq!(init.table, CHARACTER_TABLE);
    assert_eq!(init.record_bytes, CHARACTER_BYTES);
    assert_eq!(init.character_stats_ram, 0xFFFF_F500);
    assert_eq!(init.struct_bytes, 0x80);
    // The fields tile the record with no gap and no overlap.
    let mut at = 0;
    for field in &init.fields {
        assert_eq!(field.record_offset, at, "{}", field.field);
        at += field.width * field.count;
    }
    assert_eq!(at, CHARACTER_BYTES);
}

#[test]
fn the_three_mirrors_and_the_three_array_copies() {
    let Some(rom) = retail("character record mirrors") else {
        return;
    };
    let init = read_char_init(&rom).unwrap();
    let mirrors: BTreeMap<&str, &str> = init
        .fields
        .iter()
        .filter_map(|f| Some((f.field.as_str(), f.mirrored_to.as_deref()?)))
        .collect();
    assert_eq!(
        mirrors,
        BTreeMap::from([
            ("curr_hp", "max_hp"),
            ("curr_tp", "max_tp"),
            ("curr_skill_uses", "max_skill_uses"),
        ])
    );
    let arrays: BTreeMap<&str, usize> = init
        .fields
        .iter()
        .filter(|f| f.count > 1)
        .map(|f| (f.field.as_str(), f.count))
        .collect();
    assert_eq!(
        arrays,
        BTreeMap::from([("techniques", 16), ("skills", 8), ("curr_skill_uses", 8)])
    );
}

// UpdateCharModStats

#[test]
fn the_seven_derived_stat_passes() {
    let Some(rom) = retail("derived stat passes") else {
        return;
    };
    let mods = read_mod_stats(&rom).unwrap();
    let passes: Vec<(&str, &str, &[u16])> = mods
        .passes
        .iter()
        .map(|p| (p.stat.as_str(), p.base_stat.as_str(), &p.item_offsets[..]))
        .collect();
    assert_eq!(
        passes,
        [
            ("strength_mod", "strength", &[0x0B][..]),
            ("mental_mod", "mental", &[0x0C]),
            ("agility_mod", "agility", &[0x0D]),
            ("dexterity_mod", "dexterity", &[0x0E]),
            ("atk_pow", "strength", &[0x0B, 0x0F]),
            ("dfs_pow", "agility", &[0x0D, 0x10]),
            ("magic_dfs", "mental", &[0x0C, 0x11]),
        ]
    );
    // The single-stat passes wrap a byte; the derived ones sign-extend.
    for pass in &mods.passes {
        let json = serde_json::Value::from(&pass.to_json());
        if pass.stat.ends_with("_mod") {
            assert_eq!(pass.result_bytes, 1, "{}", pass.stat);
            assert_eq!(json["arithmetic"], "add.b", "{}", pass.stat);
            assert_eq!(json["signed"], false, "{}", pass.stat);
        } else {
            assert_eq!(pass.result_bytes, 2, "{}", pass.stat);
            assert_eq!(json["signed"], true, "{}", pass.stat);
        }
    }
    // Every bonus byte is summed by some pass.
    let used: BTreeSet<u16> = mods
        .passes
        .iter()
        .flat_map(|p| p.item_offsets.iter().copied())
        .collect();
    assert_eq!(
        used,
        BONUS_FIELDS.iter().map(|(offset, _)| *offset).collect()
    );
}

#[test]
fn the_adders_agree_with_the_item_table() {
    let Some(rom) = retail("bonus adders") else {
        return;
    };
    let mods = read_mod_stats(&rom).unwrap();
    assert_eq!(mods.inventory, ITEM_TABLE);
    assert_eq!(usize::from(mods.stride), ITEM_BYTES);
    assert_eq!(mods.slots, SLOTS.len());
    assert_ne!(mods.byte_adder, mods.word_adder);
}

// UpdateEquipment

#[test]
fn the_equip_jump_table_places_each_type() {
    let Some(rom) = retail("equip jump table") else {
        return;
    };
    let rules = read_equip_rules(&rom).unwrap();
    assert_eq!(rules.max_type, 7);
    assert_eq!(rules.type_offset, 0x0A);
    assert_eq!(rules.usable_by, 0x08);
    assert_eq!(rules.filters.len(), 2);
    let placed: Vec<(u16, Vec<&str>)> = rules
        .handlers
        .iter()
        .map(|(t, h)| (*t, h.equips_to.iter().copied().collect()))
        .collect();
    assert_eq!(
        placed,
        [
            (1, vec!["right_hand"]),
            (2, vec!["right_hand"]),
            (3, vec!["right_hand"]),
            (4, vec!["right_hand"]),
            (5, vec!["left_hand"]),
            (6, vec!["head"]),
            (7, vec!["body"]),
        ]
    );
}

#[test]
fn hands_shields_and_two_handed_types() {
    let Some(rom) = retail("item type rules") else {
        return;
    };
    let types = type_rules(&read_rules(&rom).unwrap()).unwrap();
    let left: Vec<u8> = types
        .values()
        .filter(|r| r.slot == Some("left_hand"))
        .map(|r| r.item_type)
        .collect();
    assert_eq!(left, [5], "only a shield ever reaches the left hand");
    let two_handed: Vec<u8> = types
        .values()
        .filter(|r| r.two_handed)
        .map(|r| r.item_type)
        .collect();
    assert_eq!(two_handed, [3, 4]);
    for key in [3, 4] {
        let research = serde_json::Value::from(&types[&key].research);
        assert_eq!(research["clears"], serde_json::json!(["left_hand"]));
    }
    // A shield displaces a two-handed weapon from the other hand.
    let shield = &types[&5];
    let research = serde_json::Value::from(&shield.research);
    assert_eq!(
        research["clears_when_other_hand_holds"],
        serde_json::json!([3, 4])
    );
    assert_eq!(
        research["clears_conditionally"],
        serde_json::json!(["right_hand"])
    );
    assert!(!shield.two_handed);
}

// Battle_AttackCommand and UpdateCharElems

#[test]
fn which_types_can_attack() {
    let Some(rom) = retail("attack rules") else {
        return;
    };
    let rules = read_attack_rules(&rom).unwrap();
    assert_eq!(rules.weapon_types, BTreeSet::from([1, 2, 3, 4]));
    assert_eq!(rules.multi_target_types, BTreeSet::from([2, 4]));
    assert_eq!(rules.get_type, site(&rom, "Battle_GetItemType").unwrap());
}

#[test]
fn the_element_byte_has_two_readings() {
    let Some(rom) = retail("element rules") else {
        return;
    };
    let e = read_element_rules(&rom).unwrap();
    assert_eq!(e.element_offset, 0x12);
    assert_eq!(e.type_offset, 0x0A);
    assert_eq!(e.shield_type, 5);
    assert_eq!(e.elements, 14);
    assert_eq!(e.resistance_value, 1);
    assert_eq!(e.props, 0x30);
    assert_eq!((e.right, e.left), (0x50, 0x51));
    let types = type_rules(&read_rules(&rom).unwrap()).unwrap();
    for rule in types.values() {
        let expected = if rule.is_weapon {
            Some("attack_element")
        } else if rule.equippable {
            Some("resistance_granted")
        } else {
            None
        };
        assert_eq!(rule.element_role, expected, "type {}", rule.item_type);
    }
}
