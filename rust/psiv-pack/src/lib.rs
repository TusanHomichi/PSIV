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
//!   then `progress <total>/<total> manifest` once `manifest.json` is
//!   written ([`build_pack`]), then `installed <pack>` once the pack is in
//!   place. There is one progress line for each entry `--claims` lists;
//! - stderr: `psiv-pack: error: <message>` and exit status 2 on any failure,
//!   with `<pack>` left as it was. A wrong ROM is refused in the Python
//!   builder's words ([`rom::wrong_rom`]). A problem in the cleanup after the
//!   new pack is in place is a `psiv-pack: warning: ...` line on stderr, and
//!   the exit status stays 0.
//!
//! `psiv-pack --claims` prints the sections, then the manifest, and the
//! pack-relative paths each claims, as JSON ([`claims`]): a path names a file, a directory and
//! everything in it, or (`maps/*.png`) the matching files directly in a
//! directory.

// The sections hold no unsafe code; only the install's OS calls (liveness
// and the atomic exchange) allow it. Integer pack data stays integer.
#![deny(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

pub mod battle;
pub mod decode;
pub mod dialogue;
pub mod game_start;
pub mod gfx;
pub mod install;
pub mod json;
pub mod m68k;
mod map_symbols;
pub mod maps;
pub mod npc_commands;
pub mod png;
pub mod presentation;
pub mod rom;
mod scene_tables;
pub mod ship_menu;
pub mod shops;
pub mod sound;
mod sound_defs;
pub mod sprites;
pub mod text;
pub mod title;
pub mod travel;
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
    /// Write the section under the pack root, returning its part of
    /// `manifest.json`: an object of the top-level keys it owns.
    pub build: fn(&Rom, &Path) -> Result<json::Json, PackError>,
}

/// `PACK_FORMAT_VERSION`: every pack file's `format_version`.
pub const PACK_FORMAT_VERSION: u32 = 2;

/// The manifest, written once every section has finished.
pub const MANIFEST_NAME: &str = "manifest.json";

/// The sections `psiv-pack` builds, in the order #216 ports them.
pub const SECTIONS: &[Section] = &[
    Section {
        name: "sound",
        claims: &["sound"],
        build: sound::build,
    },
    Section {
        name: "maps",
        claims: &["maps", "sprites", "shops.json", "shops"],
        build: maps::pictures::build,
    },
    Section {
        name: "travel",
        claims: &["travel.json"],
        build: travel::build,
    },
    Section {
        name: "npc_commands",
        claims: &["npc_commands.json"],
        build: npc_commands::build,
    },
    Section {
        name: "ship_menu",
        claims: &["ship_menu"],
        build: ship_menu::build,
    },
    Section {
        name: "game_start",
        claims: &["game_start.json"],
        build: game_start::build,
    },
    Section {
        name: "dialogue",
        claims: &[
            "dialogue/font.json",
            "dialogue/font.png",
            "dialogue/menu_font.png",
            "dialogue/portraits",
            "dialogue/portraits.json",
            "dialogue/scroll_arrow.png",
            "dialogue/trees.json",
            "dialogue/window.json",
            "dialogue/window.png",
        ],
        build: dialogue::build,
    },
    Section {
        name: "scene_tables",
        claims: &["fifth", "guild"],
        build: scene_tables::build,
    },
    Section {
        name: "title",
        claims: &["title"],
        build: title::build,
    },
    Section {
        name: "presentation",
        claims: &["presentation"],
        build: presentation::build,
    },
    Section {
        name: "battle",
        claims: &[
            "battle/abilities.json",
            "battle/art/backgrounds",
            "battle/art/backgrounds.json",
            "battle/art/characters",
            "battle/art/characters.json",
            "battle/art/command_ui.json",
            "battle/art/damage_digits.json",
            "battle/art/enemies",
            "battle/art/enemies.json",
            "battle/art/enemy_attacks",
            "battle/art/enemy_attacks.json",
            "battle/art/enemy_overlays",
            "battle/art/enemy_overlays.json",
            "battle/art/fusion",
            "battle/art/fusion.json",
            "battle/characters.json",
            "battle/enemies.json",
            "battle/enemy_animations.json",
            "battle/equipment.json",
            "battle/formations.json",
            "battle/levels.json",
            "battle/status_portraits",
        ],
        build: battle::build,
    },
];

/// What `psiv-pack --claims` prints: `{"sections": [{"name": ..., "claims":
/// [...]}, ...]}`, every section in order with the paths it writes, and last
/// the manifest.
pub fn claims(sections: &[Section]) -> json::Json {
    let mut listed: Vec<json::Json> = sections
        .iter()
        .map(|s| {
            json::Json::object()
                .with("name", s.name)
                .with("claims", s.claims.to_vec())
        })
        .collect();
    listed.push(
        json::Json::object()
            .with("name", MANIFEST_STEP)
            .with("claims", vec![MANIFEST_NAME]),
    );
    json::Json::object().with("sections", listed)
}

/// Build `sections` from `rom` under `root`, on up to `workers` threads,
/// calling `progress(done, total, name)` on this thread as each finishes.
/// The first failure stops further sections from starting and is returned
/// once the running ones end; a section that panics is a failure too.
/// Returns the sections' manifest parts merged, in section order; two
/// sections that give the same key are a failure.
pub fn build_sections(
    rom: &Rom,
    root: &Path,
    sections: &[Section],
    workers: usize,
    mut progress: impl FnMut(usize, usize, &str),
) -> Result<json::Json, PackError> {
    let total = sections.len();
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel::<(usize, Result<json::Json, PackError>)>();
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
        let mut parts: Vec<Option<json::Json>> = vec![None; total];
        for (index, result) in receiver {
            match result {
                Ok(part) => {
                    done += 1;
                    parts[index] = Some(part);
                    progress(done, total, sections[index].name);
                }
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        merge_parts(sections, parts.into_iter().flatten())
    })
}

/// Lay each section's manifest keys side by side.
fn merge_parts(
    sections: &[Section],
    parts: impl Iterator<Item = json::Json>,
) -> Result<json::Json, PackError> {
    let mut merged: Vec<(String, json::Json)> = Vec::new();
    for (section, part) in sections.iter().zip(parts) {
        let json::Json::Object(pairs) = part else {
            return Err(PackError(format!(
                "section {} returned a manifest part that is not an object",
                section.name
            )));
        };
        for (key, value) in pairs {
            if merged.iter().any(|(k, _)| *k == key) {
                return Err(PackError(format!(
                    "section {} gives manifest key {key}, which another section gave",
                    section.name
                )));
            }
            merged.push((key, value));
        }
    }
    Ok(json::Json::Object(merged))
}

/// The name the manifest's claim and progress line carry.
pub const MANIFEST_STEP: &str = "manifest";

/// The whole pack: `sections` on up to `workers` threads, then the manifest
/// from their parts. `progress(done, total, name)` is called once for every
/// entry [`claims`] lists, the manifest last, so the two always agree.
pub fn build_pack(
    rom: &Rom,
    root: &Path,
    sections: &[Section],
    workers: usize,
    mut progress: impl FnMut(usize, usize, &str),
) -> Result<(), PackError> {
    let total = sections.len() + 1;
    let parts = build_sections(rom, root, sections, workers, |done, _, name| {
        progress(done, total, name)
    })?;
    write_manifest(rom, root, parts)?;
    progress(total, total, MANIFEST_STEP);
    Ok(())
}

/// `write_manifest`: the sections' parts under the format, the generator and
/// the ROM's identity.
pub fn write_manifest(rom: &Rom, root: &Path, parts: json::Json) -> Result<(), PackError> {
    let mut manifest = json::Json::object()
        .with("format_version", PACK_FORMAT_VERSION)
        .with("generator", "psiv_tools.pack")
        .with(
            "rom",
            json::Json::object()
                .with("sha256", rom::EXPECTED_SHA256)
                .with("size_bytes", rom.bytes().len()),
        );
    json::overlay(&mut manifest, &parts);
    write_json(root, MANIFEST_NAME, &manifest).map(drop)
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

/// The PNGs under `directory`, counted and summed in bytes: what the Python
/// builder totals as it writes them.
pub fn png_census(root: &Path, directory: &str) -> Result<(usize, u64), PackError> {
    fn walk(path: &Path, totals: &mut (usize, u64)) -> std::io::Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&entry.path(), totals)?;
            } else if entry.path().extension().is_some_and(|e| e == "png") {
                totals.0 += 1;
                totals.1 += entry.metadata()?.len();
            }
        }
        Ok(())
    }
    let mut totals = (0, 0);
    walk(&root.join(directory), &mut totals)
        .map_err(|e| PackError(format!("cannot list {directory}: {e}")))?;
    Ok(totals)
}

/// A manifest part naming one file: `{key: {"path": .., "sha256": ..}}`.
pub fn file_part(key: &str, path: &str, sha256: String) -> json::Json {
    json::Json::object().with(
        key,
        json::Json::object()
            .with("path", path)
            .with("sha256", sha256),
    )
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

    fn ok_a(_: &Rom, root: &Path) -> Result<json::Json, PackError> {
        write_json(root, "a/a.json", &json::Json::object().with("a", 1))?;
        Ok(json::Json::object().with("a", 1))
    }

    fn ok_b(_: &Rom, root: &Path) -> Result<json::Json, PackError> {
        write_file(root, "b.bin", &[1, 2, 3])?;
        Ok(json::Json::object().with("b", 2))
    }

    fn also_a(_: &Rom, _: &Path) -> Result<json::Json, PackError> {
        Ok(json::Json::object().with("a", 3))
    }

    fn fails(_: &Rom, _: &Path) -> Result<json::Json, PackError> {
        Err(PackError("section c failed".into()))
    }

    fn panics(_: &Rom, _: &Path) -> Result<json::Json, PackError> {
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
            let parts = build_sections(&rom(), &root, &sections, workers, |done, total, name| {
                seen.push((done, total, name.to_owned()));
            })
            .unwrap();
            assert_eq!(parts, json::Json::object().with("a", 1).with("b", 2));
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
            r#"{"sections":[{"claims":["a"],"name":"a"},{"claims":["b.bin"],"name":"b"},{"claims":["manifest.json"],"name":"manifest"}]}"#
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failing_or_panicking_section_fails_the_build() {
        let root = scratch("failing");
        for bad in [
            fails as fn(&Rom, &Path) -> Result<json::Json, PackError>,
            panics,
            also_a,
        ] {
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
    fn a_png_census_counts_every_png_under_a_directory_and_nothing_else() {
        let root = scratch("census");
        write_file(&root, "d/a.png", &[0; 3]).unwrap();
        write_file(&root, "d/deep/b.png", &[0; 5]).unwrap();
        write_file(&root, "d/c.json", &[0; 7]).unwrap();
        write_file(&root, "e/d.png", &[0; 11]).unwrap();
        assert_eq!(png_census(&root, "d").unwrap(), (2, 8));
        assert!(png_census(&root, "missing").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_pack_reports_one_progress_line_per_claimed_entry() {
        let root = scratch("pack");
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
        let mut seen = Vec::new();
        build_pack(&rom(), &root, &sections, 2, |done, total, name| {
            seen.push((done, total, name.to_owned()));
        })
        .unwrap();
        let claimed = serde_json::Value::from(&claims(&sections));
        let names: Vec<&str> = claimed["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert_eq!(seen.len(), names.len());
        assert!(seen.iter().all(|s| s.1 == names.len()));
        assert_eq!(
            seen.iter().map(|s| s.0).collect::<Vec<_>>(),
            (1..=names.len()).collect::<Vec<_>>()
        );
        let mut printed: Vec<&str> = seen.iter().map(|s| s.2.as_str()).collect();
        printed.sort_unstable();
        let mut expected = names.clone();
        expected.sort_unstable();
        assert_eq!(printed, expected);
        assert_eq!(seen.last().unwrap().2, MANIFEST_STEP);
        assert!(root.join(MANIFEST_NAME).is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn no_sections_build_nothing() {
        let root = scratch("empty");
        build_sections(&rom(), &root, &[], 4, |_, _, _| panic!("no section")).unwrap();
        assert!(!root.exists());
    }
}
