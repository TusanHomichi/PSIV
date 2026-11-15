//! A scene's map load before `Pal_VariableFadeIn` on the pack (#612):
//! `RefreshMap` (`ps4.asm:121767`) fills `Palette_Table_Buffer` and returns
//! with no VBlank, and the fade-in copies and clears it before its own, so no
//! frame shows the new map's palette lit before the fade up. The scenes are
//! `Cutscene_LutzRevelation`'s shape (`$0778C6` fade-out, `RefreshMap` to
//! Inner Sanctuary B1, `$077902` fade-in), run on the runtime's scene seam.

use super::palette::{pack_present, runtime_at};
use crate::{Runtime, RuntimeEvent};
use psiv_core::{CharId, Direction, Input, PresentationOp, SceneOp, SceneRunner, StepFrames};

const FADE_OUT: SceneOp = SceneOp::Presentation {
    op: PresentationOp::VariablePaletteFade {
        fade_in: false,
        mode: 2,
    },
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

/// Runs `ops` on the Inner Sanctuary to their end: each tick's live words,
/// and the tick the map changed on.
fn run(ops: &'static [SceneOp]) -> (Vec<[u16; 64]>, usize) {
    let party = [Some(CharId(0)), Some(CharId(9)), None, None, None];
    let mut runtime: Runtime = runtime_at(0x16E, (0x1F0, 0x1E0), party, &[]);
    assert_ne!(runtime.cram().words, [0; 64], "the sanctuary is lit");
    runtime.scene = Some(SceneRunner::new(ops, Vec::new(), StepFrames::default()));
    let mut words = Vec::new();
    let mut load = None;
    for tick in 0..2_000 {
        let events = runtime.tick(Input::Neutral);
        for event in &events {
            assert!(
                !matches!(
                    event,
                    RuntimeEvent::SceneFaulted { .. } | RuntimeEvent::UnpackedTarget { .. }
                ),
                "{event:?}"
            );
            if matches!(event, RuntimeEvent::MapChanged { .. }) {
                load = Some(tick);
            }
        }
        words.push(runtime.cram().words);
        if !runtime.scene_active() {
            assert!(runtime.cram().missing.is_empty());
            return (words, load.expect("the scene loads B1"));
        }
    }
    panic!("the scene did not finish");
}

#[test]
fn a_load_before_a_variable_fade_in_shows_no_lit_tick() {
    if !pack_present() {
        return;
    }
    static OPS: &[SceneOp] = &[FADE_OUT, LOAD, FADE_IN];
    let (words, load) = run(OPS);
    let b1 = *words.last().unwrap();
    assert_ne!(b1, [0; 64], "the fade up ends on B1's palette");
    // From the fade-out's black to the fade-in's first pass, every tick is
    // black: the load's, and the fade-in's display-enable frame after it.
    let black = words.iter().position(|words| *words == [0; 64]).unwrap();
    assert!(black < load);
    for (tick, words) in words.iter().enumerate().take(load + 2).skip(black) {
        assert_eq!(*words, [0; 64], "tick {tick}: black until the fade up");
    }
    // The first pass lifts every word by one level at most, toward B1.
    let first = words[load + 2];
    assert_ne!(first, [0; 64]);
    assert_ne!(first, b1);
    // The fade-in is emitted the tick after the load, and its length is
    // worked from the load's words: the display frame, then a pass every
    // three frames (`$ED52` = 2), the last of which changes nothing. So B1
    // is reached one pass before the fade's last frame.
    let up = usize::from(psiv_core::palette::variable_fade_in_frames(&b1, 2));
    let every = usize::from(psiv_core::palette::dbf_frames(2));
    assert_eq!(words[load + up], b1);
    assert_eq!(words[load + up - every - 2], b1);
    assert_ne!(words[load + up - every - 3], b1);
}

/// Negative control: a wait between the load and the fade-in spends a VBlank,
/// so the load's tick uploads B1 lit, as retail's would.
#[test]
fn a_load_before_anything_else_shows_the_new_palette() {
    if !pack_present() {
        return;
    }
    static OPS: &[SceneOp] = &[FADE_OUT, LOAD, SceneOp::Wait { ticks: 1 }, FADE_IN];
    let (words, load) = run(OPS);
    let b1 = *words.last().unwrap();
    assert_eq!(words[load], b1);
    assert!(
        words[load + 1..].contains(&[0; 64]),
        "the fade-in clears it"
    );
}
