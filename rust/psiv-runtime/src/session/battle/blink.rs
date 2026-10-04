//! The red cursor's blink: `Battle_UpdateRedCursor` and
//! `Battle_UpdateRedCursor2` (`ps4.asm:1561-1620`).
//!
//! The selected row's cursor tile is red (`$6E8`) or blue (`$6E7`). Two words
//! of battle RAM run the cycle, `$FFFF41D2` (a countdown) and `$FFFF41D4` (the
//! phase), and every frame a red-cursor window runs its routine they advance:
//!
//! - an accept press (`ButtonSpeak` or `ButtonCamp`) shows red and touches
//!   nothing;
//! - otherwise the countdown drops; when it reaches zero the phase toggles
//!   and the countdown reloads with `$19` (25 frames), or with `$0F` (15
//!   frames) when the phase just became blue;
//! - then any held button shows red, else phase 0 shows red and phase 1 blue.
//!
//! The timers are battle RAM, cleared when the battle loads, and nothing else
//! writes them, so the phase is the count of frames the red-cursor windows
//! have run since the battle began.

/// The two timer words and what they decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CursorBlink {
    /// `$FFFF41D2`.
    timer: i16,
    /// `$FFFF41D4`.
    phase: u16,
    /// Whether the selected row is red this frame.
    pub(super) red: bool,
}

impl Default for CursorBlink {
    fn default() -> CursorBlink {
        CursorBlink {
            timer: 0,
            phase: 0,
            red: true,
        }
    }
}

impl CursorBlink {
    /// A blink in the state the timers hold: for fixtures that start where an
    /// oracle receipt read `$FFFF41D2` and `$FFFF41D4`.
    pub(super) fn from_timers(timer: i16, phase: u16) -> CursorBlink {
        CursorBlink {
            timer,
            phase,
            red: true,
        }
    }

    /// One frame of a red-cursor window's routine. `held` is
    /// `Joypad_Held != 0` and `accepting` is an accept edge in
    /// `Joypad_Pressed`.
    pub(super) fn tick(&mut self, held: bool, accepting: bool) {
        if accepting {
            self.red = true;
            return;
        }
        self.timer = self.timer.wrapping_sub(1);
        if self.timer <= 0 {
            self.phase = (self.phase + 1) & 1;
            self.timer = 0x19;
            if self.phase != 0 {
                self.timer = 0x0F;
            }
        }
        self.red = held || self.phase == 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(frames: usize) -> Vec<bool> {
        let mut blink = CursorBlink::default();
        (0..frames)
            .map(|_| {
                blink.tick(false, false);
                blink.red
            })
            .collect()
    }

    #[test]
    fn the_cursor_runs_fifteen_blue_then_twenty_five_red() {
        // A cleared battle starts with the countdown expired: the first frame
        // flips to the blue phase and reloads the short count.
        let frames = run(80);
        assert!(frames[..15].iter().all(|red| !red), "15 blue frames first");
        assert!(frames[15..40].iter().all(|red| *red), "then 25 red frames");
        assert!(
            frames[40..55].iter().all(|red| !red),
            "and the cycle repeats"
        );
        assert!(frames[55..80].iter().all(|red| *red));
    }

    #[test]
    fn a_held_button_or_an_accept_press_shows_red_without_stopping_the_timers() {
        let mut blink = CursorBlink::default();
        blink.tick(true, false);
        assert!(blink.red, "any held button shows red");
        let timer = blink.timer;
        blink.tick(false, true);
        assert!(
            blink.red && blink.timer == timer,
            "accept shows red and holds the timers"
        );
        blink.tick(false, false);
        assert!(!blink.red, "the phase underneath is still blue");
    }
}
