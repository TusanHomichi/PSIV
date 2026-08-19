//! Scene execution and translation into runtime events.

use psiv_core::battle::Outcome;
use psiv_core::{
    ActorRef, Cell, Driver, Input, MapId, ONE_PIXEL, PixelPos, SceneEffect, SceneInput, SceneOp,
    WarpTrigger,
};

use crate::{Runtime, RuntimeEvent};

impl Runtime {
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
            self.scene = None;
            self.scene_camera_locked = false;
            if !self.retry_scene() {
                events.push(RuntimeEvent::SceneEnded);
            }
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
            // Mid-conversation resumes reopen saved dialogue state the window
            // does not model yet; auto-resume so the scene continues.
            SceneEffect::DialogueResume => self.scene_input = SceneInput::DialogueClosed,
            // The opening act asks no choices; auto-answer yes if one appears.
            SceneEffect::ChoiceRequested => self.scene_input = SceneInput::Choice(true),
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
                let map = self.map.id().0;
                for i in npc_index..npc_index + count {
                    self.despawned.insert((map, i));
                    let _ = self.map.set_npc_active(i, false);
                }
                events.push(RuntimeEvent::NpcsDespawned {
                    first: npc_index,
                    count,
                });
            }
            SceneEffect::NpcPromoted { npc, .. } => {
                self.despawned.insert((self.map.id().0, npc));
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
                op:
                    SceneOp::LoadMap {
                        map,
                        start_x,
                        start_y,
                        facing,
                        ..
                    },
            } => {
                // Start words are 8px units; the standing shift applies on Y,
                // as everywhere in the pack.
                let cell = Cell::new(start_x / 2, start_y / 2 + 1);
                match self.change_map(MapId(map), cell, facing) {
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
