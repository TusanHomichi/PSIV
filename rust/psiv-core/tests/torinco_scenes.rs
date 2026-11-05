//! Scene outcomes for Torinco (#81): `$84`, `$85` and `$87`
//! (`docs/scenes/162_TorincoCulvers.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneRunner,
    ScriptedActor, StepFrames, scene_for,
};

/// Runs `event` with the leader below object 0, facing up, and returns the
/// lines it opened.
fn play(event: u16, state: &mut GameState) -> (Vec<u16>, Direction) {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X}"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(1, 2), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(1, 1), Direction::Left),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut lines = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..100 {
        input = match runner
            .tick(&map, state, input)
            .iter()
            .find_map(|e| match e {
                SceneEffect::DialogueOpen(id) => Some(id.0),
                _ => None,
            }) {
            Some(id) => {
                lines.push(id);
                SceneInput::DialogueClosed
            }
            None => SceneInput::None,
        };
        if runner.is_finished() {
            let facing = runner.actor(ActorRef::Npc(0)).unwrap().facing;
            return (lines, facing);
        }
    }
    panic!("${event:04X} did not finish");
}

/// `$072D44..$072D6B`, `$072D6C..$072D93`, `$072FB4..$072FDB`: object 0 turns
/// to face the leader, speaks one entry, and the event's flag is set.
#[test]
fn each_turns_speaks_and_sets_its_flag() {
    for (event, line, flag) in [(0x84, 0x39, 0xA9), (0x85, 0x3A, 0xAB), (0x87, 0x3C, 0xAE)] {
        let mut state = GameState::new();
        let (lines, facing) = play(event, &mut state);
        assert_eq!(lines, [line], "${event:02X}");
        assert_eq!(
            facing,
            Direction::Down,
            "${event:02X} faces the leader below"
        );
        assert!(state.is_set(Flag::event(flag)), "${event:02X}");
    }
}

/// The recorded facing-frame deviation (`docs/source-notes/README.md`, #164):
/// retail's `Event_UpdateObjFacing` ends in a `VInt_Prepare`, so the cartridge
/// opens the text one frame after the turn. The port's `FaceOppositeOf` takes
/// no frame, so the text opens on the scene's first tick. When #164 charges the
/// frame this fails, and the note goes with it.
#[test]
fn the_facing_frame_is_the_recorded_deviation() {
    for event in [0x84u16, 0x85, 0x87] {
        let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
        let scene = scene_for(EventIndex(event)).unwrap();
        let cast = vec![
            ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(1, 2), Direction::Up),
            ScriptedActor::new(ActorRef::Npc(0), Cell::new(1, 1), Direction::Left),
        ];
        let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
        let mut state = GameState::new();
        let opened_on = (1..=10).find(|_| {
            runner
                .tick(&map, &mut state, SceneInput::None)
                .iter()
                .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        });
        assert_eq!(opened_on, Some(1), "${event:02X}: retail opens it on tick 2");
    }
}
