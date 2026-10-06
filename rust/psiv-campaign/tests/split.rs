//! `split-tape`: a completed run's report names every chapter, and the run's
//! tape cuts back into one save-start tape per chapter through the shared
//! codec. The pack-free cases refuse the inputs a split must not accept.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use psiv_campaign::tape::{Tape, TapeStart, fnv1a64};

use common::{MAIN_ROUTE, pack};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_psiv-campaign"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("split")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn split(tape: &Path, report: &Path, out: &Path) -> std::process::Output {
    bin()
        .arg("split-tape")
        .arg(tape)
        .arg(report)
        .arg(out)
        .output()
        .unwrap()
}

#[test]
fn a_completed_report_names_its_chapters_and_the_tape_cuts_back() {
    if pack().is_none() {
        return;
    }
    let dir = scratch("two-chapters");
    let saves = dir.join("saves");
    let (tape_path, report_path) = (dir.join("run.tape"), dir.join("report.json"));
    let run = bin()
        .arg("run")
        .arg(MAIN_ROUTE)
        .args(["--until-chapter", "holt", "--save-dir"])
        .arg(&saves)
        .arg("--tape")
        .arg(&tape_path)
        .arg("--report")
        .arg(&report_path)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "{run:?}");

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
    assert_eq!(report["result"], "completed");
    let chapters = report["chapters"].as_array().unwrap();
    assert_eq!(chapters.len(), 2);
    let run_tape = Tape::parse(&std::fs::read_to_string(&tape_path).unwrap()).unwrap();
    assert_eq!(report["frames"], run_tape.pads.len());
    let mut save_hashes = Vec::new();
    for (index, id) in ["academy", "holt"].iter().enumerate() {
        let save = std::fs::read(saves.join(format!("{index:02}-{id}/slot_1.sram"))).unwrap();
        assert_eq!(chapters[index]["id"], *id);
        assert_eq!(chapters[index]["index"], index);
        assert_eq!(
            chapters[index]["save_fnv"],
            format!("{:016x}", fnv1a64(&save)),
            "{id}: the report hashes the chapter's own save"
        );
        save_hashes.push(fnv1a64(&save));
    }

    let out = dir.join("tapes");
    let cut = split(&tape_path, &report_path, &out);
    assert_eq!(cut.status.code(), Some(0), "{cut:?}");
    let pieces: Vec<Tape> = ["00-academy", "01-holt"]
        .iter()
        .map(|name| {
            let text = std::fs::read_to_string(out.join(format!("{name}.tape"))).unwrap();
            Tape::parse(&text).unwrap()
        })
        .collect();
    let joined: Vec<u8> = pieces.iter().flat_map(|piece| piece.pads.clone()).collect();
    assert_eq!(joined, run_tape.pads, "the chapter tapes add up to the run");
    assert_eq!(pieces[0].start, TapeStart::NewGame);
    assert_eq!(
        pieces[1].start,
        TapeStart::Save {
            hash: save_hashes[0]
        },
        "chapter two starts from chapter one's save"
    );
    for (piece, chapter) in pieces.iter().zip(chapters) {
        assert_eq!(piece.pads.len() as u64, chapter["frames"].as_u64().unwrap());
    }

    // A second split into the same directory refuses and overwrites nothing.
    let before = std::fs::read(out.join("01-holt.tape")).unwrap();
    let again = split(&tape_path, &report_path, &out);
    assert_eq!(again.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&again.stderr).contains("exists"));
    assert_eq!(std::fs::read(out.join("01-holt.tape")).unwrap(), before);
}

#[test]
fn a_split_refuses_a_halted_report_a_foreign_tape_and_missing_arguments() {
    let dir = scratch("refusals");
    let tape = Tape {
        start: TapeStart::NewGame,
        pads: vec![0; 10],
    };
    let tape_path = dir.join("run.tape");
    std::fs::write(&tape_path, tape.render()).unwrap();

    let halted = dir.join("halted.json");
    std::fs::write(&halted, r#"{"result": "halted", "chapter": "academy"}"#).unwrap();
    let out = split(&tape_path, &halted, &dir.join("halted-out"));
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a completed"));
    assert!(!dir.join("halted-out").exists(), "nothing is written");

    // Chapters whose frames do not add up to the tape's are another run's.
    let foreign = dir.join("foreign.json");
    std::fs::write(
        &foreign,
        r#"{"result": "completed", "chapters": [
            {"index": 0, "id": "a", "frames": 4, "save_fnv": "00000000000000aa"}]}"#,
    )
    .unwrap();
    let out = split(&tape_path, &foreign, &dir.join("foreign-out"));
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("add up to"));
    assert!(!dir.join("foreign-out").exists(), "nothing is written");

    let out = bin().args(["split-tape", "one-argument"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = split(&dir.join("no-such.tape"), &foreign, &dir.join("x"));
    assert_eq!(out.status.code(), Some(1));
}
