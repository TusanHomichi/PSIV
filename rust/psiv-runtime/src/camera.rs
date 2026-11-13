//! The field camera seam: the retail viewport query, absolute parking,
//! `Event_MoveCamera` pans and the post-load gate write.

use psiv_core::Camera;

use crate::geometry::refresh_camera_gates;
use crate::{BridgeError, CameraGlide, Runtime};
use psiv_core::MapId;

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

    /// Boarding's `Event_MoveCamera` call at speed 2 (`US ROM $05AAEE`).
    pub(crate) fn scene_move_camera_boarding(&mut self, x: i32, y: i32) {
        self.scene_move_camera(x, y, 2);
    }

    /// `SceneOp::PanCamera`: the routine as it stands, X then Y on the
    /// twelve-bit short path. The runner waits for the arrival; with no glide
    /// left, `Runtime::scene_tick` releases it within the frame.
    pub(crate) fn scene_pan_camera(&mut self, x: i32, y: i32, speed: i32) {
        self.scene_move_camera(x, y, speed);
        // Both of the routine's loops exit before their first frame.
        let (raw_x, raw_y) = self.camera.raw();
        if self.camera_glide.is_some_and(|glide| {
            (raw_x >> 16) & 0xFFF == glide.target_x && (raw_y >> 16) & 0xFFF == glide.target_y
        }) {
            self.camera_glide = None;
        }
    }

    /// `Event_MoveCamera` (`ps4.asm:121468`): pan the camera to frame a world
    /// position. The operands are a *subject*, not a scroll — the routine
    /// subtracts the driver's home offset (`$98`/`$58`, `HOME_X`/`HOME_Y`) and
    /// masks each target to twelve bits, then steps toward it at `speed`
    /// px/frame, the X loop before the Y loop. `AlysFound` ends on exactly
    /// this op to hand the view back to the new leader; parking the raw
    /// operands instead left the party in the top-left corner of the screen.
    fn scene_move_camera(&mut self, x: i32, y: i32, speed: i32) {
        let target_x = (x - psiv_core::HOME_X) & 0xFFF;
        let target_y = (y - psiv_core::HOME_Y) & 0xFFF;
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
        // The frame after the last movement releases the pan: the final
        // `VInt_Prepare` (`ps4.asm:121499-121549`) has shown the arrival.
        if x & 0xFFF == glide.target_x && y & 0xFFF == glide.target_y {
            self.camera_glide = None;
            return;
        }
        let next_x = wrapped_camera_step(x, glide.target_x, glide.speed);
        // The Y loop starts only once the X loop has exited.
        let next_y = if x & 0xFFF == glide.target_x {
            wrapped_camera_step(y, glide.target_y, glide.speed)
        } else {
            y
        };
        self.camera.set_position(next_x, next_y);
    }

    /// Applies `target`'s packed `loc_51AB2` gate write to the current camera
    /// without repositioning the view: the part of a load that keeps the
    /// camera (`docs/field/CAMERA.md`). The battle return calls it; a map
    /// change and the cutscene reload build the camera from the record, which
    /// applies the same write.
    pub(crate) fn refresh_map_camera_gates_for(
        &mut self,
        target: MapId,
    ) -> Result<(), BridgeError> {
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        refresh_camera_gates(&mut self.camera, record).map_err(BridgeError::Rejected)
    }
}

/// The helper's `$FFF` target and `$7FF` shortest-path branch, one axis at a
/// time. Every pan steps through it.
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

#[cfg(test)]
mod pan_release_tests {
    use std::path::Path;

    use psiv_core::{Cell, Direction, StepFrames};
    use psiv_data::GameData;

    use crate::Runtime;

    const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

    fn runtime() -> Option<Runtime> {
        if !Path::new(PACK).join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return None;
        }
        let data = GameData::load(Path::new(PACK)).expect("pack loads");
        Some(
            Runtime::new(
                data,
                0x013,
                Cell::new(38, 18),
                Direction::Up,
                StepFrames::default(),
            )
            .expect("spawn"),
        )
    }

    /// Ticks until the glide releases; returns the camera's pixel position.
    fn run(rt: &mut Runtime) -> (i32, i32) {
        for _ in 0..4000 {
            if rt.camera_glide.is_none() {
                let (x, y) = rt.camera().raw();
                return (x >> 16, y >> 16);
            }
            rt.tick_camera_glide();
        }
        panic!("pan never released");
    }

    #[test]
    fn consecutive_two_axis_pans_land_on_their_targets() {
        let Some(mut rt) = runtime() else { return };
        let (cx, cy) = rt.camera().raw();
        let (cx, cy) = (cx >> 16, cy >> 16);
        // Both axes, then a consecutive pan back.
        for (x, y) in [(cx + 6, cy + 4), (cx, cy)] {
            rt.scene_pan_camera(x + psiv_core::HOME_X, y + psiv_core::HOME_Y, 2);
            assert!(rt.camera_glide.is_some());
            assert_eq!(run(&mut rt), (x & 0xFFF, y & 0xFFF));
        }
    }

    #[test]
    fn final_move_frame_still_owns_the_camera() {
        let Some(mut rt) = runtime() else { return };
        let (cx, cy) = rt.camera().raw();
        rt.scene_pan_camera(
            (cx >> 16) + psiv_core::HOME_X + 4,
            (cy >> 16) + psiv_core::HOME_Y,
            2,
        );
        rt.tick_camera_glide();
        assert!(rt.camera_glide.is_some(), "first of two steps");
        rt.tick_camera_glide();
        assert!(rt.camera_glide.is_some(), "arrival frame keeps the glide");
        rt.tick_camera_glide();
        assert!(rt.camera_glide.is_none(), "released the frame after");
    }

    #[test]
    fn zero_distance_pan_releases_without_a_frame() {
        let Some(mut rt) = runtime() else { return };
        let (cx, cy) = rt.camera().raw();
        rt.scene_pan_camera(
            (cx >> 16) + psiv_core::HOME_X,
            (cy >> 16) + psiv_core::HOME_Y,
            2,
        );
        assert!(rt.camera_glide.is_none());
    }
}
