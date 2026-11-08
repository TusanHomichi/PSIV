//! Runtime bridge for the mounted field state and vehicle battle handoff.
//!
//! The core owns the rules. This module owns the mutable shell seams: the
//! `$F43C` selector, the current map/effects, encounter RNG, map transitions,
//! and the saved record copied back after a battle.

use psiv_core::battle::{Rng2, Rolls};
use psiv_core::{Cell, Driver, Input, ONE_PIXEL, PixelPos, VehicleEffect, VehicleState};

use crate::encounters::{EncounterClock, VEHICLE_MASK};
use psiv_core::EventIndex;

/// `EventFlag_DarkForce2`, which `loc_6C24` tests to swap background 4 for 5.
const DARK_FORCE_2_FLAG: u16 = 0x9E;

/// `Event_GettingOffVehicle`, the event `loc_56084` writes.
const GETTING_OFF_VEHICLE: u16 = 0x10;
use crate::events::RuntimeEvent;
use crate::{BridgeError, Runtime};

impl Runtime {
    /// Whether `Event_GettingOffVehicle` is the scene running. It plays the
    /// map's music itself (or leaves it alone), so the shell asks no other.
    #[must_use]
    pub fn getting_off_scene_active(&self) -> bool {
        self.scene_event() == Some(EventIndex(GETTING_OFF_VEHICLE))
    }

    /// Synchronises the in-memory mounted state with the persisted selector.
    /// Used at construction and after loading a save.
    pub(crate) fn sync_vehicle_selector(&mut self) -> Result<(), BridgeError> {
        let index = self.game.vehicle_index();
        if index == 0 {
            self.vehicle = None;
            return Ok(());
        }
        self.set_vehicle_index(index)
    }

    /// Sets `Vehicle_Index` (`$F43C`): zero dismounts, 1/2/3 mount the saved
    /// Land Rover/Ice Digger/Hydrofoil record at the party's current anchor.
    pub(crate) fn set_vehicle_index(&mut self, index: u16) -> Result<(), BridgeError> {
        if index > psiv_core::VEHICLE_INDEX_MAX {
            return Err(BridgeError::Rejected(format!(
                "vehicle selector {index} is outside 0..={} ",
                psiv_core::VEHICLE_INDEX_MAX
            )));
        }
        if index == 0 {
            self.game.set_vehicle_index(0);
            // Both of the cartridge's dismounts rebuild every character
            // object on `Character_1`, the machine's body while mounted, with
            // its position and facing (`Event_GettingOffVehicle`, `loc_6BF4E`,
            // `ps4.asm:145362-145380`; `Event_EclipseTorchUsed`'s mounted
            // branch, `loc_701DE`, `ps4.asm:149559-149577`). The party objects
            // here stayed where the party boarded, so they are stacked on the
            // machine's cell before it goes.
            if let Some(vehicle) = self.vehicle.take() {
                self.stack_party_at(vehicle.cell(), vehicle.facing());
                if let Some(runner) = self.scene.as_mut() {
                    runner.park_party(vehicle.cell(), vehicle.facing());
                }
            }
            self.boarding_body = None;
            return Ok(());
        }
        if self
            .vehicle
            .as_ref()
            .is_some_and(|vehicle| vehicle.index() == index)
        {
            self.game.set_vehicle_index(index);
            self.boarding_body = None;
            return Ok(());
        }
        let leader = self.party.leader();
        let vehicle = VehicleState::new(&self.map, index, leader.cell(), leader.facing())
            .ok_or_else(|| BridgeError::Rejected(format!("cannot mount vehicle {index}")))?;
        self.game.set_vehicle_index(index);
        self.vehicle = Some(vehicle);
        self.boarding_body = None;
        Ok(())
    }

    /// The active vehicle selector, or `None` while walking.
    #[must_use]
    pub fn vehicle_index(&self) -> Option<u16> {
        self.vehicle.as_ref().map(|vehicle| vehicle.index())
    }

    /// Whether field input currently drives a vehicle.
    #[must_use]
    pub fn vehicle_active(&self) -> bool {
        self.vehicle.is_some()
    }

    /// The mounted field state for presentation and diagnostics.
    #[must_use]
    pub fn vehicle_state(&self) -> Option<VehicleState> {
        self.vehicle
    }

    /// The field sprite body. During a boarding pan the cartridge has already
    /// replaced `Character_1` but has not written persistent `Vehicle_Index`.
    #[must_use]
    pub fn vehicle_draw_state(&self) -> Option<VehicleState> {
        self.boarding_body.or(self.vehicle)
    }

    /// The vehicle's standing cell, used by save placement and renderers.
    #[must_use]
    pub fn vehicle_cell(&self) -> Option<Cell> {
        self.vehicle.map(VehicleState::cell)
    }

    /// The saved vehicle record currently backing the battle surface.
    #[must_use]
    pub fn vehicle_record(&self) -> Option<psiv_core::VehicleRecord> {
        let index = self.vehicle_index()?;
        self.game
            .vehicles()
            .get(index.saturating_sub(1) as usize)
            .copied()
    }

    /// Raw chunk id used by retail's Motavia battle-background selector.
    /// Returns `None` when the pack has no raw collision-plane chunk grid.
    #[must_use]
    pub fn vehicle_battle_terrain(&self) -> Option<u8> {
        let record = self.data.map(psiv_data::MapId(self.map.id().0))?;
        // A map effect can replace the render/collision layout without
        // replacing the raw chunk identity that mounted battle selection
        // reads.  Older packs also legitimately have a variant with no
        // vehicle-battle plane.  Keep the variant when it supplies one, and
        // otherwise retain the map's base chunk grid instead of turning a
        // valid mounted battle into the generic background-0 fallback.
        let layout = self
            .effects
            .variant
            .and_then(|index| record.layout_variants.get(index))
            .and_then(|variant| variant.vehicle_battle.as_ref())
            .or(record.vehicle_battle.as_ref())?;
        if layout.width_chunks == 0 || layout.height_chunks == 0 {
            return None;
        }
        let cell = self
            .vehicle
            .map(VehicleState::cell)
            .unwrap_or_else(|| self.party.leader().cell());
        let x = (u32::from(cell.x) / 2) % layout.width_chunks;
        let y = (u32::from(cell.y) / 2) % layout.height_chunks;
        for &(patch_x, patch_y, chunk) in self.effects.chunk_patches.iter().rev() {
            if patch_x == x && patch_y == y {
                return Some((chunk & 0xFF) as u8);
            }
        }
        layout
            .rows
            .get(y as usize)
            .and_then(|row| row.get(x as usize))
            .map(|chunk| (*chunk & 0xFF) as u8)
    }

    /// `Mota_Battle_BG_Index` as `FieldRoutine_Battle` stores it: the
    /// pack's `MotaBattleBGIndexes` entry for the chunk under the leader,
    /// through `GetMotaBattleBGIndex` (`ps4.asm:117936-117944`, `118165`).
    /// 0 when the map has no chunk grid, the pack no tables, or the chunk is
    /// past the table.
    #[must_use]
    pub fn motavia_battle_background(&self) -> u8 {
        self.vehicle_battle_terrain()
            .and_then(|chunk| self.data.battle_backgrounds()?.motavia(chunk))
            .unwrap_or(0)
    }

    /// The background `Battle_SetupBackground` (`ps4.asm:10124`) draws for a
    /// battle starting now: `event_battle` is the scene's
    /// `Event_Battle_Index`, `None` for a random encounter. `None` when the
    /// pack has no tables or the tables pick none.
    #[must_use]
    pub fn battle_background(&self, event_battle: Option<u16>) -> Option<u8> {
        self.battle_background_on(event_battle, self.map.id().0)
    }

    /// [`Self::battle_background`] as if the battle started on `map`, with
    /// everything else live: the chunk under the leader, the mounted state and
    /// Dark Force 2's flag. Debug fixtures that stage a battle on another map
    /// use it, so their choice is the one a real battle there would make.
    #[must_use]
    pub fn battle_background_on(&self, event_battle: Option<u16>, map: u16) -> Option<u8> {
        let mounted = self.vehicle_index().is_some();
        let chunk = (map == 0 || mounted)
            .then(|| self.vehicle_battle_terrain())
            .flatten();
        self.data.battle_backgrounds()?.select(
            event_battle,
            map,
            chunk,
            self.game.is_set(psiv_core::Flag::event(DARK_FORCE_2_FLAG)),
            mounted,
        )
    }

    /// One mounted field frame. The core has already transcribed movement,
    /// footprint collision, transition and dismount rules; this method wires
    /// their effects to map loads, triggers, encounters and the camera.
    pub(super) fn tick_vehicle(&mut self, input: Input) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        let effects = {
            let Some(vehicle) = self.vehicle.as_mut() else {
                return events;
            };
            vehicle.tick(&self.map, input)
        };
        let mut landed = None;
        let mut map_changed = false;
        for effect in effects {
            match effect {
                VehicleEffect::StepCompleted { cell } => {
                    landed = Some(cell);
                    events.push(RuntimeEvent::StepCompleted { cell });
                }
                VehicleEffect::Warp {
                    trigger,
                    target_map,
                    target_cell,
                    facing,
                    ..
                } => match self.change_map(target_map, target_cell, facing) {
                    Ok(loaded) => {
                        map_changed = true;
                        events.push(loaded.event(trigger));
                    }
                    // The loader's redirects can name another map than the
                    // warp; report the one that was not packed, as on foot.
                    Err(BridgeError::NotPacked(id)) => {
                        events.push(RuntimeEvent::UnpackedTarget {
                            map: psiv_core::MapId(id),
                        });
                    }
                    Err(_) => events.push(RuntimeEvent::UnpackedTarget { map: target_map }),
                },
                VehicleEffect::WarpUnmapped { cell } => {
                    events.push(RuntimeEvent::WarpUnmapped { cell });
                }
                // The field input's `$C` routine: `Event_GettingOffVehicle`
                // runs as an event, which stands the party down or refuses.
                VehicleEffect::GetOff => {
                    if self.install_scene(EventIndex(GETTING_OFF_VEHICLE)) {
                        events.push(RuntimeEvent::VehicleGetOff);
                        // Neither branch has a scene-exit music restore: the
                        // refusal leaves the music alone and the stand-down
                        // writes `AdjustMusicIDs`'s answer itself.
                        events.push(RuntimeEvent::SceneMusicRetained);
                    } else {
                        events.push(RuntimeEvent::SceneMissing {
                            event: GETTING_OFF_VEHICLE,
                        });
                    }
                }
            }
        }

        if let Some(cell) = landed
            && !map_changed
            && self.scene.is_none()
        {
            events.extend(self.evaluate_triggers(cell));
        }

        let map_id = self.map.id().0;
        let background = self.motavia_battle_background();
        if let Some(cell) = landed
            && !map_changed
            && self.scene.is_none()
            && let Some(set) = self.battles.as_mut()
            && set.table.enabled(map_id)
            && !EncounterClock::vehicle_suppressed(&self.map, cell)
            && set.clock.step()
            && self.rng.next_roll() & VEHICLE_MASK == 0
        {
            let mut rolls = Rng2::with_surrogate(&mut self.rng, self.frames);
            if let Some(formation) = set.table.select_vehicle(map_id, background, &mut rolls) {
                events.push(RuntimeEvent::EncounterRolled {
                    formation: formation.id,
                });
            }
        }

        self.update_visibility();
        if !self.field_suspended {
            self.tick_vehicle_wander();
        }
        if let Some(vehicle) = self.vehicle.as_ref() {
            self.camera.tick(driver_of(vehicle));
        }
        events
    }

    fn tick_vehicle_wander(&mut self) {
        if self.wander.is_empty() {
            return;
        }
        let Some(vehicle) = self.vehicle else { return };
        let party_cells = [vehicle.cell()];
        let driver = driver_of(&vehicle);
        self.wander.tick_with_driver_pixels(
            &mut self.map,
            &mut self.rng,
            &party_cells,
            (driver.x >> 16, driver.y >> 16),
            |i| !self.offscreen.get(i).copied().unwrap_or(true),
        );
    }
}

/// The camera driver for a mounted state.
pub(super) fn driver_of(vehicle: &VehicleState) -> Driver {
    let (ox, oy) = vehicle.render_offset_16ths();
    let at = PixelPos::from_cell(vehicle.cell());
    Driver {
        x: (at.x + ox) * ONE_PIXEL,
        y: (at.y + oy) * ONE_PIXEL,
    }
}
