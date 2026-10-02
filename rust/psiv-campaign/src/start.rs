//! Starting a session: power-on, or a slot file.
//!
//! Both build the runtime the way the title does (`Runtime::new_game` plus the
//! opening event for START, `Runtime::from_save` for CONTINUE) and arm its
//! battles. The runtime is then handed to a [`Session`], and from there on
//! the run is pads only.

use std::path::{Path, PathBuf};

use psiv_core::{RetailSlot, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, Session};

use crate::tape::{TapeStart, fnv1a64};

/// Where a run starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartPoint {
    /// The title's START.
    NewGame,
    /// A slot file (`slot_N.sram`).
    Save(PathBuf),
}

/// A setup failure: the pack, the save or the route could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupError(pub String);

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SetupError {}

/// Opens a session at `start` over the pack in `pack`.
///
/// # Errors
///
/// [`SetupError`] when the pack, the battle files or the save cannot be read
/// or built.
pub fn open_session(pack: &Path, start: &StartPoint) -> Result<(Session, TapeStart), SetupError> {
    let data = GameData::load(pack)
        .map_err(|e| SetupError(format!("cannot load pack {}: {e}", pack.display())))?;
    let files = BattleFiles::load(pack)
        .map_err(|e| SetupError(format!("cannot load battle files: {e}")))?;
    let (mut runtime, tape_start) = match start {
        StartPoint::NewGame => {
            let event = data
                .new_game()
                .ok_or_else(|| SetupError("pack has no title initializer".into()))?
                .event_index;
            let mut runtime = Runtime::new_game(data, StepFrames::default())
                .map_err(|e| SetupError(format!("new game: {e}")))?;
            runtime
                .enable_battles(&files)
                .map_err(|e| SetupError(format!("battles: {e}")))?;
            if !runtime.start_event(event) {
                return Err(SetupError(format!(
                    "the opening event {event:#x} has no scene"
                )));
            }
            return Ok((Session::new(runtime), TapeStart::NewGame));
        }
        StartPoint::Save(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| SetupError(format!("cannot read save {}: {e}", path.display())))?;
            let slot = slot_of(path);
            let decoded = RetailSlot::from_bytes(&bytes, slot)
                .and_then(|encoded| encoded.decode())
                .map_err(|e| SetupError(format!("bad save {}: {e}", path.display())))?;
            let runtime = Runtime::from_save(data, decoded, StepFrames::default())
                .map_err(|e| SetupError(format!("save {}: {e}", path.display())))?;
            (
                runtime,
                TapeStart::Save {
                    hash: fnv1a64(&bytes),
                },
            )
        }
    };
    runtime
        .enable_battles(&files)
        .map_err(|e| SetupError(format!("battles: {e}")))?;
    Ok((Session::new(runtime), tape_start))
}

/// The slot a file `slot_N.sram` holds (zero-based); slot 0 for any other name.
fn slot_of(path: &Path) -> usize {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("slot_"))
        .and_then(|rest| rest.strip_suffix(".sram"))
        .and_then(|n| n.parse::<usize>().ok())
        .map_or(0, |n| n.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_file_name_gives_its_slot() {
        assert_eq!(slot_of(Path::new("a/slot_1.sram")), 0);
        assert_eq!(slot_of(Path::new("slot_3.sram")), 2);
        assert_eq!(slot_of(Path::new("elsewhere.bin")), 0);
    }
}
