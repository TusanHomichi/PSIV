//! Game over: the defeat fade that ends play and returns to the title.
//!
//! Retail's defeat does not restart play where it stopped: `end_game` clears
//! the scene, the battle and the pending notices, and the presentation fades
//! to black before the front door comes back. A battle defeat reaches
//! `Battle_DefeatedMsg` (`ps4.asm:6458`), which sets `Game_Mode_Index` back to
//! 0 — the title — after its palette fade and plane clear; a field defeat
//! reaches `DisplayPerishedMessage` (`ps4.asm:117087`), whose window dismisses
//! on `ButtonCancel|ButtonSpeak|ButtonCamp`, fades through
//! `PalFadeOut_ClrSpriteTbl` (`ps4.asm:117115`) and returns through
//! `MainGameProgram_Continue`.
//!
//! The fade is *frames*, not decoration: it gates when the title returns, and
//! nothing else runs while it does — not the field, not the shared seed. This
//! mode is that gate. The shell plays the fade's picture (a scene transition
//! and the title rebuild); the count that decides when it ends is here.

/// The fade's length: the shell's old `defeat_fade` counted to 14 before it
/// restored the title, and the S5 move keeps that count frame for frame.
pub const GAME_OVER_FADE_FRAMES: u16 = 14;

/// What one fade frame produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameOverFrame {
    /// The frames the fade has run: 0 on the frame it began.
    pub frames: u16,
    /// Whether the title takes this frame back. The shell tears its defeat
    /// picture down here (the last retail sound cue included) and builds the
    /// front door.
    pub title_restored: bool,
}

use super::title::{TitleFrame, TitleMode};
use super::{Frame, FrameMode, Mode, Session};

/// The fade's own state.
#[derive(Debug, Default)]
pub(crate) struct GameOverMode {
    frames: u16,
}

impl GameOverMode {
    /// The fade, at its first frame.
    pub(crate) fn new() -> GameOverMode {
        GameOverMode { frames: 0 }
    }

    /// One fade frame. `frames` counts from 0; the title returns on the frame
    /// the count reaches [`GAME_OVER_FADE_FRAMES`].
    pub(crate) fn frame(&mut self) -> GameOverFrame {
        let frames = self.frames;
        self.frames = self.frames.saturating_add(1);
        GameOverFrame {
            frames,
            title_restored: frames >= GAME_OVER_FADE_FRAMES,
        }
    }
}

impl Session {
    /// One frame of the defeat fade.
    ///
    /// Nothing else runs: the runtime's tick refuses while `game_over` is up,
    /// and this mode does not call it. On the frame the count expires the title
    /// takes the session back, over the slots the defeat left untouched, and
    /// the confirm press is blocked so the fade's last press cannot read as a
    /// talk on the field's next frame.
    pub(crate) fn game_over_frame(&mut self) -> Frame {
        let fade = match &mut self.mode {
            Mode::GameOver(mode) => mode.frame(),
            _ => unreachable!("game_over_frame runs only in game-over mode"),
        };
        if !fade.title_restored {
            return Frame {
                mode: FrameMode::GameOver,
                game_over: Some(fade),
                ..Frame::default()
            };
        }
        let slots = self.save_slots();
        let autostart = self.title_autostart;
        self.mode = Mode::Title(Box::new(TitleMode::new(slots, autostart)));
        self.accept_blocked = true;
        let view = match &self.mode {
            Mode::Title(mode) => mode.view(),
            _ => unreachable!("the title was just installed"),
        };
        Frame {
            mode: FrameMode::GameOver,
            game_over: Some(fade),
            title: Some(TitleFrame {
                view: Some(view),
                entered: None,
                erased: None,
                failure: None,
            }),
            ..Frame::default()
        }
    }
}

#[cfg(test)]
mod game_over_tests {
    use super::*;

    #[test]
    fn the_fade_runs_fourteen_frames_and_then_restores_the_title() {
        let mut fade = GameOverMode::new();
        assert_eq!(
            fade.frame(),
            GameOverFrame {
                frames: 0,
                title_restored: false
            },
            "the fade begins on its own first frame"
        );
        for frames in 1..GAME_OVER_FADE_FRAMES {
            assert_eq!(
                fade.frame(),
                GameOverFrame {
                    frames,
                    title_restored: false
                }
            );
        }
        assert_eq!(
            fade.frame(),
            GameOverFrame {
                frames: GAME_OVER_FADE_FRAMES,
                title_restored: true
            },
            "the title takes the frame after fourteen fade frames"
        );
    }
}
