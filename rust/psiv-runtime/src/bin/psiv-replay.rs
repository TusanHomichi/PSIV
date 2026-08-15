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
    Cell, CollisionType, Direction, FieldMap, FrameSample, Input, OBJECT_ID_LOADED, OBJECT_SLOTS,
    ObjectSample, OracleLog, PixelPos, ReplayRow, StepFrames, Tape, all_modelled_columns,
    csv_header,
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
    /// The RNG seed to start from, as the oracle's `rng_seed` column spells it
    /// (an eight-digit longword). Without it the wander stream is arbitrary and
    /// every object column will diverge on the first roll.
    seed: Option<u32>,
    /// Restore the map's objects from the log at the alignment frame, instead
    /// of starting them at their pack spawn cells.
    restore_objects: bool,
}

enum Align {
    Mark(String),
    Frame(u32),
}

fn usage() -> &'static str {
    "usage: psiv-replay --tape <t> --pack <dir> (--align-mark <m> | --align-frame <n>) \
     [--log <csv>] [--out <csv>] [--limit <frames>] [--skip <column>]... [--seed <hex>] [--restore-objects]"
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
    })
}

/// The raw collision value at `cell`, or 0 off the map.
fn collision(map: &FieldMap, cell: Cell) -> u8 {
    map.collision_at(cell).map_or(0, CollisionType::to_raw)
}

/// Seeds every object from the oracle's own columns at `frame`.
///
/// The engine cannot execute the opening scene, so by a replay's alignment
/// frame the cartridge's objects have wandered for thousands of frames: off
/// their spawn cells, leashes no longer centred, several mid-step. Starting
/// them at the pack's spawn state makes every object column diverge on frame
/// one for a reason that has nothing to do with the wander model.
fn restore_objects_from(
    runtime: &mut Runtime,
    log: &OracleLog,
    frame: u32,
) -> Result<(usize, Vec<(u8, u8)>), String> {
    let count = runtime.map().npcs().len();
    let mut restored = 0;
    let mut bounds = vec![(0u8, 0u8); OBJECT_SLOTS];
    for (slot, slot_bounds) in bounds.iter_mut().enumerate().take(count.min(OBJECT_SLOTS)) {
        let read = |kind: &str| log.get(frame, &psiv_core::object_column(slot, kind));
        let (Some(x_px), Some(y_px), Some(facing)) = (read("x_px"), read("y_px"), read("facing"))
        else {
            continue;
        };
        let x_px: i32 = x_px.parse().map_err(|_| format!("slot {slot}: bad x_px"))?;
        let y_px: i32 = y_px.parse().map_err(|_| format!("slot {slot}: bad y_px"))?;
        let facing = match facing.parse::<u16>() {
            Ok(0) => Direction::Down,
            Ok(4) => Direction::Up,
            Ok(8) => Direction::Right,
            Ok(12) => Direction::Left,
            _ => {
                return Err(format!(
                    "slot {slot}: facing {facing:?} is not one of 0/4/8/12"
                ));
            }
        };
        let timer: i16 = read("timer").and_then(|v| v.parse().ok()).unwrap_or(0);
        let x_dur: u32 = read("xdur").and_then(|v| v.parse().ok()).unwrap_or(0);
        let y_dur: u32 = read("ydur").and_then(|v| v.parse().ok()).unwrap_or(0);
        let x_bnd: u8 = read("xbnd").and_then(|v| v.parse().ok()).unwrap_or(2);
        let y_bnd: u8 = read("ybnd").and_then(|v| v.parse().ok()).unwrap_or(2);

        // A running duration says how many frames into its step the object is:
        // it counts $1000 down to 0 in $80 steps.
        const FULL: u32 = 0x1000;
        const PER_FRAME: u32 = FULL / psiv_core::WANDER_STEP_FRAMES as u32;
        // A running duration says how many frames into its step the object is.
        // A wandering object faces the way it walks, so the facing is the
        // step's direction.
        let progress = if x_dur > 0 || y_dur > 0 {
            Some((((FULL - x_dur.max(y_dur)) / PER_FRAME) as u8).max(1))
        } else {
            None
        };

        // Undo the part-cell travel to recover the origin cell. The travel is
        // floored, not truncated, so this cannot be done by dividing the
        // current pixel: an object one frame into a leftward step reads a pixel
        // *past* its origin while one stepping down reads its origin exactly.
        let (dx, dy) = facing.delta();
        let (tx, ty) = progress.map_or((0, 0), |p| {
            let n = i32::from(p) * 16;
            (
                (dx * n).div_euclid(i32::from(psiv_core::WANDER_STEP_FRAMES)),
                (dy * n).div_euclid(i32::from(psiv_core::WANDER_STEP_FRAMES)),
            )
        });
        let origin = Cell::new(
            u16::try_from(((x_px - tx).max(0)) / 16).map_err(|_| "origin x")?,
            u16::try_from((((y_px - ty).max(0)) / 16) + 1).map_err(|_| "origin y")?,
        );
        // The engine commits the destination at step start, so that is the
        // object's cell.
        let cell = match progress {
            Some(_) => runtime.map().neighbor(origin, facing).unwrap_or(origin),
            None => origin,
        };
        let step = progress.map(|p| (facing, p, origin));

        runtime
            .restore_object(
                slot,
                cell,
                facing,
                psiv_core::WanderState {
                    timer,
                    leash: psiv_core::Leash {
                        x_max: 4,
                        y_max: 4,
                        x: x_bnd,
                        y: y_bnd,
                    },
                    step,
                },
            )
            .map_err(|e| format!("slot {slot}: {e}"))?;
        *slot_bounds = (x_bnd, y_bnd);
        restored += 1;
    }
    Ok((restored, bounds))
}

/// The map's objects in the oracle's slot order: slot `i` is `npcs[i]`.
///
/// A wandering object contributes its timer, durations and leash; everything
/// else reads zero there, which is what the cartridge shows for an object whose
/// routine never touches them. Empty slots past the map's object count are all
/// zero, id included.
fn object_samples(
    runtime: &psiv_runtime::Runtime,
    static_bounds: &[(u8, u8)],
) -> Vec<ObjectSample> {
    let map = runtime.map();
    let wanderers = runtime.wanderers();
    let mut out = vec![ObjectSample::default(); OBJECT_SLOTS];

    for (slot, npc) in map.npcs().iter().enumerate().take(OBJECT_SLOTS) {
        let wanderer = wanderers.iter().find(|w| w.npc_index() == slot);
        // A mid-step object's pixels run out of the cell it left, because the
        // engine commits the destination cell the moment the step starts.
        let base = wanderer
            .and_then(psiv_core::Wanderer::step_origin)
            .unwrap_or(npc.cell);
        let at = PixelPos::from_cell(base);
        let (tx, ty) = wanderer.map_or((0, 0), psiv_core::Wanderer::travelled_px);
        let (x_dur, y_dur) = wanderer.map_or((0, 0), psiv_core::Wanderer::step_durations);
        let leash = wanderer.map(psiv_core::Wanderer::leash);

        out[slot] = ObjectSample {
            id: OBJECT_ID_LOADED | npc.id.0,
            facing: psiv_core::facing_value(npc.facing),
            timer: wanderer.map_or(0, psiv_core::Wanderer::timer),
            x_dur,
            y_dur,
            x_px: at.x + tx,
            y_px: at.y + ty,
            // A non-wandering object still has boundary bytes, written once by
            // its own init routine and never touched again — Alys on the
            // academy floor reads 8/8. The engine models no state for those
            // objects, so the comparator carries whatever the restore read.
            x_bnd: leash.map_or_else(|| static_bounds.get(slot).map_or(0, |b| b.0), |l| l.x),
            y_bnd: leash.map_or_else(|| static_bounds.get(slot).map_or(0, |b| b.1), |l| l.y),
        };
    }
    out
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

    if let Some(seed) = args.seed {
        runtime.set_rng_seed(seed);
        eprintln!("seeded the RNG with {seed:08X}");
    }

    let mut static_bounds = vec![(0u8, 0u8); OBJECT_SLOTS];
    if args.restore_objects {
        let Some(log_path) = &args.log else {
            return Err("--restore-objects needs --log to read the state from".into());
        };
        let text = std::fs::read_to_string(log_path)
            .map_err(|e| format!("{}: {e}", log_path.display()))?;
        let log = OracleLog::parse(&text);
        // The log samples at end-of-frame, so the state the replay must start
        // from is the row *before* the alignment frame — the same convention
        // the RNG seed uses.
        let from = align_frame.saturating_sub(1);
        let (restored, bounds) = restore_objects_from(&mut runtime, &log, from)?;
        static_bounds = bounds;
        eprintln!("restored {restored} objects from the log at frame {from}");
    }

    // Replay from the alignment frame on. Everything before it is boot, which
    // the engine has no way to reproduce.
    let mut rows = Vec::new();
    // `FieldRoutine_Controls` refreshes the standing and neighbour collision
    // caches only when both step durations read zero *at the top of the frame*
    // (`UpdateCharacterStandCollision` / `UpdateCharacterCollision` sit behind
    // that gate). So the caches lag a landing by exactly one frame: the frame
    // the step completes still reports the pre-step neighbours, and the frame
    // after picks up the new ones. Sampling them live instead is a one-frame
    // error at every landing that changes a neighbour.
    let mut cached_standing = collision(runtime.map(), runtime.state().cell());
    let mut previous_standing = cached_standing;
    let mut cached_neighbours = neighbours(runtime.map(), runtime.state().cell());
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

        // Top of frame: refresh the caches only if the party is at rest, using
        // the position it holds *before* this frame's movement.
        if !runtime.state().is_stepping() {
            let map = runtime.map();
            let cell = runtime.state().cell();
            previous_standing = cached_standing;
            cached_standing = collision(map, cell);
            cached_neighbours = neighbours(map, cell);
        }

        let input: Input = frame.buttons.to_input();
        runtime.tick(input);

        let state = runtime.state();
        let follower = runtime
            .members()
            .get(1)
            .map(|m| (m.facing, PixelPos::from_cell(m.cell)));
        let objects = object_samples(&runtime, &static_bounds);

        rows.push(ReplayRow::from_sample(FrameSample {
            frame: frame.number,
            mark: frame.mark.as_deref(),
            buttons: frame.buttons,
            map_index: runtime.map_id().0,
            state,
            follower,
            standing: cached_standing,
            previously_standing: previous_standing,
            neighbours: cached_neighbours,
            game: runtime.game(),
            objects: &objects,
        }));

        if runtime.scene_active() && scene_from.is_none() {
            scene_from = Some(frame.number);
        }
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
