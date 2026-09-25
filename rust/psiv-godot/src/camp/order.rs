//! Original ORDER: choose each front slot, undo picks, auto-append the last.
use super::draw::{draw_text, frame};
use super::layout::{CHILD_CURSOR_PATTERN, CellRect, ITEM_MESSAGE, SELECTED_CURSOR_PATTERN};
use super::{CampChrome, CampMenu, DrawList, Mode, wrap};
use psiv_runtime::Runtime;

#[derive(Default)]
pub(super) struct OrderDraft {
    pub(super) cursor: usize,
    pub(super) remaining: Vec<u8>,
    pub(super) chosen: Vec<u8>,
    removed_indices: Vec<usize>,
    cursor_timer: u8,
    cursor_visible: bool,
}

impl OrderDraft {
    fn new(members: Vec<u8>) -> Self {
        Self {
            remaining: members,
            cursor_timer: 25,
            cursor_visible: true,
            ..Self::default()
        }
    }

    fn tick_cursor(&mut self) {
        // RedCursor_Main uses the OLD visibility bit after bchg: hidden
        // lasts 26 ticks, visible lasts 16 (26 initially or after movement).
        if self.cursor_timer == 0 {
            self.cursor_visible = !self.cursor_visible;
            self.cursor_timer = if self.cursor_visible { 15 } else { 25 };
        } else {
            self.cursor_timer -= 1;
        }
    }

    fn choose(&mut self) -> Option<Vec<u8>> {
        if self.remaining.len() < 2 || self.cursor >= self.remaining.len() {
            return None;
        }
        self.removed_indices.push(self.cursor);
        self.chosen.push(self.remaining.remove(self.cursor));
        self.cursor = 0;
        if self.remaining.len() == 1 {
            self.chosen.push(self.remaining.remove(0));
            return Some(self.chosen.clone());
        }
        None
    }

    pub(super) fn undo(&mut self) -> bool {
        let Some(index) = self.removed_indices.pop() else {
            return false;
        };
        let id = self.chosen.pop().expect("one chosen member per removal");
        self.remaining.insert(index, id);
        self.cursor = 0;
        true
    }
}

impl CampMenu {
    pub(super) fn begin_order(&mut self) {
        self.order_draft = OrderDraft::new(self.snapshot.party.iter().map(|c| c.id).collect());
        self.mode = if self.snapshot.party.len() < 2 {
            Mode::OrderAlone
        } else {
            Mode::Order
        };
        self.order_wait = 121;
    }

    pub(super) fn handle_order_input(
        &mut self,
        runtime: &mut Runtime,
        up: bool,
        down: bool,
        accept: bool,
    ) {
        if self.mode == Mode::Order {
            self.order_draft.tick_cursor();
        }
        if self.mode != Mode::Order {
            self.order_wait = self.order_wait.saturating_sub(1);
            if accept || self.order_wait == 0 {
                self.mode = if self.mode == Mode::OrderAlone {
                    Mode::State
                } else {
                    Mode::Root
                };
                if self.mode == Mode::Root {
                    self.state_selection = 0;
                }
            }
        } else if up || down {
            self.sound_request = Some(0xF2); // SFXID_MovingCursor
            self.order_draft.cursor_timer = 25;
            self.order_draft.cursor_visible = true;
            self.order_draft.cursor = wrap(
                self.order_draft.cursor,
                self.order_draft.remaining.len(),
                down,
            );
        } else if accept {
            self.sound_request = Some(0xF3); // SFXID_Selection
            if let Some(order) = self.order_draft.choose() {
                match runtime.order_camp_party(&order) {
                    Ok(events) => {
                        self.runtime_events.extend(events);
                        self.mode = Mode::OrderDone;
                        self.order_wait = 61;
                    }
                    Err(_) => {
                        self.message = "CANNOT CHANGE ORDER".into();
                        self.mode = Mode::Unsupported;
                    }
                }
            }
        }
    }

    pub(super) fn draw_order(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_state(chrome, list);
        if self.mode == Mode::OrderAlone {
            frame(chrome, &mut list.quads, ITEM_MESSAGE);
            draw_text(chrome, &mut list.quads, "There's only", (8, 22));
            draw_text(chrome, &mut list.quads, "one of you!", (8, 23));
            return;
        }
        // WinGroup_Menu D2..D9: dimensions are stored one cell short.
        let height = self.snapshot.party.len() as i32 * 2 + 2;
        if self.mode == Mode::Order {
            frame(chrome, &mut list.quads, CellRect::new(2, 13, 9, height));
            draw_text(
                chrome,
                &mut list.quads,
                "ORDER",
                (4, 14 + self.snapshot.party.len() as i32 * 2),
            );
        }
        frame(chrome, &mut list.quads, CellRect::new(11, 13, 7, height));
        for (ids, x) in [
            (&self.order_draft.remaining, 5),
            (&self.order_draft.chosen, 13),
        ] {
            for (row, id) in ids.iter().enumerate() {
                if x == 5
                    && let Some(marker) =
                        chrome.window_word(CHILD_CURSOR_PATTERN, (3, 14 + row as i32 * 2))
                {
                    list.quads.push(marker);
                }
                if let Some(member) = self.snapshot.party.iter().find(|c| c.id == *id) {
                    draw_text(
                        chrome,
                        &mut list.quads,
                        &member.name,
                        (x, 14 + row as i32 * 2),
                    );
                }
            }
        }
        if self.mode == Mode::Order
            && self.order_draft.cursor_visible
            && let Some(cursor) = chrome.window_word(
                SELECTED_CURSOR_PATTERN,
                (3, 14 + self.order_draft.cursor as i32 * 2),
            )
        {
            list.quads.push(cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_undo_restores_the_removed_position_and_last_member_is_automatic() {
        let mut draft = OrderDraft::new(vec![1, 0, 2, 4]);
        draft.cursor = 3;
        assert_eq!(draft.choose(), None);
        draft.cursor = 1;
        assert_eq!(draft.choose(), None);
        assert_eq!(draft.chosen, vec![4, 0]);
        assert!(draft.undo());
        assert_eq!(draft.remaining, vec![1, 0, 2]);
        assert_eq!(draft.cursor, 0);
        assert!(draft.undo());
        assert_eq!(draft.remaining, vec![1, 0, 2, 4]);
        assert!(!draft.undo());
        draft.cursor = 3;
        assert_eq!(draft.choose(), None);
        assert_eq!(draft.choose(), None);
        assert_eq!(draft.choose(), Some(vec![4, 1, 0, 2]));
    }
}
