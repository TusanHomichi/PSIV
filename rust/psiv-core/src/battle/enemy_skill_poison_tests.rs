use super::*;
use crate::battle::{
    Battle, Command, FormationEnemy, FormationRecord, PartyMember, RoundOrders, SliceRolls,
    fixtures, status,
};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
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
        assert!(resolve_poison(
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
        assert!(resolve_poison(
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
    // PoisonMist (`$24`) carries the same effect byte `$1B` and the same stat
    // selectors; only the hit-chance byte differs, so a dispatcher keyed on the
    // effect alone would wrongly treat it as record 17.
    let mut mist = data.enemy_skill(17).unwrap().clone();
    mist.power = 80;
    let with_mist = data.clone().with_enemy_skills([mist]);

    for (case, skills, enemy_id) in [
        ("PoisonMist's record", &with_mist, 32),
        ("an unrelated enemy", &data, 9),
    ] {
        let mut r = poison_roster(skills);
        r.get_mut(id(6)).unwrap().stats.enemy_id = enemy_id;
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        assert!(
            !resolve_poison(
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

    for ability in [0, 16, 36] {
        let mut r = poison_roster(&data);
        assert!(!resolve_poison(
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
            resolve_poison(&mut r, id(6), 17, target, &data, &mut rolls, &mut events),
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
        0,
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
fn a_lookalike_poison_record_still_falls_back_to_an_unsupported_attack() {
    // The negative control for the dispatcher: the same enemy and the same
    // ability id, carrying a record the port has not proven. It must keep the
    // explicit diagnostic and the physical fallback rather than poisoning.
    let mut mist = poison_data().enemy_skill(17).unwrap().clone();
    mist.power = 80;
    let data = poison_data().with_enemy_skills([mist]);
    let mut rolls = SliceRolls::new(&[63]);
    let (mut battle, _) = Battle::start(
        &one_crawler_formation(),
        vec![poison_member(&data)],
        &data,
        true,
        0,
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
        ability: 17,
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
        "the unproven record falls back to a real physical attack: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::StatusInflicted { .. })),
        "{events:?}"
    );
}
