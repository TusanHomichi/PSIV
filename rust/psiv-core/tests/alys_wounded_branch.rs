//! The Saya flag branches to `loc_74ECA` ($074ECA), past all healing pages.
mod common;

use common::map_with;
use psiv_core::{
    DialogueId, DialogueWindow, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneOp,
    SceneRunner, StepFrames, scene_for,
};

fn healing_path(already_met_saya: bool) {
    let scene = scene_for(EventIndex(0x8009)).unwrap();
    // Run the real prefix through the shared aftermath's first operation.
    // Locate the boundary by meaning so insertions cannot silently move it.
    let aftermath = scene
        .ops
        .iter()
        .position(|op| matches!(op, SceneOp::PlaySound { id: 0xA5 }))
        .unwrap();
    let mut ops = scene.ops[..=aftermath].to_vec();
    ops.push(SceneOp::End);
    let map = map_with(&["....", "...."], vec![], vec![]);
    let mut state = GameState::new();
    state.write(Flag::event(0x12), already_met_saya).unwrap();
    let mut runner = SceneRunner::new(
        Box::leak(ops.into_boxed_slice()),
        vec![],
        StepFrames::new(8).unwrap(),
    );
    let mut input = SceneInput::None;
    let mut trace = Vec::new();
    for _ in 0..1000 {
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in effects {
            match effect {
                SceneEffect::DialogueOpen(DialogueId(id)) => {
                    trace.push(("open", id, runner.dialogue_window()));
                    input = SceneInput::DialogueClosed;
                }
                SceneEffect::DialogueResume => {
                    trace.push(("resume", 0, runner.dialogue_window()));
                    input = SceneInput::DialogueClosed;
                }
                SceneEffect::MapRequested { .. } => {
                    trace.push(("map", 0, DialogueWindow::Standard));
                    input = SceneInput::MapLoaded;
                }
                SceneEffect::Presentation {
                    op: SceneOp::PlaySound { id },
                } => {
                    trace.push(("sound", u16::from(id), DialogueWindow::Standard));
                }
                SceneEffect::Faulted(fault) => panic!("{fault:?}"),
                _ => {}
            }
        }
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished());
    use DialogueWindow::{Cutscene, Cutscene5, Standard};
    let mut expected = vec![
        ("open", 0x43, Cutscene),
        ("sound", 0xDE, Standard),
        ("resume", 0, Cutscene5),
        ("sound", 0xDE, Standard),
    ];
    if !already_met_saya {
        expected.extend([
            ("map", 0, Standard),
            ("sound", 0x83, Standard),
            ("open", 0x2C, Standard),
            ("resume", 0, Standard),
            ("sound", 0xBD, Standard),
            ("resume", 0, Standard),
            ("resume", 0, Standard),
        ]);
    }
    expected.push(("sound", 0xA5, Standard));
    assert_eq!(trace, expected);
    assert!(state.is_set(Flag::event(0x12)));
}

#[test]
fn already_met_saya_bypasses_the_entire_healing_block() {
    healing_path(true);
}

#[test]
fn first_visit_runs_three_standard_pages_and_healing_sound_before_aftermath() {
    healing_path(false);
}
