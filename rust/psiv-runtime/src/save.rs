//! Runtime file I/O and the store a [`Session`](crate::Session) saves through.
//!
//! The title's CONTINUE loads a validated retail-shaped slot, rebuilds
//! GameState, evaluates map effects, and constructs a runtime at its saved
//! map/cell; the camp's SAVE writes the same shape. Both go through
//! [`SaveStore`], which is constructed with the run's directory: the shell
//! owns the directory policy (`PSIV_SAVE_DIR`, and the refusal of a scripted
//! run that names none), the runtime never guesses one, and the `PSIV_LOAD_SLOT`
//! boot path reaches the same validation for isolated debug runs.

use std::fmt;
use std::path::{Path, PathBuf};

use psiv_core::battle::Lcg41;
use psiv_core::{
    Cell, Direction, GameState, Party, RetailLocation, RetailSave, RetailSlot, SceneInput,
    StepFrames,
};
use psiv_data::GameData;

use super::bridge::{build_bespoke, build_wander, clear_bespoke_entry_flags};
use super::{BridgeError, Runtime};
use crate::geometry::{camera_for_record, driver_of};

/// An error while reading, writing or constructing a runtime save.
#[derive(Debug)]
pub enum RuntimeSaveError {
    /// The filesystem rejected the save directory or file.
    Io(std::io::Error),
    /// The file was not a valid retail-shaped slot.
    Format(psiv_core::SaveError),
    /// The saved map could not be constructed by the runtime bridge.
    Runtime(BridgeError),
    /// The retail pixel position is not representable by the 16-pixel core
    /// movement grid.
    InvalidPosition {
        /// The saved or requested x pixel coordinate.
        x: u16,
        /// The saved or requested y pixel coordinate.
        y: u16,
    },
}

impl fmt::Display for RuntimeSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeSaveError::Io(error) => write!(f, "save file I/O failed: {error}"),
            RuntimeSaveError::Format(error) => write!(f, "save file format failed: {error}"),
            RuntimeSaveError::Runtime(error) => write!(f, "saved runtime could not start: {error}"),
            RuntimeSaveError::InvalidPosition { x, y } => {
                write!(
                    f,
                    "saved position ({x}, {y}) is not aligned to a 16-pixel cell"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeSaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RuntimeSaveError::Io(error) => Some(error),
            RuntimeSaveError::Format(error) => Some(error),
            RuntimeSaveError::Runtime(error) => Some(error),
            RuntimeSaveError::InvalidPosition { .. } => None,
        }
    }
}

impl From<std::io::Error> for RuntimeSaveError {
    fn from(error: std::io::Error) -> RuntimeSaveError {
        RuntimeSaveError::Io(error)
    }
}

impl From<psiv_core::SaveError> for RuntimeSaveError {
    fn from(error: psiv_core::SaveError) -> RuntimeSaveError {
        RuntimeSaveError::Format(error)
    }
}

impl From<BridgeError> for RuntimeSaveError {
    fn from(error: BridgeError) -> RuntimeSaveError {
        RuntimeSaveError::Runtime(error)
    }
}

/// Builds a normal runtime from a `GameState` that has already been chosen by
/// either new-game initialization or a loaded save.
#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimePlacement {
    map_id: u16,
    spawn: Cell,
    facing: Direction,
    step_frames: StepFrames,
    world_index: u16,
    map_index_2: u16,
}

impl RuntimePlacement {
    pub(super) const fn new(
        map_id: u16,
        spawn: Cell,
        facing: Direction,
        step_frames: StepFrames,
        world_index: u16,
        map_index_2: u16,
    ) -> RuntimePlacement {
        RuntimePlacement {
            map_id,
            spawn,
            facing,
            step_frames,
            world_index,
            map_index_2,
        }
    }
}

pub(super) fn construct_runtime(
    data: GameData,
    placement: RuntimePlacement,
    mut game: GameState,
    followers: usize,
) -> Result<Runtime, BridgeError> {
    let record = data
        .map(psiv_data::MapId(placement.map_id))
        .ok_or(BridgeError::NotPacked(placement.map_id))?;
    let saved_sound_index = crate::map_change::adjusted_map_music(
        psiv_core::MapId(placement.map_id),
        record.music.id,
        &game,
        0,
    )
    .unwrap_or(0);
    // The message box is part of the runtime, not a second step a caller can
    // forget: the dialogue comes from the loaded data, and data without it
    // (a synthetic pack built from parts) is refused here, where the error can
    // name the problem instead of failing on the first window a scene opens.
    let dialogue_pack = data
        .dialogue()
        .ok_or_else(|| BridgeError::Rejected("pack has no dialogue set".to_owned()))?;
    let effects = super::effects::evaluate(record, &mut game);
    let map = crate::bridge::field_map_entered(record, &effects, &game)?;
    clear_bespoke_entry_flags(&mut game, record);
    let party = Party::new(
        &map,
        placement.spawn,
        placement.facing,
        placement.step_frames,
        followers,
    )
    .map_err(|error| BridgeError::Rejected(error.to_string()))?;
    let wander = build_wander(&map, record)?;
    let bespoke = build_bespoke(&map, record)?;
    let camera = camera_for_record(driver_of(party.leader()), &map, record)
        .map_err(BridgeError::Rejected)?;
    let mut runtime = Runtime {
        data,
        map,
        party,
        game,
        saved_world_index: placement.world_index,
        saved_map_index_2: placement.map_index_2,
        dungeon_exit_index: 0,
        pending_travel: None,
        loot: None,
        scene: None,
        scene_input: SceneInput::None,
        scene_event: psiv_core::EventIndex(0),
        dialogue_answer: None,
        scene_choice_pending: false,
        dialogue: super::dialogue::DialogueRunner::with_pack(dialogue_pack),
        scene_tree_address: None,
        scene_panel_sprites: false,
        game_cleared: false,
        game_over: false,
        field_status: super::field_status::FieldStatus::default(),
        prev_standing: None,
        wander,
        bespoke,
        rng: Lcg41::default(),
        field_suspended: false,
        frames: 0,
        camera,
        scene_warmup: false,
        scene_triggers_pending: true,
        offscreen: Vec::new(),
        battles: None,
        battle: None,
        battle_field_refresh_pending: false,
        scene_battle: None,
        effects,
        vehicle: None,
        boarding_body: None,
        saved_sound_index,
        map_load_flags: 0,
        saved_party_slots: None,
        camera_glide: None,
        scene_camera_locked: false,
        scene_returned: None,
        scene_tail: None,
    };
    runtime.sync_vehicle_selector()?;
    runtime.apply_travel_entry();
    Ok(runtime)
}

/// A save operation the session refused before it reached a file.
#[derive(Debug)]
pub enum SessionSaveError {
    /// The session has no store: the shell's directory policy refused to guess
    /// one, and a runtime never falls back to a repository default.
    NoDirectory,
    /// The store's own operation failed.
    Save(RuntimeSaveError),
}

impl fmt::Display for SessionSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionSaveError::NoDirectory => {
                write!(f, "the session has no save directory for this run")
            }
            SessionSaveError::Save(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SessionSaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SessionSaveError::NoDirectory => None,
            SessionSaveError::Save(error) => Some(error),
        }
    }
}

impl From<RuntimeSaveError> for SessionSaveError {
    fn from(error: RuntimeSaveError) -> SessionSaveError {
        SessionSaveError::Save(error)
    }
}

/// The one save store a session owns: the run directory the shell resolved.
///
/// Every slot read, write and erase a session performs goes through this
/// store, so the shell's policy (`rust/psiv-godot/src/save_dir.rs`) is the only
/// place a directory can come from: a run that named none gets no store, and
/// the session refuses the operation instead of touching a default path.
#[derive(Debug, Clone)]
pub struct SaveStore {
    directory: PathBuf,
}

impl SaveStore {
    /// A store over `directory`, the run directory the caller resolved.
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> SaveStore {
        SaveStore {
            directory: directory.into(),
        }
    }

    /// The directory every operation goes to.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The three visible slots, presence only.
    ///
    /// Loading is the same checksum and slot validation CONTINUE runs; no
    /// save bytes are written, repaired or reserialized by the probe.
    #[must_use]
    pub fn slots(&self, data: &GameData) -> [bool; 3] {
        std::array::from_fn(|slot| self.load(data.clone(), slot, StepFrames::default()).is_ok())
    }

    /// Loads one visible slot: the title's CONTINUE.
    pub(crate) fn load(
        &self,
        data: GameData,
        slot: usize,
        step_frames: StepFrames,
    ) -> Result<Runtime, RuntimeSaveError> {
        Runtime::load_slot(data, &self.directory, slot, step_frames)
    }

    /// Writes one visible slot: the camp's SAVE.
    pub(crate) fn write(
        &self,
        runtime: &Runtime,
        slot: usize,
    ) -> Result<PathBuf, RuntimeSaveError> {
        runtime.save_slot(&self.directory, slot)
    }

    /// Performs the title's destructive ERASE DATA for one visible slot.
    pub fn erase(&self, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        Runtime::erase_slot(&self.directory, slot)
    }
}

impl Runtime {
    /// Returns the deterministic disk path for a visible save slot.
    pub(crate) fn slot_path(directory: &Path, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        validate_slot(slot)?;
        Ok(directory.join(format!("slot_{}.sram", slot + 1)))
    }

    /// The exact retail-shaped slot bytes the ordinary writer persists.
    /// Read-only: native replay compares a chapter boundary without performing
    /// a SAVE action or introducing a second encoding path.
    ///
    /// # Errors
    /// A slot outside the three visible rows or an unrepresentable position.
    pub fn slot_bytes(&self, slot: usize) -> Result<Vec<u8>, RuntimeSaveError> {
        validate_slot(slot)?;
        let cell = self.vehicle_cell().unwrap_or_else(|| self.state().cell());
        let (char_x, char_y) = cell_pixels(cell)?;
        let save = RetailSave {
            snapshot: self.game.snapshot(),
            location: RetailLocation {
                // World_Index is the high byte of the retail word; the second
                // map word now follows ordinary warps and explicit scene loads.
                world_index: self.saved_world_index,
                map_index_2: self.saved_map_index_2,
                map_index: self.map.id().0,
                char_x,
                char_y,
            },
        };
        let encoded = RetailSlot::encode(&save, slot)?.with_dungeon_exit(self.dungeon_exit_index);
        Ok(encoded.as_bytes().to_vec())
    }

    /// Writes the current state to a retail-shaped slot file.
    ///
    /// `directory` is the caller's run directory, never a repository default:
    /// the Godot shell resolves `PSIV_SAVE_DIR` for it and refuses a scripted
    /// run without that variable (`rust/psiv-godot/src/save_dir.rs`), while
    /// tests pass a temporary directory without changing runtime state.
    ///
    /// Read-only on the runtime (`&self`), which is why it is public under the
    /// S6 boundary: a caller holding a session's `&Runtime` may record the game
    /// it is looking at — the campaign runner writes each chapter's boundary
    /// save this way — but cannot change it.
    pub fn save_slot(&self, directory: &Path, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        let path = Self::slot_path(directory, slot)?;
        let bytes = self.slot_bytes(slot)?;
        std::fs::create_dir_all(directory)?;
        std::fs::write(&path, bytes)?;
        Ok(path)
    }

    /// Loads a slot and constructs the same runtime seam the title continue
    /// path will use once a title screen exists.
    pub(crate) fn load_slot(
        data: GameData,
        directory: &Path,
        slot: usize,
        step_frames: StepFrames,
    ) -> Result<Runtime, RuntimeSaveError> {
        let path = Self::slot_path(directory, slot)?;
        slot_runtime(&data, &std::fs::read(path)?, slot, step_frames)
    }

    /// Performs the retail title's destructive erase for one visible slot.
    ///
    /// The selected file's interleaved common header survives; only its
    /// physical payload is zeroed, matching `loc_64DC0` on the shared SRAM
    /// device. The directory is the caller's run directory — the Godot shell
    /// resolves `PSIV_SAVE_DIR` for it (`rust/psiv-godot/src/save_dir.rs`) and
    /// refuses a scripted run without that variable — so an erase never falls
    /// back to a repository default.
    pub(crate) fn erase_slot(directory: &Path, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        let path = Self::slot_path(directory, slot)?;
        let bytes = std::fs::read(&path)?;
        let erased = RetailSlot::erase_physical_payload(&bytes, slot)?;
        std::fs::write(&path, erased.as_bytes())?;
        Ok(path)
    }

    /// Constructs a runtime from an already decoded retail save.
    pub(crate) fn from_save(
        data: GameData,
        save: RetailSave,
        step_frames: StepFrames,
    ) -> Result<Runtime, RuntimeSaveError> {
        let cell = saved_cell(save.location)?;
        let followers = GameState::from_snapshot(&save.snapshot)
            .party_len()
            .saturating_sub(1);
        construct_runtime(
            data,
            RuntimePlacement::new(
                save.location.map_index,
                cell,
                // Retail does not persist facing in the save range. The title
                // load path supplies map state, so Down is the explicit interim
                // default until a title-facing seam is decoded.
                Direction::Down,
                step_frames,
                save.location.world_index,
                save.location.map_index_2,
            ),
            GameState::from_snapshot(&save.snapshot),
            followers,
        )
        .map_err(RuntimeSaveError::Runtime)
    }
}

/// Builds the runtime a slot's own bytes describe.
///
/// One path for both readers of a slot file — the title's CONTINUE, which has a
/// directory, and `Session::start(...).from_slot_bytes`, which has the bytes —
/// so a slot a run resumes from and a slot a player continues from are the same
/// game.
pub(super) fn slot_runtime(
    data: &GameData,
    bytes: &[u8],
    slot: usize,
    step_frames: StepFrames,
) -> Result<Runtime, RuntimeSaveError> {
    let encoded = RetailSlot::from_bytes(bytes, slot)?;
    let mut runtime = Runtime::from_save(data.clone(), encoded.decode()?, step_frames)?;
    // Positive map bytes and explicit place-entry selectors already
    // reconstruct the exit. Only inherited maps need native metadata.
    if runtime
        .data
        .map(psiv_data::MapId(runtime.map.id().0))
        .is_some_and(|record| record.flags.dungeon_teleport_index.is_none())
        && let Some(index) = encoded.dungeon_exit()
        && (index == 0
            || runtime
                .data
                .travel()
                .is_some_and(|travel| travel.dungeons.iter().any(|exit| exit.index == index)))
    {
        runtime.dungeon_exit_index = index;
    }
    Ok(runtime)
}

fn validate_slot(slot: usize) -> Result<(), RuntimeSaveError> {
    if slot < psiv_core::RETAIL_SLOT_COUNT {
        Ok(())
    } else {
        Err(RuntimeSaveError::Format(
            psiv_core::SaveError::InvalidSlot {
                slot,
                slots: psiv_core::RETAIL_SLOT_COUNT,
            },
        ))
    }
}

fn cell_pixels(cell: Cell) -> Result<(u16, u16), RuntimeSaveError> {
    let x = cell
        .x
        .checked_mul(16)
        .ok_or(RuntimeSaveError::InvalidPosition {
            x: cell.x,
            y: cell.y,
        })?;
    let y = cell
        .y
        .checked_mul(16)
        .ok_or(RuntimeSaveError::InvalidPosition {
            x: cell.x,
            y: cell.y,
        })?;
    Ok((x, y))
}

fn saved_cell(location: RetailLocation) -> Result<Cell, RuntimeSaveError> {
    if !location.char_x.is_multiple_of(16) || !location.char_y.is_multiple_of(16) {
        return Err(RuntimeSaveError::InvalidPosition {
            x: location.char_x,
            y: location.char_y,
        });
    }
    Ok(Cell::new(location.char_x / 16, location.char_y / 16))
}
