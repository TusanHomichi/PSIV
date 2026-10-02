//! The retail yes/no window's cursor and answer rules.
//!
//! `Win_YesNo` draws two rows and a cursor that starts on YES. Up and down are
//! edge-driven and swap the cursor; **Speak** answers the row the cursor is on
//! and **Cancel** is retail's direct NO shortcut, whatever the cursor shows.
//! Merely advancing the text never picks an answer.
//!
//! A scene branch without a preceding text choice (`SceneChoiceRequested`)
//! opens the same window as a standalone prompt: the answer belongs to the
//! scene, not to an entry's `$F5` operands, so the window closes on the answer
//! instead of continuing a flow.

use crate::pad::{Button, Pad};

/// One yes/no window's cursor and kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChoiceState {
    /// Row the cursor is on: 0 = YES, 1 = NO.
    cursor: usize,
    /// A scene's own branch choice, with no flow behind it.
    standalone: bool,
}

impl ChoiceState {
    /// The YES row.
    pub const YES: usize = 0;
    /// The NO row.
    pub const NO: usize = 1;

    /// The row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether the cursor is on YES.
    #[must_use]
    pub const fn is_yes(&self) -> bool {
        self.cursor == Self::YES
    }

    /// Whether this window is a scene's standalone branch prompt.
    #[must_use]
    pub const fn standalone(&self) -> bool {
        self.standalone
    }

    /// Opens a window with the cursor back at YES.
    pub(crate) fn reset(&mut self) {
        self.cursor = Self::YES;
        self.standalone = false;
    }

    /// Opens a scene's standalone branch prompt.
    pub(crate) fn reset_standalone(&mut self) {
        self.cursor = Self::YES;
        self.standalone = true;
    }

    /// The cursor movement this frame's presses ask for.
    pub(crate) fn move_cursor(&mut self, pressed: Pad) {
        if pressed.held(Button::Up) || pressed.held(Button::Down) {
            self.cursor = if self.is_yes() { Self::NO } else { Self::YES };
        }
    }

    /// The answer this frame's presses give, if any.
    ///
    /// Cancel wins over Speak because the cartridge reads it first on the same
    /// frame, and it answers NO regardless of where the cursor is.
    #[must_use]
    pub(crate) fn answer(&self, pressed: Pad) -> Option<bool> {
        if pressed.held(Button::Cancel) {
            return Some(false);
        }
        if pressed.held(Button::Speak) {
            return Some(self.is_yes());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ChoiceState;
    use crate::pad::{Button, Pad};

    #[test]
    fn the_cursor_starts_on_yes_and_up_down_toggle_it() {
        let mut choice = ChoiceState::default();
        assert!(choice.is_yes());
        choice.move_cursor(Pad::new(Button::Down));
        assert!(!choice.is_yes());
        choice.move_cursor(Pad::new(Button::Up));
        assert!(choice.is_yes());
        // Holding a direction across frames is one press, not many.
        choice.move_cursor(Pad::NEUTRAL);
        assert!(choice.is_yes());
    }

    #[test]
    fn speak_answers_the_cursor_and_cancel_answers_no() {
        let mut choice = ChoiceState::default();
        assert_eq!(choice.answer(Pad::new(Button::Speak)), Some(true));
        assert_eq!(choice.answer(Pad::new(Button::Cancel)), Some(false));
        assert_eq!(choice.answer(Pad::NEUTRAL), None);
        choice.move_cursor(Pad::new(Button::Down));
        assert_eq!(choice.answer(Pad::new(Button::Speak)), Some(false));
        // Cancel is NO even from the NO row, and beats Speak on one frame.
        assert_eq!(
            choice.answer(Pad::of(&[Button::Cancel, Button::Speak])),
            Some(false)
        );
    }
}
