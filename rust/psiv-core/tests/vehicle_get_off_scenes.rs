//! `Event_GettingOffVehicle` (`$10`), #81: open ground stands the party
//! down, anything else opens the refusal. Record:
//! `docs/scenes/207_GettingOffVehicle.md`.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, GameState, GetOffOp, SceneEffect, SceneInput, SceneOp,
    SceneRunner, ScriptedActor, StepFrames, SystemLine, scene_for,
};

/// Open ground on the left, water (`~`, raw 9) on the right.
const ROWS: [&str; 4] = ["....~~~~", "....~~~~", "....~~~~", "....~~~~"];

fn play(leader: Cell, state: &mut GameState) -> Vec<SceneEffect> {
    let map = map_with(&ROWS, vec![], vec![]);
    let scene = scene_for(EventIndex(0x10)).expect("registered");
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        leader,
        Direction::Right,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..100 {
        let effects = runner.tick(&map, state, input);
        input = if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::SystemLineOpen(_)))
        {
            SceneInput::DialogueEnded
        } else {
            SceneInput::None
        };
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "{log:?}");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    log
}

fn music_requested(log: &[SceneEffect]) -> bool {
    log.iter().any(|e| {
        matches!(
            e,
            SceneEffect::Presentation {
                op: SceneOp::GetOff(GetOffOp::PlayMapMusic)
            }
        )
    })
}

#[test]
fn open_ground_stands_the_party_down() {
    let mut state = GameState::new();
    state.set_vehicle_index(1);
    let log = play(Cell::new(1, 2), &mut state);
    assert!(
        log.iter()
            .any(|e| matches!(e, SceneEffect::VehicleChanged { index: 0 }))
    );
    assert_eq!(state.vehicle_index(), 0);
    assert!(music_requested(&log));
    assert!(
        !log.iter()
            .any(|e| matches!(e, SceneEffect::SystemLineOpen(_)))
    );
}

/// Negative control: the footprint's right column on water refuses, opens the
/// line, reloads the chunks and keeps the machine.
#[test]
fn water_under_the_footprint_refuses() {
    let mut state = GameState::new();
    state.set_vehicle_index(1);
    let log = play(Cell::new(3, 2), &mut state);
    assert!(
        log.iter()
            .any(|e| matches!(e, SceneEffect::SystemLineOpen(SystemLine::CannotGetOff)))
    );
    assert_eq!(state.vehicle_index(), 1, "still mounted");
    assert!(!music_requested(&log));
}
