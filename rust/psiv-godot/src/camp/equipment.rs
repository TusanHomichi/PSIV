//! EQUIP pages: the roster, the stat sheet, the item list and the hand choice.
//! What equipping does is the session's.

use super::draw::{draw_text, frame};
use super::layout::{
    CHILD_CURSOR_PATTERN, CellRect, EQUIP_ITEM_LIST, EQUIP_MESSAGE, EQUIP_STATS, EQUIPPED_ITEMS,
};
use super::{CampChrome, CampMenu, DrawList};
use psiv_runtime::CampView;

impl CampMenu {
    pub(super) fn draw_equip_characters(
        &self,
        view: &CampView,
        chrome: &CampChrome,
        list: &mut DrawList,
    ) {
        self.draw_root(view, chrome, list, false);
        frame(
            chrome,
            &mut list.quads,
            super::layout::equip_character_list(view.snapshot.party.len()),
        );
        for (row, character) in view.snapshot.party.iter().enumerate() {
            draw_text(
                chrome,
                &mut list.quads,
                &character.name,
                (5, 14 + row as i32 * 2),
            );
        }
        if !view.snapshot.party.is_empty()
            && let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (3, 14 + view.equipment_character_selection as i32 * 2),
            )
        {
            list.quads.push(quad);
        }
    }

    pub(super) fn draw_equip_stats(
        &self,
        view: &CampView,
        chrome: &CampChrome,
        list: &mut DrawList,
    ) {
        let Some(character) = view.snapshot.party.get(view.equipment_character_selection) else {
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
            (4, 14 + view.equipment_slot_selection as i32 * 2),
        ) {
            list.quads.push(quad);
        }
    }

    pub(super) fn draw_equip_items(
        &self,
        view: &CampView,
        chrome: &CampChrome,
        list: &mut DrawList,
    ) {
        self.draw_equip_stats(view, chrome, list);
        frame(chrome, &mut list.quads, EQUIP_ITEM_LIST);
        const VISIBLE: usize = 8;
        let page_start = (view.equipment_item_selection / VISIBLE) * VISIBLE;
        for (row, item) in view
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
        if !view.equipment_options.is_empty()
            && let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (21, 3 + (view.equipment_item_selection % VISIBLE) as i32 * 2),
            )
        {
            list.quads.push(quad);
        }
    }

    pub(super) fn draw_equip_result(
        &self,
        view: &CampView,
        chrome: &CampChrome,
        list: &mut DrawList,
    ) {
        self.draw_equip_stats(view, chrome, list);
        frame(chrome, &mut list.quads, EQUIP_MESSAGE);
        draw_text(chrome, &mut list.quads, &view.message, (8, 22));
    }

    pub(super) fn draw_equip_hands(
        &self,
        view: &CampView,
        chrome: &CampChrome,
        list: &mut DrawList,
    ) {
        self.draw_equip_stats(view, chrome, list);
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
            (22, 15 + view.equipment_hand_selection as i32 * 2),
        ) {
            list.quads.push(quad);
        }
    }
}
