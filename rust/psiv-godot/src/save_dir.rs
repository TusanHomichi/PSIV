//! The one save-directory resolver.
//!
//! Every interactive vanilla launch uses Godot's per-OS user data directory
//! (`<user_data>/saves`). When one or more mods are active, interactive
//! launches automatically isolate into `<user_data>/saves/mods/<profile>/`
//! (keyed by mod IDs in load order, not versions) so modded runs can never
//! overwrite vanilla saves. A scripted run must explicitly name a separate
//! directory with `PSIV_SAVE_DIR` before any save access; an explicit
//! `PSIV_SAVE_DIR` also overrides both vanilla and modded interactive paths.

use godot::prelude::{godot_error, godot_print, godot_warn};
use psiv_data::mods::{ModManifest, ModProfile, ModProfileDiff, mod_save_profile_slug};
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

/// The save directory a run with `arguments`, `save_dir`, and `active_mods`
/// uses; `default_saves` is Godot's user data directory with the `saves`
/// subdirectory.
///
/// Pure: no environment or filesystem access, so the rule is directly
/// testable. An empty `save_dir` counts as unset — `PSIV_SAVE_DIR=` must not
/// silently become the working directory. When `save_dir` is unset and
/// `active_mods` is non-empty, returns `<default_saves>/mods/<profile_slug>` so
/// modded saves stay isolated from vanilla `saves/slot_1..3.sram`.
pub(crate) fn resolve_save_directory_for_mods<Argument: AsRef<OsStr>>(
    arguments: &[Argument],
    save_dir: Option<&OsStr>,
    default_saves: &Path,
    active_mods: &[ModManifest],
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
    if let Some(slug) = mod_save_profile_slug(active_mods) {
        return Ok(default_saves.join("mods").join(slug));
    }
    Ok(default_saves.to_path_buf())
}

/// Vanilla convenience wrapper over [`resolve_save_directory_for_mods`].
#[cfg(test)]
pub(crate) fn resolve_save_directory<Argument: AsRef<OsStr>>(
    arguments: &[Argument],
    save_dir: Option<&OsStr>,
    default_saves: &Path,
) -> Result<PathBuf, SaveDirError> {
    resolve_save_directory_for_mods(arguments, save_dir, default_saves, &[])
}

/// The run's save directory, or the rule that refuses to guess one.
///
/// The thin wrapper over `std::env`; every call site resolves through here and
/// reports the error instead of touching a fallback path.
pub(crate) fn save_directory_for_mods(
    active_mods: &[ModManifest],
) -> Result<PathBuf, SaveDirError> {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    resolve_save_directory_for_mods(
        &arguments,
        std::env::var_os(SAVE_DIR_ENV).as_deref(),
        &PathBuf::from(Os::singleton().get_user_data_dir().to_string()).join("saves"),
        active_mods,
    )
}

/// In-game notice when a save directory's recorded `mod_profile.json` differs
/// from the currently active mods or cannot be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SaveProfileNotice {
    Diff(ModProfileDiff),
    Unreadable(String),
}

/// Result of resolving the run's save store and checking `mod_profile.json`.
pub(crate) struct ResolvedSaveStore {
    pub(crate) store: Option<SaveStore>,
    pub(crate) notice: Option<SaveProfileNotice>,
}

/// Compares `active_mods` against any existing `mod_profile.json` in
/// `directory`. Pure filesystem read: returns `None` when no sidecar exists or
/// when the active mod list matches the sidecar exactly.
pub(crate) fn inspect_save_profile(
    directory: &Path,
    active_mods: &[ModManifest],
) -> Option<SaveProfileNotice> {
    match ModProfile::read_from_dir(directory) {
        Ok(Some(previous)) => {
            let current = ModProfile::from_manifests(active_mods);
            let diff = current.diff_against(&previous);
            (!diff.is_empty()).then_some(SaveProfileNotice::Diff(diff))
        }
        Ok(None) => None,
        Err(error) => Some(SaveProfileNotice::Unreadable(error.to_string())),
    }
}

/// The run's save directory as a store, plus any `mod_profile.json` mismatch
/// notice to surface in-game.
///
/// The policy is the shell's (`save_dir.rs`): a scripted run that named no
/// directory gets no store, and the session refuses every slot operation
/// instead of falling back to a default path. Modded runs isolate into
/// `<saves>/mods/<profile>/` unless `PSIV_SAVE_DIR` overrides it, inspect any
/// existing `mod_profile.json` sidecar for version or mod-set changes, and
/// attach the active [`ModProfile`] so the sidecar stays current on save.
pub(crate) fn session_store(active_mods: &[ModManifest]) -> ResolvedSaveStore {
    match save_directory_for_mods(active_mods) {
        Ok(directory) => {
            let notice = inspect_save_profile(&directory, active_mods);
            if let Some(notice) = &notice {
                match notice {
                    SaveProfileNotice::Diff(diff) => {
                        if !diff.added.is_empty() {
                            godot_warn!("mod save profile notice: added mods {:?}", diff.added);
                        }
                        if !diff.removed.is_empty() {
                            godot_warn!("mod save profile notice: removed mods {:?}", diff.removed);
                        }
                        if diff.order_changed {
                            godot_warn!("mod save profile notice: mod load order changed");
                        }
                        for (id, prev, curr) in &diff.version_changes {
                            godot_print!(
                                "mod save profile notice: mod `{id}` version changed ({prev} -> {curr})"
                            );
                        }
                    }
                    SaveProfileNotice::Unreadable(error) => {
                        godot_warn!("could not read existing mod save profile: {error}");
                    }
                }
            }
            if active_mods.is_empty() {
                return ResolvedSaveStore {
                    store: Some(SaveStore::new(directory)),
                    notice,
                };
            }
            godot_print!("modded save directory: {}", directory.display());
            let profile = ModProfile::from_manifests(active_mods);
            ResolvedSaveStore {
                store: Some(SaveStore::new(directory).with_mod_profile(profile)),
                notice,
            }
        }
        Err(error) => {
            godot_error!("save slot scan refused: {error}");
            ResolvedSaveStore {
                store: None,
                notice: None,
            }
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

    #[test]
    fn modded_interactive_run_isolates_saves_by_mod_profile() {
        let single = [ModManifest::new("hard-mode", "Hard Mode", "1.0.0")];
        let dir = resolve_save_directory_for_mods(
            &["--main-pack", "psiv.pck"],
            None,
            Path::new(INSTALLED),
            &single,
        )
        .expect("isolated");
        assert_eq!(dir, Path::new(INSTALLED).join("mods").join("hard-mode"));

        let combo = [
            ModManifest::new("alpha", "Alpha", "1.0.0"),
            ModManifest::new("hard-mode", "Hard Mode", "1.2.0"),
        ];
        let combo_dir = resolve_save_directory_for_mods(
            &["--main-pack", "psiv.pck"],
            None,
            Path::new(INSTALLED),
            &combo,
        )
        .expect("isolated combo");
        assert_eq!(
            combo_dir,
            Path::new(INSTALLED).join("mods").join("alpha+hard-mode")
        );
    }

    #[test]
    fn modded_run_still_honours_explicit_save_dir_and_refuses_unconfigured_scripts() {
        let mods = [ModManifest::new("hard-mode", "Hard Mode", "1.0.0")];
        let explicit = resolve_save_directory_for_mods(
            &["--main-pack", "psiv.pck"],
            Some(OsStr::new(RUN_DIR)),
            Path::new(INSTALLED),
            &mods,
        )
        .expect("explicit override");
        assert_eq!(explicit, PathBuf::from(RUN_DIR));

        let refused =
            resolve_save_directory_for_mods(&scripted(), None, Path::new(INSTALLED), &mods);
        assert_eq!(refused, Err(SaveDirError));
    }

    #[test]
    fn inspect_save_profile_detects_diffs_and_stays_quiet_when_matching_or_absent() {
        let dir = std::env::temp_dir().join(format!("psiv-save-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let alpha = ModManifest::new("alpha", "Alpha", "1.0.0");
        let beta = ModManifest::new("beta", "Beta", "1.0.0");

        // Negative control 1: no mod_profile.json -> no notice (vanilla and first-run modded).
        assert_eq!(inspect_save_profile(&dir, &[]), None);
        assert_eq!(
            inspect_save_profile(&dir, std::slice::from_ref(&alpha)),
            None
        );

        // Write a sidecar for [alpha v1.0.0, beta v1.0.0].
        ModProfile::from_manifests(&[alpha.clone(), beta.clone()])
            .write_to_dir(&dir)
            .unwrap();

        // Negative control 2: exact match produces no notice.
        assert_eq!(
            inspect_save_profile(&dir, &[alpha.clone(), beta.clone()]),
            None
        );

        // Version change + added + removed.
        let alpha_v2 = ModManifest::new("alpha", "Alpha", "2.0.0");
        let gamma = ModManifest::new("gamma", "Gamma", "1.0.0");
        let notice = inspect_save_profile(&dir, &[alpha_v2, gamma]).expect("diff notice");
        match notice {
            SaveProfileNotice::Diff(diff) => {
                assert_eq!(diff.added, vec!["gamma"]);
                assert_eq!(diff.removed, vec!["beta"]);
                assert_eq!(
                    diff.version_changes,
                    vec![("alpha".into(), "1.0.0".into(), "2.0.0".into())]
                );
                assert!(!diff.order_changed);
            }
            other => panic!("expected Diff, got {other:?}"),
        }

        // Reorder only.
        let reorder = inspect_save_profile(&dir, &[beta, alpha]).expect("reorder notice");
        assert!(matches!(reorder, SaveProfileNotice::Diff(diff) if diff.order_changed));

        // Corrupt JSON sidecar surfaces Unreadable notice.
        std::fs::write(dir.join(psiv_data::mods::MOD_PROFILE_FILE), "{bad-json").unwrap();
        let corrupt = inspect_save_profile(&dir, &[]).expect("corrupt notice");
        assert!(
            matches!(corrupt, SaveProfileNotice::Unreadable(msg) if msg.contains("invalid JSON"))
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
