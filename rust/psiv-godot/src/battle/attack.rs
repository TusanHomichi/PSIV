//! Proven enemy attack timing on the current battle body surface.
//!
//! The retail extraction proves the fixed mapping clock for a subset of
//! attack-object graphs. It does not yet prove how those mapping pointers map
//! to the already-rendered body sheet, so this module presents only the proven
//! timed attack beat as a brief body flash. It deliberately does not invent a
//! lunge or replace a sprite texture.

use godot::classes::Sprite2D;
use godot::prelude::*;

use psiv_core::battle::FighterId;
use psiv_runtime::BattleAnimationEvent;

use super::enemy_overlay::EnemyAnimation;

/// One enemy body and its already-decoded idle overlay animation.
pub(super) struct EnemySprite {
    pub(super) fighter: FighterId,
    pub(super) node: Gd<Sprite2D>,
    pub(super) animation: Option<EnemyAnimation>,
    pub(super) attack: Option<EnemyAttackState>,
}

/// Timed attack beat. Position and sprite-sheet mapping remain intentionally
/// absent until the retail mapping pointers are decoded to body pixels.
pub(super) struct EnemyAttackState {
    remaining: u16,
}

impl EnemySprite {
    pub(super) fn begin_attack(&mut self, event: &BattleAnimationEvent) {
        let Some(total_frames) = event.total_frames.filter(|frames| *frames > 0) else {
            return;
        };
        if !event.flash_timing_proven {
            return;
        }
        self.attack = Some(EnemyAttackState {
            remaining: total_frames,
        });
        self.node
            .set_self_modulate(Color::from_rgba8(255, 224, 224, 255));
    }

    pub(super) fn advance_attack(&mut self) {
        let Some(attack) = self.attack.as_mut() else {
            return;
        };
        if attack.remaining > 1 {
            attack.remaining -= 1;
            return;
        }
        self.attack = None;
        self.node
            .set_self_modulate(Color::from_rgba8(255, 255, 255, 255));
    }

    pub(super) fn clear_attack(&mut self) {
        self.attack = None;
        self.node
            .set_self_modulate(Color::from_rgba8(255, 255, 255, 255));
    }
}

/// Converts the formation position byte and art dimensions into a top-left
/// pixel. The position byte is the body's bottom-right column.
pub(super) fn enemy_sprite_origin(position: u8, width_cells: u16, height_cells: u16) -> (i32, i32) {
    let column = i32::from(position & 0x7F);
    (
        (column - i32::from(width_cells)) * 8,
        (15 - i32::from(height_cells)) * 8,
    )
}
