//! STATE > ORDER: choose each front slot, undo picks, the last member is
//! appended automatically, and the finished page closes itself after a frame
//! count (`loc_5E5A0` commits a complete permutation).

use crate::Runtime;

use super::{CampPage, CampView, wrap};

/// Frames the "only one of you" page stays up on its own, and the committed
/// page after a three-member pick: the cartridge's wait counts.
const ALONE_WAIT_FRAMES: u16 = 121;
const DONE_WAIT_FRAMES: u16 = 61;

/// ORDER's draft: who is still to be placed, and who has been.
#[derive(Clone, Debug, Default)]
pub struct OrderDraft {
    /// The cursor on the members still to be placed.
    pub cursor: usize,
    /// The character ids not yet placed, in roster order.
    pub remaining: Vec<u8>,
    /// The character ids placed so far, front slot first.
    pub chosen: Vec<u8>,
    removed_indices: Vec<usize>,
    cursor_timer: u8,
    /// Whether the red cursor is in its visible half of the blink.
    pub cursor_visible: bool,
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

impl CampView {
    pub(super) fn begin_order(&mut self) {
        self.order_draft = OrderDraft::new(self.snapshot.party.iter().map(|c| c.id).collect());
        self.page = if self.snapshot.party.len() < 2 {
            CampPage::OrderAlone
        } else {
            CampPage::Order
        };
        self.order_wait = ALONE_WAIT_FRAMES;
    }

    pub(super) fn order_frame(
        &mut self,
        runtime: &mut Runtime,
        up: bool,
        down: bool,
        accept: bool,
    ) {
        if self.page == CampPage::Order {
            self.order_draft.tick_cursor();
        }
        if self.page != CampPage::Order {
            self.order_wait = self.order_wait.saturating_sub(1);
            if accept || self.order_wait == 0 {
                self.page = if self.page == CampPage::OrderAlone {
                    CampPage::State
                } else {
                    CampPage::Root
                };
                if self.page == CampPage::Root {
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
                        self.page = CampPage::OrderDone;
                        self.order_wait = DONE_WAIT_FRAMES;
                    }
                    Err(_) => {
                        self.message = "CANNOT CHANGE ORDER".into();
                        self.page = CampPage::Unsupported;
                    }
                }
            }
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
