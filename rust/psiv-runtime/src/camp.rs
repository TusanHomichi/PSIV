//! Runtime-owned state and actions for the field camp menu.
//!
//! The renderer gets a snapshot and sends item-use commands back here. It
//! never edits HP, status, inventory, or equipment itself. The catalog is
//! copied from the battle pack when the runtime arms battles, because that is
//! also the point at which the roster receives its live character records.

use std::collections::BTreeMap;

use psiv_core::battle::{self, EquipmentError};
use psiv_core::{CharId, DEAD_STATUS_MASK, GameState};
use psiv_data::BattleFiles;

use crate::{BattleSet, Runtime};

const STATUS_POISON: u8 = 0x01;
const STATUS_PARALYZED: u8 = 0x02;
const HEAL_EFFECT: u8 = 0x12;
const CURE_POISON_EFFECT: u8 = 0x13;
const CURE_PARALYSIS_EFFECT: u8 = 0x14;
const REVIVE_EFFECT: u8 = 0x15;
const FULL_HEAL_EFFECT: u8 = 0x16;

#[derive(Clone)]
pub(super) struct CampCatalog {
    names: BTreeMap<u8, String>,
    professions: BTreeMap<u8, String>,
    ages: BTreeMap<u8, u16>,
    item_names: BTreeMap<u8, String>,
    item_kinds: BTreeMap<u8, u8>,
    item_masks: BTreeMap<u8, Option<u16>>,
    effects: BTreeMap<u8, CampItemEffect>,
    levels: BTreeMap<u8, Vec<(u16, u32)>>,
}

#[derive(Clone)]
struct CampItemEffect {
    name: String,
    effect_id: u8,
    mental_power: u8,
    item_power: u16,
    targeting: u8,
}

pub(super) fn catalog(files: &BattleFiles) -> CampCatalog {
    CampCatalog {
        names: files
            .characters
            .characters
            .iter()
            .map(|character| {
                (
                    character.character_id,
                    character
                        .display_name
                        .clone()
                        .unwrap_or_else(|| character.symbol.clone()),
                )
            })
            .collect(),
        professions: files
            .characters
            .characters
            .iter()
            .map(|character| {
                (
                    character.character_id,
                    character
                        .profession
                        .name
                        .clone()
                        .unwrap_or_else(|| "UNKNOWN".to_owned()),
                )
            })
            .collect(),
        // `characters.json` does not carry an AGE field. The only decoded
        // status capture is Chaz, whose retail record displays 16.
        ages: [(0, 16)].into_iter().collect(),
        item_names: files
            .equipment
            .items
            .iter()
            .map(|item| {
                (
                    item.id,
                    item.display_name
                        .clone()
                        .unwrap_or_else(|| item.symbol.clone()),
                )
            })
            .collect(),
        item_kinds: files
            .equipment
            .items
            .iter()
            .map(|item| (item.id, item.kind.id))
            .collect(),
        item_masks: files
            .equipment
            .items
            .iter()
            .map(|item| (item.id, item.usable_by_mask()))
            .collect(),
        effects: files
            .abilities
            .item_effects
            .iter()
            .filter_map(|effect| {
                let id = u8::try_from(effect.id).ok()?;
                Some((
                    id,
                    CampItemEffect {
                        name: effect
                            .display_name
                            .clone()
                            .or_else(|| effect.symbol.clone())
                            .unwrap_or_else(|| format!("ITEM-{id:02X}")),
                        effect_id: effect.effect_id,
                        mental_power: effect.parameter_2.unwrap_or(0),
                        item_power: effect.power_or_hit_chance.unwrap_or(0),
                        targeting: effect.targeting_or_parameter_3.unwrap_or(4),
                    },
                ))
            })
            .collect(),
        levels: files
            .levels
            .characters
            .iter()
            .map(|table| {
                (
                    table.character_id,
                    table
                        .levels
                        .iter()
                        .map(|level| (level.level, level.experience_required))
                        .collect(),
                )
            })
            .collect(),
    }
}

/// A complete, renderer-safe view of the camp menu's live data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CampState {
    /// Current meseta.
    pub money: u32,
    /// Party members in party-slot order.
    pub party: Vec<CampCharacter>,
    /// Non-empty inventory slots in cartridge slot order.
    pub inventory: Vec<CampItem>,
}

/// One party member as displayed by the roster and STATUS pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampCharacter {
    /// The party slot, not merely the character id.
    pub party_slot: usize,
    /// The persistent character record id.
    pub id: u8,
    /// Display name.
    pub name: String,
    /// Profession label from the battle pack.
    pub profession: String,
    /// Decoded age, when the pack supplies or the oracle establishes one.
    pub age: Option<u16>,
    /// Current level.
    pub level: u16,
    /// Current total experience.
    pub experience: u32,
    /// Current HP.
    pub current_hp: u16,
    /// Maximum HP.
    pub max_hp: u16,
    /// Current TP.
    pub current_tp: u16,
    /// Maximum TP.
    pub max_tp: u16,
    /// Status bitfield from the persistent character record.
    pub status: u8,
    /// Equipment-modified strength.
    pub strength: u8,
    /// Equipment-modified mental.
    pub mental: u8,
    /// Equipment-modified agility.
    pub agility: u8,
    /// Equipment-modified dexterity.
    pub dexterity: u8,
    /// Derived attack power.
    pub attack_power: u16,
    /// Derived defence power.
    pub defense_power: u16,
    /// Read-only equipment names in right-hand, left-hand, head, body order.
    pub equipment: [String; 4],
    /// Experience required for the next level, when a next level exists.
    pub next_level_experience: Option<u32>,
}

/// One non-empty party inventory slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampItem {
    /// The raw inventory slot. This is the command key used by item use.
    pub slot: usize,
    /// The pack item id.
    pub id: u8,
    /// Display label.
    pub name: String,
    /// Whether the pack identifies this item as a field-consumable effect.
    pub usable: bool,
    /// The decoded item target mode (`4` single target, `5` party target in
    /// the current pack).
    pub targeting: u8,
}

/// The runtime result of a camp item command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampUseResult {
    /// The item changed the target's state.
    Used {
        /// Item display name.
        item_name: String,
        /// Target display name.
        character_name: String,
        /// HP restored, or zero for a status-only cure.
        amount: u16,
    },
    /// The cartridge consumed a recognized item whose effect had no targetable
    /// change (for example a Monomate on full HP).
    NoEffect {
        /// Item display name.
        item_name: String,
        /// Target display name.
        character_name: String,
        /// Short reason for the renderer's result line.
        reason: String,
    },
    /// The command was rejected without mutating the game state.
    Unavailable {
        /// Short reason for the renderer's result line.
        reason: String,
    },
}

/// The runtime result of an equipment command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampEquipResult {
    /// The selected inventory item was committed to the character.
    Equipped {
        /// Item display name.
        item_name: String,
        /// Character display name.
        character_name: String,
    },
    /// The selected equipment byte was returned to inventory.
    Unequipped {
        /// Item display name.
        item_name: String,
        /// Character display name.
        character_name: String,
    },
    /// The command was rejected without mutating persistent state.
    Unavailable {
        /// Short reason for the renderer's result line.
        reason: String,
    },
}

impl Runtime {
    /// Returns a copy of the live roster, inventory, equipment labels, and
    /// meseta for the camp renderer.
    #[must_use]
    pub fn camp_state(&self) -> CampState {
        let Some(set) = self.battles.as_ref() else {
            return CampState {
                money: self.game.money(),
                ..CampState::default()
            };
        };
        CampState {
            money: self.game.money(),
            party: party_snapshot(&self.game, set),
            inventory: inventory_snapshot(&self.game, set),
        }
    }

    /// Returns the cartridge-filtered equipment list for one party slot.
    ///
    /// The returned `slot` values are raw inventory positions, so duplicate
    /// item ids remain independently selectable just as they are in the
    /// cartridge's forty-byte list.
    #[must_use]
    pub fn camp_equipment(&self, party_slot: usize) -> Vec<CampItem> {
        let Some(set) = self.battles.as_ref() else {
            return Vec::new();
        };
        let Some(character_id) = self.game.party_slot(party_slot) else {
            return Vec::new();
        };
        let item = |item_id| set.data.item(item_id).ok().cloned();
        let mask = |item_id| set.camp.item_masks.get(&item_id).copied().flatten();
        battle::equipment_candidates(self.game.inventory(), character_id.0, &item, &mask)
            .into_iter()
            .map(|candidate| CampItem {
                slot: candidate.inventory_slot,
                id: candidate.item_id,
                name: set
                    .camp
                    .item_names
                    .get(&candidate.item_id)
                    .cloned()
                    .unwrap_or_else(|| format!("ITEM-{:02X}", candidate.item_id)),
                usable: false,
                targeting: 0,
            })
            .collect()
    }

    /// Equips one filtered inventory entry through the core transaction seam.
    pub fn equip_camp_item(&mut self, party_slot: usize, inventory_slot: usize) -> CampEquipResult {
        let Some(set) = self.battles.as_ref() else {
            return CampEquipResult::Unavailable {
                reason: "EQUIPMENT DATA UNAVAILABLE".to_owned(),
            };
        };
        let Some(character_id) = self.game.party_slot(party_slot) else {
            return CampEquipResult::Unavailable {
                reason: "NO PARTY MEMBER".to_owned(),
            };
        };
        let Some(mut stats) = self.game.roster().get(character_id).cloned() else {
            return CampEquipResult::Unavailable {
                reason: "ROSTER RECORD MISSING".to_owned(),
            };
        };
        let Some(item_id) = self.game.inventory().get(inventory_slot) else {
            return CampEquipResult::Unavailable {
                reason: "ITEM SLOT EMPTY".to_owned(),
            };
        };
        let item_name = set
            .camp
            .item_names
            .get(&item_id)
            .cloned()
            .unwrap_or_else(|| format!("ITEM-{item_id:02X}"));
        let character_name = character_name(set, character_id);
        let mut inventory = self.game.inventory().clone();
        let item = |id| set.data.item(id).ok().cloned();
        let mask = |id| set.camp.item_masks.get(&id).copied().flatten();
        match battle::equip_item(
            &mut stats,
            &mut inventory,
            character_id.0,
            inventory_slot,
            &item,
            &mask,
        ) {
            Ok(()) => {
                *self
                    .game
                    .roster_mut()
                    .get_mut(character_id)
                    .expect("roster record was cloned above") = stats;
                *self.game.inventory_mut() = inventory;
                CampEquipResult::Equipped {
                    item_name,
                    character_name,
                }
            }
            Err(error) => CampEquipResult::Unavailable {
                reason: equipment_error_message(error),
            },
        }
    }

    /// Unequips a raw `$4C..$4F` equipment slot through the core seam.
    pub fn unequip_camp_item(
        &mut self,
        party_slot: usize,
        equipment_slot: usize,
    ) -> CampEquipResult {
        let Some(set) = self.battles.as_ref() else {
            return CampEquipResult::Unavailable {
                reason: "EQUIPMENT DATA UNAVAILABLE".to_owned(),
            };
        };
        let Some(character_id) = self.game.party_slot(party_slot) else {
            return CampEquipResult::Unavailable {
                reason: "NO PARTY MEMBER".to_owned(),
            };
        };
        let Some(mut stats) = self.game.roster().get(character_id).cloned() else {
            return CampEquipResult::Unavailable {
                reason: "ROSTER RECORD MISSING".to_owned(),
            };
        };
        let Some(item_id) = stats.equipment.get(equipment_slot).copied() else {
            return CampEquipResult::Unavailable {
                reason: "EQUIPMENT SLOT INVALID".to_owned(),
            };
        };
        if item_id == 0 {
            return CampEquipResult::Unavailable {
                reason: "EQUIPMENT SLOT EMPTY".to_owned(),
            };
        }
        let item_name = set
            .camp
            .item_names
            .get(&item_id)
            .cloned()
            .unwrap_or_else(|| format!("ITEM-{item_id:02X}"));
        let character_name = character_name(set, character_id);
        let mut inventory = self.game.inventory().clone();
        let item = |id| set.data.item(id).ok().cloned();
        match battle::unequip_item(&mut stats, &mut inventory, equipment_slot, &item) {
            Ok(()) => {
                *self
                    .game
                    .roster_mut()
                    .get_mut(character_id)
                    .expect("roster record was cloned above") = stats;
                *self.game.inventory_mut() = inventory;
                CampEquipResult::Unequipped {
                    item_name,
                    character_name,
                }
            }
            Err(error) => CampEquipResult::Unavailable {
                reason: equipment_error_message(error),
            },
        }
    }

    /// Applies one field-consumable item through the persistent [`GameState`]
    /// inventory and roster APIs. Target mode `5` is the decoded all-party
    /// form used by Star Dew; single-target items use the requested party
    /// slot.
    pub fn use_camp_item(&mut self, inventory_slot: usize, party_slot: usize) -> CampUseResult {
        let Some(item_id) = self.game.inventory().get(inventory_slot) else {
            return CampUseResult::Unavailable {
                reason: "ITEM SLOT EMPTY".to_owned(),
            };
        };
        let Some(set) = self.battles.as_ref() else {
            return CampUseResult::Unavailable {
                reason: "ITEM DATA UNAVAILABLE".to_owned(),
            };
        };
        if set.camp.item_kinds.get(&item_id).copied() != Some(8) {
            return CampUseResult::Unavailable {
                reason: "NOT A CONSUMABLE".to_owned(),
            };
        }
        let Some(effect) = set.camp.effects.get(&item_id).cloned() else {
            return CampUseResult::Unavailable {
                reason: "ITEM EFFECT UNPACKED".to_owned(),
            };
        };
        if !matches!(
            effect.effect_id,
            HEAL_EFFECT
                | CURE_POISON_EFFECT
                | CURE_PARALYSIS_EFFECT
                | REVIVE_EFFECT
                | FULL_HEAL_EFFECT
        ) {
            return CampUseResult::Unavailable {
                reason: "ITEM EFFECT UNSUPPORTED".to_owned(),
            };
        }
        let Some(character_id) = self.game.party_slot(party_slot) else {
            return CampUseResult::Unavailable {
                reason: "NO PARTY MEMBER".to_owned(),
            };
        };
        let item_name = set
            .camp
            .item_names
            .get(&item_id)
            .cloned()
            .unwrap_or_else(|| effect.name.clone());
        let all_party = effect.targeting == 5;
        let target_ids: Vec<CharId> = if all_party {
            (0..self.game.party_len())
                .filter_map(|slot| self.game.party_slot(slot))
                .collect()
        } else {
            vec![character_id]
        };
        let target_label = if all_party {
            "PARTY".to_owned()
        } else {
            character_name(set, character_id)
        };
        let mut outcomes = Vec::with_capacity(target_ids.len());
        for target_id in target_ids {
            let target_name = character_name(set, target_id);
            let Some(stats) = self.game.roster_mut().get_mut(target_id) else {
                return CampUseResult::Unavailable {
                    reason: "ROSTER RECORD MISSING".to_owned(),
                };
            };
            let Some(outcome) = apply_item_effect(stats, &effect, &item_name, &target_name) else {
                return CampUseResult::Unavailable {
                    reason: "ITEM EFFECT UNSUPPORTED".to_owned(),
                };
            };
            outcomes.push(outcome);
        }
        // Retail consumes a recognized item even when the target is already
        // full or the status bit is absent. Inventory::remove deliberately
        // leaves the same hole the cartridge's forty-byte array leaves.
        let _ = self.game.inventory_mut().remove(inventory_slot);
        let mut amount: u16 = 0;
        let mut changed = false;
        let mut no_effect_reason = None;
        for outcome in outcomes {
            match outcome {
                CampUseResult::Used { amount: value, .. } => {
                    changed = true;
                    amount = amount.saturating_add(value);
                }
                CampUseResult::NoEffect { reason, .. } => {
                    no_effect_reason.get_or_insert(reason);
                }
                CampUseResult::Unavailable { reason } => {
                    return CampUseResult::Unavailable { reason };
                }
            }
        }
        if changed {
            CampUseResult::Used {
                item_name,
                character_name: target_label,
                amount,
            }
        } else {
            CampUseResult::NoEffect {
                item_name,
                character_name: target_label,
                reason: no_effect_reason.unwrap_or_else(|| "NO PARTY MEMBER".to_owned()),
            }
        }
    }
}

fn party_snapshot(game: &GameState, set: &BattleSet) -> Vec<CampCharacter> {
    (0..game.party_len())
        .filter_map(|party_slot| {
            let id = game.party_slot(party_slot)?;
            let stats = game.roster().get(id)?;
            let equipment = std::array::from_fn(|slot| {
                let item = stats.equipment[slot];
                if item == 0 {
                    "--".to_owned()
                } else {
                    set.camp
                        .item_names
                        .get(&item)
                        .cloned()
                        .unwrap_or_else(|| format!("ITEM-{item:02X}"))
                }
            });
            let next_level_experience = set
                .camp
                .levels
                .get(&id.0)
                .and_then(|levels| levels.iter().find(|(level, _)| *level > stats.level))
                .map(|(_, experience)| *experience);
            Some(CampCharacter {
                party_slot,
                id: id.0,
                name: character_name(set, id),
                profession: set
                    .camp
                    .professions
                    .get(&id.0)
                    .cloned()
                    .unwrap_or_else(|| "UNKNOWN".to_owned()),
                age: set.camp.ages.get(&id.0).copied(),
                level: stats.level,
                experience: stats.experience,
                current_hp: stats.curr_hp,
                max_hp: stats.max_hp,
                current_tp: stats.curr_tp,
                max_tp: stats.max_tp,
                status: stats.status,
                strength: stats.strength.modified,
                mental: stats.mental.modified,
                agility: stats.agility.modified,
                dexterity: stats.dexterity.modified,
                attack_power: stats.attack.derived,
                defense_power: stats.defence.derived,
                equipment,
                next_level_experience,
            })
        })
        .collect()
}

fn inventory_snapshot(game: &GameState, set: &BattleSet) -> Vec<CampItem> {
    game.inventory()
        .slots()
        .iter()
        .enumerate()
        .filter(|(_, id)| **id != 0)
        .map(|(slot, &id)| CampItem {
            slot,
            id,
            name: set
                .camp
                .item_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("ITEM-{id:02X}")),
            usable: set.camp.item_kinds.get(&id).copied() == Some(8)
                && set.camp.effects.contains_key(&id),
            targeting: set
                .camp
                .effects
                .get(&id)
                .map_or(4, |effect| effect.targeting),
        })
        .collect()
}

fn character_name(set: &BattleSet, id: CharId) -> String {
    set.camp
        .names
        .get(&id.0)
        .cloned()
        .unwrap_or_else(|| format!("CHAR-{:02X}", id.0))
}

fn equipment_error_message(error: EquipmentError) -> String {
    error.to_string().to_uppercase()
}

fn apply_item_effect(
    stats: &mut psiv_core::battle::Stats,
    effect: &CampItemEffect,
    item_name: &str,
    character_name: &str,
) -> Option<CampUseResult> {
    let result = match effect.effect_id {
        HEAL_EFFECT => {
            if stats.status & DEAD_STATUS_MASK != 0 {
                CampUseResult::NoEffect {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    reason: "TARGET IS DOWN".to_owned(),
                }
            } else if stats.curr_hp >= stats.max_hp {
                CampUseResult::NoEffect {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    reason: "HP ALREADY FULL".to_owned(),
                }
            } else {
                let amount = field_heal_amount(effect);
                let before = stats.curr_hp;
                stats.curr_hp = stats.curr_hp.saturating_add(amount).min(stats.max_hp);
                CampUseResult::Used {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    amount: stats.curr_hp.saturating_sub(before),
                }
            }
        }
        CURE_POISON_EFFECT => {
            cure_status(stats, STATUS_POISON, item_name, character_name, "NO POISON")
        }
        CURE_PARALYSIS_EFFECT => cure_status(
            stats,
            STATUS_PARALYZED,
            item_name,
            character_name,
            "NOT PARALYZED",
        ),
        REVIVE_EFFECT => {
            if stats.status & DEAD_STATUS_MASK == 0 {
                CampUseResult::NoEffect {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    reason: "TARGET IS ALIVE".to_owned(),
                }
            } else {
                stats.status &= !DEAD_STATUS_MASK;
                stats.curr_hp = (stats.max_hp / 4).max(1);
                CampUseResult::Used {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    amount: stats.curr_hp,
                }
            }
        }
        FULL_HEAL_EFFECT => {
            let before = stats.curr_hp;
            let changed = stats.curr_hp != stats.max_hp || stats.status != 0;
            stats.curr_hp = stats.max_hp;
            stats.status = 0;
            if changed {
                CampUseResult::Used {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    amount: stats.curr_hp.saturating_sub(before),
                }
            } else {
                CampUseResult::NoEffect {
                    item_name: item_name.to_owned(),
                    character_name: character_name.to_owned(),
                    reason: "TARGET ALREADY FULL".to_owned(),
                }
            }
        }
        _ => return None,
    };
    Some(result)
}

fn cure_status(
    stats: &mut psiv_core::battle::Stats,
    bit: u8,
    item_name: &str,
    character_name: &str,
    reason: &str,
) -> CampUseResult {
    if stats.status & bit == 0 {
        CampUseResult::NoEffect {
            item_name: item_name.to_owned(),
            character_name: character_name.to_owned(),
            reason: reason.to_owned(),
        }
    } else {
        stats.status &= !bit;
        CampUseResult::Used {
            item_name: item_name.to_owned(),
            character_name: character_name.to_owned(),
            amount: 0,
        }
    }
}

/// The field menu's healing routine uses the two item power bytes. The
/// cartridge rolls sixteen 0..7 values; the fixed midpoint keeps a renderer
/// action deterministic while preserving the decoded scaling and clamps.
fn field_heal_amount(effect: &CampItemEffect) -> u16 {
    let midpoint_roll_sum = 56u32;
    let mental = u32::from(effect.mental_power);
    let amount =
        (((midpoint_roll_sum + 8) * mental) >> 7) + mental / 2 + u32::from(effect.item_power);
    amount.min(u32::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_monomate_power_has_the_retail_midpoint_amount() {
        let effect = CampItemEffect {
            name: "MONOMATE".to_owned(),
            effect_id: HEAL_EFFECT,
            mental_power: 24,
            item_power: 24,
            targeting: 4,
        };
        assert_eq!(field_heal_amount(&effect), 48);
    }
}
