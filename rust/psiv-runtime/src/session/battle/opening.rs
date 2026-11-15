//! `loc_52D6` starts an eight-frame timer, not a narration dwell. Meanwhile
//! the fighters draw the status strip and grow the enemy name by two cells.
//! `loc_5366`, `loc_5380` and `Battle_OpenMainOptions` then open the options.
//! See docs/battle/BATTLE_COMMAND_UI.md, "The battle opening".

/// Progress in counted battle-loop calls, separate from display-only lag.
#[derive(Debug, Clone, Copy)]
pub(super) struct Opening {
    step: u8,
    lag: u8,
    options_lag: u8,
}

impl Opening {
    pub(super) fn new(party_size: usize) -> Self {
        Self {
            step: 0,
            lag: 1,
            options_lag: u8::try_from(party_size.saturating_sub(1)).unwrap_or(4) / 2,
        }
    }

    /// Advances the visible frame; false means VInt skipped the count/seed.
    pub(super) fn advance(&mut self) -> bool {
        if self.lag > 0 {
            self.lag -= 1;
            return false;
        }
        self.step += 1;
        if self.step == 11 {
            self.lag = self.options_lag;
        }
        true
    }

    pub(super) fn name_width(self) -> u8 {
        (2 * (self.step + 1)).min(12)
    }

    pub(super) fn options_width(self) -> Option<u8> {
        (self.step >= 11).then(|| 2 * (self.step - 10))
    }

    pub(super) fn done(self, full_width: u8) -> bool {
        self.options_width()
            .is_some_and(|width| width >= full_width)
    }
}

impl super::BattleMode {
    pub(super) fn frame_opening(
        &mut self,
        runtime: &mut crate::Runtime,
    ) -> Option<super::BattleFrame> {
        let opening = self.opening.as_mut()?;
        if opening.advance() {
            let _ = runtime.tick(psiv_core::Input::Neutral);
        }
        if opening.done(super::menu::top_width(runtime.vehicle_active())) {
            self.opening = None;
            self.command_open = true;
            // The full window has not called Battle_UpdateRedCursor2 yet.
            self.blink.red = false;
        }
        Some(super::BattleFrame {
            view: Some(self.view(runtime, false)),
            ..super::BattleFrame::default()
        })
    }
}
