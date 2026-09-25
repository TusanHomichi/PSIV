//! The one save-directory resolver, and every slot operation that uses it.
//!
//! Every slot read, write and erase in the shell goes through
//! [`save_directory`]. An interactive launch keeps the `saves/` default that
//! `AGENTS.md` ("Protect local inputs and evidence") warns about; a scripted
//! run (`--script` on the command line, how every native driver starts) must
//! name its run directory, so a driver launched from the repository root
//! cannot read, overwrite or erase the checkout's `saves/`.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use godot::prelude::*;
use psiv_core::StepFrames;
use psiv_data::GameData;
use psiv_runtime::Runtime;

/// The environment variable that names a run's save directory.
const SAVE_DIR_ENV: &str = "PSIV_SAVE_DIR";

/// The interactive default: `saves/` relative to the working directory.
const DEFAULT_SAVE_DIR: &str = "saves";

/// Godot's "run this script instead of the project's main scene" switch; the
/// command line of every `tools/native/native_*.gd` run carries it.
const SCRIPT_ARGUMENT: &str = "--script";

/// A run asked for a save directory that must not be guessed.
///
/// The only case today is a scripted run with no `PSIV_SAVE_DIR`; the type is
/// the seam's error so the rule can grow without call sites changing shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SaveDirError;

impl std::fmt::Display for SaveDirError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{SAVE_DIR_ENV} is unset: an automated run ({SCRIPT_ARGUMENT}) must copy the \
             source save to a run directory and point {SAVE_DIR_ENV} at it"
        )
    }
}

impl std::error::Error for SaveDirError {}

/// The save directory a run with `arguments` and `save_dir` uses.
///
/// Pure: no environment or filesystem access, so the rule is directly
/// testable. An empty `save_dir` counts as unset — `PSIV_SAVE_DIR=` must not
/// silently become the working directory.
pub(crate) fn resolve_save_directory<Argument: AsRef<OsStr>>(
    arguments: &[Argument],
    save_dir: Option<&OsStr>,
) -> Result<PathBuf, SaveDirError> {
    if let Some(directory) = save_dir.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(directory));
    }
    let scripted = arguments
        .iter()
        .any(|argument| argument.as_ref() == OsStr::new(SCRIPT_ARGUMENT));
    if scripted {
        return Err(SaveDirError);
    }
    Ok(PathBuf::from(DEFAULT_SAVE_DIR))
}

/// The run's save directory, or the rule that refuses to guess one.
///
/// The thin wrapper over `std::env`; every call site resolves through here and
/// reports the error instead of touching a fallback path.
pub(crate) fn save_directory() -> Result<PathBuf, SaveDirError> {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    resolve_save_directory(&arguments, std::env::var_os(SAVE_DIR_ENV).as_deref())
}

/// The three visible slots the title presents, presence only.
///
/// A refused run directory reads as three empty slots after the guard's
/// error: the title is presentation, and no lookup falls back to a directory
/// the run did not name.
pub(crate) fn presented_save_slots(data: &GameData) -> [bool; 3] {
    match save_directory() {
        Ok(directory) => crate::boot::available_save_slots(data, &directory),
        Err(error) => {
            godot_error!("save slot scan refused: {error}");
            [false; 3]
        }
    }
}

/// Loads one visible slot for the title's CONTINUE.
///
/// The resolver's refusal comes back before any filesystem access, so a load
/// never reads a directory the run did not name. The error is text for the
/// caller's message, the shell's convention for runtime boot failures.
pub(crate) fn load_slot(data: GameData, slot: usize) -> Result<Runtime, String> {
    let directory = save_directory().map_err(|error| error.to_string())?;
    Runtime::load_slot(data, &directory, slot, StepFrames::default())
        .map_err(|error| error.to_string())
}

/// Writes one visible slot for the camp menu's STATE · SAVE.
///
/// The resolver's refusal comes back before any filesystem access, so a save
/// never writes into a directory the run did not name.
pub(crate) fn save_slot(runtime: &Runtime, slot: usize) -> Result<PathBuf, String> {
    let directory = save_directory().map_err(|error| error.to_string())?;
    runtime
        .save_slot(&directory, slot)
        .map_err(|error| error.to_string())
}

/// Performs the title's destructive ERASE DATA for one visible slot.
///
/// The resolver's refusal comes back before any filesystem access, so an
/// erase never falls back to a directory the run did not name.
pub(crate) fn erase_slot(slot: usize) -> Result<PathBuf, String> {
    let directory = save_directory().map_err(|error| error.to_string())?;
    Runtime::erase_slot(&directory, slot).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN_DIR: &str = "/tmp/psiv-run/saves";

    fn scripted() -> [&'static str; 2] {
        ["--script", "/repo/tools/native/native_continue.gd"]
    }

    #[test]
    fn scripted_run_without_the_variable_is_refused() {
        let error = resolve_save_directory(&scripted(), None).expect_err("refused");
        let message = error.to_string();
        assert!(message.contains(SAVE_DIR_ENV), "{message}");
        assert!(message.contains("copy the source save"), "{message}");
    }

    #[test]
    fn scripted_run_with_an_empty_variable_is_refused() {
        let error = resolve_save_directory(&scripted(), Some(OsStr::new(""))).expect_err("refused");
        assert_eq!(error, SaveDirError);
    }

    #[test]
    fn scripted_run_uses_the_named_directory() {
        let directory =
            resolve_save_directory(&scripted(), Some(OsStr::new(RUN_DIR))).expect("named");
        assert_eq!(directory, PathBuf::from(RUN_DIR));
    }

    #[test]
    fn interactive_run_falls_back_to_the_default() {
        let arguments = ["--path", "godot"];
        let directory = resolve_save_directory(&arguments, None).expect("default");
        assert_eq!(directory, PathBuf::from(DEFAULT_SAVE_DIR));
    }

    #[test]
    fn interactive_run_uses_the_named_directory() {
        let arguments = ["--path", "godot"];
        let directory =
            resolve_save_directory(&arguments, Some(OsStr::new(RUN_DIR))).expect("named");
        assert_eq!(directory, PathBuf::from(RUN_DIR));
    }
}
