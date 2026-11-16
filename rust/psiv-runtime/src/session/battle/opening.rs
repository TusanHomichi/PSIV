//! Signed-priority load-to-options clocks, separate from narration.
//! For normal priority `loc_52D6` starts an eight-frame timer. Meanwhile
//! the fighters draw the status strip and grow the enemy name by two cells.
//! `loc_5366`, `loc_5380` and `Battle_OpenMainOptions` then open the options.
//! See docs/battle/BATTLE_COMMAND_UI.md, "The battle opening". Upload timing
//! outside the two measured rosters is a deliberate approximation recorded
//! in docs/source-notes/battle-party.md, "2026-11-15 — Normal-opening upload timing approximation".

use psiv_core::battle::Priority;

/// The signed dispatch at US `$530E`; ambush retains its existing path until #722.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Normal,
    Preemptive,
}

/// Progress in counted battle-loop calls, separate from display-only lag.
#[derive(Debug, Clone, Copy)]
pub(super) struct Opening {
    kind: Kind,
    step: u8,
    lag: u8,
    options_lag: u8,
    party_size: usize,
}

impl Opening {
    pub(super) fn new(priority: Priority, party_size: usize) -> Option<Self> {
        let kind = match priority {
            Priority::Normal => Kind::Normal,
            Priority::Preemptive => Kind::Preemptive,
            Priority::Ambush => return None,
        };
        Some(Self {
            kind,
            step: 0,
            lag: 1,
            options_lag: u8::try_from(party_size.saturating_sub(1)).unwrap_or(4) / 2,
            party_size,
        })
    }

    /// Advances the visible frame; false means VInt skipped the count/seed.
    pub(super) fn advance(&mut self) -> bool {
        if self.lag > 0 {
            self.lag -= 1;
            return false;
        }
        self.step += 1;
        if self.step == self.options_start() {
            self.lag = self.options_lag;
        }
        true
    }

    pub(super) fn name_width(self) -> u8 {
        if self.first_upload() && self.party_size >= 3 {
            return 0;
        }
        (2 * (self.step + 1)).min(12)
    }

    pub(super) fn first_upload(self) -> bool {
        self.step == 0 && self.lag != 0
    }

    fn options_start(self) -> u8 {
        match self.kind {
            Kind::Normal => 11,
            // Nine transient growth passes, 36 timer decrements and nine
            // close passes, then `$5380`. The origin already ran pass one.
            Kind::Preemptive => 54,
        }
    }

    pub(super) fn options_width(self) -> Option<u8> {
        (self.step >= self.options_start()).then(|| {
            // In the measured three-member preemptive fixture the first
            // options upload reaches video only after its held VInt.
            if self.kind == Kind::Preemptive && self.lag > 0 {
                0
            } else {
                2 * (self.step - self.options_start() + 1)
            }
        })
    }

    /// Visible transient width, including zero while the upload is pending.
    /// US `$48F6` / `$4974`; tape 07's video shows the preceding loop's write.
    pub(super) fn transient_width(self) -> Option<u8> {
        if self.kind != Kind::Preemptive {
            return None;
        }
        Some(match self.step {
            0 if self.first_upload() => 0,
            0..=7 => 2 * (self.step + 1),
            8..=44 => 18,
            45..=52 => 2 * (53 - self.step),
            _ => 0,
        })
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
        let advance_enemy_overlays = opening.advance();
        if advance_enemy_overlays {
            let _ = runtime.tick(psiv_core::Input::Neutral);
        }
        if opening.done(super::menu::top_width(runtime.vehicle_active())) {
            self.opening = None;
            self.command_open = true;
            self.message.clear();
            self.message_kind = super::MessageKind::None;
            // The full window has not called Battle_UpdateRedCursor2 yet.
            self.blink.red = false;
        }
        Some(super::BattleFrame {
            view: Some(self.view(runtime, false)),
            advance_enemy_overlays,
            ..super::BattleFrame::default()
        })
    }
}
