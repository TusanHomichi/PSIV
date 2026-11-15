//! tests/test_battle_records.py's `TestTheRecords`, the character half
//! (#549): `battle/characters.json` as [`build`](super::build) writes it from
//! the local retail ROM, read through `psiv_data::CharactersFile` and, for
//! what the schema doesn't carry, the raw JSON.

use std::collections::BTreeSet;

use psiv_data::{CharactersFile, EquipmentFile};
use serde_json::{Value, json};

use crate::battle::tests::retail;
use crate::test_support::text_pin;

/// `docs/battle/BATTLE_RULES.md` section 12 works the opening party's
/// derived stats by hand: (character, atk_pow, dfs_pow).
const OPENING_DERIVED: [(usize, u16, u16); 2] = [(0, 18, 10), (1, 13, 18)];
/// Chaz and Rika, the two characters who start with a weapon in the left
/// hand.
const DUAL_WIELDERS: [u8; 2] = [0, 5];

fn files() -> Option<(&'static Value, CharactersFile, EquipmentFile)> {
    let retail = retail()?;
    Some((
        retail.file("characters"),
        retail.typed("characters"),
        retail.typed("equipment"),
    ))
}

#[test]
fn eleven_characters_with_hp_tp_and_uses_at_their_maxima() {
    let Some((_, file, _)) = files() else { return };
    assert_eq!((file.count, file.characters.len()), (11, 11));
    for (id, character) in file.characters.iter().enumerate() {
        assert_eq!(usize::from(character.character_id), id);
        assert_eq!(character.hp, character.max_hp, "{}", character.symbol);
        assert_eq!(character.tp, character.max_tp, "{}", character.symbol);
    }
}

#[test]
fn the_ability_arrays_stay_positional() {
    let Some((raw, file, _)) = files() else {
        return;
    };
    for (character, json) in file
        .characters
        .iter()
        .zip(raw["characters"].as_array().unwrap())
    {
        let who = &character.symbol;
        assert_eq!(character.techniques.slots.len(), 16, "{who}");
        assert_eq!(character.skills.slots.len(), 8, "{who}");
        assert_eq!(character.skills.uses.len(), 8, "{who}");
        for (kind, slots) in [
            ("techniques", &character.techniques.slots),
            ("skills", &character.skills.slots),
        ] {
            for known in json[kind]["known"].as_array().unwrap() {
                let slot = known["slot"].as_u64().unwrap() as usize;
                assert_eq!(known["id"], slots[slot], "{who} {kind} slot {slot}");
            }
        }
        let skills = json["skills"]["known"].as_array().unwrap();
        for skill in skills {
            assert_eq!(skill["uses"], skill["max_uses"], "{who}");
        }
        // Learned abilities go in the first empty slot, so the known ones
        // are a prefix with no holes.
        assert!(
            character.skills.slots[skills.len()..]
                .iter()
                .all(|&s| s == 0),
            "{who}"
        );
    }
}

#[test]
fn chaz_is_the_record_the_disassembly_documents() {
    let Some((raw, file, _)) = files() else {
        return;
    };
    let chaz = &file.characters[0];
    assert_eq!(chaz.symbol, "Chaz");
    assert_eq!(chaz.display_name.as_deref(), Some("Chaz"));
    assert_eq!(raw["characters"][0]["rom_offset"], "0x2A8ACA");
    assert_eq!(raw["characters"][0]["profession"]["symbol"], "Hunter");
    let profession = chaz.profession.name.as_deref().unwrap();
    assert_eq!(
        text_pin(profession),
        (
            6,
            "06659319dce5b93c72dc9fd5a6e52ca0c9aa14d30602a967c7afdaf81272ccc7".into()
        )
    );
    assert_eq!((chaz.level, chaz.hp), (1, 25));
    let s = &chaz.stats;
    assert_eq!((s.strength, s.mental, s.agility, s.dexterity), (8, 6, 7, 5));
    assert_eq!(chaz.equipment.item_ids(), [2, 2, 5, 4]);
}

#[test]
fn the_initialized_block_derives_what_the_scout_worked_by_hand() {
    let Some((_, file, equipment)) = files() else {
        return;
    };
    for (id, attack, defence) in OPENING_DERIVED {
        let initialized = &file.characters[id].initialized;
        assert_eq!(
            (initialized.atk_pow, initialized.dfs_pow),
            (attack, defence)
        );
    }
    for character in &file.characters {
        let who = &character.symbol;
        let initialized = &character.initialized;
        let stats: BTreeSet<&str> = initialized.stats.keys().map(String::as_str).collect();
        assert_eq!(
            stats,
            BTreeSet::from(["agility", "dexterity", "mental", "strength"]),
            "{who}"
        );
        assert_eq!(initialized.element_props.len(), 14, "{who}");
        let s = &character.stats;
        for (name, base) in [
            ("strength", s.strength),
            ("mental", s.mental),
            ("agility", s.agility),
            ("dexterity", s.dexterity),
        ] {
            assert_eq!(initialized.stats[name].base, base, "{who} {name}");
        }
        // A weapon hand carries its item's element; a shield or an empty
        // hand carries none.
        for (slot, observed) in [
            ("right_hand", &initialized.weapon_elements.right_hand),
            ("left_hand", &initialized.weapon_elements.left_hand),
        ] {
            let entry = character
                .equipment
                .slots()
                .into_iter()
                .find(|(name, _)| *name == slot)
                .unwrap()
                .1;
            let expected = match entry {
                Some(item) if item.kind != 5 => {
                    u16::from(equipment.item(item.item_id).unwrap().element.id)
                }
                _ => 0,
            };
            assert_eq!(observed.id, expected, "{who} {slot}");
        }
    }
}

#[test]
fn every_initial_item_is_one_its_owner_could_re_equip() {
    let Some((raw, file, _)) = files() else {
        return;
    };
    let census = &raw["census"];
    assert_eq!(census["equipment_the_owner_cannot_equip"], json!([]));
    let retail = retail().unwrap();
    let items: std::collections::BTreeMap<u64, &Value> = retail.file("equipment")["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| (item["id"].as_u64().unwrap(), item))
        .collect();
    for character in &file.characters {
        for (slot, equipped) in character.equipment.slots() {
            let Some(equipped) = equipped else { continue };
            let item = items[&u64::from(equipped.item_id)];
            assert_eq!(item["equippable"], true, "{} {slot}", character.symbol);
            let owners: Vec<&Value> = item["equippable_by"]
                .as_array()
                .unwrap()
                .iter()
                .map(|who| &who["character_id"])
                .collect();
            assert!(
                owners.contains(&&json!(character.character_id)),
                "{} {slot}",
                character.symbol
            );
        }
    }
}

#[test]
fn the_left_hand_weapons_the_equip_menu_could_never_replace() {
    let Some((raw, file, equipment)) = files() else {
        return;
    };
    assert_eq!(raw["census"]["weapon_in_left_hand"], json!(DUAL_WIELDERS));
    let retail = retail().unwrap();
    for id in DUAL_WIELDERS {
        let held = file.characters[usize::from(id)]
            .equipment
            .left_hand
            .as_ref()
            .unwrap();
        assert!(equipment.item(held.item_id).unwrap().is_weapon, "{id}");
        // Nothing but a shield is ever placed there by the menu.
        let item = retail.file("equipment")["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == held.item_id)
            .unwrap();
        assert_eq!(item["slot"], "right_hand", "{id}");
    }
}
