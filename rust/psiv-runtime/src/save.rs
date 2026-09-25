//! Runtime file I/O and the load-at-title seam for retail-shaped saves.
//!
//! Godot's title CONTINUE loads a validated retail-shaped slot, rebuilds
//! GameState, evaluates map effects, and constructs a runtime at its saved
//! map/cell. PSIV_LOAD_SLOT reaches the same path for isolated debug runs.

use std::fmt;
use std::path::{Path, PathBuf};

use psiv_core::battle::Lcg41;
use psiv_core::{
    Cell, Direction, GameState, Party, RetailLocation, RetailSave, RetailSlot, SceneInput,
    StepFrames,
};
use psiv_data::GameData;

use super::bridge::{build_bespoke, build_wander, clear_bespoke_entry_flags};
use super::{BridgeError, Runtime, camera_for_record, driver_of, field_map_patched};

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
    let effects = super::effects::evaluate(record, &mut game);
    let mut map = field_map_patched(record, Some(&effects))?;
    crate::bridge::attach_chests(&mut map, record, &game, &[], &effects)?;
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
        saved_party_slots: None,
        camera_glide: None,
        scene_camera_locked: false,
    };
    runtime.sync_vehicle_selector()?;
    runtime.apply_travel_entry();
    Ok(runtime)
}

impl Runtime {
    /// Returns the deterministic disk path for a visible save slot.
    pub fn slot_path(directory: &Path, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        validate_slot(slot)?;
        Ok(directory.join(format!("slot_{}.sram", slot + 1)))
    }

    /// Writes the current state to a retail-shaped slot file.
    ///
    /// `directory` is the caller's run directory, never a repository default:
    /// the Godot shell resolves `PSIV_SAVE_DIR` for it and refuses a scripted
    /// run without that variable (`rust/psiv-godot/src/save_dir.rs`), while
    /// tests pass a temporary directory without changing runtime state.
    pub fn save_slot(&self, directory: &Path, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        let path = Self::slot_path(directory, slot)?;
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
        std::fs::create_dir_all(directory)?;
        std::fs::write(&path, encoded.as_bytes())?;
        Ok(path)
    }

    /// Loads a slot and constructs the same runtime seam the title continue
    /// path will use once a title screen exists.
    pub fn load_slot(
        data: GameData,
        directory: &Path,
        slot: usize,
        step_frames: StepFrames,
    ) -> Result<Runtime, RuntimeSaveError> {
        let path = Self::slot_path(directory, slot)?;
        let bytes = std::fs::read(path)?;
        let encoded = RetailSlot::from_bytes(&bytes, slot)?;
        let mut runtime = Self::from_save(data, encoded.decode()?, step_frames)?;
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

    /// Performs the retail title's destructive erase for one visible slot.
    ///
    /// The selected file's interleaved common header survives; only its
    /// physical payload is zeroed, matching `loc_64DC0` on the shared SRAM
    /// device. The directory is the caller's run directory — the Godot shell
    /// resolves `PSIV_SAVE_DIR` for it (`rust/psiv-godot/src/save_dir.rs`) and
    /// refuses a scripted run without that variable — so an erase never falls
    /// back to a repository default.
    pub fn erase_slot(directory: &Path, slot: usize) -> Result<PathBuf, RuntimeSaveError> {
        let path = Self::slot_path(directory, slot)?;
        let bytes = std::fs::read(&path)?;
        let erased = RetailSlot::erase_physical_payload(&bytes, slot)?;
        std::fs::write(&path, erased.as_bytes())?;
        Ok(path)
    }

    /// Constructs a runtime from an already decoded retail save.
    pub fn from_save(
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
