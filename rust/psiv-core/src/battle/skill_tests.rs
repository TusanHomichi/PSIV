use super::*;
use crate::battle::{Battle, Command, Outcome, PartyMember, RoundOrders, SliceRolls, fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn data() -> BattleData {
    fixtures::data().with_skills([
        Skill {
            id: 1,
            name: "CROSSCUT".into(),
            effect: 1,
            power_stat: 5,
            requires_weapon: true,
            targeting: 0x11,
            power: 80,
            resistance: 6,
            element: 16,
        },
        Skill {
            id: 6,
            name: "VORTEX".into(),
            effect: 1,
            power_stat: 5,
            requires_weapon: true,
            targeting: 0x11,
            power: 48,
            resistance: 6,
            element: 16,
        },
        Skill {
            id: 31,
            name: "EARTH".into(),
            effect: 7,
            power_stat: 2,
            requires_weapon: true,
            targeting: 0x11,
            power: 32,
            resistance: 3,
            element: 11,
        },
        Skill {
            id: 34,
            name: "CRASH".into(),
            effect: 2,
            power_stat: 1,
            requires_weapon: true,
            targeting: 0x11,
            power: 48,
            resistance: 1,
            element: 14,
        },
        Skill {
            id: 47,
            name: "VISION".into(),
            effect: 38,
            power_stat: 0,
            requires_weapon: false,
            targeting: 0x15,
            power: 0,
            resistance: 0,
            element: 0,
        },
    ])
}

fn roster(data: &BattleData) -> Roster {
    let mut roster = Roster::new();
    for record in [fixtures::chaz(), fixtures::alys(), fixtures::hahn()] {
        let member = PartyMember::seat(&record, data).unwrap();
        roster.add_party_member(member.character, member.name, member.stats);
    }
    roster.add_enemy(1, &fixtures::zoran_bult());
    roster.add_enemy(2, &fixtures::zoran_bult());
    roster
}

#[test]
fn vortex_is_one_hit_with_sixteen_draws_and_spends_the_matching_slot() {
    let data = data();
    let mut roster = roster(&data);
    let stats = &mut roster.get_mut(id(2)).unwrap().stats;
    stats.skills.swap(0, 3);
    stats.curr_skill_uses.swap(0, 3);
    stats.curr_tp = 0;
    stats.status = status::TECH_SEALED;
    roster.get_mut(id(7)).unwrap().stats.curr_hp = 100;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    let died = resolve_skill(
        &mut roster,
        id(2),
        6,
        Some(id(7)),
        &data,
        &mut rolls,
        &mut events,
    )
    .unwrap();
    assert!(died.is_empty());
    assert_eq!(roster.get(id(7)).unwrap().stats.curr_hp, 47); // ((1 + 13 + 96) * 2 >> 2) - 2
    assert_eq!(roster.get(id(6)).unwrap().stats.curr_hp, 25);
    assert_eq!(
        roster.get(id(2)).unwrap().stats.curr_skill_uses,
        [0, 0, 0, 4, 0, 0, 0, 0]
    );
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_tp, 0);
    assert_eq!(rolls.drawn(), 16);
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
}

#[test]
fn crosscut_rolls_both_hits_independently_and_spends_one_matching_use() {
    let data = data();
    let mut roster = roster(&data);
    let stats = &mut roster.get_mut(id(2)).unwrap().stats;
    stats.skills[3] = 1;
    stats.curr_skill_uses[3] = 2;
    stats.curr_tp = 0;
    stats.status = status::TECH_SEALED;
    roster.get_mut(id(7)).unwrap().stats.curr_hp = 300;
    let mut draws = [0; 32];
    draws[16..].fill(7);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    let died = resolve_skill(
        &mut roster,
        id(2),
        1,
        Some(id(7)),
        &data,
        &mut rolls,
        &mut events,
    )
    .unwrap();
    assert!(died.is_empty());
    let hits: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Resolved {
                target,
                damage,
                remaining_hp,
                ..
            } => Some((*target, *damage, *remaining_hp)),
            _ => None,
        })
        .collect();
    assert_eq!(hits, vec![(id(7), Some(85), 215), (id(7), Some(96), 119)]);
    assert_eq!(rolls.drawn(), 32);
    assert_eq!(roster.get(id(6)).unwrap().stats.curr_hp, 25);
    assert_eq!(
        roster.get(id(2)).unwrap().stats.curr_skill_uses,
        [5, 0, 0, 1, 0, 0, 0, 0]
    );
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_tp, 0);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::SkillUsed { .. }))
            .count(),
        1
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { .. }))
    );
}

#[test]
fn crosscut_stops_after_a_kill_without_rolling_or_retargeting_its_second_hit() {
    let data = data();
    let mut roster = roster(&data);
    let stats = &mut roster.get_mut(id(2)).unwrap().stats;
    stats.skills[3] = 1;
    stats.curr_skill_uses[3] = 2;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    let died = resolve_skill(
        &mut roster,
        id(2),
        1,
        Some(id(7)),
        &data,
        &mut rolls,
        &mut events,
    )
    .unwrap();
    assert_eq!(died, vec![id(7)]);
    assert_eq!(rolls.drawn(), 16);
    assert_eq!(roster.get(id(7)).unwrap().stats.curr_hp, 0);
    assert_eq!(roster.get(id(6)).unwrap().stats.curr_hp, 25);
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_skill_uses[3], 1);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Resolved { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Died { .. }))
            .count(),
        1
    );
}

#[test]
fn earth_uses_mental_against_agility_and_does_not_lower_agility() {
    for (roll, succeeds) in [(16, false), (17, true)] {
        let data = data();
        let mut roster = roster(&data);
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        resolve_skill(
            &mut roster,
            id(1),
            31,
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events,
        )
        .unwrap();
        let target = &roster.get(id(6)).unwrap().stats;
        assert_eq!(target.status & status::ASLEEP != 0, succeeds);
        assert_eq!(target.agility.battle, 6);
        assert_eq!(target.curr_hp, 25);
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_skill_uses[0], 2);
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 10);
        assert_eq!(rolls.drawn(), 1);
    }
}

#[test]
fn earth_on_sleep_or_paralysis_spends_a_use_without_rolling() {
    for condition in [status::ASLEEP, status::PARALYZED] {
        let data = data();
        let mut roster = roster(&data);
        roster.get_mut(id(6)).unwrap().stats.status = condition;
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_skill(
            &mut roster,
            id(1),
            31,
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events,
        )
        .unwrap();
        assert_eq!(rolls.drawn(), 0);
        assert_eq!(roster.get(id(6)).unwrap().stats.status, condition);
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_skill_uses[0], 2);
        assert!(events.contains(&BattleEvent::SkillIneffective {
            actor: id(1),
            target: id(6)
        }));
    }
}

#[test]
fn round_recovery_rolls_in_slot_order_and_clears_only_enemy_paralysis() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.status = status::ASLEEP | status::PARALYZED;
    let second = &mut roster.get_mut(id(2)).unwrap().stats;
    second.status = status::ASLEEP | status::ASLEEP_2;
    second.agility.battle = 1;
    second.dexterity.battle = 21;
    roster.get_mut(id(6)).unwrap().stats.status = status::ASLEEP_2 | status::DEAD;
    roster.get_mut(id(7)).unwrap().stats.status = status::PARALYZED;
    let mut rolls = SliceRolls::new(&[2, 3, 5]);
    let mut events = Vec::new();
    recover_round_status(&mut roster, &mut rolls, &mut events);
    assert_eq!(rolls.drawn(), 3);
    assert_eq!(
        roster.get(id(1)).unwrap().stats.status,
        status::ASLEEP | status::PARALYZED
    );
    let second = &roster.get(id(2)).unwrap().stats;
    assert_eq!(second.status, 0);
    assert_eq!(second.agility.battle, 15);
    assert_eq!(second.dexterity.battle, 21);
    assert_eq!(roster.get(id(6)).unwrap().stats.status, status::DEAD);
    assert_eq!(roster.get(id(7)).unwrap().stats.status, 0);
    assert_eq!(
        events,
        vec![
            BattleEvent::WokeUp { fighter: id(2) },
            BattleEvent::WokeUp { fighter: id(6) },
            BattleEvent::ParalysisCleared { fighter: id(7) }
        ]
    );
}

#[test]
fn vision_is_plus_eight_nonstacking_and_independent_of_hahns_name() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(3)).unwrap().stats.name_bytes[0] = 26; // Rename to Z...
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    for _ in 0..2 {
        resolve_skill(&mut roster, id(3), 47, None, &data, &mut rolls, &mut events).unwrap();
        for (actor, dex) in [(1, 13), (2, 21), (3, 13)] {
            assert_eq!(roster.get(id(actor)).unwrap().stats.dexterity.battle, dex);
        }
    }
    assert_eq!(rolls.drawn(), 0);
    assert_eq!(roster.get(id(3)).unwrap().stats.curr_tp, 25);
    assert_eq!(roster.get(id(3)).unwrap().stats.curr_skill_uses[0], 3);
}

#[test]
fn vision_excludes_dead_and_android_targets() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.status = status::DEAD;
    roster.get_mut(id(2)).unwrap().stats.profession = 5;
    let mut events = Vec::new();
    resolve_skill(
        &mut roster,
        id(3),
        47,
        None,
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    )
    .unwrap();
    assert_eq!(roster.get(id(1)).unwrap().stats.dexterity.battle, 5);
    assert_eq!(roster.get(id(2)).unwrap().stats.dexterity.battle, 13);
    assert_eq!(roster.get(id(3)).unwrap().stats.dexterity.battle, 13);
}

#[test]
fn rejected_skills_do_not_become_attacks_or_spend_uses() {
    for (skill, target, exhausted, reason) in [
        (99, Some(id(6)), false, SkillRejection::Unavailable),
        (6, Some(id(6)), false, SkillRejection::NotLearned),
        (31, Some(id(1)), false, SkillRejection::InvalidTarget),
        (31, None, false, SkillRejection::InvalidTarget),
        (31, Some(id(6)), true, SkillRejection::Exhausted),
    ] {
        let data = data();
        let mut roster = roster(&data);
        if exhausted {
            roster.get_mut(id(1)).unwrap().stats.curr_skill_uses[0] = 0;
        }
        let before = roster.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        resolve_skill(
            &mut roster,
            id(1),
            skill,
            target,
            &data,
            &mut rolls,
            &mut events,
        )
        .unwrap();
        assert_eq!(rolls.drawn(), 0);
        assert_eq!(roster, before);
        assert_eq!(
            events,
            vec![BattleEvent::SkillRejected {
                actor: id(1),
                skill,
                reason
            }]
        );
    }
}

#[test]
fn losing_the_required_weapon_after_selection_still_spends_one_use() {
    let data = data();
    let mut roster = roster(&data);
    // Two shields must not pass the weapon requirement.
    roster.get_mut(id(1)).unwrap().stats.equipment = [10, 10, 5, 4];
    let mut rolls = SliceRolls::new(&[63]);
    let mut events = Vec::new();
    resolve_skill(
        &mut roster,
        id(1),
        31,
        Some(id(6)),
        &data,
        &mut rolls,
        &mut events,
    )
    .unwrap();
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_skill_uses[0], 2);
    assert_eq!(roster.get(id(6)).unwrap().stats.status, 0);
    assert_eq!(rolls.drawn(), 0);
    assert!(events.contains(&BattleEvent::SkillRejected {
        actor: id(1),
        skill: 31,
        reason: SkillRejection::Unarmed
    }));
}

#[test]
fn vortex_retargets_a_dead_enemy_and_awards_victory_rewards() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(6)).unwrap().stats.status = status::DEAD;
    let died = resolve_skill(
        &mut roster,
        id(2),
        6,
        Some(id(6)),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(died, vec![id(7)]);
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.enemies.truncate(1);
    let (mut battle, _) = Battle::start(
        &formation,
        vec![PartyMember::seat(&fixtures::alys(), &data).unwrap()],
        &data,
        false,
        0,
        &mut SliceRolls::new(&[63]),
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Skill {
                skill: 6,
                target: Some(id(6)),
            }]),
            &data,
            &mut SliceRolls::new(&[0]),
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::Ended {
        outcome: Outcome::Victory
    }));
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::Rewarded {
            meseta: 3,
            experience_total: 12,
            ..
        }
    )));
    assert_eq!(battle.into_party()[0].stats.curr_skill_uses[0], 4);
}

#[test]
fn earth_skips_a_later_enemy_turn_and_recovers_at_round_end() {
    let data = data();
    let mut chaz = PartyMember::seat(&fixtures::chaz(), &data).unwrap();
    chaz.stats.agility.battle = 100;
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.enemies.truncate(1);
    let (mut battle, _) = Battle::start(
        &formation,
        vec![chaz],
        &data,
        true,
        0,
        &mut SliceRolls::new(&[32]),
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Skill {
                skill: 31,
                target: Some(id(6)),
            }]),
            &data,
            &mut SliceRolls::new(&[63]),
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::FellAsleep {
        actor: id(1),
        target: id(6)
    }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::TurnSkipped { actor, .. } if *actor == id(6)))
    );
    assert!(events.contains(&BattleEvent::WokeUp { fighter: id(6) }));
    assert_eq!(battle.roster().get(id(6)).unwrap().stats.status, 0);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut SliceRolls::new(&[32]),
        )
        .unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6)))
    );
}

#[test]
fn crash_uses_strength_destroy_resistance_and_one_use_in_its_learned_slot() {
    let data = data();
    for (element, roll, killed) in [(0, 63, false), (2, 24, false), (2, 25, true), (3, 17, true)] {
        let mut roster = roster(&data);
        let caster = &mut roster.get_mut(id(1)).unwrap().stats;
        caster.skills = [0; 8];
        caster.skills[5] = 34;
        caster.curr_skill_uses[5] = 7;
        caster.strength.battle = 20;
        caster.mental.battle = 0;
        caster.curr_tp = 0;
        caster.status |= status::TECH_SEALED;
        let target = &mut roster.get_mut(id(6)).unwrap().stats;
        target.strength.battle = 20;
        target.element_props[13] = element;
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        let died = resolve_skill(
            &mut roster,
            id(1),
            34,
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events,
        )
        .unwrap();
        assert_eq!(!died.is_empty(), killed, "element={element} roll={roll}");
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_skill_uses[5], 6);
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 0);
        assert_eq!(
            rolls.drawn(),
            1,
            "no physical damage, accuracy or critical draws"
        );
        assert_eq!(
            roster.get(id(6)).unwrap().stats.curr_hp,
            if killed { 0 } else { 25 }
        );
        assert!(!events.iter().any(|e| matches!(
            e,
            BattleEvent::Resolved {
                damage: Some(_),
                ..
            }
        )));
    }
}

#[test]
fn crash_death_awards_enemy_rewards_and_persists_its_spent_use() {
    let data = data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.enemies.truncate(1);
    let mut member = PartyMember::seat(&fixtures::alys(), &data).unwrap();
    member.stats.skills[0] = 34;
    let (mut battle, _) = Battle::start(
        &formation,
        vec![member],
        &data,
        false,
        0,
        &mut SliceRolls::new(&[63]),
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Skill {
                skill: 34,
                target: Some(id(6)),
            }]),
            &data,
            &mut SliceRolls::new(&[63]),
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::Ended {
        outcome: Outcome::Victory
    }));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                BattleEvent::Rewarded {
                    meseta: 3,
                    experience_total: 12,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(
        !events.iter().any(
            |e| matches!(e, BattleEvent::Attacked { actor, .. } if actor.side() == Side::Enemy)
        )
    );
    assert_eq!(battle.into_party()[0].stats.curr_skill_uses[0], 4);
}
