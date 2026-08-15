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

use psiv_core::{
    Cell, CollisionType, Direction, FieldMap, FrameSample, Input, OracleLog, ReplayRow, StepFrames,
    Tape, csv_header, modelled_columns,
};
use psiv_data::GameData;
use psiv_runtime::Runtime;

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
}

enum Align {
    Mark(String),
    Frame(u32),
}

fn usage() -> &'static str {
    "usage: psiv-replay --tape <t> --pack <dir> (--align-mark <m> | --align-frame <n>) \
     [--log <csv>] [--out <csv>] [--limit <frames>] [--skip <column>]..."
}

fn parse_args() -> Result<Args, String> {
    let mut tape = None;
    let mut pack = None;
    let mut log = None;
    let mut out = None;
    let mut align = None;
    let mut limit = None;
    let mut skip = Vec::new();

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
    })
}

/// The raw collision value at `cell`, or 0 off the map.
fn collision(map: &FieldMap, cell: Cell) -> u8 {
    map.collision_at(cell).map_or(0, CollisionType::to_raw)
}

/// Left, up, right, down — the order `UpdateCharacterCollision` caches them.
fn neighbours(map: &FieldMap, cell: Cell) -> [u8; 4] {
    [
        Direction::Left,
        Direction::Up,
        Direction::Right,
        Direction::Down,
    ]
    .map(|dir| map.neighbor(cell, dir).map_or(0, |c| collision(map, c)))
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

    let mut runtime = Runtime::new(data, start.map.id, spawn, facing, StepFrames::default())
        .map_err(|e| format!("runtime: {e}"))?;

    // Replay from the alignment frame on. Everything before it is boot, which
    // the engine has no way to reproduce.
    let mut rows = Vec::new();
    let mut previous_standing = collision(runtime.map(), runtime.state().cell());
    let mut replayed = 0;
    let mut scene_from: Option<u32> = None;

    for frame in tape.frames() {
        if frame.number < align_frame {
            continue;
        }
        if args.limit.is_some_and(|max| replayed >= max) {
            break;
        }
        replayed += 1;

        let input: Input = frame.buttons.to_input();
        runtime.tick(input);

        let map = runtime.map();
        let state = runtime.state();
        let cell = state.cell();
        let standing = collision(map, cell);
        let follower = runtime
            .members()
            .get(1)
            .map(|m| (m.facing, psiv_core::PixelPos::from_cell(m.cell)));

        rows.push(ReplayRow::from_sample(FrameSample {
            frame: frame.number,
            mark: frame.mark.as_deref(),
            buttons: frame.buttons,
            map_index: runtime.map_id().0,
            state,
            follower,
            standing,
            previously_standing: previous_standing,
            neighbours: neighbours(map, cell),
            game: runtime.game(),
        }));
        previous_standing = standing;

        if runtime.scene_active() && scene_from.is_none() {
            scene_from = Some(frame.number);
        }
    }

    if let Some(path) = &args.out {
        let mut text = csv_header();
        text.push('\n');
        let columns = modelled_columns();
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
    let divergences = log.diff(&rows, &skip);
    if divergences.is_empty() {
        println!(
            "CLEAN: {} frames from {align_frame}, {} columns, zero divergences",
            rows.len(),
            modelled_columns().len() - 2
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
