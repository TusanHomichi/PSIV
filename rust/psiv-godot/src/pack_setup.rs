//! The screen a run shows when it has no usable runtime pack.
//!
//! PSIV ships none of the game's data: the player supplies the ROM and the
//! pack builder makes the pack from it. When the pack is missing, or one the
//! game cannot load (built by an older release, or damaged), `Field` shows
//! this screen instead of the title. The player picks the ROM (a file dialog,
//! a file dropped on the window, or one named on the command line), the
//! screen runs the builder
//! (`install::builder_command`) as a child process and shows its progress,
//! and a finished build reloads the scene, which loads the new pack. The
//! builder checks the ROM's hash and installs the pack whole, so a wrong file
//! or a failed build leaves any earlier pack as it was.
//!
//! The builder's side of this is its command line: it reports each finished
//! part of the pack on stdout as `progress <done>/<total> <part>`
//! ([`progress`]), and a failure as `<program>: error: <message>` with a
//! non-zero exit ([`failure`]).

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use godot::classes::box_container::AlignmentMode;
use godot::classes::control::LayoutPreset;
use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{
    Button, CanvasLayer, ColorRect, FileDialog, ICanvasLayer, Label, MarginContainer, Os,
    VBoxContainer,
};
use godot::global::HorizontalAlignment;
use godot::obj::Singleton;
use godot::prelude::*;
use godot::signal::ConnectHandle;

use crate::install::{BuilderCommand, Install, Platform, builder_command, checkout_instructions};

/// The builder's last lines a failed build shows when it named no error.
const FAILURE_LINES: usize = 12;

/// What a failed build tells the player: the builder's own error (the
/// command line's `prog: error: message`, as the ROM check reports a wrong
/// file), else its last lines.
pub(crate) fn failure(lines: &[String]) -> String {
    let errors: Vec<&str> = lines
        .iter()
        .filter_map(|line| line.split_once(": error: ").map(|(_, message)| message))
        .collect();
    if errors.is_empty() {
        lines[lines.len().saturating_sub(FAILURE_LINES)..].join("\n")
    } else {
        errors.join("\n")
    }
}

/// The builder's latest progress line, `progress <done>/<total> <part>`, as
/// (done, total, part).
pub(crate) fn progress(lines: &[String]) -> Option<(usize, usize, &str)> {
    lines.iter().rev().find_map(|line| {
        let rest = line.strip_prefix("progress ")?;
        let (count, part) = rest.split_once(' ').unwrap_or((rest, ""));
        let (done, total) = count.split_once('/')?;
        Some((done.parse().ok()?, total.parse().ok()?, part))
    })
}

/// The ROM named on the game's command line: the first argument Godot leaves
/// to the game (`OS.get_cmdline_args`, which holds none of the engine's own
/// switches) that is not a switch itself. Dropping a file on the game's
/// executable, or opening one with it, starts the game this way.
pub(crate) fn command_line_rom(arguments: &[String]) -> Option<PathBuf> {
    arguments
        .iter()
        .find(|argument| !argument.is_empty() && !argument.starts_with('-'))
        .map(PathBuf::from)
}

/// Windows: start the builder without a console window of its own.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A pack build in progress.
struct Build {
    child: Child,
    output: Arc<Mutex<Vec<String>>>,
    readers: Vec<JoinHandle<()>>,
    started: Instant,
    rom: PathBuf,
}

impl Build {
    fn start(command: &BuilderCommand, rom: PathBuf) -> std::io::Result<Self> {
        let mut process = Command::new(&command.program);
        process
            .args(&command.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            process.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = process.spawn()?;
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(collect(stdout, Arc::clone(&output)));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(collect(stderr, Arc::clone(&output)));
        }
        Ok(Build {
            child,
            output,
            readers,
            started: Instant::now(),
            rom,
        })
    }

    /// The builder's lines so far, both streams in arrival order.
    fn lines(&self) -> Vec<String> {
        self.output
            .lock()
            .map(|lines| lines.clone())
            .unwrap_or_default()
    }

    fn finish(mut self) -> (bool, Vec<String>) {
        let succeeded = self.child.wait().is_ok_and(|status| status.success());
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        let lines = self.lines();
        (succeeded, lines)
    }
}

impl Drop for Build {
    /// A build still running when the screen goes (the window closed) is
    /// stopped; the builder's next run removes what it left.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn collect(stream: impl Read + Send + 'static, into: Arc<Mutex<Vec<String>>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).split(b'\n').map_while(Result::ok) {
            let text = String::from_utf8_lossy(&line).trim_end().to_owned();
            if let Ok(mut lines) = into.lock() {
                lines.push(text);
            }
        }
    })
}

/// What the screen says: an installed game asks for the ROM, a checkout
/// gives the command that builds its pack.
/// `overridden` says `PSIV_RUNTIME_PACK` named `pack`.
pub(crate) fn introduction(
    pack: &Path,
    problem: Option<&str>,
    installed: bool,
    overridden: bool,
) -> String {
    let opening = match problem {
        None => "PSIV builds its graphics, sound, maps and text from a ROM image of the US \
                 Genesis release of Phantasy Star IV, which you supply. It ships none of the \
                 game's data."
            .to_owned(),
        Some(problem) => format!(
            "The game data PSIV built before can't be used ({problem}). Build it again from \
             your ROM."
        ),
    };
    if !installed {
        let named = overridden.then_some(pack);
        return format!("{opening}\n\n{}", checkout_instructions(named));
    }
    format!(
        "{opening}\n\nChoose your ROM file, or drop it on this window. PSIV checks that it is \
         the right one, then builds its data once.\n\nThe data goes to {}",
        pack.display()
    )
}

/// The no-pack screen. `Field` adds it in place of the game.
#[derive(GodotClass)]
#[class(base=CanvasLayer, no_init)]
pub(crate) struct PackSetup {
    base: Base<CanvasLayer>,
    install: Install,
    pack: PathBuf,
    overridden: bool,
    problem: Option<String>,
    status: Option<Gd<Label>>,
    choose: Option<Gd<Button>>,
    dialog: Option<Gd<FileDialog>>,
    dropped: Option<ConnectHandle>,
    build: Option<Build>,
}

impl PackSetup {
    /// The screen for `pack` (`overridden` when `PSIV_RUNTIME_PACK` named
    /// it); `problem` is why an existing pack failed to load, `None` when
    /// there is none.
    pub(crate) fn create(
        install: Install,
        pack: PathBuf,
        overridden: bool,
        problem: Option<String>,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| PackSetup {
            base,
            install,
            pack,
            overridden,
            problem,
            status: None,
            choose: None,
            dialog: None,
            dropped: None,
            build: None,
        })
    }

    fn label(text: &str, size: i32) -> Gd<Label> {
        let mut label = Label::new_alloc();
        label.set_text(text);
        label.set_autowrap_mode(AutowrapMode::WORD_SMART);
        label.set_horizontal_alignment(HorizontalAlignment::CENTER);
        label.add_theme_font_size_override("font_size", size);
        label
    }

    fn on_choose(&mut self) {
        if let Some(dialog) = self.dialog.as_mut() {
            dialog.popup_file_dialog();
        }
    }

    fn on_file_selected(&mut self, path: GString) {
        self.start(PathBuf::from(path.to_string()));
    }

    fn on_files_dropped(&mut self, files: PackedStringArray) {
        if let Some(path) = files.as_slice().first() {
            self.start(PathBuf::from(path.to_string()));
        }
    }

    fn start(&mut self, rom: PathBuf) {
        let Install::Installed { executable, .. } = &self.install else {
            return;
        };
        if self.build.is_some() {
            return;
        }
        let command = builder_command(executable, Platform::current(), &rom, &self.pack);
        godot_print!(
            "pack setup: building {} from {} with {}",
            self.pack.display(),
            rom.display(),
            command.program.display()
        );
        match Build::start(&command, rom) {
            Ok(build) => {
                self.build = Some(build);
                if let Some(choose) = self.choose.as_mut() {
                    choose.set_disabled(true);
                }
            }
            Err(error) => self.show(&format!(
                "The pack builder could not start ({}: {error}).",
                command.program.display()
            )),
        }
    }

    fn show(&mut self, text: &str) {
        if let Some(status) = self.status.as_mut() {
            status.set_text(text);
        }
    }
}

#[godot_api]
impl ICanvasLayer for PackSetup {
    fn ready(&mut self) {
        let mut backdrop = ColorRect::new_alloc();
        backdrop.set_color(Color::from_rgb(0.05, 0.05, 0.08));
        backdrop.set_anchors_preset(LayoutPreset::FULL_RECT);
        self.base_mut().add_child(&backdrop);

        let mut margin = MarginContainer::new_alloc();
        margin.set_anchors_preset(LayoutPreset::FULL_RECT);
        for side in ["margin_left", "margin_right", "margin_top", "margin_bottom"] {
            margin.add_theme_constant_override(side, 96);
        }
        let mut column = VBoxContainer::new_alloc();
        column.set_alignment(AlignmentMode::CENTER);
        column.add_theme_constant_override("separation", 28);
        column.add_child(&Self::label("PSIV needs your copy of the game", 32));
        let installed = self.install.user_data().is_some();
        column.add_child(&Self::label(
            &introduction(
                &self.pack,
                self.problem.as_deref(),
                installed,
                self.overridden,
            ),
            20,
        ));
        if installed {
            self.add_picker(&mut column);
        }
        margin.add_child(&column);
        self.base_mut().add_child(&margin);
        // Focus once the button is in the tree, so a controller starts on it.
        if let Some(choose) = self.choose.as_mut() {
            choose.grab_focus();
        }
        godot_print!(
            "pack setup: no usable runtime pack at {}; asking for the ROM",
            self.pack.display()
        );
        if installed {
            let arguments: Vec<String> = Os::singleton()
                .get_cmdline_args()
                .as_slice()
                .iter()
                .map(GString::to_string)
                .collect();
            if let Some(rom) = command_line_rom(&arguments) {
                self.start(rom);
            }
        }
    }

    fn process(&mut self, _delta: f64) {
        self.poll();
    }

    fn exit_tree(&mut self) {
        if let Some(handle) = self.dropped.take() {
            handle.disconnect();
        }
        self.build = None;
    }
}

impl PackSetup {
    /// The ROM picker: a button that opens the file browser (the system's
    /// where there is one, Godot's own otherwise, which a controller can
    /// drive), the browser, a status line, and dropping a file on the window.
    fn add_picker(&mut self, column: &mut Gd<VBoxContainer>) {
        let mut choose = Button::new_alloc();
        choose.set_text("Choose your ROM file");
        choose.add_theme_font_size_override("font_size", 22);
        choose.set_h_size_flags(godot::classes::control::SizeFlags::SHRINK_CENTER);
        choose
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::on_choose);
        column.add_child(&choose);

        let status = Self::label("", 20);
        column.add_child(&status);

        let mut dialog = FileDialog::new_alloc();
        dialog.set_title("Choose your Phantasy Star IV ROM");
        dialog.set_file_mode(FileMode::OPEN_FILE);
        dialog.set_access(Access::FILESYSTEM);
        dialog.set_use_native_dialog(true);
        dialog
            .add_filter_ex("*.md, *.bin, *.gen, *.smd")
            .description("Genesis ROM image")
            .done();
        dialog
            .signals()
            .file_selected()
            .connect_other(&self.to_gd(), Self::on_file_selected);
        self.base_mut().add_child(&dialog);

        if let Some(window) = self.base().get_window() {
            self.dropped = Some(
                window
                    .signals()
                    .files_dropped()
                    .connect_other(&self.to_gd(), Self::on_files_dropped),
            );
        }
        self.choose = Some(choose);
        self.status = Some(status);
        self.dialog = Some(dialog);
    }

    /// Follows a running build: its elapsed time while it runs; when it ends,
    /// the game (a reload loads the new pack) or the builder's error.
    fn poll(&mut self) {
        let Some(build) = self.build.as_mut() else {
            return;
        };
        match build.child.try_wait() {
            Ok(None) => {
                let seconds = build.started.elapsed().as_secs();
                let lines = build.lines();
                let text = match progress(&lines) {
                    Some((done, total, part)) => {
                        format!("Building {part} ({done} of {total})... {seconds} s")
                    }
                    None => format!("Building from {}... {seconds} s", build.rom.display()),
                };
                self.show(&text);
            }
            Ok(Some(_)) | Err(_) => {
                let Some(build) = self.build.take() else {
                    return;
                };
                let (succeeded, lines) = build.finish();
                for line in &lines {
                    godot_print!("pack builder: {line}");
                }
                if succeeded {
                    self.show("Built. Starting the game...");
                    self.base().get_tree().reload_current_scene();
                } else {
                    self.show(&format!("The build failed:\n{}", failure(&lines)));
                    if let Some(choose) = self.choose.as_mut() {
                        choose.set_disabled(false);
                        choose.grab_focus();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_run_explains_the_rom_and_names_the_data_folder() {
        let text = introduction(Path::new("/data/runtime-pack"), None, true, false);
        assert!(text.contains("which you supply"), "{text}");
        assert!(text.contains("ships none of the game's data"), "{text}");
        assert!(text.ends_with("/data/runtime-pack"), "{text}");
    }

    #[test]
    fn a_failed_build_shows_the_builders_error_not_its_usage() {
        let lines = [
            "progress 1/9 sound",
            "psiv-pack: error: game.bin is not the US retail ROM PSIV builds from",
        ]
        .map(String::from);
        assert_eq!(
            failure(&lines),
            "game.bin is not the US retail ROM PSIV builds from"
        );
        let crash: Vec<String> = (0..20).map(|n| format!("line {n}")).collect();
        assert_eq!(failure(&crash), crash[8..].join("\n"));
    }

    #[test]
    fn a_rom_on_the_command_line_is_its_first_argument_that_is_not_a_switch() {
        let arguments = ["--verbose", "", "/roms/Game (USA).md", "/other.md"].map(String::from);
        assert_eq!(
            command_line_rom(&arguments),
            Some(PathBuf::from("/roms/Game (USA).md"))
        );
        assert_eq!(command_line_rom(&["--verbose".to_owned()]), None);
        assert_eq!(command_line_rom(&[]), None);
    }

    #[test]
    fn the_screen_follows_the_latest_progress_line() {
        let lines = [
            "progress 1/9 sound",
            "a warning",
            "progress 2/9 maps",
            "noise",
        ]
        .map(String::from);
        assert_eq!(progress(&lines), Some((2, 9, "maps")));
        let lines = ["progress", "progress x/9 maps", "progress 3/9"].map(String::from);
        assert_eq!(progress(&lines), Some((3, 9, "")));
        assert_eq!(progress(&[]), None);
    }

    #[cfg(unix)]
    fn shell(script: &str) -> BuilderCommand {
        BuilderCommand {
            program: PathBuf::from("sh"),
            arguments: vec!["-c".into(), script.into()],
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_build_collects_both_streams_and_its_exit_status() {
        let build = Build::start(&shell("echo built; echo warned >&2"), PathBuf::from("rom"))
            .expect("starts");
        let (succeeded, mut lines) = build.finish();
        lines.sort();
        assert!(succeeded);
        assert_eq!(lines, ["built", "warned"]);

        let build =
            Build::start(&shell("echo nope >&2; exit 2"), PathBuf::from("rom")).expect("starts");
        assert_eq!(build.finish(), (false, vec!["nope".to_owned()]));
    }

    #[test]
    fn a_missing_builder_is_an_error_to_show_not_a_panic() {
        let missing = BuilderCommand {
            program: PathBuf::from("/nonexistent/psiv-pack"),
            arguments: Vec::new(),
        };
        assert!(Build::start(&missing, PathBuf::from("rom")).is_err());
    }

    #[test]
    fn a_checkout_gets_the_command_and_no_picker_text() {
        let text = introduction(Path::new("/tmp/run-root/runtime-pack"), None, false, false);
        assert!(!text.contains("run-root"), "{text}");
        assert!(text.contains("python3 -m psiv_tools pack"), "{text}");
        assert!(!text.contains("Choose your ROM file"), "{text}");
    }

    #[test]
    fn an_unusable_pack_says_why_and_asks_for_a_rebuild() {
        let text = introduction(
            Path::new("/data/runtime-pack"),
            Some("format version 3"),
            true,
            false,
        );
        assert!(text.contains("can't be used (format version 3)"), "{text}");
        assert!(text.contains("Build it again"), "{text}");
    }
}
