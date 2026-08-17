//! Retail enemy attack frame playback and motion.
//!
//! `BattleAnimationEvent` remains the dispatch surface. The art index loaded
//! by `BattleArt` supplies exact mapping frames; this module advances the
//! decoded clock and moves the child layer. An event with partial/deferred
//! composition retains the body flash as an explicit fallback.

use godot::builtin::{Vector2, Vector2i};
use godot::classes::{ImageTexture, Sprite2D};
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

/// Live attack layer and its event clock.
pub(super) struct EnemyAttackState {
    remaining: u16,
    elapsed: u16,
    frame_duration: u8,
    frames: Vec<Gd<ImageTexture>>,
    layer: Gd<Sprite2D>,
    origin_pixels: Vector2i,
    initial_offset_pixels: Vector2i,
    step_pixels: Vector2i,
    limit_offset_pixels: Option<Vector2i>,
}

impl EnemySprite {
    pub(super) fn begin_attack(&mut self, event: &BattleAnimationEvent) {
        self.clear_attack();
        let Some(total_frames) = event.total_frames.filter(|frames| *frames > 0) else {
            return;
        };
        if event.sprite_sheet_proven
            && event.flash_timing_proven
            && let Some(spec) = self
                .animation
                .as_ref()
                .and_then(EnemyAnimation::attack_spec)
            && let (Some(frame_count), Some(frame_duration)) =
                (event.frame_count, event.frame_duration)
            && frame_count > 0
            && frame_duration > 0
            && usize::from(frame_count) == spec.frames.len()
        {
            let mut layer = Sprite2D::new_alloc();
            layer.set_centered(false);
            layer.set_z_index(1);
            layer.set_texture(&spec.frames[0]);
            layer.set_position(Vector2::new(
                (spec.origin_pixels.x + spec.initial_offset_pixels.x) as f32,
                (spec.origin_pixels.y + spec.initial_offset_pixels.y) as f32,
            ));
            self.node.add_child(&layer);
            self.attack = Some(EnemyAttackState {
                remaining: total_frames,
                elapsed: 0,
                frame_duration,
                frames: spec.frames,
                layer,
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
                remaining: total_frames,
                elapsed: 0,
                frame_duration: 1,
                frames: Vec::new(),
                layer: Sprite2D::new_alloc(),
                origin_pixels: Vector2i::ZERO,
                initial_offset_pixels: Vector2i::ZERO,
                step_pixels: Vector2i::ZERO,
                limit_offset_pixels: None,
            });
        }
    }

    pub(super) fn advance_attack(&mut self) {
        let Some(attack) = self.attack.as_mut() else {
            return;
        };
        if attack.remaining > 1 {
            attack.remaining -= 1;
            if !attack.frames.is_empty() {
                attack.elapsed = attack.elapsed.saturating_add(1);
                let frame = frame_index(attack.elapsed, attack.frame_duration, attack.frames.len());
                attack.layer.set_texture(&attack.frames[frame]);
                let offset = motion_offset(
                    attack.elapsed,
                    attack.initial_offset_pixels,
                    attack.step_pixels,
                    attack.limit_offset_pixels,
                );
                attack.layer.set_position(Vector2::new(
                    (attack.origin_pixels.x + offset.x) as f32,
                    (attack.origin_pixels.y + offset.y) as f32,
                ));
            }
            return;
        }
        self.clear_attack();
    }

    pub(super) fn clear_attack(&mut self) {
        if let Some(mut attack) = self.attack.take()
            && !attack.frames.is_empty()
        {
            attack.layer.queue_free();
        }
        self.node
            .set_self_modulate(Color::from_rgba8(255, 255, 255, 255));
    }
}

fn frame_index(elapsed: u16, duration: u8, frame_count: usize) -> usize {
    if frame_count <= 1 {
        return 0;
    }
    (usize::from(elapsed) / usize::from(duration.max(1))).min(frame_count - 1)
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

/// Converts the formation position byte and art dimensions into a top-left
/// pixel. The position byte is the body's bottom-right column.
pub(super) fn enemy_sprite_origin(position: u8, width_cells: u16, height_cells: u16) -> (i32, i32) {
    let column = i32::from(position & 0x7F);
    (
        (column - i32::from(width_cells)) * 8,
        (15 - i32::from(height_cells)) * 8,
    )
}

#[cfg(test)]
mod tests {
    use super::{frame_index, motion_offset};
    use godot::builtin::Vector2i;

    #[test]
    fn zoran_frame_clock_is_deterministic() {
        let frames: Vec<_> = (0..8).map(|elapsed| frame_index(elapsed, 2, 8)).collect();
        assert_eq!(frames, vec![0, 0, 1, 1, 2, 2, 3, 3]);
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
