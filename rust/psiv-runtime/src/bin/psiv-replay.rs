//! Replay an oracle tape through the engine and diff it against the log.
//!
//! The machinery is in `psiv_core::replay`, which is pure; this binary is only
//! the file handling — read a tape, load the pack, drive [`Runtime`], write
//! CSV, compare. Keeping it here rather than in `psiv-core` is what lets the
//! core stay I/O-free while the comparator still gets real map collision out
//! of the pack through `psiv-data`.
//!
//! ```text
//! psiv-replay --tape oracle/tapes/02_walk_timing.tape \
//!             --pack runtime-pack \
//!             --align-mark settle \
//!             [--log oracle/logs/02_walk_timing.csv] \
//!             [--out replay.csv]
//! ```
//!
//! Exit status is 0 when the diff is clean (or when no log was given), 1 on a
//! divergence, 2 on a usage or data error. The first divergence is the
//! product: either movement is bit-exact or we learn precisely where it is not.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use psiv_core::{Cell, Direction, OracleLog, StepFrames, Tape, all_modelled_columns, csv_header};
use psiv_data::GameData;
use psiv_runtime::tools::oracle_replay::{ReplayRequest, replay};

struct Args {
    tape: PathBuf,
    pack: PathBuf,
    log: Option<PathBuf>,
    out: Option<PathBuf>,
    align: Align,
    /// Stop after this many replayed frames; the field segment of a boot tape
    /// is short and there is no point walking the rest.
    limit: Option<u32>,
    /// Columns to leave out of the comparison. Every exclusion is named on
    /// stdout, so a clean verdict always says what it ignored.
    skip: Vec<String>,
    /// The RNG seed to start from, as the oracle's `rng_seed` column spells it
    /// (an eight-digit longword). Without it the wander stream is arbitrary and
    /// every object column will diverge on the first roll.
    seed: Option<u32>,
    /// Restore the map's objects from the log at the alignment frame, instead
    /// of starting them at their pack spawn cells.
    restore_objects: bool,
    /// Print the camera position for frames in `lo..=hi`, including raw
    /// 16.16 positions and step counters.
    trace_camera: Option<(u32, u32)>,
    /// Take the starting map, position and facing from the log at the
    /// alignment frame instead of from the pack's `game_start`.
    ///
    /// The alignment contract already treats the RNG seed, the camera and the
    /// objects as state inherited from frames the engine cannot replay. A tape
    /// that walks to another map before doing anything interesting makes the
    /// party's own position inherited state too — without this, aligning
    /// mid-tape fails on `map_index` before comparing anything.
    start_from_log: bool,
    /// Park the camera at `x,y` before replaying. The opening scene positions
    /// the retail camera and the engine cannot execute it, so at an alignment
    /// frame the camera is inherited state like the RNG seed is.
    camera: Option<(i32, i32)>,
}

enum Align {
    Mark(String),
    Frame(u32),
}

fn usage() -> &'static str {
    "usage: psiv-replay --tape <t> --pack <dir> (--align-mark <m> | --align-frame <n>) \
     [--log <csv>] [--out <csv>] [--limit <frames>] [--skip <column>]... [--seed <hex>] [--restore-objects] [--start-from-log] [--camera <x,y>] [--trace-camera <lo,hi>]"
}

fn parse_args() -> Result<Args, String> {
    let mut tape = None;
    let mut pack = None;
    let mut log = None;
    let mut out = None;
    let mut align = None;
    let mut limit = None;
    let mut skip = Vec::new();
    let mut seed = None;
    let mut restore_objects = false;
    let mut start_from_log = false;
    let mut camera = None;
    let mut trace_camera = None;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--tape" => tape = Some(PathBuf::from(value()?)),
            "--pack" => pack = Some(PathBuf::from(value()?)),
            "--log" => log = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--align-mark" => align = Some(Align::Mark(value()?)),
            "--align-frame" => {
                let raw = value()?;
                align = Some(Align::Frame(
                    raw.parse().map_err(|_| format!("bad frame {raw:?}"))?,
                ));
            }
            "--skip" => skip.push(value()?),
            "--restore-objects" => restore_objects = true,
            "--start-from-log" => start_from_log = true,
            "--trace-camera" => {
                let raw = value()?;
                let (lo, hi) = raw
                    .split_once(',')
                    .ok_or_else(|| format!("bad frame window {raw:?}, want lo,hi"))?;
                trace_camera = Some((
                    lo.trim().parse().map_err(|_| format!("bad frame {lo:?}"))?,
                    hi.trim().parse().map_err(|_| format!("bad frame {hi:?}"))?,
                ));
            }
            "--camera" => {
                let raw = value()?;
                let (x, y) = raw
                    .split_once(',')
                    .ok_or_else(|| format!("bad camera {raw:?}, want x,y"))?;
                camera = Some((
                    x.trim()
                        .parse()
                        .map_err(|_| format!("bad camera x {x:?}"))?,
                    y.trim()
                        .parse()
                        .map_err(|_| format!("bad camera y {y:?}"))?,
                ));
            }
            "--seed" => {
                let raw = value()?;
                let text = raw.trim_start_matches("0x");
                seed =
                    Some(u32::from_str_radix(text, 16).map_err(|_| format!("bad seed {raw:?}"))?);
            }
            "--limit" => {
                let raw = value()?;
                limit = Some(raw.parse().map_err(|_| format!("bad limit {raw:?}"))?);
            }
            other => return Err(format!("unknown flag {other:?}\n{}", usage())),
        }
    }

    Ok(Args {
        tape: tape.ok_or_else(|| usage().to_string())?,
        pack: pack.ok_or_else(|| usage().to_string())?,
        log,
        out,
        align: align.ok_or_else(|| usage().to_string())?,
        limit,
        skip,
        seed,
        restore_objects,
        start_from_log,
        camera,
        trace_camera,
    })
}

fn run() -> Result<bool, String> {
    let args = parse_args()?;

    let tape_text =
        std::fs::read_to_string(&args.tape).map_err(|e| format!("{}: {e}", args.tape.display()))?;
    let tape = Tape::parse(&tape_text).map_err(|e| format!("{}: {e}", args.tape.display()))?;

    let align_frame = match &args.align {
        Align::Frame(frame) => *frame,
        Align::Mark(mark) => tape
            .mark_frame(mark)
            .ok_or_else(|| format!("tape has no mark {mark:?}"))?,
    };

    let data = GameData::load(Path::new(&args.pack)).map_err(|e| format!("pack: {e}"))?;
    let start = data
        .manifest()
        .game_start
        .clone()
        .ok_or("the pack has no game_start record")?;
    let spawn = Cell::new(
        u16::try_from(start.x_cell).map_err(|_| "game_start x out of range")?,
        u16::try_from(start.y_cell).map_err(|_| "game_start y out of range")?,
    );
    let facing = match start.facing.id {
        0x0 => Direction::Down,
        0x4 => Direction::Up,
        0x8 => Direction::Right,
        0xC => Direction::Left,
        other => {
            return Err(format!(
                "game_start facing byte {other:#04X} is not one of 0/4/8/$C"
            ));
        }
    };

    // The alignment frame's own state, when the tape has walked somewhere the
    // engine cannot reach on its own.
    let (start_map, spawn, facing) = if args.start_from_log {
        let log_path = args
            .log
            .as_ref()
            .ok_or("--start-from-log needs --log to read the starting state from")?;
        let text = std::fs::read_to_string(log_path)
            .map_err(|e| format!("{}: {e}", log_path.display()))?;
        let log = OracleLog::parse(&text);
        let read = |column: &str| {
            log.get(align_frame, column)
                .ok_or_else(|| format!("the log has no {column} at frame {align_frame}"))
        };
        let map = u16::from_str_radix(read("map_index")?.trim(), 16)
            .map_err(|_| "bad map_index in the log")?;
        let x: i32 = read("c1_x_px")?.parse().map_err(|_| "bad c1_x_px")?;
        let y: i32 = read("c1_y_px")?.parse().map_err(|_| "bad c1_y_px")?;
        let cell = Cell::new(
            u16::try_from(x / 16).map_err(|_| "log x out of range")?,
            // The standing-cell shift: the occupied cell is one row below
            // `curr_y_pos / 16`.
            u16::try_from(y / 16 + 1).map_err(|_| "log y out of range")?,
        );
        let facing = match read("c1_facing")?.trim().parse::<u16>() {
            Ok(0) => Direction::Down,
            Ok(4) => Direction::Up,
            Ok(8) => Direction::Right,
            Ok(12) => Direction::Left,
            other => return Err(format!("log facing {other:?} is not one of 0/4/8/12")),
        };
        eprintln!("starting from the log: map {map:#06X} at {cell:?} facing {facing:?}");
        (map, cell, facing)
    } else {
        (start.map.id, spawn, facing)
    };

    if let Some(seed) = args.seed {
        eprintln!("seeded the RNG with {seed:08X}");
    }
    if let Some((x, y)) = args.camera {
        eprintln!("parked the camera at {x},{y}");
    }
    // The objects a replay inherits: the log's own columns at the frame before
    // the alignment one, because the log samples at end-of-frame.
    let restore = match (args.restore_objects, &args.log) {
        (false, _) => None,
        (true, None) => return Err("--restore-objects needs --log to read the state from".into()),
        (true, Some(log_path)) => {
            let text = std::fs::read_to_string(log_path)
                .map_err(|e| format!("{}: {e}", log_path.display()))?;
            Some((OracleLog::parse(&text), align_frame.saturating_sub(1)))
        }
    };
    let request = ReplayRequest {
        data,
        start: (start_map, spawn, facing),
        step_frames: StepFrames::default(),
        seed: args.seed,
        camera: args.camera,
        objects: restore,
        limit: args.limit,
        trace_camera: args.trace_camera,
    };
    let replayed = replay(&tape, align_frame, &request)?;
    let rows = replayed.rows;
    let scene_from = replayed.scene_from;
    for trace in &replayed.traces {
        eprintln!(
            "cam f{} pos={:?} step={:?} leader=({},{})",
            trace.frame, trace.position, trace.raw_step, trace.leader.0, trace.leader.1
        );
    }

    if let Some(path) = &args.out {
        let mut text = csv_header();
        text.push('\n');
        let columns = all_modelled_columns();
        for row in &rows {
            text.push_str(&row.to_csv(&columns));
            text.push('\n');
        }
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("wrote {} frames to {}", rows.len(), path.display());
    }

    if let Some(frame) = scene_from {
        eprintln!(
            "note: a scene took control at frame {frame}; the engine stops walking there. \
             If the oracle kept moving, the pack's starting state is missing a flag that \
             would have suppressed the trigger."
        );
    }

    let Some(log_path) = &args.log else {
        eprintln!(
            "replayed {} frames from {align_frame}; no log to diff",
            rows.len()
        );
        return Ok(true);
    };

    let log_text =
        std::fs::read_to_string(log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    let log = OracleLog::parse(&log_text);

    // The alignment assertion: if the engine's starting state disagrees with
    // the oracle's row at the alignment frame, nothing after it is meaningful.
    let mut misaligned = Vec::new();
    if let Some(first) = rows.first() {
        for column in [
            "map_index",
            "c1_x_px",
            "c1_y_px",
            "c1_facing",
            "party_slots",
        ] {
            let (Some(engine), Some(oracle)) = (first.field(column), log.get(first.frame, column))
            else {
                continue;
            };
            if engine != oracle {
                misaligned.push(format!("  {column}: engine={engine} oracle={oracle}"));
            }
        }
    }
    if !misaligned.is_empty() {
        eprintln!(
            "ALIGNMENT MISMATCH at frame {align_frame} — the pack's game_start and the tape \
             disagree about where the game begins:\n{}",
            misaligned.join("\n")
        );
        return Ok(false);
    }

    let skip: Vec<&str> = args.skip.iter().map(String::as_str).collect();
    if !skip.is_empty() {
        println!("skipping columns: {}", skip.join(", "));
    }
    let report = log.compare(&rows, &skip);
    if !report.unavailable.is_empty() {
        println!(
            "not in the log, so not compared: {}",
            report.unavailable.join(", ")
        );
    }
    let divergences = report.divergences;
    if divergences.is_empty() {
        println!(
            "CLEAN: {} frames from {align_frame}, {} columns compared, zero divergences",
            rows.len(),
            report.compared.len()
        );
        return Ok(true);
    }

    println!(
        "DIVERGENT: {} of {} frames from {align_frame}",
        divergences
            .iter()
            .map(|d| d.frame)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        rows.len()
    );
    println!("first: {}", divergences[0]);

    let mut by_column: HashMap<&str, (usize, u32)> = HashMap::new();
    for divergence in &divergences {
        let entry = by_column
            .entry(divergence.column.as_str())
            .or_insert((0, divergence.frame));
        entry.0 += 1;
    }
    let mut summary: Vec<_> = by_column.into_iter().collect();
    summary.sort_by_key(|(name, _)| *name);
    for (column, (count, first)) in summary {
        println!("  {column}: {count} frames, first at {first}");
    }
    Ok(false)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("psiv-replay: {message}");
            ExitCode::from(2)
        }
    }
}
