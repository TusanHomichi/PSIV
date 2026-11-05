//! Scene outcomes for the Pennant (`$97`) and the Wood Carving (`$98`), #81.
//! Record: `docs/scenes/203_HomeDecor.md`.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneOp,
    SceneRunner, ScriptedActor, StepFrames, scene_for,
};

fn play(event: u16, state: &mut GameState) -> Vec<SceneEffect> {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).expect("registered");
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(1, 1),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..200 {
        let effects = runner.tick(&map, state, input);
        input = if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            SceneInput::DialogueClosed
        } else if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. }))
        {
            SceneInput::MapLoaded
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

fn loaded(log: &[SceneEffect]) -> (u16, u16, u16, Direction) {
    log.iter()
        .find_map(|e| match e {
            SceneEffect::MapRequested {
                op:
                    SceneOp::LoadMap {
                        map,
                        start_x,
                        start_y,
                        facing,
                        ..
                    },
            } => Some((*map, *start_x, *start_y, *facing)),
            _ => None,
        })
        .expect("the house reloads")
}

#[test]
fn each_souvenir_goes_up_once_with_every_copy_taken() {
    for (event, item, line, flag, place) in [
        (0x97, 0x94, 0x64, 0x1C, (0x5E, 0x50, 0x46, Direction::Left)),
        (0x98, 0x95, 0x66, 0x1D, (0x5E, 0x38, 0x3E, Direction::Up)),
    ] {
        let mut state = GameState::new();
        for held in [item, 0x10, item] {
            state.inventory_mut().add(held).unwrap();
        }
        let log = play(event, &mut state);
        assert_eq!(
            log.iter()
                .filter_map(|e| match e {
                    SceneEffect::DialogueOpen(id) => Some(id.0),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [line]
        );
        assert!(
            !state.inventory().slots().contains(&item),
            "every copy goes"
        );
        assert_eq!(state.inventory().slots()[0], 0x10, "compacted");
        assert!(state.is_set(Flag::temp(flag)));
        assert!(
            !state.is_set(Flag::chest(flag)),
            "the temp bank, not chests"
        );
        assert_eq!(loaded(&log), place);
    }
}
