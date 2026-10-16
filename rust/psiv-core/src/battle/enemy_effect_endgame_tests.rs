//! Lane A6's status and stat arms: SHIFT and SANER behind their guards, the
//! new handlers (`$09`, `$0C`, `$21`, `$27`), Radhin's random enemy pick,
//! SPARK's android pick and CYANICBOMB's caster removal.
//!
//! Constructed records only: the effect id and range nibble are the route's
//! gate, every other byte is chosen to make one outcome certain.

use super::*;
use crate::battle::{EnemyRecord, PartyMember, SliceRolls, fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

/// A record with no power stat, no resistance (no roll) unless asked for.
fn skill(ability: u8, effect: u8, range: u8, resistance: u8) -> EnemySkill {
    EnemySkill {
        id: ability,
        name: format!("constructed {ability:#04X}"),
        effect,
        power_stat: 0x82,
        target: range,
        power: 64,
        resistance,
        element: 1,
    }
}

fn setup(enemy: u16, skill: EnemySkill, enemies: u8) -> (BattleData, Roster) {
    let record = EnemyRecord {
        id: enemy,
        mental: 30,
        ..fixtures::zoran_bult()
    };
    let data = fixtures::data()
        .with_enemies([record])
        .with_enemy_skills([skill]);
    let mut roster = Roster::new();
    for _ in 0..3 {
        let mut stats = PartyMember::seat(&fixtures::alys(), &data).unwrap().stats;
        stats.element_props = [2; crate::battle::ELEMENT_SLOTS];
        roster.add_party_member(0, "constructed".into(), stats);
    }
    for slot in 1..=enemies {
        roster.add_enemy(slot, data.enemy(enemy).unwrap()).unwrap();
    }
    (data, roster)
}

fn run(
    roster: &mut Roster,
    data: &BattleData,
    ability: u8,
    intended: Option<FighterId>,
    draws: &[u16],
) -> (EffectTurn, Vec<BattleEvent>, usize) {
    let mut rolls = SliceRolls::new(draws);
    let mut events = Vec::new();
    let turn = resolve_effect_skill(
        roster,
        id(6),
        ability,
        intended,
        data,
        &mut rolls,
        &mut events,
    );
    (turn, events, rolls.drawn())
}

#[test]
fn shift_raises_the_casters_attack_once_and_then_swings() {
    for enemy in [64, 65, 66, 72] {
        let (data, mut roster) = setup(enemy, skill(0x26, 0x09, 3, 0), 1);
        let derived = roster.get(id(6)).unwrap().stats.attack.derived;
        let (turn, _, drawn) = run(&mut roster, &data, 0x26, Some(id(1)), &[]);
        assert_eq!((turn, drawn), (EffectTurn::Resolved, 0), "{enemy}");
        // `atk_pow_battle = atk_pow + MEN` (30), on the caster itself.
        assert_eq!(roster.get(id(6)).unwrap().stats.attack.battle, derived + 30);
        // Raised now: the arm's guard sends the next roll to the swing.
        let (turn, events, _) = run(&mut roster, &data, 0x26, Some(id(1)), &[]);
        assert_eq!(turn, EffectTurn::Swing, "{enemy}");
        assert!(events.is_empty());
        assert_eq!(roster.get(id(6)).unwrap().ability, 0);
    }
}

#[test]
fn deathbearrs_saner_compares_words_and_always_swings_while_agility_stands() {
    let (data, mut roster) = setup(65, skill(0x27, 0x0C, 2, 0), 2);
    let (turn, _, _) = run(&mut roster, &data, 0x27, None, &[]);
    assert_eq!(turn, EffectTurn::Swing);
    // Only a battle agility of zero lets the word compare pass.
    roster.get_mut(id(6)).unwrap().stats.agility.battle = 0;
    let (turn, _, _) = run(&mut roster, &data, 0x27, None, &[]);
    assert_eq!(turn, EffectTurn::Resolved);
}

#[test]
fn radhins_saner_raises_every_enemys_agility_with_a_byte_wrap() {
    let (data, mut roster) = setup(116, skill(0x27, 0x0C, 2, 0), 2);
    for slot in [6, 7] {
        roster.get_mut(id(slot)).unwrap().stats.agility.modified = 240;
    }
    let (turn, _, drawn) = run(&mut roster, &data, 0x27, None, &[]);
    assert_eq!((turn, drawn), (EffectTurn::Resolved, 0));
    for slot in [6, 7] {
        // 240 + 30 wraps to 14: `add.b`, no clamp.
        assert_eq!(roster.get(id(slot)).unwrap().stats.agility.battle, 14);
    }
}

#[test]
fn radhins_shift_redraws_its_animation_pick_until_an_enemy_stands_there() {
    let (data, mut roster) = setup(116, skill(0x26, 0x09, 3, 0), 1);
    // Slots 7-9 are empty: `rand & 3` of 1, 2 and 3 re-draw, 0 stops.
    let (turn, _, drawn) = run(&mut roster, &data, 0x26, None, &[1, 2, 3, 4]);
    assert_eq!((turn, drawn), (EffectTurn::Resolved, 4));
    // Sealed, the prelude ends the turn before the pick.
    let (data, mut roster) = setup(116, skill(0x26, 0x09, 3, 0), 1);
    roster.get_mut(id(6)).unwrap().stats.status |= status::TECH_SEALED;
    let (_, _, drawn) = run(&mut roster, &data, 0x26, None, &[1, 2, 3, 4]);
    assert_eq!(drawn, 0);
}

#[test]
fn flash_lowers_dexterity_to_one_at_least() {
    let (data, mut roster) = setup(43, skill(0x16, 0x21, 9, 2), 1);
    for slot in 1..=3 {
        roster.get_mut(id(slot)).unwrap().stats.dexterity.modified = 20;
    }
    // Power is the caster's strength selector here: masked `$82` = mental 30,
    // so 20 - 30 borrows and lands on 1. One roll per living member (63
    // lands at threshold 64 with factor 2).
    let (turn, _, drawn) = run(&mut roster, &data, 0x16, None, &[63, 63, 63]);
    assert_eq!((turn, drawn), (EffectTurn::Resolved, 3));
    for slot in 1..=3 {
        assert_eq!(roster.get(id(slot)).unwrap().stats.dexterity.battle, 1);
    }
}

#[test]
fn canceling_restores_every_living_members_stats_without_a_roll() {
    let (data, mut roster) = setup(135, skill(0x69, 0x27, 9, 0), 1);
    {
        let stats = &mut roster.get_mut(id(2)).unwrap().stats;
        stats.attack.battle = 1;
        stats.agility.battle = 200;
        stats.element_props[0] = 9;
    }
    let before = roster.get(id(2)).unwrap().stats.clone();
    let (turn, _, drawn) = run(&mut roster, &data, 0x69, None, &[]);
    assert_eq!((turn, drawn), (EffectTurn::Resolved, 0));
    let after = &roster.get(id(2)).unwrap().stats;
    assert_eq!(after.attack.battle, before.attack.derived);
    assert_eq!(after.agility.battle, before.agility.modified);
    assert_eq!(after.element_props, before.element_shadow);
}

#[test]
fn spark_hunts_androids_and_without_one_the_turn_is_a_swing() {
    let (data, mut roster) = setup(49, skill(0x1E, 0x02, 8, 0), 1);
    let (turn, _, drawn) = run(&mut roster, &data, 0x1E, Some(id(1)), &[]);
    assert_eq!((turn, drawn), (EffectTurn::Swing, 0));
    // One android: no draw, and it is the target whatever was drawn.
    roster.get_mut(id(3)).unwrap().stats.profession = crate::battle::PROFESSION_ANDROID;
    let (turn, _, drawn) = run(&mut roster, &data, 0x1E, Some(id(1)), &[]);
    assert_eq!((turn, drawn), (EffectTurn::Resolved, 0));
    assert!(roster.get(id(3)).unwrap().stats.is_out());
    assert!(!roster.get(id(1)).unwrap().stats.is_out());
}

#[test]
fn two_androids_take_one_draw_whose_low_bit_picks_the_last() {
    for (roll, victim) in [(0u16, 1u8), (1, 2)] {
        let (data, mut roster) = setup(49, skill(0x1E, 0x02, 8, 0), 1);
        for slot in [1, 2] {
            roster.get_mut(id(slot)).unwrap().stats.profession = crate::battle::PROFESSION_ANDROID;
        }
        let (turn, _, drawn) = run(&mut roster, &data, 0x1E, Some(id(3)), &[roll]);
        assert_eq!((turn, drawn), (EffectTurn::Resolved, 1));
        assert!(
            roster.get(id(victim)).unwrap().stats.is_out(),
            "roll {roll}"
        );
        assert!(!roster.get(id(3)).unwrap().stats.is_out());
    }
}

#[test]
fn cyanicbomb_kills_its_target_and_removes_its_caster() {
    let (data, mut roster) = setup(46, skill(0x1A, 0x02, 8, 0), 2);
    let (turn, events, _) = run(&mut roster, &data, 0x1A, Some(id(2)), &[]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert!(roster.get(id(2)).unwrap().stats.is_out());
    let caster = roster.get(id(6)).unwrap();
    assert!(!caster.active);
    // Removed, not killed: no death bit, so no reward.
    assert_eq!(caster.stats.status & status::DEAD, 0);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Died { fighter } if *fighter == id(6)))
    );
    assert!(roster.get(id(7)).unwrap().active);
}

#[test]
fn an_unlisted_carrier_of_the_same_records_is_not_routed() {
    // The handlers are gated on the pair: 63 GerotLux has none of these arms.
    for (ability, effect, range) in [
        (0x26, 0x09, 3),
        (0x27, 0x0C, 2),
        (0x16, 0x21, 9),
        (0x69, 0x27, 9),
    ] {
        let (data, mut roster) = setup(63, skill(ability, effect, range, 0), 1);
        let (turn, events, drawn) = run(&mut roster, &data, ability, Some(id(1)), &[]);
        assert_eq!((turn, drawn), (EffectTurn::NotMine, 0));
        assert!(events.is_empty());
    }
}
