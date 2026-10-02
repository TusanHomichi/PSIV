//! Ordinary field chest windows using the cartridge's font and window tiles.
//! The result line and the confirmations are the session's.
use super::draw::{draw_text, frame};
use super::layout::{CHILD_CURSOR_PATTERN, CellRect};
use super::{CampMenu, DrawList, chrome::CampChrome};
use psiv_runtime::{CampPage, CampView};

impl CampMenu {
    pub(super) fn draw_loot(&self, view: &CampView, chrome: &CampChrome, list: &mut DrawList) {
        frame(chrome, &mut list.quads, CellRect::new(1, 1, 38, 8));
        for (row, text) in view.message.lines().enumerate().take(3) {
            draw_text(chrome, &mut list.quads, text, (3, 3 + row as i32 * 2));
        }
        if view.page == CampPage::LootMessage || view.page == CampPage::LootBlocked {
            return;
        }
        frame(chrome, &mut list.quads, CellRect::new(13, 9, 26, 18));
        let page = view.item_selection / 8;
        for (row, item) in view
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
                (16, 10 + row as i32 * 2),
            );
        }
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (14, 10 + (view.item_selection % 8) as i32 * 2),
        ) {
            list.quads.push(quad);
        }
        if matches!(
            view.page,
            CampPage::LootReturnConfirm | CampPage::LootDiscardConfirm
        ) {
            frame(chrome, &mut list.quads, CellRect::new(1, 9, 38, 10));
            let prompt = if view.page == CampPage::LootReturnConfirm {
                "Give up this item?"
            } else {
                "Discard the selected item?"
            };
            if view.page == CampPage::LootDiscardConfirm
                && let Some(item) = view.snapshot.inventory.get(view.item_selection)
            {
                draw_text(chrome, &mut list.quads, &item.name, (3, 10));
            }
            draw_text(chrome, &mut list.quads, prompt, (3, 12));
            draw_text(chrome, &mut list.quads, "YES", (6, 14));
            draw_text(chrome, &mut list.quads, "NO", (6, 16));
            if let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (4, 14 + view.confirm_selection as i32 * 2),
            ) {
                list.quads.push(quad);
            }
        }
    }
}
