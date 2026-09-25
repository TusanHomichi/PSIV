//! EQUIP camp flow kept separate from the retail root and status renderer.

use psiv_runtime::{CampEquipResult, Runtime};

use super::draw::{draw_text, frame};
use super::layout::{
    CHILD_CURSOR_PATTERN, CellRect, EQUIP_ITEM_LIST, EQUIP_MESSAGE, EQUIP_STATS, EQUIPPED_ITEMS,
};
use super::{CampChrome, CampMenu, DrawList};

impl CampMenu {
    pub(super) fn confirm_equipment_slot(&mut self, runtime: &mut Runtime) {
        let Some(character) = self.snapshot.party.get(self.equipment_character_selection) else {
            self.mode = super::Mode::EquipResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let raw_slot = equipment_ui_slot(self.equipment_slot_selection);
        if character.equipment[raw_slot] == "--" {
            self.equipment_item_selection = 0;
            self.equipment_options = runtime.camp_equipment(character.party_slot);
            self.mode = super::Mode::EquipItems;
            return;
        }
        let result = runtime.unequip_camp_item(character.party_slot, raw_slot);
        self.message = equip_result_message(result);
        self.mode = super::Mode::EquipResult;
        self.sync(runtime);
    }

    pub(super) fn equip_selected_item(&mut self, runtime: &mut Runtime) {
        let Some(character) = self.snapshot.party.get(self.equipment_character_selection) else {
            self.mode = super::Mode::EquipResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let Some(item) = self.equipment_options.get(self.equipment_item_selection) else {
            self.mode = super::Mode::EquipResult;
            self.message = "NO EQUIPMENT".to_owned();
            return;
        };
        let has_hand_choice = runtime.camp_equipment_hand_choice(item.slot);
        if has_hand_choice && self.mode != super::Mode::EquipHands {
            self.equipment_hand_selection = 0;
            self.mode = super::Mode::EquipHands;
            return;
        }
        let result = if has_hand_choice {
            let hand = if self.equipment_hand_selection == 0 {
                psiv_core::battle::EquipSlot::RightHand
            } else {
                psiv_core::battle::EquipSlot::LeftHand
            };
            runtime.equip_camp_item_in_hand(character.party_slot, item.slot, hand)
        } else {
            runtime.equip_camp_item(character.party_slot, item.slot)
        };
        self.message = equip_result_message(result);
        self.mode = super::Mode::EquipResult;
        self.sync(runtime);
    }

    pub(super) fn draw_equip_characters(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(
            chrome,
            &mut list.quads,
            super::layout::equip_character_list(self.snapshot.party.len()),
        );
        for (row, character) in self.snapshot.party.iter().enumerate() {
            draw_text(
                chrome,
                &mut list.quads,
                &character.name,
                (5, 14 + row as i32 * 2),
            );
        }
        if !self.snapshot.party.is_empty()
            && let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (3, 14 + self.equipment_character_selection as i32 * 2),
            )
        {
            list.quads.push(quad);
        }
    }

    pub(super) fn draw_equip_stats(&self, chrome: &CampChrome, list: &mut DrawList) {
        let Some(character) = self.snapshot.party.get(self.equipment_character_selection) else {
            return;
        };
        // Native labels and numeric fields need more room than the original
        // tile strings. Keep oracle geometry constants intact for its fixtures.
        frame(
            chrome,
            &mut list.quads,
            CellRect::new(EQUIP_STATS.x, EQUIP_STATS.y, 28, EQUIP_STATS.h),
        );
        frame(
            chrome,
            &mut list.quads,
            CellRect::new(EQUIPPED_ITEMS.x, EQUIPPED_ITEMS.y, 23, EQUIPPED_ITEMS.h),
        );
        let stats = [
            (character.name.clone(), (4, 3)),
            (format!("LV: {}", character.level), (4, 5)),
            (
                format!("HP:{}/{}", character.current_hp, character.max_hp),
                (4, 7),
            ),
            (format!("ATK: {}", character.attack_power), (18, 3)),
            (format!("DFS: {}", character.defense_power), (18, 5)),
            (format!("STR: {}", character.strength), (18, 7)),
        ];
        for (text, cell) in stats {
            draw_text(chrome, &mut list.quads, &text, cell);
        }
        let equipment = [
            ("HEAD", 2usize),
            ("RIGHT", 0usize),
            ("LEFT", 1usize),
            ("BODY", 3usize),
        ];
        for (row, (label, raw_slot)) in equipment.into_iter().enumerate() {
            draw_text(
                chrome,
                &mut list.quads,
                &format!("{label}: {}", character.equipment[raw_slot]),
                (5, 14 + row as i32 * 2),
            );
        }
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (4, 14 + self.equipment_slot_selection as i32 * 2),
        ) {
            list.quads.push(quad);
        }
    }

    pub(super) fn draw_equip_items(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_equip_stats(chrome, list);
        frame(chrome, &mut list.quads, EQUIP_ITEM_LIST);
        const VISIBLE: usize = 8;
        let page_start = (self.equipment_item_selection / VISIBLE) * VISIBLE;
        for (row, item) in self
            .equipment_options
            .iter()
            .enumerate()
            .skip(page_start)
            .take(VISIBLE)
        {
            draw_text(
                chrome,
                &mut list.quads,
                &item.name,
                (22, 3 + (row - page_start) as i32 * 2),
            );
        }
        if !self.equipment_options.is_empty()
            && let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (21, 3 + (self.equipment_item_selection % VISIBLE) as i32 * 2),
            )
        {
            list.quads.push(quad);
        }
    }

    pub(super) fn draw_equip_result(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_equip_stats(chrome, list);
        frame(chrome, &mut list.quads, EQUIP_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }

    pub(super) fn draw_equip_hands(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_equip_stats(chrome, list);
        frame(
            chrome,
            &mut list.quads,
            super::layout::CellRect::new(21, 13, 17, 7),
        );
        for (index, label) in ["RIGHT HAND", "LEFT HAND"].into_iter().enumerate() {
            draw_text(chrome, &mut list.quads, label, (24, 15 + index as i32 * 2));
        }
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (22, 15 + self.equipment_hand_selection as i32 * 2),
        ) {
            list.quads.push(quad);
        }
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
