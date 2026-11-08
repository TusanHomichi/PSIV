//! A battle window opening or closing: `Battle_CreateWindow` and
//! `Battle_DestroyWindow` (`ps4.asm:1657`, `1678`) as frame state.
//!
//! Both keep the width in `$FFFF418C` and change it by two cells a call, one
//! call a frame. `Battle_CreateWindow` draws an empty window that wide until
//! the width reaches the window's `d1`, then the whole window, and returns
//! past its caller's `rts`: that frame the routine moves on. `Battle_DestroyWindow`
//! narrows the empty window until the width reaches zero, draws nothing then,
//! and returns past the `rts` the same way. Neither frame reads a button.
//! Oracle tapes 32 and 37 show every case (`docs/battle/BATTLE_COMMAND_UI.md`).

/// Cells either routine moves a frame (`addq.w #2` / `subq.w #2`,
/// `ps4.asm:1658`, `1679`).
pub(crate) const STEP: u8 = 2;

/// What one frame of a phase did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step<T> {
    /// Still opening or closing.
    Running,
    /// The window was drawn whole this frame; its own routine runs next.
    Whole,
    /// The window is gone; `T` happens on this frame.
    Gone(T),
}

/// A window opening or closing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase<T> {
    /// Opening: `setup` frames before the first `Battle_CreateWindow` call
    /// draws anything, then the width reached so far.
    Grow { setup: u8, width: u8 },
    /// Closing from `width`; `then` happens on the frame it is gone.
    Shrink { width: u8, then: T },
}

impl<T: Copy> Phase<T> {
    /// Opening from nothing.
    pub(crate) const fn grow() -> Self {
        Phase::Grow { setup: 0, width: 0 }
    }

    /// Closing a window `full` cells wide. `$FFFF418C` holds the width the
    /// growth stopped at, the first even number not below `full`: a 15-cell
    /// window closes from 16.
    pub(crate) const fn shrink(full: u8, then: T) -> Self {
        Phase::Shrink {
            width: (full + 1) & !1,
            then,
        }
    }

    /// One call of the routine, for a window `full` cells wide.
    pub(crate) fn step(&mut self, full: u8) -> Step<T> {
        match self {
            Phase::Grow { setup, .. } if *setup > 0 => {
                *setup -= 1;
                Step::Running
            }
            Phase::Grow { width, .. } => {
                *width = width.saturating_add(STEP);
                if *width >= full {
                    Step::Whole
                } else {
                    Step::Running
                }
            }
            Phase::Shrink { width, then } => {
                *width = width.saturating_sub(STEP);
                if *width == 0 {
                    Step::Gone(*then)
                } else {
                    Step::Running
                }
            }
        }
    }

    /// The empty window drawn this frame, `None` while the whole window
    /// still shows (a close's first frame). Zero draws nothing.
    pub(crate) fn box_width(&self, full: u8) -> Option<u8> {
        match *self {
            Phase::Grow { width, .. } => Some(width),
            Phase::Shrink { width, .. } if width >= full => None,
            Phase::Shrink { width, .. } => Some(width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mut phase: Phase<()>, full: u8) -> (Vec<Option<u8>>, Step<()>) {
        let mut boxes = Vec::new();
        loop {
            let step = phase.step(full);
            if step != Step::Running {
                return (boxes, step);
            }
            boxes.push(phase.box_width(full));
        }
    }

    #[test]
    fn a_window_grows_two_cells_a_frame_and_is_whole_at_its_width() {
        let (boxes, step) = run(Phase::grow(), 12);
        assert_eq!(boxes, [Some(2), Some(4), Some(6), Some(8), Some(10)]);
        assert_eq!(step, Step::Whole);
        // An odd width: 14 is the last box, then whole.
        let (boxes, _) = run(Phase::grow(), 15);
        assert_eq!(boxes.last(), Some(&Some(14)));
    }

    #[test]
    fn a_setup_frame_draws_nothing_before_the_growth() {
        let mut phase: Phase<()> = Phase::Grow { setup: 1, width: 0 };
        assert_eq!(phase.step(16), Step::Running);
        assert_eq!(phase.box_width(16), Some(0));
        assert_eq!(phase.step(16), Step::Running);
        assert_eq!(phase.box_width(16), Some(2));
    }

    #[test]
    fn a_window_shrinks_from_its_growth_width_and_is_gone_at_zero() {
        let phase = Phase::shrink(12, ());
        assert_eq!(phase.box_width(12), None, "the press frame still shows it");
        let (boxes, step) = run(phase, 12);
        assert_eq!(boxes, [Some(10), Some(8), Some(6), Some(4), Some(2)]);
        assert_eq!(step, Step::Gone(()));
        let (boxes, _) = run(Phase::shrink(15, ()), 15);
        assert_eq!(
            boxes.first(),
            Some(&Some(14)),
            "a 15-cell window closes from 16"
        );
        assert_eq!(boxes.len(), 7);
    }
}
