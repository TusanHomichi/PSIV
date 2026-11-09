//! Retail enemy attack frame playback and motion.
//!
//! `BattleAnimationEvent` remains the dispatch surface. The art index loaded
//! by `BattleArt` supplies exact mapping frames; this module advances the
//! absolute runtime age and moves the child layer. An event with partial/deferred
//! composition retains the body flash as an explicit fallback.

use godot::builtin::{Vector2, Vector2i};
use godot::classes::{ImageTexture, Sprite2D};
use godot::prelude::*;

use psiv_core::battle::FighterId;
use psiv_runtime::{AttackView, BattleAnimationEvent};

use super::enemy_overlay::EnemyAnimation;

/// One enemy body and its already-decoded idle overlay animation.
pub(super) struct EnemySprite {
    pub(super) fighter: FighterId,
    /// The enemy record the body was built for: a Fusion changes it.
    pub(super) enemy_id: u16,
    pub(super) node: Gd<Sprite2D>,
    pub(super) animation: Option<EnemyAnimation>,
    pub(super) attack: Option<EnemyAttackState>,
}

/// Live attack layer and its event clock.
pub(super) struct EnemyAttackState {
    id: u64,
    elapsed: u16,
    frame_durations: Vec<u8>,
    frames: Vec<Gd<ImageTexture>>,
    high_frames: Vec<Gd<ImageTexture>>,
    /// The sprite layer an attack with a proven sheet draws its frames on,
    /// parented to the enemy's node. A flash-only attack has none: it only
    /// tints the body (#92: it once allocated a layer it never parented or
    /// freed, one orphan node per flash attack).
    layer: Option<Gd<Sprite2D>>,
    high_layer: Option<Gd<Sprite2D>>,
    origin_pixels: Vector2i,
    initial_offset_pixels: Vector2i,
    step_pixels: Vector2i,
    limit_offset_pixels: Option<Vector2i>,
}

impl EnemySprite {
    fn begin_attack(&mut self, id: u64, event: &BattleAnimationEvent) {
        self.clear_attack();
        let Some(_) = event.total_frames.filter(|frames| *frames > 0) else {
            return;
        };
        if event.sprite_sheet_proven
            && event.flash_timing_proven
            && let Some(spec) = self
                .animation
                .as_ref()
                .and_then(EnemyAnimation::attack_spec)
            && let Some(frame_count) = event.frame_count
            && let Some(frame_durations) = event.frame_durations.clone().or_else(|| {
                event
                    .frame_duration
                    .map(|duration| vec![duration; usize::from(frame_count)])
            })
            && frame_count > 0
            && usize::from(frame_count) == spec.frames.len()
            && spec.frames.len() == spec.high_frames.len()
            && frame_durations.len() == spec.frames.len()
            && frame_durations.iter().all(|duration| *duration > 0)
        {
            let mut layer = Sprite2D::new_alloc();
            layer.set_centered(false);
            // Low-priority sprite pixels cover the low-priority enemy plane,
            // but the later party plane (priority bit set) covers them.
            layer.set_z_index(0);
            layer.set_texture(&spec.frames[0]);
            layer.set_position(Vector2::new(
                (spec.origin_pixels.x + spec.initial_offset_pixels.x) as f32,
                (spec.origin_pixels.y + spec.initial_offset_pixels.y) as f32,
            ));
            self.node.add_child(&layer);
            let mut high_layer = Sprite2D::new_alloc();
            high_layer.set_centered(false);
            high_layer.set_z_index(1);
            high_layer.set_texture(&spec.high_frames[0]);
            high_layer.set_position(layer.get_position());
            self.node.add_child(&high_layer);
            self.attack = Some(EnemyAttackState {
                id,
                elapsed: 0,
                frame_durations,
                frames: spec.frames,
                high_frames: spec.high_frames,
                layer: Some(layer),
                high_layer: Some(high_layer),
                origin_pixels: spec.origin_pixels,
                initial_offset_pixels: spec.initial_offset_pixels,
                step_pixels: spec.step_pixels,
                limit_offset_pixels: spec.limit_offset_pixels,
            });
            return;
        }
        if event.flash_timing_proven {
            self.node
                .set_self_modulate(Color::from_rgba8(255, 224, 224, 255));
            self.attack = Some(EnemyAttackState {
                id,
                elapsed: 0,
                frame_durations: vec![1],
                frames: Vec::new(),
                high_frames: Vec::new(),
                layer: None,
                high_layer: None,
                origin_pixels: Vector2i::ZERO,
                initial_offset_pixels: Vector2i::ZERO,
                step_pixels: Vector2i::ZERO,
                limit_offset_pixels: None,
            });
        }
    }

    /// Draws the absolute runtime age; repeated views cannot advance or restart
    /// a live layer, and a missing view removes it on the runtime's expiry frame.
    pub(super) fn set_attack(&mut self, view: Option<&AttackView>) {
        let Some(view) = view else {
            self.clear_attack();
            return;
        };
        if self
            .attack
            .as_ref()
            .is_none_or(|attack| attack.id != view.id)
        {
            self.begin_attack(view.id, &view.event);
        }
        let Some(attack) = self.attack.as_mut() else {
            return;
        };
        attack.elapsed = view.elapsed;
        if let Some(layer) = attack.layer.as_mut() {
            let frame = frame_index(view.elapsed, &attack.frame_durations);
            layer.set_texture(&attack.frames[frame]);
            let offset = motion_offset(
                view.elapsed,
                attack.initial_offset_pixels,
                attack.step_pixels,
                attack.limit_offset_pixels,
            );
            layer.set_position(Vector2::new(
                (attack.origin_pixels.x + offset.x) as f32,
                (attack.origin_pixels.y + offset.y) as f32,
            ));
            if let Some(high_layer) = attack.high_layer.as_mut() {
                high_layer.set_texture(&attack.high_frames[frame]);
                high_layer.set_position(layer.get_position());
            }
        }
    }

    /// The attack this body is drawing now, for the native drivers
    /// (`BattleScreen::debug_menu`): `attack` is `null` between attacks,
    /// `"sheet"` while a sheet layer draws and `"flash"` while the body is only
    /// tinted; `tint` is the body's `self_modulate` as 8-bit RGBA and
    /// `children` its child count (a sheet uses two priority children; the flash
    /// branch adds none). `tools/verify_native_flash_attack.py` reads it.
    pub(super) fn debug_attack(&self) -> serde_json::Value {
        let attack = self.attack.as_ref().map(|attack| {
            if attack.layer.is_some() {
                "sheet"
            } else {
                "flash"
            }
        });
        let tint = self.node.get_self_modulate();
        serde_json::json!({
            "fighter": self.fighter.get(),
            "enemy_id": self.enemy_id,
            "attack": attack,
            "attack_elapsed": self.attack.as_ref().map(|attack| attack.elapsed),
            "tint": [tint.r8(), tint.g8(), tint.b8(), tint.a8()],
            "children": self.node.get_child_count(),
        })
    }

    pub(super) fn clear_attack(&mut self) {
        if let Some(attack) = self.attack.take() {
            for mut layer in [attack.layer, attack.high_layer].into_iter().flatten() {
                layer.queue_free();
            }
        }
        self.node
            .set_self_modulate(Color::from_rgba8(255, 255, 255, 255));
    }
}

fn frame_index(elapsed: u16, durations: &[u8]) -> usize {
    if durations.len() <= 1 {
        return 0;
    }
    let mut start = 0usize;
    for (index, duration) in durations.iter().enumerate() {
        let end = start + usize::from((*duration).max(1));
        if usize::from(elapsed) < end {
            return index;
        }
        start = end;
    }
    durations.len() - 1
}

fn motion_offset(
    elapsed: u16,
    initial: Vector2i,
    step: Vector2i,
    limit: Option<Vector2i>,
) -> Vector2i {
    let elapsed = i32::from(elapsed);
    let mut x = initial.x + step.x * elapsed;
    let mut y = initial.y + step.y * elapsed;
    if let Some(limit) = limit {
        if step.x > 0 {
            x = x.min(limit.x);
        } else if step.x < 0 {
            x = x.max(limit.x);
        }
        if step.y > 0 {
            y = y.min(limit.y);
        } else if step.y < 0 {
            y = y.max(limit.y);
        }
    }
    Vector2i::new(x, y)
}

/// Converts the formation position byte and the art record's half-width into
/// a top-left pixel. The position byte is the body's centre anchor; the
/// rendered body is `2 * half_width_cells` wide.
pub(super) fn enemy_sprite_origin(
    position: u8,
    half_width_cells: u16,
    height_cells: u16,
) -> (i32, i32) {
    let column = i32::from(position & 0x7F);
    (
        (column - i32::from(half_width_cells)) * 8,
        (15 - i32::from(height_cells)) * 8,
    )
}

#[cfg(test)]
mod tests {
    use super::{frame_index, motion_offset};
    use godot::builtin::Vector2i;

    #[test]
    fn zoran_frame_clock_is_deterministic() {
        let frames: Vec<_> = (0..8)
            .map(|elapsed| frame_index(elapsed, &[2; 8]))
            .collect();
        assert_eq!(frames, vec![0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn variable_retail_durations_select_cumulative_frames() {
        let durations = [4, 3, 4];
        let frames: Vec<_> = (0..11)
            .map(|elapsed| frame_index(elapsed, &durations))
            .collect();
        assert_eq!(frames, vec![0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn twin_arms_motion_clamps_at_the_retail_terminal_y() {
        let positions: Vec<_> = (0..8)
            .map(|elapsed| {
                motion_offset(
                    elapsed,
                    Vector2i::new(0, -184),
                    Vector2i::new(0, 64),
                    Some(Vector2i::new(0, 8)),
                )
                .y
            })
            .collect();
        assert_eq!(positions, vec![-184, -120, -56, 8, 8, 8, 8, 8]);
    }
}
