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
    pub fn set_camera(&mut self, x: i32, y: i32) {
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
    pub fn scene_move_camera(&mut self, x: i32, y: i32, speed: i32) {
        let target_x = x - psiv_core::HOME_X;
        let target_y = y - psiv_core::HOME_Y;
        if speed <= 0 {
            self.set_camera(target_x, target_y);
            return;
        }
        self.camera_glide = Some(CameraGlide {
            target_x,
            target_y,
            speed,
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
        let (next_x, next_y) = (step(x, glide.target_x), step(y, glide.target_y));
        self.camera.set_position(next_x, next_y);
        if next_x == glide.target_x && next_y == glide.target_y {
            self.camera_glide = None;
        }
    }

    /// Applies the packed `loc_51AB2` gate write to the current camera without
    /// repositioning the view. This is the runtime seam for `RefreshMap` calls
    /// made after a scene or warp has already entered the map.
    pub fn refresh_map_camera_gates(&mut self) -> Result<(), BridgeError> {
        let record = self
            .map_record()
            .cloned()
            .ok_or(BridgeError::NotPacked(self.map.id().0))?;
        refresh_camera_gates(&mut self.camera, &record).map_err(BridgeError::Rejected)
    }
}
