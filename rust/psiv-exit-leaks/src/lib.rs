//! Godot's own reports, at exit, of what it could not free: the one check
//! every native verifier applies to a Godot run's output (#123).
//!
//! Godot 4.7.1 prints them from `ObjectDB::cleanup` ("1 ObjectDB instance was
//! leaked at exit", "N ObjectDB instances were leaked at exit", and under
//! `--verbose` one "Leaked instance: Class:id" each), `ResourceCache::clear`
//! ("N resources still in use at exit", "Resource still in use: path
//! (Class)"), `RID_Alloc` ("N RID allocations of type 'T' were leaked at
//! exit") and `PagedAllocator` ("Pages in use exist at exit"). They name no
//! particular class, so any one of them fails a run. StringName's "unclaimed
//! string names at exit" is left out: it prints only under `--verbose`, about
//! names, not objects.
//!
//! The verifiers (`tools/certify.py`, `tools/verify_native_tape.py`,
//! `tools/verify_native_flash_attack.py`) run the `psiv-exit-leaks` binary
//! over a run's logs through `logged_exit_leaks` in `tools/godot.py`, which
//! only builds and runs it; `tests/census.rs` holds every tool that starts
//! Godot to that call.

use std::fs;
use std::io;
use std::path::Path;

/// The phrases, one per report form above, that mark a line as a leak report.
pub const REPORT_MARKS: [&str; 5] = [
    "leaked at exit",
    "Leaked instance: ",
    "still in use at exit",
    "Resource still in use: ",
    "Pages in use exist at exit",
];

/// Whether `line` is one of Godot's leak reports at exit.
pub fn is_exit_leak(line: &str) -> bool {
    REPORT_MARKS.iter().any(|mark| line.contains(mark))
}

/// The lines of a Godot run's output that report a leak at exit, trimmed, in
/// order; none for a clean exit.
pub fn exit_leaks(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| is_exit_leak(line))
        .map(|line| line.trim().to_owned())
        .collect()
}

/// `exit_leaks` over each of a run's logs that exists (its captured stdout and
/// stderr, its `--log-file`), each line once, in the order first seen. Godot
/// writes its report both to stderr and to its log file, so the same line in
/// two logs is one report. A log that was never written is skipped; one that
/// exists but cannot be read is an error, not a clean run. Bytes that are not
/// UTF-8 are replaced, never refused.
pub fn logged_exit_leaks<P: AsRef<Path>>(logs: &[P]) -> io::Result<Vec<String>> {
    let mut found: Vec<String> = Vec::new();
    for log in logs {
        let path = log.as_ref();
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(path)?;
        for line in exit_leaks(&String::from_utf8_lossy(&bytes)) {
            if !found.contains(&line) {
                found.push(line);
            }
        }
    }
    Ok(found)
}
