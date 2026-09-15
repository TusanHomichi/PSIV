//! Field TECH/SKILL: learned-slot order, confirmation costs and recovery.
//! Win_TechUsedMsg, loc_61AD4 and GetItemTechSkillEffect in the retail field loop.

use super::{CampUseResult, Runtime, character_name};
use psiv_core::battle::status;
use psiv_core::{CharId, field_healing};

/// The resource and learned-slot table used by a camp command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampAbilityKind {
    /// Learned techniques spend TP.
    Technique,
    /// Learned skills spend one use from their own slot.
    Skill,
}

/// One learned field ability, safe for a renderer to browse without mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampAbility {
    /// Which menu owns the command.
    pub kind: CampAbilityKind,
    /// Cartridge ability id.
    pub id: u8,
    /// Display name.
    pub name: String,
    /// TP cost for TECH, one use for SKILL.
    pub cost: u8,
    /// Caster's current TP or this skill's uses.
    pub remaining: u16,
    /// Raw target byte from the pack.
    pub targeting: u8,
    /// The field dispatcher is implemented.
    pub supported: bool,
}

impl CampAbility {
    /// Whether confirming requires another party-member selection.
    #[must_use]
    pub fn needs_target(&self) -> bool {
        matches!(self.targeting & 15, 4 | 6 | 8)
    }
}

impl Runtime {
    /// Win_LoadTechList/Win_SkillList scan learned slots in cartridge order
    /// and show only records with usability bit 5 set. Browsing spends nothing.
    #[must_use]
    pub fn camp_abilities(&self, party_slot: usize, kind: CampAbilityKind) -> Vec<CampAbility> {
        let Some(set) = self.battles.as_ref() else {
            return Vec::new();
        };
        let Some(stats) = self
            .game
            .party_slot(party_slot)
            .and_then(|id| self.game.roster().get(id))
        else {
            return Vec::new();
        };
        match kind {
            CampAbilityKind::Technique => stats
                .techniques
                .iter()
                .filter_map(|&id| {
                    let tech = set.data.techniques().find(|tech| tech.id == id)?;
                    (tech.targeting & 0x20 != 0).then(|| CampAbility {
                        kind,
                        id,
                        name: tech.name.clone(),
                        cost: tech.cost,
                        remaining: stats.curr_tp,
                        targeting: tech.targeting,
                        supported: (matches!(tech.effect, 18..=22)
                            && matches!(tech.targeting & 15, 4 | 5))
                            || (matches!(id, crate::HINAS | crate::RYUKA)
                                && self.data.travel().is_some()),
                    })
                })
                .collect(),
            CampAbilityKind::Skill => stats
                .skills
                .iter()
                .enumerate()
                .filter_map(|(slot, &id)| {
                    let skill = set.data.skills().find(|skill| skill.id == id)?;
                    (skill.targeting & 0x20 != 0).then(|| CampAbility {
                        kind,
                        id,
                        name: skill.name.clone(),
                        cost: 1,
                        remaining: u16::from(stats.curr_skill_uses[slot]),
                        targeting: skill.targeting,
                        supported: matches!((id, skill.effect), (42..=44 | 54, 18) | (45, 23)),
                    })
                })
                .collect(),
        }
    }

    /// Confirm one learned field command. Invalid commands spend no resource
    /// or RNG; an eligible no-effect target still costs the confirmed use.
    pub fn use_camp_ability(
        &mut self,
        kind: CampAbilityKind,
        caster_slot: usize,
        id: u8,
        target_slot: usize,
    ) -> CampUseResult {
        let unavailable = |reason: &str| CampUseResult::Unavailable {
            reason: reason.to_owned(),
        };
        let Some(ability) = self
            .camp_abilities(caster_slot, kind)
            .into_iter()
            .find(|ability| ability.id == id)
        else {
            return unavailable("ABILITY NOT LEARNED");
        };
        if !ability.supported {
            return unavailable("FIELD EFFECT NOT READY");
        }
        if kind == CampAbilityKind::Technique && matches!(id, crate::RYUKA | crate::HINAS) {
            return unavailable("USE THE TRAVEL MENU");
        }
        let caster_id = self
            .game
            .party_slot(caster_slot)
            .expect("learned ability has caster");
        let stats = self
            .game
            .roster()
            .get(caster_id)
            .expect("learned ability has record");
        // loc_5FF98 checks death and paralysis only; field TECH ignores seal.
        if stats.status & status::DEAD != 0 {
            return unavailable("CASTER IS DOWN");
        }
        if stats.status & status::PARALYZED != 0 {
            return unavailable("CASTER IS PARALYZED");
        }
        if ability.remaining < u16::from(ability.cost) {
            return unavailable(if kind == CampAbilityKind::Technique {
                "NOT ENOUGH TP"
            } else {
                "NO USES LEFT"
            });
        }
        let set = self.battles.as_ref().expect("ability has catalog");
        let (effect, mental, power, skill_slot) = match kind {
            CampAbilityKind::Technique => {
                let tech = set.data.techniques().find(|tech| tech.id == id).unwrap();
                (tech.effect, stats.mental.modified, tech.power, None)
            }
            CampAbilityKind::Skill => {
                let skill = set.data.skills().find(|skill| skill.id == id).unwrap();
                let mental = match skill.power_stat & 15 {
                    1 => stats.strength.modified,
                    2 => stats.mental.modified,
                    3 => stats.agility.modified,
                    4 => stats.dexterity.modified,
                    _ => return unavailable("SKILL STAT UNAVAILABLE"),
                };
                (
                    skill.effect,
                    mental,
                    skill.power,
                    stats.skills.iter().position(|&known| known == id),
                )
            }
        };
        let group = matches!(ability.targeting & 15, 5 | 7 | 9);
        let self_only = kind == CampAbilityKind::Skill && matches!(id, 42 | 54);
        let target_slot = if self_only { caster_slot } else { target_slot };
        let targets: Vec<Option<CharId>> = if group {
            (0..5).map(|slot| self.game.party_slot(slot)).collect()
        } else {
            let Some(target) = self.game.party_slot(target_slot) else {
                return unavailable("NO PARTY MEMBER");
            };
            vec![Some(target)]
        };
        let target_label = if group {
            "PARTY".to_owned()
        } else {
            character_name(set, targets[0].unwrap())
        };
        let caster = self.game.roster_mut().get_mut(caster_id).unwrap();
        match kind {
            CampAbilityKind::Technique => caster.curr_tp -= u16::from(ability.cost),
            CampAbilityKind::Skill => caster.curr_skill_uses[skill_slot.unwrap()] -= 1,
        }
        let mask = match effect {
            18 => 0x0F,
            19 => 0x0E,
            20 => 0x0D,
            _ => 0,
        };
        let mut changed = false;
        let mut total = 0u16;
        for target in targets {
            // Both group loops calculate before testing their first $FF slot,
            // and also before skipping full, dead or ineligible targets.
            let healing = match effect {
                18 | 23 => field_healing(mental, u16::from(power), &mut self.rng),
                22 => 999,
                _ => 0,
            };
            let Some(target) = target else { break };
            let target = self
                .game
                .roster_mut()
                .get_mut(target)
                .expect("party roster initialized");
            let dead = target.status & status::DEAD != 0;
            let wrong_kind = match kind {
                CampAbilityKind::Technique => target.is_android(),
                CampAbilityKind::Skill if self_only => !target.is_android(),
                CampAbilityKind::Skill if id == 45 => target.is_android(),
                // Medice and Miracle's field routines also heal androids.
                CampAbilityKind::Skill => false,
            };
            if wrong_kind || (effect == 21 && !dead) || (dead && !matches!(effect, 21..=23)) {
                continue;
            }
            let before = (target.curr_hp, target.status);
            let healing = if effect == 21 {
                target.max_hp / 4
            } else {
                healing
            };
            target.curr_hp = target.curr_hp.wrapping_add(healing).min(target.max_hp);
            target.status &= mask;
            total = total.saturating_add(target.curr_hp.saturating_sub(before.0));
            changed |= before != (target.curr_hp, target.status);
        }
        if changed {
            CampUseResult::Used {
                item_name: ability.name,
                character_name: target_label,
                amount: total,
            }
        } else {
            CampUseResult::NoEffect {
                item_name: ability.name,
                character_name: target_label,
                reason: "NO EFFECT".to_owned(),
            }
        }
    }
}
