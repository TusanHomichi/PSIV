//! Between battles: curing the party through the camp, the way a player does.
//!
//! A random battle leaves the party hurt, and the next one is a few steps away.
//! After a battle ends and the party stands at rest, the runner opens the camp
//! and, from whoever knows the technique and has the TP, first raises the
//! fallen (battle effect 21, `REVER`), then frees the paralyzed (battle effect
//! 20, `RIMPA`), then casts a healing technique (battle effect 18, "heal HP")
//! on the most hurt member until nobody is below [`SUSTAIN_PERCENT`] or nobody
//! can cast. A member who cannot act is no fighter for the next battle: field
//! paralysis lasts until about 25 steps pass without a map load
//! (`DoCharStatsUpdate`, `psiv_core::FieldStatusClock`), and a battle's return
//! is a load. It uses the same TECH menu the route's `use_technique` does; the
//! cure itself, its cost and what it heals are the runtime's, and so is who
//! may cast: the camp refuses a fallen or paralyzed caster (`loc_5FF98`,
//! [`psiv_runtime::Runtime::camp_caster_refusal`]), so the planner never
//! offers one.
//!
//! The members it cannot help (an android, or a member nobody left standing
//! can pay to raise, free or cure) are the route's to handle: a patrol takes
//! its refuge ([`Driver::needs_refuge`]), and a route that grinds names an
//! inn. Before a boss the route asks for `restore`, which cures the living to
//! full. Camp healing techniques and restoratives exclude androids (the camp
//! answers "NO EFFECT"; `Win_ItemUsedMsg`, `ps4.asm`), which only a Repair Kit
//! or a night at an inn mends.

use psiv_core::battle::status;
use psiv_runtime::CampAbilityKind;

use crate::driver::Driver;
use crate::halt::Res;
use crate::route::NameOrId;

/// A living member below this share of their maximum HP is cured between
/// battles.
pub const SUSTAIN_PERCENT: u32 = 70;

/// Casts in one recovery: a safety stop, well above any party's needs.
const MAX_CASTS: usize = 24;

/// Battle effect 18, "heal HP" (`RES`, `GIRES`, `NARES`, `SAR`, ...).
const EFFECT_HEAL: u8 = 18;

/// Battle effect 21, "revive" (`REVER`).
const EFFECT_REVIVE: u8 = 21;

/// Battle effect 20, "cure paralysis" (`RIMPA`; the field mask `$0D`,
/// `HealingEffectData`).
const EFFECT_CURE_PARALYSIS: u8 = 20;

/// One planned cast.
struct Cure {
    caster: String,
    technique: u8,
    patient: String,
}

impl Driver {
    /// Raises the fallen, frees the paralyzed and cures the hurt through the
    /// camp. `Ok(true)` when anything was cast.
    ///
    /// # Errors
    ///
    /// A halt from the camp controller or a frame.
    pub fn recover(&mut self) -> Res<bool> {
        let freed = self.free_the_fallen_and_paralyzed()?;
        Ok(self.cure_until(SUSTAIN_PERCENT)? || freed)
    }

    /// The `restore` objective: revives every fallen member a living one can
    /// revive, frees the paralyzed, then cures the living to full. `Ok(true)`
    /// when anything was cast.
    ///
    /// # Errors
    ///
    /// A halt from the camp controller or a frame.
    pub fn restore(&mut self) -> Res<bool> {
        let freed = self.free_the_fallen_and_paralyzed()?;
        Ok(self.cure_until(100)? || freed)
    }

    /// Revives every fallen member, then frees every paralyzed one, while a
    /// caster the camp accepts can pay. Revival first: a raised member stands
    /// with no ailment (the field mask of effect 21 is zero), and a freed
    /// healer can cast the cures that follow.
    fn free_the_fallen_and_paralyzed(&mut self) -> Res<bool> {
        let mut cast = false;
        for _ in 0..MAX_CASTS {
            let Some(cure) = self.next_revive().or_else(|| self.next_unparalyze()) else {
                break;
            };
            self.cast(&cure)?;
            cast = true;
        }
        Ok(cast)
    }

    /// Cures the most hurt living member while anyone is below `percent` of
    /// their maximum HP and someone can pay for a healing technique.
    fn cure_until(&mut self, percent: u32) -> Res<bool> {
        let mut cast = false;
        for _ in 0..MAX_CASTS {
            let Some(cure) = self.next_cure(percent) else {
                break;
            };
            self.cast(&cure)?;
            cast = true;
        }
        Ok(cast)
    }

    fn cast(&mut self, cure: &Cure) -> Res {
        self.decide(format!(
            "recovery: {} casts technique {} on {}",
            cure.caster, cure.technique, cure.patient
        ));
        self.use_technique(
            &NameOrId::Name(cure.caster.clone()),
            &NameOrId::Id(u16::from(cure.technique)),
            &NameOrId::Name(cure.patient.clone()),
        )
    }

    /// The next fallen member a living one can revive, by the cheapest
    /// reviving technique anyone can pay for. Androids are skipped: the camp
    /// answers NO EFFECT to a revive on one (only a Repair Kit or an inn mends
    /// it), so asking would halt the restore.
    fn next_revive(&self) -> Option<Cure> {
        let camp = self.runtime().camp_state();
        let patient = camp.party.iter().find(|m| {
            (m.current_hp == 0 || m.status & status::DEAD != 0)
                && !m.profession.eq_ignore_ascii_case("android")
        })?;
        self.cheapest(EFFECT_REVIVE, &patient.name)
    }

    /// The first living, paralyzed human someone can free. Androids are
    /// skipped as for a revive: camp TECH answers NO EFFECT on one.
    fn next_unparalyze(&self) -> Option<Cure> {
        let camp = self.runtime().camp_state();
        camp.party
            .iter()
            .filter(|m| {
                m.current_hp > 0
                    && m.status & status::DEAD == 0
                    && m.status & status::PARALYZED != 0
                    && !m.profession.eq_ignore_ascii_case("android")
            })
            .find_map(|patient| self.cheapest(EFFECT_CURE_PARALYSIS, &patient.name))
    }

    /// The next cure worth casting, from the camp's own roster.
    fn next_cure(&self, percent: u32) -> Option<Cure> {
        let runtime = self.runtime();
        let camp = runtime.camp_state();
        let patient = camp
            .party
            .iter()
            .filter(|m| {
                m.current_hp > 0
                    && m.status & status::DEAD == 0
                    && !m.profession.eq_ignore_ascii_case("android")
                    && u32::from(m.current_hp) * 100 < u32::from(m.max_hp) * percent
            })
            .min_by(|a, b| {
                (u32::from(a.current_hp) * u32::from(b.max_hp))
                    .cmp(&(u32::from(b.current_hp) * u32::from(a.max_hp)))
            })?;
        self.cheapest(EFFECT_HEAL, &patient.name)
    }

    /// The cheapest camp technique with battle effect `effect` a member the
    /// camp accepts as a caster can pay for, cast on `patient`.
    fn cheapest(&self, effect: u8, patient: &str) -> Option<Cure> {
        let runtime = self.runtime();
        let camp = runtime.camp_state();
        let wanted: Vec<u8> = runtime
            .battle_techniques()
            .filter(|t| t.effect == effect)
            .map(|t| t.id)
            .collect();
        camp.party
            .iter()
            .filter(|m| runtime.camp_caster_refusal(m.party_slot).is_none())
            .flat_map(|caster| {
                runtime
                    .camp_abilities(caster.party_slot, CampAbilityKind::Technique)
                    .into_iter()
                    .filter(|a| {
                        a.supported && wanted.contains(&a.id) && a.remaining >= u16::from(a.cost)
                    })
                    .map(move |a| (caster, a))
            })
            .min_by_key(|(_, ability)| ability.cost)
            .map(|(caster, ability)| Cure {
                caster: caster.name.clone(),
                technique: ability.id,
                patient: patient.to_owned(),
            })
    }
}
