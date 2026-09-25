//! Player techniques from the eight-byte cartridge records.
//!
//! Damage: $281E -> $26F8 -> $266C. Healing: $2EC4 -> $2F02.
//! Support effects: AbilityEffectsOffs ($61BE), Effect_DoTechnique ($654A).
//! Damage techniques do not take physical hit/critical rolls ($B75A).
//! Unsupported effects fail before payment; they never become an attack.

use super::{BattleData, BattleEvent, FighterId, Roster, Side, Stats, Verdict};
use super::{Rolls, calc_healing, calculate_chances, calculate_damage, clamp_damage, status};

#[cfg(test)]
#[path = "technique_tests.rs"]
mod tests;

/// Normalized technique data, owned by the engine rather than its JSON loader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Technique {
    /// One-based cartridge id.
    pub id: u8,
    /// Cartridge display name.
    pub name: String,
    /// AbilityEffectsOffs index.
    pub effect: u8,
    /// TP charged on execution.
    pub cost: u8,
    /// Byte 2: usability and target mode.
    pub targeting: u8,
    /// Byte 3: damage/healing power or effect miss threshold.
    pub power: u8,
    /// Byte 4: target stat selector.
    pub resistance: u8,
    /// Byte 5: element selector.
    pub element: u8,
}

impl Technique {
    /// Only dispatch effects implemented here, with valid battle parameters.
    #[must_use]
    pub const fn supported(&self) -> bool {
        matches!(self.targeting >> 4, 1 | 3)
            && self.targeting & 15 <= 9
            && self.resistance <= 7
            && self.element <= 14
            && matches!(self.effect, 1 | 2 | 3 | 6 | 7 | 9 | 10 | 11 | 12 | 18..=22)
    }

    /// Whether command selection needs a target cursor.
    #[must_use]
    pub const fn single_target(&self) -> bool {
        matches!(self.targeting & 15, 1 | 4 | 6 | 8)
    }
}

/// Why a technique could not execute. Late sealing is the retail exception
/// that consumes TP; other rejections spend nothing and consume no RNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechniqueRejection {
    /// No definition or effect dispatcher.
    Unavailable,
    /// The actor has not learned this id.
    NotLearned,
    /// The current TP balance cannot cover the cost.
    InsufficientTp,
    /// The selected fighter is outside the technique's range.
    InvalidTarget,
    /// Techniques were sealed before the actor's turn.
    Sealed,
}

/// Battle-only stat changed by a support technique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechniqueStat {
    /// Attack power.
    Attack,
    /// Defense power.
    Defence,
    /// Mental defense.
    MentalDefence,
    /// Agility.
    Agility,
    /// Dexterity (also used by skill support effects).
    Dexterity,
}

pub(super) fn in_range(stats: &Stats, id: FighterId, actor: FighterId, range: u8) -> bool {
    match range {
        0 => true,
        1 | 2 => id.side() == actor.side().opposing(),
        3 => id == actor,
        4 | 5 => id.side() == actor.side() && !stats.is_android(),
        6 | 7 => id.side() == actor.side() && stats.is_android(),
        8 | 9 => id.side() == actor.side(),
        _ => false,
    }
}

/// Eligible targets, in fighter-id order. Revival also permits downed allies.
/// Shared with command menus; an ineligible living REVER target still spends TP.
#[must_use]
pub fn technique_targets(roster: &Roster, actor: FighterId, tech: &Technique) -> Vec<FighterId> {
    roster
        .iter()
        .filter(|f| {
            (f.is_alive() || matches!(tech.effect, 21 | 22))
                && in_range(&f.stats, f.id, actor, tech.targeting & 15)
        })
        .map(|f| f.id)
        .collect()
}

pub(super) fn stat(stats: &Stats, selector: u8) -> u16 {
    match selector {
        1 => stats.strength.battle.into(),
        2 => stats.mental.battle.into(),
        3 => stats.agility.battle.into(),
        4 => stats.dexterity.battle.into(),
        5 => stats.attack.battle,
        6 => stats.defence.battle,
        7 => stats.mental_defence.battle,
        _ => 0,
    }
}

pub(super) fn resolve_technique(
    roster: &mut Roster,
    actor: FighterId,
    id: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Vec<FighterId> {
    let Some(caster) = roster.get(actor) else {
        return Vec::new();
    };
    let tech = data.technique(id);
    let rejection = match tech {
        None => Some(TechniqueRejection::Unavailable),
        Some(t) if !t.supported() => Some(TechniqueRejection::Unavailable),
        Some(_) if actor.side() != Side::Party || !caster.stats.techniques.contains(&id) => {
            Some(TechniqueRejection::NotLearned)
        }
        Some(t) if caster.stats.curr_tp < u16::from(t.cost) => {
            Some(TechniqueRejection::InsufficientTp)
        }
        Some(t)
            if t.single_target()
                && !intended
                    .and_then(|target| roster.get(target))
                    .is_some_and(|f| in_range(&f.stats, f.id, actor, t.targeting & 15)) =>
        {
            Some(TechniqueRejection::InvalidTarget)
        }
        _ => None,
    };
    if let Some(reason) = rejection {
        events.push(BattleEvent::TechniqueRejected {
            actor,
            technique: id,
            reason,
        });
        return Vec::new();
    }
    let tech = tech.expect("validated");
    let power = u16::from(caster.stats.mental.battle);
    let sealed = caster.stats.status & status::TECH_SEALED != 0;
    let caster = roster.get_mut(actor).expect("present");
    caster.stats.curr_tp -= u16::from(tech.cost);
    events.push(BattleEvent::TechniqueUsed {
        actor,
        technique: id,
        name: tech.name.clone(),
        remaining_tp: caster.stats.curr_tp,
    });
    // Retail pays in CharTech_CheckTPCost, then tests sealing in CharTech_Cast.
    // A seal landing after selection therefore still costs TP.
    if sealed {
        events.push(BattleEvent::TechniqueRejected {
            actor,
            technique: id,
            reason: TechniqueRejection::Sealed,
        });
        return Vec::new();
    }
    let mut targets = technique_targets(roster, actor, tech);
    if tech.single_target() {
        let selected = intended.filter(|id| targets.contains(id));
        // An enemy killed earlier in the queue gets the same first-living
        // retarget as an attack. A dead healing recipient is never replaced.
        let selected = selected.or_else(|| {
            (tech.targeting & 15 == 1)
                .then(|| targets.first().copied())
                .flatten()
        });
        targets = selected.into_iter().collect();
    }
    let mut died = Vec::new();
    for target in targets {
        // `loc_27D4` (`ps4.asm:3989-3992`) on the way into
        // `Character_DamageEnemy`: a technique marks the enemy it reaches, and
        // bit 4 joins it when the command covered the whole side rather than a
        // named fighter (`Current_Target_Index` negative).
        if target.side() == Side::Enemy
            && let Some(fighter) = roster.get_mut(target)
        {
            fighter.reaction_flags |= super::fighters::reaction::MAGIC
                | super::fighters::reaction::TECHNIQUE
                | if tech.single_target() {
                    0
                } else {
                    super::fighters::reaction::MULTI_TARGET
                };
        }
        let stats = &mut roster.get_mut(target).expect("selected").stats;
        match tech.effect {
            1 => {
                let element = u16::from(stats.element_factor(tech.element).unwrap_or(0));
                let damage = clamp_damage(calculate_damage(
                    power,
                    stat(stats, tech.resistance),
                    element,
                    u16::from(tech.power),
                    rolls,
                ));
                stats.curr_hp = stats.curr_hp.saturating_sub(damage);
                events.push(BattleEvent::Resolved {
                    actor,
                    target,
                    verdict: Verdict::Normal,
                    damage: Some(damage),
                    remaining_hp: stats.curr_hp,
                });
                if stats.curr_hp == 0 {
                    stats.status |= status::DEAD;
                    events.push(BattleEvent::Died { fighter: target });
                    died.push(target);
                }
            }
            18 => {
                let before = stats.curr_hp;
                stats.curr_hp = stats
                    .curr_hp
                    .saturating_add(calc_healing(power, u16::from(tech.power), rolls))
                    .min(stats.max_hp);
                events.push(BattleEvent::Healed {
                    actor,
                    target,
                    amount: stats.curr_hp - before,
                    remaining_hp: stats.curr_hp,
                });
            }
            19..=22 => resolve_recovery(stats, actor, target, tech.effect, events),
            effect => {
                // AbilityEffect_SleepParalyze returns before the chance roll
                // when the target is already asleep or paralyzed.
                if effect == 7 && stats.status & (status::ASLEEP | status::PARALYZED) != 0 {
                    events.push(BattleEvent::TechniqueIneffective { actor, target });
                    continue;
                }
                // Effect_DoTechnique bypasses the chance roll when byte 4 is
                // zero. Successful buffs/debuffs use the caster's MEN itself.
                if tech.resistance != 0
                    && calculate_chances(
                        power as i16,
                        stat(stats, tech.resistance) as i16,
                        i16::from(stats.element_factor(tech.element).unwrap_or(0)),
                        i16::from(tech.power),
                        i16::from(effect),
                        rolls,
                    ) == Verdict::Miss
                {
                    events.push(BattleEvent::Resolved {
                        actor,
                        target,
                        verdict: Verdict::Miss,
                        damage: None,
                        remaining_hp: stats.curr_hp,
                    });
                    continue;
                }
                if effect == 2 {
                    // BROSE/VOL/SAVOL mark successful targets in effect 2;
                    // their animation completion at loc_39068 clears HP.
                    // Death earns the ordinary enemy rewards, without a
                    // damage roll or a synthetic amount displayed as damage.
                    stats.curr_hp = 0;
                    stats.status |= status::DEAD;
                    events.push(BattleEvent::Died { fighter: target });
                    died.push(target);
                    continue;
                }
                if effect == 7 {
                    // BattleObj_RimitMain sets only StatusAsleep. It does not
                    // apply the generic sleep object's agility reduction.
                    stats.status |= status::ASLEEP;
                    events.push(BattleEvent::FellAsleep { actor, target });
                    continue;
                }
                let (stat, value) = match effect {
                    3 => {
                        stats.attack.battle = stats.attack.derived.saturating_sub(power);
                        (TechniqueStat::Attack, stats.attack.battle)
                    }
                    6 => {
                        stats.agility.battle =
                            stats.agility.modified.saturating_sub(power as u8).max(1);
                        (TechniqueStat::Agility, stats.agility.battle.into())
                    }
                    9 => {
                        stats.attack.battle = stats.attack.derived.wrapping_add(power);
                        (TechniqueStat::Attack, stats.attack.battle)
                    }
                    10 => {
                        stats.defence.battle = stats.defence.derived.wrapping_add(power);
                        (TechniqueStat::Defence, stats.defence.battle)
                    }
                    11 => {
                        stats.mental_defence.battle =
                            stats.mental_defence.derived.wrapping_add(power);
                        (TechniqueStat::MentalDefence, stats.mental_defence.battle)
                    }
                    12 => {
                        stats.agility.battle = stats.agility.modified.wrapping_add(power as u8);
                        (TechniqueStat::Agility, stats.agility.battle.into())
                    }
                    _ => unreachable!("supported effects checked before payment"),
                };
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat,
                    value,
                });
            }
        }
    }
    died
}

/// Ability_ProcessRange and BattleObj_StatusHeal/BattleObj_Rever. The effect
/// pass resets AGI/DEX even if no ailment is present (US bugfixes=0). The
/// animation's completion clears the corresponding flags and restores HP.
fn resolve_recovery(
    stats: &mut Stats,
    actor: FighterId,
    target: FighterId,
    effect: u8,
    events: &mut Vec<BattleEvent>,
) {
    if effect == 21 && !stats.is_out() {
        events.push(BattleEvent::TechniqueIneffective { actor, target });
        return;
    }
    let before = stats.clone();
    stats.agility.battle = stats.agility.modified;
    stats.dexterity.battle = stats.dexterity.modified;
    match effect {
        19 => stats.status &= !status::POISONED,
        20 => stats.status &= !status::PARALYZED,
        21 | 22 => {
            // loc_395A4: both REVER and REGEN clear all but tech seal and
            // the transient sprite bit. The latter is represented by Revived.
            stats.status &= status::TECH_SEALED;
            stats.curr_hp = if effect == 21 {
                stats.max_hp / 4
            } else {
                stats.max_hp
            };
        }
        _ => unreachable!("recovery effects only"),
    }
    if before.is_out() && !stats.is_out() {
        events.push(BattleEvent::Revived {
            actor,
            target,
            remaining_hp: stats.curr_hp,
        });
    } else if stats.curr_hp > before.curr_hp {
        events.push(BattleEvent::Healed {
            actor,
            target,
            amount: stats.curr_hp - before.curr_hp,
            remaining_hp: stats.curr_hp,
        });
    }
    let removed = before.status & !stats.status;
    if removed != 0 {
        events.push(BattleEvent::StatusRestored {
            actor,
            target,
            removed,
        });
    }
    if before.agility.battle != stats.agility.battle
        || before.dexterity.battle != stats.dexterity.battle
    {
        events.push(BattleEvent::StatsRestored { actor, target });
    }
    if *stats == before {
        events.push(BattleEvent::TechniqueIneffective { actor, target });
    }
}
