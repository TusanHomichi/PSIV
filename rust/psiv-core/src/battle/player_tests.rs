//! Family regressions with synthetic parameters; cartridge values stay in the
//! decoded pack and captured replay fixtures.
use super::*;

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn skill(id: u8, effect: u8, range: u8) -> Skill {
    Skill {
        id,
        name: "sample".into(),
        effect,
        power_stat: 2,
        requires_weapon: false,
        targeting: 0x10 | range,
        power: 2,
        resistance: 0,
        element: 1,
    }
}

fn roster(data: &BattleData) -> Roster {
    let mut result = Roster::new();
    for record in [fixtures::chaz(), fixtures::alys(), fixtures::hahn()] {
        let member = PartyMember::seat(&record, data).unwrap();
        result.add_party_member(member.character, member.name, member.stats);
    }
    let mut enemy = fixtures::zoran_bult();
    enemy.hp = 800;
    result.add_enemy(1, &enemy);
    result.add_enemy(2, &enemy);
    result
}

fn cast(
    roster: &mut Roster,
    skill: Skill,
    target: Option<FighterId>,
    draws: &[u16],
) -> (Vec<BattleEvent>, usize) {
    let stats = &mut roster.get_mut(id(1)).unwrap().stats;
    stats.skills[0] = skill.id;
    stats.curr_skill_uses[0] = 2;
    let data = fixtures::data().with_skills([skill.clone()]);
    let mut rolls = SliceRolls::new(draws);
    let mut events = Vec::new();
    super::skill::resolve_skill(
        roster,
        id(1),
        skill.id,
        target,
        &data,
        &mut rolls,
        &mut events,
    )
    .unwrap();
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_skill_uses[0], 1);
    (events, rolls.drawn())
}

#[test]
fn every_damage_dispatch_has_one_damage_run_plus_its_animation_rng() {
    for ability in (1..=19).chain(49..=51) {
        let mut fighters = roster(&fixtures::data());
        let animation_draws = match ability {
            13 => 16,
            15 => 12,
            _ => 0,
        };
        let (events, draws) = cast(&mut fighters, skill(ability, 1, 1), Some(id(6)), &[0]);
        assert_eq!(draws, 16 + animation_draws, "ability {ability}");
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::Resolved { .. }))
                .count(),
            1
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::Attacked { .. }))
        );
        assert_eq!(fighters.get(id(7)).unwrap().stats.curr_hp, 800);
    }
}

#[test]
fn double_slash_has_no_extra_damage_or_retarget_after_a_kill() {
    let mut fighters = roster(&fixtures::data());
    fighters.get_mut(id(6)).unwrap().stats.curr_hp = 1;
    let (events, draws) = cast(&mut fighters, skill(3, 1, 1), Some(id(6)), &[0]);
    assert_eq!(draws, 16);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Died { .. }))
            .count(),
        1
    );
    assert_eq!(fighters.get(id(7)).unwrap().stats.curr_hp, 800);
}

#[test]
fn multi_target_damage_walks_all_targets_in_fighter_order() {
    let mut fighters = roster(&fixtures::data());
    let (events, draws) = cast(&mut fighters, skill(11, 1, 2), None, &[0]);
    let targets: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved { target, .. } => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(targets, [id(6), id(7)]);
    assert_eq!(draws, 32);
}

#[test]
fn group_skill_reaims_a_stored_dead_target_without_shrinking_its_range() {
    for (aim, expected_draws) in [(Some(id(6)), 33), (None, 32), (Some(id(7)), 32)] {
        let mut fighters = roster(&fixtures::data());
        fighters.get_mut(id(6)).unwrap().stats.curr_hp = 0;
        fighters.get_mut(id(6)).unwrap().stats.status = status::DEAD;
        fighters.add_enemy(3, &fixtures::zoran_bult());
        fighters.get_mut(id(8)).unwrap().stats.curr_hp = 800;
        fighters.get_mut(id(8)).unwrap().stats.max_hp = 800;
        let (events, draws) = cast(&mut fighters, skill(11, 1, 2), aim, &[0]);
        assert_eq!(draws, expected_draws);
        let targets: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::Resolved { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        assert_eq!(
            targets,
            [id(7), id(8)],
            "the group effect still reaches both"
        );
    }
}

#[test]
fn skill_sleep_short_circuits_and_never_lowers_agility() {
    for ability in [31, 32, 33, 35, 36, 37] {
        let mut fighters = roster(&fixtures::data());
        let before = fighters.get(id(6)).unwrap().stats.agility.battle;
        let mut record = skill(ability, 7, 1);
        record.resistance = 2;
        let (_, draws) = cast(&mut fighters, record.clone(), Some(id(6)), &[0]);
        assert_eq!(draws, 1, "a fresh target takes its chance draw");
        assert_eq!(fighters.get(id(6)).unwrap().stats.agility.battle, before);
        let (events, draws) = cast(&mut fighters, record.clone(), Some(id(6)), &[0]);
        assert_eq!(draws, 0);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BattleEvent::SkillIneffective { .. }))
        );
        assert_eq!(fighters.get(id(6)).unwrap().stats.status, status::ASLEEP);
        fighters.get_mut(id(6)).unwrap().stats.status = status::PARALYZED;
        let (_, draws) = cast(&mut fighters, record, Some(id(6)), &[0]);
        assert_eq!(
            draws, 0,
            "paralysis also short circuits before the chance draw"
        );
        assert_eq!(fighters.get(id(6)).unwrap().stats.status, status::PARALYZED);
    }
}

#[test]
fn ataraxia_restores_tp_not_hp_and_clears_only_sleep_even_on_an_android() {
    let mut fighters = roster(&fixtures::data());
    for fighter in fighters.iter_mut().filter(|f| f.id.side() == Side::Party) {
        fighter.stats.curr_tp = 0;
        fighter.stats.curr_hp = 7;
    }
    let android = &mut fighters.get_mut(id(2)).unwrap().stats;
    android.profession = 5;
    android.status = status::ASLEEP | status::ASLEEP_2;
    android.agility.battle = 1;
    let (events, draws) = cast(&mut fighters, skill(46, 15, 5), None, &[0]);
    assert_eq!(draws, 32, "two humans, sixteen draws each");
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::TpRestored { .. }))
            .count(),
        2
    );
    for fighter in fighters.iter().filter(|f| f.id.side() == Side::Party) {
        assert_eq!(fighter.stats.curr_hp, 7);
    }
    let android = &fighters.get(id(2)).unwrap().stats;
    assert_eq!(android.curr_tp, 0);
    assert_eq!(android.status, status::ASLEEP_2);
    assert_eq!(android.agility.battle, 1);
}

#[test]
fn medic_power_revives_and_cures_humans_without_restoring_battle_stats() {
    let mut fighters = roster(&fixtures::data());
    let target = &mut fighters.get_mut(id(2)).unwrap().stats;
    target.curr_hp = 0;
    target.status = status::DEAD | status::POISONED | status::TECH_SEALED;
    target.agility.battle = 1;
    fighters.get_mut(id(3)).unwrap().stats.profession = 5;
    let (events, draws) = cast(&mut fighters, skill(45, 23, 5), None, &[0]);
    assert_eq!(draws, 32);
    let target = &fighters.get(id(2)).unwrap().stats;
    assert!(target.curr_hp > 0);
    assert_eq!(target.status, status::TECH_SEALED);
    assert_eq!(target.agility.battle, 1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Revived { target, .. } if *target == id(2)))
    );
}

#[test]
fn feeve_preserves_the_odd_byte_shadow_write_and_does_not_consume_rng() {
    let mut fighters = roster(&fixtures::data());
    let stats = &mut fighters.get_mut(id(1)).unwrap().stats;
    stats.techniques[0] = 33;
    let data = fixtures::data().with_techniques([Technique {
        id: 33,
        name: "sample".into(),
        effect: 13,
        cost: 1,
        targeting: 0x19,
        power: 11,
        resistance: 0,
        element: 0,
    }]);
    let before = fighters.get(id(1)).unwrap().stats.element_props;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    super::technique::resolve_technique(
        &mut fighters,
        id(1),
        33,
        None,
        &data,
        &mut rolls,
        &mut events,
    );
    let stats = &fighters.get(id(1)).unwrap().stats;
    assert_eq!(stats.element_props, before);
    assert_eq!(stats.element_shadow[4], 1);
    assert_eq!(rolls.drawn(), 0);
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::ResistanceChanged {
            shadow: true,
            element: 5,
            ..
        }
    )));
}

#[test]
fn roster_wake_includes_off_party_dead_androids_and_keeps_other_bits_and_agility() {
    let mut roster = crate::CharacterRoster::new();
    let mut stats = Stats::from_character(&fixtures::chaz(), |_| None);
    stats.status = status::ASLEEP | status::ASLEEP_2 | status::ANDROID_DEAD;
    stats.agility.battle = 1;
    roster.seat(crate::CharId(10), stats).unwrap();
    BattleEvent::CharacterSleepCleared.apply_character_roster(&mut roster);
    let stats = roster.get(crate::CharId(10)).unwrap();
    assert_eq!(stats.status, status::ASLEEP_2 | status::ANDROID_DEAD);
    assert_eq!(stats.agility.battle, 1);
}

#[test]
fn every_death_skill_dispatch_kills_only_after_the_effect_pass_lands() {
    for ability in (20..=27).chain([34, 48, 52]) {
        let mut fighters = roster(&fixtures::data());
        let (events, draws) = cast(&mut fighters, skill(ability, 2, 1), Some(id(6)), &[0]);
        assert_eq!(draws, 0, "zero-resistance pass bypasses chances");
        assert_eq!(fighters.get(id(6)).unwrap().stats.curr_hp, 0);
        assert_eq!(fighters.get(id(7)).unwrap().stats.curr_hp, 800);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::Died { fighter } if *fighter == id(6)))
        );
        let mut fighters = roster(&fixtures::data());
        let mut ability_record = skill(ability, 2, 1);
        ability_record.resistance = 2;
        ability_record.element = 8;
        fighters.get_mut(id(6)).unwrap().stats.element_props[7] = 0;
        let (_, draws) = cast(&mut fighters, ability_record, Some(id(6)), &[0]);
        assert_eq!(draws, 1);
        assert_eq!(fighters.get(id(6)).unwrap().stats.curr_hp, 800);
    }
}

#[test]
fn support_skill_families_replace_base_stats_instead_of_stacking() {
    for (ability, effect, stat) in [
        (38, 11, TechniqueStat::MentalDefence),
        (53, 11, TechniqueStat::MentalDefence),
        (39, 9, TechniqueStat::Attack),
        (40, 10, TechniqueStat::Defence),
        (41, 10, TechniqueStat::Defence),
        (47, 38, TechniqueStat::Dexterity),
    ] {
        let mut fighters = roster(&fixtures::data());
        let (first, draws) = cast(&mut fighters, skill(ability, effect, 3), None, &[0]);
        assert_eq!(draws, 0);
        let (second, draws) = cast(&mut fighters, skill(ability, effect, 3), None, &[0]);
        assert_eq!(draws, 0);
        let value = |events: Vec<BattleEvent>| {
            events
                .into_iter()
                .find_map(|e| match e {
                    BattleEvent::StatChanged {
                        stat: changed,
                        value,
                        ..
                    } if changed == stat => Some(value),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(value(first), value(second), "ability {ability}");
    }
    for (ability, effect) in [(28, 6), (29, 3), (30, 6)] {
        let mut fighters = roster(&fixtures::data());
        let (events, draws) = cast(&mut fighters, skill(ability, effect, 2), None, &[0]);
        assert_eq!(draws, 0);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::StatChanged { .. }))
                .count(),
            2
        );
    }
}

#[test]
fn recovery_skills_respect_biology_and_draw_for_full_recipients() {
    for ability in [42, 43, 44, 54] {
        let mut fighters = roster(&fixtures::data());
        fighters.get_mut(id(1)).unwrap().stats.profession = 5;
        fighters.get_mut(id(2)).unwrap().stats.profession = 5;
        let range = if ability == 43 {
            6
        } else if ability == 44 {
            9
        } else {
            3
        };
        let target = (range == 6).then(|| id(2));
        let (events, draws) = cast(&mut fighters, skill(ability, 18, range), target, &[0]);
        // MIRACLE's decoded range 9 covers all three, including both
        // androids and the human (Ability_ProcessRange, ps4.asm:8975-9017).
        let count = if ability == 44 { 3 } else { 1 };
        assert_eq!(draws, count * 16, "ability {ability}");
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::Healed { amount: 0, .. }))
                .count(),
            count
        );
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::Healed { target, .. } if *target == id(3))),
            ability == 44
        );
    }
}
