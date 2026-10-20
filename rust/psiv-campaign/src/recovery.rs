//! Between battles: curing the party through the camp, the way a player does.
//!
//! A random battle leaves the party hurt, and the next one is a few steps away.
//! After a battle ends and the party stands at rest, the runner opens the camp
//! and casts a healing technique (battle effect 18, "heal HP") from whoever
//! knows one and has the TP, on the most hurt member, until nobody is below
//! [`SUSTAIN_PERCENT`] or nobody can cast. It uses the same TECH menu the
//! route's `use_technique` does; the cure itself, its cost and what it heals are
//! the runtime's.
//!
//! The members it cannot help (fallen, an android, or hurt with no healer's TP
//! left) are the route's to handle with an inn: a route that grinds names the
//! inn. Before a boss the route asks for `restore`, which also revives the
//! fallen (battle effect 21, `REVER`) and cures the living to full. Camp healing techniques and restoratives exclude androids (the camp
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

/// One planned cast.
struct Cure {
    caster: String,
    technique: u8,
    patient: String,
}

impl Driver {
    /// Cures the hurt through the camp. `Ok(true)` when anything was cast.
    ///
    /// # Errors
    ///
    /// A halt from the camp controller or a frame.
    pub fn recover(&mut self) -> Res<bool> {
        self.cure_until(SUSTAIN_PERCENT)
    }

    /// The `restore` objective: revives every fallen member a living one can
    /// revive, then cures the living to full. `Ok(true)` when anything was
    /// cast.
    ///
    /// # Errors
    ///
    /// A halt from the camp controller or a frame.
    pub fn restore(&mut self) -> Res<bool> {
        let mut cast = false;
        for _ in 0..MAX_CASTS {
            let Some(revive) = self.next_revive() else {
                break;
            };
            self.cast(&revive)?;
            cast = true;
        }
        Ok(self.cure_until(100)? || cast)
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

    /// The cheapest camp technique with battle effect `effect` a living member
    /// can pay for, cast on `patient`.
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
            .filter(|m| m.current_hp > 0 && m.status & status::DEAD == 0)
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
