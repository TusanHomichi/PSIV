//! `psiv-pack`: the runtime pack, built from the ROM alone (#216).
//!
//! The port of `python3 -m psiv_tools pack`, a section at a time. Until the
//! switch, the Python builder is the one the game, the lanes and the gate
//! use, and this crate builds only the sections it claims ([`SECTIONS`]):
//! its output is not a pack the game can load. The full gate's parity step,
//! `tools/pack_parity.py`, builds both from the ROM and holds every claimed
//! path to the Python builder's bytes (a PNG to its decoded image).
//!
//! # The program
//!
//! `psiv-pack <rom> <pack>` checks the ROM ([`rom`]), builds every section
//! into a directory beside `<pack>` and installs it atomically
//! ([`install`]). Sections are independent and build in parallel. Its
//! output is the interface the setup screen reads:
//!
//! - stdout: `progress <done>/<total> <section>` as each section finishes,
//!   then `installed <pack>` once the pack is in place;
//! - stderr: `psiv-pack: error: <message>` and exit status 2 on any failure,
//!   with `<pack>` left as it was. A wrong ROM is refused in the Python
//!   builder's words ([`rom::wrong_rom`]). A problem in the cleanup after the
//!   new pack is in place is a `psiv-pack: warning: ...` line on stderr, and
//!   the exit status stays 0.
//!
//! `psiv-pack --claims` prints the sections and the pack-relative paths each
//! claims, as JSON ([`claims`]): a path names a file, a directory and
//! everything in it, or (`maps/*.png`) the matching files directly in a
//! directory.

// The sections hold no unsafe code; only the install's OS calls (liveness
// and the atomic exchange) allow it. Integer pack data stays integer.
#![deny(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

pub mod decode;
pub mod gfx;
pub mod install;
pub mod json;
pub mod m68k;
mod map_symbols;
pub mod maps;
pub mod png;
pub mod rom;
pub mod sound;
mod sound_defs;
pub mod sprites;
pub mod text;
pub mod warps;

pub use rom::Rom;

/// A failure that stops the build; the program prints it after
/// `psiv-pack: error: `.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackError(pub String);

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PackError {}

impl From<png::PngError> for PackError {
    fn from(error: png::PngError) -> Self {
        PackError(error.0)
    }
}

/// One independent part of the pack.
pub struct Section {
    /// The name the progress line shows.
    pub name: &'static str,
    /// The pack-relative paths it writes, all of them: the parity step
    /// fails on a file it writes outside these. A path names a file, a
    /// directory and everything in it, or with a `*` in its last part, the
    /// matching files directly in that directory.
    pub claims: &'static [&'static str],
    /// Write the section under the pack root.
    pub build: fn(&Rom, &Path) -> Result<(), PackError>,
}

/// The sections `psiv-pack` builds, in the order #216 ports them.
pub const SECTIONS: &[Section] = &[
    Section {
        name: "sound",
        claims: &["sound"],
        build: sound::build,
    },
    Section {
        name: "map_pictures",
        claims: &["maps/*.png"],
        build: maps::pictures::build,
    },
    Section {
        name: "sprites",
        claims: &["sprites"],
        build: sprites::emit::build,
    },
];

/// What `psiv-pack --claims` prints: `{"sections": [{"name": ..., "claims":
/// [...]}, ...]}`, every section in order with the paths it writes.
pub fn claims(sections: &[Section]) -> json::Json {
    let listed: Vec<json::Json> = sections
        .iter()
        .map(|s| {
            json::Json::object()
                .with("name", s.name)
                .with("claims", s.claims.to_vec())
        })
        .collect();
    json::Json::object().with("sections", listed)
}

/// Build `sections` from `rom` under `root`, on up to `workers` threads,
/// calling `progress(done, total, name)` on this thread as each finishes.
/// The first failure stops further sections from starting and is returned
/// once the running ones end; a section that panics is a failure too.
pub fn build_sections(
    rom: &Rom,
    root: &Path,
    sections: &[Section],
    workers: usize,
    mut progress: impl FnMut(usize, usize, &str),
) -> Result<(), PackError> {
    let total = sections.len();
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel::<(usize, Result<(), PackError>)>();
    std::thread::scope(|scope| {
        for _ in 0..workers.clamp(1, total.max(1)) {
            let sender = sender.clone();
            let (next, stop) = (&next, &stop);
            scope.spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(section) = sections.get(index) else {
                        break;
                    };
                    let result = std::panic::catch_unwind(|| (section.build)(rom, root))
                        .unwrap_or_else(|_| {
                            Err(PackError(format!("section {} panicked", section.name)))
                        });
                    if result.is_err() {
                        stop.store(true, Ordering::Relaxed);
                    }
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut done = 0;
        let mut failure = None;
        for (index, result) in receiver {
            match result {
                Ok(()) => {
                    done += 1;
                    progress(done, total, sections[index].name);
                }
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    })
}

/// The worker count: one per core the process may use.
pub fn default_workers() -> usize {
    std::thread::available_parallelism().map_or(1, usize::from)
}

/// Write a pack JSON file ([`json::pack_bytes`]) at `relative` under `root`,
/// creating its directory; returns the file's SHA-256, as the Python
/// builder's `_write_json` does.
pub fn write_json(root: &Path, relative: &str, value: &json::Json) -> Result<String, PackError> {
    let bytes = json::pack_bytes(value);
    write_file(root, relative, &bytes)?;
    Ok(png::hex(&<sha2::Sha256 as sha2::Digest>::digest(&bytes)))
}

/// Write `bytes` at `relative` under `root`, creating its directory.
pub fn write_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), PackError> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PackError(format!("cannot create {}: {e}", parent.display())))?;
    }
    std::fs::write(&path, bytes)
        .map_err(|e| PackError(format!("cannot write {}: {e}", path.display())))
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    /// A fresh directory path for one test call; the caller creates it.
    pub fn scratch(name: &str) -> PathBuf {
        let n = CALLS.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("psiv-pack-unit-{}-{n}-{name}", std::process::id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::scratch;

    fn rom() -> Rom {
        Rom::for_tests(vec![0; 16])
    }

    fn ok_a(_: &Rom, root: &Path) -> Result<(), PackError> {
        write_json(root, "a/a.json", &json::Json::object().with("a", 1)).map(drop)
    }

    fn ok_b(_: &Rom, root: &Path) -> Result<(), PackError> {
        write_file(root, "b.bin", &[1, 2, 3])
    }

    fn fails(_: &Rom, _: &Path) -> Result<(), PackError> {
        Err(PackError("section c failed".into()))
    }

    fn panics(_: &Rom, _: &Path) -> Result<(), PackError> {
        panic!("a bug")
    }

    #[test]
    fn sections_report_progress_once_each() {
        let root = scratch("sections");
        let sections = [
            Section {
                name: "a",
                claims: &["a"],
                build: ok_a,
            },
            Section {
                name: "b",
                claims: &["b.bin"],
                build: ok_b,
            },
        ];
        for workers in [1, 2, 8] {
            let mut seen = Vec::new();
            build_sections(&rom(), &root, &sections, workers, |done, total, name| {
                seen.push((done, total, name.to_owned()));
            })
            .unwrap();
            seen.sort_by(|x, y| x.2.cmp(&y.2));
            let dones: std::collections::BTreeSet<usize> = seen.iter().map(|s| s.0).collect();
            assert_eq!(dones, [1, 2].into());
            assert!(seen.iter().all(|s| s.1 == 2));
            assert_eq!(
                seen.iter().map(|s| s.2.as_str()).collect::<Vec<_>>(),
                ["a", "b"]
            );
        }
        assert_eq!(std::fs::read(root.join("b.bin")).unwrap(), [1, 2, 3]);
        assert_eq!(
            json::dumps(&claims(&sections), true).replace([' ', '\n'], ""),
            r#"{"sections":[{"claims":["a"],"name":"a"},{"claims":["b.bin"],"name":"b"}]}"#
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failing_or_panicking_section_fails_the_build() {
        let root = scratch("failing");
        for bad in [fails as fn(&Rom, &Path) -> Result<(), PackError>, panics] {
            let sections = [
                Section {
                    name: "a",
                    claims: &["a"],
                    build: ok_a,
                },
                Section {
                    name: "c",
                    claims: &["c"],
                    build: bad,
                },
            ];
            let error = build_sections(&rom(), &root, &sections, 2, |_, _, _| {}).unwrap_err();
            assert!(error.0.starts_with("section c"), "{}", error.0);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_sections_build_nothing() {
        let root = scratch("empty");
        build_sections(&rom(), &root, &[], 4, |_, _, _| panic!("no section")).unwrap();
        assert!(!root.exists());
    }
}
