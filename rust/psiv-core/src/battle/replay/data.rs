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
    files.sort();
    assert!(
        files.len() >= 2,
        "the fixture directory holds {} fixture(s): the extraction is missing",
        files.len()
    );
    files
}

/// The directory's own data files: the manifest, and the swept records
/// [`super::pack`] reads (`oracle/sweep/replay_pack.py`). Everything else
/// under the directory is a fixture - a file that is not one fails the walk
/// with the extractor's own parse error, which is the honest way to find out.
fn not_a_fixture(name: &std::ffi::OsStr) -> bool {
    name == "divergences.json" || name == "motavia_pack.json"
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

        let replay = replay_inner(&fixture, &data);
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
}

/// Prints the manifest entry every diverging fixture needs, one JSON object per
/// line, starting with the fixture's own name.
///
/// The test above is one fixture at a time: a missing entry is a panic, so
/// harvesting a whole directory's entries would take one test run per fixture.
/// This is that walk with the panic replaced by a line on stdout - a new
/// directory's manifest is generated with
/// `cargo test -p psiv-core -- --ignored --nocapture dump_manifest_entries`,
/// the entries are pasted into `divergences.json`, and `ledger` (and a clearer
/// `action`) are written by hand, because those are a reader's and not the
/// harness's. It is off by default for that reason: the walk above is the
/// check, this is the tool.
///
/// The one claim it does make is that a fixture's name is a line's own
/// `fixture` field, and that every entry the manifest already carries still
/// names a fixture that exists.
#[test]
#[ignore = "harvesting tool: prints the manifest entries a new directory needs"]
fn dump_manifest_entries() {
    let (manifest, manifest_path) = manifest();
    let data = pack::data();
    let files = fixture_files();
    for (name, path) in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let fixture = fixture(&text);
        let replay = replay_inner(&fixture, &data);
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
            "{name}: the line on stdout parses"
        );
        println!("{line}");
    }
    for name in manifest.fixtures.keys() {
        assert!(
            files.iter().any(|(file, _)| file == name),
            "{} carries an entry for {name}, which is not a fixture here",
            manifest_path.display()
        );
    }
}
