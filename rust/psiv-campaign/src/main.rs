//! `psiv-campaign`: validate a route file or print a walking plan.
//!
//! ```text
//! psiv-campaign validate <route.json> [--pack DIR]
//! psiv-campaign plan --from-map M --from-cell X,Y --to-map N [--to-cell X,Y]
//!                    [--flag bank:id]... [--pack DIR]
//! ```
//!
//! Map ids are decimal or `0x` hex. The pack defaults to `$PSIV_PACK`, then
//! `runtime-pack` beside the repository root. Exit status: 0 success, 1 a
//! failed validation or an impossible plan, 2 a usage or input error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use psiv_campaign::route::{FlagRef, Route};
use psiv_campaign::validate::validate;
use psiv_campaign::{MapGraph, Plan, Position, Target};
use psiv_core::{Cell, Direction};
use psiv_data::{BattleFiles, GameData};

const USAGE: &str = "usage:\n  psiv-campaign validate <route.json> [--pack DIR]\n  \
psiv-campaign plan --from-map M --from-cell X,Y --to-map N [--to-cell X,Y] [--flag bank:id]... [--pack DIR]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
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
    let mut graph = MapGraph::new(&data, &flags).map_err(|e| e.to_string())?;
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
