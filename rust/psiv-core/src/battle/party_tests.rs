//! Conformance: seating each of the eleven starting characters must reproduce
//! the pack's `initialized` vector exactly.
//!
//! This is the end-to-end check on [`Stats::update_mod_stats`]
//! (`UpdateCharModStats`, `$0005F754`) and [`Stats::update_char_elems`]
//! (`UpdateCharElems`, `$0005FD2A`) together. The vectors come from the
//! cartridge by way of `psiv_tools.battle_records`, so agreement here is
//! agreement with the ROM, not with my own arithmetic.

use super::party_fixtures::{self, Initialized};
use super::records::{BattleData, EquipSlot, ItemKind, ItemRecord};
use super::stats::{GRANTED_RESISTANCE, Stats};
use crate::battle::engine::PartyMember;

fn data() -> BattleData {
    BattleData::new().with_items(party_fixtures::equipment())
}

fn seated() -> Vec<(Stats, Initialized)> {
    let data = data();
    party_fixtures::records()
        .iter()
        .zip(party_fixtures::expected())
        .map(|(record, expected)| {
            let member = PartyMember::seat(record, &data).expect("every item resolves");
            (member.stats, expected)
        })
        .collect()
}

#[test]
fn all_eleven_characters_derive_their_conformance_vector() {
    for (stats, want) in seated() {
        let who = want.character;
        assert_eq!(
            [
                stats.strength.base,
                stats.mental.base,
                stats.agility.base,
                stats.dexterity.base
            ],
            want.base,
            "character {who} base stats"
        );
        assert_eq!(
            [
                stats.strength.modified,
                stats.mental.modified,
                stats.agility.modified,
                stats.dexterity.modified
            ],
            want.modified,
            "character {who} modified stats"
        );
        assert_eq!(
            [
                stats.attack.derived,
                stats.defence.derived,
                stats.mental_defence.derived
            ],
            want.derived,
            "character {who} atk_pow / dfs_pow / magic_dfs"
        );
        assert_eq!(
            stats.element_props, want.element_props,
            "character {who} finished element properties"
        );
        assert_eq!(
            stats.weapon_elements, want.weapon_elements,
            "character {who} weapon elements"
        );
    }
}

#[test]
fn the_two_hand_worked_numbers_are_among_them() {
    // Scout §12 derived Chaz 18/10 and Alys 13/18 by hand from the record and
    // the item bonuses. The pack derives them from the cartridge. They agree.
    let by_id: Vec<(u8, u16, u16)> = seated()
        .iter()
        .map(|(stats, want)| (want.character, stats.attack.derived, stats.defence.derived))
        .collect();
    assert!(by_id.contains(&(0, 18, 10)), "Chaz");
    assert!(by_id.contains(&(1, 13, 18)), "Alys");
    assert!(by_id.contains(&(2, 8, 9)), "Hahn");
}

#[test]
fn a_seated_character_starts_with_battle_stats_matching_their_derived_ones() {
    // `FillBattleStats` copies mod into battle on entering a battle, and a
    // freshly seated character is already in that state.
    for (stats, want) in seated() {
        assert_eq!(stats.attack.battle, want.derived[0]);
        assert_eq!(stats.defence.battle, want.derived[1]);
        assert_eq!(stats.mental_defence.battle, want.derived[2]);
        assert_eq!(stats.strength.battle, want.modified[0]);
        assert_eq!(stats.dexterity.battle, want.modified[3]);
    }
}

#[test]
fn three_of_the_eleven_swing_with_something_other_than_physical() {
    // Wren, Kyra and Seth start with energy weapons, so the weapon-element
    // cache is not uniformly 1 and a bug that hardcoded physical would show.
    let energy: Vec<u8> = seated()
        .iter()
        .filter(|(stats, _)| stats.weapon_elements[0] == 2)
        .map(|(_, want)| want.character)
        .collect();
    assert_eq!(energy, vec![7, 9, 10], "Wren, Kyra, Seth");
}

#[test]
fn an_empty_hand_caches_no_weapon_element() {
    // Nine of the eleven have an empty left hand; the byte must stay zero
    // rather than inherit the right hand's.
    let (stats, _) = &seated()[1]; // Alys: Boomerang right, nothing left
    assert_eq!(stats.equipment[1], 0);
    assert_eq!(stats.weapon_elements, [1, 0]);
}

#[test]
fn no_starting_loadout_is_weakened_by_its_own_armour() {
    // The pack's census says `element_props_weakened_by_equipment` is empty.
    // Deriving the same conclusion independently is what makes it a check
    // rather than a restatement.
    for (stats, want) in seated() {
        for (index, (finished, innate)) in stats
            .element_props
            .iter()
            .zip(stats.element_shadow)
            .enumerate()
        {
            assert!(
                *finished >= innate,
                "character {} element {index}: {innate} became {finished}",
                want.character
            );
        }
    }
}

// ---------------------------------------------------------------------------
// UpdateCharElems, on loadouts the eleven do not happen to exercise
// ---------------------------------------------------------------------------

fn item(id: u8, kind: ItemKind, element: u8) -> ItemRecord {
    ItemRecord {
        id,
        name: format!("test{id}"),
        kind,
        bonuses: super::records::Bonuses::default(),
        element,
    }
}

/// Chaz's record with an arbitrary loadout, and the items to resolve it.
fn chaz_wearing(equipment: [u8; 4], extra: Vec<ItemRecord>) -> Stats {
    let mut record = party_fixtures::records()[0].clone();
    record.equipment = equipment;
    let data = data().with_items(extra);
    PartyMember::seat(&record, &data).expect("resolves").stats
}

#[test]
fn armour_grants_resistance_one_to_the_element_it_names() {
    // Chaz is innately normal (2) to fire. Ceramic armour naming fire makes
    // him resistant (1).
    let stats = chaz_wearing([2, 0, 0, 60], vec![item(60, ItemKind::Body, 3)]);
    assert_eq!(stats.element_props[2], GRANTED_RESISTANCE, "fire");
    assert_eq!(stats.element_props[0], 2, "physical is untouched");
    assert_eq!(stats.element_props[7], 0, "and holyword keeps its immunity");
}

#[test]
fn a_shield_grants_a_resistance_instead_of_an_attack_element() {
    // The same element byte, read the other way round because the type is 5.
    let shield = item(61, ItemKind::Shield, 3);
    let stats = chaz_wearing([2, 61, 0, 0], vec![shield]);
    assert_eq!(
        stats.element_props[2], GRANTED_RESISTANCE,
        "fire resistance"
    );
    assert_eq!(
        stats.weapon_elements,
        [1, 0],
        "the shield hand caches no attack element"
    );
}

#[test]
fn armour_downgrades_an_innate_immunity_and_that_is_deliberate() {
    // `move.b #1` is unconditional. Chaz is immune (0) to holyword; armour
    // naming holyword makes him merely resistant (1) — strictly worse. The
    // cartridge does this and so does the port.
    let stats = chaz_wearing([2, 0, 0, 62], vec![item(62, ItemKind::Body, 8)]);
    assert_eq!(stats.element_shadow[7], 0, "innately immune to holyword");
    assert_eq!(
        stats.element_props[7], GRANTED_RESISTANCE,
        "and the armour made it worse"
    );
}

#[test]
fn an_element_of_zero_grants_nothing() {
    // `beq.s` skips the write. Chaz's own Leather Cloth is element 0, which is
    // why his finished properties are his record's.
    let stats = chaz_wearing([2, 2, 5, 4], Vec::new());
    assert_eq!(stats.element_props, stats.element_shadow);
}

#[test]
fn every_untouched_slot_falls_back_to_the_record() {
    let stats = chaz_wearing([0, 0, 0, 0], Vec::new());
    assert_eq!(stats.element_props, stats.element_shadow);
    assert_eq!(stats.weapon_elements, [0, 0], "and no hand cached anything");
}

#[test]
fn the_pass_rebuilds_rather_than_accumulating() {
    // Running it twice must not compound, and re-running after a change must
    // drop the old grant — it clears all fourteen before it starts.
    let data = data().with_items(vec![item(63, ItemKind::Body, 3)]);
    let mut record = party_fixtures::records()[0].clone();
    record.equipment = [2, 0, 0, 63];
    let mut stats = PartyMember::seat(&record, &data).expect("resolves").stats;
    let once = stats.element_props;
    let lookup = |id: u8| data.item(id).ok().cloned();
    stats.update_char_elems(&lookup);
    assert_eq!(stats.element_props, once, "idempotent");

    stats.equipment[3] = 0;
    stats.update_char_elems(&lookup);
    assert_eq!(stats.element_props[2], 2, "the fire grant is gone");
}

// ---------------------------------------------------------------------------
// Defend against armour: the ratified fix
// ---------------------------------------------------------------------------

#[test]
fn defending_no_longer_discards_what_armour_granted() {
    // Elast armour grants physical resistance. On the cartridge, defending
    // once and then reaching `Battle_RestoreStatsAtTurnEnd` restores $30 from
    // $31 — the record's innate 2 — and the armour's 1 is gone for the rest of
    // the battle. `docs/RUNTIME_DESIGN.md` "Battle bug policy" fixes it.
    let data = data().with_items(vec![item(96, ItemKind::Body, 1)]);
    let mut record = party_fixtures::records()[0].clone();
    record.equipment = [2, 0, 0, 96];
    let mut stats = PartyMember::seat(&record, &data).expect("resolves").stats;

    assert_eq!(
        stats.element_props[0], GRANTED_RESISTANCE,
        "armour granted 1"
    );
    assert_eq!(stats.element_shadow[0], 2, "but the record says 2");

    stats.begin_defending();
    assert_eq!(stats.element_props[0], GRANTED_RESISTANCE);
    stats.restore_physical_prop();

    // The deliberate deviation, stated as a delta from retail.
    assert_eq!(
        stats.element_props[0], GRANTED_RESISTANCE,
        "FIXED: the armour's resistance survives the round"
    );
    assert_eq!(
        stats.element_shadow[0], 2,
        "retail would have restored this instead, silently costing the armour"
    );
}

#[test]
fn defending_still_works_for_a_character_with_no_elemental_armour() {
    // The observable behaviour the oracle measured is unchanged: physical
    // property 2 becomes 1 for the round, then goes back to 2.
    let mut stats = chaz_wearing([2, 2, 5, 4], Vec::new());
    assert_eq!(stats.element_props[0], 2);
    stats.begin_defending();
    assert_eq!(stats.element_props[0], 1, "a damage-class change");
    assert_eq!(stats.defence.battle, 10, "dfs_pow never moves");
    stats.restore_physical_prop();
    assert_eq!(stats.element_props[0], 2);
    assert_eq!(stats.defence.battle, 10);
}

// ---------------------------------------------------------------------------
// The seating path itself
// ---------------------------------------------------------------------------

#[test]
fn seating_refuses_a_loadout_it_cannot_resolve() {
    let mut record = party_fixtures::records()[0].clone();
    record.equipment = [200, 0, 0, 0];
    assert_eq!(
        PartyMember::seat(&record, &data()),
        Err(super::records::BattleDataError::UnknownItem(200))
    );
}

#[test]
fn seating_carries_the_records_identity_across() {
    let record = &party_fixtures::records()[1];
    let member = PartyMember::seat(record, &data()).expect("resolves");
    assert_eq!(member.character, record.id);
    assert_eq!(member.name, record.name);
    assert_eq!(member.stats.level, record.level);
    assert_eq!(member.stats.curr_hp, record.hp);
    assert_eq!(member.stats.max_hp, record.max_hp);
    assert_eq!(member.stats.curr_tp, record.tp);
    assert_eq!(member.stats.max_tp, record.max_tp);
    assert_eq!(member.stats.profession, record.profession);
}

#[test]
fn the_type_table_matches_the_packs_decoded_one() {
    // `battle/equipment.json`'s `types` array is `Equip_Item`'s jump table read
    // out of the ROM. These are the same facts, so they must agree.
    let table: [(u8, Option<EquipSlot>, bool, bool, bool); 10] = [
        (1, Some(EquipSlot::RightHand), false, true, false),
        (2, Some(EquipSlot::RightHand), false, true, true),
        (3, Some(EquipSlot::RightHand), true, true, false),
        (4, Some(EquipSlot::RightHand), true, true, true),
        (5, Some(EquipSlot::LeftHand), false, false, false),
        (6, Some(EquipSlot::Head), false, false, false),
        (7, Some(EquipSlot::Body), false, false, false),
        (8, None, false, false, false),
        (9, None, false, false, false),
        (10, None, false, false, false),
    ];
    for (byte, slot, two_handed, is_weapon, multi) in table {
        let kind = ItemKind::from_byte(byte).expect("a known type");
        assert_eq!(kind.slot(), slot, "type {byte} slot");
        assert_eq!(kind.is_two_handed(), two_handed, "type {byte} two-handed");
        assert_eq!(kind.is_weapon(), is_weapon, "type {byte} weapon");
        assert_eq!(kind.is_multi_target(), multi, "type {byte} multi-target");
        assert_eq!(kind.is_equippable(), slot.is_some(), "type {byte}");
    }
    assert_eq!(EquipSlot::RightHand.index(), 0);
    assert_eq!(EquipSlot::LeftHand.index(), 1);
    assert_eq!(EquipSlot::Head.index(), 2);
    assert_eq!(EquipSlot::Body.index(), 3);
}
