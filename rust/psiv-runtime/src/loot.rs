//! FieldRoutine_ItemFound: object input, blocking grant and full-pack return.
use crate::Runtime;
use psiv_core::{Chest, ChestOutcome, Direction};
use std::collections::BTreeMap;

pub(super) struct LootItem {
    name: String,
    necessary: bool,
}

pub(super) fn catalog(files: &psiv_data::BattleFiles) -> BTreeMap<u8, LootItem> {
    files
        .equipment
        .items
        .iter()
        .map(|item| {
            (
                item.id,
                LootItem {
                    name: item
                        .display_name
                        .clone()
                        .unwrap_or_else(|| item.symbol.clone()),
                    // loc_67ABA: type 9 or price zero cannot be discarded. Old packs
                    // without the word remain protected until they are regenerated.
                    necessary: item.kind.id == 9 || item.meseta_cost.is_none_or(|price| price == 0),
                },
            )
        })
        .collect()
}

/// The visible chest transaction. The runtime owns the grant and its flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootState {
    /// Slot in the shared NPC/chest pool.
    pub object_slot: usize,
    /// What opening the chest did, or the item still waiting for space.
    pub outcome: ChestOutcome,
    /// Cartridge item display name; empty for meseta and an already-open lid.
    pub item_name: String,
    /// Retail uses "Box" for a white chest.
    pub white: bool,
}

pub(super) struct PendingLoot {
    chest: Chest,
    state: LootState,
}

/// Result of a full-pack discard command; failures leave both stores intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootResult {
    /// The first copy was removed, remaining items shifted, and loot appended.
    Took {
        /// The treasure item.
        item: u8,
        /// The inventory item given up.
        discarded: u8,
    },
    /// Retail's "This item may become necessary" guard.
    Necessary,
    /// No waiting item or no item in that slot.
    Invalid,
}

impl Runtime {
    /// A pending chest window. Field input and event triggers remain parked.
    pub fn loot_state(&self) -> Option<&LootState> {
        self.loot.as_ref().map(|loot| &loot.state)
    }

    pub(super) fn start_chest_interaction(&mut self, slot: usize) -> bool {
        let Some(chest) = self.map.chest_at_slot(slot).copied() else {
            return false;
        };
        let outcome = self.game.open_chest(&chest);
        if outcome != ChestOutcome::AlreadyOpen {
            self.map.open_chest_at_slot(slot);
        }
        let item = match outcome {
            ChestOutcome::Took { item, .. } | ChestOutcome::Full { item } => Some(item),
            _ => None,
        };
        let name = item
            .and_then(|item| self.battles.as_ref()?.loot.get(&item))
            .map(|item| item.name.clone())
            .unwrap_or_default();
        self.loot = Some(PendingLoot {
            chest,
            state: LootState {
                object_slot: slot,
                outcome,
                item_name: name,
                white: chest.white,
            },
        });
        true
    }

    /// Acknowledge a completed grant. Full packs require an explicit decision.
    pub fn acknowledge_loot(&mut self) -> bool {
        if self
            .loot_state()
            .is_none_or(|loot| matches!(loot.outcome, ChestOutcome::Full { .. }))
        {
            return false;
        }
        self.loot = None;
        // Finding Alshline observes the chest flag after the window closes.
        self.scene_triggers_pending = true;
        true
    }

    /// Return the waiting item to its chest, closing the lid without a flag.
    pub fn return_loot(&mut self) -> bool {
        let Some(loot) = self.loot.as_ref() else {
            return false;
        };
        if !matches!(loot.state.outcome, ChestOutcome::Full { .. }) {
            return false;
        }
        let _ = self
            .map
            .set_npc_facing(loot.state.object_slot, Direction::Down);
        self.loot = None;
        true
    }

    /// loc_67ABA / loc_67C5C: protect plot items, discard the first matching
    /// inventory byte, compact once, then put the found item at the end.
    pub fn discard_for_loot(&mut self, slot: usize) -> LootResult {
        let Some(loot) = self.loot.as_ref() else {
            return LootResult::Invalid;
        };
        let ChestOutcome::Full { item } = loot.state.outcome else {
            return LootResult::Invalid;
        };
        let Some(discarded) = self.game.inventory().get(slot) else {
            return LootResult::Invalid;
        };
        if self
            .battles
            .as_ref()
            .and_then(|set| set.loot.get(&discarded))
            .is_none_or(|item| item.necessary)
        {
            return LootResult::Necessary;
        }
        let chest = loot.chest;
        let Ok(outcome) = self.game.complete_chest_swap(&chest, slot) else {
            return LootResult::Invalid;
        };
        self.loot.as_mut().expect("pending chest").state.outcome = outcome;
        LootResult::Took { item, discarded }
    }
}
