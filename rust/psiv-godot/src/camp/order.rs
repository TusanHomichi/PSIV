//! Original ORDER pages: the pick list and its blinking cursor. The draft, the
//! undo and the frame-counted close are the session's.
use super::draw::{draw_text, frame};
use super::layout::{CHILD_CURSOR_PATTERN, CellRect, ITEM_MESSAGE, SELECTED_CURSOR_PATTERN};
use super::{CampChrome, CampMenu, DrawList};
use psiv_runtime::{CampPage, CampView};

impl CampMenu {
    pub(super) fn draw_order(&self, view: &CampView, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_state(view, chrome, list);
        if view.page == CampPage::OrderAlone {
            frame(chrome, &mut list.quads, ITEM_MESSAGE);
            draw_text(chrome, &mut list.quads, "There's only", (8, 22));
            draw_text(chrome, &mut list.quads, "one of you!", (8, 23));
            return;
        }
        // WinGroup_Menu D2..D9: dimensions are stored one cell short.
        let height = view.snapshot.party.len() as i32 * 2 + 2;
        if view.page == CampPage::Order {
            frame(chrome, &mut list.quads, CellRect::new(2, 13, 9, height));
            draw_text(
                chrome,
                &mut list.quads,
                "ORDER",
                (4, 14 + view.snapshot.party.len() as i32 * 2),
            );
        }
        frame(chrome, &mut list.quads, CellRect::new(11, 13, 7, height));
        for (ids, x) in [
            (&view.order_draft.remaining, 5),
            (&view.order_draft.chosen, 13),
        ] {
            for (row, id) in ids.iter().enumerate() {
                if x == 5
                    && let Some(marker) =
                        chrome.window_word(CHILD_CURSOR_PATTERN, (3, 14 + row as i32 * 2))
                {
                    list.quads.push(marker);
                }
                if let Some(member) = view.snapshot.party.iter().find(|c| c.id == *id) {
                    draw_text(
                        chrome,
                        &mut list.quads,
                        &member.name,
                        (x, 14 + row as i32 * 2),
                    );
                }
            }
        }
        if view.page == CampPage::Order
            && view.order_draft.cursor_visible
            && let Some(cursor) = chrome.window_word(
                SELECTED_CURSOR_PATTERN,
                (3, 14 + view.order_draft.cursor as i32 * 2),
            )
        {
            list.quads.push(cursor);
        }
    }
}
