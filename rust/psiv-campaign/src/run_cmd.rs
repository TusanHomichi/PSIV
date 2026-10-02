//! The `run` and `replay` subcommands.
//!
//! Exit status for these two: 0 the run completed (or the replay played), 2 the
//! run halted (a report was written), 1 a usage or setup error. (`validate`
//! and `plan` keep their own, older statuses: see `main.rs`.)

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use psiv_campaign::replay::replay;
use psiv_campaign::route::Route;
use psiv_campaign::runner::{RunConfig, run};
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
        write_file(
            Path::new(path),
            &serde_json::json!({"result": "completed", "digest": result.digest.to_string()})
                .to_string(),
        )?;
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
