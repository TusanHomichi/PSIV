//! `psiv-campaign`: validate a route file, print a walking plan, play a route
//! headlessly with pad presses, or replay a recorded tape.
//!
//! ```text
//! psiv-campaign validate <route.json> [--pack DIR]
//! psiv-campaign plan --from-map M --from-cell X,Y --to-map N [--to-cell X,Y]
//!                    [--flag bank:id]... [--vehicle N] [--pack DIR]
//! psiv-campaign run <route.json> [--from-chapter ID] [--until-chapter ID]
//!                   [--save-dir DIR] [--tape OUT] [--report OUT] [--pack DIR]
//! psiv-campaign replay <tape> [--from-save FILE] [--pack DIR]
//! psiv-campaign inspect <slot.sram> [--pack DIR]
//! psiv-campaign save-probe-tape <slot.sram> <out.tape> [neutral-frames]
//! psiv-campaign split-tape <run.tape> <report.json> <out-dir>
//! psiv-campaign prefix-check <base.tape> <base-report.json> <run.tape> <run-save-dir>
//! ```
//!
//! Map ids are decimal or `0x` hex. The pack defaults to `$PSIV_PACK`, then
//! `runtime-pack` beside the repository root.
//!
//! Exit status. `validate` and `plan`: 0 success, 1 a failed validation or an
//! impossible plan, 2 a usage or input error. `run` and `replay`: 0 the run
//! completed, 2 the run halted (a report was written), 1 a usage or setup
//! error. The two families differ because `validate` and `plan` shipped first
//! with R0, and a verified command's statuses are not changed under it.

mod run_cmd;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use psiv_campaign::cell_plan::Mover;
use psiv_campaign::route::{FlagRef, Route};
use psiv_campaign::tape::{Tape, TapeStart, fnv1a64};
use psiv_campaign::validate::validate;
use psiv_campaign::{MapGraph, Plan, Position, Target};
use psiv_core::{Cell, Direction};
use psiv_data::{BattleFiles, GameData};

const USAGE: &str = "usage:\n  psiv-campaign validate <route.json> [--pack DIR]\n  \
psiv-campaign plan --from-map M --from-cell X,Y --to-map N [--to-cell X,Y] [--flag bank:id]... [--vehicle N] [--pack DIR]\n  \
psiv-campaign run <route.json> [--from-chapter ID] [--until-chapter ID] [--save-dir DIR] [--tape OUT] [--report OUT] [--pack DIR]\n  \
psiv-campaign replay <tape> [--from-save FILE] [--pack DIR]\n  \
psiv-campaign save-probe-tape <slot.sram> <out.tape> [neutral-frames]\n  \
psiv-campaign split-tape <run.tape> <report.json> <out-dir>\n  \
psiv-campaign prefix-check <base.tape> <base-report.json> <run.tape> <run-save-dir>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => return run_cmd::cmd_run(&args[1..]),
        Some("replay") => return run_cmd::cmd_replay(&args[1..]),
        Some("inspect") => return run_cmd::cmd_inspect(&args[1..]),
        Some("save-probe-tape") => return cmd_save_probe_tape(&args[1..]),
        Some("split-tape") => return run_cmd::cmd_split_tape(&args[1..]),
        Some("prefix-check") => return run_cmd::cmd_prefix_check(&args[1..]),
        _ => {}
    }
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

const MAX_SAVE_PROBE_FRAMES: usize = 600;

fn probe_frame_count(value: Option<&String>) -> Result<usize, String> {
    let Some(value) = value else { return Ok(0) };
    let frames = value
        .parse::<usize>()
        .map_err(|_| "neutral-frames must be decimal".to_owned())?;
    if frames > MAX_SAVE_PROBE_FRAMES {
        return Err(format!("neutral-frames exceeds {MAX_SAVE_PROBE_FRAMES}"));
    }
    Ok(frames)
}

/// Make an ordinary CONTINUE probe through the shared tape codec. Zero pads
/// check loading only; neutral pads allow the normal title fade to render.
fn cmd_save_probe_tape(args: &[String]) -> ExitCode {
    let (source, output, count) = match args {
        [source, output] => (source, output, None),
        [source, output, count] => (source, output, Some(count)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(1);
        }
    };
    let result = (|| -> Result<(), String> {
        let frames = probe_frame_count(count)?;
        let bytes = std::fs::read(source).map_err(|e| format!("cannot read {source}: {e}"))?;
        let mut tape = Tape::new(TapeStart::Save {
            hash: fnv1a64(&bytes),
        });
        tape.pads.resize(frames, 0);
        let path = Path::new(output);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("cannot create {output}: {e}"))?;
        file.write_all(tape.render().as_bytes())
            .map_err(|e| format!("cannot write {output}: {e}"))?;
        println!("CONTINUE probe {output}: {frames} neutral pads from {source}");
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("validate") => cmd_validate(&args[1..]),
        Some("plan") => cmd_plan(&args[1..]),
        _ => Err(USAGE.to_owned()),
    }
}

/// `--name value` pairs and bare positionals.
struct Args {
    positional: Vec<String>,
    options: Vec<(String, String)>,
}

impl Args {
    fn parse(args: &[String]) -> Result<Args, String> {
        let mut parsed = Args {
            positional: Vec::new(),
            options: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            if let Some(name) = arg.strip_prefix("--") {
                let value = it
                    .next()
                    .ok_or_else(|| format!("--{name} needs a value\n{USAGE}"))?;
                parsed.options.push((name.to_owned(), value.clone()));
            } else {
                parsed.positional.push(arg.clone());
            }
        }
        Ok(parsed)
    }

    fn one(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn all(&self, name: &str) -> Vec<&str> {
        self.options
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    fn require(&self, name: &str) -> Result<&str, String> {
        self.one(name)
            .ok_or_else(|| format!("missing --{name}\n{USAGE}"))
    }
}

fn pack_dir(args: &Args) -> PathBuf {
    if let Some(dir) = args.one("pack") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("PSIV_PACK") {
        return PathBuf::from(dir);
    }
    let beside_repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    if beside_repo.join("manifest.json").is_file() {
        beside_repo
    } else {
        PathBuf::from("runtime-pack")
    }
}

fn load_pack(dir: &Path) -> Result<GameData, String> {
    GameData::load(dir).map_err(|e| format!("cannot load pack {}: {e}", dir.display()))
}

fn parse_map(text: &str) -> Result<u16, String> {
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u16::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .map_err(|_| format!("bad map id {text:?}"))
}

fn parse_cell(text: &str) -> Result<Cell, String> {
    let bad = || format!("bad cell {text:?}: expected X,Y");
    let (x, y) = text.split_once(',').ok_or_else(bad)?;
    Ok(Cell::new(
        x.trim().parse().map_err(|_| bad())?,
        y.trim().parse().map_err(|_| bad())?,
    ))
}

fn cmd_validate(args: &[String]) -> Result<ExitCode, String> {
    let args = Args::parse(args)?;
    let [path] = args.positional.as_slice() else {
        return Err(USAGE.to_owned());
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let route = Route::parse(&text).map_err(|e| format!("{path}: {e}"))?;
    let dir = pack_dir(&args);
    let data = load_pack(&dir)?;
    let battle = BattleFiles::load(&dir).map_err(|e| format!("cannot load battle files: {e}"))?;
    let report = validate(&route, &data, &battle);
    for warning in &report.warnings {
        println!("warning: {warning}");
    }
    for (chapter, index) in route.unverified() {
        println!("verify: chapter {chapter:?} objective {index}");
    }
    for error in &report.errors {
        println!("error: {error}");
    }
    println!(
        "{} chapters, {} objectives, {} planned warps, {} planned steps, {} errors",
        route.chapters.len(),
        report.objectives,
        report.planned_warps,
        report.planned_steps,
        report.errors.len()
    );
    Ok(if report.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn cmd_plan(args: &[String]) -> Result<ExitCode, String> {
    let args = Args::parse(args)?;
    let from = Position {
        map: parse_map(args.require("from-map")?)?,
        cell: parse_cell(args.require("from-cell")?)?,
    };
    let to_map = parse_map(args.require("to-map")?)?;
    let target = match args.one("to-cell") {
        Some(cell) => Target::Cell {
            map: to_map,
            cell: parse_cell(cell)?,
        },
        None => Target::Map(to_map),
    };
    let flags = args
        .all("flag")
        .into_iter()
        .map(|f| FlagRef::try_from(f.to_owned()).map(|f| f.0))
        .collect::<Result<Vec<_>, _>>()?;
    let data = load_pack(&pack_dir(&args))?;
    let mover = match args.one("vehicle") {
        Some(text) => match text
            .parse::<u16>()
            .map_err(|_| format!("bad vehicle {text:?}"))?
        {
            0 => Mover::Foot,
            index => Mover::Vehicle(index),
        },
        None => Mover::Foot,
    };
    let mut graph = MapGraph::new(&data, &flags)
        .map_err(|e| e.to_string())?
        .with_mover(mover);
    match graph.plan(from, target) {
        Ok(plan) => {
            print_plan(&plan);
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            println!("no plan: {error}");
            Ok(ExitCode::from(1))
        }
    }
}

fn runs(steps: &[Direction]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < steps.len() {
        let n = steps[i..].iter().take_while(|d| **d == steps[i]).count();
        out.push(format!("{:?} x{n}", steps[i]));
        i += n;
    }
    out.join(", ")
}

fn print_plan(plan: &Plan) {
    println!(
        "from map {:#05x} ({},{})",
        plan.from.map, plan.from.cell.x, plan.from.cell.y
    );
    for (n, leg) in plan.legs.iter().enumerate() {
        println!(
            "leg {}: map {:#05x} from ({},{}), {} steps: {}",
            n + 1,
            leg.map,
            leg.from.x,
            leg.from.y,
            leg.steps.len(),
            runs(&leg.steps)
        );
        println!(
            "  warp record {} (transition index {}) -> map {:#05x} arrive ({},{}) facing {:?}",
            leg.hop.record_offset,
            leg.hop.pack_index,
            leg.hop.target_map,
            leg.hop.arrival.x,
            leg.hop.arrival.y,
            leg.hop.facing
        );
    }
    if !plan.tail.is_empty() {
        println!(
            "final walk on map {:#05x}: {} steps: {}",
            plan.to.map,
            plan.tail.len(),
            runs(&plan.tail)
        );
    }
    println!(
        "arrive map {:#05x} ({},{}); {} warps, {} steps",
        plan.to.map,
        plan.to.cell.x,
        plan.to.cell.y,
        plan.legs.len(),
        plan.total_steps()
    );
}

#[cfg(test)]
mod save_probe_tests {
    use super::*;

    #[test]
    fn neutral_probe_count_defaults_to_zero_and_has_a_finite_limit() {
        assert_eq!(probe_frame_count(None), Ok(0));
        assert_eq!(probe_frame_count(Some(&"180".to_owned())), Ok(180));
        assert_eq!(probe_frame_count(Some(&"600".to_owned())), Ok(600));
        assert!(probe_frame_count(Some(&"601".to_owned())).is_err());
        assert!(probe_frame_count(Some(&"-1".to_owned())).is_err());
        assert!(probe_frame_count(Some(&"not-a-count".to_owned())).is_err());
    }
}
