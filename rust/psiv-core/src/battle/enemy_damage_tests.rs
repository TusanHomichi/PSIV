//! Damage-skill routes: Acid Breath's four carriers, FLAME BOLT's two and the
//! Motavia single-target pairs, through [`resolve_damage_skill`] and through
//! `roll_enemy_ability`.
//!
//! The Acid Breath tests below moved here verbatim from `enemy_skill_tests.rs`
//! when the resolver moved out of `enemy_skill`: only the resolver's name
//! changed, from `resolve_acid_breath` to `resolve_damage_skill`. Their
//! fixtures (`acid_data`, `acid_carrier_data`, `acid_carrier_roster`,
//! `acid_roster`, `acid_carrier_round`) moved with them. One of their controls
//! did change with the gate: `acid_invalid_definition_or_dispatcher...` used to
//! move record byte 2 for its "invalid definition" case, and now moves the
//! effect byte, because byte 2's target nibble is no longer a gate at all.

use super::*;
use crate::battle::enemy_skill::resolve_fission;
use crate::battle::{
    Battle, Command, EnemySkill, FormationEnemy, FormationRecord, PartyMember, RoundOrders,
    SliceRolls, fixtures, status,
};
use crate::battle::{ELEMENT_SLOTS, STAT_INDEX_MASK, technique};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn kill(r: &mut Roster, n: u8) {
    let f = r.get_mut(id(n)).unwrap();
    f.stats.curr_hp = 0;
    f.stats.status = status::DEAD;
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

/// `generated/enemies.json`: 0 Helex is `$02` in all eight regular slots, 5
/// ForcedFly in slots 5-8 only. The fixture hands every slot to whichever
/// carrier is under test, so the ability roll always reaches `$02`.
fn flame_carrier_data(enemy_id: u16, strength: u8) -> BattleData {
    let mut carrier = fixtures::zoran_bult();
    carrier.id = enemy_id;
    carrier.name = format!("CARRIER-{enemy_id}");
    carrier.hp = 135;
    carrier.strength = strength;
    carrier.attack = 204;
    carrier.agility = 100;
    carrier.regular_abilities = [2; 8];
    carrier.condition_ids = [0; 4];
    fixtures::data()
        .with_enemies([carrier])
        .with_enemy_skills([EnemySkill {
            id: 2,
            name: "FLAME BOLT".into(),
            effect: 1,
            power_stat: 1,
            target: 8,
            power: 80,
            resistance: 7,
            element: 3,
        }])
}

fn flame_data() -> BattleData {
    flame_carrier_data(0, 10)
}

/// Three identical party members: every slot carries fire property 2 (normal)
/// and no magic defense, so a hit lands the same number whichever target the
/// engine draws. A test about the target's own record changes one of them
/// before resolving.
fn flame_carrier_roster(data: &BattleData, enemy_id: u16) -> Roster {
    let mut r = Roster::new();
    for record in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()] {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 255;
        member.stats.element_props[2] = 2;
        member.stats.mental_defence.battle = 0;
        r.add_party_member(member.character, member.name, member.stats);
    }
    r.add_enemy(1, data.enemy(enemy_id).unwrap());
    r
}

fn flame_roster(data: &BattleData) -> Roster {
    flame_carrier_roster(data, 0)
}

/// `Battle_CalculateDamage` (`ps4.asm:17374`) on record `$02` as
/// `Enemy_DamageCharacter` (`ps4.asm:3775`) reads it: the caster's own strength
/// as the power, the target's magic defense as the resistance, the target's
/// fire property as the element factor, the record's power byte 80 as the
/// doubled bonus, and 16 zero draws:
/// `(((8*str)>>6) + str + 160) * el >> 2 - mdef`.
fn flame_damage(strength: u16, magic_defence: u16, fire: u8) -> u16 {
    let t = ((8 * strength) >> 6) + strength + 160;
    let after_element = (t * u16::from(fire)) >> 2;
    crate::battle::clamp_damage(after_element as i16 - magic_defence as i16)
}

#[test]
fn flame_bolt_resolves_for_every_proven_carrier_through_the_record() {
    // Both carriers, through the shared record: the numbers come from
    // `EnemySkillData` `$02` (`01 01 08 50 07 03 00 00` at `$283374`) and the
    // acting enemy's strength, exactly as `Enemy_DamageCharacter`'s
    // enemy-skill branch reads them.
    for (carrier, strength, expected) in [(0u16, 10u8, 85u16), (5, 154, 166)] {
        let data = flame_carrier_data(carrier, strength);
        let mut r = flame_carrier_roster(&data, carrier);
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            resolve_damage_skill(
                &mut r,
                id(6),
                2,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "carrier {carrier} owns a traced `$02` arm"
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
                remaining_hp: 400 - expected,
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
        assert_eq!(
            r.get(id(1)).unwrap().stats.curr_hp,
            400,
            "carrier {carrier}"
        );
        assert_eq!(
            r.get(id(3)).unwrap().stats.curr_hp,
            400,
            "carrier {carrier}"
        );
    }
}

#[test]
fn flame_bolt_follows_fire_resistance_and_magic_defense_not_defending() {
    let data = flame_data();
    // Same carrier, same draw stream: only the target's own record changes.
    // The party fixture's fire property is 2 (normal) and its modified magic
    // defense is what `$07` selects.
    for (fire, magic_defence, defending, expected) in [
        (2u8, 0u16, false, 85u16),
        (1, 0, false, 42),
        (2, 40, false, 45),
        (0, 0, false, 1),
        (2, 0, true, 85),
    ] {
        let mut r = flame_roster(&data);
        let stats = &mut r.get_mut(id(2)).unwrap().stats;
        stats.curr_hp = 400;
        stats.max_hp = 400;
        stats.element_props[2] = fire;
        stats.mental_defence.battle = magic_defence;
        if defending {
            stats.begin_defending();
        }
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(resolve_damage_skill(
            &mut r,
            id(6),
            2,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(
            events[1],
            BattleEvent::Resolved {
                actor: id(6),
                target: id(2),
                verdict: crate::battle::Verdict::Normal,
                damage: Some(expected),
                remaining_hp: 400 - expected,
            },
            "fire {fire}, magic defense {magic_defence}, defending {defending}"
        );
        assert_eq!(
            expected,
            flame_damage(10, magic_defence, fire),
            "the formula's own terms"
        );
    }
}

/// One round against a single carrier, driven through `roll_enemy_ability` on
/// a fully specified draw stream: nine ordering draws, the four enemy-target
/// draws, the ability index (0, and every slot holds `$02`) and the 16 damage
/// draws — 30 in all when `$02` resolves, plus the fallback attack's accuracy
/// roll when it does not. Party agility 1 keeps the enemy first in the queue,
/// so Defend has raised nobody's resistance yet.
fn flame_carrier_round(carrier: u16, strength: u8) -> (Vec<BattleEvent>, usize) {
    let data = flame_carrier_data(carrier, strength);
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
        member.stats.agility.battle = 1;
        // Identical targets: fire 2 and no magic defense, so the numbers below
        // do not depend on which party slot the target draws pick.
        member.stats.element_props[2] = 2;
        member.stats.mental_defence.battle = 0;
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
fn flame_bolt_replaces_the_swing_in_an_ordinary_round() {
    let (events, drawn) = flame_carrier_round(0, 10);
    assert_eq!(drawn, 30, "one damage request, no swing");
    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::EnemySkillUsed { actor, skill: 2, name }
                if *actor == id(6) && name == "FLAME BOLT"
        )),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::Resolved { actor, damage: Some(85), remaining_hp: 315, .. }
                if *actor == id(6)
        )),
        "{events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(
            e,
            BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
        )),
        "dispatched, so nothing falls back: {events:?}"
    );
}

#[test]
fn an_unproven_carrier_still_reports_flame_bolt_as_unsupported() {
    // 0 Helex and 5 ForcedFly are the only enemies whose regular list holds
    // `$02`, and `EnemyAttackOffs` (`ps4.asm:19206`) gives ids `$00` and `$05`
    // the two traced arms. Any other carrier — whatever its own routine does
    // with a `$02` roll — stays outside the gate.
    for carrier in [1u16, 10u16] {
        let data = flame_carrier_data(carrier, 10);
        let mut r = flame_carrier_roster(&data, carrier);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            !resolve_damage_skill(
                &mut r,
                id(6),
                2,
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

        let (events, drawn) = flame_carrier_round(carrier, 10);
        assert_eq!(
            drawn, 31,
            "carrier {carrier}: the fallback attack adds its accuracy roll"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { actor, ability: 2 } if *actor == id(6)
            )),
            "carrier {carrier}: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(6))),
            "carrier {carrier}: the ordinary attack path is the fallback: {events:?}"
        );
    }

    // The same carrier with a different ability: `$33` is listed for 75/76/85/86
    // only, so Helex rolling it is not a route either.
    let acid = EnemySkill {
        id: 51,
        name: "ACIDBREATH".into(),
        effect: 1,
        power_stat: 1,
        target: 8,
        power: 24,
        resistance: 6,
        element: 1,
    };
    let data = flame_data().with_enemy_skills([acid]);
    let mut r = flame_roster(&data);
    let before = r.clone();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(!resolve_damage_skill(
        &mut r,
        id(6),
        51,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(r, before);
    assert_eq!(rolls.drawn(), 0);
    assert!(events.is_empty());
}

/// The gate is the pair plus the record's effect byte, not record byte 2's
/// target nibble. `AbilityRangeOffs` (`ps4.asm:8903`) resolves nibble 9 to
/// `AbilityRange_MultiChars` (`ps4.asm:8962`), which only decides how often
/// `Ability_ProcessRange` (`ps4.asm:8975`) calls the effect handler — and
/// these records' handler is `AbilityEffect_None` (`ps4.asm:9092`). The damage
/// request comes from the arm's object chain, so the old target-byte check is
/// gone: both listed pairs below resolve with the byte moved to 9.
#[test]
fn a_listed_route_resolves_with_its_record_on_the_nibble_9_target() {
    // The pairs are in the table — 75 FlattrPlnt's `$33` and 0 Helex's `$02` —
    // so only the record's effect byte can still refuse one.
    for (enemy_id, ability) in [(75u16, 51u8), (0, 2)] {
        assert!(
            DAMAGE_SKILL_ROUTES
                .iter()
                .any(|route| route.enemy_id == enemy_id && route.ability == ability),
            "({enemy_id}, {ability}) must be a listed route for this control"
        );
    }
    for (carrier, skill, target) in [(0u16, 2u8, 1u8), (0, 2, 8), (0, 2, 9), (75, 51, 9)] {
        let data = if skill == 2 {
            flame_carrier_data(carrier, 10)
        } else {
            acid_carrier_data(carrier, 10)
        };
        let mut record = data.enemy_skill(skill).unwrap().clone();
        record.target = target;
        let data = data.with_enemy_skills([record]);
        let mut r = flame_carrier_roster(&data, carrier);
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            resolve_damage_skill(
                &mut r,
                id(6),
                skill,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "carrier {carrier} ability {skill}: target {target} is not a gate"
        );
        assert_eq!(rolls.drawn(), 16, "carrier {carrier}: one damage request");
        assert_eq!(events.len(), 2, "carrier {carrier}: ability and damage");
        assert!(
            matches!(
                events[1],
                BattleEvent::Resolved {
                    damage: Some(_),
                    ..
                }
            ),
            "carrier {carrier}: {events:?}"
        );
    }
}

/// The refusal that replaces the target-byte check: the resolver models the
/// damage request alone, so a listed route whose record runs an effect handler
/// as well stays on the caller's unsupported path, drawing nothing and
/// emitting nothing. `AbilityEffectsOffs` (`ps4.asm:9036`) index `$02` is
/// `AbilityEffect_Death` (`ps4.asm:9098`), index `$01` is
/// `AbilityEffect_None` (`ps4.asm:9092`).
#[test]
fn a_listed_route_with_an_effect_handler_is_refused() {
    for (carrier, ability) in [
        (FROST_SABER, GIWAT),
        (DESRT_LEACH, SAND_STORM),
        (TECH_USER, WAT),
        (JUZA, FOI),
        (RAPPY, ROUND_EYES),
        (BLUE_RAPPY, LOVEL_EYES),
    ] {
        assert!(
            DAMAGE_SKILL_ROUTES
                .iter()
                .any(|route| route.enemy_id == carrier.enemy_id && route.ability == ability),
            "({}, {ability:#04X}) must be a listed route for this control",
            carrier.enemy_id
        );
        let listed = motavia_data(&[carrier], &[ability]);
        let record = EnemySkill {
            effect: 2,
            ..motavia_record(ability)
        };
        let data = listed.with_enemy_skills([record]);
        let mut r = motavia_roster(&data, &carrier);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            !resolve_damage_skill(
                &mut r,
                id(6),
                ability,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "{} {ability:#04X}: effect $02 needs its own handler first",
            carrier.symbol
        );
        assert_eq!(r, before, "{}", carrier.symbol);
        assert_eq!(rolls.drawn(), 0, "{}", carrier.symbol);
        assert!(events.is_empty(), "{}", carrier.symbol);
    }
}

/// The carriers of `docs/ENEMY_DAMAGE_ROUTES.md` §3's single-target pairs, with
/// the stat line `generated/enemies.json` gives them. `Enemy_DamageCharacter`
/// (`ps4.asm:3775`) reads the caster's stat through record byte 1, so each
/// expected number below is that carrier's own stat.
#[derive(Clone, Copy)]
struct Carrier {
    enemy_id: u16,
    symbol: &'static str,
    hp: u16,
    strength: u8,
    mental: u8,
    attack: u16,
    abilities: [u8; 8],
}

const FROST_SABER: Carrier = Carrier {
    enemy_id: 71,
    symbol: "FrostSaber",
    hp: 211,
    strength: 70,
    mental: 45,
    attack: 175,
    abilities: [0, 0, 0, 0, 0, GIWAT, GIWAT, GIWAT],
};
const TECH_PLANT: Carrier = Carrier {
    enemy_id: 77,
    symbol: "TechPlant",
    hp: 124,
    strength: 19,
    mental: 27,
    attack: 141,
    abilities: [0, 0, 42, GIWAT, GIWAT, GIWAT, 53, 53],
};
const HEW_GILLA: Carrier = Carrier {
    enemy_id: 91,
    symbol: "HewGilla",
    hp: 165,
    strength: 62,
    mental: 79,
    attack: 154,
    abilities: [
        GIWAT, GIWAT, GIWAT, WAT, WAT, FLODBREATH, FLODBREATH, FLODBREATH,
    ],
};
const DARK_WITCH: Carrier = Carrier {
    enemy_id: 101,
    symbol: "DarkWitch",
    hp: 253,
    strength: 40,
    mental: 76,
    attack: 124,
    abilities: [GIWAT, GIWAT, GIWAT, 53, 53, 72, 72, 72],
};
const DELM_LARS: Carrier = Carrier {
    enemy_id: 122,
    symbol: "DElmLars",
    hp: 777,
    strength: 50,
    mental: 45,
    attack: 185,
    abilities: [0, 0, 0, 0, GIWAT, GIWAT, GIWAT, 53],
};
const XE_ATHOUL: Carrier = Carrier {
    enemy_id: 123,
    symbol: "XeAThoul",
    hp: 1520,
    strength: 40,
    mental: 118,
    attack: 162,
    abilities: [0, 0, 0, GIWAT, GIWAT, GIWAT, 53, 53],
};
const DESRT_LEACH: Carrier = Carrier {
    enemy_id: 81,
    symbol: "DesrtLeach",
    hp: 1040,
    strength: 68,
    mental: 1,
    attack: 286,
    abilities: [0, 0, 0, 0, 0, SAND_STORM, SAND_STORM, SAND_STORM],
};
const LEVIATHAN: Carrier = Carrier {
    enemy_id: 82,
    symbol: "Leviathan",
    hp: 1240,
    strength: 84,
    mental: 1,
    attack: 252,
    abilities: [0, 0, 0, 0, 0, MAELSTROM, MAELSTROM, MAELSTROM],
};
const DEPCEN: Carrier = Carrier {
    enemy_id: 90,
    symbol: "Depcen",
    hp: 155,
    strength: 30,
    mental: 19,
    attack: 101,
    abilities: [0, 0, 0, 0, 0, FLODBREATH, FLODBREATH, FLODBREATH],
};
const ELMELEW: Carrier = Carrier {
    enemy_id: 92,
    symbol: "Elmelew",
    hp: 362,
    strength: 72,
    mental: 59,
    attack: 184,
    abilities: [
        WAT, WAT, WAT, WAT, FLODBREATH, FLODBREATH, FLODBREATH, FLODBREATH,
    ],
};
const TECH_USER: Carrier = Carrier {
    enemy_id: 99,
    symbol: "TechUser",
    hp: 80,
    strength: 10,
    mental: 25,
    attack: 42,
    abilities: [WAT, WAT, WAT, WAT, FOI, FOI, FOI, FOI],
};
const TECH_MASTER: Carrier = Carrier {
    enemy_id: 100,
    symbol: "TechMaster",
    hp: 120,
    strength: 21,
    mental: 38,
    attack: 85,
    abilities: [62, WAT, WAT, WAT, FOI, FOI, FOI, 71],
};
const JUZA: Carrier = Carrier {
    enemy_id: 114,
    symbol: "Juza",
    hp: 1523,
    strength: 28,
    mental: 30,
    attack: 92,
    abilities: [WAT, WAT, FOI, FOI, 71, 71, 86, 86],
};
const RAPPY: Carrier = Carrier {
    enemy_id: 147,
    symbol: "Rappy",
    hp: 65,
    strength: 34,
    mental: 23,
    attack: 173,
    abilities: [0, 0, 0, 0, ROUND_EYES, ROUND_EYES, ROUND_EYES, ROUND_EYES],
};
const BLUE_RAPPY: Carrier = Carrier {
    enemy_id: 148,
    symbol: "BlueRappy",
    hp: 130,
    strength: 54,
    mental: 35,
    attack: 184,
    abilities: [0, 0, 0, 0, LOVEL_EYES, LOVEL_EYES, LOVEL_EYES, LOVEL_EYES],
};

/// The one `EnemySkillData` record each ability's pair runs, as
/// `generated/enemy_skills.json` decodes it. Byte 1 stays raw — `$82` is the
/// masked-selector case.
fn motavia_record(ability: u8) -> EnemySkill {
    let (name, power_stat, target, power, resistance, element) = match ability {
        GIWAT => ("GIWAT", 0x82, 8, 88, 7, 5),
        SAND_STORM => ("SAND STORM", 0x05, 9, 96, 6, 1),
        MAELSTROM => ("MAELSTROM", 0x05, 9, 32, 6, 1),
        FLODBREATH => ("FLODBREATH", 0x05, 8, 20, 6, 1),
        WAT => ("WAT", 0x82, 8, 24, 7, 5),
        FOI => ("FOI", 0x82, 8, 20, 7, 3),
        ROUND_EYES => ("ROUND EYES", 0x01, 8, 0, 6, 1),
        LOVEL_EYES => ("LOVEL EYES", 0x05, 8, 32, 6, 1),
        other => panic!("no Motavia record for {other:#04X}"),
    };
    EnemySkill {
        id: ability,
        name: name.into(),
        effect: 1,
        power_stat,
        target,
        power,
        resistance,
        element,
    }
}

/// Carrier records plus the records the pairs under test run.
fn motavia_data(carriers: &[Carrier], abilities: &[u8]) -> BattleData {
    let enemies = carriers.iter().map(|carrier| {
        let mut record = fixtures::zoran_bult();
        record.id = carrier.enemy_id;
        record.name = carrier.symbol.into();
        record.hp = carrier.hp;
        record.strength = carrier.strength;
        record.mental = carrier.mental;
        record.attack = carrier.attack;
        record.agility = 100;
        record.regular_abilities = carrier.abilities;
        record.condition_ids = [0; 4];
        record
    });
    fixtures::data()
        .with_enemies(enemies)
        .with_enemy_skills(abilities.iter().map(|ability| motavia_record(*ability)))
}

/// Three party members with defense 7, no magic defense and every element
/// factor 2 (normal), so a number below depends only on the carrier's stat and
/// the record's bytes.
fn motavia_roster(data: &BattleData, carrier: &Carrier) -> Roster {
    let mut r = Roster::new();
    for record in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()] {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 255;
        member.stats.defence.battle = 7;
        member.stats.mental_defence.battle = 0;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        r.add_party_member(member.character, member.name, member.stats);
    }
    r.add_enemy(1, data.enemy(carrier.enemy_id).unwrap());
    r
}

/// `Battle_CalculateDamage` (`ps4.asm:17374`) as `Enemy_DamageCharacter`
/// (`ps4.asm:3775`) feeds it, at 16 zero draws (so `S + 8` is 8):
/// `(((8 * power) >> 6) + power + 2 * bonus) * element >> 2 - resistance`,
/// every step a 16-bit word like the cartridge's.
fn record_damage(power: u16, resistance: u16, element_factor: u8, bonus: u8) -> u16 {
    let mut t = 8u16.wrapping_mul(power);
    t >>= 6;
    t = t.wrapping_add(power);
    t = t.wrapping_add(u16::from(bonus).wrapping_mul(2));
    t = t.wrapping_mul(u16::from(element_factor));
    t >>= 2;
    crate::battle::clamp_damage(t.wrapping_sub(resistance) as i16)
}

/// Resolves one listed pair once and pins the request it makes: one damage
/// roll, an `EnemySkillUsed` and a `Resolved` — and nothing else. The party
/// fixture's defense is 7 and its magic defense 0, so a record selecting `$06`
/// subtracts 7 and one selecting `$07` subtracts 0.
fn resolve_pair(carrier: &Carrier, ability: u8) -> u16 {
    let data = motavia_data(&[*carrier], &[ability]);
    let mut r = motavia_roster(&data, carrier);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(
        resolve_damage_skill(
            &mut r,
            id(6),
            ability,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ),
        "{} {ability:#04X} is a listed route",
        carrier.symbol
    );
    assert_eq!(rolls.drawn(), 16, "{}: one damage request", carrier.symbol);
    assert_eq!(
        events.len(),
        2,
        "{}: ability and damage only: {events:?}",
        carrier.symbol
    );
    assert!(
        matches!(
            events[0],
            BattleEvent::EnemySkillUsed { actor, skill, .. }
                if actor == id(6) && skill == ability
        ),
        "{}: {events:?}",
        carrier.symbol
    );
    let BattleEvent::Resolved {
        actor,
        target,
        verdict,
        damage: Some(damage),
        remaining_hp,
    } = events[1]
    else {
        panic!("{}: {events:?}", carrier.symbol);
    };
    assert_eq!(actor, id(6), "{}", carrier.symbol);
    assert_eq!(target, id(2), "{}", carrier.symbol);
    assert_eq!(
        verdict,
        crate::battle::Verdict::Normal,
        "{}",
        carrier.symbol
    );
    assert_eq!(remaining_hp, 400 - damage, "{}", carrier.symbol);
    assert_eq!(
        r.get(id(1)).unwrap().stats.curr_hp,
        400,
        "{}",
        carrier.symbol
    );
    assert_eq!(
        r.get(id(3)).unwrap().stats.curr_hp,
        400,
        "{}",
        carrier.symbol
    );
    assert_eq!(
        r.get(id(2)).unwrap().stats.curr_hp,
        400 - damage,
        "{}",
        carrier.symbol
    );
    damage
}

/// `$2E` GIWAT: record byte 1 is `$82`. `Enemy_DamageCharacter` (line 3798)
/// and `Effect_SetupSkillParams` (line 9580) mask it with `$7F` before indexing
/// their stat table, so it is selector 2 (mental) — not selector 0, which
/// would deal `record_damage(0, ..)` — and each carrier's own mental stat
/// decides the number.
#[test]
fn giwat_reads_the_masked_mental_selector_for_every_carrier() {
    for (carrier, mental, expected) in [
        (FROST_SABER, 45u16, 113u16),
        (TECH_PLANT, 27, 103),
        (HEW_GILLA, 79, 132),
        (DARK_WITCH, 76, 130),
        (DELM_LARS, 45, 113),
        (XE_ATHOUL, 118, 154),
    ] {
        assert_eq!(motavia_record(GIWAT).power_stat, 0x82);
        assert_eq!(
            resolve_pair(&carrier, GIWAT),
            expected,
            "{}",
            carrier.symbol
        );
        assert_eq!(
            expected,
            record_damage(mental, 0, 2, 88),
            "{}: the formula's own terms",
            carrier.symbol
        );
        assert_ne!(
            expected,
            record_damage(0, 0, 2, 88),
            "{}: $82 masked is selector 2, not 0",
            carrier.symbol
        );
    }
}

/// `$37` SAND STORM: the record's byte 2 is 9 and its byte 1 is `$05`. Nibble 9
/// is `AbilityRange_MultiChars`, which only multiplies the no-op effect
/// handler of `AbilityEffect_None`; the chain makes one request. Selector 5 is
/// `attack`, which both readers take as a **word** — 286 here, so a byte read
/// would produce a different number.
#[test]
fn sand_storm_reads_the_attack_word_on_a_nibble_9_record() {
    assert_eq!(motavia_record(SAND_STORM).target, 9);
    let damage = resolve_pair(&DESRT_LEACH, SAND_STORM);
    assert_eq!(damage, 249);
    assert_eq!(damage, record_damage(286, 7, 2, 96), "attack is a word");
    assert_ne!(
        damage,
        record_damage(286 & 0xFF, 7, 2, 96),
        "a byte read of attack 286 would give a different number"
    );
}

#[test]
fn maelstrom_reads_the_attack_word_on_a_nibble_9_record() {
    assert_eq!(motavia_record(MAELSTROM).target, 9);
    assert_eq!(resolve_pair(&LEVIATHAN, MAELSTROM), 166);
    assert_eq!(166, record_damage(252, 7, 2, 32));
}

#[test]
fn flodbreath_reads_its_carriers_attack_word() {
    for (carrier, attack, expected) in [
        (DEPCEN, 101u16, 69u16),
        (HEW_GILLA, 154, 99),
        (ELMELEW, 184, 116),
    ] {
        assert_eq!(
            resolve_pair(&carrier, FLODBREATH),
            expected,
            "{}",
            carrier.symbol
        );
        assert_eq!(
            expected,
            record_damage(attack, 7, 2, 20),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

#[test]
fn wat_reads_the_masked_mental_selector_for_every_carrier() {
    for (carrier, mental, expected) in [
        (HEW_GILLA, 79u16, 68u16),
        (ELMELEW, 59, 57),
        (TECH_USER, 25, 38),
        (TECH_MASTER, 38, 45),
        (JUZA, 30, 40),
    ] {
        assert_eq!(resolve_pair(&carrier, WAT), expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            record_damage(mental, 0, 2, 24),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

#[test]
fn foi_reads_the_masked_mental_selector_for_every_carrier() {
    for (carrier, mental, expected) in [
        (TECH_USER, 25u16, 34u16),
        (TECH_MASTER, 38, 41),
        (JUZA, 30, 36),
    ] {
        assert_eq!(resolve_pair(&carrier, FOI), expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            record_damage(mental, 0, 2, 20),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

/// `$6D` ROUND EYES: selector 1 (strength, a byte) and power byte 0, so the
/// number is the carrier's strength alone against the target's defense.
#[test]
fn round_eyes_reads_strength_with_a_zero_power_byte() {
    assert_eq!(motavia_record(ROUND_EYES).power, 0);
    assert_eq!(resolve_pair(&RAPPY, ROUND_EYES), 12);
    assert_eq!(12, record_damage(34, 7, 2, 0));
}

#[test]
fn lovel_eyes_reads_the_attack_word() {
    assert_eq!(resolve_pair(&BLUE_RAPPY, LOVEL_EYES), 128);
    assert_eq!(128, record_damage(184, 7, 2, 32));
}

/// The stat widths `Enemy_DamageCharacter` (`ps4.asm:3775`) and
/// `Effect_SetupSkillParams` (`ps4.asm:9576`) read: their tables
/// (`loc_275A` `ps4.asm:3892`, `AbilityStatsOffs` `ps4.asm:9619`) hold
/// `atk_pow_battle`, `dfs_pow_battle` and `magic_dfs_battle` as words —
/// `$26`, `$2A`, `$2E` (`ps4.constants.asm:30`) — and compare the offset
/// against `$26` to pick `move.w`; selectors `$01`..`$04` stay bytes.
#[test]
fn the_stat_selectors_read_a_word_for_attack_defense_and_magic_defense() {
    let data = motavia_data(&[DESRT_LEACH], &[SAND_STORM]);
    let mut r = motavia_roster(&data, &DESRT_LEACH);
    let stats = &mut r.get_mut(id(1)).unwrap().stats;
    stats.attack.battle = 320;
    stats.defence.battle = 0x123;
    stats.mental_defence.battle = 0x1F4;
    stats.strength.battle = 200;
    stats.mental.battle = 201;
    stats.agility.battle = 202;
    stats.dexterity.battle = 203;
    assert_eq!(technique::stat(stats, 5), 320, "attack is the whole word");
    assert_eq!(technique::stat(stats, 6), 0x123, "defense is a word");
    assert_eq!(technique::stat(stats, 7), 0x1F4, "magic defense is a word");
    assert_eq!(technique::stat(stats, 1), 200);
    assert_eq!(technique::stat(stats, 2), 201);
    assert_eq!(technique::stat(stats, 3), 202);
    assert_eq!(technique::stat(stats, 4), 203);
    // Byte 1 arrives raw from the record: `$82` is selector 2 only after the
    // mask, and the mask is the caller's.
    assert_eq!(technique::stat(stats, 0x82), 0, "no such selector");
    assert_eq!(technique::stat(stats, 0x82 & STAT_INDEX_MASK), 201);
    assert_eq!(STAT_INDEX_MASK, 0x7F);
}

/// One round against one carrier, driven through `roll_enemy_ability` on a
/// fully specified draw stream: nine ordering draws, the four enemy-target
/// draws, the ability index (0, and every slot holds the ability) and the 16
/// damage draws — 30 in all when the ability resolves, with no swing. Party
/// agility 1 keeps the enemy first in the queue, so Defend has raised nobody's
/// resistance yet.
fn motavia_round(carrier: &Carrier, ability: u8) -> (Vec<BattleEvent>, usize) {
    // Every regular slot holds the ability under test, so the index roll lands
    // on it. The real list above is what the gate is proven against; a zero
    // slot here would dispatch the plain attack instead.
    let carrier = &Carrier {
        abilities: [ability; 8],
        ..*carrier
    };
    let data = motavia_data(&[*carrier], &[ability]);
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id: carrier.enemy_id,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 1;
        member.stats.element_props = [2; ELEMENT_SLOTS];
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

/// Every added route is reachable through the engine's own ability roll, not
/// just through a direct call: the ability resolves, the caster never swings,
/// and no other ability of the same carrier's list gets in the way.
#[test]
fn every_motavia_route_resolves_in_an_ordinary_round() {
    for (carrier, ability) in [
        (FROST_SABER, GIWAT),
        (TECH_PLANT, GIWAT),
        (HEW_GILLA, GIWAT),
        (DARK_WITCH, GIWAT),
        (DELM_LARS, GIWAT),
        (XE_ATHOUL, GIWAT),
        (DESRT_LEACH, SAND_STORM),
        (LEVIATHAN, MAELSTROM),
        (DEPCEN, FLODBREATH),
        (HEW_GILLA, FLODBREATH),
        (ELMELEW, FLODBREATH),
        (HEW_GILLA, WAT),
        (ELMELEW, WAT),
        (TECH_USER, WAT),
        (TECH_MASTER, WAT),
        (JUZA, WAT),
        (TECH_USER, FOI),
        (TECH_MASTER, FOI),
        (JUZA, FOI),
        (RAPPY, ROUND_EYES),
        (BLUE_RAPPY, LOVEL_EYES),
    ] {
        let (events, drawn) = motavia_round(&carrier, ability);
        assert_eq!(
            drawn, 30,
            "{} {ability:#04X}: one damage request, no swing",
            carrier.symbol
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::EnemySkillUsed { actor, skill, .. }
                    if *actor == id(6) && *skill == ability
            )),
            "{} {ability:#04X}: {events:?}",
            carrier.symbol
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::Resolved { actor, damage: Some(_), .. } if *actor == id(6)
            )),
            "{} {ability:#04X}: {events:?}",
            carrier.symbol
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
            )),
            "{} {ability:#04X}: dispatched, so nothing falls back: {events:?}",
            carrier.symbol
        );
    }
}
