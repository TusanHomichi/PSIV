//! The opening party's character skills, distinct from vehicle skills.
//!
//! Each cartridge skill has an animation routine that can also change gameplay.
//! Matching an effect byte alone is insufficient (e.g. DblSlash hits twice).
//! Crosscut, Vortex, Earth, Crash and Vision have transcribed dispatchers here.

use super::technique::{in_range, stat};
use super::{
    BattleData, BattleDataError, BattleEvent, FighterId, Rolls, Roster, Side, TechniqueStat,
    Verdict,
};
use super::{
    ability_element_factor, calculate_chances, calculate_damage, clamp_damage, status, weapon_reach,
};

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
            && match (self.id, self.effect, self.targeting & 15) {
                (1 | 6, 1, 1) => self.element <= 14 || self.element == 16,
                (31, 7, 1) | (34, 2, 1) => self.element <= 14,
                (47, 38, 5) => self.element <= 14,
                _ => false,
            }
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
        .filter(|f| f.is_alive() && in_range(&f.stats, f.id, actor, skill.targeting & 15))
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
    // Retail Vision selects offset zero, reading the name's first byte: Hahn's
    // H is 8. Preserve the normal +8 while fixing name-dependent strength.
    let power = if id == 47 && skill.power_stat == 0 {
        8
    } else {
        stat(&caster.stats, skill.power_stat)
    };
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
        let target = intended
            .filter(|t| targets.contains(t))
            .or_else(|| targets.first().copied());
        targets = target.into_iter().collect();
    }
    let mut died = Vec::new();
    for target in targets {
        match id {
            1 | 6 => {
                // Crosscut's loc_3B684 reaches loc_9848 twice, 25 animation
                // ticks apart. Each live hit rolls damage independently;
                // loc_B75A skips a target killed by the first hit.
                // Vortex reaches loc_9848 once. The three trailing sprites are
                // presentation; this is one damage application, sixteen draws.
                for _ in 0..if id == 1 { 2 } else { 1 } {
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
                        break;
                    }
                }
            }
            31 => {
                let stats = &mut roster.get_mut(target).expect("present").stats;
                // AbilityEffect_SleepParalyze short-circuits before the roll.
                if stats.status & (status::ASLEEP | status::PARALYZED) != 0 {
                    events.push(BattleEvent::SkillIneffective { actor, target });
                    continue;
                }
                let verdict = calculate_chances(
                    power as i16,
                    stat(stats, skill.resistance) as i16,
                    i16::from(stats.element_factor(skill.element).unwrap_or(0)),
                    i16::from(skill.power),
                    i16::from(skill.effect),
                    rolls,
                );
                if verdict == Verdict::Miss {
                    events.push(BattleEvent::Resolved {
                        actor,
                        target,
                        verdict,
                        damage: None,
                        remaining_hp: stats.curr_hp,
                    });
                } else {
                    // BattleObj_Earth's successful timer path sets only bit 3.
                    // Unlike the generic sleep object it does not lower AGI.
                    stats.status |= status::ASLEEP;
                    events.push(BattleEvent::FellAsleep { actor, target });
                }
            }
            34 => {
                let stats = &mut roster.get_mut(target).expect("present").stats;
                // SkillObj_Crash runs GetSkillEffectAndRange once. Effect 2
                // uses STR vs STR and destroy resistance; loc_3B82C later
                // clears successful targets' HP, without physical damage.
                if skill.resistance != 0
                    && calculate_chances(
                        power as i16,
                        stat(stats, skill.resistance) as i16,
                        i16::from(stats.element_factor(skill.element).unwrap_or(0)),
                        i16::from(skill.power),
                        i16::from(skill.effect),
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
                } else {
                    stats.curr_hp = 0;
                    stats.status |= status::DEAD;
                    events.push(BattleEvent::Died { fighter: target });
                    died.push(target);
                }
            }
            47 => {
                let stats = &mut roster.get_mut(target).expect("present").stats;
                if skill.resistance != 0
                    && calculate_chances(
                        power as i16,
                        stat(stats, skill.resistance) as i16,
                        i16::from(stats.element_factor(skill.element).unwrap_or(0)),
                        i16::from(skill.power),
                        i16::from(skill.effect),
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
                stats.dexterity.battle = stats.dexterity.modified.wrapping_add(power as u8);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Dexterity,
                    value: stats.dexterity.battle.into(),
                });
            }
            _ => unreachable!("dispatcher checked before payment"),
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
