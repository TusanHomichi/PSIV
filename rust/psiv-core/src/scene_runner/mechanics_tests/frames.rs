//! Which tick spends each retail frame of the platforms and belts (#600).
//!
//! A platform routine waits `DMAPlane_A_VInt` after its start pair, sixty
//! settle frames, one `VInt_Prepare` per ride frame and `DMAPlane_A_VInt`
//! after its end pair: `62 + frames` frames. A belt's carry loop waits only
//! its `DMAPlane_A_VInt`, once per pixel. The timelines here follow the
//! runner's program counter, so each op's frames are the ticks until the op
//! after it runs, and the leader's pixel position says which of those ticks
//! move the party. `scenes/dma_census.rs` ties the same calls to the bytes.

use super::*;
use crate::CELL_PIXELS;

/// One blocking op and the frames until the scene's next op runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spent {
    op: SceneOp,
    frames: usize,
}

/// What a scene did: each blocking op with its frames, the leader's pixel
/// position after every tick, and the ticks.
struct Timeline {
    spent: Vec<Spent>,
    leader: Vec<(i32, i32)>,
    ticks: usize,
}

fn leader_pixel(runner: &SceneRunner) -> (i32, i32) {
    let leader = runner.actor(ActorRef::PartyMember(0)).unwrap();
    let (dx, dy) = leader.render_offset_16ths(StepFrames::default());
    (
        i32::from(leader.cell.x) * CELL_PIXELS + dx,
        i32::from(leader.cell.y) * CELL_PIXELS + dy,
    )
}

fn timeline(
    ops: &'static [SceneOp],
    state: &mut GameState,
    cast: Vec<ScriptedActor>,
    chunks: &dyn Fn(PixelPos) -> Option<u16>,
) -> Timeline {
    let map = big_map();
    let mut runner = SceneRunner::new(ops, cast, StepFrames::default());
    let mut leader = vec![leader_pixel(&runner)];
    // `(tick, the op that blocked it)` for every tick that ran ops.
    let mut blocked_by = Vec::new();
    let mut ticks = 0;
    while !runner.is_finished() {
        ticks += 1;
        assert!(ticks < 2_000, "the scene did not finish");
        let before = runner.pc;
        let effects = runner.tick_with(&map, state, SceneInput::None, Some(chunks));
        assert!(
            !effects.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
            "{effects:?}"
        );
        if runner.pc != before && !runner.is_finished() {
            blocked_by.push((ticks, ops[runner.pc - 1].clone()));
        }
        leader.push(leader_pixel(&runner));
    }
    let mut spent = Vec::new();
    for (index, (tick, op)) in blocked_by.iter().enumerate() {
        let next = blocked_by.get(index + 1).map_or(ticks, |(t, _)| *t);
        spent.push(Spent {
            op: op.clone(),
            frames: next - *tick,
        });
    }
    Timeline {
        spent,
        leader,
        ticks,
    }
}

/// Each tick's leader movement, `(dx, dy)` pixels.
fn steps(line: &Timeline) -> Vec<(i32, i32)> {
    line.leader
        .windows(2)
        .map(|w| (w[1].0 - w[0].0, w[1].1 - w[0].1))
        .collect()
}

/// Where a platform's flag state leaves the party standing, and the ride's
/// sign.
fn platform_start(platform: &Platform, set: bool) -> (Cell, i32) {
    let rows = i32::from(platform.frames) / 8;
    let clear_sign = if platform.clear_down { 1 } else { -1 };
    if set {
        let row = i32::from(platform.clear_cell.y) + clear_sign * rows;
        (Cell::new(platform.clear_cell.x, row as u16), -clear_sign)
    } else {
        (platform.clear_cell, clear_sign)
    }
}

/// What is wrong with a platform scene's frames: they must be the start
/// pair's DMA frame, the 60-frame settle, the ride (two pixels on each of its
/// frames, and the party still on every other) and the end pair's DMA frame.
fn platform_problems(ops: &'static [SceneOp], platform: &Platform, set: bool) -> Vec<String> {
    let mut state = state_with(&[CHAZ, RUNE, RIKA]);
    if set {
        state.set(Flag::temp(platform.flag)).unwrap();
    }
    let (from, sign) = platform_start(platform, set);
    let line = timeline(ops, &mut state, cast_at(3, from), &no_layout);
    let label = format!("${:02X} set={set}", platform.event);
    let mut problems = Vec::new();
    let frames = usize::from(platform.frames);
    let kinds: Vec<(&str, usize)> = line
        .spent
        .iter()
        .map(|s| {
            let kind = match s.op {
                SceneOp::DmaPlanes => "DmaPlanes",
                SceneOp::WaitFrames { .. } => "WaitFrames",
                SceneOp::Ride(RideOp::Platform { .. }) => "Ride",
                _ => "other",
            };
            (kind, s.frames)
        })
        .collect();
    let want = [
        ("DmaPlanes", 1),
        ("WaitFrames", 60),
        ("Ride", frames),
        ("DmaPlanes", 1),
    ];
    if kinds != want {
        problems.push(format!("{label}: spends {kinds:?}"));
    }
    // The ride's frames are the ticks after it starts, up to and including
    // the tick the end pair is written: two pixels each, and only those.
    let ride_start: usize = line.spent.iter().take(2).map(|s| s.frames).sum::<usize>() + 1;
    for (index, &(dx, dy)) in steps(&line).iter().enumerate() {
        let tick = index + 1;
        let riding = tick > ride_start && tick <= ride_start + frames;
        let want = if riding { (0, 2 * sign) } else { (0, 0) };
        if (dx, dy) != want {
            problems.push(format!("{label}: tick {tick} moved ({dx}, {dy})"));
        }
    }
    if line.ticks != frames + 63 {
        problems.push(format!("{label}: {} ticks", line.ticks));
    }
    problems
}

/// The ops with `op` inserted at `at`, every branch past `at` retargeted (a
/// branch to `at` lands on the new op).
fn insert(ops: &[SceneOp], at: usize, op: SceneOp) -> &'static [SceneOp] {
    let shift = |to: usize| if to > at { to + 1 } else { to };
    let mut out: Vec<SceneOp> = ops
        .iter()
        .map(|op| match op {
            &SceneOp::Jump { to } => SceneOp::Jump { to: shift(to) },
            &SceneOp::BranchFlag {
                flag,
                if_set,
                if_clear,
            } => SceneOp::BranchFlag {
                flag,
                if_set: shift(if_set),
                if_clear: shift(if_clear),
            },
            other => other.clone(),
        })
        .collect();
    out.insert(at, op);
    out.leak()
}

/// The ops with op `at` removed, every branch past it retargeted.
fn remove(ops: &[SceneOp], at: usize) -> &'static [SceneOp] {
    let shift = |to: usize| if to > at { to - 1 } else { to };
    let mut out: Vec<SceneOp> = ops
        .iter()
        .map(|op| match op {
            &SceneOp::Jump { to } => SceneOp::Jump { to: shift(to) },
            &SceneOp::BranchFlag {
                flag,
                if_set,
                if_clear,
            } => SceneOp::BranchFlag {
                flag,
                if_set: shift(if_set),
                if_clear: shift(if_clear),
            },
            other => other.clone(),
        })
        .collect();
    out.remove(at);
    out.leak()
}

/// All six platforms, both directions: one frame for each `DMAPlane_A_VInt`,
/// sixty for the settle, the ride's frames at two pixels each, and no second
/// frame on any ride tick.
#[test]
fn every_platform_spends_both_dma_frames_the_settle_and_the_ride_in_both_directions() {
    let mut problems = Vec::new();
    for platform in &PLATFORMS {
        let ops = scene_for(EventIndex(platform.event)).unwrap().ops;
        for set in [false, true] {
            problems.extend(platform_problems(ops, platform, set));
        }
    }
    assert_eq!(problems, Vec::<String>::new());
}

/// Negative controls: a platform scene with either `DmaPlanes` left out, or
/// with one doubled, is named, in both directions.
#[test]
fn a_platform_with_a_lost_or_doubled_dma_frame_is_named() {
    let platform = &PLATFORMS[3];
    let ops = scene_for(EventIndex(platform.event)).unwrap().ops;
    let dma: Vec<usize> = (0..ops.len())
        .filter(|&i| matches!(ops[i], SceneOp::DmaPlanes))
        .collect();
    assert_eq!(dma.len(), 2);
    for at in dma {
        for (name, changed) in [
            ("lost", remove(ops, at)),
            ("doubled", insert(ops, at, SceneOp::DmaPlanes)),
        ] {
            for set in [false, true] {
                let problems = platform_problems(changed, platform, set);
                assert!(
                    problems.iter().any(|p| p.contains("spends")),
                    "{name} op {at} set={set}: {problems:?}"
                );
                assert!(
                    problems.iter().any(|p| p.contains("ticks")),
                    "{name} op {at} set={set}: {problems:?}"
                );
            }
        }
    }
}

/// What is wrong with a belt scene's frames: the carry is its one blocking
/// op, each of its frames moves the leader one pixel along the belt (the
/// carry loop's pass, which ends in `DMAPlane_A_VInt`), and no other tick
/// moves anyone.
fn belt_problems(ops: &'static [SceneOp], event: u16, start: Cell, cells: i32) -> Vec<String> {
    let vertical = belt_layout(3..=3, 10..=14, 0xA8);
    let horizontal = belt_layout(10..=12, 5..=5, 0xAC);
    let layout: &dyn Fn(PixelPos) -> Option<u16> = if event <= 0x1E {
        &vertical
    } else {
        &horizontal
    };
    let mut state = state_with(&[CHAZ, RUNE, RIKA]);
    let line = timeline(ops, &mut state, cast_at(3, start), layout);
    let (dx, dy) = match event {
        0x1D => (0, 1),
        0x1E => (0, -1),
        0x1F => (1, 0),
        _ => (-1, 0),
    };
    let label = format!("${event:02X}");
    let mut problems = Vec::new();
    let pixels = (cells * CELL_PIXELS) as usize;
    let spent: Vec<(bool, usize)> = line
        .spent
        .iter()
        .map(|s| {
            (
                matches!(s.op, SceneOp::Ride(RideOp::Conveyor { .. })),
                s.frames,
            )
        })
        .collect();
    if spent != [(true, pixels)] {
        problems.push(format!("{label}: spends {spent:?}"));
    }
    for (index, &step) in steps(&line).iter().enumerate() {
        let tick = index + 1;
        let want = if (2..=pixels + 1).contains(&tick) {
            (dx, dy)
        } else {
            (0, 0)
        };
        if step != want {
            problems.push(format!("{label}: tick {tick} moved {step:?}"));
        }
    }
    if line.ticks != pixels + 1 {
        problems.push(format!("{label}: {} ticks", line.ticks));
    }
    problems
}

/// The four belts, one per direction (the layouts of
/// `a_belt_carries_the_leader_to_the_first_cell_off_it_at_one_pixel_a_frame`):
/// the event, the start, and the cells to the first one off the belt.
const BELTS: [(u16, Cell, i32); 4] = [
    (0x1D, Cell::new(6, 21), 9),
    (0x1E, Cell::new(7, 28), 9),
    (0x1F, Cell::new(20, 11), 6),
    (0x20, Cell::new(25, 11), 6),
];

/// All four belts: every carry tick is one pixel, the carry loop's one frame,
/// and the scene spends no other.
#[test]
fn every_belt_spends_one_frame_a_pixel_and_no_other() {
    let mut problems = Vec::new();
    for (event, start, cells) in BELTS {
        let ops = scene_for(EventIndex(event)).unwrap().ops;
        problems.extend(belt_problems(ops, event, start, cells));
    }
    assert_eq!(problems, Vec::<String>::new());
}

/// Negative control: a `DmaPlanes` for the carry loop's call, after the
/// carry or before it, spends a frame retail does not, and is named.
#[test]
fn a_belt_with_a_dma_frame_of_its_own_is_named() {
    for (event, start, cells) in BELTS {
        let ops = scene_for(EventIndex(event)).unwrap().ops;
        let carry = ops
            .iter()
            .position(|op| matches!(op, SceneOp::Ride(RideOp::Conveyor { .. })))
            .unwrap();
        for at in [carry, carry + 1] {
            let problems = belt_problems(insert(ops, at, SceneOp::DmaPlanes), event, start, cells);
            assert!(
                problems.iter().any(|p| p.contains("spends")),
                "${event:02X} at {at}: {problems:?}"
            );
            assert!(
                problems.iter().any(|p| p.contains("ticks")),
                "${event:02X} at {at}: {problems:?}"
            );
        }
    }
}
