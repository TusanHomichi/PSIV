//! Shared party support effects: AbilityEffectsOffs (ps4.asm:9036-9086),
//! Effect_DoTechnique / Effect_DoSkill (9546-9610). Animation-owned death,
//! sleep and recovery remain with the command's dispatcher.

use super::technique::stat;
use super::{
    BattleEvent, FighterId, Rolls, Stats, TechniqueStat, Verdict, calculate_chances, status,
};

/// Parameters already loaded from the command's decoded record.
pub(super) struct Effect {
    pub effect: u8,
    pub power: u16,
    pub threshold: u8,
    pub resistance: u8,
    pub element: u8,
}

/// The effect pass's short circuits and chance draw. A zero resistance byte
/// bypasses RNG; sleep and sealing short circuit before that test.
pub(super) fn lands(stats: &Stats, effect: &Effect, rolls: &mut impl Rolls) -> bool {
    if (effect.effect == 7 && stats.status & (status::ASLEEP | status::PARALYZED) != 0)
        || (effect.effect == 8 && stats.status & status::TECH_SEALED != 0)
    {
        return false;
    }
    effect.resistance == 0
        || calculate_chances(
            effect.power as i16,
            stat(stats, effect.resistance) as i16,
            i16::from(stats.element_factor(effect.element).unwrap_or(0)),
            i16::from(effect.threshold),
            i16::from(effect.effect),
            rolls,
        ) != Verdict::Miss
}

/// Successful stat and sealing effects. Retail writes from the original
/// modified/derived value, so recasting replaces rather than stacks a buff.
pub(super) fn support(
    stats: &mut Stats,
    actor: FighterId,
    target: FighterId,
    effect: &Effect,
    events: &mut Vec<BattleEvent>,
) {
    let power = effect.power;
    let (stat, value) = match effect.effect {
        3 => {
            stats.attack.battle = stats.attack.derived.saturating_sub(power);
            (TechniqueStat::Attack, stats.attack.battle)
        }
        6 => {
            // SUB.B and BHI, not a widened or saturating subtraction
            // (AbilityEffect_AgilityDown, ps4.asm:9139-9150).
            stats.agility.battle = if stats.agility.modified > power as u8 {
                stats.agility.modified.wrapping_sub(power as u8)
            } else {
                1
            };
            (TechniqueStat::Agility, stats.agility.battle.into())
        }
        8 => {
            stats.status |= status::TECH_SEALED;
            events.push(BattleEvent::StatusInflicted {
                actor,
                target,
                status: status::TECH_SEALED,
            });
            return;
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
            stats.mental_defence.battle = stats.mental_defence.derived.wrapping_add(power);
            (TechniqueStat::MentalDefence, stats.mental_defence.battle)
        }
        12 => {
            stats.agility.battle = stats.agility.modified.wrapping_add(power as u8);
            (TechniqueStat::Agility, stats.agility.battle.into())
        }
        13 => {
            // AbilityEffect_ElementResistanceUp adds byte 3 to $2E WITHOUT
            // doubling (ps4.asm:9265-9276). FEEVE's odd selector changes a
            // shadow, not the live resistance. Keep the retail byte bug.
            let offset = usize::from(effect.threshold);
            let index = offset / 2 - 1;
            let shadow = offset & 1 != 0;
            if shadow {
                stats.element_shadow[index] = 1;
            } else {
                stats.element_props[index] = 1;
            }
            events.push(BattleEvent::ResistanceChanged {
                actor,
                target,
                element: (index + 1) as u8,
                shadow,
                value: 1,
            });
            return;
        }
        38 => {
            stats.dexterity.battle = stats.dexterity.modified.wrapping_add(power as u8);
            (TechniqueStat::Dexterity, stats.dexterity.battle.into())
        }
        _ => unreachable!("support effect dispatcher"),
    };
    events.push(BattleEvent::StatChanged {
        actor,
        target,
        stat,
        value,
    });
}
