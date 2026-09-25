//! Camp input routing: one frame of menu input and the commands it fires.

use godot::classes::Input;
use godot::global::Key;
use godot::prelude::*;

use psiv_runtime::{CampAbilityKind, CampUseResult, Runtime};

use crate::save_dir::save_slot;

use super::{CampMenu, Mode, ROOT_OPTIONS, wrap};

impl CampMenu {
    /// Returns one edge-triggered X/ui_cancel press. X is checked as a
    /// physical key as well as through Godot's action so the keyboard path is
    /// usable in a stock project with no hand-edited InputMap.
    pub(crate) fn cancel_requested(&mut self) -> bool {
        let input = Input::singleton();
        let x = input.is_physical_key_pressed(Key::X);
        let x_edge = x && !self.x_down;
        self.x_down = x;
        input.is_action_just_pressed("ui_cancel") || x_edge
    }

    /// Handles one frame of camp input and runtime item commands.
    pub(crate) fn handle_input(&mut self, runtime: &mut Runtime) {
        let cancel = self.cancel_requested();
        let input = Input::singleton();
        if self.is_loot() {
            self.handle_loot_input(runtime, cancel);
            return;
        }
        if self.mode == Mode::TravelReady {
            // Enter and controller Start are both bound to ui_accept.
            if cancel || input.is_action_just_pressed("ui_accept") {
                self.complete_travel(runtime);
            }
            return;
        }
        if cancel {
            if self.go_back() {
                self.close();
                return;
            }
            self.sync(runtime);
            return;
        }
        let up = input.is_action_just_pressed("ui_up");
        let down = input.is_action_just_pressed("ui_down");
        let accept = input.is_action_just_pressed("ui_accept");
        match self.mode {
            Mode::AbilityCharacters
            | Mode::AbilityList
            | Mode::AbilityTarget
            | Mode::AbilityResult
            | Mode::TravelTowns => {
                self.handle_ability_input(runtime, up, down, accept);
            }
            Mode::Root => {
                if up {
                    self.root_selection = wrap(self.root_selection, ROOT_OPTIONS.len(), false);
                } else if down {
                    self.root_selection = wrap(self.root_selection, ROOT_OPTIONS.len(), true);
                } else if accept {
                    self.confirm_root(runtime);
                }
            }
            Mode::ItemEmpty => {
                if accept {
                    self.mode = Mode::Root;
                }
            }
            Mode::ItemList => {
                if self.snapshot.inventory.is_empty() {
                    self.mode = Mode::ItemEmpty;
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
            Mode::ItemTarget => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::ItemResult;
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
            Mode::ItemResult => {
                if accept {
                    self.mode = if self.snapshot.inventory.is_empty() {
                        Mode::Root
                    } else {
                        Mode::ItemList
                    };
                }
            }
            Mode::EquipCharacters => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::EquipResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.equipment_character_selection = wrap(
                        self.equipment_character_selection,
                        self.snapshot.party.len(),
                        false,
                    );
                } else if down {
                    self.equipment_character_selection = wrap(
                        self.equipment_character_selection,
                        self.snapshot.party.len(),
                        true,
                    );
                } else if accept {
                    self.equipment_slot_selection = 0;
                    self.equipment_options =
                        runtime.camp_equipment(self.equipment_character_selection);
                    self.mode = Mode::EquipStats;
                }
            }
            Mode::EquipStats => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::EquipResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.equipment_slot_selection = wrap(self.equipment_slot_selection, 4, false);
                } else if down {
                    self.equipment_slot_selection = wrap(self.equipment_slot_selection, 4, true);
                } else if accept {
                    self.confirm_equipment_slot(runtime);
                }
            }
            Mode::EquipItems => {
                if self.equipment_options.is_empty() {
                    self.mode = Mode::EquipResult;
                    self.message = "NO EQUIPMENT".to_owned();
                } else if up {
                    self.equipment_item_selection = wrap(
                        self.equipment_item_selection,
                        self.equipment_options.len(),
                        false,
                    );
                } else if down {
                    self.equipment_item_selection = wrap(
                        self.equipment_item_selection,
                        self.equipment_options.len(),
                        true,
                    );
                } else if accept {
                    self.equip_selected_item(runtime);
                }
            }
            Mode::EquipResult => {
                if accept {
                    self.mode = Mode::EquipStats;
                }
            }
            Mode::EquipHands => {
                if up || down {
                    self.equipment_hand_selection = 1 - self.equipment_hand_selection;
                } else if accept {
                    self.equip_selected_item(runtime);
                }
            }
            Mode::State => {
                if up {
                    self.state_selection = wrap(self.state_selection, 3, false);
                } else if down {
                    self.state_selection = wrap(self.state_selection, 3, true);
                } else if accept {
                    if self.state_selection == 0 {
                        self.status_selection = 0;
                        self.status_from_state = true;
                        self.mode = Mode::Status;
                    } else if self.state_selection == 1 {
                        self.begin_order();
                    } else {
                        self.save_selection = 0;
                        self.mode = Mode::SaveSlots;
                    }
                }
            }
            Mode::Order | Mode::OrderDone | Mode::OrderAlone => {
                self.handle_order_input(runtime, up, down, accept);
            }
            Mode::Status => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::State;
                } else if up {
                    self.status_selection =
                        wrap(self.status_selection, self.snapshot.party.len(), false);
                } else if down {
                    self.status_selection =
                        wrap(self.status_selection, self.snapshot.party.len(), true);
                }
            }
            Mode::Unsupported => {
                if accept {
                    self.mode = Mode::Root;
                }
            }
            Mode::SaveSlots => {
                if up {
                    self.save_selection = wrap(self.save_selection, 3, false);
                } else if down {
                    self.save_selection = wrap(self.save_selection, 3, true);
                } else if accept {
                    self.save_selected(runtime);
                }
            }
            Mode::SaveResult => {
                if accept {
                    self.mode = Mode::State;
                }
            }
            Mode::Closed
            | Mode::TravelReady
            | Mode::LootMessage
            | Mode::LootItems
            | Mode::LootDiscardConfirm
            | Mode::LootReturnConfirm
            | Mode::LootBlocked => {}
        }
        self.sync(runtime);
    }

    fn go_back(&mut self) -> bool {
        match self.mode {
            Mode::Order => {
                if !self.order_draft.undo() {
                    self.mode = Mode::State;
                }
                false
            }
            Mode::OrderDone => {
                self.mode = Mode::Root;
                self.state_selection = 0;
                false
            }
            Mode::OrderAlone => {
                self.mode = Mode::State;
                false
            }
            Mode::AbilityCharacters => {
                self.mode = Mode::Root;
                false
            }
            Mode::AbilityList => {
                self.mode = Mode::AbilityCharacters;
                false
            }
            Mode::AbilityTarget => {
                self.mode = Mode::AbilityList;
                false
            }
            Mode::AbilityResult => {
                self.mode = Mode::AbilityList;
                false
            }
            Mode::TravelTowns => {
                self.mode = if self.travel_item_slot.is_some() {
                    Mode::ItemList
                } else {
                    Mode::AbilityList
                };
                false
            }
            Mode::TravelReady
            | Mode::LootMessage
            | Mode::LootItems
            | Mode::LootDiscardConfirm
            | Mode::LootReturnConfirm
            | Mode::LootBlocked => false,
            Mode::Closed => true,
            Mode::Root => true,
            Mode::ItemEmpty | Mode::ItemList | Mode::ItemResult | Mode::Unsupported => {
                self.mode = Mode::Root;
                false
            }
            Mode::ItemTarget => {
                self.mode = Mode::ItemList;
                false
            }
            Mode::EquipCharacters => {
                self.mode = Mode::Root;
                false
            }
            Mode::EquipStats => {
                self.mode = Mode::EquipCharacters;
                false
            }
            Mode::EquipItems => {
                self.mode = Mode::EquipStats;
                false
            }
            Mode::EquipHands => {
                self.mode = Mode::EquipItems;
                false
            }
            Mode::EquipResult => {
                self.mode = Mode::EquipStats;
                false
            }
            Mode::State => {
                self.mode = Mode::Root;
                false
            }
            Mode::Status => {
                self.mode = if self.status_from_state {
                    Mode::State
                } else {
                    Mode::Root
                };
                false
            }
            Mode::SaveSlots | Mode::SaveResult => {
                self.mode = Mode::State;
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
                self.mode = Mode::AbilityCharacters;
            }
            0 => {
                self.mode = if runtime.camp_state().inventory.is_empty() {
                    Mode::ItemEmpty
                } else {
                    Mode::ItemList
                };
            }
            3 => {
                self.equipment_character_selection = 0;
                self.equipment_slot_selection = 0;
                self.equipment_item_selection = 0;
                self.equipment_options = runtime.camp_equipment(0);
                self.mode = Mode::EquipCharacters;
            }
            4 => self.mode = Mode::State,
            _ => {
                self.mode = Mode::Unsupported;
                self.message = format!("{} NOT READY", ROOT_OPTIONS[self.root_selection]);
            }
        }
    }

    fn confirm_item(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            self.mode = Mode::ItemEmpty;
            return;
        };
        if matches!(item.id, psiv_runtime::TELEPIPE | psiv_runtime::ESCAPIPE) {
            self.begin_selected_item_travel(runtime);
            return;
        }
        if !item.usable {
            self.mode = Mode::ItemResult;
            self.message = format!("{} NOT USABLE", item.name);
            return;
        }
        if self.snapshot.party.len() == 1 || item.targeting == 5 {
            self.target_selection = 0;
            self.use_selected_item(runtime);
        } else {
            self.target_selection = 0;
            self.mode = Mode::ItemTarget;
        }
    }

    fn use_selected_item(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            self.mode = Mode::ItemEmpty;
            return;
        };
        let Some(character) = self.snapshot.party.get(self.target_selection) else {
            self.mode = Mode::ItemResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let result = runtime.use_camp_item(item.slot, character.party_slot);
        self.message = use_result_message(result);
        self.mode = Mode::ItemResult;
        self.sync(runtime);
    }

    /// Writes the selected visible slot through the run's save directory.
    ///
    /// A scripted run without `PSIV_SAVE_DIR` is refused by the resolver
    /// before any filesystem access, so SAVE never falls back to a directory
    /// the run did not name.
    fn save_selected(&mut self, runtime: &Runtime) {
        self.message = match save_slot(runtime, self.save_selection) {
            Ok(_) => "FILE SAVED".to_owned(),
            Err(error) => {
                godot_error!(
                    "camp: save slot {} failed: {error}",
                    self.save_selection + 1
                );
                format!("SAVE ERROR: {error}")
            }
        };
        self.mode = Mode::SaveResult;
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
        CampUseResult::NoEffect { reason, .. } => reason,
        CampUseResult::Unavailable { reason } => reason,
    }
}
