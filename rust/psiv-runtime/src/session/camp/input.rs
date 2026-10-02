//! The camp's page dispatch: one frame of menu input and the commands it fires.

use crate::{CampAbilityKind, CampUseResult, Runtime};

use super::{CampPage, CampView, MenuInput, ROOT_OPTIONS, wrap};

impl CampView {
    /// Handles one frame of camp input and the runtime commands it fires.
    pub(crate) fn frame(&mut self, runtime: &mut Runtime, input: MenuInput) {
        if self.is_loot() {
            self.loot_frame(runtime, input);
            return;
        }
        if self.page == CampPage::TravelReady {
            if input.cancel || input.accept || input.start {
                self.complete_travel(runtime);
            }
            return;
        }
        // Start dismisses a result line like any button; on a cursor page it
        // closes the whole menu.
        let input = if input.start && self.is_result_page() {
            MenuInput {
                accept: true,
                ..input
            }
        } else {
            input
        };
        if input.start && !self.is_result_page() {
            self.close();
            return;
        }
        if input.cancel {
            if self.go_back() {
                self.close();
                return;
            }
            self.sync(runtime);
            return;
        }
        let MenuInput {
            up, down, accept, ..
        } = input;
        match self.page {
            CampPage::AbilityCharacters
            | CampPage::AbilityList
            | CampPage::AbilityTarget
            | CampPage::AbilityResult
            | CampPage::TravelTowns => {
                self.ability_frame(runtime, up, down, accept);
            }
            CampPage::Root => {
                if up {
                    self.root_selection = wrap(self.root_selection, ROOT_OPTIONS.len(), false);
                } else if down {
                    self.root_selection = wrap(self.root_selection, ROOT_OPTIONS.len(), true);
                } else if accept {
                    self.confirm_root(runtime);
                }
            }
            CampPage::ItemEmpty => {
                if accept {
                    self.page = CampPage::Root;
                }
            }
            CampPage::ItemList => {
                if self.snapshot.inventory.is_empty() {
                    self.page = CampPage::ItemEmpty;
                } else if up {
                    self.item_selection =
                        wrap(self.item_selection, self.snapshot.inventory.len(), false);
                } else if down {
                    self.item_selection =
                        wrap(self.item_selection, self.snapshot.inventory.len(), true);
                } else if accept {
                    self.confirm_item(runtime);
                }
            }
            CampPage::ItemTarget => {
                if self.snapshot.party.is_empty() {
                    self.page = CampPage::ItemResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.target_selection =
                        wrap(self.target_selection, self.snapshot.party.len(), false);
                } else if down {
                    self.target_selection =
                        wrap(self.target_selection, self.snapshot.party.len(), true);
                } else if accept {
                    self.use_selected_item(runtime);
                }
            }
            CampPage::ItemResult => {
                if accept {
                    self.page = if self.snapshot.inventory.is_empty() {
                        CampPage::Root
                    } else {
                        CampPage::ItemList
                    };
                }
            }
            CampPage::EquipCharacters
            | CampPage::EquipStats
            | CampPage::EquipItems
            | CampPage::EquipResult
            | CampPage::EquipHands => self.equipment_frame(runtime, up, down, accept),
            CampPage::State => {
                if up {
                    self.state_selection = wrap(self.state_selection, 3, false);
                } else if down {
                    self.state_selection = wrap(self.state_selection, 3, true);
                } else if accept {
                    if self.state_selection == 0 {
                        self.status_selection = 0;
                        self.status_from_state = true;
                        self.page = CampPage::Status;
                    } else if self.state_selection == 1 {
                        self.begin_order();
                    } else {
                        self.save_selection = 0;
                        self.page = CampPage::SaveSlots;
                    }
                }
            }
            CampPage::Order | CampPage::OrderDone | CampPage::OrderAlone => {
                self.order_frame(runtime, up, down, accept);
            }
            CampPage::Status => {
                if self.snapshot.party.is_empty() {
                    self.page = CampPage::State;
                } else if up {
                    self.status_selection =
                        wrap(self.status_selection, self.snapshot.party.len(), false);
                } else if down {
                    self.status_selection =
                        wrap(self.status_selection, self.snapshot.party.len(), true);
                }
            }
            CampPage::Unsupported => {
                if accept {
                    self.page = CampPage::Root;
                }
            }
            CampPage::SaveSlots => {
                if up {
                    self.save_selection = wrap(self.save_selection, 3, false);
                } else if down {
                    self.save_selection = wrap(self.save_selection, 3, true);
                } else if accept {
                    // The write is the session's: it owns the save directory.
                    // The result line arrives with `finish_save`.
                    self.save_slot_request = Some(self.save_selection);
                    self.message.clear();
                    self.page = CampPage::SaveResult;
                }
            }
            CampPage::SaveResult => {
                if accept {
                    self.page = CampPage::State;
                }
            }
            CampPage::TravelReady
            | CampPage::LootMessage
            | CampPage::LootItems
            | CampPage::LootDiscardConfirm
            | CampPage::LootReturnConfirm
            | CampPage::LootBlocked => {}
        }
        self.sync(runtime);
    }

    /// A page that only shows a result line, which any button dismisses.
    fn is_result_page(&self) -> bool {
        matches!(
            self.page,
            CampPage::ItemEmpty
                | CampPage::ItemResult
                | CampPage::AbilityResult
                | CampPage::EquipResult
                | CampPage::SaveResult
                | CampPage::OrderDone
                | CampPage::OrderAlone
                | CampPage::Unsupported
        )
    }

    /// Backs one page out; `true` when the camp closes.
    fn go_back(&mut self) -> bool {
        match self.page {
            CampPage::Order => {
                if !self.order_draft.undo() {
                    self.page = CampPage::State;
                }
                false
            }
            CampPage::OrderDone => {
                self.page = CampPage::Root;
                self.state_selection = 0;
                false
            }
            CampPage::OrderAlone => {
                self.page = CampPage::State;
                false
            }
            CampPage::AbilityCharacters => {
                self.page = CampPage::Root;
                false
            }
            CampPage::AbilityList => {
                self.page = CampPage::AbilityCharacters;
                false
            }
            CampPage::AbilityTarget | CampPage::AbilityResult => {
                self.page = CampPage::AbilityList;
                false
            }
            CampPage::TravelTowns => {
                self.page = if self.travel_item_slot.is_some() {
                    CampPage::ItemList
                } else {
                    CampPage::AbilityList
                };
                false
            }
            CampPage::TravelReady
            | CampPage::LootMessage
            | CampPage::LootItems
            | CampPage::LootDiscardConfirm
            | CampPage::LootReturnConfirm
            | CampPage::LootBlocked => false,
            CampPage::Root => true,
            CampPage::ItemEmpty
            | CampPage::ItemList
            | CampPage::ItemResult
            | CampPage::Unsupported => {
                self.page = CampPage::Root;
                false
            }
            CampPage::ItemTarget => {
                self.page = CampPage::ItemList;
                false
            }
            CampPage::EquipCharacters => {
                self.page = CampPage::Root;
                false
            }
            CampPage::EquipStats => {
                self.page = CampPage::EquipCharacters;
                false
            }
            CampPage::EquipItems => {
                self.page = CampPage::EquipStats;
                false
            }
            CampPage::EquipHands => {
                self.page = CampPage::EquipItems;
                false
            }
            CampPage::EquipResult => {
                self.page = CampPage::EquipStats;
                false
            }
            CampPage::State => {
                self.page = CampPage::Root;
                false
            }
            CampPage::Status => {
                self.page = if self.status_from_state {
                    CampPage::State
                } else {
                    CampPage::Root
                };
                false
            }
            CampPage::SaveSlots | CampPage::SaveResult => {
                self.page = CampPage::State;
                false
            }
        }
    }

    fn confirm_root(&mut self, runtime: &Runtime) {
        match self.root_selection {
            1 | 2 => {
                self.ability_kind = if self.root_selection == 1 {
                    CampAbilityKind::Technique
                } else {
                    CampAbilityKind::Skill
                };
                self.ability_character_selection = 0;
                self.ability_selection = 0;
                self.page = CampPage::AbilityCharacters;
            }
            0 => {
                self.page = if runtime.camp_state().inventory.is_empty() {
                    CampPage::ItemEmpty
                } else {
                    CampPage::ItemList
                };
            }
            3 => {
                self.equipment_character_selection = 0;
                self.equipment_slot_selection = 0;
                self.equipment_item_selection = 0;
                self.equipment_options = runtime.camp_equipment(0);
                self.page = CampPage::EquipCharacters;
            }
            4 => self.page = CampPage::State,
            _ => {
                self.page = CampPage::Unsupported;
                self.message = format!("{} NOT READY", ROOT_OPTIONS[self.root_selection]);
            }
        }
    }

    fn confirm_item(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            self.page = CampPage::ItemEmpty;
            return;
        };
        if matches!(item.id, crate::TELEPIPE | crate::ESCAPIPE) {
            self.begin_selected_item_travel(runtime);
            return;
        }
        if !item.usable {
            self.page = CampPage::ItemResult;
            self.message = format!("{} NOT USABLE", item.name);
            return;
        }
        // One member, or an item that heals the whole party (target mode 5):
        // no one to pick.
        if self.snapshot.party.len() == 1 || item.targeting == 5 {
            self.target_selection = 0;
            self.use_selected_item(runtime);
        } else {
            self.target_selection = 0;
            self.page = CampPage::ItemTarget;
        }
    }

    fn use_selected_item(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            self.page = CampPage::ItemEmpty;
            return;
        };
        let Some(character) = self.snapshot.party.get(self.target_selection) else {
            self.page = CampPage::ItemResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let result = runtime.use_camp_item(item.slot, character.party_slot);
        self.message = use_result_message(result);
        self.page = CampPage::ItemResult;
        self.sync(runtime);
    }
}

fn use_result_message(result: CampUseResult) -> String {
    match result {
        CampUseResult::Used {
            item_name,
            character_name,
            amount,
        } if amount > 0 => format!("{item_name}: {character_name} {amount} HP"),
        CampUseResult::Used {
            item_name,
            character_name,
            ..
        } => format!("USED {item_name} {character_name}"),
        CampUseResult::NoEffect { reason, .. } | CampUseResult::Unavailable { reason } => reason,
    }
}
