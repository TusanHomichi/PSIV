//! GameStart's 20-pass colour helpers. Each pass spends the caller's VInt;
//! the floor path of `$073EA4` first clears plane A and spends its DMA VInt.

use super::{Blocked, SceneRunner};
use crate::{PresentationOp, SceneEffect, SceneOp};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Intro {
    pub colour: u16,
    left: Option<u8>,
    /// The floor path's DMA returned; the caller's bare VInt is next.
    dma_return: bool,
}

impl SceneRunner {
    /// Supply retail's persistent `Main_Frame_Count` for the next tick.
    /// Standalone runners count from zero until the caller supplies it.
    pub fn set_main_frame_count(&mut self, frame: u16) {
        self.main_frame_count = frame.wrapping_sub(1);
    }

    pub(super) fn intro_fade(&mut self, op: SceneOp, effects: &mut Vec<SceneEffect>) {
        let left = *self.intro.left.get_or_insert_with(|| {
            effects.push(SceneEffect::Presentation { op });
            20
        });
        if left == 0 {
            self.intro.left = None;
            self.pc += 1;
            return;
        }
        let emit = |effects: &mut Vec<SceneEffect>, op| {
            effects.push(SceneEffect::Presentation { op });
        };
        if self.intro.dma_return {
            self.intro.dma_return = false;
        } else if self.main_frame_count & 3 == 0 {
            let word = self.intro.colour;
            if op == SceneOp::IntroTextFadeDown && word == 0x0666 {
                emit(
                    effects,
                    SceneOp::Presentation {
                        op: PresentationOp::ClearPlanes,
                    },
                );
                emit(effects, SceneOp::DmaPlanes);
                self.intro.dma_return = true;
                self.blocked = Blocked::Ticks(1);
                return;
            }
            let next = if op == SceneOp::IntroTextFadeUp {
                let next = word.wrapping_add(0x0222);
                if next & 0x1000 == 0 { next } else { word }
            } else {
                word.wrapping_sub(0x0222)
            };
            self.intro.colour = next;
            emit(effects, SceneOp::SetTextColour { colour: next });
        }
        emit(effects, SceneOp::WaitFrames { frames: 1 });
        self.intro.left = Some(left - 1);
        self.blocked = Blocked::Ticks(1);
    }
}

#[cfg(test)]
mod tests;
