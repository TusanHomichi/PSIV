//! Scene execution and translation into runtime events.

use psiv_core::battle::Outcome;
use psiv_core::{
    ActorRef, Cell, Driver, Input, MapId, ONE_PIXEL, PixelPos, SceneEffect, SceneInput, SceneOp,
    WarpTrigger,
};

use crate::{Runtime, RuntimeEvent};

impl Runtime {
    /// A live layout write replaces only its named chunks. Validate the whole
    /// batch before changing collision or pixels, and retain the object cast.
    fn write_scene_map_chunks(
        &mut self,
        chunks: &[(u32, u32, u16)],
    ) -> Result<(), crate::BridgeError> {
        let reject = |message: String| crate::BridgeError::Rejected(message);
        let record = self
            .map_record()
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
                .find(|tile| tile.chunk_id == id)
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
        self.map = map;
        self.effects = effects;
        Ok(())
    }

    /// `loc_6DBAC` is an explicit live layout write, independent of the
    /// map-load flag walk. Reveal only the named base chunks; keep NPCs,
    /// unrelated overlays, party movement and camera state intact.
    fn restore_scene_map_chunks(
        &mut self,
        chunks: &[(u32, u32, u16)],
    ) -> Result<(), crate::BridgeError> {
        let record = self
            .map_record()
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
        self.map = map;
        self.effects = effects;
        Ok(())
    }

    /// Record the player's latest dialogue choice. A subsequent scene branch
    /// consumes it; if the scene is already waiting, release that gate now.
    pub fn dialogue_choice(&mut self, yes: bool) {
        if self.scene_choice_pending {
            self.scene_input = SceneInput::Choice(yes);
            self.scene_choice_pending = false;
            self.dialogue_answer = None;
        } else {
            self.dialogue_answer = Some(yes);
        }
    }

    /// One tick of a running scene: feed any pending input, translate the
    /// effects, close out the scene when the runner finishes.
    pub(crate) fn scene_tick(&mut self, input: Input) -> Vec<RuntimeEvent> {
        let pending = std::mem::take(&mut self.scene_input);
        let scene_input = if pending == SceneInput::None && input == Input::Action {
            SceneInput::EndingContinue
        } else {
            pending
        };
        let mut events = Vec::new();
        let Some(runner) = self.scene.as_mut() else {
            return events;
        };
        let effects = runner.tick(&self.map, &mut self.game, scene_input);
        let finished = runner.is_finished();
        for effect in effects {
            self.translate_scene_effect(effect, &mut events);
        }
        self.tick_scene_camera();
        if finished {
            let actors: Vec<_> = (0..self.party.len())
                .filter_map(|slot| self.scene_party_actor(slot).copied())
                .collect();
            if let Err(error) = self.party.resume_scripted(&self.map, &actors) {
                events.push(RuntimeEvent::MapRefreshFailed {
                    error: error.to_string(),
                });
            }
            self.scene = None;
            self.scene_camera_locked = false;
            self.scene_triggers_pending = true;
            events.push(RuntimeEvent::SceneEnded);
        }
        events
    }

    /// The camera latch, driven by the scripted leader.
    ///
    /// Retail scene walks move the real `Character_1` object, so the field
    /// camera follows them exactly as it follows player walking. The clone's
    /// scripted actors are a parallel cast, so the latch has to be fed their
    /// position explicitly — otherwise the party strolls off a frozen screen.
    /// A scene camera lock (`SetFollowMode` bit 2) or an in-flight
    /// `Event_MoveCamera` pan owns the camera instead, and a scripted teleport
    /// (`PlaceActor`, a map load) reseats rather than chasing the jump as a
    /// one-frame velocity.
    fn tick_scene_camera(&mut self) {
        if self.scene_camera_locked || self.camera_glide.is_some() {
            return;
        }
        let Some(actor) = self.scene_party_actor(0).copied() else {
            return;
        };
        let (ox, oy) = actor.render_offset_16ths(self.party.leader().step_frames());
        let at = PixelPos::from_cell(actor.cell);
        let driver = Driver {
            x: (at.x + ox) * ONE_PIXEL,
            y: (at.y + oy) * ONE_PIXEL,
        };
        let (last_x, last_y) = self.camera.driver_position();
        let jumped = (driver.x - last_x).abs() > 16 * ONE_PIXEL
            || (driver.y - last_y).abs() > 16 * ONE_PIXEL;
        if jumped {
            self.camera.reseat(driver);
        } else {
            self.camera.tick(driver);
        }
    }

    fn translate_scene_effect(&mut self, effect: SceneEffect, events: &mut Vec<RuntimeEvent>) {
        match effect {
            SceneEffect::DialogueOpen(id) => {
                self.dialogue_answer = None;
                events.push(RuntimeEvent::SceneDialogue { entry: id.0 });
            }
            SceneEffect::DialogueOpenFromNpc { actor } => {
                let entry = match actor {
                    ActorRef::Npc(i) => self
                        .map_record()
                        .and_then(|r| r.npcs.get(i))
                        .map(|n| n.dialogue_id),
                    _ => None,
                };
                match entry {
                    Some(entry) => events.push(RuntimeEvent::SceneDialogue { entry }),
                    // A missing binding is a data defect; resume the scene so
                    // it cannot hang, and say so.
                    None => self.scene_input = SceneInput::DialogueClosed,
                }
            }
            SceneEffect::DialogueResume => events.push(RuntimeEvent::SceneDialogueResume),
            SceneEffect::ChoiceRequested => {
                if let Some(answer) = self.dialogue_answer.take() {
                    self.scene_input = SceneInput::Choice(answer);
                } else {
                    self.scene_choice_pending = true;
                    events.push(RuntimeEvent::SceneChoiceRequested);
                }
            }
            SceneEffect::BattleRequested { index } => match self.start_boss_battle(index) {
                Ok(initial) => events.push(RuntimeEvent::SceneBattleStarted {
                    index,
                    events: initial.events,
                    sounds: initial.sounds,
                    animations: initial.animations,
                }),
                Err(error) => {
                    events.push(RuntimeEvent::SceneBattleFailed {
                        index,
                        error: error.to_string(),
                    });
                    self.scene_input = SceneInput::BattleFinished {
                        outcome: Outcome::Escaped,
                    };
                }
            },
            SceneEffect::NpcDespawned { npc_index, count } => {
                for i in npc_index..npc_index + count {
                    let _ = self.map.set_npc_active(i, false);
                }
                events.push(RuntimeEvent::NpcsDespawned {
                    first: npc_index,
                    count,
                });
            }
            SceneEffect::NpcPromoted { npc, .. } => {
                let _ = self.map.set_npc_active(npc, false);
                events.push(RuntimeEvent::NpcsDespawned {
                    first: npc,
                    count: 1,
                });
                events.push(RuntimeEvent::PartyChanged);
            }
            SceneEffect::PartySlotsSaved { slots } => {
                self.saved_party_slots = Some(slots);
            }
            SceneEffect::PartySlotsRestored => {
                let Some(slots) = self.saved_party_slots.take() else {
                    events.push(RuntimeEvent::SceneFaulted {
                        fault: psiv_core::SceneFault::BadWrite,
                    });
                    return;
                };
                self.game.set_party(slots);
                self.resize_party();
                events.push(RuntimeEvent::PartyChanged);
            }
            SceneEffect::PartyChanged | SceneEffect::CharSlotCopied { .. } => {
                self.resize_party();
                // A scene may name a character immediately after joining it
                // (the Rika cutscene does exactly that), so the cast must
                // grow refs with the party — but positions the script has
                // already staged survive: sync, don't recast.
                let cast = self.build_cast();
                if let Some(runner) = self.scene.as_mut() {
                    runner.sync_cast(cast);
                }
                events.push(RuntimeEvent::PartyChanged);
            }
            SceneEffect::InventoryChanged => events.push(RuntimeEvent::InventoryChanged),
            SceneEffect::MapChunksRestored { chunks } => {
                match self.restore_scene_map_chunks(chunks) {
                    Ok(()) => events.push(RuntimeEvent::MapRefreshed),
                    Err(error) => events.push(RuntimeEvent::MapRefreshFailed {
                        error: error.to_string(),
                    }),
                }
            }
            SceneEffect::MapChunksWritten { chunks } => {
                match self.write_scene_map_chunks(&chunks) {
                    Ok(()) => events.push(RuntimeEvent::MapRefreshed),
                    Err(error) => {
                        self.scene = None;
                        events.push(RuntimeEvent::MapRefreshFailed {
                            error: error.to_string(),
                        });
                        events.push(RuntimeEvent::SceneFaulted {
                            fault: psiv_core::SceneFault::BadWrite,
                        });
                    }
                }
            }
            SceneEffect::VehicleChanged { index } => {
                if self.set_vehicle_index(index).is_err() {
                    events.push(RuntimeEvent::SceneFaulted {
                        fault: psiv_core::SceneFault::BadWrite,
                    });
                    return;
                }
                events.push(RuntimeEvent::VehicleChanged { index });
            }
            SceneEffect::RosterChanged { who } => {
                events.push(RuntimeEvent::RosterChanged { who });
            }
            SceneEffect::MapRequested {
                op: SceneOp::TakeMapTransition,
            } => {
                self.take_scene_map_transition(events);
            }
            SceneEffect::MapRequested {
                op:
                    SceneOp::LoadMap {
                        map,
                        prev_map,
                        start_x,
                        start_y,
                        facing,
                        ..
                    },
            } => {
                // Start words are 8px units; the standing shift applies on Y,
                // as everywhere in the pack.
                let cell = Cell::new(start_x / 2, start_y / 2 + 1);
                match self.change_map_from(MapId(map), cell, facing, prev_map) {
                    Ok(()) => {
                        let cast = self.build_cast();
                        if let Some(runner) = self.scene.as_mut() {
                            runner.recast(cast);
                        }
                        events.push(RuntimeEvent::MapChanged {
                            map: MapId(map),
                            trigger: WarpTrigger::MapChange,
                        });
                    }
                    Err(_) => events.push(RuntimeEvent::UnpackedTarget { map: MapId(map) }),
                }
                // Whether the target was accepted or rejected, release the
                // interpreter's map-load barrier. A rejected target is then
                // allowed to report its own missing-actor fault instead of
                // hanging a scene forever.
                self.scene_input = SceneInput::MapLoaded;
            }
            // MapDataManager effects are load-time work, not a live rebuild on
            // every flag write. The next map load evaluates the new flag and
            // applies the pack's Igglanova despawn gate.
            SceneEffect::FlagChanged { .. } => {}
            SceneEffect::Faulted(fault) => events.push(RuntimeEvent::SceneFaulted { fault }),
            // Presentation is already sequenced by SceneRunner. Preserve the
            // effect's position in this tick's Vec so the shell sees the
            // same choreography and timing as the interpreter produced.
            // Camera ops are consumed here as well: the camera is runtime
            // state (it feeds the RNG stream through the on-screen tests), so
            // a headless run must pan exactly like a rendered one.
            SceneEffect::Presentation { op } => {
                match op {
                    SceneOp::SetCameraPos { x, y } => self.set_camera(x, y),
                    SceneOp::MoveCamera { x, y, speed } => {
                        self.scene_move_camera(x, y, i32::from(speed));
                    }
                    // Bit 2 hands the camera to the scene (the opening's
                    // walk off the screen edge); the scripted-leader follow
                    // in `tick_scene_camera` honours it.
                    SceneOp::SetFollowMode { bits } => {
                        self.scene_camera_locked = bits & 0b100 != 0;
                    }
                    _ => {}
                }
                events.push(RuntimeEvent::ScenePresentation { op });
            }
            SceneEffect::GameCleared => {
                self.game_cleared = true;
            }
            // A scripted facing is written straight into the field object slot
            // by the cartridge, so it has to reach the map's own record and not
            // only the scene's actor list.
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(index),
                facing,
            } => {
                let _ = self.map.set_npc_facing(index, facing);
            }
            // Scripted movement has two consumers: the runner's actor list for
            // interpolation, and the live FieldMap for collision/comparator
            // reads. The old bridge handled facing but dropped both movement
            // edges, so a scene NPC appeared to walk only in the renderer and
            // landed back at its old map cell. Land on the map at the start
            // edge as retail's destination write does, and assert the arrival
            // again when the runner reaches it.
            SceneEffect::ActorMoveStarted {
                actor: ActorRef::Npc(index),
                to,
            }
            | SceneEffect::ActorArrived {
                actor: ActorRef::Npc(index),
                at: to,
            }
            | SceneEffect::ActorPlaced {
                actor: ActorRef::Npc(index),
                at: to,
            } => {
                let _ = self.map.set_npc_cell(index, to);
            }
            // Party/character objects are represented by the scene cast for
            // now; their authoritative live positions are rebuilt by the
            // party driver. Presentation ops and their arrivals need no other
            // runtime action.
            _ => {}
        }
    }
}
