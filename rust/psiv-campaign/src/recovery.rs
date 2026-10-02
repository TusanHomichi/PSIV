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
//! inn. Camp healing techniques and restoratives exclude androids (the camp
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
        let mut cast = false;
        for _ in 0..MAX_CASTS {
            let Some(cure) = self.next_cure() else {
                break;
            };
            self.note(format!(
                "recovery: {} casts technique {} on {}",
                cure.caster, cure.technique, cure.patient
            ));
            self.use_technique(
                &NameOrId::Name(cure.caster),
                &NameOrId::Id(u16::from(cure.technique)),
                &NameOrId::Name(cure.patient),
            )?;
            cast = true;
        }
        Ok(cast)
    }

    /// The next cure worth casting, from the camp's own roster.
    fn next_cure(&self) -> Option<Cure> {
        let runtime = self.runtime();
        let camp = runtime.camp_state();
        let patient = camp
            .party
            .iter()
            .filter(|m| {
                m.current_hp > 0
                    && m.status & status::DEAD == 0
                    && !m.profession.eq_ignore_ascii_case("android")
                    && u32::from(m.current_hp) * 100 < u32::from(m.max_hp) * SUSTAIN_PERCENT
            })
            .min_by(|a, b| {
                (u32::from(a.current_hp) * u32::from(b.max_hp))
                    .cmp(&(u32::from(b.current_hp) * u32::from(a.max_hp)))
            })?;
        let healing: Vec<u8> = runtime
            .battle_techniques()
            .filter(|t| t.effect == 18)
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
                        a.supported && healing.contains(&a.id) && a.remaining >= u16::from(a.cost)
                    })
                    .map(move |a| (caster, a))
            })
            .min_by_key(|(_, ability)| ability.cost)
            .map(|(caster, ability)| Cure {
                caster: caster.name.clone(),
                technique: ability.id,
                patient: patient.name.clone(),
            })
    }
}
