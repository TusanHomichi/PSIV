//! Damage-skill routes: Acid Breath's four carriers and FLAME BOLT's two,
//! through [`resolve_damage_skill`] and through `roll_enemy_ability`.
//!
//! The Acid Breath tests below moved here verbatim from `enemy_skill_tests.rs`
//! when the resolver moved out of `enemy_skill`: only the resolver's name
//! changed, from `resolve_acid_breath` to `resolve_damage_skill`. Their
//! fixtures (`acid_data`, `acid_carrier_data`, `acid_carrier_roster`,
//! `acid_roster`, `acid_carrier_round`) moved with them.

use super::*;
use crate::battle::enemy_skill::resolve_fission;
use crate::battle::{
    Battle, Command, EnemySkill, FormationEnemy, FormationRecord, PartyMember, RoundOrders,
    SliceRolls, fixtures, status,
};

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
            skill.target = 9;
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

#[test]
fn a_non_single_target_record_is_refused_even_for_a_listed_route() {
    // The pair is in the table — 75 FlattrPlnt's `$33` and 0 Helex's `$02` —
    // so only the record's own byte can refuse: `AbilityRangeOffs`
    // (`ps4.asm:8903`) resolves range 8 to `AbilityRange_Single`
    // (`ps4.asm:8938`), and no other range is proven here.
    for (enemy_id, ability) in [(75u16, 51u8), (0, 2)] {
        assert!(
            DAMAGE_SKILL_ROUTES.contains(&DamageRoute { enemy_id, ability }),
            "({enemy_id}, {ability}) must be a listed route for this control"
        );
    }
    for (carrier, skill) in [(0u16, 2u8), (75, 51)] {
        // The carrier's own fixture data, with only the record's target byte
        // moved off the proven range.
        let data = if skill == 2 {
            flame_carrier_data(carrier, 10)
        } else {
            acid_carrier_data(carrier, 10)
        };
        let mut record = data.enemy_skill(skill).unwrap().clone();
        record.target = 9;
        let data = data.with_enemy_skills([record]);
        let mut r = Roster::new();
        for member in [fixtures::alys(), fixtures::chaz()] {
            let mut seated = PartyMember::seat(&member, &data).unwrap();
            seated.stats.curr_hp = 400;
            seated.stats.max_hp = 400;
            r.add_party_member(seated.character, seated.name, seated.stats);
        }
        r.add_enemy(1, data.enemy(carrier).unwrap());
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            !resolve_damage_skill(
                &mut r,
                id(6),
                skill,
                Some(id(1)),
                &data,
                &mut rolls,
                &mut events
            ),
            "carrier {carrier} ability {skill}: target 9 is not the proven range"
        );
        assert_eq!(r, before);
        assert_eq!(rolls.drawn(), 0);
        assert!(events.is_empty());
    }
}
