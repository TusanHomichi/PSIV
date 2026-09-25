//! Map transitions: a warp's map load — effects, objects, party and camera —
//! and the battle-return refresh that keeps live objects.

use psiv_core::{Cell, Direction, MapId};

use crate::bridge::{self, build_bespoke, build_wander, clear_bespoke_entry_flags};
use crate::effects;
use crate::geometry::{camera_for_record, driver_of};
use crate::vehicle;
use crate::{BridgeError, Runtime, RuntimeEvent, field_map_patched};

impl Runtime {
    /// Applies the cartridge's battle-return map load before revealing the
    /// field. Presentation calls this after results; headless callers receive
    /// the same refresh automatically on their next field tick.
    pub fn return_to_field(&mut self) -> Vec<RuntimeEvent> {
        if self.battle.is_some() || !std::mem::take(&mut self.battle_field_refresh_pending) {
            return Vec::new();
        }
        match self.refresh_field_after_battle() {
            Ok(()) => vec![RuntimeEvent::MapRefreshed],
            Err(error) => vec![RuntimeEvent::MapRefreshFailed {
                error: error.to_string(),
            }],
        }
    }

    pub(crate) fn refresh_field_after_battle(&mut self) -> Result<(), BridgeError> {
        let target = self.map.id();
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        // GameMode_LoadFieldMap with Map_Load_Flags bit 0: the map-data
        // walk runs, while object initialization, party placement and camera
        // initialization do not. Keep the wander clocks and scene cast too.
        let effects = effects::evaluate(record, &mut self.game);
        let mut map = bridge::field_map_retaining_objects(record, Some(&effects), self.map.npcs())?;
        bridge::attach_chests(&mut map, record, &self.game, self.map.npcs(), &effects)?;
        self.map = map;
        self.effects = effects;
        self.scene_triggers_pending = true;
        self.field_status.clock.reset();
        Ok(())
    }

    pub(crate) fn change_map(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
    ) -> Result<(), BridgeError> {
        self.change_map_from(target, cell, facing, self.map.id().0)
    }

    pub(crate) fn change_map_from(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
        previous_map: u16,
    ) -> Result<(), BridgeError> {
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        // MapDataManager runs inside map load, and its walk is STATEFUL:
        // flag_clear writes (from the pack's decoded data) land mid-walk so
        // later entries' gates see them — the Xanafalgue-respawn mechanism
        // tape 18 measured. Cleared flags resurrect gated objects on OTHER
        // maps at their next build; this map's own build below already sees
        // the post-clear state.
        let effects = effects::evaluate(record, &mut self.game);
        let mut map = field_map_patched(record, Some(&effects))?;
        bridge::attach_chests(&mut map, record, &self.game, &[], &effects)?;
        clear_bespoke_entry_flags(&mut self.game, record);
        self.effects = effects;
        // LoadMapObjects creates a fresh cast. Scene despawns only change
        // the live objects; persistent removals come from MapDataManager's
        // extracted flag gates above (including recruited party members).
        self.party
            .enter_map(&map, cell, facing)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.wander = build_wander(&map, record)?;
        self.bespoke = build_bespoke(&map, record)?;
        // Map entry places the view rather than scrolling it in, so the camera
        // starts framed on the party wherever the warp dropped them.
        self.camera = camera_for_record(driver_of(self.party.leader()), &map, record)
            .map_err(BridgeError::Rejected)?;
        self.map = map;
        if let Some(vehicle) = self.vehicle.as_mut() {
            if !vehicle.enter_map(&self.map, cell, facing) {
                return Err(BridgeError::Rejected(format!(
                    "vehicle destination ({}, {}) is outside map {}",
                    cell.x, cell.y, target.0
                )));
            }
            self.camera = camera_for_record(vehicle::driver_of(vehicle), &self.map, record)
                .map_err(BridgeError::Rejected)?;
        }
        // The first field control tick checks RunEvents before accepting a
        // step. loc_518D2 also resets the encounter countdown on EVERY load.
        self.prev_standing = None;
        self.scene_triggers_pending = true;
        self.field_status.clock.reset();
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
        self.saved_map_index_2 = previous_map;
        self.apply_travel_entry();
        Ok(())
    }
}
