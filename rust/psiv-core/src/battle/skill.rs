//! Character skills, distinct from vehicle skills.
//!
//! Each cartridge skill has an animation routine that can also change gameplay.
//! Matching an effect byte alone is insufficient: animation can consume RNG,
//! while Crosscut/DblSlash's repeated visual strikes calculate damage once.
//! CharSkill_SkillObjsOffs (ps4.asm:15403-15459) owns the ID dispatch.
//! Shared support rules live in player_effect; healing tails in skill_recovery.

use super::player_effect::{self, Effect};
use super::retarget;
use super::technique::{in_range, stat};
#[path = "skill_recovery.rs"]
mod recovery;
use super::{BattleData, BattleDataError, BattleEvent, FighterId, Rolls, Roster, Side, Verdict};
use super::{ability_element_factor, calculate_damage, clamp_damage, status, weapon_reach};

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;

/// Normalized eight-byte record from the character skill table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// One-based cartridge id.
    pub id: u8,
    /// Cartridge display name.
    pub name: String,
    /// Ability effect index.
    pub effect: u8,
    /// Actor stat selector from byte 1's low seven bits.
    pub power_stat: u8,
    /// Byte 1's high bit requires either hand to hold a weapon.
    pub requires_weapon: bool,
    /// Byte 2, usability and target range.
    pub targeting: u8,
    /// Byte 3, damage power or effect miss threshold.
    pub power: u8,
    /// Byte 4, target stat selector.
    pub resistance: u8,
    /// Byte 5, resistance element (16 selects the actor's weapon element).
    pub element: u8,
}

impl Skill {
    /// Only enable skills with transcribed gameplay, not merely familiar effects.
    #[must_use]
    pub const fn supported(&self) -> bool {
        matches!(self.targeting >> 4, 1 | 3)
            && self.power_stat <= 7
            && self.resistance <= 7
            && self.targeting & 15 <= 9
            && (self.element <= 14 || self.element == 16)
            && matches!((self.id, self.effect),
                (1..=19 | 49..=51, 1)
                | (20..=27 | 34 | 48 | 52, 2)
                | (28 | 30, 6) | (29, 3)
                | (31..=33 | 35..=37, 7)
                | (38 | 53, 11) | (39, 9) | (40 | 41, 10)
                | (42..=44 | 54, 18) | (45, 23) | (46, 15) | (47, 38))
    }

    /// Whether the skill needs a target cursor.
    #[must_use]
    pub const fn single_target(&self) -> bool {
        matches!(self.targeting & 15, 1 | 4 | 6 | 8)
    }
}

/// Why a character skill could not execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillRejection {
    /// Missing definition or unimplemented gameplay.
    Unavailable,
    /// No matching skill in the actor's learned slots.
    NotLearned,
    /// No saved uses remain in the matching slot.
    Exhausted,
    /// Target lies outside the skill's range.
    InvalidTarget,
    /// Neither hand holds a weapon. Retail checks this after consuming a use.
    Unarmed,
}

/// Living, eligible recipients, shared with the native command menu.
#[must_use]
pub fn skill_targets(roster: &Roster, actor: FighterId, skill: &Skill) -> Vec<FighterId> {
    roster
        .iter()
        .filter(|f| {
            (f.is_alive() || skill.id == 45)
                && in_range(&f.stats, f.id, actor, skill.targeting & 15)
        })
        .map(|f| f.id)
        .collect()
}

pub(super) fn resolve_skill(
    roster: &mut Roster,
    actor: FighterId,
    id: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Result<Vec<FighterId>, BattleDataError> {
    let Some(caster) = roster.get(actor) else {
        return Ok(Vec::new());
    };
    let skill = data.skill(id);
    let slot = caster.stats.skills.iter().position(|known| *known == id);
    let rejection = match skill {
        None => Some(SkillRejection::Unavailable),
        Some(s) if !s.supported() => Some(SkillRejection::Unavailable),
        Some(_) if actor.side() != Side::Party || slot.is_none() => {
            Some(SkillRejection::NotLearned)
        }
        Some(_) if caster.stats.curr_skill_uses[slot.expect("known")] == 0 => {
            Some(SkillRejection::Exhausted)
        }
        Some(s)
            if s.single_target()
                && !intended
                    .and_then(|target| roster.get(target))
                    .is_some_and(|f| in_range(&f.stats, f.id, actor, s.targeting & 15)) =>
        {
            Some(SkillRejection::InvalidTarget)
        }
        _ => None,
    };
    if let Some(reason) = rejection {
        events.push(BattleEvent::SkillRejected {
            actor,
            skill: id,
            reason,
        });
        return Ok(Vec::new());
    }
    let skill = skill.expect("validated");
    let armed = !skill.requires_weapon || weapon_reach(&caster.stats, data)?.is_some();
    // Preserve the ratified Vision name-dependency fix: normal Hahn gives
    // +8, regardless of a renamed save (docs/source-notes/battle-party.md).
    let power = if id == 47 && skill.power_stat == 0 {
        8
    } else {
        stat(&caster.stats, skill.power_stat)
    };
    let intended =
        retarget::ability_target(roster, intended, skill.targeting & 15, skill.effect, rolls);
    let caster = roster.get_mut(actor).expect("present");
    let slot = slot.expect("known");
    caster.stats.curr_skill_uses[slot] -= 1;
    events.push(BattleEvent::SkillUsed {
        actor,
        skill: id,
        name: skill.name.clone(),
        remaining: caster.stats.curr_skill_uses[slot],
    });
    if !armed {
        events.push(BattleEvent::SkillRejected {
            actor,
            skill: id,
            reason: SkillRejection::Unarmed,
        });
        return Ok(Vec::new());
    }
    let mut targets = skill_targets(roster, actor, skill);
    if skill.single_target() {
        // The shared turn-time aim check leaves party-side skills' aims alone.
        targets = intended
            .filter(|chosen| targets.contains(chosen))
            .into_iter()
            .collect();
    }
    let mut died = Vec::new();
    if skill.effect == 1 {
        super::player_animation::skill_before_damage(id, rolls);
    }
    for target in targets {
        // `loc_27D4` / `loc_2836` (`ps4.asm:3989` and `4031`): a skill sets bit 1
        // on the enemy it reaches — and bit 4 with it when the command covered
        // the whole side. The skill arm sets no bit 2: that is the technique
        // and combo arm only.
        if target.side() == Side::Enemy
            && let Some(fighter) = roster.get_mut(target)
        {
            fighter.reaction_flags |= super::fighters::reaction::MAGIC
                | if skill.single_target() {
                    0
                } else {
                    super::fighters::reaction::MULTI_TARGET
                };
        }
        match skill.effect {
            1 => {
                // Crosscut's two loc_9848 passes (ps4.asm:77168-77226)
                // and DblSlash's claw passes (74862-74946) schedule shake
                // choreography, not damage. loc_9880 sets fighter routine $C
                // once (14997-15013), which reaches loc_2836/loc_266C.
                // Owner-specific stock captures pin one 16-draw damage pass.
                let attacker = &roster.get(actor).expect("present").stats;
                let recipient = &roster.get(target).expect("present").stats;
                let element = ability_element_factor(attacker, recipient, skill.element, data)?;
                let damage = clamp_damage(calculate_damage(
                    power,
                    stat(recipient, skill.resistance),
                    element,
                    u16::from(skill.power),
                    rolls,
                ));
                let stats = &mut roster.get_mut(target).expect("present").stats;
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
            15 | 18 | 23 => recovery::resolve(roster, actor, target, skill, power, rolls, events),
            effect => {
                let stats = &mut roster.get_mut(target).expect("present").stats;
                let effect = Effect {
                    effect,
                    power,
                    threshold: skill.power,
                    resistance: skill.resistance,
                    element: skill.element,
                };
                if !player_effect::lands(stats, &effect, rolls) {
                    if effect.effect == 7
                        && stats.status & (status::ASLEEP | status::PARALYZED) != 0
                    {
                        events.push(BattleEvent::SkillIneffective { actor, target });
                    } else {
                        events.push(BattleEvent::Resolved {
                            actor,
                            target,
                            verdict: Verdict::Miss,
                            damage: None,
                            remaining_hp: stats.curr_hp,
                        });
                    }
                    continue;
                }
                match effect.effect {
                    2 => {
                        // SkillObj_Explode through Negatis and Crash call the
                        // chance pass once, then their object tails clear HP
                        // (ps4.asm:15680-15730, 15789-15802; e.g. 77781-77789).
                        stats.curr_hp = 0;
                        stats.status |= status::DEAD;
                        events.push(BattleEvent::Died { fighter: target });
                        died.push(target);
                    }
                    7 => {
                        // Earth and the shared loc_3C130 tail set only sleep:
                        // ps4.asm:76737-76742, 78059-78076. No AGI reduction.
                        stats.status |= status::ASLEEP;
                        events.push(BattleEvent::FellAsleep { actor, target });
                    }
                    _ => player_effect::support(stats, actor, target, &effect, events),
                }
            }
        }
    }
    if skill.effect == 1 {
        super::player_animation::skill_after_damage(id, rolls);
    }
    if id == 46 {
        // loc_3CB70 (ps4.asm:78878-78886) clears bit 3 for every occupied
        // party fighter, even one excluded from the TP effect's human range.
        for fighter in roster.iter_mut().filter(|f| f.id.side() == Side::Party) {
            let removed = fighter.stats.status & status::ASLEEP;
            fighter.stats.status &= !status::ASLEEP;
            if removed != 0 {
                events.push(BattleEvent::StatusRestored {
                    actor,
                    target: fighter.id,
                    removed,
                });
            }
        }
    }
    Ok(died)
}

/// Battle_RestoreStatsAtTurnEnd: one draw per sleeping occupied
/// fighter, in slot order. Odd wakes; even retains sleep. Enemy paralysis
/// then clears without a roll or stat restoration.
pub(super) fn recover_round_status(
    roster: &mut Roster,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) {
    for fighter in roster.iter_mut().filter(|f| f.active) {
        if fighter.stats.status & (status::ASLEEP | status::ASLEEP_2) != 0
            && rolls.next_roll() & 1 != 0
        {
            fighter.stats.status &= !(status::ASLEEP | status::ASLEEP_2);
            fighter.stats.agility.battle = fighter.stats.agility.modified;
            events.push(BattleEvent::WokeUp {
                fighter: fighter.id,
            });
        }
    }
    for fighter in roster
        .iter_mut()
        .filter(|f| f.active && f.id.side() == Side::Enemy)
    {
        if fighter.stats.status & status::PARALYZED != 0 {
            fighter.stats.status &= !status::PARALYZED;
            events.push(BattleEvent::ParalysisCleared {
                fighter: fighter.id,
            });
        }
    }
}
