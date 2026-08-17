//! Runtime bridge for the mounted field state and vehicle battle handoff.
//!
//! The core owns the rules. This module owns the mutable shell seams: the
//! `$F43C` selector, the current map/effects, encounter RNG, map transitions,
//! and the saved record copied back after a battle.

use psiv_core::battle::{Rng2, Rolls};
use psiv_core::{Cell, Driver, Input, ONE_PIXEL, PixelPos, VehicleEffect, VehicleState};

use crate::encounters::{EncounterClock, VEHICLE_MASK};
use crate::events::RuntimeEvent;
use crate::{BridgeError, Runtime};

/// Retail's `MotaBattleBGIndexes`, `ps4.asm:118181-118224`.
const MOTA_BATTLE_BG_INDEXES: [u8; 43] = [
    0, 3, 3, 3, 3, 1, 1, 1, 3, 3, 0, 1, 0, 1, 0, 1, 0, 3, 0, 3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];

impl Runtime {
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
    pub fn set_vehicle_index(&mut self, index: u16) -> Result<(), BridgeError> {
        if index > psiv_core::VEHICLE_INDEX_MAX {
            return Err(BridgeError::Rejected(format!(
                "vehicle selector {index} is outside 0..={} ",
                psiv_core::VEHICLE_INDEX_MAX
            )));
        }
        if index == 0 {
            self.game.set_vehicle_index(0);
            self.vehicle = None;
            return Ok(());
        }
        if self
            .vehicle
            .as_ref()
            .is_some_and(|vehicle| vehicle.index() == index)
        {
            self.game.set_vehicle_index(index);
            return Ok(());
        }
        let leader = self.party.leader();
        let vehicle = VehicleState::new(&self.map, index, leader.cell(), leader.facing())
            .ok_or_else(|| BridgeError::Rejected(format!("cannot mount vehicle {index}")))?;
        self.game.set_vehicle_index(index);
        self.vehicle = Some(vehicle);
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
        let layout = match self.effects.variant {
            Some(index) => record.layout_variants.get(index)?.vehicle_battle.as_ref()?,
            None => record.vehicle_battle.as_ref()?,
        };
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

    /// The zero-based `Mota_Battle_BG_Index` after
    /// `GetMotaBattleBGIndex` subtracts a non-zero table byte.
    #[must_use]
    pub fn vehicle_motavia_background(&self) -> u8 {
        let Some(raw) = self.vehicle_battle_terrain() else {
            return 0;
        };
        match MOTA_BATTLE_BG_INDEXES
            .get(raw as usize)
            .copied()
            .unwrap_or(0)
        {
            0 => 0,
            value => value - 1,
        }
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
                    Ok(()) => {
                        map_changed = true;
                        events.push(RuntimeEvent::MapChanged {
                            map: target_map,
                            trigger,
                        });
                    }
                    Err(_) => events.push(RuntimeEvent::UnpackedTarget { map: target_map }),
                },
                VehicleEffect::WarpUnmapped { cell } => {
                    events.push(RuntimeEvent::WarpUnmapped { cell });
                }
                VehicleEffect::Dismount => {
                    if self.set_vehicle_index(0).is_ok() {
                        events.push(RuntimeEvent::VehicleChanged { index: 0 });
                    }
                }
                VehicleEffect::DismountBlocked => {
                    events.push(RuntimeEvent::VehicleDismountBlocked);
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
        let background = self.vehicle_motavia_background();
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
