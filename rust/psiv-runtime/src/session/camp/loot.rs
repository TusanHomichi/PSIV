//! The chest windows: the result line, and the full-pack discard and give-up
//! confirmations (`FieldRoutine_ItemFound`).

use psiv_core::ChestOutcome;

use crate::{LootResult, Runtime};

use super::{CampPage, CampView, MenuInput, wrap};

impl CampView {
    /// Opens the chest or full-pack battle-drop window over the camp's own state.
    pub(crate) fn open_loot(runtime: &Runtime) -> CampView {
        let mut view = CampView::open(runtime);
        view.show_loot_message(runtime);
        view.item_selection = 0;
        if let Some(loot) = runtime.loot_state() {
            if loot.battle_drop {
                // Battle narration already displayed "<ITEM> found! But itempack full!"
                // and waited for a confirm press (`loc_3DE0` -> `loc_3BEC`), so open
                // straight onto the discard picker without the chest-lid SFX.
                view.page = CampPage::LootItems;
            } else if loot.outcome != ChestOutcome::AlreadyOpen {
                view.sound_request = Some(0xE1);
            }
        }
        view
    }

    fn show_loot_message(&mut self, runtime: &Runtime) {
        let Some(loot) = runtime.loot_state() else {
            self.close();
            return;
        };
        if loot.battle_drop {
            self.message = match loot.outcome {
                ChestOutcome::Full { .. } => {
                    format!("{} found!\nBut itempack full!", loot.item_name)
                }
                ChestOutcome::Took { .. } => format!("{} is procured!", loot.item_name),
                ChestOutcome::Meseta { amount } => format!("{amount} meseta is procured!"),
                ChestOutcome::AlreadyOpen => String::new(),
            };
            self.page = CampPage::LootMessage;
            return;
        }
        let opened = if loot.white {
            "Box has been opened!"
        } else {
            "Chest has been opened!"
        };
        self.message = match loot.outcome {
            ChestOutcome::AlreadyOpen => "It is already open.".into(),
            ChestOutcome::Took { .. } => format!("{opened}\n{} is procured!", loot.item_name),
            ChestOutcome::Meseta { amount } => format!("{opened}\n{amount} meseta is procured!"),
            ChestOutcome::Full { .. } => format!("{opened}\nItempack is full!\n{}", loot.item_name),
        };
        self.page = CampPage::LootMessage;
    }

    pub(super) fn loot_frame(&mut self, runtime: &mut Runtime, input: MenuInput) {
        let MenuInput {
            up,
            down,
            left,
            right,
            accept,
            cancel,
            ..
        } = input;
        match self.page {
            CampPage::LootMessage if accept || cancel => {
                if runtime.acknowledge_loot() {
                    self.close();
                } else {
                    self.page = CampPage::LootItems;
                }
            }
            CampPage::LootBlocked if accept || cancel => {
                self.show_loot_message(runtime);
                self.page = CampPage::LootItems;
            }
            CampPage::LootItems => {
                let len = self.snapshot.inventory.len();
                if up {
                    self.item_selection = wrap(self.item_selection, len, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, len, true);
                } else if left {
                    self.item_selection = self.item_selection.saturating_sub(8);
                } else if right {
                    self.item_selection = (self.item_selection + 8).min(len.saturating_sub(1));
                } else if cancel {
                    self.page = CampPage::LootReturnConfirm;
                    self.confirm_selection = 1; // NO is the cartridge's default.
                } else if accept && len > 0 {
                    self.page = CampPage::LootDiscardConfirm;
                    self.confirm_selection = 1;
                }
            }
            CampPage::LootReturnConfirm | CampPage::LootDiscardConfirm => {
                if up || down {
                    self.confirm_selection ^= 1;
                } else if cancel || (accept && self.confirm_selection == 1) {
                    self.page = CampPage::LootItems;
                } else if accept {
                    if self.page == CampPage::LootReturnConfirm {
                        let battle_drop = runtime.loot_state().is_some_and(|loot| loot.battle_drop);
                        if runtime.return_loot() {
                            if !battle_drop {
                                self.sound_request = Some(0xE1);
                            }
                            self.close();
                        }
                    } else if let Some(item) = self.snapshot.inventory.get(self.item_selection) {
                        let name = item.name.clone();
                        match runtime.discard_for_loot(item.slot) {
                            LootResult::Took { .. } => {
                                self.show_loot_message(runtime);
                                let found = runtime
                                    .loot_state()
                                    .map(|loot| loot.item_name.as_str())
                                    .unwrap_or("");
                                self.message = format!("{name} discarded\n{found} is procured.");
                            }
                            LootResult::Necessary => {
                                self.message = "This item may\nbecome necessary.".into();
                                self.page = CampPage::LootBlocked;
                            }
                            LootResult::Invalid => self.page = CampPage::LootItems,
                        }
                    }
                }
            }
            _ => {}
        }
        self.sync(runtime);
    }
}
