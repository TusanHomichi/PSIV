//! Where a finished flow stopped, for [`DialogueRunner::stop_byte`].
//!
//! A child of `text_flow` so it can read the flow's cursor without widening
//! the fields' visibility.
//!
//! [`DialogueRunner::stop_byte`]: crate::dialogue::DialogueRunner::stop_byte

use super::TextFlow;
use psiv_data::PageEnd;

impl TextFlow {
    /// The cursor the cartridge's `pushdlg` would have stored: `(entry id,
    /// index of the next segment, whether an `$F7` stopped it)`. `None` while
    /// the flow has not stopped (a window closed by a `$F6` or a fault stores
    /// nothing).
    pub(in crate::dialogue) fn stop_cursor(&self) -> Option<(u16, usize, bool)> {
        (self.stop.is_some() || self.done).then_some((
            self.entry_id,
            self.index,
            self.stop == Some(PageEnd::Close),
        ))
    }
}
