//! `Event_MonsenEarthquake`'s camera shake ([`psiv_core::PresentationOp::Quake`]).
//!
//! Each frame the routine loads a table entry into the camera's step counters
//! and folds them in, so the view jumps eight pixels and back. Both tables are
//! in `docs/scenes/143_MonsenEarthquake.md`. The scene waits the frames itself.

use psiv_core::{SceneEffect, SceneOp};

use crate::{Runtime, RuntimeEvent};

/// `SFXID_GraveOpening`.
const SHAKE_SOUND: u8 = 0xDD;

/// A running shake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Quake {
    frame: u16,
    frames: u16,
    opening: bool,
}

impl Quake {
    pub(crate) const fn new(frames: u16, opening: bool) -> Quake {
        Quake {
            frame: 0,
            frames,
            opening,
        }
    }

    /// The step this frame: `loc_6E73A`'s two rows when it opens, then
    /// `loc_6E6B6`'s, a row a frame, every other frame coming back.
    const fn step(&self) -> (i32, i32) {
        let back = self.frame % 2 == 1;
        let x = if back { -8 } else { 8 };
        let y = if self.opening && self.frame < 2 {
            -x
        } else {
            0
        };
        (x, y)
    }
}

impl Runtime {
    /// One frame of a running shake: the view's step, and the sound on every
    /// fourth frame.
    pub(crate) fn tick_quake(&mut self, events: &mut Vec<RuntimeEvent>) {
        let Some(mut quake) = self.quake else {
            return;
        };
        let (dx, dy) = quake.step();
        let (x, y) = self.camera.position();
        self.set_camera(x + dx, y + dy);
        if quake.frame % 4 == 0 {
            self.translate_scene_effect(
                SceneEffect::Presentation {
                    op: SceneOp::PlaySound { id: SHAKE_SOUND },
                },
                events,
            );
        }
        quake.frame += 1;
        self.quake = (quake.frame < quake.frames).then_some(quake);
    }
}

#[cfg(test)]
mod tests {
    use super::Quake;

    fn steps(frames: u16, opening: bool) -> Vec<(i32, i32)> {
        (0..frames)
            .map(|frame| {
                Quake {
                    frame,
                    frames,
                    opening,
                }
                .step()
            })
            .collect()
    }

    /// `loc_6E6B6`: `+8`, `-8`, on X only.
    #[test]
    fn the_plain_shake_is_x_only() {
        assert_eq!(steps(4, false), [(8, 0), (-8, 0), (8, 0), (-8, 0)]);
    }

    /// `loc_6E73A` for two frames, then `loc_6E6B6`.
    #[test]
    fn the_opening_shake_has_two_diagonal_frames() {
        assert_eq!(steps(4, true), [(8, -8), (-8, 8), (8, 0), (-8, 0)]);
    }

    /// Both lengths the routine uses end where they began.
    #[test]
    fn the_frames_cancel() {
        for (frames, opening) in [(120, false), (60, true)] {
            let (x, y) = steps(frames, opening)
                .iter()
                .fold((0, 0), |at, step| (at.0 + step.0, at.1 + step.1));
            assert_eq!((x, y), (0, 0));
        }
    }
}
