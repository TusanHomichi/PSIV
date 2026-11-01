//! Scene outcomes for the strip club (#81): the dancers' entrance (`$94`) and
//! the flag the guild's doorway clears (`$95`). Every case cites the retail
//! bytes (`docs/scenes/123_StripClub.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, PresentationOp, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const STRIPPERS_DANCE: Flag = Flag::temp(0x1B);

/// Runs `event` to its end, logging each tick's effects with its tick number.
fn play(event: u16, state: &mut GameState) -> Vec<(usize, SceneEffect)> {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(2, 3), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(1, 1), Direction::Down),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    for tick in 0..1_000 {
        // The runtime's glide takes frames; here each pan arrives at once.
        let input = if runner.is_waiting_for_camera() {
            SceneInput::CameraArrived
        } else {
            SceneInput::None
        };
        for effect in runner.tick(&map, state, input) {
            log.push((tick, effect));
        }
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish");
    assert!(
        !log.iter()
            .any(|(_, e)| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    log
}

/// The tick a presentation record was emitted on.
fn tick_of(log: &[(usize, SceneEffect)], wanted: impl Fn(&SceneOp) -> bool) -> usize {
    log.iter()
        .find_map(|(tick, e)| match e {
            SceneEffect::Presentation { op } if wanted(op) => Some(*tick),
            _ => None,
        })
        .expect("record present")
}

#[test]
fn the_dancers_come_out_and_the_flag_is_the_last_write() {
    let mut state = GameState::new();
    let log = play(0x94, &mut state);
    assert!(state.is_set(STRIPPERS_DANCE));
    let routines: Vec<_> = log
        .iter()
        .filter_map(|(_, e)| match e {
            SceneEffect::ObjectRoutineSet { npc_index, routine } => Some((*npc_index, *routine)),
            _ => None,
        })
        .collect();
    assert_eq!(routines, [(0, 0x34C)], "the customer, `$0735AE`");
    let created: Vec<_> = log
        .iter()
        .filter_map(|(_, e)| match e {
            SceneEffect::Presentation {
                op:
                    SceneOp::CreateFieldObject {
                        slot, object_id, ..
                    },
            } => Some((*slot, *object_id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        created,
        [
            (1, 0x344),
            (2, 0x344),
            (3, 0x344),
            (5, 0x348),
            (6, 0x348),
            (7, 0x348),
            (1, 0x35C),
            (2, 0x360),
            (3, 0x364),
        ]
    );
    let cleared: Vec<_> = log
        .iter()
        .filter_map(|(_, e)| match e {
            SceneEffect::NpcDespawned { npc_index, count } => Some((*npc_index, *count)),
            _ => None,
        })
        .collect();
    assert_eq!(cleared, [(4, 3)], "`$C400..$C4BF`: the third coat flies on");
    let last_flag = log
        .iter()
        .rev()
        .find_map(|(_, e)| match e {
            SceneEffect::FlagChanged { flag, value } => Some((*flag, *value)),
            _ => None,
        })
        .expect("a flag write");
    assert_eq!(last_flag, (STRIPPERS_DANCE, true));
}

/// `Event_MoveObjectStatic` runs every object until stripper 1 stands on its
/// destination: from `($160,$190)` to `($220,$210)`, twelve cells right then
/// eight down at eight frames a cell. `DoMapUpdateLoop($3B)` follows, and the
/// coats are written on the next frame.
#[test]
fn the_walk_and_the_pause_before_the_coats_take_220_frames() {
    let mut state = GameState::new();
    let log = play(0x94, &mut state);
    let walk = tick_of(&log, |op| {
        matches!(
            op,
            SceneOp::Presentation {
                op: PresentationOp::WalkObject { slot: 1, .. }
            }
        )
    });
    let coat = tick_of(&log, |op| {
        matches!(op, SceneOp::CreateFieldObject { slot: 5, .. })
    });
    assert_eq!(coat - walk, 160 + 60);
    // The wait after the walk is the walk's own length, read from the ops'
    // coordinates: where object 1 was created and where it is sent.
    let ops = scene_for(EventIndex(0x94)).unwrap().ops;
    let at = |wanted: &dyn Fn(&SceneOp) -> bool| ops.iter().position(wanted).unwrap();
    let from = match ops[at(&|op| matches!(op, SceneOp::CreateFieldObject { slot: 1, .. }))] {
        SceneOp::CreateFieldObject { x, y, .. } => (x, y),
        _ => unreachable!(),
    };
    let walk_at = at(&|op| {
        matches!(
            op,
            SceneOp::Presentation {
                op: PresentationOp::WalkObject { slot: 1, .. }
            }
        )
    });
    let SceneOp::Presentation {
        op: PresentationOp::WalkObject { x, y, .. },
    } = ops[walk_at]
    else {
        unreachable!()
    };
    let cells = ((x - from.0).abs() + (y - from.1).abs()) / 16;
    assert_eq!(
        ops[walk_at + 1],
        SceneOp::Wait {
            ticks: u16::try_from(cells * 8).unwrap()
        }
    );
}

#[test]
fn leaving_clears_the_flag_so_the_dance_repeats() {
    let mut state = GameState::new();
    state.set(STRIPPERS_DANCE).unwrap();
    play(0x95, &mut state);
    assert!(!state.is_set(STRIPPERS_DANCE));
}

/// `Event_MoveCamera` returns on arrival, so neither pan lets the scene on:
/// no number of frames releases it, only the arrival does.
#[test]
fn both_pans_hold_the_scene_until_the_camera_arrives() {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(0x94)).unwrap();
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(2, 3),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut state = GameState::new();
    let mut seen = Vec::new();
    let mut held = 0;
    for _ in 0..2_000 {
        if runner.is_finished() {
            break;
        }
        let mut effects = runner.tick(&map, &mut state, SceneInput::None);
        if runner.is_waiting_for_camera() {
            held += 1;
            seen.append(&mut effects);
            for _ in 0..600 {
                assert!(runner.tick(&map, &mut state, SceneInput::None).is_empty());
                assert!(runner.is_waiting_for_camera());
            }
            let pans = seen
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        SceneEffect::Presentation {
                            op: SceneOp::PanCamera { .. }
                        }
                    )
                })
                .count();
            assert_eq!(pans, held, "the op that blocks is the pan");
            assert!(!state.is_set(STRIPPERS_DANCE));
            assert!(
                !matches!(
                    seen.last(),
                    Some(SceneEffect::Presentation {
                        op: SceneOp::CreateFieldObject { .. }
                    })
                ),
                "the pan is the last thing written before the wait"
            );
            effects = runner.tick(&map, &mut state, SceneInput::CameraArrived);
        }
        seen.append(&mut effects);
    }
    assert!(runner.is_finished());
    assert_eq!(held, 2, "the outbound pan and the return");
    assert!(state.is_set(STRIPPERS_DANCE));
}
