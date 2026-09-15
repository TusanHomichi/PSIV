use super::*;
use crate::battle::{Battle, Command, Outcome, PartyMember, RoundOrders, SliceRolls, fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn data() -> BattleData {
    let records = [
        (1, "FOI", 1, 3, 0x11, 24, 7, 3),
        (10, "ZAN", 1, 8, 0x12, 16, 7, 1),
        (17, "BROSE", 2, 16, 0x12, 48, 1, 9),
        (18, "VOL", 2, 8, 0x11, 48, 2, 10),
        (19, "SAVOL", 2, 18, 0x12, 48, 2, 10),
        (20, "GELUN", 3, 5, 0x12, 32, 2, 10),
        (23, "RIMIT", 7, 10, 0x12, 48, 2, 11),
        (24, "RES", 18, 3, 0x34, 16, 0, 0),
        (27, "SAR", 18, 12, 0x35, 0, 0, 0),
        (30, "SHIFT", 9, 7, 0x14, 0, 0, 0),
        (31, "SANER", 12, 6, 0x19, 0, 0, 0),
        (33, "FEEVE", 13, 5, 0x19, 11, 0, 0),
        (34, "ANTI", 19, 2, 0x34, 0, 0, 0),
        (35, "RIMPA", 20, 5, 0x34, 0, 0, 0),
        (36, "REVER", 21, 12, 0x34, 0, 0, 0),
        (37, "REGEN", 22, 36, 0x34, 0, 0, 0),
    ];
    fixtures::data().with_techniques(records.into_iter().map(
        |(id, name, effect, cost, targeting, power, resistance, element)| Technique {
            id,
            name: name.into(),
            effect,
            cost,
            targeting,
            power,
            resistance,
            element,
        },
    ))
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
fn res_pays_once_heals_with_sixteen_draws_and_caps_at_maximum() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.curr_hp = 1;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        24,
        Some(id(1)),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 7);
    // MEN 6, power 16, sixteen zero draws: (0 + 6 + 32) / 2 = 19.
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_hp, 20);
    assert_eq!(rolls.drawn(), 16);
    resolve_technique(
        &mut roster,
        id(1),
        24,
        Some(id(1)),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_hp, 25);
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::Healed {
            amount: 5,
            remaining_hp: 25,
            ..
        }
    )));
    assert_eq!(rolls.drawn(), 32);
}

#[test]
fn foi_uses_mental_element_and_magic_defense_without_a_hit_roll() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(7)).unwrap().stats.mental_defence.battle = 8;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(2),
        1,
        Some(id(7)),
        &data,
        &mut rolls,
        &mut events,
    );
    // ((8*12 >> 6) + 12 + 48) * 2 >> 2, minus 8 = 22.
    assert_eq!(roster.get(id(7)).unwrap().stats.curr_hp, 3);
    assert_eq!(roster.get(id(6)).unwrap().stats.curr_hp, 25);
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_tp, 37);
    assert_eq!(rolls.drawn(), 16);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { .. }))
    );
}

#[test]
fn invalid_commands_do_not_spend_tp_or_roll_or_become_attacks() {
    for (tech, target, reason) in [
        (99, Some(id(1)), TechniqueRejection::Unavailable),
        (33, None, TechniqueRejection::Unavailable),
        (1, Some(id(6)), TechniqueRejection::NotLearned),
        (24, Some(id(6)), TechniqueRejection::InvalidTarget),
    ] {
        let data = data();
        let mut roster = roster(&data);
        let before = roster.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        resolve_technique(
            &mut roster,
            id(1),
            tech,
            target,
            &data,
            &mut rolls,
            &mut events,
        );
        assert_eq!(roster, before);
        assert_eq!(rolls.drawn(), 0);
        assert_eq!(
            events,
            vec![BattleEvent::TechniqueRejected {
                actor: id(1),
                technique: tech,
                reason
            }]
        );
    }
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.curr_tp = 2;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        24,
        Some(id(1)),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 2);
    assert_eq!(rolls.drawn(), 0);
    assert!(matches!(
        events[0],
        BattleEvent::TechniqueRejected {
            reason: TechniqueRejection::InsufficientTp,
            ..
        }
    ));
}

#[test]
fn sealing_after_selection_costs_tp_but_produces_no_effect() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.status |= status::TECH_SEALED;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        24,
        Some(id(1)),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 7);
    assert_eq!(rolls.drawn(), 0);
    assert!(matches!(
        events[1],
        BattleEvent::TechniqueRejected {
            reason: TechniqueRejection::Sealed,
            ..
        }
    ));
}

#[test]
fn dead_healing_target_is_not_replaced_and_android_is_not_eligible() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(2)).unwrap().stats.status = status::DEAD;
    roster.get_mut(id(2)).unwrap().stats.curr_hp = 0;
    roster.get_mut(id(3)).unwrap().stats.profession = super::super::PROFESSION_ANDROID;
    assert_eq!(
        technique_targets(&roster, id(1), data.technique(24).unwrap()),
        vec![id(1)]
    );
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        24,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_hp, 0);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 7);
    assert_eq!(rolls.drawn(), 0);
    assert_eq!(events.len(), 1);
}

#[test]
fn a_dead_enemy_target_retargets_and_kills_with_rewards() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(6)).unwrap().stats.status = status::DEAD;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert_eq!(
        resolve_technique(
            &mut roster,
            id(2),
            1,
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events
        ),
        vec![id(7)]
    );
    let mut party = vec![PartyMember::seat(&fixtures::alys(), &data).unwrap()];
    party[0].stats.techniques[3] = 10;
    // ZAN with zero draws does 22 to each. Pre-injure the formation to 20.
    let mut enemy = fixtures::zoran_bult();
    enemy.hp = 20;
    let data = data.with_enemies([enemy]);
    let (mut battle, _) = Battle::start(
        &fixtures::formation_two_zoran_bults(),
        party,
        &data,
        false,
        &mut SliceRolls::new(&[63]),
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Technique {
                technique: 10,
                target: None,
            }]),
            &data,
            &mut SliceRolls::new(&[0]),
        )
        .unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::Rewarded {
            meseta: 6,
            experience_total: 24,
            ..
        }
    )));
    assert!(events.contains(&BattleEvent::Ended {
        outcome: Outcome::Victory
    }));
}

#[test]
fn support_uses_caster_mental_and_replaces_previous_buff() {
    let data = data();
    let mut roster = roster(&data);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    for _ in 0..2 {
        resolve_technique(
            &mut roster,
            id(2),
            30,
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events,
        );
    }
    assert_eq!(roster.get(id(1)).unwrap().stats.attack.battle, 30); // base 18 + Alys MEN 12
    resolve_technique(&mut roster, id(2), 31, None, &data, &mut rolls, &mut events);
    assert_eq!(roster.get(id(1)).unwrap().stats.agility.battle, 19);
    assert_eq!(roster.get(id(2)).unwrap().stats.agility.battle, 27);
    assert_eq!(roster.get(id(3)).unwrap().stats.agility.battle, 16);
    assert_eq!(rolls.drawn(), 0);
}

#[test]
fn gelun_rolls_once_per_enemy_and_preserves_stats_on_a_miss() {
    let data = data();
    let mut roster = roster(&data);
    let mut rolls = SliceRolls::new(&[0, 63]);
    let mut events = Vec::new();
    resolve_technique(&mut roster, id(3), 20, None, &data, &mut rolls, &mut events);
    assert_eq!(roster.get(id(6)).unwrap().stats.attack.battle, 16);
    assert_eq!(roster.get(id(7)).unwrap().stats.attack.battle, 8);
    assert_eq!(rolls.drawn(), 2);
    assert_eq!(roster.get(id(3)).unwrap().stats.curr_tp, 20);
}

#[test]
fn battle_exit_removes_support_buffs_but_keeps_tp_spent() {
    let data = data();
    let party = vec![PartyMember::seat(&fixtures::alys(), &data).unwrap()];
    let (mut battle, _) = Battle::start(
        &fixtures::formation_two_zoran_bults(),
        party,
        &data,
        false,
        &mut SliceRolls::new(&[63]),
    )
    .unwrap();
    battle
        .round(
            &RoundOrders::Commands(vec![Command::Technique {
                technique: 30,
                target: Some(id(1)),
            }]),
            &data,
            &mut SliceRolls::new(&[0]),
        )
        .unwrap();
    assert_eq!(battle.roster().get(id(1)).unwrap().stats.attack.battle, 25);
    let party = battle.into_party();
    assert_eq!(party[0].stats.attack.battle, 13);
    assert_eq!(party[0].stats.curr_tp, 33);
}

#[test]
fn brose_uses_one_chance_per_enemy_in_order_and_spends_tp_once() {
    let data = data();
    let mut roster = roster(&data);
    let caster = &mut roster.get_mut(id(1)).unwrap().stats;
    caster.techniques[0] = 17;
    caster.curr_tp = 40;
    caster.mental.battle = 20;
    caster.strength.battle = 0; // BROSE uses MEN, despite STR resistance.
    for target in [6, 7] {
        let stats = &mut roster.get_mut(id(target)).unwrap().stats;
        stats.strength.battle = 20;
        stats.element_props[8] = 2;
    }
    let mut rolls = SliceRolls::new(&[24, 25]);
    let mut events = Vec::new();
    assert_eq!(
        resolve_technique(&mut roster, id(1), 17, None, &data, &mut rolls, &mut events),
        vec![id(7)]
    );
    assert_eq!(rolls.drawn(), 2);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 24);
    assert_eq!(
        roster.get(id(6)).unwrap().stats.curr_hp,
        25,
        "equality misses"
    );
    assert_eq!(roster.get(id(7)).unwrap().stats.curr_hp, 0);
    assert_ne!(roster.get(id(7)).unwrap().stats.status & status::DEAD, 0);
    assert!(events.iter().any(|e| matches!(e, BattleEvent::Resolved { target, verdict: Verdict::Miss, damage: None, .. } if *target == id(6))));
    assert!(!events.iter().any(|e| matches!(
        e,
        BattleEvent::Resolved {
            damage: Some(_),
            ..
        }
    )));
}

#[test]
fn vol_retargets_a_dead_enemy_but_respects_biological_immunity() {
    let data = data();
    let mut roster = roster(&data);
    let caster = &mut roster.get_mut(id(1)).unwrap().stats;
    caster.techniques[0] = 18;
    caster.curr_tp = 40;
    caster.mental.battle = 255;
    let dead = &mut roster.get_mut(id(6)).unwrap().stats;
    dead.curr_hp = 0;
    dead.status |= status::DEAD;
    roster.get_mut(id(7)).unwrap().stats.element_props[9] = 0;
    let mut rolls = SliceRolls::new(&[63]);
    let mut events = Vec::new();
    assert!(
        resolve_technique(
            &mut roster,
            id(1),
            18,
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events
        )
        .is_empty()
    );
    assert_eq!(
        rolls.drawn(),
        1,
        "dead target takes no draw, immune living target does"
    );
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 32);
    assert_eq!(roster.get(id(7)).unwrap().stats.curr_hp, 25);
    assert!(events.iter().any(|e| matches!(e, BattleEvent::Resolved { target, verdict: Verdict::Miss, .. } if *target == id(7))));
}

#[test]
fn savol_kills_award_the_whole_formation_once_and_skip_enemy_turns() {
    let data = data();
    let mut member = PartyMember::seat(&fixtures::alys(), &data).unwrap();
    member.stats.techniques[3] = 19;
    let (mut battle, _) = Battle::start(
        &fixtures::formation_two_zoran_bults(),
        vec![member],
        &data,
        false,
        &mut SliceRolls::new(&[63]),
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Technique {
                technique: 19,
                target: None,
            }]),
            &data,
            &mut SliceRolls::new(&[63]),
        )
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Died { .. }))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                BattleEvent::Rewarded {
                    meseta: 6,
                    experience_total: 24,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(events.contains(&BattleEvent::Ended {
        outcome: Outcome::Victory
    }));
    assert!(
        !events.iter().any(
            |e| matches!(e, BattleEvent::Attacked { actor, .. } if actor.side() == Side::Enemy)
        )
    );
    assert_eq!(battle.into_party()[0].stats.curr_tp, 22);
}

fn recovery_roster(data: &BattleData) -> Roster {
    let mut roster = roster(data);
    let caster = &mut roster.get_mut(id(1)).unwrap().stats;
    caster.techniques[..4].copy_from_slice(&[34, 35, 36, 37]);
    caster.curr_tp = 100;
    roster
}

#[test]
fn anti_and_rimpa_remove_only_their_ailment_and_reset_agi_dex_without_rng() {
    let data = data();
    for (tech, cleared, cost) in [(34, status::POISONED, 2), (35, status::PARALYZED, 5)] {
        let mut roster = recovery_roster(&data);
        let stats = &mut roster.get_mut(id(2)).unwrap().stats;
        stats.status = status::POISONED | status::PARALYZED | status::ASLEEP | status::TECH_SEALED;
        stats.curr_hp = 7;
        stats.curr_tp = 9;
        stats.curr_skill_uses[0] = 1;
        stats.agility.battle = 200;
        stats.dexterity.battle = 180;
        stats.attack.battle = 190;
        let mut expected = stats.clone();
        expected.status &= !cleared;
        expected.agility.battle = expected.agility.modified;
        expected.dexterity.battle = expected.dexterity.modified;
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_technique(
            &mut roster,
            id(1),
            tech,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events,
        );
        assert_eq!(roster.get(id(2)).unwrap().stats, expected);
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 100 - cost);
        assert_eq!(rolls.drawn(), 0);
        assert!(events.contains(&BattleEvent::StatusRestored {
            actor: id(1),
            target: id(2),
            removed: cleared
        }));
    }
}

#[test]
fn a_paid_cure_on_healthy_target_still_resets_agility_and_dexterity() {
    let data = data();
    let mut roster = recovery_roster(&data);
    roster.get_mut(id(2)).unwrap().stats.agility.battle = 200;
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        34,
        Some(id(2)),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    );
    let stats = &roster.get(id(2)).unwrap().stats;
    assert_eq!(stats.agility.battle, stats.agility.modified);
    assert!(events.contains(&BattleEvent::StatsRestored {
        actor: id(1),
        target: id(2)
    }));
    events.clear();
    resolve_technique(
        &mut roster,
        id(1),
        34,
        Some(id(2)),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    );
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 96);
    assert!(events.contains(&BattleEvent::TechniqueIneffective {
        actor: id(1),
        target: id(2)
    }));
}

#[test]
fn rever_revives_a_human_to_one_quarter_and_regen_to_full_preserving_seal_and_tp() {
    let data = data();
    for (tech, hp, cost) in [(36, 20, 12), (37, 83, 36)] {
        let mut roster = recovery_roster(&data);
        let stats = &mut roster.get_mut(id(2)).unwrap().stats;
        stats.status = status::DEAD
            | status::POISONED
            | status::PARALYZED
            | status::ASLEEP
            | status::TECH_SEALED;
        stats.curr_hp = 0;
        stats.max_hp = 83;
        stats.curr_tp = 7;
        stats.curr_skill_uses[0] = 1;
        let before_tp = stats.max_tp;
        assert!(technique_targets(&roster, id(1), data.technique(tech).unwrap()).contains(&id(2)));
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_technique(
            &mut roster,
            id(1),
            tech,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events,
        );
        let stats = &roster.get(id(2)).unwrap().stats;
        assert_eq!(
            (
                stats.curr_hp,
                stats.status,
                stats.curr_tp,
                stats.max_tp,
                stats.curr_skill_uses[0]
            ),
            (hp, status::TECH_SEALED, 7, before_tp, 1)
        );
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 100 - cost);
        assert_eq!(rolls.drawn(), 0);
        assert!(events.contains(&BattleEvent::Revived {
            actor: id(1),
            target: id(2),
            remaining_hp: hp
        }));
    }
}

#[test]
fn regen_heals_and_cures_a_living_human_but_rever_is_a_paid_no_effect() {
    let data = data();
    let mut roster = recovery_roster(&data);
    let stats = &mut roster.get_mut(id(2)).unwrap().stats;
    stats.curr_hp = 7;
    stats.status = status::POISONED | status::PARALYZED | status::ASLEEP | status::TECH_SEALED;
    stats.agility.battle = 200;
    let before = stats.clone();
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        36,
        Some(id(2)),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    );
    assert_eq!(roster.get(id(2)).unwrap().stats, before);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 88);
    assert!(events.contains(&BattleEvent::TechniqueIneffective {
        actor: id(1),
        target: id(2)
    }));
    events.clear();
    resolve_technique(
        &mut roster,
        id(1),
        37,
        Some(id(2)),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    );
    let stats = &roster.get(id(2)).unwrap().stats;
    assert_eq!(
        (stats.curr_hp, stats.status),
        (stats.max_hp, status::TECH_SEALED)
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::Revived { .. }))
    );
}

#[test]
fn recovery_techniques_refuse_androids_before_payment_and_skip_dead_cure_targets() {
    let data = data();
    let mut roster = recovery_roster(&data);
    roster.get_mut(id(3)).unwrap().stats.profession = super::super::PROFESSION_ANDROID;
    for tech in 34..=37 {
        assert!(!technique_targets(&roster, id(1), data.technique(tech).unwrap()).contains(&id(3)));
        let before = roster.clone();
        let mut events = Vec::new();
        resolve_technique(
            &mut roster,
            id(1),
            tech,
            Some(id(3)),
            &data,
            &mut SliceRolls::new(&[0]),
            &mut events,
        );
        assert_eq!(roster, before);
        assert!(matches!(
            events[0],
            BattleEvent::TechniqueRejected {
                reason: TechniqueRejection::InvalidTarget,
                ..
            }
        ));
    }
    let stats = &mut roster.get_mut(id(2)).unwrap().stats;
    stats.curr_hp = 0;
    stats.status = status::DEAD | status::POISONED;
    let before = stats.clone();
    let mut events = Vec::new();
    resolve_technique(
        &mut roster,
        id(1),
        34,
        Some(id(2)),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    );
    assert_eq!(roster.get(id(2)).unwrap().stats, before);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 98);
    assert_eq!(events.len(), 1);
}

#[test]
fn rimit_uses_psychic_chances_in_order_without_damage_or_agility_reduction() {
    let data = data();
    let mut roster = roster(&data);
    let caster = &mut roster.get_mut(id(1)).unwrap().stats;
    caster.techniques[0] = 23;
    caster.curr_tp = 30;
    caster.mental.battle = 20;
    for target in [6, 7] {
        let stats = &mut roster.get_mut(id(target)).unwrap().stats;
        stats.mental.battle = 20;
        stats.agility.battle = 77;
        stats.element_props[10] = 2;
    }
    let mut rolls = SliceRolls::new(&[24, 25]);
    let mut events = Vec::new();
    assert!(
        resolve_technique(&mut roster, id(1), 23, None, &data, &mut rolls, &mut events).is_empty()
    );
    assert_eq!(rolls.drawn(), 2);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 20);
    assert_eq!(
        roster.get(id(6)).unwrap().stats.status,
        0,
        "equality misses"
    );
    assert_eq!(roster.get(id(7)).unwrap().stats.status, status::ASLEEP);
    for target in [6, 7] {
        let stats = &roster.get(id(target)).unwrap().stats;
        assert_eq!((stats.curr_hp, stats.agility.battle), (25, 77));
    }
    assert!(events.contains(&BattleEvent::FellAsleep {
        actor: id(1),
        target: id(7)
    }));
}

#[test]
fn rimit_skips_existing_sleep_or_paralysis_without_rolling_but_rolls_for_immunity() {
    for condition in [status::ASLEEP, status::PARALYZED] {
        let data = data();
        let mut roster = roster(&data);
        let caster = &mut roster.get_mut(id(1)).unwrap().stats;
        caster.techniques[0] = 23;
        caster.curr_tp = 30;
        caster.mental.battle = 100;
        roster.get_mut(id(6)).unwrap().stats.status = condition;
        roster.get_mut(id(7)).unwrap().stats.element_props[10] = 0;
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_technique(&mut roster, id(1), 23, None, &data, &mut rolls, &mut events);
        assert_eq!(rolls.drawn(), 1);
        assert_eq!(roster.get(id(6)).unwrap().stats.status, condition);
        assert_eq!(roster.get(id(7)).unwrap().stats.status, 0);
        assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 20);
    }
}
