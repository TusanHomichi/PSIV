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
mod battle_lifecycle;
mod boss_battles;
mod bridge;
mod camera;
mod camp;
mod effects;
mod encounters;
mod events;
#[cfg(test)]
mod field_entry_tests;
mod field_objects;
mod field_status;
mod field_tick;
mod field_triggers;
mod loot;
#[cfg(test)]
mod loot_tests;
pub use field_status::FieldNotice;
pub use loot::{LootResult, LootState};
#[cfg(test)]
mod field_status_tests;
mod geometry;
mod map_change;
mod new_game;
mod progression;
pub use progression::ProgressionRepair;
mod runtime_seams;
mod save;
mod scene_control;
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
mod view;
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

use bridge::char_id_by_symbol;

use psiv_core::battle::{Battle, Lcg41};
use psiv_core::{
    BespokeSet, Camera, Cell, CharId, Direction, EventIndex, FieldMap, Flag, GameState,
    PARTY_SLOTS, Party, SceneInput, SceneRunner, StepFrames, WanderSet,
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
}
