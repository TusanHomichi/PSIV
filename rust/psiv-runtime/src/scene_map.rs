//! Original layout-byte conditions and event-owned map transitions.
use crate::{Runtime, RuntimeEvent};
use psiv_core::{Cell, PixelPos, SceneFault, SceneInput, WarpTrigger};

/// The chunk id under a pixel position on the collision-selected plane, patches
/// included: what `GetChunkAndCollision` (`$45A52`) and `GetMapLayoutOffset`
/// read. A free function over the record and the live effects so the scene
/// runner can be handed a probe while the runtime's other fields are borrowed.
pub(crate) fn live_chunk(
    record: &psiv_data::MapRecord,
    effects: &crate::EffectOutcome,
    at: PixelPos,
) -> Option<u16> {
    let layout = effects
        .variant
        .and_then(|index| record.layout_variants.get(index))
        .and_then(|variant| variant.vehicle_battle.as_ref())
        .or(record.vehicle_battle.as_ref())?;
    let x = u32::try_from(at.x).ok()? / 32;
    let y = u32::try_from(at.y).ok()? / 32;
    let base = layout.rows.get(y as usize)?.get(x as usize)?;
    Some(
        effects
            .chunk_patches
            .iter()
            .rev()
            .find_map(|&(cx, cy, id)| ((cx, cy) == (x, y)).then_some(id))
            .unwrap_or(*base),
    )
}

impl Runtime {
    /// Read the live collision-selected plane, including scene/load patches.
    pub(crate) fn map_chunk_at(&self, at: PixelPos) -> Option<u16> {
        live_chunk(self.map_record()?, &self.effects, at)
    }

    /// Whether `RunEvent_RidingElevator` would fire for a leader at `cell`.
    pub(crate) fn elevator_at(&self, cell: Cell) -> bool {
        let at = PixelPos::from_cell(cell);
        self.map_record()
            .is_some_and(|record| record.events.contains(&0x0D))
            && self.map_chunk_at(PixelPos {
                x: at.x,
                y: at.y + 16,
            }) == Some(0x53)
    }

    pub(crate) fn take_scene_map_transition(&mut self, events: &mut Vec<RuntimeEvent>) {
        let warp = self
            .map
            .warp_at(self.party.leader().cell(), WarpTrigger::NormalGround)
            .copied();
        let Some(warp) = warp else {
            self.scene = None;
            events.push(RuntimeEvent::SceneFaulted {
                fault: SceneFault::BadWrite,
            });
            return;
        };
        match self.change_map(warp.target_map, warp.target_cell, warp.facing) {
            Ok(()) => {
                let cast = self.build_cast();
                if let Some(runner) = self.scene.as_mut() {
                    runner.recast(cast);
                }
                self.scene_input = SceneInput::MapLoaded;
                events.push(RuntimeEvent::MapChanged {
                    map: warp.target_map,
                    trigger: WarpTrigger::NormalGround,
                });
            }
            Err(error) => {
                self.scene = None;
                events.push(RuntimeEvent::MapRefreshFailed {
                    error: error.to_string(),
                });
                events.push(RuntimeEvent::SceneFaulted {
                    fault: SceneFault::BadWrite,
                });
            }
        }
    }
}
