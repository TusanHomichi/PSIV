//! `Event_GettingOffVehicle`'s two needs ([`GetOffOp`]): the standing test
//! and the map's music. The scene is `scenes/vehicles.rs`.

use super::SceneRunner;
use crate::scene::{SceneEffect, SceneFault, SceneOp};
use crate::scene_types::GetOffOp;

impl SceneRunner {
    pub(super) fn get_off_op(
        &mut self,
        op: GetOffOp,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        match op {
            GetOffOp::BranchIfBlocked {
                if_blocked,
                if_clear,
            } => {
                let target = usize::from(if self.dismount_clear {
                    if_clear
                } else {
                    if_blocked
                });
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            GetOffOp::PlayMapMusic => {
                effects.push(SceneEffect::Presentation {
                    op: SceneOp::GetOff(op),
                });
                self.pc += 1;
            }
        }
        None
    }
}
