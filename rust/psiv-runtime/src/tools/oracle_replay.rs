//! The oracle-tape field replay driver.
//!
//! `rust/psiv-runtime/src/bin/psiv-replay.rs` is the command line; this module
//! is the driving half of it. An oracle tape holds one `Input` per frame and
//! the engine is expected to reproduce the cartridge's field state frame for
//! frame (`docs/oracle/`, `docs/field/CAMERA.md`). Everything the cartridge
//! inherited from frames the engine cannot execute — the shared RNG seed, the
//! camera, the objects' positions and their half-finished steps — is *state*,
//! not play, so it is set here and the loop then hands the field one `Input` per
//! frame and samples the result. Nothing in this module is a game path: a game
//! is a [`Session`](crate::Session) and a pad.

use psiv_core::{
    Cell, CollisionType, Direction, FieldMap, FrameSample, Input, OBJECT_ID_LOADED, OBJECT_SLOTS,
    ObjectSample, OracleLog, PixelPos, ReplayRow, StepFrames, Tape,
};
use psiv_data::GameData;

use crate::Runtime;

/// What to replay, and the state the engine cannot derive.
pub struct ReplayRequest {
    /// The loaded pack.
    pub data: GameData,
    /// The map, spawn cell and facing the replay starts from.
    pub start: (u16, Cell, Direction),
    /// The field's step timing.
    pub step_frames: StepFrames,
    /// The shared RNG seed, as the oracle's own `rng_seed` column spells it.
    /// Without it the wander stream is arbitrary and every object column
    /// diverges on the first roll.
    pub seed: Option<u32>,
    /// Camera position to park at `(x, y)` before the first frame: a replay's
    /// alignment frame inherits the camera, because the engine cannot execute
    /// the opening scene that positioned it.
    pub camera: Option<(i32, i32)>,
    /// Restore every object from the log's own columns at this frame, instead
    /// of starting them at the pack's spawn state.
    pub objects: Option<(OracleLog, u32)>,
    /// Stop after this many replayed frames.
    pub limit: Option<u32>,
    /// Print-grade camera samples for frames in `lo..=hi`.
    pub trace_camera: Option<(u32, u32)>,
}

/// One camera sample, for the caller's `--trace-camera` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraTrace {
    /// The tape frame.
    pub frame: u32,
    /// The foreground plane's camera position, in pixels.
    pub position: (i32, i32),
    /// The foreground plane's raw 16.16 step counter.
    pub raw_step: (i32, i32),
    /// The leader's standing cell.
    pub leader: (u16, u16),
}

/// One replay: a row per frame, and what the field did on them.
pub struct Replay {
    /// The compared rows, in tape order.
    pub rows: Vec<ReplayRow>,
    /// The frame a scene took control on, if one did.
    pub scene_from: Option<u32>,
    /// The camera samples the request asked for.
    pub traces: Vec<CameraTrace>,
}

/// Replays `tape` from `align_frame`, sampling one row per frame.
///
/// # Errors
///
/// The requested start could not be built, or an object could not be restored
/// from the log's own columns.
pub fn replay(tape: &Tape, align_frame: u32, request: &ReplayRequest) -> Result<Replay, String> {
    let (map, spawn, facing) = request.start;
    let mut runtime = Runtime::new(
        request.data.clone(),
        map,
        spawn,
        facing,
        request.step_frames,
    )
    .map_err(|error| format!("runtime: {error}"))?;
    if let Some(seed) = request.seed {
        runtime.set_rng_seed(seed);
    }
    if let Some((x, y)) = request.camera {
        runtime.set_camera(x, y);
    }
    let mut static_bounds = vec![(0u8, 0u8); OBJECT_SLOTS];
    if let Some((log, frame)) = &request.objects {
        let (restored, bounds) = restore_objects_from(&mut runtime, log, *frame)?;
        static_bounds = bounds;
        eprintln!("restored {restored} objects from the log at frame {frame}");
    }

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
    let mut rows = Vec::new();
    let mut traces = Vec::new();

    for frame in tape.frames() {
        if frame.number < align_frame {
            continue;
        }
        if request.limit.is_some_and(|max| replayed >= max) {
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

        if let Some((lo, hi)) = request.trace_camera
            && (lo..=hi).contains(&frame.number)
        {
            let cam = runtime.camera();
            traces.push(CameraTrace {
                frame: frame.number,
                position: cam.position(),
                raw_step: cam.raw_step(),
                leader: (runtime.state().cell().x, runtime.state().cell().y),
            });
        }
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
            camera: {
                let cam = runtime.camera();
                let (x, y) = cam.position_on(psiv_core::CameraPlane::Foreground);
                let (sx, sy) = cam.raw_step_on(psiv_core::CameraPlane::Foreground);
                (x, y, sx, sy)
            },
            camera_bg: {
                let cam = runtime.camera();
                let (x, y) = cam.position_on(psiv_core::CameraPlane::Background);
                let (sx, sy) = cam.raw_step_on(psiv_core::CameraPlane::Background);
                (x, y, sx, sy)
            },
            camera_gates: {
                let gates = runtime.camera().gates();
                (gates.ec24, gates.ec25, gates.ec26)
            },
            camera_raw: {
                let cam = runtime.camera();
                cam.raw_on(psiv_core::CameraPlane::Foreground)
            },
            camera_bg_raw: {
                let cam = runtime.camera();
                cam.raw_on(psiv_core::CameraPlane::Background)
            },
        }));

        if runtime.scene_active() && scene_from.is_none() {
            scene_from = Some(frame.number);
        }
    }
    Ok(Replay {
        rows,
        scene_from,
        traces,
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
        let wanderer = runtime
            .wanderers()
            .iter()
            .find(|wanderer| wanderer.npc_index() == slot);
        let speed_frames = wanderer.map_or(
            psiv_core::WANDER_STEP_FRAMES,
            psiv_core::Wanderer::step_frames,
        );
        let (x_max, y_max) = wanderer.map_or((4, 4), |wanderer| {
            let leash = wanderer.leash();
            (leash.x_max, leash.y_max)
        });
        let leash = psiv_core::Leash {
            x_max,
            y_max,
            x: x_bnd,
            y: y_bnd,
        };

        // A running duration says how many frames into its step the object is:
        // it counts $1000 down to 0 in $80 steps.
        const FULL: u32 = 0x1000;
        let per_frame = FULL / u32::from(speed_frames);
        // A running duration says how many frames into its step the object is.
        // A wandering object faces the way it walks, so the facing is the
        // step's direction.
        let progress = if x_dur > 0 || y_dur > 0 {
            Some((((FULL - x_dur.max(y_dur)) / per_frame) as u8).max(1))
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
                (dx * n).div_euclid(i32::from(speed_frames)),
                (dy * n).div_euclid(i32::from(speed_frames)),
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
                psiv_core::WanderState { timer, leash, step },
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
fn object_samples(runtime: &Runtime, static_bounds: &[(u8, u8)]) -> Vec<ObjectSample> {
    let map = runtime.map();
    let wanderers = runtime.wanderers();
    let mut out = vec![ObjectSample::default(); OBJECT_SLOTS];

    for (slot, npc) in map.npcs().iter().enumerate().take(OBJECT_SLOTS) {
        let wanderer = wanderers.iter().find(|w| w.npc_index() == slot);
        let bespoke = runtime
            .bespoke_actors()
            .iter()
            .find(|actor| actor.npc_index() == slot);
        let bespoke_state = bespoke.filter(|actor| actor.kind().reports_state());
        // A mid-step object's pixels run out of the cell it left, because the
        // engine commits the destination cell the moment the step starts.
        let base = wanderer
            .and_then(psiv_core::Wanderer::step_origin)
            .or_else(|| bespoke.and_then(psiv_core::BespokeActor::step_origin))
            .unwrap_or(npc.cell);
        let at = PixelPos::from_cell(base);
        let (tx, ty) = wanderer
            .map(psiv_core::Wanderer::travelled_px)
            .or_else(|| bespoke.map(psiv_core::BespokeActor::travelled_px))
            .unwrap_or((0, 0));
        let (x_dur, y_dur) = wanderer
            .map(psiv_core::Wanderer::step_durations)
            .or_else(|| bespoke_state.map(psiv_core::BespokeActor::step_durations))
            .unwrap_or((0, 0));
        let leash = wanderer
            .map(psiv_core::Wanderer::leash)
            .or_else(|| bespoke_state.map(psiv_core::BespokeActor::leash));

        out[slot] = ObjectSample {
            id: OBJECT_ID_LOADED | npc.id.0,
            facing: psiv_core::facing_value(npc.facing),
            timer: wanderer
                .map(psiv_core::Wanderer::timer)
                .or_else(|| bespoke_state.map(psiv_core::BespokeActor::timer))
                .unwrap_or(0),
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
            offscreen: u8::from(runtime.object_offscreen(slot)),
        };
    }
    out
}
