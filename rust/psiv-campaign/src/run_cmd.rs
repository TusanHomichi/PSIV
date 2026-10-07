//! The `run` and `replay` subcommands.
//!
//! Exit status for these two: 0 the run completed (or the replay played), 2 the
//! run halted (a report was written), 1 a usage or setup error. (`validate`
//! and `plan` keep their own, older statuses: see `main.rs`.)

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use psiv_campaign::prefix::check_prefix;
use psiv_campaign::replay::replay;
use psiv_campaign::route::Route;
use psiv_campaign::runner::{RunConfig, run};
use psiv_campaign::split::{ChapterCut, chapters_of, report_chapters, split_tape};
use psiv_campaign::tape::Tape;

use super::{Args, pack_dir};

/// The save directory when `--save-dir` is not given: under `build/`, which the
/// repository ignores.
const DEFAULT_SAVE_DIR: &str = "build/campaign";

/// `run <route> [--from-chapter ID] [--until-chapter ID] [--save-dir DIR]
/// [--tape OUT] [--report OUT] [--pack DIR]`.
pub fn cmd_run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn run_inner(args: &[String]) -> Result<ExitCode, String> {
    let args = Args::parse(args)?;
    let [path] = args.positional.as_slice() else {
        return Err(super::USAGE.to_owned());
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let route = Route::parse(&text).map_err(|e| format!("{path}: {e}"))?;
    let save_dir = PathBuf::from(args.one("save-dir").unwrap_or(DEFAULT_SAVE_DIR));
    let first_index = args
        .one("from-chapter")
        .and_then(|id| route.chapters.iter().position(|c| c.id == id))
        .unwrap_or(0);
    let config = RunConfig {
        route,
        pack: pack_dir(&args),
        from_chapter: args.one("from-chapter").map(str::to_owned),
        until_chapter: args.one("until-chapter").map(str::to_owned),
        save_dir: save_dir.clone(),
        verbose: true,
    };
    let result = run(&config).map_err(|e| e.to_string())?;
    let tape_path = args
        .one("tape")
        .map_or_else(|| save_dir.join("run.tape"), PathBuf::from);
    write_file(&tape_path, &result.tape.render())?;
    for chapter in &result.chapters {
        println!(
            "chapter {}: {} frames, {} battles, saved {}; party {}",
            chapter.id,
            chapter.frames,
            chapter.battles,
            chapter.save.display(),
            chapter.party
        );
    }
    println!(
        "tape {} ({} frames); digest {}",
        tape_path.display(),
        result.tape.pads.len(),
        result.digest
    );
    if let Some(report) = &result.report {
        let report_path = args
            .one("report")
            .map_or_else(|| save_dir.join("halt-report.json"), PathBuf::from);
        let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
        write_file(&report_path, &json)?;
        println!(
            "HALTED in chapter {} at objective {} ({}): {} -- {}\nreport {}",
            report["chapter"].as_str().unwrap_or("?"),
            report["objective_index"],
            report["objective_kind"].as_str().unwrap_or("?"),
            report["halt"]["kind"].as_str().unwrap_or("?"),
            report["halt"]["detail"].as_str().unwrap_or("?"),
            report_path.display()
        );
        return Ok(ExitCode::from(2));
    }
    if let Some(path) = args.one("report") {
        let chapters = report_chapters(first_index, &result.chapters)
            .map_err(|e| format!("cannot hash a chapter save: {e}"))?;
        let report = serde_json::json!({
            "result": "completed",
            "digest": result.digest.to_string(),
            "frames": result.tape.pads.len(),
            "chapters": chapters,
        });
        write_file(Path::new(path), &report.to_string())?;
    }
    println!("COMPLETED");
    Ok(ExitCode::SUCCESS)
}

/// `replay <tape> [--from-save FILE] [--pack DIR]`.
pub fn cmd_replay(args: &[String]) -> ExitCode {
    match replay_inner(args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn replay_inner(args: &[String]) -> Result<ExitCode, String> {
    let args = Args::parse(args)?;
    let [path] = args.positional.as_slice() else {
        return Err(super::USAGE.to_owned());
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let tape = Tape::parse(&text).map_err(|e| format!("{path}: {e}"))?;
    let from_save = args.one("from-save").map(Path::new);
    let result = replay(&pack_dir(&args), &tape, from_save).map_err(|e| e.to_string())?;
    for (frame, fault) in &result.faults {
        println!("fault at frame {frame}: {fault}");
    }
    println!(
        "replayed {} frames; digest {}",
        result.frames, result.digest
    );
    Ok(ExitCode::SUCCESS)
}

/// `split-tape <run.tape> <report.json> <out-dir>`: one save-start tape per
/// chapter of a completed run, named `NN-id.tape` by route position. The
/// pieces concatenate to the run's tape; a piece replayed from the previous
/// chapter's save plays other battles (a slot holds no RNG state), so the
/// route is verified as one tape, not piece by piece (see `split.rs`).
pub fn cmd_split_tape(args: &[String]) -> ExitCode {
    match split_inner(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn split_inner(args: &[String]) -> Result<(), String> {
    let [tape_path, report_path, out_dir] = args else {
        return Err(super::USAGE.to_owned());
    };
    let text =
        std::fs::read_to_string(tape_path).map_err(|e| format!("cannot read {tape_path}: {e}"))?;
    let tape = Tape::parse(&text).map_err(|e| format!("{tape_path}: {e}"))?;
    let text = std::fs::read_to_string(report_path)
        .map_err(|e| format!("cannot read {report_path}: {e}"))?;
    let report: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{report_path}: {e}"))?;
    let chapters = chapters_of(&report).map_err(|e| format!("{report_path}: {e}"))?;
    let pieces = split_tape(&tape, &chapters).map_err(|e| e.to_string())?;
    let dir = Path::new(out_dir);
    for chapter in &chapters {
        let path = dir.join(format!("{:02}-{}.tape", chapter.index, chapter.id));
        if path.exists() {
            return Err(format!(
                "{} exists; choose a fresh directory",
                path.display()
            ));
        }
    }
    for (chapter, piece) in chapters.iter().zip(&pieces) {
        let path = dir.join(format!("{:02}-{}.tape", chapter.index, chapter.id));
        write_file(&path, &piece.render())?;
        println!(
            "{}: {} frames, ends on save {:016x}",
            path.display(),
            piece.pads.len(),
            chapter.save_fnv
        );
    }
    Ok(())
}

/// `prefix-check <base.tape> <base-report.json> <run.tape> <run-save-dir>`: the
/// run's tape begins with every pad of the base's, and each base chapter's save
/// hashes the same in the run's save directory. 0 they match, 2 they differ, 1
/// a usage or input error.
pub fn cmd_prefix_check(args: &[String]) -> ExitCode {
    let [base_tape, base_report, run_tape, run_saves] = args else {
        eprintln!("{}", super::USAGE);
        return ExitCode::from(1);
    };
    let (base, chapters, run) = match prefix_inputs(base_tape, base_report, run_tape) {
        Ok(inputs) => inputs,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(1);
        }
    };
    match check_prefix(&base, &chapters, &run, Path::new(run_saves)) {
        Ok(found) => {
            println!(
                "prefix matches: {} chapters, {} frames, every pad and every chapter save",
                found.chapters, found.frames
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("prefix differs: {error}");
            ExitCode::from(2)
        }
    }
}

/// The base tape, the base report's chapters and the run's tape.
fn prefix_inputs(
    base_tape: &str,
    base_report: &str,
    run_tape: &str,
) -> Result<(Tape, Vec<ChapterCut>, Tape), String> {
    let read = |path: &str| -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
    };
    let base = Tape::parse(&read(base_tape)?).map_err(|e| format!("{base_tape}: {e}"))?;
    let run = Tape::parse(&read(run_tape)?).map_err(|e| format!("{run_tape}: {e}"))?;
    let report: serde_json::Value =
        serde_json::from_str(&read(base_report)?).map_err(|e| format!("{base_report}: {e}"))?;
    let chapters = chapters_of(&report).map_err(|e| format!("{base_report}: {e}"))?;
    Ok((base, chapters, run))
}

/// `inspect <slot.sram> [--pack DIR]`: the save's position, party, pack and flags.
pub fn cmd_inspect(args: &[String]) -> ExitCode {
    let result = Args::parse(args).and_then(|args| {
        let [path] = args.positional.as_slice() else {
            return Err(super::USAGE.to_owned());
        };
        psiv_campaign::inspect::inspect(&pack_dir(&args), Path::new(path))
            .map_err(|e| e.to_string())
    });
    match result {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
