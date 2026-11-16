//! Rocky's inventory branch and running pace (`Event_CatchingRocky`).

use super::SceneRunner;
use crate::field::StepFrames;
use crate::scene::{SceneEffect, SceneFault};
use crate::scene_types::RockyOp;
use crate::state::GameState;

/// `loc_4BC80`'s `FieldObj_Step_Offset` 2: `$0400` a frame.
const ROCKY_STEP_FRAMES: u8 = 4;

impl SceneRunner {
    pub(super) fn rocky_op(
        &mut self,
        rocky: RockyOp,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        match rocky {
            RockyOp::BranchIfHolding {
                item,
                if_held,
                if_not,
            } => {
                let held = state.inventory().get_item(item);
                let target = usize::from(if held { if_held } else { if_not });
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            RockyOp::RemoveAll { item } => {
                while let Some(slot) = state.inventory().find_item(item) {
                    let _ = state.inventory_mut().remove_in_field(slot);
                }
                effects.push(SceneEffect::InventoryChanged);
                self.pc += 1;
            }
            RockyOp::RunFast { actor } => {
                let Some(runner) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                runner.pace = StepFrames::new(ROCKY_STEP_FRAMES).ok();
                self.pc += 1;
            }
        }
        None
    }
}
