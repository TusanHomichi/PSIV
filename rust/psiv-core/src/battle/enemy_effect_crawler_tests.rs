//! The crawler family's two arms - THREAD (`$10`, effect `$06`) and POISON
//! (`$11`, effect `$1B`) - run through the same effect handlers every other
//! status and stat ability uses (`ROUTES`), so this file is where their
//! numbers are pinned: the hit-chance boundary, the skipped draws, the target
//! rules and the engine-level turn.

use super::*;
use crate::battle::{
    Battle, Command, FormationEnemy, FormationRecord, PartyMember, RoundOrders, SliceRolls,
    fixtures, status,
};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

/// `resolve_effect_skill`, read as the "did this arm take the turn" answer the
/// crawler tests ask.
fn resolve_effect(
    r: &mut Roster,
    actor: FighterId,
    ability: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    resolve_effect_skill(r, actor, ability, intended, data, rolls, events) == EffectTurn::Resolved
}

/// Enemy 32 Caterpillr's shape: the crawler family's shared routine carrying
/// record `$11`. Every slot holds the ability so the roll's index cannot
/// change the outcome; the cartridge's own list is
/// `[0, 0, 0, 0, 17, 17, 17, 17]`.
///
/// Both fighting strengths are 20, which puts the boundary at half the
/// record's hit-chance byte: `v = (r + 20 - 20) * 2`, so `v <= $40` misses up
/// to r = 32 and r = 33 lands.
fn poison_data() -> BattleData {
    let mut crawler = fixtures::zoran_bult();
    crawler.id = 32;
    crawler.name = "CATERPILLR".into();
    crawler.strength = 20;
    crawler.regular_abilities = [17; 8];
    fixtures::data()
        .with_enemies([crawler])
        .with_enemy_skills([EnemySkill {
            id: 17,
            name: "POISON".into(),
            effect: 27,
            power_stat: 1,
            target: 8,
            power: 64,
            resistance: 1,
            element: 13,
        }])
}

/// Chaz at the crawler's strength, normal to efess (element 13).
fn poison_member(data: &BattleData) -> PartyMember {
    let mut member = PartyMember::seat(&fixtures::chaz(), data).unwrap();
    member.stats.curr_hp = 100;
    member.stats.max_hp = 100;
    member.stats.strength.battle = 20;
    member.stats.element_props[12] = 2;
    member
}

fn poison_roster(data: &BattleData) -> Roster {
    let mut r = Roster::new();
    let member = poison_member(data);
    r.add_party_member(member.character, member.name, member.stats);
    r.add_enemy(1, data.enemy(32).unwrap());
    r
}

fn one_crawler_formation() -> FormationRecord {
    FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id: 32,
            position: 20,
        }],
    }
}

#[test]
fn poison_lands_at_the_hit_chance_boundary_and_misses_below_it() {
    let data = poison_data();
    for (roll, poisoned) in [(32, false), (33, true)] {
        let mut r = poison_roster(&data);
        let hp = r.get(id(1)).unwrap().stats.curr_hp;
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        assert!(resolve_effect(
            &mut r,
            id(6),
            17,
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(rolls.drawn(), 1, "exactly one chance draw, r = {roll}");
        assert_eq!(
            events[0],
            BattleEvent::EnemySkillUsed {
                actor: id(6),
                skill: 17,
                name: "POISON".into(),
            }
        );
        let stats = &r.get(id(1)).unwrap().stats;
        assert_eq!(stats.status & status::POISONED != 0, poisoned, "r = {roll}");
        assert_eq!(stats.curr_hp, hp, "poison deals no damage");
        if poisoned {
            assert_eq!(
                events[1],
                BattleEvent::StatusInflicted {
                    actor: id(6),
                    target: id(1),
                    status: status::POISONED,
                }
            );
            assert_eq!(events.len(), 2);
        } else {
            assert_eq!(events.len(), 1, "a failed roll adds nothing");
            assert_eq!(stats.status, 0);
        }
    }
}

#[test]
fn an_existing_ailment_skips_the_draw_while_immunity_still_spends_it() {
    for case in ["already poisoned", "immune"] {
        let data = poison_data();
        let mut r = poison_roster(&data);
        let stats = &mut r.get_mut(id(1)).unwrap().stats;
        let hp = stats.curr_hp;
        if case == "already poisoned" {
            stats.status = status::POISONED | status::TECH_SEALED;
        } else {
            // efess immunity: the scale is zero, so every roll is a miss. The
            // draw happens anyway, exactly as AbilityEffect_Poison's caller
            // spends it before the verdict.
            stats.element_props[12] = 0;
        }
        let already = case == "already poisoned";
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        assert!(resolve_effect(
            &mut r,
            id(6),
            17,
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(rolls.drawn(), usize::from(!already), "{case}");
        let stats = &r.get(id(1)).unwrap().stats;
        assert_eq!(
            stats.status & status::POISONED != 0,
            case != "immune",
            "{case}: only the existing bit counts; the effect never lands here"
        );
        assert_eq!(stats.curr_hp, hp);
        assert_eq!(events.len(), 1, "{case}: only the ability is reported");
    }
}

#[test]
fn a_lookalike_record_or_another_carrier_cannot_poison() {
    let data = poison_data();
    // The route is the `(enemy, ability)` pair plus the record's effect byte
    // and range nibble: a record 17 that no longer says `$1B` (poison) or range
    // 8 (the drawn target) is not the traced one.
    let mut other_effect = data.enemy_skill(17).unwrap().clone();
    other_effect.effect = 0x07;
    let mut other_range = data.enemy_skill(17).unwrap().clone();
    other_range.target = 9;
    let with_effect = data.clone().with_enemy_skills([other_effect]);
    let with_range = data.clone().with_enemy_skills([other_range]);

    for (case, skills, enemy_id) in [
        ("an effect that is not poison", &with_effect, 32),
        ("a range that is not the drawn target", &with_range, 32),
        ("an unrelated enemy", &data, 9),
        // 31 CarrionCr shares the routine but its list holds THREAD, not
        // POISON; the pair is not a route.
        ("the sibling carrier", &data, 31),
    ] {
        let mut r = poison_roster(skills);
        r.get_mut(id(6)).unwrap().stats.enemy_id = enemy_id;
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        assert!(
            !resolve_effect(
                &mut r,
                id(6),
                17,
                Some(id(1)),
                skills,
                &mut rolls,
                &mut events
            ),
            "{case}"
        );
        assert_eq!(r, before, "{case}: nothing may change");
        assert_eq!(rolls.drawn(), 0, "{case}");
        assert!(events.is_empty(), "{case}");
    }

    // PoisonMist (`$24`) is a route of its own (57 Mistralgec), not the
    // crawler's: 32 Caterpillr rolling it is outside the table.
    for ability in [0, 16, 36] {
        let mut r = poison_roster(&data);
        assert!(!resolve_effect(
            &mut r,
            id(6),
            ability,
            Some(id(1)),
            &data,
            &mut SliceRolls::new(&[63]),
            &mut Vec::new()
        ));
    }
}

#[test]
fn a_missing_dead_or_enemy_target_consumes_the_ability_without_a_roll() {
    // Ability_ProcessRange skips a slot with the dead bits and applies the
    // effect to Current_Target_Index only; an already-empty or enemy target
    // still costs the crawler its turn, and the roll never happens.
    let data = poison_data();
    for case in ["no target", "dead", "enemy target"] {
        let mut r = poison_roster(&data);
        let target = match case {
            "no target" => None,
            "dead" => {
                let stats = &mut r.get_mut(id(1)).unwrap().stats;
                stats.curr_hp = 0;
                stats.status = status::DEAD;
                Some(id(1))
            }
            _ => Some(id(6)),
        };
        let before = r.get(id(1)).unwrap().stats.clone();
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        assert!(
            resolve_effect(&mut r, id(6), 17, target, &data, &mut rolls, &mut events),
            "{case}"
        );
        assert_eq!(rolls.drawn(), 0, "{case}");
        assert_eq!(
            events,
            vec![BattleEvent::EnemySkillUsed {
                actor: id(6),
                skill: 17,
                name: "POISON".into(),
            }],
            "{case}"
        );
        assert_eq!(
            r.get(id(1)).unwrap().stats.status,
            if case == "dead" { status::DEAD } else { 0 }
        );
        assert_eq!(
            r.get(id(1)).unwrap().stats.curr_hp,
            before.curr_hp,
            "{case}"
        );
    }
}

#[test]
fn the_engine_dispatches_poison_without_a_physical_attack() {
    let data = poison_data();
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &one_crawler_formation(),
        vec![poison_member(&data)],
        &data,
        true,
        &mut rolls,
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::EnemySkillUsed {
        actor: id(6),
        skill: 17,
        name: "POISON".into(),
    }));
    assert!(events.contains(&BattleEvent::StatusInflicted {
        actor: id(6),
        target: id(1),
        status: status::POISONED,
    }));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))),
        "POISON must not also swing: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. })),
        "{events:?}"
    );
    assert_eq!(
        battle.roster().get(id(1)).unwrap().stats.curr_hp,
        100,
        "the crawler's turn deals no damage"
    );
}

#[test]
fn an_unrouted_pair_still_falls_back_to_an_unsupported_attack() {
    // The negative control for the dispatcher: the same enemy rolling an
    // ability id the table does not route for it (36, PoisonMist's record) must
    // keep the explicit diagnostic and the physical fallback rather than
    // poisoning.
    let mut mist = poison_data().enemy_skill(17).unwrap().clone();
    mist.id = 36;
    mist.name = "POISONMIST".into();
    mist.power = 80;
    let mut crawler = fixtures::zoran_bult();
    crawler.id = 32;
    crawler.name = "CATERPILLR".into();
    crawler.strength = 20;
    crawler.regular_abilities = [36; 8];
    let data = fixtures::data()
        .with_enemies([crawler])
        .with_enemy_skills([mist]);
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &one_crawler_formation(),
        vec![poison_member(&data)],
        &data,
        true,
        &mut rolls,
    )
    .unwrap();
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend]),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(events.contains(&BattleEvent::UnsupportedAbility {
        actor: id(6),
        ability: 36,
    }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e,
            BattleEvent::Resolved { actor, damage: Some(damage), .. }
                if *actor == id(6) && *damage > 0)),
        "the unrouted pair falls back to a real physical attack: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::StatusInflicted { .. })),
        "{events:?}"
    );
}

// ---- THREAD ----

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
        assert!(resolve_effect(
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
            assert!(resolve_effect(
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
