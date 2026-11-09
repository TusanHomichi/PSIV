//! The one save-directory resolver.
//!
//! Every interactive launch uses Godot's per-OS user data directory. A
//! scripted run must explicitly name a separate directory with `PSIV_SAVE_DIR`
//! before any save access; the override also applies to interactive launches.

use godot::prelude::godot_error;
use psiv_runtime::SaveStore;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use godot::classes::Os;
use godot::obj::Singleton;

/// The environment variable that names a run's save directory.
const SAVE_DIR_ENV: &str = "PSIV_SAVE_DIR";

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

/// The save directory a run with `arguments` and `save_dir` uses;
/// `default_saves` is Godot's user data directory with the saves subdirectory.
///
/// Pure: no environment or filesystem access, so the rule is directly
/// testable. An empty `save_dir` counts as unset — `PSIV_SAVE_DIR=` must not
/// silently become the working directory.
pub(crate) fn resolve_save_directory<Argument: AsRef<OsStr>>(
    arguments: &[Argument],
    save_dir: Option<&OsStr>,
    default_saves: &Path,
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
    Ok(default_saves.to_path_buf())
}

/// The run's save directory, or the rule that refuses to guess one.
///
/// The thin wrapper over `std::env`; every call site resolves through here and
/// reports the error instead of touching a fallback path.
pub(crate) fn save_directory() -> Result<PathBuf, SaveDirError> {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    resolve_save_directory(
        &arguments,
        std::env::var_os(SAVE_DIR_ENV).as_deref(),
        &PathBuf::from(Os::singleton().get_user_data_dir().to_string()).join("saves"),
    )
}

/// The run's save directory as a store, or the logged refusal.
///
/// The policy is the shell's (`save_dir.rs`): a scripted run that named no
/// directory gets no store, and the session refuses every slot operation
/// instead of falling back to a default path. Resolved once per boot, so the
/// fixtures and the constructors share one answer.
pub(crate) fn session_store() -> Option<SaveStore> {
    match save_directory() {
        Ok(directory) => Some(SaveStore::new(directory)),
        Err(error) => {
            godot_error!("save slot scan refused: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN_DIR: &str = "/tmp/psiv-run/saves";
    const INSTALLED: &str = "/home/player/.local/share/PSIV/saves";

    fn scripted() -> [&'static str; 2] {
        ["--script", "/repo/tools/native/native_continue.gd"]
    }

    #[test]
    fn scripted_run_without_the_variable_is_refused() {
        let error =
            resolve_save_directory(&scripted(), None, Path::new(INSTALLED)).expect_err("refused");
        let message = error.to_string();
        assert!(message.contains(SAVE_DIR_ENV), "{message}");
        assert!(message.contains("copy the source save"), "{message}");
    }

    #[test]
    fn scripted_run_with_an_empty_variable_is_refused() {
        let error = resolve_save_directory(&scripted(), Some(OsStr::new("")), Path::new(INSTALLED))
            .expect_err("refused");
        assert_eq!(error, SaveDirError);
    }

    #[test]
    fn scripted_run_uses_the_named_directory() {
        let directory =
            resolve_save_directory(&scripted(), Some(OsStr::new(RUN_DIR)), Path::new(INSTALLED))
                .expect("named");
        assert_eq!(directory, PathBuf::from(RUN_DIR));
    }

    #[test]
    fn interactive_checkout_run_uses_godot_user_data() {
        let arguments = ["--path", "godot"];
        let directory =
            resolve_save_directory(&arguments, None, Path::new(INSTALLED)).expect("default");
        assert_eq!(directory, PathBuf::from(INSTALLED));
    }

    #[test]
    fn interactive_run_uses_the_named_directory() {
        let arguments = ["--path", "godot"];
        let directory =
            resolve_save_directory(&arguments, Some(OsStr::new(RUN_DIR)), Path::new(INSTALLED))
                .expect("named");
        assert_eq!(directory, PathBuf::from(RUN_DIR));
    }

    #[test]
    fn save_paths_preserve_spaces_parentheses_and_unicode() {
        let named = OsStr::new("player data/é 日本 (saves)");
        assert_eq!(
            resolve_save_directory(&scripted(), Some(named), Path::new(INSTALLED)),
            Ok(PathBuf::from(named))
        );
        assert_eq!(
            resolve_save_directory(&["--path", "godot"], None, Path::new(named)),
            Ok(PathBuf::from(named))
        );
    }

    #[test]
    fn installed_game_saves_in_its_user_data_folder() {
        let arguments = ["--main-pack", "psiv.pck"];
        let directory =
            resolve_save_directory(&arguments, None, Path::new(INSTALLED)).expect("installed");
        assert_eq!(directory, PathBuf::from(INSTALLED));
    }

    #[test]
    fn installed_game_still_honours_the_variable_and_the_scripted_rule() {
        let named = resolve_save_directory(
            &["--main-pack", "psiv.pck"],
            Some(OsStr::new(RUN_DIR)),
            Path::new(INSTALLED),
        )
        .expect("named");
        assert_eq!(named, PathBuf::from(RUN_DIR));
        let refused = resolve_save_directory(&scripted(), None, Path::new(INSTALLED));
        assert_eq!(refused, Err(SaveDirError));
    }
}
