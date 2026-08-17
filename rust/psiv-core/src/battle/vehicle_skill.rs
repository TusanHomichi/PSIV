//! Retail vehicle-skill records and their Tier-2 battle dispatcher.
//!
//! The records are `VehicleSkillData` at `ps4.asm:321275`. Vehicle commands
//! use the ordinary hit-flag pass (`loc_B6A2`), then `Character_DamageEnemy`
//! reads the skill record for the damage formula. A non-damage effect is
//! dispatched afterwards by `Battle_DoAttackEffect`; the current retail data
//! has one such record, N-Spher's death effect.

use super::action::roll_hits;
use super::chances::{Verdict, calculate_chances};
use super::damage::{calculate_damage, clamp_damage};
use super::event::{BattleEvent, Skipped};
use super::fighters::{FighterId, Roster};
use super::rng::Rolls;

/// The effect id in byte 1 of `VehicleSkillData` that the current records
/// exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VehicleSkillEffectKind {
    /// Effect id 1: the ordinary damage pipeline, with no extra status effect.
    Damage,
    /// Effect id 2: `AbilityEffect_Death`, rendered by `BattleObj_DeathEffect`.
    Death,
}

/// Which target stat byte 5 selects for a vehicle skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VehicleSkillResistance {
    /// Stat selector 2: mental.
    Mental,
    /// Stat selector 6: physical defence.
    Defence,
    /// Stat selector 7: magic defence.
    MentalDefence,
}

/// One decoded eight-byte `VehicleSkillData` record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleSkillData {
    /// One-based skill id and vehicle skill slot.
    pub id: u8,
    /// Retail display name from `VehicleAttackNames`.
    pub name: &'static str,
    /// The effect dispatcher selected by byte 1.
    pub effect: VehicleSkillEffectKind,
    /// Byte 4, passed as the damage bonus or the effect chance lower bound.
    pub power: u16,
    /// Byte 5, the target resistance stat selector.
    pub resistance: VehicleSkillResistance,
    /// Byte 6, the one-based target element selector.
    pub element: u8,
}

const VEHICLE_SKILLS: [VehicleSkillData; 8] = [
    VehicleSkillData {
        id: 1,
        name: "CLUSTER",
        effect: VehicleSkillEffectKind::Damage,
        power: 0x80,
        resistance: VehicleSkillResistance::Defence,
        element: 1,
    },
    VehicleSkillData {
        id: 2,
        name: "GRAVITN",
        effect: VehicleSkillEffectKind::Damage,
        power: 0xF0,
        resistance: VehicleSkillResistance::MentalDefence,
        element: 4,
    },
    VehicleSkillData {
        id: 3,
        name: "TH.GRID",
        effect: VehicleSkillEffectKind::Damage,
        power: 0,
        resistance: VehicleSkillResistance::MentalDefence,
        element: 7,
    },
    VehicleSkillData {
        id: 4,
        name: "X-BURST",
        effect: VehicleSkillEffectKind::Damage,
        power: 0xF0,
        resistance: VehicleSkillResistance::MentalDefence,
        element: 2,
    },
    VehicleSkillData {
        id: 5,
        name: "NAPALM",
        effect: VehicleSkillEffectKind::Damage,
        power: 0x80,
        resistance: VehicleSkillResistance::MentalDefence,
        element: 3,
    },
    VehicleSkillData {
        id: 6,
        name: "NOTHING",
        effect: VehicleSkillEffectKind::Damage,
        power: 0x80,
        resistance: VehicleSkillResistance::Defence,
        element: 1,
    },
    VehicleSkillData {
        id: 7,
        name: "N-SPHER",
        effect: VehicleSkillEffectKind::Death,
        power: 0x20,
        resistance: VehicleSkillResistance::Mental,
        element: 0x0E,
    },
    VehicleSkillData {
        id: 8,
        name: "NOTHING",
        effect: VehicleSkillEffectKind::Damage,
        power: 0x80,
        resistance: VehicleSkillResistance::Defence,
        element: 1,
    },
];

/// Returns the decoded retail record for a one-based vehicle skill id.
#[must_use]
pub fn vehicle_skill_data(skill: u8) -> Option<&'static VehicleSkillData> {
    VEHICLE_SKILLS.get(usize::from(skill.checked_sub(1)?))
}

fn resistance(stats: &super::stats::Stats, kind: VehicleSkillResistance) -> u16 {
    match kind {
        VehicleSkillResistance::Mental => u16::from(stats.mental.battle),
        VehicleSkillResistance::Defence => stats.defence.battle,
        VehicleSkillResistance::MentalDefence => stats.mental_defence.battle,
    }
}

fn target_factor(stats: &super::stats::Stats, element: u8) -> u16 {
    u16::from(stats.element_factor(element).unwrap_or(0))
}

fn apply_damage(
    roster: &mut Roster,
    actor: FighterId,
    target: FighterId,
    verdict: Verdict,
    skill: &VehicleSkillData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    if verdict == Verdict::Miss {
        let remaining = roster
            .get(target)
            .map_or(0, |fighter| fighter.stats.curr_hp);
        events.push(BattleEvent::Resolved {
            actor,
            target,
            verdict,
            damage: None,
            remaining_hp: remaining,
        });
        return false;
    }

    let attack = roster
        .get(actor)
        .map_or(0, |fighter| u16::from(fighter.stats.strength.battle));
    let (defence, element) = roster.get(target).map_or((0, 0), |fighter| {
        (
            resistance(&fighter.stats, skill.resistance),
            target_factor(&fighter.stats, skill.element),
        )
    });
    // Vehicle damage calls `loc_26D0` with the record's byte 4 as d4. It does
    // not substitute the physical critical bonus even when the generic hit
    // flag came back critical.
    let damage = clamp_damage(calculate_damage(
        attack,
        defence,
        element,
        skill.power,
        rolls,
    ));
    let fighter = roster.get_mut(target).expect("vehicle target present");
    let signed = i32::from(fighter.stats.curr_hp) - i32::from(damage);
    fighter.stats.curr_hp = signed.max(0) as u16;
    let remaining = fighter.stats.curr_hp;
    let killed = signed <= 0 && !fighter.stats.is_out();
    if killed {
        fighter.stats.status |= super::stats::status::DEAD;
    }
    events.push(BattleEvent::Resolved {
        actor,
        target,
        verdict,
        damage: Some(damage),
        remaining_hp: remaining,
    });
    if killed {
        events.push(BattleEvent::Died { fighter: target });
    }
    killed
}

/// Resolves a proven vehicle-skill record against every living enemy.
///
/// This follows the retail order: generic hit flags, skill damage, then the
/// effect dispatcher. The return value is the complete death list in event
/// order so the battle engine can award rewards exactly as it does for a
/// physical attack.
pub fn resolve_vehicle_skill(
    roster: &mut Roster,
    actor: FighterId,
    skill: u8,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Vec<FighterId> {
    let Some(record) = vehicle_skill_data(skill) else {
        events.push(BattleEvent::VehicleSkillEffectUnavailable { actor, skill });
        return Vec::new();
    };
    let targets: Vec<FighterId> = roster
        .living(actor.side().opposing())
        .map(|f| f.id)
        .collect();
    if targets.is_empty() {
        events.push(BattleEvent::TurnSkipped {
            actor,
            reason: Skipped::NoTarget,
        });
        return Vec::new();
    }

    events.push(BattleEvent::Attacked {
        actor,
        targets: targets.clone(),
    });
    let pass = roll_hits(roster, actor, &targets, true, rolls);
    let mut died = Vec::new();
    for (target, verdict) in &pass.verdicts {
        if apply_damage(roster, actor, *target, *verdict, record, rolls, events) {
            died.push(*target);
        }
    }

    let mut effect_targets = Vec::new();
    if record.effect == VehicleSkillEffectKind::Death {
        let actor_strength = roster
            .get(actor)
            .map_or(0, |fighter| i16::from(fighter.stats.strength.battle));
        for target in &targets {
            let Some(fighter) = roster.get(*target) else {
                continue;
            };
            if fighter.stats.is_out() {
                continue;
            }
            // `AbilityEffect_Death` calls `Battle_CalculateChances` with the
            // skill's byte 4 as d4 and effect id 2 as d5. The element factor
            // and mental resistance come from the same record.
            let verdict = calculate_chances(
                actor_strength,
                resistance(&fighter.stats, record.resistance) as i16,
                target_factor(&fighter.stats, record.element) as i16,
                record.power as i16,
                2,
                rolls,
            );
            if verdict != Verdict::Miss {
                effect_targets.push(*target);
            }
        }
    } else {
        effect_targets.extend(
            pass.verdicts
                .iter()
                .filter_map(|(target, verdict)| (*verdict != Verdict::Miss).then_some(*target)),
        );
    }

    events.push(BattleEvent::VehicleSkillEffect {
        actor,
        skill,
        effect: record.effect,
        targets: effect_targets.clone(),
    });
    for target in effect_targets {
        if roster
            .get(target)
            .is_some_and(|fighter| fighter.stats.is_out())
        {
            continue;
        }
        if record.effect == VehicleSkillEffectKind::Death {
            let fighter = roster
                .get_mut(target)
                .expect("vehicle effect target present");
            fighter.stats.status |= super::stats::status::DEAD;
            events.push(BattleEvent::Died { fighter: target });
            died.push(target);
        }
    }
    died
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures;
    use crate::battle::rng::SliceRolls;
    use crate::battle::stats::status;

    fn id(value: u8) -> FighterId {
        FighterId::new(value).expect("valid fighter id")
    }

    fn vehicle_roster() -> Roster {
        let mut roster = Roster::new();
        let vehicle = crate::vehicle::battle_member(
            1,
            crate::VehicleRecord {
                current_hp: 500,
                max_hp: 500,
                skill_mask: 0x03,
                current_skill_uses: [1, 1, 0, 0, 0, 0, 0, 0],
                max_skill_uses: [1, 1, 0, 0, 0, 0, 0, 0],
                ..crate::VehicleRecord::default()
            },
        )
        .expect("Land Rover");
        roster.add_party_member(vehicle.character, vehicle.name, vehicle.stats);
        roster.add_enemy(1, &fixtures::zoran_bult());
        roster.add_enemy(2, &fixtures::zoran_bult());
        roster
    }

    #[test]
    fn retail_skill_records_match_the_disassembly() {
        assert_eq!(vehicle_skill_data(1).expect("Cluster").power, 0x80);
        assert_eq!(vehicle_skill_data(2).expect("Gravitn").element, 4);
        assert_eq!(
            vehicle_skill_data(7).expect("N-Spher").effect,
            VehicleSkillEffectKind::Death
        );
        assert_eq!(
            vehicle_skill_data(8).expect("Nothing2").resistance,
            VehicleSkillResistance::Defence
        );
        assert!(vehicle_skill_data(0).is_none());
        assert!(vehicle_skill_data(9).is_none());
    }

    #[test]
    fn cluster_uses_skill_damage_and_not_a_physical_fallback() {
        let mut roster = vehicle_roster();
        roster.get_mut(id(6)).expect("first enemy").stats.curr_hp = 500;
        roster.get_mut(id(7)).expect("second enemy").stats.curr_hp = 500;
        let mut events = Vec::new();
        let draws = std::iter::once(40u16)
            .chain(std::iter::repeat_n(7u16, 16))
            .chain(std::iter::once(40u16))
            .chain(std::iter::repeat_n(0u16, 16))
            .collect::<Vec<_>>();
        let mut rolls = SliceRolls::new(&draws);
        let died = resolve_vehicle_skill(&mut roster, id(1), 1, &mut rolls, &mut events);
        assert!(died.is_empty());
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::VehicleSkillEffect {
                skill: 1,
                effect: VehicleSkillEffectKind::Damage,
                ..
            }
        )));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, BattleEvent::VehicleSkillEffectUnavailable { .. }))
        );
        assert_eq!(rolls.drawn(), 2 + 2 * super::super::damage::DAMAGE_DRAWS);
    }

    #[test]
    fn a_death_effect_marks_only_targets_that_pass_its_separate_chance() {
        let mut roster = vehicle_roster();
        roster.get_mut(id(6)).expect("first enemy").stats.curr_hp = 500;
        roster.get_mut(id(7)).expect("second enemy").stats.curr_hp = 500;
        roster
            .get_mut(id(6))
            .expect("first enemy")
            .stats
            .mental
            .battle = 100;
        let mut events = Vec::new();
        // Two generic hit rolls, then two damage rolls, then two N-Spher
        // effect rolls. The first effect misses at r=0; the second lands at
        // r=63 against the fixture's mental resistance.
        let draws = std::iter::repeat_n(63u16, 2)
            .chain(std::iter::repeat_n(0u16, 32))
            .chain([0u16, 63u16])
            .collect::<Vec<_>>();
        let mut rolls = SliceRolls::new(&draws);
        let died = resolve_vehicle_skill(&mut roster, id(1), 7, &mut rolls, &mut events);
        assert_eq!(died, vec![id(7)]);
        assert_eq!(
            roster.get(id(6)).expect("first enemy").stats.status & status::DEAD,
            0
        );
        assert!(!roster.get(id(7)).expect("second enemy").is_alive());
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::VehicleSkillEffect {
                effect: VehicleSkillEffectKind::Death,
                targets,
                ..
            } if targets == &vec![id(7)]
        )));
    }
}
