//! Every tool that starts Godot judges the run's exit with the leak check
//! (#123): a tool under `tools/` that calls `godot_command(` also calls
//! `logged_exit_leaks(`, the `tools/godot.py` glue that runs `psiv-exit-leaks`.

use std::fs;
use std::path::{Path, PathBuf};

/// Callers of `godot_command` that do not judge Godot's output, and why.
const UNJUDGED: [&str; 2] = [
    // Defines both; its `run` hands Godot the terminal, so the output is the
    // player's, not captured.
    "tools/godot.py",
    // The release build exports with the editor and smoke-runs the release
    // template: packaging, not native verification (#123 scope).
    "tools/package/release.py",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether `text` has `call` outside a comment line.
fn calls(text: &str, call: &str) -> bool {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line.contains(call))
}

/// `name` when it starts Godot without the exit check.
fn unchecked_run(name: &str, text: &str) -> Option<String> {
    let starts = calls(text, "godot_command(");
    (!UNJUDGED.contains(&name) && starts && !calls(text, "logged_exit_leaks(")).then(|| name.into())
}

/// Every `.py` file under `dir`, as a path relative to the repository root.
fn python_files(dir: &Path, found: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "__pycache__") {
                python_files(&path, found);
            }
        } else if path.extension().is_some_and(|e| e == "py") {
            let relative = path.strip_prefix(root()).unwrap();
            found.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn tools() -> Vec<String> {
    let mut found = Vec::new();
    python_files(&root().join("tools"), &mut found);
    found
}

#[test]
fn every_tool_that_runs_godot_checks_its_exit() {
    let problems: Vec<String> = tools()
        .iter()
        .filter_map(|name| unchecked_run(name, &fs::read_to_string(root().join(name)).unwrap()))
        .collect();
    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn the_census_sees_the_verifiers() {
    // Not vacuous: the three verifiers are found, run Godot and are judged.
    let names = tools();
    for name in [
        "tools/certify.py",
        "tools/verify_native_tape.py",
        "tools/verify_native_flash_attack.py",
    ] {
        assert!(names.iter().any(|n| n == name), "{name} not found");
        let text = fs::read_to_string(root().join(name)).unwrap();
        assert!(calls(&text, "godot_command("), "{name}");
        assert!(calls(&text, "logged_exit_leaks("), "{name}");
    }
}

#[test]
fn negative_control_a_run_without_the_check_fails() {
    let text = "from tools.godot import godot_command\n\
                command = godot_command(run_root, '--headless')\n";
    assert_eq!(
        unchecked_run("a-new-verifier", text),
        Some("a-new-verifier".into())
    );
    let checked = format!("{text}failures = logged_exit_leaks(log)\n");
    assert_eq!(unchecked_run("a-new-verifier", &checked), None);
    // A check only named in a comment is no check.
    let commented = format!("{text}# failures = logged_exit_leaks(log)\n");
    assert_eq!(
        unchecked_run("a-new-verifier", &commented),
        Some("a-new-verifier".into())
    );
}
