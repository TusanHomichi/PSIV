//! Camp window drawing: one page builder per menu mode.
//!
//! The retail rectangles, window words and fonts come from `chrome`; this
//! module only decides which page the current mode shows and which summary
//! values ride along with it.

use godot::prelude::*;

use psiv_runtime::CampCharacter;

use super::chrome::{CampChrome, Quad};
use super::layout::{
    self, CHARACTER_SUMMARY, CHILD_CURSOR_PATTERN, ITEM_EMPTY_TEXT, ITEM_MESSAGE, MESETA,
    ROOT_CURSOR_CELL, ROOT_MENU, ROOT_TEXT, SAVE_SLOT_TEXT, SAVE_SLOTS_OPTIONS,
    SELECTED_CURSOR_PATTERN, STATE_CURSOR_CELL, STATE_SAVE_OPTIONS, STATE_SAVE_TEXT, STATE_TEXT,
    STATUS_EQUIPMENT, STATUS_EXP, STATUS_INFO, STATUS_PORTRAIT, STATUS_STATS, STATUS_TEXT,
};
use super::status::{draw_level, draw_status_pair, draw_status_text};
use super::{CampMenu, DrawList, ITEM_LIST, ITEM_TARGET, Mode, ROOT_OPTIONS};

impl CampMenu {
    pub(super) fn draw_list(&self) -> Option<DrawList> {
        let chrome = self.chrome.as_ref()?;
        let mut list = DrawList {
            quads: Vec::new(),
            portrait: None,
        };
        match self.mode {
            Mode::TravelTowns | Mode::TravelReady if self.travel_item_slot.is_some() => {
                self.draw_item_list(chrome, &mut list);
                self.draw_travel_overlay(chrome, &mut list);
            }
            Mode::AbilityCharacters
            | Mode::AbilityList
            | Mode::AbilityTarget
            | Mode::AbilityResult
            | Mode::TravelTowns
            | Mode::TravelReady => self.draw_abilities(chrome, &mut list),
            Mode::LootMessage
            | Mode::LootItems
            | Mode::LootDiscardConfirm
            | Mode::LootReturnConfirm
            | Mode::LootBlocked => self.draw_loot(chrome, &mut list),
            Mode::Root => self.draw_root(chrome, &mut list, true),
            Mode::ItemEmpty => self.draw_item_empty(chrome, &mut list),
            Mode::ItemList => self.draw_item_list(chrome, &mut list),
            Mode::ItemTarget => self.draw_item_target(chrome, &mut list),
            Mode::ItemResult => self.draw_item_result(chrome, &mut list),
            Mode::EquipCharacters => self.draw_equip_characters(chrome, &mut list),
            Mode::EquipStats => self.draw_equip_stats(chrome, &mut list),
            Mode::EquipItems => self.draw_equip_items(chrome, &mut list),
            Mode::EquipHands => self.draw_equip_hands(chrome, &mut list),
            Mode::EquipResult => self.draw_equip_result(chrome, &mut list),
            Mode::State => self.draw_state(chrome, &mut list),
            Mode::Order | Mode::OrderDone | Mode::OrderAlone => self.draw_order(chrome, &mut list),
            Mode::SaveSlots => self.draw_save_slots(chrome, &mut list),
            Mode::SaveResult => self.draw_save_result(chrome, &mut list),
            Mode::Status => self.draw_status(chrome, &mut list),
            Mode::Unsupported => self.draw_unsupported(chrome, &mut list),
            Mode::Closed => return None,
        }
        Some(list)
    }

    pub(super) fn draw_root(&self, chrome: &CampChrome, list: &mut DrawList, meseta: bool) {
        frame(chrome, &mut list.quads, ROOT_MENU);
        frame(chrome, &mut list.quads, CHARACTER_SUMMARY);
        if meseta {
            draw_meseta(chrome, &mut list.quads, self.snapshot.money);
        }
        draw_root_options(chrome, &mut list.quads);
        let cursor = (
            ROOT_CURSOR_CELL.0,
            ROOT_CURSOR_CELL.1 + self.root_selection as i32 * 2,
        );
        if let Some(quad) = chrome.window_word(SELECTED_CURSOR_PATTERN, cursor) {
            list.quads.push(quad);
        }
        if let Some(character) = self.snapshot.party.first() {
            draw_summary(chrome, &mut list.quads, character);
        }
    }

    fn draw_item_empty(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(
            chrome,
            &mut list.quads,
            ITEM_EMPTY_TEXT.text,
            ITEM_EMPTY_TEXT.cell,
        );
    }

    fn draw_item_list(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_LIST);
        let page = self.item_selection / 8;
        for (row, item) in self
            .snapshot
            .inventory
            .iter()
            .skip(page * 8)
            .take(8)
            .enumerate()
        {
            draw_text(
                chrome,
                &mut list.quads,
                &item.name,
                (16, 3 + row as i32 * 2),
            );
        }
        if self.snapshot.inventory.len() > 8 {
            draw_text(
                chrome,
                &mut list.quads,
                &format!(
                    "PAGE {}/{}",
                    page + 1,
                    self.snapshot.inventory.len().div_ceil(8)
                ),
                (16, 20),
            );
        }
        if !self.snapshot.inventory.is_empty()
            && let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (15, 3 + (self.item_selection % 8) as i32 * 2),
            )
        {
            list.quads.push(quad);
        }
    }

    fn draw_item_target(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_item_list(chrome, list);
        frame(chrome, &mut list.quads, ITEM_TARGET);
        for (row, character) in self.snapshot.party.iter().enumerate() {
            draw_text(
                chrome,
                &mut list.quads,
                &character.name,
                (20, 7 + row as i32),
            );
        }
        if !self.snapshot.party.is_empty()
            && let Some(quad) =
                chrome.window_word(CHILD_CURSOR_PATTERN, (19, 7 + self.target_selection as i32))
        {
            list.quads.push(quad);
        }
    }

    fn draw_item_result(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }

    pub(super) fn draw_state(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, true);
        // STATE_OPTIONS is the oracle-pinned retail two-row rectangle. This
        // modern branch adds SAVE without rewriting that retail constant.
        frame(chrome, &mut list.quads, STATE_SAVE_OPTIONS);
        for text in STATE_TEXT {
            draw_text(chrome, &mut list.quads, text.text, text.cell);
        }
        draw_text(
            chrome,
            &mut list.quads,
            STATE_SAVE_TEXT.text,
            STATE_SAVE_TEXT.cell,
        );
        let cursor = (
            STATE_CURSOR_CELL.0,
            STATE_CURSOR_CELL.1 + self.state_selection as i32 * 2,
        );
        if let Some(quad) = chrome.window_word(CHILD_CURSOR_PATTERN, cursor) {
            list.quads.push(quad);
        }
    }

    fn draw_save_slots(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, true);
        frame(chrome, &mut list.quads, SAVE_SLOTS_OPTIONS);
        for text in SAVE_SLOT_TEXT {
            draw_text(chrome, &mut list.quads, text.text, text.cell);
        }
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (10, 8 + self.save_selection as i32 * 2),
        ) {
            list.quads.push(quad);
        }
    }

    fn draw_save_result(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_save_slots(chrome, list);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }

    fn draw_status(&self, chrome: &CampChrome, list: &mut DrawList) {
        let Some(character) = self.snapshot.party.get(self.status_selection) else {
            return;
        };
        for rect in [
            STATUS_PORTRAIT,
            STATUS_INFO,
            STATUS_STATS,
            STATUS_EQUIPMENT,
            STATUS_EXP,
            MESETA,
        ] {
            frame(chrome, &mut list.quads, rect);
        }
        draw_text(
            chrome,
            &mut list.quads,
            STATUS_TEXT[19].text,
            STATUS_TEXT[19].cell,
        );
        draw_status_text(chrome, &mut list.quads, character, self.snapshot.money);
        if let Some(portrait) = self.portraits.get(&character.id) {
            list.portrait = Some((
                portrait.clone(),
                Rect2::new(Vector2::new(24.0, 16.0), Vector2::new(80.0, 80.0)),
            ));
        }
    }

    fn draw_unsupported(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, true);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }
}

pub(super) fn frame(chrome: &CampChrome, quads: &mut Vec<Quad>, rect: layout::CellRect) {
    quads.extend(chrome.frame(rect));
}

pub(super) fn draw_text(chrome: &CampChrome, quads: &mut Vec<Quad>, text: &str, cell: (i32, i32)) {
    quads.extend(chrome.text(text, cell));
}

fn draw_root_options(chrome: &CampChrome, quads: &mut Vec<Quad>) {
    for text in &ROOT_TEXT[4..11] {
        draw_text(chrome, quads, text.text, text.cell);
    }
    draw_selectors(chrome, quads, ROOT_CURSOR_CELL, ROOT_OPTIONS.len());
}

fn draw_selectors(
    chrome: &CampChrome,
    quads: &mut Vec<Quad>,
    first_cell: (i32, i32),
    count: usize,
) {
    for row in 0..count {
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (first_cell.0, first_cell.1 + row as i32 * 2),
        ) {
            quads.push(quad);
        }
    }
}

fn draw_meseta(chrome: &CampChrome, quads: &mut Vec<Quad>, money: u32) {
    frame(chrome, quads, MESETA);
    draw_text(chrome, quads, &format!("{money} MST"), ROOT_TEXT[11].cell);
}

fn draw_summary(chrome: &CampChrome, quads: &mut Vec<Quad>, character: &CampCharacter) {
    draw_text(chrome, quads, &character.name, ROOT_TEXT[0].cell);
    draw_level(chrome, quads, character.level, ROOT_TEXT[1].cell);
    draw_status_pair(
        chrome,
        quads,
        "HP: ",
        character.current_hp,
        character.max_hp,
        ROOT_TEXT[2].cell,
    );
    draw_status_pair(
        chrome,
        quads,
        "TP: ",
        character.current_tp,
        character.max_tp,
        ROOT_TEXT[3].cell,
    );
}
