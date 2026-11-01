//! Scene outcomes for Tallas's rescue (#81): `$7A`, `$7B`, `$7C` and `$7E`.
//! Every case cites the retail bytes (`docs/scenes/140_TallasRescue.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneOp,
    SceneRunner, ScriptedActor, StepFrames, scene_for,
};

/// Runs `event` to its end, closing each window, and returns its effects.
fn play(event: u16, state: &mut GameState) -> Vec<SceneEffect> {
    // `MonsenCave`'s size, so Tallas's course stays on the map.
    let row = ".".repeat(128);
    let rows = vec![row.as_str(); 128];
    let map = map_with(&rows, vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(0, 3), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(0, 1), Direction::Down),
        ScriptedActor::new(ActorRef::Npc(2), Cell::new(0x650 / 16, 46), Direction::Down),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..4_000 {
        let effects = runner.tick(&map, state, input);
        input = if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    log
}

fn lines(log: &[SceneEffect]) -> Vec<u16> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

/// `$072AD0` and `$072AF6`: the mother turns to the leader (who faces up, so
/// she faces down), speaks, and the scene's one flag is the last write.
#[test]
fn the_mother_turns_speaks_and_sets_her_flag() {
    for (event, line, flag) in [(0x7A, 0x2D, 0x2D), (0x7B, 0x2E, 0x3C)] {
        let mut state = GameState::new();
        let log = play(event, &mut state);
        assert_eq!(lines(&log), [line]);
        assert!(state.is_set(Flag::event(flag)));
        assert!(log.iter().any(|e| matches!(
            e,
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(0),
                facing: Direction::Down,
            }
        )));
    }
}

/// `$072B1C`: the line, then `$2E`, nothing else.
#[test]
fn the_fall_speaks_once_and_ends_its_trigger() {
    let mut state = GameState::new();
    let log = play(0x7C, &mut state);
    assert_eq!(lines(&log), [0x24]);
    assert!(state.is_set(Flag::event(0x2E)));
}

/// `$072B52..$072C07`: the routine, each corner in order, the slot cleared and
/// the music saved, with `$3E` last.
#[test]
fn tallas_walks_the_records_course_and_leaves() {
    let mut state = GameState::new();
    let log = play(0x7E, &mut state);
    assert_eq!(lines(&log), [0x27]);
    assert!(log.iter().any(|e| matches!(
        e,
        SceneEffect::ObjectRoutineSet {
            npc_index: 2,
            routine: 0x2FC,
        }
    )));
    let corners: Vec<(u16, u16)> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::ActorMoveStarted {
                actor: ActorRef::Npc(2),
                to,
            } => Some((to.x, to.y)),
            _ => None,
        })
        .collect();
    assert_eq!(
        corners,
        [
            (0x61, 0x2E),
            (0x61, 0x2E),
            (0x61, 0x33),
            (0x67, 0x33),
            (0x67, 0x31),
            (0x5F, 0x31),
            (0x5F, 0x3A),
        ]
    );
    let order: Vec<usize> = [
        log.iter().position(|e| {
            matches!(
                e,
                SceneEffect::ActorArrived {
                    actor: ActorRef::Npc(2),
                    at
                } if (at.x, at.y) == (0x5F, 0x3A)
            )
        }),
        log.iter().position(|e| {
            matches!(
                e,
                SceneEffect::NpcDespawned {
                    npc_index: 2,
                    count: 1
                }
            )
        }),
        log.iter().position(|e| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::SetSavedMusic { id: 0x8A }
                }
            )
        }),
        log.iter().position(|e| {
            matches!(
                e,
                SceneEffect::FlagChanged { flag, value: true } if *flag == Flag::event(0x3E)
            )
        }),
    ]
    .into_iter()
    .map(|at| at.expect("the record's write"))
    .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
}
