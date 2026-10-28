//! What kind of run this is, and where it keeps its runtime pack and saves.
//!
//! An installed game (an exported build, Godot's `template` feature) keeps the
//! pack and the saves in the per-user data folder Godot names for the project
//! (`config/custom_user_dir_name` in `project.godot`): a release folder may be
//! read-only, and a game started from a launcher has no useful working
//! directory. It builds the pack with `psiv-pack`, the pack builder the
//! release ships beside the game (#216). A checkout run (the editor binary, from a run root or the
//! editor) reads the checkout's `runtime-pack` and keeps the `saves/` default
//! in `save_dir.rs`. `PSIV_RUNTIME_PACK` names a pack for either.
//!
//! The rules are pure functions of their inputs, so they are tested here;
//! [`Install::current`] is the thin wrapper over Godot that feeds them.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use godot::classes::{Os, ProjectSettings};
use godot::obj::Singleton;

/// The environment variable that names a runtime pack directory.
const PACK_ENV: &str = "PSIV_RUNTIME_PACK";

/// The pack's directory name, in the user data folder and in a checkout.
const PACK_NAME: &str = "runtime-pack";

/// The installed game's save directory, in the user data folder.
const SAVES_NAME: &str = "saves";

/// The pack builder a release ships beside the game's executable.
const BUILDER_NAME: &str = "psiv-pack";

/// How this run was started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Install {
    /// An exported build: `user_data` is Godot's user data folder.
    Installed {
        user_data: PathBuf,
        executable: PathBuf,
    },
    /// The editor binary running the project: `root` is the directory that
    /// holds `godot/` (a checkout, or a run root).
    Checkout { root: PathBuf },
}

impl Install {
    /// This run's kind, from Godot.
    pub(crate) fn current() -> Self {
        let os = Os::singleton();
        if os.has_feature("template") {
            Install::Installed {
                user_data: PathBuf::from(os.get_user_data_dir().to_string()),
                executable: PathBuf::from(os.get_executable_path().to_string()),
            }
        } else {
            let root = ProjectSettings::singleton()
                .globalize_path("res://..")
                .to_string();
            Install::Checkout {
                root: PathBuf::from(root),
            }
        }
    }

    /// The user data folder of an installed game; `None` for a checkout.
    pub(crate) fn user_data(&self) -> Option<&Path> {
        match self {
            Install::Installed { user_data, .. } => Some(user_data),
            Install::Checkout { .. } => None,
        }
    }

    /// The pack this run reads, before `PSIV_RUNTIME_PACK` is applied.
    fn default_pack(&self) -> PathBuf {
        match self {
            Install::Installed { user_data, .. } => user_data.join(PACK_NAME),
            Install::Checkout { root } => root.join(PACK_NAME),
        }
    }

    /// The installed game's save directory; `None` for a checkout, which
    /// keeps `save_dir.rs`'s working-directory default.
    pub(crate) fn installed_saves(&self) -> Option<PathBuf> {
        self.user_data().map(|user_data| user_data.join(SAVES_NAME))
    }
}

/// The runtime pack a run reads: `PSIV_RUNTIME_PACK` when it is set and not
/// empty, else the install's own.
pub(crate) fn resolve_pack_directory(override_dir: Option<&OsStr>, install: &Install) -> PathBuf {
    match override_dir.filter(|value| !value.is_empty()) {
        Some(directory) => PathBuf::from(directory),
        None => install.default_pack(),
    }
}

/// The run's pack directory, and whether `PSIV_RUNTIME_PACK` chose it.
pub(crate) fn pack_directory(install: &Install) -> (PathBuf, bool) {
    let override_dir = std::env::var_os(PACK_ENV).filter(|value| !value.is_empty());
    (
        resolve_pack_directory(override_dir.as_deref(), install),
        override_dir.is_some(),
    )
}

/// The command that builds the pack at `pack` from the ROM at `rom`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BuilderCommand {
    pub(crate) program: PathBuf,
    pub(crate) arguments: Vec<OsString>,
}

/// The operating system a release was built for, which names its programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    pub(crate) fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

/// How an installed game at `executable` runs its pack builder on `platform`.
///
/// A release ships `psiv-pack` in the executable's own folder (in a macOS
/// app, `Contents/MacOS`), so a player needs nothing else installed. It takes
/// the ROM and the pack directory. A checkout has no such command: its pack
/// is built from the checkout ([`checkout_instructions`]).
pub(crate) fn builder_command(
    executable: &Path,
    platform: Platform,
    rom: &Path,
    pack: &Path,
) -> BuilderCommand {
    let beside = executable.parent().unwrap_or(Path::new("."));
    let program = match platform {
        Platform::Windows => beside.join(format!("{BUILDER_NAME}.exe")),
        Platform::Linux | Platform::MacOs => beside.join(BUILDER_NAME),
    };
    BuilderCommand {
        program,
        arguments: vec![rom.as_os_str().to_owned(), pack.as_os_str().to_owned()],
    }
}

/// What a checkout run with no usable pack tells its developer: the run
/// cannot find the checkout's `psiv_tools` (a run root holds only the
/// project and the library), so the pack is built the documented way, into
/// `named` (the directory `PSIV_RUNTIME_PACK` names) or the checkout's own.
pub(crate) fn checkout_instructions(named: Option<&Path>) -> String {
    let pack = named.map_or_else(|| PACK_NAME.to_owned(), |path| path.display().to_string());
    format!(
        "Build it from your checkout's root, then start the game again:\n\n\
         python3 -m psiv_tools pack /path/to/your/ROM {pack}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed() -> Install {
        Install::Installed {
            user_data: PathBuf::from("/home/player/.local/share/PSIV"),
            executable: PathBuf::from("/games/psiv/psiv.x86_64"),
        }
    }

    fn checkout() -> Install {
        Install::Checkout {
            root: PathBuf::from("/src/PSIV"),
        }
    }

    #[test]
    fn an_installed_game_reads_the_pack_in_its_user_data_folder() {
        assert_eq!(
            resolve_pack_directory(None, &installed()),
            PathBuf::from("/home/player/.local/share/PSIV/runtime-pack")
        );
    }

    #[test]
    fn a_checkout_reads_its_own_runtime_pack() {
        assert_eq!(
            resolve_pack_directory(None, &checkout()),
            PathBuf::from("/src/PSIV/runtime-pack")
        );
    }

    #[test]
    fn the_variable_names_the_pack_for_either_and_empty_counts_as_unset() {
        for install in [installed(), checkout()] {
            assert_eq!(
                resolve_pack_directory(Some(OsStr::new("/tmp/pack")), &install),
                PathBuf::from("/tmp/pack")
            );
            assert_eq!(
                resolve_pack_directory(Some(OsStr::new("")), &install),
                install.default_pack()
            );
        }
    }

    #[test]
    fn only_an_installed_game_keeps_its_saves_in_the_user_data_folder() {
        assert_eq!(
            installed().installed_saves(),
            Some(PathBuf::from("/home/player/.local/share/PSIV/saves"))
        );
        assert_eq!(checkout().installed_saves(), None);
    }

    fn command(executable: &str, platform: Platform) -> BuilderCommand {
        builder_command(
            Path::new(executable),
            platform,
            Path::new("/roms/game.md"),
            Path::new("/data/runtime-pack"),
        )
    }

    #[test]
    fn a_release_runs_the_builder_beside_its_executable() {
        let linux = command("/games/psiv/psiv.x86_64", Platform::Linux);
        assert_eq!(linux.program, PathBuf::from("/games/psiv/psiv-pack"));

        let mac = command(
            "/Applications/PSIV.app/Contents/MacOS/PSIV",
            Platform::MacOs,
        );
        assert_eq!(
            mac.program,
            PathBuf::from("/Applications/PSIV.app/Contents/MacOS/psiv-pack")
        );

        let windows = command("C:/Games/PSIV/PSIV.exe", Platform::Windows);
        assert_eq!(
            windows.program,
            PathBuf::from("C:/Games/PSIV/psiv-pack.exe")
        );
    }

    #[test]
    fn a_checkout_is_told_the_documented_command() {
        let text = checkout_instructions(None);
        assert!(
            text.ends_with("python3 -m psiv_tools pack /path/to/your/ROM runtime-pack"),
            "{text}"
        );
        let text = checkout_instructions(Some(Path::new("/mods/pack")));
        assert!(
            text.ends_with("python3 -m psiv_tools pack /path/to/your/ROM /mods/pack"),
            "{text}"
        );
    }

    #[test]
    fn the_builder_gets_the_rom_then_the_pack() {
        let built = command("/games/psiv/psiv.x86_64", Platform::Linux);
        let words: Vec<&OsStr> = built.arguments.iter().map(OsString::as_os_str).collect();
        assert_eq!(
            words,
            ["/roms/game.md", "/data/runtime-pack"].map(OsStr::new)
        );
    }
}
