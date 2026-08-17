//! Deterministic game core: `State + Input -> State + Effects`.
//!
//! See `docs/RUNTIME_DESIGN.md`. This crate is the field engine for the
//! walk + warp + NPCs slice. It has no dependencies, does no I/O, knows nothing
//! about serde, Godot, or the pack format, and never touches a float — the
//! original is a 68000, and floats are how a reimplementation drifts off its
//! oracle.
//!
//! # The model
//!
//! Position lives on the 16-pixel collision-cell grid exactly as
//! `GetChunkAndCollision` models it. Movement is whole cell-steps carrying an
//! integer animation progress the renderer interpolates. The blocking set is
//! the cartridge's: types 8 (solid), 9 (water), `$A` (sand), `$B` (ice),
//! `$C` (shop). Type 1 (map change) does not block — the walker steps onto it
//! and the transition fires on arrival. Transitions come from the cartridge's
//! two tables, which fire under different conditions; see [`WarpTrigger`].
//!
//! Most maps are bounded rooms whose edges stop the walker. The two overworlds
//! are globes: see [`Topology`].
//!
//! Collision grids are supplied by the caller and never treated as immutable
//! cartridge truth. The overworlds' event-flag-gated `layout_patches` are
//! applied by the bridge before the map is built; this crate models no event
//! flags and sees only the resulting grid.
//!
//! # Usage
//!
//! ```
//! use psiv_core::{
//!     Cell, CollisionGrid, Direction, Effect, FieldMap, FieldState, Input, MapId,
//!     StepFrames, Warp, WarpTrigger,
//! };
//!
//! let mut grid = CollisionGrid::filled(8, 8, 0x0)?;
//! grid.set(Cell::new(4, 0), 0x1)?; // a doorway
//!
//! let door = Warp::door(Cell::new(4, 0), MapId(0x13), Cell::new(2, 6), Direction::Up);
//! let map = FieldMap::new(MapId(0x05), grid, vec![door], vec![])?;
//!
//! let mut state = FieldState::new(&map, Cell::new(4, 1), Direction::Up, StepFrames::default())?;
//!
//! let mut effects = Vec::new();
//! for _ in 0..StepFrames::default().get() {
//!     effects.extend(state.tick(&map, Input::Direction(Direction::Up)));
//! }
//! assert_eq!(
//!     effects,
//!     vec![
//!         Effect::StepCompleted { cell: Cell::new(4, 0) },
//!         Effect::Warp {
//!             from: Cell::new(4, 0),
//!             trigger: WarpTrigger::MapChange,
//!             target_map: MapId(0x13),
//!             target_cell: Cell::new(2, 6),
//!             facing: Direction::Up,
//!         },
//!     ],
//! );
//! # Ok::<(), psiv_core::MapError>(())
//! ```
//!
//! # Determinism
//!
//! The same `(state, map, input sequence)` always yields the same state and the
//! same effect log. Nothing here reads a clock, draws a random number, uses
//! interior mutability, or iterates a hashed container: ordered lookups go
//! through `Vec` and `BTreeMap` so iteration order can never leak into
//! behaviour.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// The cartridge is a 68000. A float anywhere in the core is a fidelity bug, so
// the compiler enforces their absence rather than a code-review habit.
#![deny(clippy::float_arithmetic)]

pub mod battle;
mod camera;
mod chest;
mod collision;
mod error;
mod field;
mod geom;
mod inventory;
mod map;
mod map_load;
mod party;
mod replay;
mod roster;
mod save;
mod scene;
mod scene_presentation;
mod scene_runner;
mod scenes;
mod state;
mod trigger;
mod trigger_custom;
mod trigger_table;
pub mod vehicle;
mod wander;

pub use camera::{
    Camera, CameraBounds, CameraEdges, CameraGates, CameraPlane, Driver, HOME_X, HOME_Y, ONE_PIXEL,
    SCREEN_HEIGHT, SCREEN_WIDTH, SPRITE_ORIGIN, THRESHOLD_X, THRESHOLD_Y, on_screen,
    type_tests_visibility,
};
pub use chest::{CHEST_OPEN_FACING, CHEST_SHUT_FACING, Chest, ChestContents, ChestOutcome};
pub use collision::{CollisionGrid, CollisionType, MAX_COLLISION_VALUE};
pub use error::MapError;
pub use field::{
    Effect, FieldState, Input, InteractReach, SUBCELL_UNITS, StepFrames, TALK_RANGE_PX,
};
pub use geom::{CELL_PIXELS, Cell, CellRect, Direction};
pub use inventory::{EMPTY, INVENTORY_SLOTS, Inventory};
pub use map::{FieldMap, MapId, Npc, NpcId, SubCellOffset, Topology, Warp, WarpTrigger};
pub use map_load::{apply_map_load, flag_clears_for_entry};
pub use party::{MAX_PARTY_MEMBERS, MemberView, Party};
pub use replay::{
    Buttons, COLUMNS, Column, Coverage, DiffReport, Divergence, FrameSample, OBJECT_COLUMN_KINDS,
    OBJECT_ID_LOADED, OBJECT_SLOTS, ObjectSample, OracleLog, ReplayRow, Tape, TapeError, TapeFrame,
    TapeStep, all_modelled_columns, csv_header, facing_value, modelled_columns, object_column,
    object_columns,
};
pub use roster::{
    CHARACTER_COUNT, CharacterRoster, DEAD_STATUS_MASK, EXPERIENCE_CAP, REUNION_FLAG,
};
pub use save::{
    RETAIL_HEADER_LOGICAL_BYTES, RETAIL_HEADER_PHYSICAL_BYTES, RETAIL_PAYLOAD_BYTES,
    RETAIL_SLOT_COUNT, RETAIL_SLOT_FILE_BYTES, RETAIL_SLOT_STRIDE_BYTES, RetailLocation,
    RetailSave, RetailSlot, SaveError,
};
pub use scene::{
    ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, OP_BUDGET_PER_TICK, SceneEffect,
    SceneFault, SceneInput, SceneOp, ScriptedActor,
};
pub use scene_presentation::{PresentationAsset, PresentationOp};
pub use scene_runner::{Scene, SceneRunner, runner_for};
pub use scenes::{SCENES, scene_for};
pub use state::{CharId, Flag, FlagBank, GameState, PARTY_SLOTS};
pub use state::{
    MACRO_COMMANDS, MACRO_COUNT, MacroCommand, MacroRecord, StateSnapshot, VEHICLE_COUNT,
    VEHICLE_RECORD_BYTES, VehicleRecord,
};
pub use trigger::{
    AxisPredicate, Condition, CustomTrigger, EventIndex, PixelPos, PositionPredicate, Trigger,
    TriggerContext, TriggerResult, Unsupported, evaluate_list,
};
pub use trigger_table::TRIGGERS;
pub use vehicle::{
    DEFAULT_STEP_OFFSET, VEHICLE_INDEX_MAX, VEHICLE_INDEX_MIN, VehicleEffect, VehicleProfile,
    VehicleState, battle_member, can_cross, can_enter, directional_collision, dismount_allowed,
    encounter_suppressed, profile, standing_collision,
};
pub use wander::{
    Leash, WANDER_STEP_FRAMES, WanderKind, WanderSet, WanderSpeed, WanderState, Wanderer,
};
