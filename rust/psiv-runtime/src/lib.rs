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

mod effects;
mod encounters;
pub use effects::{EffectOutcome, evaluate as evaluate_map_effects};
pub use encounters::{
    EncounterClock, EncounterTable, FOOT_MASK, GRACE_STEPS, GROUP_MASK, battle_data,
    formation_record,
};

use psiv_core::battle::{Battle, BattleEvent, Lcg41, Rng2, Rolls, RoundOrders};
use psiv_core::{
    ActorRef, Camera, CameraBounds, CameraEdges, Cell, CharId, CollisionGrid, Direction, Driver,
    Effect, FieldMap, FieldState, Flag, GameState, Input, InteractReach, MapId, MemberView, Npc,
    NpcId, ONE_PIXEL, Party, PixelPos, SceneEffect, SceneInput, SceneOp, SceneRunner,
    ScriptedActor, StepFrames, TRIGGERS, Topology, TriggerContext, TriggerResult, WanderKind,
    WanderSet, Wanderer, Warp, WarpTrigger, runner_for, scene_for,
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
    field_map_patched(record, None)
}

/// [`field_map`], with a map-effects outcome applied during construction —
/// the cartridge's own order: `MapDataManager` runs inside map load, so a
/// patched map never exists unpatched.
pub fn field_map_patched(
    record: &MapRecord,
    outcome: Option<&EffectOutcome>,
) -> Result<FieldMap, BridgeError> {
    if let Some(out) = outcome
        && !out.unknown_banks.is_empty()
    {
        return Err(BridgeError::Rejected(format!(
            "map {}: effect schema drift: {:?}",
            record.label(),
            out.unknown_banks
        )));
    }
    // The grid: the variant's collision when a layout_replace is active,
    // the record's otherwise; then any resolved layout_write cells.
    let (width_u32, height_u32, mut cells): (u32, u32, Vec<u8>) = match outcome
        .and_then(|o| o.variant)
    {
        Some(index) => {
            let variant = record.layout_variants.get(index).ok_or_else(|| {
                BridgeError::Rejected(format!("map {}: variant {index} missing", record.label()))
            })?;
            let flat: Vec<u8> = variant.collision.rows.iter().flatten().copied().collect();
            (
                variant.collision.width_cells,
                variant.collision.height_cells,
                flat,
            )
        }
        None => {
            let grid = &record.collision.grid;
            (
                grid.width(),
                grid.height(),
                grid.cells().iter().map(|c| c.code()).collect(),
            )
        }
    };
    if let Some(out) = outcome {
        for &(x, y, collision) in &out.cell_patches {
            if x < width_u32 && y < height_u32 {
                cells[(y * width_u32 + x) as usize] = collision;
            } else {
                return Err(BridgeError::OutOfRange(format!(
                    "map {}: layout_write cell ({x},{y})",
                    record.label()
                )));
            }
        }
    }
    let width = u16::try_from(width_u32)
        .map_err(|_| BridgeError::OutOfRange(format!("width {width_u32}")))?;
    let height = u16::try_from(height_u32)
        .map_err(|_| BridgeError::OutOfRange(format!("height {height_u32}")))?;
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
    for (index, npc) in record.npcs.iter().enumerate() {
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
        // Effects apply at construction: a rewritten object carries its new
        // id from the first tick, a despawned one is born inactive.
        let object_id = outcome
            .and_then(|o| {
                o.rewrites
                    .iter()
                    .find(|(i, _)| *i == index)
                    .map(|(_, id)| *id)
            })
            .unwrap_or(npc.object_id);
        let active = outcome.is_none_or(|o| !o.despawns.contains(&index));
        npcs.push(
            Npc::with_offset(NpcId(object_id), cell, offset, facing)
                .with_interactable(npc.interactable)
                .with_active(active),
        );
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
    /// A random encounter fired on this landing. The shell seats the party
    /// and calls [`Runtime::start_battle`] with this formation.
    EncounterRolled {
        /// The formation id the encounter tables picked.
        formation: u16,
    },
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
    /// The map's wandering townsfolk (NPCType2/3), rebuilt per map load.
    wander: WanderSet,
    /// The one shared seed, ticked once per frame like the cartridge's
    /// vblank call; wander decisions draw from the same stream, as retail's
    /// FieldObj_GetRandomMove shares UpdateRNGSeed with encounter rolls.
    rng: Lcg41,
    /// The cartridge suspends field-object updates while a window is up
    /// (proven by the oracle's RNG census: 2 calls/frame in field, 1 in a
    /// menu). The renderer sets this while dialogue is open.
    field_suspended: bool,
    /// `Main_Frame_Count`: ticks since the runtime started, wrapped to the
    /// word the cartridge keeps. `Rng2` mixes it into every battle roll.
    frames: u16,
    /// The field camera. Not presentation: it decides which objects are on
    /// screen, and `FieldObj_OnScreenTest` freezes the ones that are not, so
    /// its position reaches the shared RNG stream through the rolls those
    /// objects do or do not draw.
    camera: Camera,
    /// Set when a scene has just been created and has not run an op yet.
    ///
    /// A trigger does not enter the event mode itself — it sets
    /// `Game_Mode_Index`, and the mode dispatcher picks the new mode up on the
    /// *following* frame, which it spends entering. So a scene's first op runs
    /// two frames after the landing that fired it, not one. Tape 02 measures
    /// exactly that: the trigger fires at 7874 and Alys turns at 7876.
    scene_warmup: bool,
    /// `offscreen_flag` (`$12`) per object, refreshed every field frame.
    ///
    /// `FieldObj_OnScreenTest` writes it at the top of every object's routine
    /// from the sprite position the *previous* frame computed, so it is a frame
    /// old by construction and is stored rather than recomputed on demand.
    offscreen: Vec<bool>,
    /// Battle data + encounter tables, present once [`Runtime::enable_battles`]
    /// has run. `None` means encounters never roll — a pre-battle pack.
    battles: Option<BattleSet>,
    /// A battle in progress. Field input is ignored while this is `Some`.
    battle: Option<Battle>,
    /// The current map's evaluated MapDataManager outcome — dialogue
    /// overrides, the active layout variant, and the surfaced gaps.
    effects: EffectOutcome,
}

/// Everything encounters need, converted from the pack once.
struct BattleSet {
    data: psiv_core::battle::BattleData,
    table: EncounterTable,
    clock: EncounterClock,
    /// Display names by character id, for battle timelines.
    names: std::collections::BTreeMap<u8, String>,
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

        // Seed persistent state FIRST: map effects are evaluated against the
        // flag state at load, exactly the cartridge's MapDataManager order.
        let mut game = GameState::new();
        if let Some(start) = data.manifest().game_start.as_ref() {
            for flag in &start.event_flags_set {
                let _ = game.set(Flag::event(*flag));
            }
            // The pack emits extended ids already in the combined $100..
            // space Flag::event models, so no offset is applied here.
            for flag in &start.extended_event_flags_set {
                let _ = game.set(Flag::event(*flag));
            }
            for flag in &start.town_flags_set {
                let _ = game.set(Flag::town(*flag));
            }
            for flag in &start.chest_flags_set {
                let _ = game.set(Flag::chest(*flag));
            }
            for (slot, symbol) in start.party.iter().enumerate() {
                if let Some(id) = char_id_by_symbol(&data, symbol) {
                    let _ = game.set_party_slot(slot, Some(CharId(id)));
                }
            }
        } else {
            let _ = game.set_party_slot(0, Some(CharId(0)));
        }

        // The destination map's flag-clearing entries run first, then the
        // patch entries evaluate — the load order change_map() documents.
        let entries: Vec<u8> = record.map_effects.iter().map(|e| e.entry as u8).collect();
        let _cleared = psiv_core::apply_map_load(&mut game, &entries);
        let effects = effects::evaluate(record, &game);
        let map = field_map_patched(record, Some(&effects))?;
        // Follower count comes from game-start state once extracted; the
        // solo default keeps behavior identical until then.
        let party = Party::new(&map, spawn, facing, step_frames, 0)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;

        let wander = build_wander(&map, record)?;
        let camera = Camera::placed_on(driver_of(party.leader()), bounds_of(&map));
        Ok(Runtime {
            data,
            map,
            party,
            game,
            scene: None,
            scene_input: SceneInput::None,
            despawned: BTreeSet::new(),
            prev_standing: None,
            wander,
            rng: Lcg41::default(),
            field_suspended: false,
            frames: 0,
            camera,
            scene_warmup: false,
            offscreen: Vec::new(),
            battles: None,
            battle: None,
            effects,
        })
    }

    /// Converts the pack's battle files and arms random encounters.
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when a record does not fit the engine.
    pub fn enable_battles(&mut self, files: &psiv_data::BattleFiles) -> Result<(), BridgeError> {
        let data = battle_data(files)?;
        // Seat all eleven characters, exactly as InitializeCharStats does
        // whether or not they are in the party. The roster refuses a second
        // constructor path by design, so this goes through the same
        // PartyMember::seat every battle uses.
        for character in &files.characters.characters {
            let record = encounters::character_record(character, &files.enemies.properties)?;
            let member = psiv_core::battle::PartyMember::seat(&record, &data)
                .map_err(|e| BridgeError::Rejected(e.to_string()))?;
            self.game
                .roster_mut()
                .seat(CharId(member.character), member.stats)
                .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        }
        self.battles = Some(BattleSet {
            data,
            table: EncounterTable::from_files(files)?,
            clock: EncounterClock::new(),
            names: files
                .characters
                .characters
                .iter()
                .map(|c| {
                    (
                        c.character_id,
                        c.display_name.clone().unwrap_or_else(|| c.symbol.clone()),
                    )
                })
                .collect(),
        });
        Ok(())
    }

    /// The current party as battle members, drawn from the roster — the
    /// records battles read and write in place, per the cartridge's own
    /// model. Empty if battles are not enabled or the roster is unseated.
    #[must_use]
    pub fn battle_party(&self) -> Vec<psiv_core::battle::PartyMember> {
        let Some(set) = self.battles.as_ref() else {
            return Vec::new();
        };
        self.game
            .party_members()
            .into_iter()
            .filter_map(|id| {
                let stats = self.game.roster().get(id)?.clone();
                Some(psiv_core::battle::PartyMember {
                    character: id.0,
                    name: set.names.get(&id.0).cloned().unwrap_or_default(),
                    stats,
                })
            })
            .collect()
    }

    /// Ends a battle by absorbing the party records back into the roster and
    /// running both award passes — the full cartridge epilogue. The caller
    /// passes the per-member award (the split the battle computed).
    pub fn finish_battle_absorbing(&mut self, each: u16) {
        if let Some(battle) = self.battle.take() {
            let party = battle.into_party();
            self.game.roster_mut().absorb(&party);
            let _ = self.game.award_experience(each);
        }
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
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
    /// filenames; the pack names its own files. When a `layout_replace` is
    /// active this is the variant's render.
    #[must_use]
    pub fn map_png(&self) -> Option<&str> {
        let record = self.data.map(psiv_data::MapId(self.map.id().0))?;
        match self.effects.variant {
            Some(index) => record.layout_variants.get(index).map(|v| v.png.as_str()),
            None => Some(record.png.as_str()),
        }
    }

    /// The current map's priority-overlay path — tiles the VDP draws above
    /// sprites — or `None` when the map has no priority tiles. Variant-aware
    /// like [`Runtime::map_png`].
    #[must_use]
    pub fn map_png_over(&self) -> Option<&str> {
        let record = self.data.map(psiv_data::MapId(self.map.id().0))?;
        match self.effects.variant {
            Some(index) => record
                .layout_variants
                .get(index)
                .and_then(|v| v.png_over.as_deref()),
            None => record.png_over.as_deref(),
        }
    }

    /// An object's live dialogue id: the map-effect override when one is
    /// active, the record's own binding otherwise. The renderer's talk path
    /// must use this, not the record directly — `object_dialogue` patches
    /// are how clinics and story rooms change what a person says.
    #[must_use]
    pub fn npc_dialogue_id(&self, index: usize) -> Option<u16> {
        if let Some(id) = self.effects.dialogue_overrides.get(&index) {
            return Some(*id);
        }
        self.data
            .map(psiv_data::MapId(self.map.id().0))
            .and_then(|record| record.npcs.get(index))
            .map(|npc| npc.dialogue_id)
    }

    /// The current map's evaluated effect outcome, for the renderer's gap
    /// logging (unresolved layout writes, undecoded entries).
    #[must_use]
    pub fn map_effects(&self) -> &EffectOutcome {
        &self.effects
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
        // `Main_Frame_Count` first, then the vblank tick: the cartridge
        // stirs the one seed every frame in every game mode (VInt handler,
        // ps4.asm:617) — scenes included.
        self.frames = self.frames.wrapping_add(1);
        self.rng.step();
        // A battle owns the frame: the field is parked exactly as
        // GameMode_Battle parks it, and the shell drives rounds through
        // [`Runtime::battle_round`].
        if self.battle.is_some() {
            return Vec::new();
        }
        if self.scene.is_some() {
            // The frame the mode dispatcher spends entering event mode.
            if self.scene_warmup {
                self.scene_warmup = false;
                return Vec::new();
            }
            return self.scene_tick();
        }
        // The field-mode tick: GameMode_Field opens with an unconditional
        // UpdateRNGSeed before dispatching (ps4.asm:107638). It vanishes
        // while a window is up because window loops never return to the
        // mode dispatcher — hence the suspension gate, which also parks the
        // wander draws further down. docs/NPC_WANDER.md, "per-frame tick
        // structure".
        if !self.field_suspended {
            self.rng.step();
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

        // RunRandomBattles ($05784E): only on a landing that neither changed
        // the map nor started a scene, on a map whose binding rolls at all,
        // and never standing on/next to transition tiles. Ten free steps,
        // then seed-word & $1F == 0 fires — one extra LCG draw per rolling
        // step, exactly the cartridge's extra call.
        if let Some(cell) = landed
            && !map_changed
            && self.scene.is_none()
            && let Some(set) = self.battles.as_mut()
            && set.table.enabled(self.map.id().0)
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
        // Wander draws are conditional consumers of the same stream — an
        // idle NPC whose countdown expires rolls once; most frames none do.
        // This runs before the camera's own tick because the cartridge's
        // visibility test reads sprite positions written the frame before.
        if !self.field_suspended {
            self.tick_wander();
        }
        // `UpdateCamera*PosFG` folds in last frame's scroll and `FieldObj_*`
        // latches this frame's, both against the party's post-movement
        // position.
        self.camera.tick(driver_of(self.party.leader()));
        events
    }

    /// One frame of NPC wander: visibility from the retail camera box, the
    /// party's occupied cells as obstacles, decisions from the shared seed.
    /// Refreshes `offscreen_flag` for every object.
    ///
    /// Runs before anything that consumes it, against the camera as it stood at
    /// the top of the frame — `FieldObj_OnScreenTest` reads the sprite position
    /// the previous frame's `FieldObj_CalcSpritePos` wrote, so the gate is one
    /// frame behind the camera by construction, not by approximation.
    fn update_visibility(&mut self) {
        let camera = self.camera;
        self.offscreen.clear();
        self.offscreen
            .extend(self.map.npcs().iter().enumerate().map(|(index, npc)| {
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
                let (x, y) = object_position(npc, wanderer);
                !camera.sees(x, y)
            }));
    }

    /// Whether object `index` was off screen this frame, and so was not updated.
    #[must_use]
    pub fn object_offscreen(&self, index: usize) -> bool {
        self.offscreen.get(index).copied().unwrap_or(true)
    }

    fn tick_wander(&mut self) {
        if self.wander.is_empty() {
            return;
        }
        let party_cells: Vec<Cell> = self.party.members().iter().map(|m| m.cell).collect();
        let offscreen = &self.offscreen;
        self.wander
            .tick(&mut self.map, &mut self.rng, &party_cells, |i| {
                !offscreen.get(i).copied().unwrap_or(true)
            });
    }

    /// The camera bounds this map imposes.
    /// The leader as the camera reads it: 16.16 position and this frame's
    /// velocity.
    ///
    /// The velocity is the cartridge's `y_step_constant` — a whole cell divided
    /// by the step's frame count, which is 2 px/frame at the default eight
    /// frames per cell — and it is zero at rest, which is what stops the camera
    /// dead rather than letting it drift.
    /// The field camera, for the renderer's authentic 320x224 viewport.
    #[must_use]
    pub const fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Parks the camera at an absolute position.
    ///
    /// Scenes do this (`SceneOp::SetCameraPos`), and so does a replay whose
    /// alignment frame inherits a camera the engine could not have produced,
    /// the opening scene having placed it.
    pub fn set_camera(&mut self, x: i32, y: i32) {
        self.camera.set_position(x, y);
    }

    /// The map's wanderers, for the renderer's per-frame positions.
    #[must_use]
    pub fn wanderers(&self) -> &[Wanderer] {
        self.wander.wanderers()
    }

    /// Whether a battle currently owns the frame.
    #[must_use]
    pub fn battle_active(&self) -> bool {
        self.battle.is_some()
    }

    /// Starts a battle against `formation`, seating `party`.
    ///
    /// The party's stats are the caller's until the pack carries
    /// `battle/characters.json` + `battle/equipment.json`; then the runtime
    /// seats its own party from game state and this takes only the formation.
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when battles are not enabled, the formation
    /// id is unknown, or the engine refuses the setup.
    pub fn start_battle(
        &mut self,
        formation: u16,
        party: Vec<psiv_core::battle::PartyMember>,
    ) -> Result<Vec<BattleEvent>, BridgeError> {
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("battles not enabled".into()))?;
        let record = set
            .table
            .formation(formation)
            .ok_or_else(|| BridgeError::Rejected(format!("unknown formation {formation}")))?;
        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
        let (battle, events) = Battle::start(record, party, &set.data, false, &mut rng2)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.battle = Some(battle);
        Ok(events)
    }

    /// Resolves one battle round with the party's orders.
    ///
    /// After an `Ended` event appears in the timeline the shell calls
    /// [`Runtime::finish_battle`] to return to the field.
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when no battle is active or a data lookup
    /// fails mid-round.
    pub fn battle_round(&mut self, orders: &RoundOrders) -> Result<Vec<BattleEvent>, BridgeError> {
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("battles not enabled".into()))?;
        let battle = self
            .battle
            .as_mut()
            .ok_or_else(|| BridgeError::Rejected("no battle in progress".into()))?;
        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
        battle
            .round(orders, &set.data, &mut rng2)
            .map_err(|e| BridgeError::Rejected(e.to_string()))
    }

    /// Ends the battle and re-arms the encounter grace period, as the
    /// cartridge resets `$FFFFECE4` to 10 after every fight.
    pub fn finish_battle(&mut self) {
        self.battle = None;
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
    }

    /// Restores one object's position, facing and wander state — the
    /// object-side twin of [`Runtime::set_rng_seed`].
    ///
    /// A replay picking a tape up mid-run inherits objects that have been
    /// wandering since the opening scene: off their spawn cells, leashes no
    /// longer centred, several mid-step. Without this they start from the
    /// pack's spawn state and every object column diverges on frame one.
    ///
    /// A mid-step object's `cell` is its **destination**, because the engine
    /// commits that the moment a step starts.
    ///
    /// # Errors
    ///
    /// Whatever [`FieldMap`] or [`psiv_core::WanderSet`] rejects.
    pub fn restore_object(
        &mut self,
        npc_index: usize,
        cell: Cell,
        facing: Direction,
        state: psiv_core::WanderState,
    ) -> Result<(), psiv_core::MapError> {
        self.map.set_npc_cell(npc_index, cell)?;
        self.map.set_npc_facing(npc_index, facing)?;
        // Objects that do not wander (Alys on the academy floor) have position
        // and facing but no wander state; a missing wanderer is not an error.
        let _ = self.wander.restore(npc_index, state);
        Ok(())
    }

    /// Turns an object to face a direction — the cartridge's default when
    /// spoken to (`$F3` exists to suppress it). Out-of-range indices are the
    /// renderer's bug to log, not the engine's to crash on.
    pub fn face_npc(&mut self, index: usize, facing: Direction) {
        let _ = self.map.set_npc_facing(index, facing);
    }

    /// Mirrors the cartridge's window-up suspension of field-object updates.
    /// The renderer sets this while a dialogue window is open.
    pub fn set_field_suspended(&mut self, suspended: bool) {
        self.field_suspended = suspended;
    }

    /// Seeds the shared RNG word — for replays that align to an oracle log.
    pub fn set_rng_seed(&mut self, seed: u32) {
        self.rng = Lcg41::new(seed);
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
            // A scripted facing is written straight into the field object slot
            // by the cartridge, so it has to reach the map's own record and not
            // only the scene's actor list. Two representations of one object's
            // facing is how the oracle's `oNN_facing` column diverged for the
            // whole length of a scene while everything else matched.
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(index),
                facing,
            } => {
                let _ = self.map.set_npc_facing(index, facing);
            }
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
                            self.scene_warmup = true;
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

    /// Starts an event's scene directly — the `$F6` dialogue path (the
    /// principal's briefing). Returns whether a transcribed scene began.
    pub fn start_event(&mut self, event: u16) -> bool {
        if self.scene.is_some() {
            return false;
        }
        let Some(scene) = scene_for(psiv_core::EventIndex(event)) else {
            return false;
        };
        let cast = self.build_cast();
        match runner_for(scene, cast, StepFrames::default()) {
            Ok(runner) => {
                self.scene = Some(runner);
                self.scene_input = SceneInput::None;
                self.scene_warmup = true;
                true
            }
            Err(_) => false,
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
        // MapDataManager runs inside map load: first the destination map's
        // flag-clearing entries mutate state (the basement un-looter's
        // mechanism), then the patch entries evaluate against the result.
        // The cartridge walks one list doing both interleaved; clear-first
        // is equivalent for every retail map (no map patches on a flag its
        // own later entry clears) and the simpler model wins until a
        // counterexample exists.
        let entries: Vec<u8> = record.map_effects.iter().map(|e| e.entry as u8).collect();
        // Cleared flags resurrect gated objects on OTHER maps at their next
        // build; this map's own build below already sees the post-clear
        // state, so nothing needs rebuilding here.
        let _cleared = psiv_core::apply_map_load(&mut self.game, &entries);
        let effects = effects::evaluate(record, &self.game);
        let mut map = field_map_patched(record, Some(&effects))?;
        self.effects = effects;
        // Re-apply this session's scene-driven despawns (the interim ledger
        // for despawns whose gating flag is not yet modelled).
        for &(m, i) in &self.despawned {
            if m == target.0 {
                let _ = map.set_npc_active(i, false);
            }
        }
        self.party
            .enter_map(&map, cell, facing)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.wander = build_wander(&map, record)?;
        // Map entry places the view rather than scrolling it in, so the camera
        // starts framed on the party wherever the warp dropped them.
        self.camera = Camera::placed_on(driver_of(self.party.leader()), bounds_of(&map));
        self.map = map;
        Ok(())
    }
}

/// The wandering objects on a map: exactly the pack NPCs whose behaviour
/// routine is NPCType2 or NPCType3 — the two types that share the cartridge's
/// random walker (`docs/NPC_WANDER.md`). Everything else stands still until
/// its own routine is transcribed.
/// The camera bounds a map imposes: its pixel extent, and whether the edges
/// clamp or wrap.
fn bounds_of(map: &FieldMap) -> CameraBounds {
    CameraBounds::from_cells(
        map.width(),
        map.height(),
        if map.topology() == Topology::Torus {
            CameraEdges::Wrapping
        } else {
            CameraEdges::Clamped
        },
    )
}

/// The leader's 16.16 position, as the camera reads it.
///
/// The camera derives the velocity it latches on from successive positions, so
/// there is deliberately no velocity here to get wrong.
fn driver_of(leader: &FieldState) -> Driver {
    let (ox, oy) = leader.render_offset_16ths();
    let at = PixelPos::from_cell(leader.cell());
    Driver {
        x: (at.x + ox) * ONE_PIXEL,
        y: (at.y + oy) * ONE_PIXEL,
    }
}

/// An object's 16.16 map position, including the part-cell travel of a step in
/// progress.
///
/// A stepping object's cell is already its destination — the engine commits it
/// at step start — so the pixel position interpolates from the origin the step
/// remembers, not from the cell.
fn object_position(npc: &Npc, wanderer: Option<&Wanderer>) -> (i32, i32) {
    let base = wanderer.and_then(Wanderer::step_origin).unwrap_or(npc.cell);
    let at = PixelPos::from_cell(base);
    let (tx, ty) = wanderer.map_or((0, 0), Wanderer::travelled_px);
    ((at.x + tx) * ONE_PIXEL, (at.y + ty) * ONE_PIXEL)
}

fn build_wander(map: &FieldMap, record: &MapRecord) -> Result<WanderSet, BridgeError> {
    let objects: Vec<(usize, WanderKind)> = record
        .npcs
        .iter()
        .enumerate()
        .filter_map(|(i, npc)| match npc.symbol.as_deref() {
            Some("NPCType2") => Some((i, WanderKind::Type2)),
            Some("NPCType3") => Some((i, WanderKind::Type3)),
            _ => None,
        })
        .collect();
    WanderSet::build(map, &objects).map_err(|e| BridgeError::Rejected(e.to_string()))
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
