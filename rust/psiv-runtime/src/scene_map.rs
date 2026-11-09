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
    #[must_use]
    pub fn map_chunk_at(&self, at: PixelPos) -> Option<u16> {
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
        match self.change_map_aligned(
            warp.target_map,
            warp.target_cell,
            warp.facing,
            warp.alignment,
        ) {
            Ok(loaded) => {
                let cast = self.build_cast();
                if let Some(runner) = self.scene.as_mut() {
                    runner.recast(cast);
                }
                self.scene_input = SceneInput::MapLoaded;
                events.push(loaded.event(WarpTrigger::NormalGround));
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

impl Runtime {
    /// A live layout write replaces only its named chunks. Validate the whole
    /// batch before changing collision or pixels, and retain the object cast.
    pub(super) fn write_scene_map_chunks(
        &mut self,
        chunks: &[(u32, u32, u16)],
    ) -> Result<(), crate::BridgeError> {
        let reject = |message: String| crate::BridgeError::Rejected(message);
        // The pack field alone, so the controllers stay borrowable below.
        let record = self
            .data
            .map(psiv_data::MapId(self.map.id().0))
            .ok_or(crate::BridgeError::NotPacked(self.map.id().0))?;
        let atlas = record
            .patch_tiles
            .as_ref()
            .ok_or_else(|| reject("scene chunk atlas is absent".into()))?;
        let mut effects = self.effects.clone();
        for &(x, y, id) in chunks {
            if x >= record.dimensions.width_chunks || y >= record.dimensions.height_chunks {
                return Err(reject(format!("scene chunk ({x},{y}) is outside the map")));
            }
            let tile = atlas
                .tiles
                .iter()
                .find(|tile| tile.chunk_id == Some(id))
                .or_else(|| {
                    // The overworlds compose some chunks they never carry as a
                    // raw atlas tile: a page hook writes them, and the pack
                    // keeps the resolved form as an `overworld_patches` entry
                    // whose `collision_chunk_id` is that raw id. A scene write
                    // of the same id at the same coordinate is that chunk —
                    // `Event_MachineCenterAppearing`'s `$D3` at (57,90) is the
                    // twin of the hook gated on Machine Center `$43`.
                    let composed = record
                        .overworld_patches
                        .as_ref()?
                        .iter()
                        .flat_map(|patch| &patch.tiles)
                        .find(|entry| {
                            entry.chunk_x == x
                                && entry.chunk_y == y
                                && entry.collision_chunk_id == id
                        })?;
                    atlas
                        .tiles
                        .iter()
                        .find(|tile| tile.index == composed.patch_tile)
                })
                .ok_or_else(|| reject(format!("scene chunk {id:#04x} has no atlas tile")))?;
            let cells = tile.collision.ok_or_else(|| {
                reject(format!("scene chunk {id:#04x} has no collision definition"))
            })?;
            if cells.iter().any(|cell| *cell > 15) {
                return Err(reject(format!(
                    "scene chunk {id:#04x} has invalid collision"
                )));
            }
            effects
                .cell_patches
                .retain(|&(cx, cy, _)| (cx / 2, cy / 2) != (x, y));
            effects
                .chunk_patches
                .retain(|&(cx, cy, _)| (cx, cy) != (x, y));
            effects
                .patch_blits
                .retain(|&(cx, cy, _)| (cx, cy) != (x, y));
            for (index, collision) in cells.into_iter().enumerate() {
                effects.cell_patches.push((
                    x * 2 + index as u32 % 2,
                    y * 2 + index as u32 / 2,
                    collision,
                ));
            }
            effects.chunk_patches.push((x, y, id));
            effects.patch_blits.push((x, y, tile.index));
        }
        let mut map =
            crate::bridge::field_map_retaining_objects(record, Some(&effects), self.map.npcs())?;
        crate::bridge::attach_chests(&mut map, record, &self.game, self.map.npcs(), &effects)?;
        crate::bridge::reconcile_controllers(
            self.map.npcs(),
            &map,
            record,
            &mut self.wander,
            &mut self.bespoke,
        );
        self.map = map;
        self.effects = effects;
        Ok(())
    }

    /// `SceneOp::ReplaceMapLayout`: install the pack's layout variant decoded
    /// from `fg` and `bg` on the live map, as the map's own load would with
    /// the variant's flag set, keeping the objects, the party and the camera.
    /// Scene chunk writes made over the old layout go with it: the cartridge's
    /// `KosDecomp` overwrites the whole layout buffer.
    pub(super) fn replace_scene_map_layout(
        &mut self,
        fg: u32,
        bg: u32,
    ) -> Result<(), crate::BridgeError> {
        let reject = |message: String| crate::BridgeError::Rejected(message);
        // The pack field alone, so the controllers stay borrowable below.
        let record = self
            .data
            .map(psiv_data::MapId(self.map.id().0))
            .ok_or(crate::BridgeError::NotPacked(self.map.id().0))?;
        let mut variant = None;
        for source in [fg, bg] {
            match crate::effects::layout_replace_variant(record, source) {
                Ok(Some(index)) => variant = Some(index),
                Ok(None) => {}
                Err(()) => {
                    return Err(reject(format!(
                        "layout ${source:06X} is no layout variant of this map"
                    )));
                }
            }
        }
        let Some(index) = variant else {
            return Err(reject(format!(
                "layouts ${fg:06X}/${bg:06X} change nothing on this map"
            )));
        };
        let mut effects = self.effects.clone();
        effects.variant = Some(index);
        effects.cell_patches.clear();
        effects.chunk_patches.clear();
        effects.patch_blits.clear();
        let mut map =
            crate::bridge::field_map_retaining_objects(record, Some(&effects), self.map.npcs())?;
        crate::bridge::attach_chests(&mut map, record, &self.game, self.map.npcs(), &effects)?;
        crate::bridge::reconcile_controllers(
            self.map.npcs(),
            &map,
            record,
            &mut self.wander,
            &mut self.bespoke,
        );
        self.map = map;
        self.effects = effects;
        Ok(())
    }

    /// `loc_6DBAC` is an explicit live layout write, independent of the
    /// map-load flag walk. Reveal only the named base chunks; keep NPCs,
    /// unrelated overlays, party movement and camera state intact.
    pub(super) fn restore_scene_map_chunks(
        &mut self,
        chunks: &[(u32, u32, u16)],
    ) -> Result<(), crate::BridgeError> {
        // The pack field alone, so the controllers stay borrowable below.
        let record = self
            .data
            .map(psiv_data::MapId(self.map.id().0))
            .ok_or(crate::BridgeError::NotPacked(self.map.id().0))?;
        let base = record.vehicle_battle.as_ref().ok_or_else(|| {
            crate::BridgeError::Rejected("scene needs the base chunk grid".into())
        })?;
        if self.effects.variant.is_some()
            || chunks.iter().any(|&(x, y, id)| {
                base.rows
                    .get(y as usize)
                    .and_then(|row| row.get(x as usize))
                    != Some(&id)
            })
        {
            return Err(crate::BridgeError::Rejected(
                "scene base chunks disagree with the pack".into(),
            ));
        }
        let contains = |x, y| chunks.iter().any(|&(cx, cy, _)| cx == x && cy == y);
        let mut effects = self.effects.clone();
        effects
            .cell_patches
            .retain(|&(x, y, _)| !contains(x / 2, y / 2));
        effects.chunk_patches.retain(|&(x, y, _)| !contains(x, y));
        effects.patch_blits.retain(|&(x, y, _)| !contains(x, y));
        let mut map =
            crate::bridge::field_map_retaining_objects(record, Some(&effects), self.map.npcs())?;
        crate::bridge::attach_chests(&mut map, record, &self.game, self.map.npcs(), &effects)?;
        crate::bridge::reconcile_controllers(
            self.map.npcs(),
            &map,
            record,
            &mut self.wander,
            &mut self.bespoke,
        );
        self.map = map;
        self.effects = effects;
        Ok(())
    }
}
