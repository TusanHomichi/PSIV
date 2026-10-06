//! Constructed arithmetic and object-lifetime controls for the A3 chains.
//! No retail stat/ability records are transcribed here.

use super::tests::id;
use super::*;
use crate::battle::{EnemySkill, PartyMember, SliceRolls, fixtures, status};

fn data(enemy: u16, ability: u8, effect: u8) -> BattleData {
    let mut record = fixtures::zoran_bult();
    record.id = enemy;
    record.hp = 500;
    record.mental = 40;
    record.strength = 3;
    record.attack = 301;
    record.regular_abilities = [ability; 8];
    record.condition_ids = [0; 4];
    record.conditional_abilities = [0; 4];
    fixtures::data()
        .with_enemies([record])
        .with_enemy_skills([EnemySkill {
            id: ability,
            name: "constructed".into(),
            effect,
            power_stat: 0x82,
            target: 9,
            power: 10,
            resistance: 7,
            element: 3,
        }])
}

fn roster(data: &BattleData, enemy: u16) -> Roster {
    let mut roster = Roster::new();
    for _ in 0..3 {
        let mut stats = PartyMember::seat(&fixtures::alys(), data).unwrap().stats;
        stats.curr_hp = 999;
        stats.max_hp = 999;
        stats.mental_defence.battle = 7;
        stats.defence.battle = 317;
        stats.element_props = [4; crate::battle::ELEMENT_SLOTS];
        roster.add_party_member(0, "constructed".into(), stats);
    }
    roster.get_mut(id(3)).unwrap().mark_defeated();
    roster.add_enemy(1, data.enemy(enemy).unwrap());
    roster
}

#[test]
fn every_new_carrier_uses_the_cited_request_shape_and_record_arithmetic() {
    // Carrier families are independently enumerated from the cited arms,
    // rather than trusting the registry to enumerate its own coverage.
    let families: &[(&[u16], u8, bool)] = &[
        (&[4, 7, 8, 51, 52], 0x04, false),
        (&[37, 38, 137], 0x13, true),
        (&[40], 0x15, true),
        (&[44, 50], 0x18, false),
        (&[45], 0x19, true),
        (&[47, 48, 49, 130], 0x1C, false),
        (&[52, 53, 145, 146], 0x1F, false),
        (&[53, 130], 0x20, true),
        (&[60, 61, 118], 0x22, false),
        (&[69, 142], 0x23, true),
        (&[68, 69, 146], 0x2B, false),
        (&[73, 74], 0x30, true),
        (&[108, 111, 112, 113, 138, 121], 0x4F, true),
        (&[111, 112, 113, 138, 121, 125], 0x5A, false),
        (&[130], 0x63, true),
        (&[143], 0x6A, false),
    ];
    for &(carriers, ability, all_party) in families {
        for &enemy in carriers {
            let effect = match ability {
                0x15 => 0x20,
                0x18 => 0x23,
                0x19 => 0x24,
                _ => 1,
            };
            let data = data(enemy, ability, effect);
            let mut roster = roster(&data, enemy);
            let mut rolls = SliceRolls::new(&[0]);
            let mut events = Vec::new();
            assert!(
                resolve_damage_skill(
                    &mut roster,
                    id(6),
                    ability,
                    Some(id(2)),
                    &data,
                    &mut rolls,
                    &mut events
                ),
                "{enemy}/{ability:#x}"
            );
            let targets: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    BattleEvent::Resolved { target, damage, .. } => Some((*target, *damage)),
                    _ => None,
                })
                .collect();
            // 16 zero draws: ((8*40)>>6)+40+2*10 = 65; factor 4,
            // mental defense 7 => 58. Attack/strength/physical defense differ.
            let expected = if all_party {
                vec![(id(1), Some(58)), (id(2), Some(58))]
            } else {
                vec![(id(2), Some(58))]
            };
            assert_eq!(targets, expected, "{enemy}/{ability:#x}");
            assert_eq!(rolls.drawn(), if all_party { 32 } else { 16 });
            assert_eq!(roster.get(id(3)).unwrap().stats.curr_hp, 0);
        }
    }
}

#[test]
fn all_party_rolls_stay_in_slot_order_even_when_the_first_target_dies() {
    let data = data(37, 0x13, 1);
    let mut roster = roster(&data, 37);
    roster.get_mut(id(1)).unwrap().stats.curr_hp = 1;
    let numbers: Vec<_> = [vec![0; 16], vec![7; 16]].concat();
    let mut rolls = SliceRolls::new(&numbers);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut roster,
        id(6),
        0x13,
        Some(id(9)),
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(rolls.drawn(), 32);
    assert!(roster.get(id(1)).unwrap().stats.is_out());
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_hp, 999 - 128);
}

#[test]
fn object_removal_retains_cached_hp_and_does_not_create_reward_deaths() {
    for enemy in [44, 50] {
        let data = data(enemy, 0x18, 0x23);
        let mut roster = roster(&data, enemy);
        roster.get_mut(id(6)).unwrap().reaction_flags = 0xFF;
        let mut events = Vec::new();
        assert!(resolve_damage_skill(
            &mut roster,
            id(6),
            0x18,
            Some(id(2)),
            &data,
            &mut SliceRolls::new(&[0]),
            &mut events
        ));
        let actor = roster.get(id(6)).unwrap();
        assert!(!actor.active);
        assert_eq!(actor.stats.curr_hp, 500);
        assert_eq!(actor.stats.status & status::DEAD, 0);
        assert_eq!(actor.reaction_flags, 0);
    }
    let data = data(45, 0x19, 1);
    let mut roster = roster(&data, 45);
    let actor = roster.get(id(6)).unwrap().clone();
    roster.add_enemy(2, data.enemy(45).unwrap());
    roster.add_enemy(3, data.enemy(45).unwrap());
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut roster,
        id(7),
        0x19,
        None,
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events
    ));
    assert!(roster.get(id(7)).unwrap().is_alive());
    for removed in [6, 8] {
        assert!(!roster.get(id(removed)).unwrap().active);
        assert_eq!(roster.get(id(removed)).unwrap().stats, actor.stats);
    }
    let removed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Died { fighter } if fighter.side() == Side::Enemy => Some(*fighter),
            _ => None,
        })
        .collect();
    assert_eq!(
        removed,
        [id(8), id(6)],
        "retail next-neighbor then previous"
    );
}

#[test]
fn unproven_pair_and_real_effect_handler_are_negative_controls() {
    for (enemy, effect) in [(101, 1), (138, 7)] {
        let data = data(enemy, 0x5A, effect);
        let mut roster = roster(&data, enemy);
        let before = roster.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(!resolve_damage_skill(
            &mut roster,
            id(6),
            0x5A,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(roster, before);
        assert_eq!(rolls.drawn(), 0);
        assert!(events.is_empty());
    }
}

#[test]
fn distortion_is_not_enabled_for_an_untraced_carrier_or_real_status_effect() {
    for (enemy, effect) in [(72, 1), (73, 7)] {
        let data = data(enemy, 0x30, effect);
        let mut roster = roster(&data, enemy);
        let before = roster.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(!resolve_damage_skill(
            &mut roster,
            id(6),
            0x30,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(roster, before);
        assert_eq!(rolls.drawn(), 0);
        assert!(events.is_empty());
    }
}
