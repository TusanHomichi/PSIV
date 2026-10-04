//! The byte under `Saved_Dialogue_Addr` after a dialogue closes.
//!
//! `Event_Gyuna` ends with `popdlg` and `cmpi.b #$35, (a0)`
//! (`$070C9A..$070CA6`): it reads whatever the text engine stored when the
//! last dialogue stopped. The engine stores with `pushdlg` in
//! `TextCtrlCode_Terminate2` (`ps4.asm:142643`, retail `$06A12C`), which the
//! `$F7` handler (`TextCtrlCode_Terminate3`, a `bra.w` to it) and both
//! terminators (`$FE`, `$FF`) reach. `RunText_CharacterLoop` has already read
//! the terminator with `move.b (a0)+, d0`, so the stored address is the byte
//! *after* it: for an `$F7` the next byte of the same entry, for a terminator
//! the first byte of the next entry (entries are the tree's `$FF`-separated
//! runs, `GetDialogueByID` counts them). That byte is not the last glyph
//! shown; it is the first of what the text would show next, and for
//! `Event_Gyuna`'s `$35` it is the `.` that opens entry 61 after the
//! space-ship answer (entry 60) ends.
//!
//! The pack stores entries as decoded segments, not bytes, so the first byte
//! of a segment is rebuilt: a control's code, or the first character's glyph
//! byte (`font.json`). The decoder drops `$F0`, `$F1`, `$F8` and `$FB`
//! (`TextCtrlCode_Null`) and the terminators; an entry that opens on one of
//! those would read differently, and
//! `tests::every_entry_opens_on_the_byte_its_segments_give` holds the rebuild
//! to the pack's own `raw_hex` for every entry.

use super::DialogueRunner;
use psiv_data::{DialogueSet, Segment};

/// An entry's terminator, which is also the first byte of an empty entry.
const TERMINATOR: u8 = 0xFF;

/// The first byte `segment` was decoded from.
pub(super) fn segment_byte(set: &DialogueSet, segment: &Segment) -> Option<u8> {
    match segment {
        Segment::Control(ctrl) => Some(ctrl.code()),
        Segment::Text(run) => {
            let first = run.chars().next()?;
            set.font.by_char.get(&first).copied()
        }
    }
}

/// The byte `pushdlg` leaves `(a0)` on after `entry_id` of `tree` stopped with
/// its next unread segment at `index`; `at_close` says an `$F7` stopped it.
pub(super) fn stop_byte_at(
    set: &DialogueSet,
    tree: u8,
    entry_id: u16,
    index: usize,
    at_close: bool,
) -> Option<u8> {
    let entry = set.entry(tree, entry_id)?;
    if at_close {
        // Inside the entry: the next segment, or the terminator when the `$F7`
        // was the last thing in it.
        return match entry.segments.get(index) {
            Some(segment) => segment_byte(set, segment),
            None => Some(TERMINATOR),
        };
    }
    // A terminator: the next entry's first byte (its own terminator if it is
    // empty); past the last entry there is nothing readable.
    let next = set.entry(tree, entry_id.checked_add(1)?)?;
    match next.segments.first() {
        Some(segment) => segment_byte(set, segment),
        None => Some(TERMINATOR),
    }
}

impl DialogueRunner {
    /// Records where the flow about to be dropped (or already suspended)
    /// stopped. Called by `close_window`, the one place every close passes.
    pub(super) fn note_stop(&mut self) {
        let Some(set) = self.set.as_deref() else {
            return;
        };
        let (tree, flow) = match (&self.flow, &self.suspended) {
            (Some(flow), _) => (self.tree, flow),
            (None, Some((tree, flow))) => (*tree, flow),
            (None, None) => return,
        };
        let Some((entry_id, index, at_close)) = flow.stop_cursor() else {
            return;
        };
        self.stop_byte = stop_byte_at(set, tree, entry_id, index, at_close);
    }

    /// The byte under the saved dialogue address after the last dialogue that
    /// stopped, which is what `popdlg` / `cmpi.b #n, (a0)` reads.
    #[must_use]
    pub(crate) fn stop_byte(&self) -> Option<u8> {
        self.stop_byte
    }
}

#[cfg(test)]
mod tests;
