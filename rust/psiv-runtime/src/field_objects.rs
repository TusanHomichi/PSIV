//! Runtime glue for the shared field-object tick.

use psiv_core::{BespokeContext, BespokeFlags, BespokeKind, Cell, Flag};

use crate::geometry::{driver_of, object_position};
use crate::{Runtime, RuntimeEvent};

impl Runtime {
    /// One frame of field-object visibility: the retail camera box, with
    /// off-screen routines frozen before any movement or RNG draw.
    pub(crate) fn update_visibility(&mut self) {
        let flags = self.bespoke_flags();
        self.bespoke.sync_flags(&mut self.map, flags);
        let camera = self.camera;
        self.offscreen.clear();
        self.offscreen
            .extend(self.map.npcs().iter().enumerate().map(|(index, npc)| {
                if !npc.active {
                    return true;
                }
                // An object whose routine never calls the test keeps the
                // flag its slot was initialised with and is updated
                // wherever it is.
                if !psiv_core::type_tests_visibility(npc.id.0) {
                    return false;
                }
                let wanderer = self
                    .wander
                    .wanderers()
                    .iter()
                    .find(|w| w.npc_index() == index);
                let bespoke = self
                    .bespoke
                    .actors()
                    .iter()
                    .find(|actor| actor.npc_index() == index);
                let (x, y) = object_position(npc, wanderer, bespoke);
                !camera.sees_with_camera_bypass(x, y, npc.camera_bypass)
            }));
    }

    /// Whether object `index` was off screen this frame, and so was not updated.
    #[must_use]
    pub fn object_offscreen(&self, index: usize) -> bool {
        self.offscreen.get(index).copied().unwrap_or(true)
    }

    /// One frame of a scene's `RunObjectRoutine` loop: only object `npc`
    /// runs. Its routine's `FieldObj_OnScreenTest` comes first, on the
    /// position the last frame left; off screen, a departing routine clears
    /// its own slot, which is what releases the scene.
    pub(crate) fn tick_scene_object(&mut self, npc: usize, events: &mut Vec<RuntimeEvent>) {
        let Some(object) = self.map.npcs().get(npc).copied() else {
            return;
        };
        if !object.active {
            return;
        }
        let actor = self.bespoke.get(npc);
        let (x, y) = object_position(&object, None, actor);
        if !self
            .camera
            .sees_with_camera_bypass(x, y, object.camera_bypass)
        {
            if actor.is_some_and(|a| matches!(a.kind(), BespokeKind::Departing { .. })) {
                self.bespoke.remove(npc);
                let _ = self.map.set_npc_active(npc, false);
                events.push(RuntimeEvent::NpcsDespawned {
                    first: npc,
                    count: 1,
                });
            }
            return;
        }
        let party_cells: Vec<Cell> = self.party.members().iter().map(|m| m.cell).collect();
        let driver = driver_of(self.party.leader());
        let context = BespokeContext {
            frame: self.frames,
            driver_pixels: (driver.x >> 16, driver.y >> 16),
            party: &party_cells,
            flags: self.bespoke_flags(),
        };
        self.bespoke
            .tick_one_for_npc(npc, &mut self.map, &mut self.rng, context);
    }

    pub(crate) fn tick_field_objects(&mut self) {
        if self.wander.is_empty() && self.bespoke.is_empty() {
            return;
        }
        let party_cells: Vec<Cell> = self.party.members().iter().map(|m| m.cell).collect();
        let driver = driver_of(self.party.leader());
        let context = BespokeContext {
            frame: self.frames,
            driver_pixels: (driver.x >> 16, driver.y >> 16),
            party: &party_cells,
            flags: self.bespoke_flags(),
        };
        for index in 0..self.map.npcs().len() {
            if self.offscreen.get(index).copied().unwrap_or(true) {
                continue;
            }
            if self.wander.get(index).is_some() {
                self.wander.tick_one_for_npc(
                    index,
                    &mut self.map,
                    &mut self.rng,
                    &party_cells,
                    context.driver_pixels,
                );
            } else {
                self.bespoke
                    .tick_one_for_npc(index, &mut self.map, &mut self.rng, context);
            }
        }
    }

    fn bespoke_flags(&self) -> BespokeFlags {
        BespokeFlags {
            principal_confession: self.game.is_set(Flag::event(0x0C)),
            igglanova_zema: self.game.is_set(Flag::event(0x33)),
            penguin: self.game.is_set(Flag::event(0x8A)),
            musk_cats: self.game.is_set(Flag::event(0x90)),
            inner_sanctuary: self.game.is_set(Flag::event(0x96)),
            esp_mansion_guards: self.game.is_set(Flag::temp(0x1A)),
        }
    }
}
