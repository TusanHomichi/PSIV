//! A scene's map load whose next op is `Pal_VariableFadeIn` (#612):
//! `RefreshMap` (`ps4.asm:121767`) returns with no VBlank, and the fade-in
//! clears the new palette from the buffer before its first one, so the
//! runtime holds the load's words unshown while the runner says so.

mod common;

use common::map_with;
use psiv_core::{
    Direction, GameState, PresentationOp, SceneEffect, SceneInput, SceneOp, SceneRunner, StepFrames,
};

const LOAD: SceneOp = SceneOp::LoadMap {
    map: 0x16F,
    prev_map: 0x16E,
    start_x: 0x3C,
    start_y: 0x4C,
    facing: Direction::Left,
    align: 0,
    clear_load_flags: 0x08,
};

const FADE_IN: SceneOp = SceneOp::Presentation {
    op: PresentationOp::VariablePaletteFade {
        fade_in: true,
        mode: 2,
    },
};

fn room() -> psiv_core::FieldMap {
    let row = ".".repeat(8);
    let rows: Vec<&str> = (0..8).map(|_| row.as_str()).collect();
    map_with(&rows, vec![], vec![])
}

/// Ticks `ops` to its map load and answers it: whether the runner reported a
/// fade-in next on the load's tick, and on the tick after.
fn around_the_load(ops: &'static [SceneOp]) -> (bool, bool) {
    let map = room();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(ops, Vec::new(), StepFrames::default());
    assert!(!runner.fades_in_after_map_load(), "nothing has run yet");
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, SceneEffect::MapRequested { .. })),
        "{effects:?}"
    );
    let on_load = runner.fades_in_after_map_load();
    let _ = runner.tick(&map, &mut state, SceneInput::MapLoaded);
    (on_load, runner.fades_in_after_map_load())
}

#[test]
fn a_load_before_a_variable_fade_in_is_reported_until_the_fade_runs() {
    static OPS: &[SceneOp] = &[LOAD, FADE_IN, SceneOp::Wait { ticks: 1 }];
    assert_eq!(around_the_load(OPS), (true, false));
}

#[test]
fn a_load_before_anything_else_is_not() {
    // A wait spends a VBlank first, so the new palette is shown lit.
    static WAIT: &[SceneOp] = &[LOAD, SceneOp::Wait { ticks: 2 }, FADE_IN];
    assert_eq!(around_the_load(WAIT), (false, false));
    // The shell's fade-in, and the variable fade-out.
    static SHELL: &[SceneOp] = &[LOAD, SceneOp::FadeIn];
    assert_eq!(around_the_load(SHELL), (false, false));
    static OUT: &[SceneOp] = &[
        LOAD,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 2,
            },
        },
    ];
    assert_eq!(around_the_load(OUT), (false, false));
}
