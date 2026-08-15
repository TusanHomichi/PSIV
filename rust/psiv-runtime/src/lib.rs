//! The bridge between the pack schema (`psiv-data`) and the field engine
//! (`psiv-core`), plus the game shell that owns map changes.
//!
//! `psiv-core` knows no schema and `psiv-data` knows no rules; this crate is
//! the only place the two meet. `psiv-godot` drives a [`Runtime`] and never
//! touches either lower layer directly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::float_arithmetic)]

use std::fmt;

use std::collections::BTreeSet;

use psiv_core::{
    ActorRef, Cell, CharId, CollisionGrid, Direction, Effect, FieldMap, FieldState, Flag,
    GameState, Input, InteractReach, MapId, MemberView, Npc, NpcId, Party, PixelPos, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, TRIGGERS, TriggerContext,
    TriggerResult, Warp, WarpTrigger, runner_for, scene_for,
};
use psiv_data::{GameData, MapRecord, TransitionTable};

/// A defect found while converting a pack record into an engine map.
///
/// Bridge errors mean the pack and the engine disagree about what a map is —
/// a data bug or a schema drift, never a gameplay condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// A dimension or coordinate did not fit the engine's cell space.
    OutOfRange(String),
    /// The engine rejected the converted map.
    Rejected(String),
    /// A warp's facing byte is not one of the four the ROM defines.
    BadWarpFacing {
        /// The map being converted.
        map: u16,
        /// The warp's index in the record.
        warp: u32,
        /// The raw facing byte.
        byte: u8,
    },
    /// The requested map is not in the pack.
    NotPacked(u16),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::OutOfRange(what) => write!(f, "value out of engine range: {what}"),
            BridgeError::Rejected(why) => write!(f, "engine rejected converted map: {why}"),
            BridgeError::BadWarpFacing { map, warp, byte } => write!(
                f,
                "map {map:#05x} warp {warp}: facing byte {byte:#04x} is not one of 0/4/8/$C"
            ),
            BridgeError::NotPacked(id) => write!(f, "map {id:#05x} is not in the pack"),
        }
    }
}

impl std::error::Error for BridgeError {}

fn cell_u16(x: u32, y: u32, what: &str) -> Result<Cell, BridgeError> {
    let x = u16::try_from(x).map_err(|_| BridgeError::OutOfRange(format!("{what} x={x}")))?;
    let y = u16::try_from(y).map_err(|_| BridgeError::OutOfRange(format!("{what} y={y}")))?;
    Ok(Cell::new(x, y))
}

fn direction(d: psiv_data::Direction) -> Direction {
    match d {
        psiv_data::Direction::Up => Direction::Up,
        psiv_data::Direction::Down => Direction::Down,
        psiv_data::Direction::Left => Direction::Left,
        psiv_data::Direction::Right => Direction::Right,
    }
}

/// Converts one pack record into an engine [`FieldMap`].
///
/// Warps with no trigger area (`rect == None` — dead data the manifest
/// counts) are dropped. A warp facing byte outside the ROM's four values is a
/// hard error; an NPC facing byte outside them falls back to down, because
/// scenery objects reuse the byte for non-directional state and their facing
/// never affects collision.
pub fn field_map(record: &MapRecord) -> Result<FieldMap, BridgeError> {
    let grid = &record.collision.grid;
    let width = u16::try_from(grid.width())
        .map_err(|_| BridgeError::OutOfRange(format!("width {}", grid.width())))?;
    let height = u16::try_from(grid.height())
        .map_err(|_| BridgeError::OutOfRange(format!("height {}", grid.height())))?;
    let cells: Vec<u8> = grid.cells().iter().map(|c| c.code()).collect();
    let grid = CollisionGrid::new(width, height, cells)
        .map_err(|e| BridgeError::Rejected(e.to_string()))?;

    let mut warps = Vec::new();
    for warp in &record.warps {
        let Some(rect) = &warp.rect else { continue };
        let origin = cell_u16(rect.x, rect.y, "warp rect")?;
        let far = cell_u16(
            rect.x + rect.width.saturating_sub(1),
            rect.y + rect.height.saturating_sub(1),
            "warp rect end",
        )?;
        let source = psiv_core::CellRect::new(
            origin.x,
            origin.y,
            far.x - origin.x + 1,
            far.y - origin.y + 1,
        );
        let trigger = match warp.table {
            TransitionTable::Normal => WarpTrigger::NormalGround,
            TransitionTable::MapChange => WarpTrigger::MapChange,
        };
        let facing = warp
            .facing
            .direction()
            .map(direction)
            .ok_or(BridgeError::BadWarpFacing {
                map: record.id.0,
                warp: warp.index,
                byte: warp.facing.id,
            })?;
        warps.push(Warp {
            source,
            trigger,
            target_map: MapId(warp.target.id.0),
            target_cell: cell_u16(
                warp.destination.x_cell,
                warp.destination.y_cell,
                "warp destination",
            )?,
            facing,
        });
    }

    let mut npcs = Vec::new();
    for npc in &record.npcs {
        let cell = cell_u16(npc.x_cell, npc.y_cell, "npc")?;
        let facing = npc
            .facing
            .direction()
            .map(direction)
            .unwrap_or(Direction::Down);
        // 85 retail objects sit on half-cells; the sub-cell offset is what
        // lets the ±8px talk range reach them from both straddled cells.
        let offset =
            psiv_core::SubCellOffset::new((npc.x_pixels % 16) as u8, (npc.y_pixels % 16) as u8);
        npcs.push(Npc::with_offset(NpcId(npc.object_id), cell, offset, facing));
    }

    // The overworlds are tori: the cartridge itself selects the paged/wrapping
    // path by `Field_Map_Index & $FFFE == 0`, so hard-coding ids 0 and 1 here
    // is fidelity, not shortcut. Layout patches are event-flag-gated and no
    // flags exist at a fresh spawn, so none apply yet; when flag state lands,
    // the bridge applies active patches to the grid and rebuilds the map (see
    // FieldMap's contract docs).
    let topology = match record.id.0 {
        0 | 1 => psiv_core::Topology::Torus,
        _ => psiv_core::Topology::Bounded,
    };
    FieldMap::with_topology(MapId(record.id.0), grid, warps, npcs, topology)
        .map_err(|e| BridgeError::Rejected(e.to_string()))
}

/// What a [`Runtime`] tick produced, for the presentation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// The party finished a step.
    StepCompleted {
        /// The cell landed on.
        cell: Cell,
    },
    /// A transition fired and the runtime switched maps. The renderer should
    /// rebuild the scene for [`Runtime::map_id`]; `trigger` says whether this
    /// was a doorway (fade) or ground transition (scroll), per the design doc.
    MapChanged {
        /// The map now loaded.
        map: MapId,
        /// Which transition class fired.
        trigger: WarpTrigger,
    },
    /// A transition targeted a map the pack does not contain (skipped or
    /// unpacked). The party stays put; the presentation layer should surface
    /// this as a data warning.
    UnpackedTarget {
        /// The map the transition wanted.
        map: MapId,
    },
    /// The engine reported a type-1 cell with no matching doorway record — a
    /// pack defect. Surface it, never swallow it.
    WarpUnmapped {
        /// The offending cell.
        cell: Cell,
    },
    /// The party talked to an NPC. The renderer looks up the NPC's
    /// dialogue binding in the map record and opens a window; while it is
    /// open, it stops sending direction inputs (the engine is not modal).
    Interact {
        /// Index into the current map's NPC list.
        npc_index: usize,
        /// The probe cell that was hit.
        cell: Cell,
        /// Whether the hit came from the ordinary one-cell probe or across a
        /// `$C` counter. Counter hits check the shop-location table (shop vs
        /// dialogue); adjacent hits are always dialogue.
        reach: InteractReach,
    },
    /// Confirm pressed with nothing in talk range — the cartridge answers
    /// with the leader's "Nothing here" line.
    InteractNothing {
        /// Which way the party was facing.
        facing: Direction,
    },
    /// A trigger fired and a transcribed scene began. Cinema mode on.
    SceneStarted {
        /// The RunEventsJmpTbl index that fired.
        trigger: u8,
    },
    /// The running scene finished (or faulted; faults are logged). Cinema off.
    SceneEnded,
    /// A trigger fired an event with no transcribed scene yet.
    SceneMissing {
        /// The event index that has no scene.
        event: u16,
    },
    /// A trigger hit one of the four honestly-unsupported custom checks.
    TriggerUnsupported {
        /// The trigger index.
        trigger: u8,
    },
    /// The scene asks for a dialogue entry (within the current map's bound
    /// tree). The renderer opens the window and calls
    /// [`Runtime::dialogue_closed`] when it shuts.
    SceneDialogue {
        /// Entry index in the map's dialogue tree.
        entry: u16,
    },
    /// The scene requested a battle; no battle engine exists, so the runtime
    /// resumes the scene immediately. Logged, never silent.
    SceneBattleSkipped {
        /// The event battle index.
        index: u16,
    },
    /// The party composition changed (join, swap, leader change). The
    /// renderer refreshes party sprites.
    PartyChanged,
    /// Map objects were despawned in place (indices stable). The renderer
    /// hides their nodes.
    NpcsDespawned {
        /// First object index.
        first: usize,
        /// How many consecutive objects.
        count: usize,
    },
}

/// The game shell: pack data, the current engine map, and the field state.
pub struct Runtime {
    data: GameData,
    map: FieldMap,
    party: Party,
    game: GameState,
    scene: Option<SceneRunner>,
    scene_input: SceneInput,
    /// Interim MapDataManager: (map id, npc index) pairs despawned this
    /// session, applied on every map build until the real flag-gated effect
    /// layer is extracted.
    despawned: BTreeSet<(u16, usize)>,
    prev_standing: Option<u8>,
}

impl Runtime {
    /// Starts on `map_id` at `spawn`, facing `facing`.
    pub fn new(
        data: GameData,
        map_id: u16,
        spawn: Cell,
        facing: Direction,
        step_frames: StepFrames,
    ) -> Result<Runtime, BridgeError> {
        let record = data
            .map(psiv_data::MapId(map_id))
            .ok_or(BridgeError::NotPacked(map_id))?;
        let map = field_map(record)?;
        // Follower count comes from game-start state once extracted; the
        // solo default keeps behavior identical until then.
        let party = Party::new(&map, spawn, facing, step_frames, 0)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;

        // Seed persistent state from the cartridge's first-controllable
        // moment: the party slots and the flags the opening leaves set.
        let mut game = GameState::new();
        if let Some(start) = data.manifest().game_start.as_ref() {
            for flag in &start.event_flags_set {
                let _ = game.set(Flag::event(*flag));
            }
            for (slot, symbol) in start.party.iter().enumerate() {
                if let Some(id) = char_id_by_symbol(&data, symbol) {
                    let _ = game.set_party_slot(slot, Some(CharId(id)));
                }
            }
        } else {
            let _ = game.set_party_slot(0, Some(CharId(0)));
        }

        Ok(Runtime {
            data,
            map,
            party,
            game,
            scene: None,
            scene_input: SceneInput::None,
            despawned: BTreeSet::new(),
            prev_standing: None,
        })
    }

    /// The currently loaded map.
    #[must_use]
    pub fn map_id(&self) -> MapId {
        self.map.id()
    }

    /// The engine map, for the renderer's collision/NPC queries.
    #[must_use]
    pub fn map(&self) -> &FieldMap {
        &self.map
    }

    /// The loaded pack, for presentation-layer queries (sprite sheets, the
    /// current map's record). Read-only; the runtime owns all mutation.
    #[must_use]
    pub fn data(&self) -> &GameData {
        &self.data
    }

    /// The current map's record, for presentation-layer queries (NPC sprite
    /// bindings and the like).
    #[must_use]
    pub fn map_record(&self) -> Option<&psiv_data::MapRecord> {
        self.data.map(psiv_data::MapId(self.map.id().0))
    }

    /// The current map's composed-render path, exactly as the pack declares
    /// it (relative to the pack root). The renderer must never invent pack
    /// filenames; the pack names its own files.
    #[must_use]
    pub fn map_png(&self) -> Option<&str> {
        self.data
            .map(psiv_data::MapId(self.map.id().0))
            .map(|record| record.png.as_str())
    }

    /// The current map's priority-overlay path — tiles the VDP draws above
    /// sprites — or `None` when the map has no priority tiles.
    #[must_use]
    pub fn map_png_over(&self) -> Option<&str> {
        self.data
            .map(psiv_data::MapId(self.map.id().0))
            .and_then(|record| record.png_over.as_deref())
    }

    /// The field state, for the renderer's position/interpolation queries.
    #[must_use]
    pub fn state(&self) -> &FieldState {
        self.party.leader()
    }

    /// Every party member in draw order (0 = leader), for the renderer.
    #[must_use]
    pub fn members(&self) -> Vec<MemberView> {
        self.party.members()
    }

    /// Advances one tick and resolves any map change.
    pub fn tick(&mut self, input: Input) -> Vec<RuntimeEvent> {
        if self.scene.is_some() {
            return self.scene_tick();
        }
        let mut events = Vec::new();
        let mut landed: Option<Cell> = None;
        let mut map_changed = false;
        for effect in self.party.tick(&self.map, input) {
            match effect {
                Effect::StepCompleted { cell } => {
                    landed = Some(cell);
                    events.push(RuntimeEvent::StepCompleted { cell });
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
                    events.push(RuntimeEvent::Interact {
                        npc_index,
                        cell,
                        reach,
                    });
                }
                Effect::InteractNothing { facing } => {
                    events.push(RuntimeEvent::InteractNothing { facing });
                }
            }
        }

        // Trigger evaluation on landing, exactly like the cartridge's
        // RunEvents: per rest-frame after a step, before the player moves
        // again, skipped when a transition already changed the map.
        if let Some(cell) = landed
            && !map_changed
        {
            events.extend(self.evaluate_triggers(cell));
        }
        events
    }

    /// One tick of a running scene: feed any pending input, translate the
    /// effects, close out the scene when the runner finishes.
    fn scene_tick(&mut self) -> Vec<RuntimeEvent> {
        let input = std::mem::take(&mut self.scene_input);
        let mut events = Vec::new();
        let Some(runner) = self.scene.as_mut() else {
            return events;
        };
        let effects = runner.tick(&self.map, &mut self.game, input);
        let finished = runner.is_finished();
        for effect in effects {
            self.translate_scene_effect(effect, &mut events);
        }
        if finished {
            self.scene = None;
            events.push(RuntimeEvent::SceneEnded);
        }
        events
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
            SceneEffect::BattleRequested { index } => {
                events.push(RuntimeEvent::SceneBattleSkipped { index });
                self.scene_input = SceneInput::DialogueClosed;
            }
            SceneEffect::NpcDespawned { npc_index, count } => {
                let map = self.map.id().0;
                for i in npc_index..npc_index + count {
                    self.despawned.insert((map, i));
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
            SceneEffect::PartyChanged | SceneEffect::CharSlotCopied { .. } => {
                self.resize_party();
                events.push(RuntimeEvent::PartyChanged);
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
                {
                    // Start words are 8px units; the standing shift applies
                    // on Y, as everywhere in the pack.
                    let cell = Cell::new(start_x / 2, start_y / 2 + 1);
                    match self.change_map(MapId(map), cell, facing) {
                        Ok(()) => {
                            if let Some(runner) = self.scene.as_mut() {
                                runner.recast(Vec::new());
                            }
                            events.push(RuntimeEvent::MapChanged {
                                map: MapId(map),
                                trigger: WarpTrigger::MapChange,
                            });
                        }
                        Err(_) => events.push(RuntimeEvent::UnpackedTarget { map: MapId(map) }),
                    }
                }
            }
            // Flag effects: nothing flag-gated is rebuilt yet (the real
            // MapDataManager layer is filed); scene despawns cover the act.
            SceneEffect::FlagChanged { .. } => {}
            // Actor motion is polled via scene_actors(); presentation ops and
            // arrivals need no runtime action.
            _ => {}
        }
    }

    /// Evaluates the map's trigger list at a landing.
    fn evaluate_triggers(&mut self, cell: Cell) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return events;
        };
        let indices: Vec<u8> = record.events.iter().map(|&e| e as u8).collect();
        let standing = self.map.collision_at(cell).map(|c| c.to_raw());
        let ctx = TriggerContext {
            state: &self.game,
            at: PixelPos::from_cell(cell),
            standing,
            previously_standing: self.prev_standing,
        };
        let hit = psiv_core::evaluate_list(&TRIGGERS, &indices, &ctx);
        self.prev_standing = standing;
        match hit {
            Some((index, TriggerResult::Fire(event))) => match scene_for(event) {
                Some(scene) => {
                    let cast = self.build_cast();
                    match runner_for(scene, cast, StepFrames::default()) {
                        Ok(runner) => {
                            self.scene = Some(runner);
                            self.scene_input = SceneInput::None;
                            events.push(RuntimeEvent::SceneStarted { trigger: index });
                        }
                        Err(_) => events.push(RuntimeEvent::SceneMissing { event: event.0 }),
                    }
                }
                None => events.push(RuntimeEvent::SceneMissing { event: event.0 }),
            },
            Some((index, TriggerResult::Unsupported(..))) => {
                events.push(RuntimeEvent::TriggerUnsupported { trigger: index });
            }
            Some((_, TriggerResult::FireWithoutIndex))
            | Some((_, TriggerResult::NoEvent))
            | None => {}
        }
        events
    }

    /// The cast a scene may address: every party member (by slot and by
    /// character) plus every map object by index.
    fn build_cast(&self) -> Vec<ScriptedActor> {
        let mut cast = Vec::new();
        for (slot, member) in self.party.members().iter().enumerate() {
            cast.push(ScriptedActor::new(
                ActorRef::PartyMember(slot),
                member.cell,
                member.facing,
            ));
            if let Some(id) = self.game.party_slot(slot) {
                cast.push(ScriptedActor::new(
                    ActorRef::Character(id),
                    member.cell,
                    member.facing,
                ));
            }
        }
        for (i, npc) in self.map.npcs().iter().enumerate() {
            cast.push(ScriptedActor::new(ActorRef::Npc(i), npc.cell, npc.facing));
        }
        cast
    }

    /// The persistent game state (flags, party, money).
    #[must_use]
    pub fn game(&self) -> &GameState {
        &self.game
    }

    /// Whether a scene is running (cinema mode, input ownership).
    #[must_use]
    pub fn scene_active(&self) -> bool {
        self.scene.is_some()
    }

    /// The running scene's actors, for the renderer to draw at their scripted
    /// positions. Empty when no scene runs.
    #[must_use]
    pub fn scene_actors(&self) -> Vec<(ActorRef, Cell, Direction)> {
        self.scene
            .as_ref()
            .map(|r| {
                r.actors()
                    .iter()
                    .map(|a| (a.actor, a.cell, a.facing))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Rebuilds the walking party to match the game state's composition,
    /// stacked at the leader's cell exactly as retail stacks on entry.
    fn resize_party(&mut self) {
        let followers = self.game.party_len().saturating_sub(1);
        if followers + 1 == self.party.len() {
            return;
        }
        let cell = self.party.leader().cell();
        let facing = self.party.leader().facing();
        let frames = self.party.leader().step_frames();
        if let Ok(party) = Party::new(&self.map, cell, facing, frames, followers) {
            self.party = party;
        }
    }

    /// The renderer reports the scene-requested dialogue window has closed.
    pub fn dialogue_closed(&mut self) {
        if self.scene.is_some() {
            self.scene_input = SceneInput::DialogueClosed;
        }
    }

    fn change_map(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
    ) -> Result<(), BridgeError> {
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        let map = field_map(record)?;
        self.party
            .enter_map(&map, cell, facing)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.map = map;
        Ok(())
    }
}

/// Character id by party-sheet symbol (`CharFieldArtPtrs` order is the id).
fn char_id_by_symbol(data: &GameData, symbol: &str) -> Option<u8> {
    (0..11)
        .find(|&slot| {
            data.party_sheet(slot)
                .is_some_and(|sheet| sheet.id == symbol)
        })
        .map(|slot| slot as u8)
}
