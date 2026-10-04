//! The field-mode frame tick: the per-frame order of `Main_Frame_Count`, the
//! shared RNG, movement effects, trigger and encounter rolls, and the object
//! and camera updates.

use psiv_core::battle::{Rng2, Rolls};
use psiv_core::{Effect, Input, MapId};

use crate::encounters::{EncounterClock, FOOT_MASK};
use crate::geometry::driver_of;
use crate::{BridgeError, Runtime, RuntimeEvent};

impl Runtime {
    /// Advances one tick and resolves any map change.
    pub(crate) fn tick(&mut self, input: Input) -> Vec<RuntimeEvent> {
        // `Main_Frame_Count` first, then the vblank tick: the cartridge
        // stirs the one seed every frame in every game mode (VInt handler,
        // ps4.asm:617) — scenes included.
        self.frames = self.frames.wrapping_add(1);
        self.rng.step();
        if self.game_over || self.field_notice().is_some() || self.loot.is_some() {
            return Vec::new();
        }
        // A battle owns the frame: the field is parked exactly as
        // GameMode_Battle parks it, and the shell drives rounds through
        // [`Runtime::battle_round`].
        if self.battle.is_some() {
            return Vec::new();
        }
        if self.battle_field_refresh_pending {
            return self.return_to_field();
        }
        self.tick_camera_glide();
        if self.scene.is_some() {
            // The frame the mode dispatcher spends entering event mode.
            if self.scene_warmup {
                self.scene_warmup = false;
                return Vec::new();
            }
            let motion_frame = self
                .scene
                .as_ref()
                .is_some_and(psiv_core::SceneRunner::completes_map_update_loop);
            let events = self.scene_tick(input);
            if (motion_frame
                || self
                    .scene
                    .as_ref()
                    .is_some_and(psiv_core::SceneRunner::runs_map_updates))
                && !self.field_suspended
            {
                self.run_map_updates();
            }
            return events;
        }
        // The field-mode tick: GameMode_Field opens with an unconditional
        // UpdateRNGSeed before dispatching (ps4.asm:107638). It vanishes
        // while a window is up because window loops never return to the
        // mode dispatcher — hence the suspension gate, which also parks the
        // wander draws further down. docs/field/NPC_WANDER.md, "per-frame tick
        // structure".
        if !self.field_suspended {
            self.rng.step();
        }
        if self.scene_triggers_pending && !self.field_suspended {
            self.scene_triggers_pending = false;
            let events = self.evaluate_triggers(self.state().cell());
            if !events.is_empty() {
                return events;
            }
        }
        if self.vehicle.is_some() {
            let events = self.tick_vehicle(input);
            if !self.field_suspended
                && self.scene.is_none()
                && !events.iter().any(|e| {
                    matches!(
                        e,
                        RuntimeEvent::MapChanged { .. } | RuntimeEvent::EncounterRolled { .. }
                    )
                })
            {
                self.run_map_updates();
            }
            return events;
        }
        let mut events = Vec::new();
        let (mut landed, field_effects) = match self.field_status.pending.take() {
            Some((cell, effects)) => (Some(cell), effects),
            None => (None, self.party.tick(&self.map, input)),
        };
        if std::mem::take(&mut self.field_status.flash_after_notices) {
            events.push(RuntimeEvent::FieldPoisonFlash);
        }
        let mut map_changed = false;
        let mut interaction_started = false;
        let mut field_effects = field_effects.into_iter();
        while let Some(effect) = field_effects.next() {
            match effect {
                Effect::StepCompleted { cell } => {
                    landed = Some(cell);
                    events.push(RuntimeEvent::StepCompleted { cell });
                    self.update_field_status(cell, &mut events);
                    if self.field_notice().is_some() {
                        self.field_status.pending = Some((cell, field_effects.collect()));
                        return events;
                    }
                    // RunEvents owns the opened elevator tile before the
                    // ordinary collision/warp path can report it unmapped.
                    if self.elevator_at(cell) {
                        events.extend(self.evaluate_triggers(cell));
                        if self.scene_active() {
                            return events;
                        }
                    }
                }
                Effect::Warp {
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
                    Err(BridgeError::NotPacked(id)) => {
                        events.push(RuntimeEvent::UnpackedTarget { map: MapId(id) });
                    }
                    // Any other bridge failure on a packed map is a defect the
                    // pack's own validation should have caught; surface it the
                    // same way rather than panicking mid-game.
                    Err(_) => events.push(RuntimeEvent::UnpackedTarget { map: target_map }),
                },
                Effect::WarpUnmapped { cell } => events.push(RuntimeEvent::WarpUnmapped { cell }),
                Effect::Interact {
                    npc_index,
                    cell,
                    reach,
                } => {
                    interaction_started = true;
                    if !self.start_chest_interaction(npc_index)
                        && !self.start_interaction_event(&mut events)
                    {
                        events.push(RuntimeEvent::Interact {
                            npc_index,
                            cell,
                            reach,
                        });
                    }
                }
                Effect::InteractNothing { facing } => {
                    interaction_started = true;
                    if !self.start_interaction_event(&mut events) {
                        events.push(RuntimeEvent::InteractNothing { facing });
                    }
                }
            }
        }

        // Trigger evaluation on landing, exactly like the cartridge's
        // RunEvents: per rest-frame after a step, before the player moves
        // again, skipped when a transition already changed the map.
        if let Some(cell) = landed
            && !map_changed
            && !interaction_started
        {
            events.extend(self.evaluate_triggers(cell));
        }

        // RunRandomBattles ($05784E): only on a landing that neither changed
        // the map nor started a scene, on a map whose binding rolls at all,
        // and never standing on/next to transition tiles. Ten free steps,
        // then seed-word & $1F == 0 fires — one extra LCG draw per rolling
        // step, exactly the cartridge's extra call.
        if let Some(cell) = landed
            && !map_changed
            && self.scene.is_none()
            && self.loot.is_none()
            && let Some(set) = self.battles.as_mut()
            && set.table.enabled(self.map.id().0)
            && self.effects.updates.random_battles != Some(false)
            && !EncounterClock::suppressed(&self.map, cell)
            && set.clock.step()
            && self.rng.next_roll() & FOOT_MASK == 0
        {
            // The formation pick is UpdateRNGSeed2's: same seed, other mixer.
            let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
            if let Some(formation) = set.table.select(self.map.id().0, cell, &mut rng2) {
                events.push(RuntimeEvent::EncounterRolled {
                    formation: formation.id,
                });
            }
        }

        // Every object's routine opens with the visibility test, whether or not
        // it wanders, so the flags are refreshed unconditionally.
        self.update_visibility();
        // Wander and bespoke draws are conditional consumers of the same
        // stream — an idle NPC whose countdown expires rolls once; most frames
        // none do.
        // This runs before the camera's own tick because the cartridge's
        // visibility test reads sprite positions written the frame before.
        if !self.field_suspended {
            self.tick_field_objects();
        }
        // Object latches use the party's post-movement position and the prior
        // sprite threshold; map updates adjust those counters before commit.
        self.camera.latch_driver(driver_of(self.party.leader()));
        // Retail FieldRoutine_Controls: objects, AnimateTiles, RunMapUpdates,
        // camera commit. Transition/scene/battle/input dispatch frames skip it
        // (ps4.asm:116755-116798); menus/dialogue never call the retail table.
        if !self.field_suspended
            && !map_changed
            && !interaction_started
            && self.scene.is_none()
            && !events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::EncounterRolled { .. }))
        {
            self.run_map_updates();
        }
        self.camera.commit_driver();
        events
    }
}
