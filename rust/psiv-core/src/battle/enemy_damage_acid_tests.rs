//! `$33` ACIDBREATH: its four carriers, through [`resolve_damage_skill`] and
//! through `roll_enemy_ability`.
//!
//! The Acid Breath tests below moved here verbatim from `enemy_skill_tests.rs`
//! when the resolver moved out of `enemy_skill`: only the resolver's name
//! changed, from `resolve_acid_breath` to `resolve_damage_skill`. Their
//! fixtures (`acid_data`, `acid_carrier_data`, `acid_carrier_roster`,
//! `acid_roster`, `acid_carrier_round`) moved with them. One of their controls
//! did change with the gate: `acid_invalid_definition_or_dispatcher...` used to
//! move record byte 2 for its "invalid definition" case, and now moves the
//! effect byte, because byte 2's target nibble is no longer a gate at all.

use super::tests::*;
use super::*;
use crate::battle::enemy_skill::resolve_fission;
use crate::battle::{
    Battle, Command, EnemySkill, FormationEnemy, FormationRecord, PartyMember, RoundOrders,
    SliceRolls, fixtures, status,
};

fn acid_data() -> BattleData {
    acid_carrier_data(75, 1)
}

/// The one shared `$33` record, on whichever carrier the caller is testing.
/// Strength is per-carrier because `Enemy_DamageCharacter` reads it from the
/// acting enemy's stats, not from the record.
pub(super) fn acid_carrier_data(enemy_id: u16, strength: u8) -> BattleData {
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

pub(super) fn acid_carrier_roster(data: &BattleData, enemy_id: u16) -> Roster {
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
        assert!(resolve_damage_skill(
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
        resolve_damage_skill(
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
            // The invalid definition: the record's effect byte is not `$01`
            // (`AbilityEffect_None`), so the one damage request this resolver
            // models would not be the record's whole effect. Record byte 2's
            // target nibble is not a gate — see
            // `a_listed_route_resolves_with_its_record_on_the_nibble_9_target`.
            skill.effect = 2;
        }
        let data = data.clone().with_enemy_skills([skill]);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(!resolve_damage_skill(
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
            resolve_damage_skill(
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
            !resolve_damage_skill(
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
