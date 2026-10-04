//! The one data-driven test: every fixture in `replay_fixtures/`.
//!
//! A capture becomes a fixture and a fixture becomes a case here, without a
//! hand-written test per tape: the directory is the list and
//! `replay_fixtures/divergences.json` is where a fixture that does not replay
//! exactly says so - one entry per diverging fixture, carrying its **first**
//! divergence and a ledger reference.
//!
//! The manifest cannot go stale, in either direction:
//!
//! * a fixture that diverges anywhere other than its entry fails the test (or,
//!   with no entry at all, fails outright);
//! * an entry whose fixture no longer diverges, or whose fixture is gone,
//!   fails too.
//!
//! That is what makes the list usable as a worklist: closing a divergence
//! means deleting its entry, and the entry cannot outlive the fix.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::*;

use crate::battle::*;

use super::pack;

/// Where the fixtures and their manifest live, relative to this crate.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/battle/replay_fixtures")
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    fixtures: BTreeMap<String, Entry>,
}

/// One fixture's known divergence.
#[derive(Debug, Deserialize)]
struct Entry {
    /// The round the divergence sits in.
    round: u16,
    /// The frame the manifest names - the action's first frame, or the round's
    /// order frame for a draw-count divergence, which has no action of its own.
    frame: u32,
    /// The divergence's kind, as [`Divergence::kind`] or `"draws"` names it.
    kind: String,
    /// What the log's action did, for a reader.
    #[allow(dead_code)]
    action: String,
    /// What the log shows there.
    expected: String,
    /// What the port did instead.
    actual: String,
    /// The round's roll count on both sides, when the manifest carries it.
    #[serde(default)]
    round_rolls: Option<RoundRolls>,
    /// Where the finding is written up.
    ledger: String,
}

#[derive(Debug, Deserialize)]
struct RoundRolls {
    log: usize,
    port: usize,
}

fn manifest() -> (Manifest, PathBuf) {
    let path = fixtures_dir().join("divergences.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let manifest: Manifest = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()));
    (manifest, path)
}

/// Every fixture under the directory, without the manifest.
///
/// Subdirectories are fixtures too: a sweep keeps its captures in one of its
/// own (`sweep_motavia/`), and the name a fixture is known by - in the panic
/// messages here and as the manifest's key - is its path relative to the
/// directory without the extension (`sweep_motavia/formation_05`), so one
/// directory's fixture cannot shadow another's.
fn fixture_files() -> Vec<(String, PathBuf)> {
    let dir = fixtures_dir();
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_fixtures(&dir, &dir, &mut files);
    let local = local_fixtures_dir();
    if local.join("replay_pack.json").is_file() {
        let mut additional = Vec::new();
        collect_fixtures(&local, &local, &mut additional);
        if additional.is_empty() {
            eprintln!(
                "no local captures at {}; skipping local captures",
                local.display()
            );
        }
        files.extend(
            additional
                .into_iter()
                .map(|(name, path)| (format!("local/{name}"), path)),
        );
    } else {
        eprintln!(
            "local replay inputs absent at {}; skipping local captures",
            local.display()
        );
    }
    files.sort();
    assert!(
        files.len() >= 2,
        "the fixture directory holds {} fixture(s): the extraction is missing",
        files.len()
    );
    files
}

fn local_fixtures_dir() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.join(
        std::env::var_os("PSIV_REPLAY_FIXTURES")
            .unwrap_or_else(|| "build/oracle/replay_fixtures".into()),
    )
}

/// The directory's own data files: the manifest, and the swept records
/// [`super::pack`] reads (`oracle/sweep/replay_pack.py`). Everything else
/// under the directory is a fixture - a file that is not one fails the walk
/// with the extractor's own parse error, which is the honest way to find out.
fn not_a_fixture(name: &std::ffi::OsStr) -> bool {
    name == "divergences.json" || name == "motavia_pack.json" || name == "replay_pack.json"
}

fn collect_fixtures(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            collect_fixtures(root, &path, files);
            continue;
        }
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }
        if path.file_name().is_some_and(not_a_fixture) {
            continue;
        }
        let name = path
            .strip_prefix(root)
            .expect("a path inside the fixture directory")
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/");
        files.push((name, path));
    }
}

/// Every captured battle in `replay_fixtures/`, on the cartridge's own stream.
///
/// Per fixture: the battle is built from the fixture's own state (a vehicle
/// battle through the port's vehicle path), its rounds are replayed on the
/// rolls the log's frames hold, and every action and every round's draw count
/// is compared with the log. A fixture the port cannot replay exactly must have
/// an entry in `divergences.json` naming where it parts company - and that
/// entry must still be the divergence it describes.
#[test]
fn every_fixture_replays_as_recorded() {
    let (manifest, manifest_path) = manifest();
    let data = pack::data();
    let local = local_fixtures_dir();
    let local_data = local
        .join("replay_pack.json")
        .is_file()
        .then(|| pack::local_data(&local.join("replay_pack.json")));
    let files = fixture_files();
    let mut checked = 0;
    for (name, path) in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let fixture = fixture(&text);
        assert_eq!(
            fixture.provenance.trace_sha256.len(),
            64,
            "{name}: the fixture records the capture's trace"
        );

        let replay = replay_inner(
            &fixture,
            if name.starts_with("local/") {
                local_data
                    .as_ref()
                    .expect("local captures need the validated pack export")
            } else {
                &data
            },
        );
        let finding = &replay.finding;
        let entry = manifest.fixtures.get(name);
        match (finding, entry) {
            (None, None) => {}
            (Some(finding), Some(entry)) => {
                let round_frame = fixture
                    .rounds
                    .iter()
                    .find(|round| round.round == finding.round())
                    .map(|round| round.order_frame)
                    .expect("the finding's round is one of the fixture's");
                let frame = if finding.kind() == "queue" {
                    round_frame
                } else {
                    finding.frame()
                };
                let (expected, actual) = finding.expectation();
                assert_eq!(
                    (finding.round(), frame, finding.kind()),
                    (entry.round, entry.frame, entry.kind.as_str()),
                    "{name}: {manifest_path:?} carries a different first divergence\n  \
                     manifest says: {} at f{} ({})\n  the replay says: {actual} where the \
                     log has {expected}\n  ledger: {}\n  expected {} actual {}",
                    entry.kind,
                    entry.frame,
                    entry.round,
                    entry.ledger,
                    entry.expected,
                    entry.actual,
                );
                if let Some(rolls) = &entry.round_rolls {
                    let (_, log, port) = replay
                        .draws
                        .iter()
                        .find(|(round, _, _)| *round == entry.round)
                        .expect("the finding's round was replayed");
                    assert_eq!(
                        (*log, *port),
                        (rolls.log, rolls.port),
                        "{name}: round {}'s roll counts",
                        entry.round
                    );
                }
            }
            (Some(finding), None) => {
                let (expected, actual) = finding.expectation();
                panic!(
                    "{name}: the port diverges at f{} (round {}, {}): the log has \
                     {expected}, the port {actual}. That is this machinery's worklist: \
                     fix it, or record it as an entry in {} with its numbers and a \
                     ledger reference",
                    finding.frame(),
                    finding.round(),
                    finding.kind(),
                    manifest_path.display()
                );
            }
            (None, Some(entry)) => panic!(
                "{name}: {} carries an entry for f{} ({}) but the fixture replays \
                 exactly - the manifest is stale; delete the entry or find the real \
                 divergence",
                manifest_path.display(),
                entry.frame,
                entry.kind
            ),
        }

        // The rewards and the outcome: what the port computed has to be what
        // the log shows, unless the fixture diverges (a finding can leave the
        // battle in another state, and carrying it to *an* outcome is the
        // engine's own contract) or the capture was capped (a truncated
        // fixture's rounds end where its tape did, with both sides standing,
        // so the outcome is not something it states).
        let outcome = replay.battle.outcome();
        if finding.is_none() && !fixture.outcome.truncated {
            assert!(
                outcome.is_some(),
                "{name}: a fixture the port replays exactly ends the way the log does,                  not mid-battle"
            );
            let want = match (fixture.outcome.victory, fixture.outcome.defeat) {
                (true, _) => Some(Outcome::Victory),
                (false, true) => Some(Outcome::Defeat),
                // Neither side was wiped inside the log's window: the outcome
                // is the engine's own to reach, and reaching one is asserted
                // above.
                (false, false) => None,
            };
            if let Some(want) = want {
                assert_eq!(outcome, Some(want), "{name}: the log's own outcome");
            }
        } else {
            // A diverging fixture can leave the battle unfinished: the log's
            // own rounds end where the cartridge's battle did, and a port that
            // resolved them differently may never wipe either side. Its entry
            // is where that is written down.
            assert_eq!(
                replay.draws.len(),
                fixture.rounds.len(),
                "{name}: every round the log holds was played"
            );
        }
        account_for_every_roll(&fixture, name);
        checked += 1;
    }
    assert_eq!(checked, files.len(), "every fixture was replayed");
    eprintln!(
        "replayed {checked} fixtures; {} manifest entries",
        manifest.fixtures.len()
    );
}

/// Writes the manifest entry every diverging fixture needs - one JSON object
/// per line, naming its fixture in the line's own `fixture` field - to the file
/// `PSIV_MANIFEST_DUMP` names, and prints a one-line summary of what it wrote.
///
/// The test above is one fixture at a time: a missing entry is a panic, so
/// harvesting a whole directory's entries would take one test run per fixture.
/// This is that walk with the panic replaced by a line in the dump -
/// `oracle/sweep/manifest.py` reads the dump, assigns each finding to its
/// cluster and writes `divergences.json`, and `ledger` (and a clearer `action`)
/// are a reading of the evidence rather than something the harness can derive,
/// which is why the manifest module carries those and not this. It is off by
/// default for that reason: the walk above is the check, this is the tool.
///
/// A **file** and never stdout: with `--nocapture` libtest prints
/// `test <name> ... ` without a newline before the test's own output, so the
/// first finding used to come back as `test <name> ... {"fixture":...}`, and a
/// reader that takes one JSON object per line drops a line it cannot read.
/// The dump is written whole (a run that panics leaves the previous one, not
/// half of this one), the summary is printed after it, and the path is not
/// optional: an unset `PSIV_MANIFEST_DUMP` fails the run rather than putting
/// findings somewhere nobody named.
///
/// The one claim it does make is that a fixture's name is a line's own
/// `fixture` field, and that every entry the manifest already carries still
/// names a fixture that exists.
#[test]
#[ignore = "harvesting tool: writes the manifest entries a new directory needs"]
fn dump_manifest_entries() {
    let dump_path = manifest_dump_path();
    let (manifest, manifest_path) = manifest();
    let data = pack::data();
    let local = local_fixtures_dir();
    let local_data = local
        .join("replay_pack.json")
        .is_file()
        .then(|| pack::local_data(&local.join("replay_pack.json")));
    let files = fixture_files();
    let mut lines: Vec<String> = Vec::new();
    for (name, path) in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let fixture = fixture(&text);
        let replay = replay_inner(
            &fixture,
            if name.starts_with("local/") {
                local_data
                    .as_ref()
                    .expect("local captures need the validated pack export")
            } else {
                &data
            },
        );
        let Some(finding) = &replay.finding else {
            continue;
        };
        let round = fixture
            .rounds
            .iter()
            .find(|round| round.round == finding.round())
            .expect("the finding's round is one of the fixture's");
        let frame = if finding.kind() == "queue" {
            round.order_frame
        } else {
            finding.frame()
        };
        let (expected, actual) = finding.expectation();
        let (log, port) = replay
            .draws
            .iter()
            .find(|(number, _, _)| *number == finding.round())
            .map(|(_, log, port)| (*log, *port))
            .expect("the finding's round was replayed");
        let entry = serde_json::json!({
            "fixture": name,
            "round": finding.round(),
            "frame": frame,
            "kind": finding.kind(),
            "expected": expected,
            "actual": actual,
            "round_rolls": {"log": log, "port": port},
            "ledger": "docs/oracle/BATTLE_ORACLE_SWEEP.md (fill in the cluster)",
        });
        let line = serde_json::to_string(&entry).expect("the entry serialises");
        assert!(
            serde_json::from_str::<serde_json::Value>(&line).is_ok(),
            "{name}: the line written for it parses"
        );
        lines.push(line);
    }
    for name in manifest.fixtures.keys() {
        assert!(
            files.iter().any(|(file, _)| file == name),
            "{} carries an entry for {name}, which is not a fixture here",
            manifest_path.display()
        );
    }
    write_manifest_dump(&dump_path, &lines);
    println!(
        "{} finding(s) written to {}",
        lines.len(),
        dump_path.display()
    );
}

/// The environment variable `dump_manifest_entries` writes its dump to.
const MANIFEST_DUMP: &str = "PSIV_MANIFEST_DUMP";

/// The repository root, which this crate lives at `rust/psiv-core` of.
fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psiv-core is a workspace member at rust/psiv-core")
}

/// The dump's path, which the tool insists on being told.
///
/// A relative path is the repository root's, not the test process's own
/// working directory: cargo runs a test binary from its package's directory
/// (`rust/psiv-core`), so the documented
/// `PSIV_MANIFEST_DUMP=build/lane-evidence/findings.jsonl` would otherwise
/// leave the dump under `rust/psiv-core/build/`, where the reader - run from the
/// root, as `docs/oracle/BATTLE_ORACLE_SWEEP.md` shows - does not look for it.
fn manifest_dump_path() -> PathBuf {
    let value = match std::env::var_os(MANIFEST_DUMP) {
        Some(value) if !value.is_empty() => value,
        _ => panic!(
            "{MANIFEST_DUMP} is not set: this tool writes its findings to the file \
             that variable names - one JSON object per line, so that libtest's own \
             progress text can never share a finding's line - and prints only a \
             summary. Set it to the dump's path (docs/oracle/BATTLE_ORACLE_SWEEP.md, \
             \"Reproducing this ledger\"):\n\n    {MANIFEST_DUMP}=build/lane-evidence/\
             findings.jsonl cargo test --manifest-path rust/Cargo.toml -p psiv-core \
             -- --ignored --nocapture dump_manifest_entries\n"
        ),
    };
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        repository_root().join(path)
    }
}

/// Writes `lines` to `path`, one per line, creating the directory above it.
///
/// All of them or none: the file is written in one go, so a run that fails
/// halfway through the walk leaves the previous dump rather than a short one.
/// No findings at all still writes a dump - an empty file is a tree whose every
/// fixture replays exactly, which is what `oracle/sweep/manifest.py` builds the
/// empty manifest from, while a *missing* dump is the error it reports.
fn write_manifest_dump(path: &Path, lines: &[String]) {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("cannot create {}: {error}", parent.display()));
    }
    let mut body = lines.join("\n");
    if !lines.is_empty() {
        body.push('\n');
    }
    std::fs::write(path, body)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
}

/// An empty dump is an empty file, not a missing one.
///
/// The reader tells the two apart: no file at all is a run that never wrote the
/// dump, and an empty one is a tree with nothing left to record - so the walk
/// above always writes, however many findings it found.
#[test]
fn an_empty_dump_is_still_written() {
    let dir = std::env::temp_dir().join("psiv-empty-manifest-dump");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("lane-evidence/findings.jsonl");
    write_manifest_dump(&path, &[]);
    assert_eq!(
        std::fs::read_to_string(&path).expect("an empty dump is written"),
        "",
        "an empty dump is an empty file, not a missing one"
    );
    std::fs::remove_dir_all(&dir).expect("the scratch directory goes away");
}
