//! Scene outcomes for Uzo's mother (`$7F`), Sekreas in Rappy Cave (`$89`) and
//! the patients of Meese's clinic (`$93`), #81. Records:
//! `docs/scenes/200_LostGirlsMother.md` onward.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, PixelPos, SceneEffect, SceneInput,
    SceneRunner, ScriptedActor, StepFrames, scene_for,
};

/// Runs `event` to its end with the leader on `leader`, closing every window
/// at once, and returns its effects.
fn play(event: u16, leader: Cell, state: &mut GameState) -> Vec<SceneEffect> {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        leader,
        Direction::Right,
    )];
    for npc in 0..0x14 {
        cast.push(ScriptedActor::new(
            ActorRef::Npc(npc),
            Cell::new(1, 1),
            Direction::Down,
        ));
    }
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..4_000 {
        let effects = runner.tick(&map, state, input);
        input = if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume
            )
        }) {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish: {log:?}");
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

/// A cell whose retail pixel words are `(x, y)`.
fn at(x: i32, y: i32) -> Cell {
    Cell::new((x / 16) as u16, (y / 16 + 1) as u16)
}

fn turned(log: &[SceneEffect]) -> Option<ActorRef> {
    log.iter().find_map(|e| match e {
        SceneEffect::ActorFaced { actor, .. } => Some(*actor),
        _ => None,
    })
}

#[test]
fn the_mother_turns_speaks_and_sets_the_flag_the_prison_reads() {
    let mut state = GameState::new();
    let log = play(0x7F, Cell::new(2, 2), &mut state);
    assert_eq!(turned(&log), Some(ActorRef::Npc(1)));
    assert_eq!(lines(&log), [0x13]);
    assert!(state.is_set(Flag::event(0x79)));
}

#[test]
fn sekreas_reason_is_the_last_write() {
    let mut state = GameState::new();
    let log = play(0x89, Cell::new(2, 2), &mut state);
    assert_eq!(lines(&log), [0x37, 0x38]);
    let resumes = log
        .iter()
        .filter(|e| matches!(e, SceneEffect::DialogueResume))
        .count();
    assert_eq!(resumes, 2, "entry $37 stops twice at $F7");
    assert!(state.is_set(Flag::event(0xBA)));
    let despawned: Vec<usize> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::NpcDespawned { npc_index, .. } => Some(*npc_index),
            _ => None,
        })
        .collect();
    assert_eq!(despawned, [2, 3], "the invisible object, then Sekreas");
}

#[test]
fn each_clinic_row_turns_its_own_patient() {
    for (x, y, patient) in [
        (0x110, 0x260, 0x13),
        (0x110, 0x1A0, 0xF),
        (0x200, 0x140, 0xD),
        (0x230, 0x140, 0x11),
    ] {
        assert_eq!(PixelPos::from_cell(at(x, y)).y, y);
        let mut state = GameState::new();
        let log = play(0x93, at(x, y), &mut state);
        assert_eq!(turned(&log), Some(ActorRef::Npc(patient)));
        assert_eq!(lines(&log), [0x3C], "before the second Dark Force");

        state.set(Flag::event(0x9E)).unwrap();
        let log = play(0x93, at(x, y), &mut state);
        assert_eq!(lines(&log), [0x3D], "after it");
    }
}

/// The same bed's other side, one word off on each axis, and a row's X with
/// another row's Y: `rts` with nothing done.
#[test]
fn off_the_table_the_clinic_scene_does_nothing() {
    for (x, y) in [
        (0x120, 0x260),
        (0x110, 0x250),
        (0x200, 0x260),
        (0x230, 0x1A0),
    ] {
        let mut state = GameState::new();
        let log = play(0x93, at(x, y), &mut state);
        assert_eq!(turned(&log), None, "({x:#x},{y:#x})");
        assert!(lines(&log).is_empty(), "({x:#x},{y:#x})");
    }
}
