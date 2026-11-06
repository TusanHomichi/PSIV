//! Godot's leak reports at exit fail a native verification run (#123).
//!
//! The report lines below are typed by hand from Godot 4.7.1's own format
//! strings (core/object/object.cpp `ObjectDB::cleanup`, core/io/resource.cpp
//! `ResourceCache::clear`, core/templates/rid_owner.h,
//! core/templates/paged_allocator.h), not from the marks under test. The
//! negative controls are each report form, the singular ObjectDB line a
//! plural-only mark would miss, a leaked class other than the one #123 saw,
//! and the lines of a clean run a loose mark would take for a report.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use psiv_exit_leaks::{exit_leaks, logged_exit_leaks};

/// The tail of a clean headless boot, with the lines a leak mark must not take
/// for a report: other warnings, a verbose StringName count, and words like
/// "leak" and "exit" in the game's own output.
const CLEAN: &str = "\
PSIV field ready: map 0x000, party at (12, 9)
WARNING: some unrelated warning
     at: push_warning (core/variant/variant_utility.cpp:1034)
native tape: no leak measured; exit at frame 240
StringName: 12 unclaimed string names at exit.
";

/// The #123 report as a verbose headless boot printed it (2026-11-05).
const AUDIO_LEAK: &str = "\
WARNING: 1 ObjectDB instance was leaked at exit (run with `--verbose` for details).
     at: cleanup (core/object/object.cpp:2536)
Leaked instance: AudioStreamGeneratorPlayback:9223372118341717473 - Reference count: 1
Hint: Leaked instances typically happen when nodes are removed from the scene tree (with `remove_child()`) but not freed (with `free()` or `queue_free()`).
";

const SINGULAR: &str =
    "WARNING: 1 ObjectDB instance was leaked at exit (run with `--verbose` for details).";
const AUDIO_INSTANCE: &str =
    "Leaked instance: AudioStreamGeneratorPlayback:9223372118341717473 - Reference count: 1";

#[test]
fn a_clean_exit_reports_nothing() {
    assert_eq!(exit_leaks(CLEAN), Vec::<String>::new());
}

#[test]
fn the_audio_playback_leak_fails() {
    assert_eq!(
        exit_leaks(&format!("{CLEAN}{AUDIO_LEAK}")),
        [SINGULAR, AUDIO_INSTANCE]
    );
}

#[test]
fn negative_control_the_singular_line_alone_fails() {
    // Without --verbose only this line prints; "instances were leaked" misses it.
    assert_eq!(exit_leaks(SINGULAR), [SINGULAR]);
}

#[test]
fn negative_control_every_report_form_fails() {
    for line in [
        "WARNING: 3 ObjectDB instances were leaked at exit (run with `--verbose` for details).",
        "Leaked instance: Sprite2D:31826181399 - Node name: Body",
        "Leaked instance: Image:-9223371990122233314",
        "ERROR: 2 resources still in use at exit (run with --verbose for details).",
        "ERROR: 2 resources still in use at exit.",
        "Resource still in use: res://field.tscn (PackedScene)",
        "ERROR: 4 RID allocations of type 'N10RendererRD12TextureStorage7TextureE' were leaked at exit.",
        "ERROR: Pages in use exist at exit in PagedAllocator: N13GodotBody2DE",
    ] {
        assert_eq!(exit_leaks(&format!("{CLEAN}{line}\n")), [line], "{line}");
    }
}

#[test]
fn report_lines_are_trimmed() {
    assert_eq!(exit_leaks(&format!("  {SINGULAR}\r\n")), [SINGULAR]);
}

/// A fresh directory for one test's logs.
fn log_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("exit-leaks-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, text: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn clean_logs_and_missing_logs_pass() {
    let dir = log_dir("clean");
    let logs = [
        write(&dir, "run.log", CLEAN.as_bytes()),
        dir.join("never-written.log"),
    ];
    assert_eq!(logged_exit_leaks(&logs).unwrap(), Vec::<String>::new());
}

#[test]
fn a_report_in_either_log_fails_once() {
    // Godot writes its report to stderr and to --log-file: one line each.
    let dir = log_dir("either");
    let run = write(&dir, "run.log", format!("{CLEAN}{AUDIO_LEAK}").as_bytes());
    let log = write(&dir, "godot.log", AUDIO_LEAK.as_bytes());
    assert_eq!(
        logged_exit_leaks(&[&run, &log]).unwrap(),
        [SINGULAR, AUDIO_INSTANCE]
    );
    let only = write(&dir, "only.log", CLEAN.as_bytes());
    assert_eq!(
        logged_exit_leaks(&[&only, &log]).unwrap(),
        exit_leaks(AUDIO_LEAK)
    );
}

#[test]
fn bytes_that_are_not_utf8_do_not_hide_a_report() {
    let dir = log_dir("lossy");
    let mut bytes = b"\xff\xfe garbage\n".to_vec();
    bytes.extend_from_slice(SINGULAR.as_bytes());
    let log = write(&dir, "run.log", &bytes);
    assert_eq!(logged_exit_leaks(&[log]).unwrap(), [SINGULAR]);
}

fn run(args: &[&Path]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_psiv-exit-leaks"))
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

#[test]
fn the_binary_exits_0_on_a_clean_run() {
    let dir = log_dir("bin-clean");
    let log = write(&dir, "run.log", CLEAN.as_bytes());
    assert_eq!(
        run(&[&log, &dir.join("missing.log")]),
        (Some(0), String::new())
    );
}

#[test]
fn negative_control_the_binary_exits_1_and_prints_each_report_once() {
    let dir = log_dir("bin-leak");
    let run_log = write(&dir, "run.log", format!("{CLEAN}{AUDIO_LEAK}").as_bytes());
    let godot_log = write(&dir, "godot.log", AUDIO_LEAK.as_bytes());
    assert_eq!(
        run(&[&run_log, &godot_log]),
        (Some(1), format!("{SINGULAR}\n{AUDIO_INSTANCE}\n"))
    );
}

#[test]
fn negative_control_the_binary_refuses_no_logs() {
    assert_eq!(run(&[]).0, Some(2));
    // A directory exists but is not a log: skipped, as a log never written is.
    let dir = log_dir("bin-dir");
    assert_eq!(run(&[&dir]).0, Some(0));
}
