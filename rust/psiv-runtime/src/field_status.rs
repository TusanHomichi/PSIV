//! Field status windows and the shared Game Over boundary.
use std::collections::VecDeque;

use psiv_core::{Cell, CharId, Effect, FieldStatusClock, SceneInput};

use crate::{Runtime, RuntimeEvent};

/// A blocking retail field-status window, acknowledged by the presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldNotice {
    /// `DisplayDeadCharacterMessage`, with the character's saved name.
    Fallen(CharId),
    /// `DisplayPerishedMessage`, followed by a return to the title.
    Perished,
}

#[derive(Default)]
pub(super) struct FieldStatus {
    pub(super) clock: FieldStatusClock,
    notices: VecDeque<FieldNotice>,
    // Windows can interrupt between the status pass and warp/event checks.
    pub(super) pending: Option<(Cell, Vec<Effect>)>,
    pub(super) flash_after_notices: bool,
}

impl Runtime {
    /// The oldest status window. Field control stays parked until acknowledged.
    #[must_use]
    pub fn field_notice(&self) -> Option<FieldNotice> {
        self.field_status.notices.front().copied()
    }

    /// Dismisses exactly the visible window; the final perished message ends play.
    pub fn acknowledge_field_notice(&mut self) {
        if self.field_status.notices.pop_front() == Some(FieldNotice::Perished) {
            self.end_game();
        }
    }

    /// Defeat has ended this runtime. START or CONTINUE must build a new one.
    #[must_use]
    pub const fn game_over(&self) -> bool {
        self.game_over
    }

    pub(super) fn end_game(&mut self) {
        self.game_over = true;
        self.scene = None;
        self.scene_battle = None;
        self.scene_input = SceneInput::None;
        self.scene_triggers_pending = false;
        self.battle_field_refresh_pending = false;
        self.field_status.pending = None;
        self.field_status.notices.clear();
        self.field_status.flash_after_notices = false;
    }

    pub(super) fn update_field_status(&mut self, cell: Cell, events: &mut Vec<RuntimeEvent>) {
        if self.field_suspended
            || self.map.collision_at(cell) == Some(psiv_core::CollisionType::MapChange)
        {
            return;
        }
        let poisons = self.map_record().is_some_and(|map| map.flags.poisons());
        let result = self
            .field_status
            .clock
            .step(&mut self.game, poisons, &mut self.rng);
        self.field_status
            .notices
            .extend(result.fallen.into_iter().map(FieldNotice::Fallen));
        if result.perished {
            self.field_status.notices.push_back(FieldNotice::Perished);
        } else if result.flash_red {
            if self.field_notice().is_some() {
                self.field_status.flash_after_notices = true;
            } else {
                events.push(RuntimeEvent::FieldPoisonFlash);
            }
        }
    }
}
