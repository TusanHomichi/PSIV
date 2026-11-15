//! The window routine's return, after its text and windows have closed.
//!
//! `Event_GetAndRunDialogue5` / `Event_RunDialogue5` (`$05ADF8..$05AE72`,
//! `ps4.asm:121718-121747`) run `PalFadeOut_ClrSpriteTbl` before destroying
//! panels, clearing the panel byte and returning. The scene cannot consume
//! its dialogue acknowledgement until that shared CRAM program finishes.

use psiv_core::{DialogueWindow, SceneEffect, SceneInput, SceneOp};

use crate::{Runtime, RuntimeEvent};

pub(crate) struct DialogueClose {
    /// None until the next field tick, after its palette clock: starting
    /// there makes the first fade level visible for a whole frame.
    left: Option<u16>,
    input: SceneInput,
}

impl Runtime {
    pub(crate) fn return_from_dialogue(&mut self, suspended: bool) {
        if self.scene.is_none() || matches!(self.scene_input, SceneInput::Choice(_)) {
            return;
        }
        let input = if suspended {
            SceneInput::DialogueClosed
        } else {
            SceneInput::DialogueEnded
        };
        if matches!(
            self.scene_dialogue_window(),
            Some(DialogueWindow::Cutscene | DialogueWindow::Cutscene5)
        ) {
            self.dialogue_close
                .get_or_insert(DialogueClose { left: None, input });
        } else {
            self.scene_panel_sprites = false;
            self.scene_input = input;
            self.sync_dialogue_stop_byte();
        }
    }

    /// Runs after CRAM's tick and before the scene. True parks the scene and
    /// its camera for this fade frame; false lets it consume the return.
    pub(crate) fn tick_dialogue_close(&mut self, events: &mut Vec<RuntimeEvent>) -> bool {
        let Some(mut close) = self.dialogue_close.take() else {
            return false;
        };
        match close.left {
            None => {
                let (cram, cycle_index) = self.effects.cram_and_cycle_index();
                close.left = cram.apply(
                    &SceneOp::FadeOut,
                    self.frames,
                    self.data.scene_palettes(),
                    None,
                    cycle_index,
                );
            }
            Some(0 | 1) => {
                self.translate_scene_effect(
                    SceneEffect::Presentation {
                        op: SceneOp::PanelDestroyAll,
                    },
                    events,
                );
                self.scene_panel_sprites = false;
                self.scene_input = close.input;
                self.sync_dialogue_stop_byte();
                return false;
            }
            Some(left) => close.left = Some(left - 1),
        }
        self.dialogue_close = Some(close);
        true
    }
}

#[cfg(test)]
mod tests;
