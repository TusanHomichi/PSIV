use super::*;
use crate::battle::{
    Battle, Command, FormationEnemy, FormationRecord, Outcome, PartyMember, RoundOrders, Skipped,
    SliceRolls, fixtures, status,
};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}
fn data() -> BattleData {
    let mut boss = fixtures::zoran_bult();
    boss.id = 12;
    boss.name = "IGGLANOVA".into();
    boss.hp = 300;
    boss.agility = 100;
    boss.condition_ids = [1; 4];
    boss.conditional_abilities = [6; 4];
    let mut guil = boss.clone();
    guil.id = 13;
    guil.conditional_abilities = [7; 4];
    let mut minion = fixtures::zoran_bult();
    minion.id = 11;
    minion.name = "GICEFALGUE".into();
    minion.hp = 40;
    fixtures::data()
        .with_enemies([boss, guil, minion])
        .with_enemy_skills([6, 7].map(|id| EnemySkill {
            id,
            name: "FISSION".into(),
            effect: 30,
            power_stat: 0,
            target: id + 3,
            power: 0,
            resistance: 0,
            element: 0,
        }))
}
fn roster(data: &BattleData, boss: u16) -> Roster {
    let mut r = Roster::new();
    for (slot, enemy) in [(1, 11), (2, boss), (3, 11)] {
        r.add_enemy(slot, data.enemy(enemy).unwrap());
    }
    r
}
fn kill(r: &mut Roster, n: u8) {
    let f = r.get_mut(id(n)).unwrap();
    f.stats.curr_hp = 0;
    f.stats.status = status::DEAD;
}

#[test]
fn fission_prefers_the_only_empty_neighbor_without_a_draw() {
    let data = data();
    for side in [6, 8] {
        let mut r = roster(&data, 12);
        kill(&mut r, side);
        let mut rolls = SliceRolls::new(&[0]);
        let mut ability = 0;
        assert_eq!(
            fission_neighbor(&r, id(7), data.enemy(12).unwrap(), &mut ability, &mut rolls),
            Some(id(side))
        );
        assert_eq!((ability, rolls.drawn()), (6, 0));
    }
}

#[test]
fn two_empty_sides_draw_once_even_left_odd_right() {
    let data = data();
    let mut r = roster(&data, 12);
    kill(&mut r, 6);
    kill(&mut r, 8);
    for (draw, expected) in [(0, 6), (1, 8), (0xFFFE, 6), (0xFFFF, 8)] {
        let draws = [draw];
        let mut rolls = SliceRolls::new(&draws);
        let mut ability = 0;
        assert_eq!(
            fission_neighbor(&r, id(7), data.enemy(12).unwrap(), &mut ability, &mut rolls),
            Some(id(expected))
        );
        assert_eq!((ability, rolls.drawn()), (6, 1));
    }
}

#[test]
fn living_sleeping_neighbors_and_terminated_conditions_do_not_spawn() {
    let data = data();
    let mut r = roster(&data, 12);
    r.get_mut(id(6)).unwrap().stats.status = status::ASLEEP;
    let mut rolls = SliceRolls::new(&[0]);
    let mut ability = 0;
    assert_eq!(
        fission_neighbor(&r, id(7), data.enemy(12).unwrap(), &mut ability, &mut rolls),
        None
    );
    assert_eq!(rolls.drawn(), 0);
    kill(&mut r, 6);
    let mut record = data.enemy(12).unwrap().clone();
    record.condition_ids = [0, 1, 1, 1];
    assert_eq!(
        fission_neighbor(&r, id(7), &record, &mut ability, &mut rolls),
        None
    );
    assert_eq!((ability, rolls.drawn()), (0, 0));
}

#[test]
fn refill_uses_cached_neighbor_identity_and_resets_all_enemy_stats() {
    let data = data();
    let mut r = roster(&data, 13);
    kill(&mut r, 8);
    let old = r.get_mut(id(8)).unwrap();
    old.stats.agility.battle = 1;
    old.stats.element_props = [0; 14];
    old.ability = 99;
    let mut events = Vec::new();
    assert!(resolve_fission(&mut r, id(7), 7, id(8), &data, &mut events).unwrap());
    assert_eq!(
        r.get(id(8)).unwrap().stats,
        Stats::from_enemy(data.enemy(11).unwrap())
    );
    assert_eq!(r.get(id(8)).unwrap().ability, 0);
    assert!(events.contains(&BattleEvent::EnemyReplenished {
        actor: id(7),
        fighter: id(8),
        enemy_id: 11,
        name: "GICEFALGUE".into(),
        hp: 40
    }));
    assert!(
        !resolve_fission(&mut r, id(7), 7, id(8), &data, &mut events).unwrap(),
        "never replace a living neighbor"
    );
}

#[test]
fn an_unsupported_fission_definition_cannot_mutate_the_roster() {
    let data = data();
    let mut r = roster(&data, 12);
    kill(&mut r, 6);
    let mut skill = data.enemy_skill(6).unwrap().clone();
    skill.effect = 37;
    let data = data.with_enemy_skills([skill]);
    let before = r.clone();
    assert!(!resolve_fission(&mut r, id(7), 6, id(6), &data, &mut Vec::new()).unwrap());
    assert_eq!(r, before);
}

#[test]
fn a_killed_and_replaced_queued_enemy_waits_until_the_next_round() {
    let data = data();
    let mut member = PartyMember::seat(&fixtures::chaz(), &data).unwrap();
    member.stats.agility.battle = 255;
    member.stats.dexterity.battle = 255;
    member.stats.attack.battle = 999;
    member.stats.curr_hp = 999;
    member.stats.max_hp = 999;
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: [(1, 11), (2, 12), (3, 11)]
            .map(|(slot, enemy_id)| FormationEnemy {
                slot,
                enemy_id,
                position: 20,
            })
            .to_vec(),
    };
    let mut rolls = SliceRolls::new(&[0, 1, 2, 3, 4, 5, 6, 7]);
    let (mut battle, _) = Battle::start(&formation, vec![member], &data, true, &mut rolls).unwrap();
    assert_eq!(
        battle
            .roster()
            .living(Side::Enemy)
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![id(7)]
    );
    for _ in 0..2 {
        battle
            .round(
                &RoundOrders::Commands(vec![Command::Defend]),
                &data,
                &mut rolls,
            )
            .unwrap();
    }
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::AttackTarget(id(6))]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::Died { fighter: id(6) }));
    assert!(
        events.iter().any(
            |e| matches!(e, BattleEvent::EnemyReplenished { fighter, .. } if *fighter == id(6))
        )
    );
    assert!(events.contains(&BattleEvent::TurnSkipped {
        actor: id(6),
        reason: Skipped::JustRevived
    }));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(7))),
        "Fission consumes Igglanova's turn"
    );
    let next = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(
        next.iter()
            .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6)))
    );
    battle
        .round(
            &RoundOrders::Commands(vec![Command::AttackTarget(id(7))]),
            &data,
            &mut rolls,
        )
        .unwrap();
    battle
        .round(
            &RoundOrders::Commands(vec![Command::AttackTarget(id(6))]),
            &data,
            &mut rolls,
        )
        .unwrap();
    let won = battle
        .round(
            &RoundOrders::Commands(vec![Command::AttackTarget(id(8))]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    assert!(
        won.iter().any(|e| matches!(
            e,
            BattleEvent::Rewarded {
                experience_total: 48,
                meseta: 12,
                ..
            }
        )),
        "each killed incarnation contributes its reward"
    );
}

#[test]
fn dormant_neighbors_keep_stats_but_are_absent_from_targets_and_turns() {
    let data = data();
    let mut r = roster(&data, 12);
    initialize_enemies(&mut r);
    assert_eq!(
        r.living(Side::Enemy).map(|f| f.id).collect::<Vec<_>>(),
        vec![id(7)]
    );
    let neighbor = r.get(id(6)).unwrap();
    assert!(!neighbor.active);
    assert_eq!((neighbor.stats.curr_hp, neighbor.stats.status), (40, 0));
    let member = PartyMember::seat(&fixtures::alys(), &data).unwrap();
    r.add_party_member(member.character, member.name, member.stats);
    let mut rolls = SliceRolls::new(&[0]);
    let queue = crate::battle::build_queue(&r, crate::battle::Priority::Normal, &mut rolls);
    assert!(!queue.iter().any(|e| [id(6), id(8)].contains(&e.fighter)));
    let mut events = Vec::new();
    crate::battle::resolve_attack(&mut r, id(1), None, &data, &mut rolls, &mut events).unwrap();
    assert!(events.contains(&BattleEvent::Attacked {
        actor: id(1),
        targets: vec![id(7)]
    }));
}

fn acid_data() -> BattleData {
    acid_carrier_data(75, 1)
}

/// The one shared `$33` record, on whichever carrier the caller is testing.
/// Strength is per-carrier because `Enemy_DamageCharacter` reads it from the
/// acting enemy's stats, not from the record.
fn acid_carrier_data(enemy_id: u16, strength: u8) -> BattleData {
    let mut plant = fixtures::zoran_bult();
    plant.id = enemy_id;
    plant.name = format!("CARRIER-{enemy_id}");
    plant.hp = 32;
    plant.strength = strength;
    plant.attack = 22;
    plant.agility = 100;
    plant.regular_abilities = [51; 8];
    plant.condition_ids = [0; 4];
    fixtures::data()
        .with_enemies([plant])
        .with_enemy_skills([EnemySkill {
            id: 51,
            name: "ACIDBREATH".into(),
            effect: 1,
            power_stat: 1,
            target: 8,
            power: 24,
            resistance: 6,
            element: 1,
        }])
}

fn acid_carrier_roster(data: &BattleData, enemy_id: u16) -> Roster {
    let mut r = Roster::new();
    for record in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()] {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 100;
        member.stats.max_hp = 100;
        member.stats.defence.battle = 7;
        member.stats.agility.battle = 255;
        r.add_party_member(member.character, member.name, member.stats);
    }
    r.add_enemy(1, data.enemy(enemy_id).unwrap());
    r
}

fn acid_roster(data: &BattleData) -> Roster {
    acid_carrier_roster(data, 75)
}

#[test]
fn acid_breath_uses_strength_and_one_damage_roll_even_against_high_agility() {
    let data = acid_data();
    for (defending, expected) in [(false, 17), (true, 5)] {
        let mut r = acid_roster(&data);
        if defending {
            r.get_mut(id(2)).unwrap().stats.begin_defending();
        }
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(resolve_acid_breath(
            &mut r,
            id(6),
            51,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(
            rolls.drawn(),
            16,
            "no accuracy roll and no second animation hit"
        );
        assert_eq!(events.len(), 2, "ability and damage; no physical follow-up");
        assert_eq!(
            events[1],
            BattleEvent::Resolved {
                actor: id(6),
                target: id(2),
                verdict: crate::battle::Verdict::Normal,
                damage: Some(expected),
                remaining_hp: 100 - expected,
            }
        );
        assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 100);
        assert_eq!(r.get(id(3)).unwrap().stats.curr_hp, 100);
    }
}

#[test]
fn acid_death_clears_ailments_preserves_seal_and_shuts_down_androids() {
    let data = acid_data();
    for (profession, death) in [(0, status::DEAD), (5, status::ANDROID_DEAD)] {
        let mut r = acid_roster(&data);
        let stats = &mut r.get_mut(id(1)).unwrap().stats;
        stats.profession = profession;
        stats.curr_hp = 1;
        stats.status = status::POISONED | status::PARALYZED | status::ASLEEP | status::TECH_SEALED;
        let mut events = Vec::new();
        resolve_acid_breath(
            &mut r,
            id(6),
            51,
            Some(id(1)),
            &data,
            &mut SliceRolls::new(&[0]),
            &mut events,
        );
        assert_eq!(
            r.get(id(1)).unwrap().stats.status,
            death | status::TECH_SEALED
        );
        assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 0);
        assert!(events.contains(&BattleEvent::Died { fighter: id(1) }));
    }
}

#[test]
fn acid_invalid_definition_or_dispatcher_cannot_silently_damage_or_spawn() {
    let data = acid_data();
    for change_enemy in [false, true] {
        let mut r = acid_roster(&data);
        let mut skill = data.enemy_skill(51).unwrap().clone();
        if change_enemy {
            r.get_mut(id(6)).unwrap().stats.enemy_id = 9;
        } else {
            skill.target = 9;
        }
        let data = data.clone().with_enemy_skills([skill]);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(!resolve_acid_breath(
            &mut r,
            id(6),
            51,
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(r, before);
        assert_eq!(rolls.drawn(), 0);
        assert!(events.is_empty());
    }
    let mut r = acid_roster(&data);
    kill(&mut r, 6);
    assert!(!resolve_fission(&mut r, id(6), 51, id(6), &data, &mut Vec::new()).unwrap());
}

/// One round against a single carrier, driven through `roll_enemy_ability` on a
/// fully specified draw stream: nine ordering draws, the four enemy-target
/// draws, the ability index (0, and every slot holds `$33`) and the 16 damage
/// draws — 30 in all when `$33` resolves, plus the fallback attack's accuracy
/// roll when it does not. Party agility 1 keeps the enemy first in the queue,
/// so Defend has not raised anyone's physical resistance yet and the numbers
/// below are the undefended ones.
fn acid_carrier_round(carrier: u16, strength: u8) -> (Vec<BattleEvent>, usize) {
    let data = acid_carrier_data(carrier, strength);
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id: carrier,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.defence.battle = 7;
        member.stats.agility.battle = 1;
        member
    });
    let (mut battle, _) = Battle::start(
        &formation,
        party.to_vec(),
        &data,
        false,
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    let mut stream = vec![0; 13];
    stream.push(0);
    stream.extend([0; 16]);
    let mut rolls = SliceRolls::new(&stream);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend; 3]),
            &data,
            &mut rolls,
        )
        .unwrap();
    (events, rolls.drawn())
}

#[test]
fn every_proven_carrier_resolves_acid_breath_through_its_own_routine() {
    // `EnemyAttackOffs` (`ps4.asm:19206`): `$4B`/`$4C` = EnemyAttack_FlattrPlnt
    // for 75 FlattrPlnt and 76 FlyScreamr, `$55`/`$56` = EnemyAttack_Piercer
    // (`ps4.asm:21518`) for 85 Piercer and 86 HakenLeft. The expected number is
    // `Battle_CalculateDamage` (`ps4.asm:17374`) on `EnemySkillData` `$33` as
    // `Enemy_DamageCharacter` reads it: the caster's own strength as the power,
    // the target's modified defense and its physical resistance, with 16 zero
    // draws and the shared fixture's defense of 7 —
    // `(((8*str)>>6) + str + 48) * 2 >> 2 - 7`.
    for (carrier, strength, expected) in [
        (75u16, 1u8, 17u16),
        (76, 88, 66),
        (85, 128, 89),
        (86, 164, 109),
    ] {
        let data = acid_carrier_data(carrier, strength);
        let mut r = acid_carrier_roster(&data, carrier);
        // Raised off the 100 the shared fixture uses: HakenLeft's strength of
        // 164 deals 109, and a death would add a third event to the pair this
        // test pins.
        for member in [id(1), id(2), id(3)] {
            let stats = &mut r.get_mut(member).unwrap().stats;
            stats.curr_hp = 200;
            stats.max_hp = 200;
        }
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            resolve_acid_breath(
                &mut r,
                id(6),
                51,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "carrier {carrier} owns a traced `$33` arm"
        );
        assert_eq!(rolls.drawn(), 16, "carrier {carrier}: one damage request");
        assert_eq!(
            events.len(),
            2,
            "carrier {carrier}: ability and damage only"
        );
        assert_eq!(
            events[1],
            BattleEvent::Resolved {
                actor: id(6),
                target: id(2),
                verdict: crate::battle::Verdict::Normal,
                damage: Some(expected),
                remaining_hp: 200 - expected,
            },
            "carrier {carrier}"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
            )),
            "carrier {carrier}: no physical swing and no unsupported notice"
        );
        assert_eq!(r.get(id(1)).unwrap().stats.curr_hp, 200);
        assert_eq!(r.get(id(3)).unwrap().stats.curr_hp, 200);
    }
}

#[test]
fn every_proven_carrier_dispatches_acid_breath_without_a_physical_swing() {
    for (carrier, strength, expected) in [(76u16, 88u8, 66u16), (85, 128, 89), (86, 164, 109)] {
        let (events, drawn) = acid_carrier_round(carrier, strength);
        assert_eq!(drawn, 30, "carrier {carrier}: one damage request, no swing");
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::EnemySkillUsed { actor, skill: 51, name }
                    if *actor == id(6) && name == "ACIDBREATH"
            )),
            "carrier {carrier}: {:?}",
            events
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::Resolved { actor, damage: Some(damage), remaining_hp, .. }
                    if *actor == id(6)
                        && *damage == expected
                        && *remaining_hp == 400 - expected
            )),
            "carrier {carrier}: {:?}",
            events
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
            )),
            "carrier {carrier}: dispatched, so nothing falls back: {:?}",
            events
        );
    }
}

#[test]
fn an_unproven_carrier_still_reports_acid_breath_as_unsupported() {
    // 77 TechPlant shares EnemyAttack_FlattrPlnt but rolls only `$2A`/`$2E`, so
    // its `$33` arm is unproven; 10 ZoranBult is outside both routines.
    for carrier in [77u16, 10u16] {
        let data = acid_carrier_data(carrier, 88);
        let mut r = acid_carrier_roster(&data, carrier);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            !resolve_acid_breath(
                &mut r,
                id(6),
                51,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "carrier {carrier} is outside the proven set"
        );
        assert_eq!(r, before, "carrier {carrier}");
        assert_eq!(rolls.drawn(), 0, "carrier {carrier}");
        assert!(events.is_empty(), "carrier {carrier}");

        let (events, drawn) = acid_carrier_round(carrier, 88);
        assert_eq!(
            drawn, 31,
            "carrier {carrier}: the fallback attack adds its accuracy roll"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { actor, ability: 51 } if *actor == id(6)
            )),
            "carrier {carrier}: {:?}",
            events
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))),
            "carrier {carrier}: the ordinary attack path is the fallback: {:?}",
            events
        );
    }
}

#[test]
fn a_second_enemy_redraws_a_dead_target_before_its_ability_roll() {
    let data = acid_data();
    let mut party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 100;
        member.stats.max_hp = 100;
        member.stats.agility.battle = 1;
        member.stats.defence.battle = 0;
        member
    });
    party[0].stats.curr_hp = 1;
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: (1..=2)
            .map(|slot| FormationEnemy {
                slot,
                enemy_id: 75,
                position: 20,
            })
            .collect(),
    };
    let (mut battle, _) = Battle::start(
        &formation,
        party.to_vec(),
        &data,
        true,
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    // Nine ordering draws, four preselected targets (all first party slot),
    // first ability, 16 damage draws, weighted redraw selecting last survivor,
    // second ability, 16 damage draws. This also pins the redraw's position.
    let mut stream = vec![0; 9];
    stream.extend([255; 4]);
    stream.push(1);
    stream.extend([0; 16]);
    stream.extend([0, 2]);
    stream.extend([0; 16]);
    let mut rolls = SliceRolls::new(&stream);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend; 3]),
            &data,
            &mut rolls,
        )
        .unwrap();
    let hits: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved { actor, target, .. } => Some((actor.get(), target.get())),
            _ => None,
        })
        .collect();
    assert_eq!(hits, [(6, 1), (7, 3)]);
    assert_eq!(rolls.drawn(), 48);
    assert!(!events.iter().any(|e| matches!(
        e,
        BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
    )));
}

fn thread_data() -> BattleData {
    let mut crawler = fixtures::zoran_bult();
    crawler.id = 31;
    crawler.strength = 20;
    fixtures::data()
        .with_enemies([crawler])
        .with_enemy_skills([EnemySkill {
            id: 16,
            name: "THREAD".into(),
            effect: 6,
            power_stat: 1,
            target: 8,
            power: 64,
            resistance: 3,
            element: 1,
        }])
}

#[test]
fn thread_spends_one_chance_roll_and_never_deals_physical_damage() {
    let data = thread_data();
    for (roll, expected_agility) in [(32, 20), (33, 30)] {
        let mut r = Roster::new();
        let member = PartyMember::seat(&fixtures::chaz(), &data).unwrap();
        r.add_party_member(member.character, member.name, member.stats);
        r.add_enemy(1, data.enemy(31).unwrap());
        let stats = &mut r.get_mut(id(1)).unwrap().stats;
        stats.agility.modified = 50;
        stats.agility.battle = 20;
        stats.element_props[0] = 2;
        let hp = stats.curr_hp;
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        assert!(resolve_thread(
            &mut r,
            id(6),
            16,
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events
        ));
        let stats = &r.get(id(1)).unwrap().stats;
        assert_eq!(
            (stats.curr_hp, stats.status, stats.agility.battle),
            (hp, 0, expected_agility)
        );
        assert_eq!(rolls.drawn(), 1);
        assert!(!events.iter().any(|e| matches!(
            e,
            BattleEvent::Attacked { .. }
                | BattleEvent::Resolved {
                    damage: Some(_),
                    ..
                }
        )));
    }
}

#[test]
fn thread_uses_modified_agility_each_time_and_floors_it_at_one() {
    let data = thread_data();
    for modified in [10, 20, 50] {
        let mut r = Roster::new();
        let member = PartyMember::seat(&fixtures::chaz(), &data).unwrap();
        r.add_party_member(member.character, member.name, member.stats);
        r.add_enemy(1, data.enemy(31).unwrap());
        let stats = &mut r.get_mut(id(1)).unwrap().stats;
        stats.agility.modified = modified;
        stats.agility.battle = 1;
        stats.element_props[0] = 2;
        let mut rolls = SliceRolls::new(&[63]);
        for _ in 0..2 {
            assert!(resolve_thread(
                &mut r,
                id(6),
                16,
                Some(id(1)),
                &data,
                &mut rolls,
                &mut Vec::new()
            ));
            assert_eq!(
                r.get(id(1)).unwrap().stats.agility.battle,
                modified.saturating_sub(20).max(1)
            );
        }
        assert_eq!(rolls.drawn(), 2);
    }
}

/// The three records these tests need, `generated/enemy_skills.json`: `$07`
/// Fission2 (`1e 00 0a 00 00 00 00 00` at `0x28339C`), `$17` Waiting
/// (`22 00 00 00 00 00 00 00` at `0x28341C`) and `$19` Detonation
/// (`24 05 09 18 06 01 00 00` at `0x28342C`), one of `EnemyAttack_FloatMine`'s
/// arms and the id 45 CommndBall really rolls. The tuple is
/// `(effect, power_stat, target, power, resistance, element)`.
fn record(id: u8, name: &str, bytes: (u8, u8, u8, u8, u8, u8)) -> EnemySkill {
    let (effect, power_stat, target, power, resistance, element) = bytes;
    EnemySkill {
        id,
        name: name.into(),
        effect,
        power_stat,
        target,
        power,
        resistance,
        element,
    }
}

fn fission2_record() -> EnemySkill {
    record(7, "FISSION", (30, 0, 10, 0, 0, 0))
}

fn waiting_record() -> EnemySkill {
    record(23, "WAITING", (34, 0, 0, 0, 0, 0))
}

fn detonation_record() -> EnemySkill {
    record(25, "DETONATION", (36, 5, 9, 24, 6, 1))
}

/// One lone `EnemyAttack_FloatMine` carrier with `slots` in all eight regular
/// entries, agile enough to act first, with the record table the caller names.
fn float_mine_data(enemy_id: u16, slots: [u8; 8], skills: Vec<EnemySkill>) -> BattleData {
    let mut carrier = fixtures::zoran_bult();
    carrier.id = enemy_id;
    carrier.hp = 300;
    carrier.agility = 100;
    carrier.regular_abilities = slots;
    carrier.condition_ids = [0; 4];
    fixtures::data()
        .with_enemies([carrier])
        .with_enemy_skills(skills)
}

/// One round against that lone carrier, on a fully specified draw stream: nine
/// ordering draws, the four enemy-target draws, then the ability index, slot 0.
/// Party agility 1 keeps the enemy first in the queue, so the round's roll count
/// is the assertion; 400 party HP keeps a fallback swing from ending it early.
fn float_mine_round(
    enemy_id: u16,
    slots: [u8; 8],
    skills: Vec<EnemySkill>,
) -> (Battle, Vec<BattleEvent>, usize) {
    let data = float_mine_data(enemy_id, slots, skills);
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.defence.battle = 7;
        member.stats.agility.battle = 1;
        member
    });
    let (mut battle, _) = Battle::start(
        &formation,
        party.to_vec(),
        &data,
        false,
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    let mut stream = vec![0; 13];
    stream.push(0);
    stream.extend([0; 40]);
    let mut rolls = SliceRolls::new(&stream);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend; 3]),
            &data,
            &mut rolls,
        )
        .unwrap();
    (battle, events, rolls.drawn())
}

/// `EnemyAttack_FloatMine` (`ps4.asm:22675`) has an arm for `$14` Warning, `$18`
/// Explosion, `$19` Detonation and `$1A` CyanicBomb only, so FloatMine2's whole
/// regular list (`$07`/`$17`) and FloatMine's and VopalSphre's (`$17`) reach the
/// fall-through `loc_10406` (`ps4.asm:22781`): it loads no object, clears
/// `Current_Target_Index` and `$24(a4)` and sets `Battle_Routine` `$16`. Every
/// `Fighters_Hit_Flags` entry stays `$FF`, so `Battle_DoAttackEffect`
/// (`ps4.asm:8553`) never reaches `Ability_GetEffectAndRange`: no object, no
/// sound, no damage, no status. The actor has acted and nothing happened.
#[test]
fn float_mine_carriers_spend_the_roll_without_an_effect_or_a_swing() {
    for (carrier, slots, ability, name) in [
        (50u16, [7u8; 8], 7u8, "FISSION"),
        (44, [23; 8], 23, "WAITING"),
        (46, [23; 8], 23, "WAITING"),
    ] {
        let (battle, events, drawn) =
            float_mine_round(carrier, slots, vec![fission2_record(), waiting_record()]);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::EnemyAbilityWasted { .. }))
                .count(),
            1,
            "carrier {carrier}: {events:?}"
        );
        assert!(
            events.contains(&BattleEvent::EnemyAbilityWasted {
                actor: id(6),
                ability,
                name: name.into(),
            }),
            "carrier {carrier}: {events:?}"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { .. }
                    | BattleEvent::Attacked { .. }
                    | BattleEvent::Resolved { .. }
                    | BattleEvent::TurnSkipped { .. }
            )),
            "carrier {carrier}: the turn is spent, neither swung nor skipped: {events:?}"
        );
        assert_eq!(
            drawn, 14,
            "carrier {carrier}: nine ordering draws, four enemy-target draws and \
             the ability index; the cartridge takes no hit roll and no damage draw"
        );
        assert!(
            battle.party_stats().all(|(_, stats)| stats.curr_hp == 400),
            "carrier {carrier}: nobody was touched"
        );
        assert_eq!(
            battle.roster().get(id(6)).unwrap().ability,
            0,
            "carrier {carrier}"
        );
    }
}

/// The negative control: the routine's own `$19` arm on its fourth carrier, and
/// the same `$07` record on an enemy outside every `EnemyAttack_FloatMine`
/// entry, both keep the ordinary fallback — an `UnsupportedAbility` notice, a
/// physical swing, its accuracy roll and the 16 damage draws.
#[test]
fn other_abilities_and_carriers_keep_the_physical_fallback() {
    for (carrier, slots, ability, skills) in [
        (45u16, [25u8; 8], 25u8, vec![detonation_record()]),
        (10, [7; 8], 7, vec![fission2_record()]),
    ] {
        let (_, events, drawn) = float_mine_round(carrier, slots, skills);
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { actor, ability: rolled }
                    if *actor == id(6) && *rolled == ability
            )),
            "carrier {carrier}: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))),
            "carrier {carrier}: the ordinary attack path is the fallback: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::EnemyAbilityWasted { .. })),
            "carrier {carrier}: {events:?}"
        );
        assert_eq!(
            drawn, 31,
            "carrier {carrier}: the fallback adds its accuracy roll and the 16 damage draws"
        );
    }
}

/// The witness itself: the traced record and a carrier of the routine both have
/// to hold, and on a miss it leaves the roster and the event list untouched.
#[test]
fn the_no_effect_witness_needs_the_traced_record_and_carrier() {
    let data = float_mine_data(50, [7; 8], vec![fission2_record(), waiting_record()]);
    let mut r = Roster::new();
    r.add_enemy(1, data.enemy(50).unwrap());
    let mut events = Vec::new();
    assert!(resolve_no_effect_turn(&mut r, id(6), 7, &data, &mut events));
    assert_eq!(
        events,
        vec![BattleEvent::EnemyAbilityWasted {
            actor: id(6),
            ability: 7,
            name: "FISSION".into(),
        }]
    );
    assert_eq!(r.get(id(6)).unwrap().ability, 0);

    // A record whose bytes moved cannot pass for the traced one.
    for skill in [
        EnemySkill {
            target: 9,
            ..fission2_record()
        },
        EnemySkill {
            effect: 30,
            ..waiting_record()
        },
    ] {
        let data = float_mine_data(50, [skill.id; 8], vec![skill.clone()]);
        let mut r = Roster::new();
        r.add_enemy(1, data.enemy(50).unwrap());
        let before = r.clone();
        let mut events = Vec::new();
        assert!(
            !resolve_no_effect_turn(&mut r, id(6), skill.id, &data, &mut events),
            "{skill:?}"
        );
        assert_eq!(r, before, "{skill:?}");
        assert!(events.is_empty(), "{skill:?}");
    }

    // The carrier gate is the enemy's record id, not its slot or its list.
    for (enemy_id, dead) in [(51u16, false), (50, true)] {
        r.get_mut(id(6)).unwrap().stats.enemy_id = enemy_id;
        if dead {
            kill(&mut r, 6);
        }
        assert!(!resolve_no_effect_turn(
            &mut r,
            id(6),
            7,
            &data,
            &mut events
        ));
    }
    assert_eq!(events.len(), 1, "a missed witness adds no event");
}
