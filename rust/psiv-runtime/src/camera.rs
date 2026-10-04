//! The field camera seam: the retail viewport query, absolute parking,
//! `Event_MoveCamera` pans and the post-load gate write.

use psiv_core::Camera;

use crate::geometry::refresh_camera_gates;
use crate::{BridgeError, CameraGlide, Runtime};

impl Runtime {
    /// The camera bounds this map imposes.
    /// The leader as the camera reads it: 16.16 position and this frame's
    /// velocity.
    ///
    /// The velocity is the cartridge's `y_step_constant` — a whole cell divided
    /// by the step's frame count, which is 2 px/frame at the default eight
    /// frames per cell — and it is zero at rest, which is what stops the camera
    /// dead rather than letting it drift.
    /// The field camera, for the renderer's authentic 320x224 viewport.
    #[must_use]
    pub const fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Parks the camera at an absolute position.
    ///
    /// Scenes do this (`SceneOp::SetCameraPos`), and so does a replay whose
    /// alignment frame inherits a camera the engine could not have produced,
    /// the opening scene having placed it.
    pub(crate) fn set_camera(&mut self, x: i32, y: i32) {
        self.camera_glide = None;
        self.camera.set_position(x, y);
    }

    /// `Event_MoveCamera` (`ps4.asm:121468`): pan the camera to frame a world
    /// position. The operands are a *subject*, not a scroll — the routine
    /// subtracts the driver's home offset (`$98`/`$58`, `HOME_X`/`HOME_Y`) to
    /// get the camera target, then steps toward it at `speed` px/frame per
    /// axis. `AlysFound` ends on exactly this op to hand the view back to the
    /// new leader; parking the raw operands instead left the party in the
    /// top-left corner of the screen.
    pub(crate) fn scene_move_camera(&mut self, x: i32, y: i32, speed: i32) {
        self.scene_move_camera_ordered(x, y, speed, false);
    }

    /// Boarding uses the same glide clock but `Event_MoveCamera`'s X loop
    /// finishes before its Y loop (`US ROM $05AAEE`). Other scene callers keep
    /// their established timing until the separate camera-class audit.
    pub(crate) fn scene_move_camera_boarding(&mut self, x: i32, y: i32) {
        // The helper masks each subject-minus-home target to twelve bits.
        self.scene_move_camera_ordered(x, y, 2, true);
    }

    fn scene_move_camera_ordered(&mut self, x: i32, y: i32, speed: i32, x_then_y: bool) {
        let target_x = x - psiv_core::HOME_X;
        let target_y = y - psiv_core::HOME_Y;
        let (target_x, target_y) = if x_then_y {
            (target_x & 0xFFF, target_y & 0xFFF)
        } else {
            (target_x, target_y)
        };
        if speed <= 0 {
            self.set_camera(target_x, target_y);
            return;
        }
        self.camera_glide = Some(CameraGlide {
            target_x,
            target_y,
            speed,
            x_then_y,
        });
    }

    /// One frame of an in-flight `Event_MoveCamera` pan.
    pub(crate) fn tick_camera_glide(&mut self) {
        let Some(glide) = self.camera_glide else {
            return;
        };
        let (raw_x, raw_y) = self.camera.raw();
        let (x, y) = (raw_x >> 16, raw_y >> 16);
        let step = |from: i32, to: i32| from + (to - from).clamp(-glide.speed, glide.speed);
        let next_x = if glide.x_then_y {
            wrapped_camera_step(x, glide.target_x, glide.speed)
        } else {
            step(x, glide.target_x)
        };
        let next_y = if glide.x_then_y && x != glide.target_x {
            y
        } else if glide.x_then_y {
            wrapped_camera_step(y, glide.target_y, glide.speed)
        } else {
            step(y, glide.target_y)
        };
        self.camera.set_position(next_x, next_y);
        if next_x == glide.target_x && next_y == glide.target_y {
            self.camera_glide = None;
        }
    }

    /// Applies the packed `loc_51AB2` gate write to the current camera without
    /// repositioning the view. This is the runtime seam for `RefreshMap` calls
    /// made after a scene or warp has already entered the map
    /// (`docs/field/CAMERA.md`); the refresh paths do not call it yet (#59).
    #[allow(dead_code)]
    pub(crate) fn refresh_map_camera_gates(&mut self) -> Result<(), BridgeError> {
        let record = self
            .map_record()
            .cloned()
            .ok_or(BridgeError::NotPacked(self.map.id().0))?;
        refresh_camera_gates(&mut self.camera, &record).map_err(BridgeError::Rejected)
    }
}

/// The helper's `$FFF` target and `$7FF` shortest-path branch, one axis at a
/// time. Kept in the existing glide tick so boarding has no second camera.
fn wrapped_camera_step(from: i32, to: i32, speed: i32) -> i32 {
    let from = from & 0xFFF;
    let to = to & 0xFFF;
    let delta = (to - from + 0x800) & 0xFFF;
    let delta = delta - 0x800;
    (from + delta.clamp(-speed, speed)) & 0xFFF
}

#[cfg(test)]
mod boarding_camera_tests {
    use super::wrapped_camera_step;

    #[test]
    fn boarding_pan_uses_the_twelve_bit_short_path_at_the_seam() {
        assert_eq!(wrapped_camera_step(4094, 2, 2), 0);
        assert_eq!(wrapped_camera_step(0, 2, 2), 2);
        assert_eq!(wrapped_camera_step(2, 4094, 2), 0);
        assert_eq!(wrapped_camera_step(0, 4094, 2), 4094);
    }
}
