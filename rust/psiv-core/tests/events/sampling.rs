//! What the oracle lane samples from this target, and the multi-map recast.
//!
//! Two cases that are about the boundary rather than about one scene: the
//! step-duration ladder the cartridge counts down, which the oracle's frame
//! samples are compared against, and a scene that survives a map change. The
//! recast case is the one the runtime's flow depends on: NPC indices mean
//! something only relative to one map's object list, so a scene that tours
//! maps must be re-seated, and a stale cast must fault rather than drive
//! ghosts.

use crate::common::map_with;
use crate::fixtures::frames;
use psiv_core::{
    ActorRef, Cell, Direction, GameState, SceneEffect, SceneFault, SceneInput, SceneOp,
    SceneRunner, ScriptedActor, StepFrames,
};

#[test]
fn step_durations_match_the_cartridges_countdown_ladder() {
    // Hardware: x_step_duration counts $1000 -> 0 in $200 steps, eight states,
    // and only the walked axis is non-zero.
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = psiv_core::FieldState::new(
        &map,
        Cell::new(0, 1),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();

    assert_eq!(state.step_durations_8_8(), (0, 0), "at rest");
    assert_eq!(state.step_remaining_frames(), 0);

    let mut ladder = Vec::new();
    for _ in 0..8 {
        state.tick(&map, psiv_core::Input::Direction(Direction::Right));
        ladder.push(state.step_durations_8_8());
    }
    assert_eq!(
        ladder,
        vec![
            (0x0E00, 0),
            (0x0C00, 0),
            (0x0A00, 0),
            (0x0800, 0),
            (0x0600, 0),
            (0x0400, 0),
            (0x0200, 0),
            (0, 0),
        ],
        "the $200-per-frame ladder, landing at zero"
    );

    // Walking vertically moves the other column.
    state.tick(&map, psiv_core::Input::Direction(Direction::Down));
    assert_eq!(state.step_durations_8_8(), (0, 0x0E00));
}

#[test]
fn a_scene_can_be_recast_across_a_map_change() {
    // The opening event tours five maps; NPC indices only mean anything
    // relative to one map's object list, so the cast is re-seated on the way.
    let first = map_with(&["....", "....", "...."], vec![], vec![]);
    let second = map_with(&["......", "......", "......"], vec![], vec![]);
    let mut state = GameState::new();

    static TOUR: &[SceneOp] = &[
        SceneOp::LoadMap {
            map: 0x13,
            prev_map: 0x11,
            start_x: 4,
            start_y: 4,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0,
        },
        SceneOp::Face {
            actor: ActorRef::Npc(0),
            facing: Direction::Left,
        },
        SceneOp::End,
    ];

    let runner = SceneRunner::new(
        TOUR,
        vec![ScriptedActor::new(
            ActorRef::Npc(3),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    );

    // Without a recast the scene faults on the op naming an actor the new map
    // does not have — loudly, which is the point. The map-load acknowledgement
    // is explicit now, so the fault is observed on the following tick rather
    // than while the runner is still waiting for the new map.
    let mut doomed = runner.clone();
    let effects = doomed.tick(&first, &mut state, SceneInput::None);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. }))
    );
    let effects = doomed.tick(&first, &mut state, SceneInput::MapLoaded);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Faulted(SceneFault::UnknownActor { .. }))),
        "a stale cast must fault rather than drive ghosts"
    );

    // The runtime's actual flow: load the map on MapRequested, recast, carry on.
    let mut runner2 = SceneRunner::new(
        TOUR,
        vec![ScriptedActor::new(
            ActorRef::Npc(3),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    );
    let effects = runner2.tick(&first, &mut state, SceneInput::None);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. })),
        "the map change is reported so the runtime can act"
    );

    runner2.recast(vec![ScriptedActor::new(
        ActorRef::Npc(0),
        Cell::new(2, 2),
        Direction::Up,
    )]);
    let effects = runner2.tick(&second, &mut state, SceneInput::MapLoaded);
    assert!(effects.iter().any(|e| matches!(
        e,
        SceneEffect::ActorFaced {
            actor: ActorRef::Npc(0),
            ..
        }
    )));
    assert_eq!(
        runner2.actor(ActorRef::Npc(0)).unwrap().facing,
        Direction::Left
    );
}
