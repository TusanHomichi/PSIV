//! EQUIP: equip against unequip, and the right/left hand choice.

use psiv_core::battle::EquipSlot;

use crate::{CampEquipResult, Runtime};

use super::{CampPage, CampView, wrap};

impl CampView {
    pub(super) fn equipment_frame(
        &mut self,
        runtime: &mut Runtime,
        up: bool,
        down: bool,
        accept: bool,
    ) {
        match self.page {
            CampPage::EquipCharacters => {
                if self.snapshot.party.is_empty() {
                    self.page = CampPage::EquipResult;
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
                    self.page = CampPage::EquipStats;
                }
            }
            CampPage::EquipStats => {
                if self.snapshot.party.is_empty() {
                    self.page = CampPage::EquipResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.equipment_slot_selection = wrap(self.equipment_slot_selection, 4, false);
                } else if down {
                    self.equipment_slot_selection = wrap(self.equipment_slot_selection, 4, true);
                } else if accept {
                    self.confirm_equipment_slot(runtime);
                }
            }
            CampPage::EquipItems => {
                if self.equipment_options.is_empty() {
                    self.page = CampPage::EquipResult;
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
            CampPage::EquipResult => {
                if accept {
                    self.page = CampPage::EquipStats;
                }
            }
            CampPage::EquipHands => {
                if up || down {
                    self.equipment_hand_selection = 1 - self.equipment_hand_selection;
                } else if accept {
                    self.equip_selected_item(runtime);
                }
            }
            _ => {}
        }
    }

    /// A slot with nothing in it opens the item list; a slot holding an item
    /// takes it off. The decision is the equipment byte, never the label.
    fn confirm_equipment_slot(&mut self, runtime: &mut Runtime) {
        let Some(character) = self.snapshot.party.get(self.equipment_character_selection) else {
            self.page = CampPage::EquipResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let raw_slot = equipment_ui_slot(self.equipment_slot_selection);
        if character.equipment_ids[raw_slot] == 0 {
            self.equipment_item_selection = 0;
            self.equipment_options = runtime.camp_equipment(character.party_slot);
            self.page = CampPage::EquipItems;
            return;
        }
        let result = runtime.unequip_camp_item(character.party_slot, raw_slot);
        self.message = equip_result_message(result);
        self.page = CampPage::EquipResult;
        self.sync(runtime);
    }

    fn equip_selected_item(&mut self, runtime: &mut Runtime) {
        let Some(character) = self.snapshot.party.get(self.equipment_character_selection) else {
            self.page = CampPage::EquipResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let Some(item) = self.equipment_options.get(self.equipment_item_selection) else {
            self.page = CampPage::EquipResult;
            self.message = "NO EQUIPMENT".to_owned();
            return;
        };
        let has_hand_choice = runtime.camp_equipment_hand_choice(item.slot);
        if has_hand_choice && self.page != CampPage::EquipHands {
            self.equipment_hand_selection = 0;
            self.page = CampPage::EquipHands;
            return;
        }
        let result = if has_hand_choice {
            let hand = if self.equipment_hand_selection == 0 {
                EquipSlot::RightHand
            } else {
                EquipSlot::LeftHand
            };
            runtime.equip_camp_item_in_hand(character.party_slot, item.slot, hand)
        } else {
            runtime.equip_camp_item(character.party_slot, item.slot)
        };
        self.message = equip_result_message(result);
        self.page = CampPage::EquipResult;
        self.sync(runtime);
    }
}

/// The retail equipped-items cursor order is head/right/left/body, while the
/// persistent record remains right/left/head/body at `$4C..$4F`.
fn equipment_ui_slot(selection: usize) -> usize {
    [2, 0, 1, 3][selection.min(3)]
}

fn equip_result_message(result: CampEquipResult) -> String {
    match result {
        CampEquipResult::Equipped {
            item_name,
            character_name,
        } => format!("EQUIPPED {item_name} {character_name}"),
        CampEquipResult::Unequipped {
            item_name,
            character_name,
        } => format!("REMOVED {item_name} {character_name}"),
        CampEquipResult::Unavailable { reason } => reason,
    }
}
