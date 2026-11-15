//! The temporary-object palette driver's failures are final:
//! `tools/native/native_temporary_palette.gd`.
//!
//!     cargo test --manifest-path rust/Cargo.toml -p psiv-native-verify --test temporary_palette_driver
//!
//! The real driver runs against a stub `field.tscn` (no extension, pack or
//! ROM): a game whose `debug_play_state` replays a scripted CRAM, Igglanova's
//! sprite and a camera. Unfaulted, the driver captures `before` and `during`
//! and exits 0. Each negative control faults one validation the driver makes:
//! the copy on the build tick, line 3 or a sheet pixel (alpha included) before
//! the first capture, and line 3 or a sheet pixel while line 1 cycles. Each
//! must exit nonzero, and no capture may follow the failure, so the `during`
//! capture's success exit cannot overwrite it.
//!
//! Godot comes from the repository's one resolver, `tools/godot.py` (its CLI
//! prints the pinned executable), and every Godot run takes one `godot` slot
//! through `tools/heavy_lock.py run --class godot`. Under a holder that already
//! covers it (the gate's exclusive hold, or a `godot`-class run) that request
//! is a no-op; the runs here go one at a time either way. Skipped, with the
//! reason on stderr, without the pinned Godot or `xvfb-run` (the viewport
//! capture needs a renderer), or under a holder of another class (`build`,
//! `extension`, `python`, `pack`), which reserved no Godot slot and which the
//! lock would refuse.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const DRIVER: &str = "tools/native/native_temporary_palette.gd";
/// How long one Godot run may take once it holds its slot.
const TIMEOUT: &str = "180";

/// The stub game. Each `debug_play_state` call is one driver tick: ticks 1-2
/// show the map's line 3 and no object; tick 3 copies line 3 and builds
/// Igglanova (copy fault: no copy); tick 4 is the first on-screen tick, the
/// `before` capture; tick 6 cycles line 1 (words 16-19), the `during` capture.
/// A fault named `PHASE-line3` or `PHASE-pixels` changes word 50 or one opaque
/// pixel's alpha from that phase's tick on.
const STUB: &str = r#"extends Node2D

const RAMP := [0, 32, 65, 98, 139, 172, 205, 238]
var fault := OS.get_environment("PSIV_STUB_FAULT")
var calls := 0
var revision := 1
var words := []
var sprite: Sprite2D

func _ready():
    for i in 64:
        words.append(0x002 * ((i % 7) + 1))
    var camera := Camera2D.new()
    camera.position = Vector2(480, 190)
    add_child(camera)
    camera.make_current()

func word(i: int) -> int:
    return ((i & 7) << 1) | (((i >> 3) & 1) << 9) | 0x0E0

func rgba(w: int) -> Color:
    return Color8(RAMP[(w >> 1) & 7], RAMP[(w >> 5) & 7], RAMP[(w >> 9) & 7], 255)

func sheet(hole: bool) -> ImageTexture:
    var image := Image.create(64, 32, false, Image.FORMAT_RGBA8)
    for y in 32:
        for x in 64:
            var index := (x + y) % 14
            if index != 0:
                image.set_pixel(x, y, rgba(words[48 + index]))
    if hole:
        image.set_pixel(1, 0, Color8(0, 0, 0, 0))
    return ImageTexture.create_from_image(image)

func at(kind: String) -> bool:
    var phase := {"before": 4, "during": 6}
    for name in phase:
        if fault == name + "-" + kind and calls >= phase[name]:
            return true
    return false

func debug_play_state() -> String:
    calls += 1
    if calls == 3 and fault != "copy":
        for i in range(48, 62):
            words[i] = word(i - 48)
        revision += 1
    if calls == 3:
        sprite = Sprite2D.new()
        sprite.centered = false
        sprite.position = Vector2(470, 180)
        sprite.region_enabled = true
        sprite.region_rect = Rect2(0, 0, 32, 32)
        sprite.texture = sheet(false)
        add_child(sprite)
    if calls == 6:
        for i in range(16, 20):
            words[i] ^= 0x002
        revision += 1
    if at("line3") and words[50] == word(2):
        words[50] ^= 0x200
        revision += 1
    if at("pixels") and sprite.texture.get_image().get_pixel(1, 0).a > 0:
        sprite.texture = sheet(true)
    var objects := [{"id": 0x188}] if calls >= 3 else []
    return JSON.stringify({"temporary_objects": objects,
        "cram": {"live": true, "revision": revision, "words": words}})
"#;

const SCENE: &str = r#"[gd_scene load_steps=2 format=3]

[ext_resource type="Script" path="res://stub_field.gd" id="1"]

[node name="Field" type="Node2D"]
script = ExtResource("1")
"#;

const PROJECT: &str = "[application]\nconfig/name=\"stub\"\n[rendering]\n\
renderer/rendering_method=\"gl_compatibility\"\n";

/// One Godot run at a time, whatever the harness's test threads.
static RUNS: Mutex<()> = Mutex::new(());
static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A throwaway directory, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Run {
    code: Option<i32>,
    log: String,
    output: PathBuf,
}

struct Harness {
    godot: String,
    xvfb: PathBuf,
    scratch: Scratch,
}

impl Harness {
    /// The pinned Godot, `xvfb-run` and a stub project, or why the test skips.
    fn new() -> Result<Self, String> {
        if let Ok(class) = std::env::var("PSIV_HEAVY_LOCK_CLASS")
            && std::env::var_os("PSIV_HEAVY_LOCK_HELD").is_some()
            && !matches!(class.as_str(), "" | "exclusive" | "godot")
        {
            return Err(format!(
                "under a {class}-class heavy-lock holder, which reserved no Godot slot"
            ));
        }
        let resolved = Command::new("python3")
            .arg("tools/godot.py")
            .current_dir(root())
            .output()
            .map_err(|error| format!("no python3 to resolve Godot: {error}"))?;
        if !resolved.status.success() {
            return Err(format!(
                "no pinned Godot: {}",
                String::from_utf8_lossy(&resolved.stderr).trim()
            ));
        }
        let godot = String::from_utf8_lossy(&resolved.stdout).trim().to_owned();
        if godot.is_empty() || godot.contains('\n') {
            return Err(format!("tools/godot.py printed no single path: {godot:?}"));
        }
        let xvfb = std::env::var_os("PATH")
            .iter()
            .flat_map(std::env::split_paths)
            .map(|dir| dir.join("xvfb-run"))
            .find(|path| path.is_file())
            .ok_or("xvfb-run is not installed")?;
        let dir = std::env::temp_dir().join(format!(
            "psiv-temporary-palette-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let scratch = Scratch(dir);
        let project = scratch.0.join("project");
        fs::create_dir_all(&project).map_err(|e| e.to_string())?;
        for (name, text) in [
            ("project.godot", PROJECT),
            ("stub_field.gd", STUB),
            ("field.tscn", SCENE),
        ] {
            fs::write(project.join(name), text).map_err(|e| e.to_string())?;
        }
        Ok(Self {
            godot,
            xvfb,
            scratch,
        })
    }

    fn run(&self, fault: &str) -> Run {
        let _one = RUNS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = &self.scratch.0;
        let output = dir.join(if fault.is_empty() { "clean" } else { fault });
        fs::create_dir_all(&output).expect("output directory");
        let log_path = dir.join(format!(
            "{}.log",
            if fault.is_empty() { "clean" } else { fault }
        ));
        let log = fs::File::create(&log_path).expect("log file");
        // The bound is inside the lock: time spent queued for a slot is not
        // the driver's. `timeout` exits 124 if Godot outlives it.
        let status = Command::new("python3")
            .args(["tools/heavy_lock.py", "run", "--class", "godot", "--"])
            .args(["timeout", "--kill-after=10", TIMEOUT])
            .arg(&self.xvfb)
            .arg("-a")
            .arg(&self.godot)
            .arg("--path")
            .arg(dir.join("project"))
            .args(["--rendering-driver", "opengl3", "--audio-driver", "Dummy"])
            .args(["--fixed-fps", "60", "--quit-after", "600", "--script"])
            .arg(root().join(DRIVER))
            .current_dir(root())
            .env("PSIV_STUB_FAULT", fault)
            .env("PSIV_TEMPORARY_OUTPUT", &output)
            .env("PSIV_SAVE_DIR", dir.join("saves"))
            .stdin(Stdio::null())
            .stdout(log.try_clone().expect("log handle"))
            .stderr(log)
            .status()
            .expect("start the driver");
        Run {
            code: status.code(),
            log: String::from_utf8_lossy(&fs::read(&log_path).expect("read log")).into_owned(),
            output,
        }
    }
}

fn harness() -> Option<Harness> {
    match Harness::new() {
        Ok(harness) => Some(harness),
        Err(reason) => {
            eprintln!("skipped: {reason}");
            None
        }
    }
}

/// `fault` exits 1 (the driver's `quit(1)`, not a timeout or a crash) with
/// `message`, prints exactly `captures` and leaves no `during` viewport behind.
fn assert_fails(fault: &str, message: &str, captures: &[&str]) {
    let Some(harness) = harness() else { return };
    let run = harness.run(fault);
    let log = &run.log;
    assert_eq!(run.code, Some(1), "{fault}: exit status\n{log}");
    assert!(log.contains(message), "{fault}: no {message:?}\n{log}");
    let printed: Vec<&str> = ["before", "during"]
        .into_iter()
        .filter(|label| log.contains(&format!("TEMPORARY_CAPTURE {label}")))
        .collect();
    assert_eq!(printed, captures, "{fault}: captures\n{log}");
    assert!(
        !run.output.join("during-viewport.png").exists(),
        "{fault}: a during capture followed the failure\n{log}"
    );
}

#[test]
fn an_unfaulted_run_captures_both_and_succeeds() {
    let Some(harness) = harness() else { return };
    let run = harness.run("");
    let log = &run.log;
    assert_eq!(run.code, Some(0), "{log}");
    for line in [
        "TEMPORARY_BUILT tick=3",
        "TEMPORARY_CAPTURE before tick=4",
        "TEMPORARY_CAPTURE during tick=6",
    ] {
        assert!(log.contains(line), "no {line:?}\n{log}");
    }
    for label in ["before", "during"] {
        assert!(
            run.output.join(format!("{label}-viewport.png")).is_file(),
            "{label}\n{log}"
        );
    }
}

#[test]
fn negative_control_no_copy_on_the_build_tick() {
    assert_fails(
        "copy",
        "line 3 did not change when Igglanova was built",
        &[],
    );
}

#[test]
fn negative_control_line_3_changes_before_the_capture() {
    assert_fails("before-line3", "line 3 changed before the capture", &[]);
}

#[test]
fn negative_control_a_pixel_changes_before_the_capture() {
    assert_fails("before-pixels", "the object changed with another line", &[]);
}

#[test]
fn negative_control_line_3_changes_with_line_1() {
    assert_fails("during-line3", "line 3 changed", &["before"]);
}

#[test]
fn negative_control_a_pixel_changes_with_line_1() {
    assert_fails(
        "during-pixels",
        "the object changed with another line",
        &["before"],
    );
}
