//! The bridge between the pack schema (`psiv-data`) and the field engine
//! (`psiv-core`), plus the game shell that owns map changes.
//!
//! `psiv-core` knows no schema and `psiv-data` knows no rules; this crate is
//! the only place the two meet. `psiv-godot` drives a [`Runtime`] and never
//! touches either lower layer directly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::float_arithmetic)]

mod battle_interim;
mod boss_battles;
mod bridge;
mod camp;
mod effects;
mod encounters;
mod events;
#[cfg(test)]
mod field_entry_tests;
mod field_objects;
mod field_status;
mod loot;
#[cfg(test)]
mod loot_tests;
pub use field_status::FieldNotice;
pub use loot::{LootResult, LootState};
#[cfg(test)]
mod field_status_tests;
mod geometry;
mod new_game;
mod progression;
pub use progression::ProgressionRepair;
mod save;
mod scene_map;
#[cfg(test)]
mod scene_map_tests;
mod scene_runtime;
mod shop;
mod travel;
#[cfg(test)]
mod travel_tests;
pub use travel::{CampTravelMenu, ESCAPIPE, HINAS, RYUKA, TELEPIPE};
mod vehicle;
pub use bridge::{BridgeError, field_map, field_map_patched};
pub use camp::{
    CampAbility, CampAbilityKind, CampCharacter, CampEquipResult, CampItem, CampState,
    CampUseResult,
};
pub use effects::{EffectOutcome, evaluate as evaluate_map_effects};
pub use encounters::{
    EncounterClock, EncounterTable, FOOT_MASK, GRACE_STEPS, GROUP_MASK, VEHICLE_MASK, battle_data,
    formation_record,
};
pub use events::{BattleAnimationEvent, BattleSoundEvent, BattleTimeline, RuntimeEvent};
pub use save::RuntimeSaveError;
pub use shop::{InnResult, ShopBuyResult, ShopSellResult};

use bridge::{build_bespoke, build_wander, char_id_by_symbol, clear_bespoke_entry_flags};
use geometry::{camera_for_record, driver_of, refresh_camera_gates};

use psiv_core::battle::{Battle, BattleEvent, Lcg41, Rng2, Rolls, RoundOrders};
use psiv_core::{
    ActorRef, BespokeActor, BespokeSet, Camera, Cell, CharId, Direction, Effect, EventIndex,
    FieldMap, FieldState, Flag, GameState, Input, MapId, MemberView, PARTY_SLOTS, Party, PixelPos,
    SceneInput, SceneRunner, ScriptedActor, StepFrames, TRIGGERS, TriggerContext, TriggerResult,
    WanderSet, Wanderer, runner_for, scene_for,
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
    dungeon_exit_index: u8,
    pending_travel: Option<travel::PendingTravel>,
    loot: Option<loot::PendingLoot>,
    scene: Option<SceneRunner>,
    scene_input: SceneInput,
    scene_event: EventIndex,
    dialogue_answer: Option<bool>,
    scene_choice_pending: bool,
    /// Volatile end-of-game latch set by the retail ending after Start.
    /// This is presentation state, not save data.
    game_cleared: bool,
    game_over: bool,
    field_status: field_status::FieldStatus,
    prev_standing: Option<u8>,
    /// The map's transcribed random-wander objects, rebuilt per map load.
    wander: WanderSet,
    /// The map's post-wave-7 bespoke routines, rebuilt in the same map-object
    /// order as [`Runtime::wander`].
    bespoke: BespokeSet,
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
    /// Map load and event return re-enter FieldRoutine_Controls, whose
    /// RunEvents check precedes movement input even without a new step.
    scene_triggers_pending: bool,
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
    /// Battle return reloads map data without initializing live field objects.
    battle_field_refresh_pending: bool,
    /// The event battle that owns the current scene block, if any.
    scene_battle: Option<u16>,
    /// The current map's evaluated MapDataManager outcome — dialogue
    /// overrides, the active layout variant, and the surfaced gaps.
    effects: EffectOutcome,
    /// Mounted field state; `None` means `Vehicle_Index == 0`.
    vehicle: Option<psiv_core::VehicleState>,
    /// Retail's transient `Saved_Char_ID_Mem_1/_5` bridge. It is deliberately
    /// outside `GameState` and SRAM: scenes use it between dispatches, while
    /// the cartridge never exposes it as an ordinary save field.
    saved_party_slots: Option<[Option<CharId>; PARTY_SLOTS]>,
    /// An `Event_MoveCamera` pan in flight: target camera position in pixels
    /// and speed in px/frame. Ticked every frame until arrival, scene or not,
    /// so a scene that ends mid-pan still delivers the camera.
    camera_glide: Option<CameraGlide>,
    /// `SetFollowMode` bit 2: the scene has locked the camera (the walk
    /// off-screen in the opening). Cleared when a scene installs or ends.
    scene_camera_locked: bool,
}

/// See [`Runtime::scene_move_camera`].
#[derive(Debug, Clone, Copy)]
struct CameraGlide {
    target_x: i32,
    target_y: i32,
    speed: i32,
}

/// Everything encounters need, converted from the pack once.
struct BattleSet {
    data: psiv_core::battle::BattleData,
    table: EncounterTable,
    clock: EncounterClock,
    /// Display names by character id, for battle timelines.
    names: std::collections::BTreeMap<u8, String>,
    camp: camp::CampCatalog,
    loot: std::collections::BTreeMap<u8, loot::LootItem>,
    /// Event battle index -> boss formation. Boss records have no normal id.
    boss_formations: std::collections::BTreeMap<u16, psiv_core::battle::FormationRecord>,
    /// Enemy id -> retail attack-object presentation record.
    enemy_animations: std::collections::BTreeMap<u16, psiv_data::EnemyAnimation>,
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
        let mut game = data
            .new_game()
            .map(new_game::initial_state)
            .unwrap_or_default();
        // This constructor starts after the opening; discard its transient
        // Chaz/Alys cast before applying the first-control summary below.
        game.set_party([None; PARTY_SLOTS]);
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
            loot: loot::catalog(files),
            boss_formations: boss_battles::boss_formation_records(files)?,
            enemy_animations: files
                .enemy_animations
                .animations
                .iter()
                .map(|animation| (animation.enemy_id, animation.clone()))
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
        if let Some(index) = self.vehicle_index()
            && let Some(record) = self.game.vehicles().get(index.saturating_sub(1) as usize)
            && let Some(member) = psiv_core::battle_member(index, *record)
        {
            return vec![member];
        }
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

    /// Live combatants for command selection; includes current HP, TP and status.
    #[must_use]
    pub fn battle_roster(&self) -> Option<&psiv_core::battle::Roster> {
        self.battle.as_ref().map(Battle::roster)
    }

    /// Cartridge technique definitions for presentation of names and costs.
    pub fn battle_techniques(&self) -> impl Iterator<Item = &psiv_core::battle::Technique> {
        self.battles.iter().flat_map(|set| set.data.techniques())
    }

    /// Character skill names, targeting rules and availability for the menu.
    pub fn battle_skills(&self) -> impl Iterator<Item = &psiv_core::battle::Skill> {
        self.battles.iter().flat_map(|set| set.data.skills())
    }

    /// Fighters with an actual weapon in either hand. Shields do not qualify.
    #[must_use]
    pub fn battle_armed_fighters(&self) -> Vec<psiv_core::battle::FighterId> {
        let Some(set) = self.battles.as_ref() else {
            return Vec::new();
        };
        self.battle_roster()
            .into_iter()
            .flat_map(|roster| roster.side(psiv_core::battle::Side::Party))
            .filter(|fighter| {
                psiv_core::battle::weapon_reach(&fighter.stats, &set.data)
                    .ok()
                    .flatten()
                    .is_some()
            })
            .map(|fighter| fighter.id)
            .collect()
    }

    /// Ends a battle by absorbing the party records back into the roster and
    /// running both award passes — the full cartridge epilogue. The caller
    /// passes the per-member award (the split the battle computed).
    pub fn finish_battle_absorbing(&mut self, each: u16) -> Vec<BattleEvent> {
        let mut timeline = Vec::new();
        if let Some(battle) = self.battle.take() {
            self.battle_field_refresh_pending = true;
            // Battle_VictoryMessage ($30E6): the displayed pool must reach
            // Current_Money once, including vehicle battles. Escaping after
            // killing an enemy does not pay its accumulated pool.
            if battle.outcome() == Some(psiv_core::battle::Outcome::Victory) {
                self.game.add_money(u32::from(battle.pools().meseta));
                self.game.set_money(self.game.money().min(9_999_999));
            }
            if battle.is_vehicle() {
                let index = self.vehicle_index().unwrap_or(0);
                if let Some(member) = battle.into_party().into_iter().next()
                    && let Some(record) = self
                        .game
                        .vehicles_mut()
                        .get_mut(index.saturating_sub(1) as usize)
                {
                    record.current_hp = member.stats.curr_hp;
                    record.current_skill_uses = member.stats.curr_skill_uses;
                }
            } else {
                // The cartridge's results order, load-bearing: absorb the
                // records whole, pay both award passes, then level everyone the
                // pay reached — levelling first levels nobody, and levelling
                // battle's copies levels stale numbers.
                let party = battle.into_party();
                self.game.roster_mut().absorb(&party);
                let (paid_party, paid_absent) = self.game.award_experience(each);
                if let Some(set) = self.battles.as_ref() {
                    for id in &paid_party {
                        let Some(stats) = self.game.roster_mut().get_mut(*id) else {
                            continue;
                        };
                        let techniques = stats.techniques;
                        let skills = stats.skills;
                        if let Ok(Some(event)) = psiv_core::battle::level_up(id.0, stats, &set.data)
                        {
                            timeline.push(event);
                            for (before, &now) in techniques.iter().zip(&stats.techniques) {
                                if *before != now
                                    && now != 0
                                    && let Some(ability) = set.data.technique(now)
                                {
                                    timeline.push(BattleEvent::LearnedAbility {
                                        character: id.0,
                                        name: ability.name.clone(),
                                    });
                                }
                            }
                            for (before, &now) in skills.iter().zip(&stats.skills) {
                                if *before != now
                                    && now != 0
                                    && let Some(ability) = set.data.skill(now)
                                {
                                    timeline.push(BattleEvent::LearnedAbility {
                                        character: id.0,
                                        name: ability.name.clone(),
                                    });
                                }
                            }
                        }
                    }
                    // The benched roster levels silently and replenishes its
                    // skill uses, unlike the visible party's results sequence.
                    for id in &paid_absent {
                        if let Some(stats) = self.game.roster_mut().get_mut(*id) {
                            let _ = psiv_core::battle::level_up_absent(id.0, stats, &set.data);
                        }
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
            return self.scene_tick(input);
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
        if self.scene_triggers_pending && !self.field_suspended {
            self.scene_triggers_pending = false;
            let events = self.evaluate_triggers(self.state().cell());
            if !events.is_empty() {
                return events;
            }
        }
        if self.vehicle.is_some() {
            return self.tick_vehicle(input);
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
                    if self.start_chest_interaction(npc_index)
                        || self.start_interaction_event(&mut events)
                    {
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
            && self.loot.is_none()
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
        // Wander and bespoke draws are conditional consumers of the same
        // stream — an idle NPC whose countdown expires rolls once; most frames
        // none do.
        // This runs before the camera's own tick because the cartridge's
        // visibility test reads sprite positions written the frame before.
        if !self.field_suspended {
            self.tick_field_objects();
        }
        // `UpdateCamera*PosFG/BG` folds in last frame's scroll and
        // `FieldObj_*` latches this frame's, both against the party's
        // post-movement position.
        self.camera.tick(driver_of(self.party.leader()));
        events
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
        self.camera_glide = None;
        self.camera.set_position(x, y);
    }

    /// `Event_MoveCamera` (`ps4.asm:121468`): pan the camera to frame a world
    /// position. The operands are a *subject*, not a scroll — the routine
    /// subtracts the driver's home offset (`$98`/`$58`, `HOME_X`/`HOME_Y`) to
    /// get the camera target, then steps toward it at `speed` px/frame per
    /// axis. `AlysFound` ends on exactly this op to hand the view back to the
    /// new leader; parking the raw operands instead left the party in the
    /// top-left corner of the screen.
    pub fn scene_move_camera(&mut self, x: i32, y: i32, speed: i32) {
        let target_x = x - psiv_core::HOME_X;
        let target_y = y - psiv_core::HOME_Y;
        if speed <= 0 {
            self.set_camera(target_x, target_y);
            return;
        }
        self.camera_glide = Some(CameraGlide {
            target_x,
            target_y,
            speed,
        });
    }

    /// One frame of an in-flight `Event_MoveCamera` pan.
    fn tick_camera_glide(&mut self) {
        let Some(glide) = self.camera_glide else {
            return;
        };
        let (raw_x, raw_y) = self.camera.raw();
        let (x, y) = (raw_x >> 16, raw_y >> 16);
        let step = |from: i32, to: i32| from + (to - from).clamp(-glide.speed, glide.speed);
        let (next_x, next_y) = (step(x, glide.target_x), step(y, glide.target_y));
        self.camera.set_position(next_x, next_y);
        if next_x == glide.target_x && next_y == glide.target_y {
            self.camera_glide = None;
        }
    }

    /// Applies the packed `loc_51AB2` gate write to the current camera without
    /// repositioning the view. This is the runtime seam for `RefreshMap` calls
    /// made after a scene or warp has already entered the map.
    pub fn refresh_map_camera_gates(&mut self) -> Result<(), BridgeError> {
        let record = self
            .map_record()
            .cloned()
            .ok_or(BridgeError::NotPacked(self.map.id().0))?;
        refresh_camera_gates(&mut self.camera, &record).map_err(BridgeError::Rejected)
    }

    /// The map's wanderers, for the renderer's per-frame positions.
    #[must_use]
    pub fn wanderers(&self) -> &[Wanderer] {
        self.wander.wanderers()
    }

    /// The map's bespoke field-object actors, for renderer and replay state.
    #[must_use]
    pub fn bespoke_actors(&self) -> &[BespokeActor] {
        self.bespoke.actors()
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
        let (battle, events) = if self.vehicle.is_some() {
            Battle::start_vehicle(record, party, &set.data, &mut rng2)
        } else {
            Battle::start(record, party, &set.data, false, &mut rng2)
        }
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
            .round_with_inventory(orders, &set.data, self.game.inventory_mut(), &mut rng2)
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

    /// Places a field object at a cartridge pixel position.
    ///
    /// This is the position-side twin of [`Runtime::face_npc`]. It is used by
    /// receipt-backed replay/debug fixtures whose object has already wandered
    /// off its packed spawn point; normal gameplay reaches the same map seam
    /// through the field-object walker.
    ///
    /// # Errors
    ///
    /// [`psiv_core::MapError`] when the object index or pixel position is not
    /// valid for the loaded map.
    pub fn set_npc_pixel_position(
        &mut self,
        index: usize,
        x: i32,
        y: i32,
    ) -> Result<(), psiv_core::MapError> {
        self.map.set_npc_pixel_position(index, x, y)
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
        let mut unsupported = None;
        let mut hit = None;
        for &index in &indices {
            let Some(trigger) = TRIGGERS.get(usize::from(index)) else {
                continue;
            };
            let result = if index == 0x0D {
                self.elevator_trigger(cell)
            } else {
                trigger.evaluate(&ctx)
            };
            match result {
                TriggerResult::NoEvent => {}
                TriggerResult::Unsupported(..) => {
                    if unsupported.is_none() {
                        unsupported = Some((index, result));
                    }
                }
                _ => {
                    hit = Some((index, result));
                    break;
                }
            }
        }
        let hit = hit.or(unsupported);
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
        // Event_ElevatorDoorOpening exits silently unless the chunk at the
        // leader's object position is the original closed-door tile.
        if event == 0x13 && self.map_chunk_at(PixelPos::from_cell(leader.cell())) != Some(0x4F) {
            return true;
        }
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
        // The runner walks with the party's own step timing, so scripted-walk
        // interpolation (renderer, camera driver) shares one clock with field
        // walking.
        let Ok(runner) = runner_for(scene, cast, self.party.leader().step_frames()) else {
            return false;
        };
        self.scene = Some(runner);
        self.scene_event = event;
        self.scene_input = SceneInput::None;
        self.scene_choice_pending = false;
        self.scene_camera_locked = false;
        self.scene_warmup = true;
        true
    }

    /// The cast a scene may address: every party member (by slot and by
    /// character through the runner alias) plus every map object by index.
    fn build_cast(&self) -> Vec<ScriptedActor> {
        let mut cast = Vec::new();
        for (slot, member) in self.party.members().iter().enumerate() {
            cast.push(ScriptedActor::new(
                ActorRef::PartyMember(slot),
                member.cell,
                member.facing,
            ));
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

    /// Applies a retail dialogue `$F2` event-flag write immediately. The
    /// renderer uses this narrow mutator instead of reaching into save state.
    pub fn set_event_flag(&mut self, flag: u8) -> Result<(), psiv_core::MapError> {
        self.game.set(Flag::event(u16::from(flag)))
    }

    /// Whether a scene is running (cinema mode, input ownership).
    #[must_use]
    pub fn scene_active(&self) -> bool {
        self.scene.is_some()
    }

    /// The running scene's actors, for the renderer to draw at their scripted
    /// positions — with live step state, so walks render as walks. Empty when
    /// no scene runs.
    #[must_use]
    pub fn scene_actors(&self) -> &[ScriptedActor] {
        self.scene.as_ref().map(|r| r.actors()).unwrap_or(&[])
    }

    /// The step timing scene walks interpolate with.
    #[must_use]
    pub fn step_frames(&self) -> StepFrames {
        self.party.leader().step_frames()
    }

    /// The live object behind a party slot, also used by Character(id) ops.
    #[must_use]
    pub fn scene_party_actor(&self, slot: usize) -> Option<&ScriptedActor> {
        self.scene.as_ref()?.actor(ActorRef::PartyMember(slot))
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

    /// Original event index while the scene is active; bit 15 distinguishes
    /// panel cutscenes from ordinary field events in the text renderer.
    #[must_use]
    pub fn scene_event(&self) -> Option<EventIndex> {
        self.scene.as_ref().map(|_| self.scene_event)
    }

    /// The active scene's selected text-window routine, including resumes.
    #[must_use]
    pub fn scene_dialogue_window(&self) -> Option<psiv_core::DialogueWindow> {
        self.scene.as_ref().map(SceneRunner::dialogue_window)
    }

    /// The renderer reports the scene-requested dialogue window has closed.
    pub fn dialogue_closed(&mut self) {
        if self.scene.is_some() && !matches!(self.scene_input, SceneInput::Choice(_)) {
            self.scene_input = SceneInput::DialogueClosed;
        }
    }

    /// The renderer reached FF, with no suspended F7 cursor left to resume.
    pub fn dialogue_ended(&mut self) {
        if self.scene.is_some() && !matches!(self.scene_input, SceneInput::Choice(_)) {
            self.scene_input = SceneInput::DialogueEnded;
        }
    }

    /// Releases the retail ending's final Start gate.
    pub fn ending_continue(&mut self) {
        if self.scene.is_some() {
            self.scene_input = SceneInput::EndingContinue;
        }
    }

    /// Returns whether the retail ending has latched the cleared-game state.
    #[must_use]
    pub fn game_cleared(&self) -> bool {
        self.game_cleared
    }

    /// Applies the cartridge's battle-return map load before revealing the
    /// field. Presentation calls this after results; headless callers receive
    /// the same refresh automatically on their next field tick.
    pub fn return_to_field(&mut self) -> Vec<RuntimeEvent> {
        if self.battle.is_some() || !std::mem::take(&mut self.battle_field_refresh_pending) {
            return Vec::new();
        }
        match self.refresh_field_after_battle() {
            Ok(()) => vec![RuntimeEvent::MapRefreshed],
            Err(error) => vec![RuntimeEvent::MapRefreshFailed {
                error: error.to_string(),
            }],
        }
    }

    fn refresh_field_after_battle(&mut self) -> Result<(), BridgeError> {
        let target = self.map.id();
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        // GameMode_LoadFieldMap with Map_Load_Flags bit 0: the map-data
        // walk runs, while object initialization, party placement and camera
        // initialization do not. Keep the wander clocks and scene cast too.
        let effects = effects::evaluate(record, &mut self.game);
        let mut map = bridge::field_map_retaining_objects(record, Some(&effects), self.map.npcs())?;
        bridge::attach_chests(&mut map, record, &self.game, self.map.npcs(), &effects)?;
        self.map = map;
        self.effects = effects;
        self.scene_triggers_pending = true;
        self.field_status.clock.reset();
        Ok(())
    }

    fn change_map(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
    ) -> Result<(), BridgeError> {
        self.change_map_from(target, cell, facing, self.map.id().0)
    }

    fn change_map_from(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
        previous_map: u16,
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
        bridge::attach_chests(&mut map, record, &self.game, &[], &effects)?;
        clear_bespoke_entry_flags(&mut self.game, record);
        self.effects = effects;
        // LoadMapObjects creates a fresh cast. Scene despawns only change
        // the live objects; persistent removals come from MapDataManager's
        // extracted flag gates above (including recruited party members).
        self.party
            .enter_map(&map, cell, facing)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.wander = build_wander(&map, record)?;
        self.bespoke = build_bespoke(&map, record)?;
        // Map entry places the view rather than scrolling it in, so the camera
        // starts framed on the party wherever the warp dropped them.
        self.camera = camera_for_record(driver_of(self.party.leader()), &map, record)
            .map_err(BridgeError::Rejected)?;
        self.map = map;
        if let Some(vehicle) = self.vehicle.as_mut() {
            if !vehicle.enter_map(&self.map, cell, facing) {
                return Err(BridgeError::Rejected(format!(
                    "vehicle destination ({}, {}) is outside map {}",
                    cell.x, cell.y, target.0
                )));
            }
            self.camera = camera_for_record(vehicle::driver_of(vehicle), &self.map, record)
                .map_err(BridgeError::Rejected)?;
        }
        // The first field control tick checks RunEvents before accepting a
        // step. loc_518D2 also resets the encounter countdown on EVERY load.
        self.prev_standing = None;
        self.scene_triggers_pending = true;
        self.field_status.clock.reset();
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
        self.saved_map_index_2 = previous_map;
        self.apply_travel_entry();
        Ok(())
    }
}
