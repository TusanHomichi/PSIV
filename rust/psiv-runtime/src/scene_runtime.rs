//! Scene execution and translation into runtime events.

use psiv_core::battle::Outcome;
use psiv_core::{
    ActorRef, Cell, Driver, Input, MapId, ONE_PIXEL, PixelPos, SceneEffect, SceneInput, SceneOp,
    WarpTrigger,
};

use crate::{Runtime, RuntimeEvent};

impl Runtime {
    /// Retail's persistent `$FFFFECEC`, for diagnostics and presentation
    /// restore seams. Reading it does not consume it.
    #[must_use]
    pub fn saved_sound_index(&self) -> u8 {
        self.saved_sound_index
    }

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
    pub(crate) fn dialogue_choice(&mut self, yes: bool) {
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
        let camera_arrived = self
            .scene
            .as_ref()
            .is_some_and(psiv_core::SceneRunner::is_waiting_for_camera)
            && self.camera_glide.is_none();
        // A camera completion belongs to the blocked op. Keep any unrelated
        // pending input for the next op rather than swallowing it here.
        let scene_input = if camera_arrived {
            SceneInput::CameraArrived
        } else {
            let pending = std::mem::take(&mut self.scene_input);
            if pending == SceneInput::None && input == Input::Action {
                SceneInput::EndingContinue
            } else {
                pending
            }
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
            self.boarding_body = None;
            self.scene_camera_locked = false;
            self.scene_triggers_pending = true;
            self.scene_tree_address = None;
            self.scene_panel_sprites = false;
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

    /// A scene's map load: `RefreshMap` (`ps4.asm:121767`) at a start given in
    /// eight-pixel units, with the `Map_Load_Flags` bits the scene cleared,
    /// then the runner is recast for the new map's objects and its map-load
    /// barrier released.
    fn scene_load_map(
        &mut self,
        (map, prev_map, start_x, start_y): (u16, u16, u16, u16),
        facing: psiv_core::Direction,
        clear_load_flags: u8,
        events: &mut Vec<RuntimeEvent>,
    ) {
        // Start words are 8px units; the standing shift applies on Y, as
        // everywhere in the pack. A scene's load is `RefreshMap`, whose
        // object-keeping bit is 3, and the op carries the `bclr`/`bset`
        // writes the scene made around it.
        let cell = Cell::new(start_x / 2, start_y / 2 + 1);
        match self.change_map_refresh(MapId(map), cell, facing, prev_map, clear_load_flags) {
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
            SceneEffect::VehicleBoardingAligned {
                index,
                cell,
                pan_camera,
            } => {
                let actors: Vec<_> = (0..self.party.len())
                    .filter_map(|slot| self.scene_party_actor(slot).copied())
                    .collect();
                if let Err(error) = self.party.resume_scripted(&self.map, &actors) {
                    events.push(RuntimeEvent::MapRefreshFailed {
                        error: error.to_string(),
                    });
                    return;
                }
                let Some(body) = psiv_core::VehicleState::new(
                    &self.map,
                    index,
                    cell,
                    self.party.leader().facing(),
                ) else {
                    events.push(RuntimeEvent::SceneFaulted {
                        fault: psiv_core::SceneFault::BadWrite,
                    });
                    return;
                };
                self.boarding_body = Some(body);
                if pan_camera {
                    // The object update preceding Event_MoveCamera leaves the
                    // camera's driver at the new sprite position. Otherwise
                    // the first normal scene-camera tick after the glide would
                    // misread the snap as a fresh 16-pixel walking velocity.
                    self.camera.reseat(psiv_core::Driver::at_cell(cell));
                    for op in [
                        psiv_core::PresentationOp::ClearHeldInput,
                        psiv_core::PresentationOp::RebuildSprites,
                    ] {
                        events.push(RuntimeEvent::ScenePresentation {
                            op: SceneOp::Presentation { op },
                        });
                    }
                    let at = psiv_core::PixelPos::from_cell(cell);
                    self.scene_move_camera_boarding(at.x, at.y);
                }
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
                        clear_load_flags,
                        ..
                    },
            } => {
                self.scene_load_map(
                    (map, prev_map, start_x, start_y),
                    facing,
                    clear_load_flags,
                    events,
                );
            }
            // The ship's flight legs read a table by the current map or by the
            // chosen world: `loc_64B02` (takeoff), `loc_64B34` (transit) and
            // `loc_64B5A` (landing, and Cancel's return), `ps4.asm:134540-134610`.
            // A key the table has no row for is a scene fault, never a silent
            // no-op: the cartridge's search would run off the table's end.
            SceneEffect::MapRequested {
                op: SceneOp::LoadFlightMap { leg },
            } => {
                let current = self.map.id().0;
                let world = self.world_index();
                match psiv_core::flight_target(leg, current, world) {
                    Some(target) => {
                        // `Field_Map_Index_2` becomes the map being left, or
                        // `$FFFF` when Cancel reloads the same map
                        // (`ps4.asm:133700`).
                        let prev_map = if target.previous_is_current {
                            current
                        } else {
                            0xFFFF
                        };
                        self.scene_load_map(
                            (target.map, prev_map, target.start_x, target.start_y),
                            psiv_core::Direction::Down,
                            0x08,
                            events,
                        );
                        if leg != psiv_core::FlightLeg::Return {
                            self.scene_camera_locked = true;
                            self.scene_input = SceneInput::None;
                            if let Some(runner) = self.scene.as_mut() {
                                runner.delay_frames(target.refresh_frames(leg));
                            }
                        }
                    }
                    None => {
                        events.push(RuntimeEvent::SceneFaulted {
                            fault: psiv_core::SceneFault::NoFlightTarget {
                                leg,
                                map: current,
                                world,
                            },
                        });
                        if let Some(runner) = self.scene.as_mut() {
                            runner.abort();
                        }
                    }
                }
            }
            // The scene's own `move.b #n, (World_Index).w`.
            SceneEffect::WorldIndexSet { world } => self.set_world_index(world),
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
                    SceneOp::FlightPlanet => {
                        // Tape 35: InitVRAMAndCRAM 15/13 frames, loc_643B8
                        // background 18, loc_644AE sprites 2/4, RunText2 3/2
                        // CPU frames. d4=1 means zero per-character waits.
                        let frames = if self.map.id().0 == 0 { 38 } else { 37 };
                        if let Some(runner) = self.scene.as_mut() {
                            runner.delay_frames(frames);
                        }
                    }
                    SceneOp::FlightPan => {
                        // loc_5ABDC pans only Y. RefreshMap seats the camera
                        // at start_y*8-$58; the subject adds $100 before the
                        // helper's twelve-bit mask, giving 128 two-pixel steps.
                        let (x, y) = self.camera.raw();
                        self.camera_glide = Some(crate::CameraGlide {
                            target_x: (x >> 16) & 0xFFF,
                            target_y: ((y >> 16) + 0x100) & 0xFFF,
                            speed: 2,
                            x_then_y: true,
                        });
                    }
                    SceneOp::FlightFieldReload => {
                        // The cutscene return's VInt plus the ordinary field
                        // reload up to Pal_FadeIn (tape 35 CPU stacks).
                        let frames = if self.map.id().0 == 0x18D { 36 } else { 26 };
                        if let Some(runner) = self.scene.as_mut() {
                            runner.delay_frames(frames);
                        }
                    }
                    SceneOp::PlayMusicIfSavedDifferent { id } => {
                        // All three boarding events compare the persistent
                        // word, then write Sound_Index and Saved_Sound_Index
                        // in that order only when the saved byte differs.
                        if self.saved_sound_index != id {
                            self.saved_sound_index = id;
                            for chosen in [SceneOp::PlaySound { id }, SceneOp::SetSavedMusic { id }]
                            {
                                events.push(RuntimeEvent::ScenePresentation { op: chosen });
                            }
                        }
                        events.push(RuntimeEvent::SceneMusicRetained);
                        return;
                    }
                    SceneOp::SetSavedMusic { id } => self.saved_sound_index = id,
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
                    // `DialogueTreesToRAM` changes which tree the scene's
                    // dialogue entries and its panel text index into. The
                    // renderer needs it for the panel text; the runtime needs
                    // it to resolve a scene dialogue's own entry, and entry
                    // ids are only meaningful against the current tree.
                    SceneOp::SetDialogueTree { rom_addr } => {
                        self.scene_tree_address = Some(rom_addr);
                    }
                    SceneOp::SetRenderSpritesInCutscene { enabled } => {
                        self.scene_panel_sprites = enabled;
                    }
                    // The scene's own `bset`/`bclr` writes on `Map_Load_Flags`
                    // (`$FFFFEC4E`). The next map load reads them: bit 3 spares
                    // the objects and the vehicle through a `RefreshMap`, and
                    // bits 0/2 through a `GameMode_LoadFieldMap`
                    // (`rust/psiv-runtime/src/map_change.rs`).
                    SceneOp::SetMapLoadFlags { set, clear } => {
                        self.map_load_flags = (self.map_load_flags | set) & !clear;
                    }
                    // One frame of `SceneOp::DriftNpcs`: the object's
                    // whole-pixel position after `FieldObj_UpdatePosition`
                    // (`$04501C`) added its step constants. Written to the
                    // field map so occupancy, collision and the renderer
                    // follow the drift at the cartridge's rate.
                    SceneOp::Presentation {
                        op: psiv_core::PresentationOp::NpcPixelPosition { npc, x, y },
                    } => {
                        let _ = self.map.set_npc_pixel_position(npc, x, y);
                    }
                    // `Event_MoveCamera` reading a live object rather than
                    // literals (`PresentationOp::CameraToActor`). The camera is
                    // runtime state because the on-screen tests feed the RNG
                    // stream, so a headless run must pan exactly like a
                    // rendered one.
                    SceneOp::Presentation {
                        op: psiv_core::PresentationOp::CameraToActor { actor, speed },
                    } => {
                        if let Some(walker) = self
                            .scene
                            .as_ref()
                            .and_then(|runner| runner.actor(actor))
                            .copied()
                        {
                            let at = psiv_core::PixelPos::from_cell(walker.cell);
                            self.scene_move_camera(at.x, at.y, i32::from(speed));
                        }
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
            SceneEffect::ActorFaced {
                actor: ActorRef::PartyMember(_) | ActorRef::Character(_),
                ..
            }
            | SceneEffect::ActorMoveStarted {
                actor: ActorRef::PartyMember(_) | ActorRef::Character(_),
                ..
            }
            | SceneEffect::ActorArrived {
                actor: ActorRef::PartyMember(_) | ActorRef::Character(_),
                ..
            }
            | SceneEffect::ActorPlaced {
                actor: ActorRef::PartyMember(_) | ActorRef::Character(_),
                ..
            } => {}
            // The runner reports these edges for its own bookkeeping: the
            // scene's value is read when it finishes, the purse is already
            // written to `GameState`, and `Finished` is handled by
            // `scene_tick` through `is_finished`.
            SceneEffect::Returned { .. }
            | SceneEffect::MoneyChanged { .. }
            | SceneEffect::Finished => {}
            // The runner emits `MapRequested` for exactly the three ops
            // handled above. Any other op is a runner bug, and it must not be
            // swallowed: the scene would wait on the map-load barrier forever.
            SceneEffect::MapRequested { .. } => {
                events.push(RuntimeEvent::SceneFaulted {
                    fault: psiv_core::SceneFault::BadWrite,
                });
                if let Some(runner) = self.scene.as_mut() {
                    runner.abort();
                }
            }
        }
    }
}

#[cfg(test)]
mod boarding_sound_tests {
    use super::*;
    use psiv_core::{Direction, MapId, StepFrames};
    use std::path::Path;

    #[test]
    fn all_three_boarding_frames_skip_same_track_and_emit_both_writes_on_mismatch() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        for event in [9, 10, 11] {
            for saved in [0x8D, 0x84] {
                let mut runtime = Runtime::new(
                    data.clone(),
                    0,
                    Cell::new(114, 177),
                    Direction::Down,
                    StepFrames::default(),
                )
                .expect("field runtime");
                runtime.saved_sound_index = saved;
                assert!(runtime.start_event(event));
                let mut sound_writes = Vec::new();
                let mut retained = 0;
                for _ in 0..8 {
                    for emitted in runtime.tick(Input::Neutral) {
                        match emitted {
                            RuntimeEvent::ScenePresentation {
                                op: SceneOp::PlaySound { id },
                            } => sound_writes.push(SceneOp::PlaySound { id }),
                            RuntimeEvent::ScenePresentation {
                                op: SceneOp::SetSavedMusic { id },
                            } => sound_writes.push(SceneOp::SetSavedMusic { id }),
                            RuntimeEvent::SceneMusicRetained => retained += 1,
                            _ => {}
                        }
                    }
                    if !runtime.scene_active() {
                        break;
                    }
                }
                assert!(!runtime.scene_active(), "event {event} completed");
                let expected = if saved == 0x8D {
                    vec![]
                } else {
                    vec![
                        SceneOp::PlaySound { id: 0x8D },
                        SceneOp::SetSavedMusic { id: 0x8D },
                    ]
                };
                assert_eq!(sound_writes, expected, "event {event}, saved {saved:#04x}");
                assert_eq!(retained, 1, "event {event} retains music at scene end");
                assert_eq!(runtime.saved_sound_index(), 0x8D);
            }
        }
    }

    #[test]
    fn saved_sound_word_resets_at_start_and_follows_field_but_not_refresh_loads() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        let map_music = data.map(psiv_data::MapId(0)).unwrap().music.id;
        assert_ne!(map_music, 0);
        assert_ne!(map_music, 0x8D);
        let new_game = Runtime::new_game(data.clone(), StepFrames::default()).unwrap();
        assert_eq!(new_game.saved_sound_index(), 0, "START clears $ECEC");

        let mut runtime = Runtime::new(
            data,
            0,
            Cell::new(114, 177),
            Direction::Down,
            StepFrames::default(),
        )
        .unwrap();
        assert_eq!(runtime.saved_sound_index(), map_music, "field entry");
        runtime.saved_sound_index = 0x8D;
        runtime
            .change_map_refresh(MapId(0), Cell::new(114, 177), Direction::Down, 0, 0)
            .unwrap();
        assert_eq!(runtime.saved_sound_index(), 0x8D, "RefreshMap keeps word");
        runtime.map_load_flags |= crate::map_change::LOAD_FLAG_AFTER_BATTLE;
        runtime
            .change_map(MapId(0), Cell::new(114, 177), Direction::Down)
            .unwrap();
        assert_eq!(runtime.saved_sound_index(), 0x8D, "battle keep bit");
        runtime
            .change_map(MapId(0), Cell::new(114, 177), Direction::Down)
            .unwrap();
        assert_eq!(
            runtime.saved_sound_index(),
            map_music,
            "ordinary field load"
        );
    }
}
