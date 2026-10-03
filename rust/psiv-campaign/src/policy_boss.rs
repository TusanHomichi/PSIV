//! The `fight_to_win` policy: what a player does against a boss.
//!
//! The default policy attacks with everyone, which loses to a boss whose
//! group techniques cut a party down in four rounds. A player fights a boss by
//! trying the strongest thing each member has and keeping the best-looking
//! damage. This policy does that:
//!
//! 1. Healing first: the most hurt member under half HP gets the cheapest
//!    sufficient single-target cure, and a learned group cure covers two or
//!    more eligible humans below 70% HP when one is available. The group cure
//!    may follow an earlier actor's single cure in the same round. With no
//!    technique the default policy's item cure applies. A sealed caster skips
//!    TECH as a player who can see the seal would, though the retail menu
//!    still lets the player select and waste it.
//! 2. Otherwise the damage action with the highest estimated damage to the
//!    first living enemy: the plain attack, a damaging technique the actor can
//!    afford, or a damaging skill with uses left. An all-enemy action counts
//!    once per living enemy.
//!
//! The estimate is the cartridge's damage formula taken at its mean roll
//! (`Battle_CalculateDamage`, `ps4.asm:17374`: sixteen draws of 0..7, mean 56)
//! on the live fighters' stats and element factors. A player learns the same
//! ranking from the damage numbers the first rounds show; the policy only
//! skips the experiment. It reads the roster through the runtime's read-only
//! `battle_roster`, never a mutator.

use psiv_core::battle::{FighterId, Stats, status, technique_targets};
use psiv_runtime::{CommandMenuView, Runtime};

use crate::policy::{DefaultPolicy, Intent, Policy, hurt_below, most_hurt_targetable, targetable};

/// A player uses a group cure early after shared damage, before the next
/// all-party hit can kill a member. This is policy timing, not an engine rule.
const GROUP_CURE_PERCENT: u32 = 70;

/// The mean of `S + 8` in the damage formula: sixteen draws of 0..7 average 56.
const MEAN_S_PLUS_8: u32 = 64;

/// The cartridge's damage shape at the mean roll, clamped to at least 1.
fn estimate(attack: u16, defence: u16, factor: u16, bonus: u16) -> u32 {
    let attack = u32::from(attack);
    let raw = ((MEAN_S_PLUS_8 * attack) >> 6) + attack + 2 * u32::from(bonus);
    ((raw * u32::from(factor)) >> 2)
        .saturating_sub(u32::from(defence))
        .max(1)
}

/// The cartridge's stat selector (`technique::stat`).
fn selected(stats: &Stats, selector: u8) -> u16 {
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

fn factor(target: &Stats, element: u8) -> u16 {
    u16::from(target.element_factor(element).unwrap_or(0))
}

/// What a plain attack of this actor does, both hands counted.
fn attack_damage(actor: &Stats, target: &Stats) -> u32 {
    let hands: Vec<u16> = actor
        .weapon_elements
        .iter()
        .filter(|element| **element != 0)
        .map(|element| factor(target, *element))
        .collect();
    let best = hands.iter().copied().max().unwrap_or(2);
    let swings = u32::try_from(hands.len().max(1)).unwrap_or(1);
    swings * estimate(actor.attack.battle, target.defence.battle, best, 0)
}

/// Fight the strongest action each member has; heal as the default does.
#[derive(Debug, Default)]
pub struct BossPolicy {
    base: DefaultPolicy,
    current: Option<(u8, Intent)>,
    healed_this_round: bool,
    group_healed_this_round: bool,
    /// Run from random encounters; only scripted battles are fought.
    run_encounters: bool,
}

impl BossPolicy {
    /// The policy that runs from random encounters and fights scripted battles
    /// with the strongest action each member has.
    #[must_use]
    pub fn running() -> BossPolicy {
        BossPolicy {
            run_encounters: true,
            ..BossPolicy::default()
        }
    }

    /// The cure this actor casts now, when the round still needs one.
    fn heal(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Option<Intent> {
        let actor = FighterId::new(menu.actor?)?;
        let roster = runtime.battle_roster()?;
        let caster = &roster.get(actor)?.stats;
        let sealed = caster.status & status::TECH_SEALED != 0;
        let men = u32::from(caster.mental.battle);

        // SAR/GISAR/NASAR use target nibble 5. Use the same eligible target
        // list as the TECH menu; a group cure is worth a separate command
        // when it reaches at least two hurt allies, even after a single cure.
        if !sealed && !self.group_healed_this_round {
            let cures: Vec<_> = menu
                .techniques
                .iter()
                .filter(|entry| entry.available)
                .filter_map(|entry| {
                    let tech = runtime.battle_techniques().find(|t| t.id == entry.id)?;
                    if tech.effect != 18 || !tech.supported() || tech.targeting & 15 != 5 {
                        return None;
                    }
                    let targets = technique_targets(roster, actor, tech);
                    let hurt_targets: Vec<_> = menu
                        .party
                        .iter()
                        .filter(|member| {
                            hurt_below(member, GROUP_CURE_PERCENT) && targetable(member, &targets)
                        })
                        .collect();
                    (hurt_targets.len() >= 2).then(|| {
                        let need = hurt_targets
                            .iter()
                            .map(|member| u32::from(member.max_hp - member.hp))
                            .max()
                            .unwrap_or(0);
                        (entry.cost, tech.id, men + u32::from(tech.power), need)
                    })
                })
                .collect();
            let chosen = cures
                .iter()
                .filter(|(_, _, amount, need)| amount >= need)
                .min_by_key(|(cost, _, _, _)| *cost)
                .or_else(|| cures.iter().max_by_key(|(_, _, amount, _)| *amount));
            if let Some(&(_, id, _, _)) = chosen {
                self.group_healed_this_round = true;
                return Some(Intent::Technique { id, target: None });
            }
        }
        if self.healed_this_round {
            return None;
        }
        // `Battle_CalcHealing` at its mean roll: (MEN + MEN + 2 * POWER) / 2.
        let mut cures: Vec<(u32, u8, u32, Vec<FighterId>)> = menu
            .techniques
            .iter()
            .filter(|entry| entry.available && !sealed)
            .filter_map(|entry| {
                let tech = runtime.battle_techniques().find(|t| t.id == entry.id)?;
                (tech.effect == 18 && tech.supported() && tech.single_target()).then_some((
                    u32::from(entry.cost),
                    entry.id,
                    men + u32::from(tech.power),
                    technique_targets(roster, actor, tech),
                ))
            })
            .collect();
        let targets: Vec<_> = cures
            .iter()
            .flat_map(|(_, _, _, targets)| targets.iter().copied())
            .collect();
        let patient = most_hurt_targetable(&menu.party, &targets);
        let need = patient.map(|p| u32::from(p.max_hp.saturating_sub(p.hp)));
        cures.sort_unstable_by_key(|(cost, _, _, _)| *cost);
        let chosen = cures
            .iter()
            .filter(|(_, _, _, targets)| patient.is_some_and(|p| targetable(p, targets)))
            .find(|(_, _, amount, _)| need.is_some_and(|need| *amount >= need))
            .or_else(|| {
                cures
                    .iter()
                    .filter(|(_, _, _, targets)| patient.is_some_and(|p| targetable(p, targets)))
                    .max_by_key(|(_, _, amount, _)| *amount)
            });
        if let (Some((_, id, _, _)), Some(patient)) = (chosen, patient) {
            self.healed_this_round = true;
            return Some(Intent::Technique {
                id: *id,
                target: Some(patient.fighter),
            });
        }
        let item = self.base.heal(menu, runtime);
        self.healed_this_round |= item.is_some();
        item
    }

    fn best_damage(menu: &CommandMenuView, runtime: &Runtime) -> Intent {
        let (Some(actor), Some(roster)) = (menu.actor, runtime.battle_roster()) else {
            return Intent::Attack;
        };
        let (Some(actor_id), Some(enemy)) = (
            FighterId::new(actor),
            menu.enemies.first().copied().and_then(FighterId::new),
        ) else {
            return Intent::Attack;
        };
        let (Some(me), Some(target)) = (roster.get(actor_id), roster.get(enemy)) else {
            return Intent::Attack;
        };
        let living = u32::try_from(menu.enemies.len()).unwrap_or(1);
        let mut best = (attack_damage(&me.stats, &target.stats), Intent::Attack);
        best = Self::techniques(menu, runtime, &me.stats, &target.stats, living, best);
        best = Self::skills(menu, runtime, &me.stats, &target.stats, living, best);
        if std::env::var_os("PSIV_CAMPAIGN_TRACE").is_some() {
            eprintln!("  estimate {} -> {:?} ({})", me.name, best.1, best.0);
        }
        best.1
    }

    fn techniques(
        menu: &CommandMenuView,
        runtime: &Runtime,
        me: &Stats,
        target: &Stats,
        living: u32,
        mut best: (u32, Intent),
    ) -> (u32, Intent) {
        if me.status & status::TECH_SEALED != 0 {
            return best;
        }
        for entry in menu.techniques.iter().filter(|entry| entry.available) {
            let Some(tech) = runtime.battle_techniques().find(|t| t.id == entry.id) else {
                continue;
            };
            if tech.effect != 1 || !tech.supported() {
                continue;
            }
            let single = tech.single_target();
            let each = estimate(
                me.mental.battle.into(),
                selected(target, tech.resistance),
                factor(target, tech.element),
                tech.power.into(),
            );
            let total = if single { each } else { each * living };
            if total > best.0 {
                best = (
                    total,
                    Intent::Technique {
                        id: tech.id,
                        target: single.then(|| menu.enemies.first().copied()).flatten(),
                    },
                );
            }
        }
        best
    }

    fn skills(
        menu: &CommandMenuView,
        runtime: &Runtime,
        me: &Stats,
        target: &Stats,
        living: u32,
        mut best: (u32, Intent),
    ) -> (u32, Intent) {
        for entry in menu
            .skills
            .iter()
            .filter(|entry| entry.available && entry.remaining > 0)
        {
            let Some(skill) = runtime.battle_skills().find(|s| s.id == entry.id) else {
                continue;
            };
            // Crosscut (1) lands twice, Vortex (6) once: the two damage skills
            // the engine runs (`rust/psiv-core/src/battle/skill.rs`).
            if !skill.supported() || skill.effect != 1 || !matches!(skill.id, 1 | 6) {
                continue;
            }
            let hits = if skill.id == 1 { 2 } else { 1 };
            let single = skill.single_target();
            let each = hits
                * estimate(
                    selected(me, skill.power_stat),
                    selected(target, skill.resistance),
                    factor(target, skill.element),
                    skill.power.into(),
                );
            let total = if single { each } else { each * living };
            if total > best.0 {
                best = (
                    total,
                    Intent::Skill {
                        id: skill.id,
                        target: single.then(|| menu.enemies.first().copied()).flatten(),
                    },
                );
            }
        }
        best
    }
}

impl Policy for BossPolicy {
    fn name(&self) -> &'static str {
        if self.run_encounters {
            "run_then_win"
        } else {
            "fight_to_win"
        }
    }

    fn wants_run(&self, boss: bool) -> bool {
        self.run_encounters && !boss
    }

    fn choose(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Intent {
        let Some(actor) = menu.actor else {
            return Intent::Defend;
        };
        if let Some((who, intent)) = &self.current
            && *who == actor
        {
            return intent.clone();
        }
        let intent = self
            .heal(menu, runtime)
            .unwrap_or_else(|| Self::best_damage(menu, runtime));
        crate::policy::trace_choice(menu, actor, &intent);
        self.current = Some((actor, intent.clone()));
        intent
    }

    fn end_round(&mut self) {
        self.current = None;
        self.healed_this_round = false;
        self.group_healed_this_round = false;
        self.base.end_round();
    }

    fn refuse(&mut self) {
        if let Some((actor, _)) = self.current {
            self.current = Some((actor, Intent::Attack));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_is_the_cartridge_shape_at_the_mean_roll() {
        // ((64*60 >> 6) + 60) * 2 >> 2, minus 15: Gryz-like attack on Juza.
        assert_eq!(estimate(60, 15, 2, 0), 45);
        // A weakness (factor 3) with a power-48 technique on mental 17:
        // (17 + 17 + 96) * 3 >> 2 = 97, minus 12.
        assert_eq!(estimate(17, 12, 3, 48), 85);
        // Never below one, as `clamp_damage`.
        assert_eq!(estimate(1, 200, 2, 0), 1);
    }
}
