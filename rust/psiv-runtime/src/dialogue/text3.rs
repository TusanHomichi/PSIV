//! `RunText3` (`ps4.asm:144053`): the ending's text loop, which reads no pad.
//!
//! The ending's windows (`DialogueWindow::EndingIntro`, the inline
//! `RunDialogue(9)` at `$078F68..$078F90`, and `DialogueWindow::Ending`,
//! `Event_RunDialogue4` at `$5ADA2`) run the same bytes as `RunText`, through
//! a loop that never calls `ReadJoypads`'s results:
//!
//! - one glyph a frame (`moveq #0, d0` before `.wait`, the US branch), held
//!   or not;
//! - no scroll arrow;
//! - a page ends where `RunText`'s would (`Text3CtrlCode_Interrupt` on a full
//!   second line or a `$FD`, `Text3CtrlCode_Terminate` on `$F7` or the
//!   entry's end), then waits `9 x` the page's glyph count (`$FFFFED5B`) plus
//!   one frames: `d2 = 12n - 3n`, and `dbf d2` runs `9n + 1` `DMAPlane_A_VInt`s.
//!   Then it clears the page and runs on, or returns.
//!
//! The glyph that fills the second line branches to the interrupt before its
//! own `.wait`, so that page's wait starts on the glyph's own frame: `9n + 1`
//! frames from the last glyph to the turn, where every other page has `9n + 2`.
//!
//! Tape 38 shows every page of entry `$09` turning on these counts
//! (`runner_tests/text3.rs`).

use psiv_data::{CHARS_PER_LINE, LINES_PER_WINDOW};

use super::{DialogueRunner, TextFlow};
use psiv_core::GameState;

/// `Text3CtrlCode_Terminate`/`Text3CtrlCode_Interrupt`'s wait per glyph:
/// `mulu.w #$C` then three `sub.w d1`.
const FRAMES_PER_GLYPH: u16 = 9;

/// The wait's `dbf` runs once more than its count.
const WAIT_EXTRA_FRAMES: u16 = 1;

impl DialogueRunner {
    /// Selects the text loop of the next scene window: `RunText3` for the
    /// ending's windows ([`psiv_core::DialogueWindow::runs_text3`]),
    /// `RunText` otherwise. Every non-scene open selects `RunText` itself.
    pub(crate) fn set_text3(&mut self, on: bool) {
        self.text3 = on;
        self.text3_wait = None;
    }

    /// Whether the window on screen runs `RunText3`.
    #[must_use]
    fn is_text3(&self) -> bool {
        self.text3 && self.flow.is_some()
    }

    /// Frames from the frame the page's last glyph is drawn to the last of
    /// its wait: `9n + 1`, one fewer for a page whose last glyph filled the
    /// second line (it has no `.wait` of its own).
    fn text3_page_frames(flow: &TextFlow) -> u16 {
        let glyphs = u16::try_from(flow.visible_chars()).unwrap_or(u16::MAX);
        let wait = glyphs
            .saturating_mul(FRAMES_PER_GLYPH)
            .saturating_add(WAIT_EXTRA_FRAMES);
        let lines = flow.lines();
        let filled = lines.len() == LINES_PER_WINDOW
            && lines
                .last()
                .is_some_and(|line| line.chars().count() == CHARS_PER_LINE);
        if filled { wait - 1 } else { wait }
    }

    /// The window half's clock: once the page is on screen (every glyph
    /// drawn, its controls run, its portrait up), its wait starts; each
    /// frame after runs one frame of it.
    pub(super) fn text3_tick(&mut self) {
        if !self.is_text3() {
            return;
        }
        if let Some(wait) = self.text3_wait.as_mut() {
            *wait = wait.saturating_sub(1);
        } else if self.is_dismissable()
            && let Some(flow) = self.flow.as_ref()
        {
            self.text3_wait = Some(Self::text3_page_frames(flow));
        }
    }

    /// The input half: the frame after the wait's last, the page turns (or
    /// the window ends, `$F7` saving its cursor) as a press would turn it.
    /// Returns whether the window runs `RunText3`, so the caller reads no
    /// press.
    pub(super) fn text3_input(&mut self, game: &mut GameState) -> bool {
        if !self.is_text3() {
            return false;
        }
        if self.text3_wait == Some(0) {
            self.text3_wait = None;
            self.advance(game);
        }
        true
    }
}
