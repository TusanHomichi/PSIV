//! `$F2` waits inside `RunText` and `RunText3` (`TextActionsOffs`).

use super::{DialogueAction, DialogueRunner, DialogueSignal, TextFlow};
use psiv_core::{DialogueWindow, GameState, SceneOp};

impl DialogueRunner {
    pub(super) fn action_holding(&self) -> bool {
        self.flow.as_ref().is_some_and(TextFlow::action_holding)
    }

    /// `Panel_Num` is global on the cartridge, including panels made by the
    /// scene before it opens a window. A close destroys the window's panels.
    pub(crate) fn scene_panel_op(&mut self, op: &SceneOp) {
        match op {
            SceneOp::PanelCreate { .. } => self.panels = self.panels.wrapping_add(1),
            SceneOp::PanelDestroyLast => self.panels = self.panels.wrapping_sub(1),
            SceneOp::PanelDestroyAll => self.panels = 0,
            _ => {}
        }
    }

    /// `Event_RunDialogue2` zeroes the count without erasing its pixels;
    /// the standard and fifth routines destroy the panels. The third and
    /// ending routines leave the count alone (`DIALOGUE_CALLERS.md`).
    pub(crate) fn return_panels(&mut self, window: Option<DialogueWindow>) {
        if matches!(
            window,
            Some(
                DialogueWindow::Standard
                    | DialogueWindow::Retained
                    | DialogueWindow::Cutscene
                    | DialogueWindow::Cutscene5
            )
        ) {
            self.panels = 0;
        }
    }

    /// Counted VInts in the handler, before the byte loop can continue.
    fn action_frames(&mut self, action: DialogueAction) -> u16 {
        match action {
            DialogueAction::LoadPanel(_) => {
                self.panels = self.panels.wrapping_add(1);
                1 // `TextAction_LoadPanel`: `DMAPlanes_VInt`.
            }
            DialogueAction::DestroyLastPanel => {
                let up = self.panels > 0;
                self.panels = self.panels.saturating_sub(1);
                u16::from(up)
            }
            DialogueAction::DestroyAllPanels => {
                let up = self.panels > 0;
                self.panels = 0;
                u16::from(up)
            }
            // Even sound zero waits, although LoadSound does not write it.
            DialogueAction::LoadSound(_)
            | DialogueAction::LoadSound2(_)
            | DialogueAction::UpdatePalette
            | DialogueAction::PauseMusic
            | DialogueAction::ResumeMusic => 1, // `VInt_Prepare`.
            DialogueAction::ElsydeonBroken => 2 * super::ELSYDEON_BROKEN_FRAMES,
            _ => 0,
        }
    }

    pub(super) fn launch_action(&mut self, action: DialogueAction, game: &GameState) {
        // A retained window zeroes `Panel_Num` while leaving its pixels.
        // An empty destroy must not pop that presentation stack either.
        let empty_destroy = self.panels == 0
            && matches!(
                action,
                DialogueAction::DestroyLastPanel | DialogueAction::DestroyAllPanels
            );
        let frames = self.action_frames(action);
        if !empty_destroy {
            self.signals.push(DialogueSignal::Action(action));
        }
        if let Some(flow) = self.flow.as_mut() {
            flow.resume_after_action(frames, game);
        }
        self.service_flow(game);
        self.drain_flow_log();
    }

    /// Release actions at their byte positions. A timed action yields before
    /// the next control or glyph; only zero-frame actions drain together.
    pub(super) fn pump_actions(&mut self, game: &mut GameState) {
        loop {
            let ready = self
                .flow
                .as_ref()
                .is_some_and(|flow| !self.is_opening() && flow.action_ready(self.revealed));
            if !ready {
                return;
            }
            let Some(action) = self.flow.as_mut().and_then(TextFlow::take_pending_action) else {
                return;
            };
            if let DialogueAction::SetEventFlag(flag) = action {
                let _ = game.set(psiv_core::Flag::event(u16::from(flag)));
            }
            if let DialogueAction::LoadPanel(id) = action {
                let lag = panel_lag_frames(id);
                if lag > 0 {
                    let frames = self.action_frames(action);
                    if let Some(flow) = self.flow.as_mut() {
                        flow.resume_after_action(frames, game);
                    }
                    self.loading_panel = Some(action);
                    self.lag = lag;
                    return;
                }
            }
            self.launch_action(action, game);
        }
    }
}

/// Tape 38, inside a page (no page-clear/setup lag mixed in): `$143`
/// holds `Main_Frame_Count` on frames 9006-9009, `$144` on 9016-9026.
/// The other panel loads on that tape follow a page clear or window setup;
/// their combined lag has not been isolated into a panel-only measurement.
fn panel_lag_frames(id: u16) -> u8 {
    match id {
        0x143 => 4,
        0x144 => 11,
        _ => 0,
    }
}
