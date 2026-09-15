//! Original layout-byte conditions and event-owned map transitions.
use crate::{Runtime, RuntimeEvent};
use psiv_core::{
    Cell, CustomTrigger, PixelPos, SceneFault, SceneInput, TriggerResult, Unsupported, WarpTrigger,
};

impl Runtime {
    /// Read the live collision-selected plane, including scene/load patches.
    pub(crate) fn map_chunk_at(&self, at: PixelPos) -> Option<u16> {
        let record = self.map_record()?;
        let layout = self
            .effects
            .variant
            .and_then(|index| record.layout_variants.get(index))
            .and_then(|variant| variant.vehicle_battle.as_ref())
            .or(record.vehicle_battle.as_ref())?;
        let x = u32::try_from(at.x).ok()? / 32;
        let y = u32::try_from(at.y).ok()? / 32;
        let base = layout.rows.get(y as usize)?.get(x as usize)?;
        Some(
            self.effects
                .chunk_patches
                .iter()
                .rev()
                .find_map(|&(cx, cy, id)| ((cx, cy) == (x, y)).then_some(id))
                .unwrap_or(*base),
        )
    }

    pub(crate) fn elevator_trigger(&self, cell: Cell) -> TriggerResult {
        let mut at = PixelPos::from_cell(cell);
        at.y += 16;
        match self.map_chunk_at(at) {
            Some(0x53) => TriggerResult::Fire(psiv_core::EventIndex(0x14)),
            Some(_) => TriggerResult::NoEvent,
            None => TriggerResult::Unsupported(
                CustomTrigger::RidingElevator,
                Unsupported::MapLayoutBytes,
            ),
        }
    }

    pub(crate) fn elevator_at(&self, cell: Cell) -> bool {
        self.map_record()
            .is_some_and(|record| record.events.contains(&0x0D))
            && matches!(self.elevator_trigger(cell), TriggerResult::Fire(_))
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
