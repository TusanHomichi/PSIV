//! `$02` FLAME BOLT: 0 Helex and 5 ForcedFly, through [`resolve_damage_skill`]
//! and through `roll_enemy_ability`.
//!
//! `EnemyAttack_ForcedFly` (`ps4.asm:23567`) falls through into
//! `EnemyAttack_Helex` (`ps4.asm:23574`) for a nonzero ability, so both carriers
//! share one object chain and the record's arithmetic, and both take the same
//! single `move.w #$C` request. This module's fixtures build that record on
//! either carrier and a party whose three members are identical, so an expected
//! number depends on the caster's strength and the target's own record alone;
//! `gate_tests` imports `flame_carrier_data` and `flame_carrier_roster` for the
//! byte-2 control.

use super::tests::*;
use super::*;
use crate::battle::{
    Battle, Command, EnemySkill, FormationEnemy, FormationRecord, PartyMember, RoundOrders,
    SliceRolls, fixtures,
};

/// `generated/enemies.json`: 0 Helex is `$02` in all eight regular slots, 5
/// ForcedFly in slots 5-8 only. The fixture hands every slot to whichever
/// carrier is under test, so the ability roll always reaches `$02`.
pub(super) fn flame_carrier_data(enemy_id: u16, strength: u8) -> BattleData {
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
pub(super) fn flame_carrier_roster(data: &BattleData, enemy_id: u16) -> Roster {
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
/// draws, the ability index (1, and every slot holds `$02`) and the 16 damage
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
    // The ability index: 1, not 0 - a battle starts the re-roll word at zero
    // (`ps4.asm:9992-9994`), so a zero draw would spend a re-roll, and every
    // slot holds the ability under test.
    stream.push(1);
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
