//! The Aiedo inn's palette and display handoff after its scene.

use psiv_core::palette::{self, PALETTE_WORDS};

use super::{Cram, Pass, Program};

/// The `DMAPlane_A_VInt` loop the Aiedo inn waits after its scene, before
/// its `Pal_FadeIn` (`moveq #$3B, d0`, `ps4.asm:136436-136439`).
pub(crate) const INN_RESUME_HOLD: u16 = 60;

impl Cram {
    /// The Aiedo inn's resume after `Event_GirlsSneakingOut`, whose
    /// `PalFadeOut_ClrSpriteTbl` left the screen black (`ps4.asm:136411-136453`):
    /// the palette the inn kept before the event (`$FFFF1300`) goes back into
    /// both buffers with the display still off, the bill's window is drawn and
    /// [`INN_RESUME_HOLD`] frames of `DMAPlane_A_VInt` pass, then `Pal_FadeIn`
    /// spends its display frame and ramps up a pass a frame. The live words
    /// stay black until then, as the switched-off display shows them (#653).
    /// The first frame runs now.
    pub(crate) fn inn_resume_fade_in(
        &mut self,
        words: [u16; PALETTE_WORDS],
        main_frame_count: u16,
        cycle_index: &mut u8,
    ) {
        self.unshown = None;
        self.queued = None;
        self.saved = words;
        self.view.display = false;
        self.display_change = Some((INN_RESUME_HOLD + 1, true));
        self.set_live([0; PALETTE_WORDS]);
        self.start(
            Program::Passes {
                kind: Pass::VariableIn,
                elapsed: 0,
                first: INN_RESUME_HOLD + 1,
                every: palette::dbf_frames(0),
                count: palette::variable_fade_in_passes(&words),
                words: PALETTE_WORDS,
            },
            main_frame_count,
            cycle_index,
        );
    }
}
