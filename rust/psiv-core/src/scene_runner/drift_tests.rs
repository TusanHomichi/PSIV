//! `BranchIfActorCoord` and `DriftNpcs`.

use super::drift::{CoordCmp, NpcDrift};
use super::*;
use crate::map::{Npc, NpcId};
use crate::{Axis, CollisionGrid, Direction, Flag, MapId, SceneEffect};

/// A 64 x 64 map with objects standing at the given cells.
fn map_of(cells: &[(u16, u16)]) -> FieldMap {
    let npcs = cells
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Npc::new(NpcId(i as u16), Cell::new(x, y), Direction::Down))
        .collect();
    FieldMap::new(
        MapId(3),
        CollisionGrid::filled(64, 64, 0).unwrap(),
        vec![],
        npcs,
    )
    .unwrap()
}

/// Applies the position reports the way `Runtime::translate_scene_effect` does.
fn apply(map: &mut FieldMap, effects: &[SceneEffect]) {
    for effect in effects {
        if let SceneEffect::Presentation {
            op:
                SceneOp::Presentation {
                    op: crate::PresentationOp::NpcPixelPosition { npc, x, y },
                },
        } = effect
        {
            map.set_npc_pixel_position(*npc, *x, *y).unwrap();
        }
    }
}

fn cast(count: usize) -> Vec<ScriptedActor> {
    (0..count)
        .map(|i| ScriptedActor::new(ActorRef::Npc(i), Cell::new(0, 0), crate::Direction::Down))
        .collect()
}

#[test]
fn the_branch_conditions_are_the_unsigned_word_bcc_family() {
    use CoordCmp::{Above, AtLeast, AtMost, Below, Equal, NotEqual};
    // (cmp, coordinate, literal, expected): `cmpi.w #literal, coordinate`.
    let cases = [
        (Equal, 0x120, 0x120, true),
        (Equal, 0x11F, 0x120, false),
        (NotEqual, 0x120, 0x120, false),
        (NotEqual, 0x110, 0x120, true),
        (Below, 0x11F, 0x120, true),
        (Below, 0x120, 0x120, false),
        (AtMost, 0x120, 0x120, true),
        (AtMost, 0x121, 0x120, false),
        (Above, 0x121, 0x120, true),
        (Above, 0x120, 0x120, false),
        (AtLeast, 0x120, 0x120, true),
        (AtLeast, 0x11F, 0x120, false),
        // Unsigned: a word with the high bit set is above any small literal.
        (Above, 0xFFF0, 0x120, true),
        (Below, 0xFFF0, 0x120, false),
    ];
    for (cmp, coordinate, value, expected) in cases {
        assert_eq!(
            cmp.holds(coordinate, value),
            expected,
            "{cmp:?} {coordinate:#X} against {value:#X}"
        );
    }
}

static BRANCH: &[SceneOp] = &[
    SceneOp::BranchIfActorCoord {
        actor: ActorRef::Npc(0),
        axis: Axis::X,
        cmp: CoordCmp::Equal,
        value: 0x120,
        if_true: 1,
        if_false: 3,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x01),
        value: true,
    },
    SceneOp::End,
    SceneOp::SetFlag {
        flag: Flag::event(0x02),
        value: true,
    },
    SceneOp::End,
];

#[test]
fn a_coordinate_branch_takes_each_arm() {
    for (cell, flag) in [((18, 25), 0x01), ((19, 25), 0x02)] {
        let map = map_of(&[cell]);
        let mut state = GameState::new();
        let mut runner = SceneRunner::new(BRANCH, cast(1), StepFrames::default());
        runner.tick(&map, &mut state, SceneInput::None);
        assert!(runner.is_finished());
        assert!(state.is_set(Flag::event(flag)), "cell {cell:?}");
        assert!(!state.is_set(Flag::event(0x03 - flag)), "cell {cell:?}");
    }
}

#[test]
fn a_coordinate_branch_reads_the_live_sub_cell_pixel() {
    // Cell 18 is pixel $120; standing 8 px into the cell is $128.
    let mut map = map_of(&[(18, 25)]);
    map.set_npc_pixel_position(0, 0x128, 384).unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(BRANCH, cast(1), StepFrames::default());
    runner.tick(&map, &mut state, SceneInput::None);
    assert!(state.is_set(Flag::event(0x02)), "$128 is not $120");
}

#[test]
fn a_coordinate_branch_on_an_unknown_actor_faults() {
    let map = map_of(&[]);
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(BRANCH, vec![], StepFrames::default());
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert!(matches!(
        effects.last(),
        Some(SceneEffect::Faulted(SceneFault::UnknownActor {
            actor: ActorRef::Npc(0)
        }))
    ));
}

const LEFT: i32 = -0x4000;
const RIGHT: i32 = 0x8000;

static SLIDE: &[SceneOp] = &[
    SceneOp::DriftNpcs {
        drifts: &[
            NpcDrift {
                npc: 0,
                step_x: LEFT,
                step_y: 0,
            },
            NpcDrift {
                npc: 1,
                step_x: RIGHT,
                step_y: 0,
            },
        ],
        frames: 64,
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x01),
        value: true,
    },
];

static WRONG_SLIDE: &[SceneOp] = &[SceneOp::DriftNpcs {
    drifts: &[NpcDrift {
        npc: 0,
        // 17 px over 64 frames instead of 16.
        step_x: -0x4400,
        step_y: 0,
    }],
    frames: 64,
}];

/// Runs `ops` to its end, applying position reports to the map as they come,
/// and returns the x pixel of each object after every tick.
fn slide(ops: &'static [SceneOp], objects: usize) -> (Vec<Vec<i32>>, GameState, FieldMap) {
    let mut map = map_of(&[(18, 25), (22, 25)][..objects]);
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(ops, cast(objects), StepFrames::default());
    let mut frames = Vec::new();
    for _ in 0..200 {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        apply(&mut map, &effects);
        frames.push(
            map.npcs()
                .iter()
                .map(|npc| i32::from(npc.cell.x) * 16 + i32::from(npc.offset.x))
                .collect(),
        );
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished());
    (frames, state, map)
}

#[test]
fn objects_drift_at_their_step_constants_and_the_scene_waits_for_the_loop() {
    let (frames, state, _) = slide(SLIDE, 2);
    // Tick 1 runs the op; ticks 2..=65 are the 64 loop iterations, and the
    // scene's next op runs on the last of them.
    assert_eq!(frames.len(), 65);
    assert_eq!(frames[0], vec![0x120, 0x160], "the op itself moves nothing");
    // -0.25 px a frame: $120.0000 - $4000 is $11F.C000, so the first frame
    // already shows $11F; four frames make a whole pixel.
    assert_eq!(frames[1][0], 0x11F);
    assert_eq!(frames[4][0], 0x11F);
    assert_eq!(frames[5][0], 0x11E);
    // +0.5 px a frame: the first frame is still $160, the second $161.
    assert_eq!(frames[1][1], 0x160);
    assert_eq!(frames[2][1], 0x161);
    // Intermediate: after 32 frames, 8 px left and 16 px right.
    assert_eq!(frames[32], vec![0x118, 0x170]);
    // End: 16 px left, 32 px right.
    assert_eq!(frames[64], vec![0x110, 0x180]);
    assert!(state.is_set(Flag::event(0x01)), "the next op ran");
}

#[test]
fn a_wrong_step_constant_misses_the_cartridges_end_position() {
    let (frames, _, _) = slide(WRONG_SLIDE, 1);
    let end = *frames.last().unwrap().first().unwrap();
    assert_ne!(end, 0x110, "-$4400 over 64 frames is 17 px, not 16");
    assert_eq!(end, 0x10F);
    let (frames, _, _) = slide(SLIDE, 2);
    assert_eq!(*frames.last().unwrap().first().unwrap(), 0x110);
}

#[test]
fn a_drift_reports_positions_to_the_runtime_every_frame() {
    let map = map_of(&[(18, 25), (22, 25)]);
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(SLIDE, cast(2), StepFrames::default());
    let mut reports = 0;
    for _ in 0..65 {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        reports += effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    SceneEffect::Presentation {
                        op: SceneOp::Presentation {
                            op: crate::PresentationOp::NpcPixelPosition { .. }
                        }
                    }
                )
            })
            .count();
    }
    assert_eq!(reports, 64 * 2, "one report per object per frame");
}

#[test]
fn a_drift_updates_the_branch_coordinate_even_without_a_runtime() {
    // The map here is never written: the overlay keeps the scene's own view.
    static THEN_BRANCH: &[SceneOp] = &[
        SceneOp::DriftNpcs {
            drifts: &[NpcDrift {
                npc: 0,
                step_x: LEFT,
                step_y: 0,
            }],
            frames: 64,
        },
        SceneOp::BranchIfActorCoord {
            actor: ActorRef::Npc(0),
            axis: Axis::X,
            cmp: CoordCmp::Equal,
            value: 0x110,
            if_true: 2,
            if_false: 4,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x01),
            value: true,
        },
        SceneOp::End,
        SceneOp::SetFlag {
            flag: Flag::event(0x02),
            value: true,
        },
    ];
    let map = map_of(&[(18, 25)]);
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(THEN_BRANCH, cast(1), StepFrames::default());
    for _ in 0..70 {
        runner.tick(&map, &mut state, SceneInput::None);
    }
    assert!(state.is_set(Flag::event(0x01)));
}

#[test]
fn a_drift_of_an_unknown_object_faults() {
    let map = map_of(&[(18, 25)]);
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(SLIDE, cast(1), StepFrames::default());
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert!(matches!(
        effects.last(),
        Some(SceneEffect::Faulted(SceneFault::UnknownActor {
            actor: ActorRef::Npc(1)
        }))
    ));
}

#[test]
fn the_drift_loop_runs_map_updates_like_do_main_updates_loop() {
    let map = map_of(&[(18, 25), (22, 25)]);
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(SLIDE, cast(2), StepFrames::default());
    runner.tick(&map, &mut state, SceneInput::None);
    assert!(runner.runs_map_updates());
}
