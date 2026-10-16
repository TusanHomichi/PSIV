//! Lane A5's status and stat arms: DEBAN's other two sabres and its seal test,
//! DTHSPELL, POSESSION, REINFORCE and DarkForce2's two-call EVIL EYE.
//!
//! Constructed records only: the effect id and range nibble are the route's
//! gate, every other byte is chosen to make one outcome certain. No retail
//! stat or ability record is copied here.

use super::*;
use crate::battle::{EnemyRecord, PartyMember, SliceRolls, fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

/// A record with the route's effect and range, no power or resistance stat,
/// and miss threshold 64: `(roll & 63) * 2 <= 64` misses, so a roll of 0
/// misses and 63 lands.
fn skill(ability: u8, effect: u8, range: u8, resistance: u8) -> EnemySkill {
    EnemySkill {
        id: ability,
        name: format!("constructed {ability:#04X}"),
        effect,
        power_stat: 0,
        target: range,
        power: 64,
        resistance,
        element: 1,
    }
}

fn setup(enemy: u16, skill: EnemySkill) -> (BattleData, Roster) {
    let record = EnemyRecord {
        id: enemy,
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
    roster.add_enemy(1, data.enemy(enemy).unwrap()).unwrap();
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
fn every_sabre_runs_the_deban_arm_and_a_sealed_one_fizzles() {
    for enemy in [70, 71, 72] {
        let (data, mut roster) = setup(enemy, skill(0x2D, 0x0A, 2, 0));
        let derived = roster.get(id(6)).unwrap().stats.defence.derived;
        let (turn, _, drawn) = run(&mut roster, &data, 0x2D, None, &[]);
        assert_eq!(turn, EffectTurn::Resolved, "{enemy}");
        assert_eq!(drawn, 0);
        // Power stat 0 reads 0: the raise is exactly the derived value.
        assert_eq!(roster.get(id(6)).unwrap().stats.defence.battle, derived);

        // Sealed: the object's `btst #4, $16(a1)` at frame `$14` ends the turn
        // before `loc_1D8C6`'s effect call (`ps4.asm:40125-40130`).
        let (data, mut roster) = setup(enemy, skill(0x2D, 0x0A, 2, 0));
        let caster = &mut roster.get_mut(id(6)).unwrap().stats;
        caster.status |= status::TECH_SEALED;
        caster.defence.battle = 1;
        caster.defence.derived = 1;
        let (turn, events, drawn) = run(&mut roster, &data, 0x2D, None, &[]);
        assert_eq!(turn, EffectTurn::Resolved);
        assert_eq!(drawn, 0);
        assert!(matches!(
            events.as_slice(),
            [BattleEvent::EnemySkillUsed { skill: 0x2D, .. }]
        ));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::StatChanged { .. }))
        );
    }
}

#[test]
fn an_unsealed_raised_sabre_still_swings_before_the_object_loads() {
    // The arm's own defence test (`loc_F89A`) comes first and needs no seal.
    let (data, mut roster) = setup(71, skill(0x2D, 0x0A, 2, 0));
    let caster = &mut roster.get_mut(id(6)).unwrap().stats;
    caster.defence.battle = caster.defence.derived + 1;
    let (turn, events, _) = run(&mut roster, &data, 0x2D, None, &[]);
    assert_eq!(turn, EffectTurn::Swing);
    assert!(events.is_empty());
}

#[test]
fn dthspell_kills_the_stored_target_on_a_landed_roll() {
    // 112 Illusionst and 113 ImagioMage reach another object, `$72C`, with the
    // same call and kill (lane A6); 106 Haunt has no DTHSPELL arm.
    for enemy in [112, 113] {
        let (data, mut roster) = setup(enemy, skill(0x4E, 0x02, 8, 2));
        let (turn, _, drawn) = run(&mut roster, &data, 0x4E, Some(id(2)), &[63]);
        assert_eq!((turn, drawn), (EffectTurn::Resolved, 1), "{enemy}");
        assert!(!roster.get(id(2)).unwrap().is_alive());
    }
    let (data, mut roster) = setup(106, skill(0x4E, 0x02, 8, 2));
    let (turn, _, drawn) = run(&mut roster, &data, 0x4E, Some(id(2)), &[63]);
    assert_eq!((turn, drawn), (EffectTurn::NotMine, 0));
    let (data, mut roster) = setup(107, skill(0x4E, 0x02, 8, 2));
    let (turn, events, drawn) = run(&mut roster, &data, 0x4E, Some(id(2)), &[63]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert_eq!(drawn, 1);
    assert!(!roster.get(id(2)).unwrap().is_alive());
    assert!(events.contains(&BattleEvent::Died { fighter: id(2) }));
    // A missed roll leaves the target standing.
    let (data, mut roster) = setup(107, skill(0x4E, 0x02, 8, 2));
    run(&mut roster, &data, 0x4E, Some(id(2)), &[0]);
    assert!(roster.get(id(2)).unwrap().is_alive());
}

#[test]
fn posession_puts_the_stored_target_to_sleep() {
    let (data, mut roster) = setup(128, skill(0x60, 0x07, 8, 2));
    let (turn, _, drawn) = run(&mut roster, &data, 0x60, Some(id(1)), &[63]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert_eq!(drawn, 1);
    assert_ne!(roster.get(id(1)).unwrap().stats.status & status::ASLEEP, 0);
    assert_eq!(roster.get(id(2)).unwrap().stats.status & status::ASLEEP, 0);
}

#[test]
fn reinforce_raises_lashiec_by_twenty_without_a_roll_and_raises_the_latch() {
    let (data, mut roster) = setup(128, skill(0x62, 0x2B, 3, 0));
    let stats = &mut roster.get_mut(id(6)).unwrap().stats;
    stats.strength.battle = 250;
    stats.attack.battle = 0xFFF0;
    let before = roster.get(id(6)).unwrap().stats.clone();
    let (turn, events, drawn) = run(&mut roster, &data, 0x62, Some(id(1)), &[]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert_eq!(drawn, 0, "record byte 4 is zero: no chance roll");
    let after = &roster.get(id(6)).unwrap().stats;
    // `add.b` wraps a byte and `add.w` a word (`ps4.asm:9498-9505`).
    assert_eq!(after.strength.battle, 250u8.wrapping_add(20));
    assert_eq!(after.attack.battle, 0xFFF0u16.wrapping_add(20));
    assert_eq!(after.mental.battle, before.mental.battle.wrapping_add(20));
    assert_eq!(after.agility.battle, before.agility.battle.wrapping_add(20));
    assert_eq!(
        after.dexterity.battle,
        before.dexterity.battle.wrapping_add(20)
    );
    assert_eq!(after.defence.battle, before.defence.battle + 20);
    assert_eq!(
        after.mental_defence.battle,
        before.mental_defence.battle + 20
    );
    // Range 3 is the actor alone: the party is untouched.
    assert!(events.iter().all(|e| match e {
        BattleEvent::StatChanged { target, .. } => *target == id(6),
        _ => true,
    }));
    assert!(raises_reinforce_latch(128, 0x62));
    assert!(!raises_reinforce_latch(128, 0x60));
}

#[test]
fn dark_force_2s_evil_eye_takes_two_rolls_and_the_second_decides() {
    let (data, mut roster) = setup(131, skill(0x4C, 0x07, 8, 2));
    let (_, _, drawn) = run(&mut roster, &data, 0x4C, Some(id(1)), &[0, 63]);
    assert_eq!(drawn, 2);
    assert_ne!(roster.get(id(1)).unwrap().stats.status & status::ASLEEP, 0);

    let (data, mut roster) = setup(131, skill(0x4C, 0x07, 8, 2));
    let (_, _, drawn) = run(&mut roster, &data, 0x4C, Some(id(1)), &[63, 0]);
    assert_eq!(drawn, 2);
    assert_eq!(roster.get(id(1)).unwrap().stats.status & status::ASLEEP, 0);

    // A sleeping target leaves the handler before either roll.
    let (data, mut roster) = setup(131, skill(0x4C, 0x07, 8, 2));
    roster.get_mut(id(1)).unwrap().stats.status |= status::ASLEEP;
    let (_, _, drawn) = run(&mut roster, &data, 0x4C, Some(id(1)), &[63, 63]);
    assert_eq!(drawn, 0);

    // The Haunt family's arm calls once (`loc_2CB1C`).
    let (data, mut roster) = setup(107, skill(0x4C, 0x07, 8, 2));
    let (_, _, drawn) = run(&mut roster, &data, 0x4C, Some(id(1)), &[63, 0]);
    assert_eq!(drawn, 1);
}

#[test]
fn only_the_sleep_handler_is_called_twice() {
    for route in ROUTES {
        if route.calls == Calls::TwiceLastDecides {
            assert_eq!(route.handler, Handler::SleepParalyze);
        }
    }
}

#[test]
fn exactly_the_effect_arms_whose_object_tests_the_caster_seal_are_sealable() {
    let mut sealable: Vec<(u16, u8)> = ROUTES
        .iter()
        .filter(|route| route.sealable)
        .map(|route| (route.enemy, route.ability))
        .collect();
    sealable.sort_unstable();
    assert_eq!(
        sealable,
        [
            // Lane A6: the DarkMaraud family's shared state 0 `loc_1E94C`
            // (frame $14, `ps4.asm:41371-41375`).
            (64, 0x26),
            (64, 0x28),
            (65, 0x26),
            (65, 0x27),
            (65, 0x29),
            (66, 0x26),
            (66, 0x29),
            (66, 0x2A),
            (70, 0x2D),
            (71, 0x2D),
            // Lane A6: BloodSaber's SHIFT object `$2C4` starts in DEBAN's
            // seal-testing state (40125-40129).
            (72, 0x26),
            (72, 0x2D),
            (72, 0x2F),
            (77, 0x2A),
            (88, 0x2F),
            (115, 0x28),
            (115, 0x29),
            (115, 0x2A),
            (115, 0x2F),
            (115, 0x57),
            // Lane A6: Radhin's objects share the Juza prelude `loc_2AB2E`
            // (seal test at 56109).
            (116, 0x26),
            (116, 0x27),
            (116, 0x29),
            (116, 0x2D),
        ]
    );
}

#[test]
fn a_sealed_vol_caster_kills_nobody() {
    let (data, mut roster) = setup(88, skill(0x2F, 0x02, 8, 2));
    roster.get_mut(id(6)).unwrap().stats.status |= status::TECH_SEALED;
    let (turn, events, drawn) = run(&mut roster, &data, 0x2F, Some(id(1)), &[63]);
    assert_eq!(turn, EffectTurn::Resolved);
    assert_eq!(drawn, 0);
    assert!(roster.get(id(1)).unwrap().is_alive());
    assert_eq!(events.len(), 1, "only the skill's name");
}

#[test]
fn a_sealed_tech_user_s_res_heals_nobody_and_draws_nothing() {
    // `resolve_res` gates on the RES record's own bytes (`is_tech_heal`); the
    // caster is otherwise constructed.
    let res = EnemySkill {
        id: 69,
        name: "RES".into(),
        effect: 18,
        power_stat: 130,
        target: 1,
        power: 16,
        resistance: 0,
        element: 0,
    };
    let (data, mut roster) = setup(99, res);
    let stats = &mut roster.get_mut(id(6)).unwrap().stats;
    stats.curr_hp = 1;
    stats.status |= status::TECH_SEALED;
    let mut rolls = SliceRolls::new(&[7]);
    let mut events = Vec::new();
    assert!(crate::battle::enemy_skill::resolve_res(
        &mut roster,
        id(6),
        69,
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(rolls.drawn(), 0);
    assert_eq!(roster.get(id(6)).unwrap().stats.curr_hp, 1);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::Healed { .. }))
    );
    // Unsealed, the same turn heals with sixteen draws.
    roster.get_mut(id(6)).unwrap().stats.status &= !status::TECH_SEALED;
    let mut rolls = SliceRolls::new(&[7]);
    assert!(crate::battle::enemy_skill::resolve_res(
        &mut roster,
        id(6),
        69,
        &data,
        &mut rolls,
        &mut Vec::new()
    ));
    assert_eq!(rolls.drawn(), 16);
    assert!(roster.get(id(6)).unwrap().stats.curr_hp > 1);
}
