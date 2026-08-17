//! The bridge between the pack schema (`psiv-data`) and the field engine
//! (`psiv-core`), plus the game shell that owns map changes.
//!
//! `psiv-core` knows no schema and `psiv-data` knows no rules; this crate is
//! the only place the two meet. `psiv-godot` drives a [`Runtime`] and never
//! touches either lower layer directly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::float_arithmetic)]

use std::collections::BTreeSet;

mod battle_interim;
mod boss_battles;
mod bridge;
mod camp;
mod effects;
mod encounters;
mod events;
mod save;
mod scene_runtime;
mod shop;
pub use bridge::{BridgeError, field_map, field_map_patched};
pub use camp::{CampCharacter, CampEquipResult, CampItem, CampState, CampUseResult};
pub use effects::{EffectOutcome, evaluate as evaluate_map_effects};
pub use encounters::{
    EncounterClock, EncounterTable, FOOT_MASK, GRACE_STEPS, GROUP_MASK, battle_data,
    formation_record,
};
pub use events::RuntimeEvent;
pub use save::RuntimeSaveError;
pub use shop::{InnResult, ShopBuyResult, ShopSellResult};

use bridge::{build_wander, char_id_by_symbol};

use psiv_core::battle::{Battle, BattleEvent, Lcg41, Rng2, Rolls, RoundOrders};
use psiv_core::{
    ActorRef, Camera, CameraBounds, CameraEdges, Cell, CharId, Direction, Driver, Effect,
    EventIndex, FieldMap, FieldState, Flag, GameState, Input, MapId, MemberView, Npc, ONE_PIXEL,
    Party, PixelPos, SceneInput, SceneRunner, ScriptedActor, StepFrames, TRIGGERS, Topology,
    TriggerContext, TriggerResult, WanderSet, Wanderer, runner_for, scene_for,
};
use psiv_data::GameData;

/// The game shell: pack data, the current engine map, and the field state.
pub struct Runtime {
    data: GameData,
    map: FieldMap,
    party: Party,
    game: GameState,
    /// Retail's saved world selector. The current runtime has one namespace,
    /// but a load/save cycle must not erase the word.
    saved_world_index: u16,
    /// Retail's saved secondary-map selector, retained across the same seam.
    saved_map_index_2: u16,
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
    /// The event battle that owns the current scene block, if any.
    scene_battle: Option<u16>,
    /// An interim Igglanova loss asks the scene to be installed again after
    /// the finished runner consumes its BattleFinished input.
    scene_retry: Option<u16>,
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
    camp: camp::CampCatalog,
    /// Event battle index -> boss formation. Boss records have no normal id.
    boss_formations: std::collections::BTreeMap<u16, psiv_core::battle::FormationRecord>,
}

/// Mirrors `Interaction_ChkMapAreas`'s already-processed gate. A story area
/// with flag zero is explicitly unconditional; nonzero selectors are clear
/// until the corresponding handler records them.
fn interaction_flag_clear(game: &GameState, area: &psiv_data::InteractionArea) -> bool {
    match area.flag_type.id {
        0 if area.flag == 0 => true,
        0 => game.is_clear(Flag::event(u16::from(area.flag))),
        1 => game.is_clear(Flag::chest(u16::from(area.flag))),
        2 => game.is_clear(Flag::temp(u16::from(area.flag))),
        _ => false,
    }
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

        // MapDataManager's walk is stateful: flag_clear writes land mid-walk
        // so later gates see them. Construction is shared with loaded saves;
        // the loaded snapshot must reach this point before map effects run.
        save::construct_runtime(
            data,
            save::RuntimePlacement::new(map_id, spawn, facing, step_frames, 0, 0),
            game,
            0,
        )
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
            let id = CharId(character.character_id);
            if self.game.roster().get(id).is_some() {
                continue;
            }
            let record = encounters::character_record(character, &files.enemies.properties)?;
            let member = psiv_core::battle::PartyMember::seat(&record, &data)
                .map_err(|e| BridgeError::Rejected(e.to_string()))?;
            self.game
                .roster_mut()
                .seat(id, member.stats)
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
            camp: camp::catalog(files),
            boss_formations: boss_battles::boss_formation_records(files)?,
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
    pub fn finish_battle_absorbing(&mut self, each: u16) -> Vec<BattleEvent> {
        let mut timeline = Vec::new();
        if let Some(battle) = self.battle.take() {
            // The cartridge's results order, load-bearing: absorb the
            // records whole, pay both award passes, then level everyone the
            // pay reached — levelling first levels nobody, and levelling
            // battle's copies levels stale numbers.
            let party = battle.into_party();
            self.game.roster_mut().absorb(&party);
            let (paid_party, paid_absent) = self.game.award_experience(each);
            if let Some(set) = self.battles.as_ref() {
                for id in paid_party.iter().chain(&paid_absent) {
                    if let Some(stats) = self.game.roster_mut().get_mut(*id)
                        && let Ok(Some(event)) = psiv_core::battle::level_up(id.0, stats, &set.data)
                    {
                        timeline.push(event);
                    }
                }
            }
        }
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
        timeline
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
        let mut interaction_started = false;
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
                    if self.start_interaction_event(&mut events) {
                        interaction_started = true;
                    } else {
                        events.push(RuntimeEvent::Interact {
                            npc_index,
                            cell,
                            reach,
                        });
                    }
                }
                Effect::InteractNothing { facing } => {
                    if self.start_interaction_event(&mut events) {
                        interaction_started = true;
                    } else {
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
        self.scene_battle = None;
        Ok(events)
    }

    /// Resolves one battle round with the party's orders.
    ///
    /// After an `Ended` event appears in the timeline the shell calls
    /// [`Runtime::finish_battle_absorbing`] to return the records to the
    /// field roster (and to run the victory award pass).
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
            Some((index, TriggerResult::Fire(event))) => {
                if self.install_scene(event) {
                    events.push(RuntimeEvent::SceneStarted { trigger: index });
                } else {
                    events.push(RuntimeEvent::SceneMissing { event: event.0 });
                }
            }
            Some((index, TriggerResult::Unsupported(..))) => {
                events.push(RuntimeEvent::TriggerUnsupported { trigger: index });
            }
            Some((_, TriggerResult::FireWithoutIndex))
            | Some((_, TriggerResult::NoEvent))
            | None => {}
        }
        events
    }

    /// Runs the type-2 map interaction area probe on a consumed confirm.
    ///
    /// The pack has already resolved the record's 8-pixel source and
    /// `XYRangeJmpTbl` selector into collision cells. Other interaction
    /// handlers remain deliberately outside this path: their parameters are
    /// dialogue/chest-specific, not event indexes.
    fn start_interaction_event(&mut self, events: &mut Vec<RuntimeEvent>) -> bool {
        let leader = self.party.leader();
        let Some(adjacent) = leader.cell().neighbor(leader.facing()) else {
            return false;
        };
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return false;
        };
        let Some((area_index, parameter, event)) = record
            .interaction_areas
            .iter()
            .find(|area| {
                area.interaction_type == 2
                    && interaction_flag_clear(&self.game, area)
                    && area.rect.is_some_and(|rect| {
                        rect.contains(psiv_data::CellPos::new(
                            u32::from(adjacent.x),
                            u32::from(adjacent.y),
                        ))
                    })
            })
            .map(|area| (area.index, area.parameter, area.event_index))
        else {
            return false;
        };
        let Some(event) = event else {
            events.push(RuntimeEvent::SceneMissing {
                event: u16::from(parameter),
            });
            return true;
        };
        if self.install_scene(EventIndex(event)) {
            events.push(RuntimeEvent::SceneStartedFromInteraction {
                area: area_index,
                event,
            });
        } else {
            events.push(RuntimeEvent::SceneMissing { event });
        }
        true
    }

    /// Installs a transcribed scene and parks the field until its runner
    /// produces a completion input. The renderer is notified separately by
    /// the caller because the source of a scene matters to its diagnostics.
    fn install_scene(&mut self, event: EventIndex) -> bool {
        if self.scene.is_some() {
            return false;
        }
        let Some(scene) = scene_for(event) else {
            return false;
        };
        let cast = self.build_cast();
        let Ok(runner) = runner_for(scene, cast, StepFrames::default()) else {
            return false;
        };
        self.scene = Some(runner);
        self.scene_input = SceneInput::None;
        self.scene_warmup = true;
        true
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
        self.install_scene(EventIndex(event))
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
        // MapDataManager runs inside map load, and its walk is STATEFUL:
        // flag_clear writes (from the pack's decoded data) land mid-walk so
        // later entries' gates see them — the Xanafalgue-respawn mechanism
        // tape 18 measured. Cleared flags resurrect gated objects on OTHER
        // maps at their next build; this map's own build below already sees
        // the post-clear state.
        let effects = effects::evaluate(record, &mut self.game);
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
