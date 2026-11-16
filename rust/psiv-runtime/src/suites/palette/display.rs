//! The display-enable bit on the pack (#653).
//!
//! `Cutscene_LashiecDefeated` (`$8016`) fades out (`PalFadeOut_ClrSpriteTbl`,
//! `ps4.asm:156906`), runs `InitVRAMAndCRAM`, which puts `Pal_Init` in the
//! buffer, and waits forty frames for its explosion before panel `$10A` and
//! `Pal_FadeIn` (`:156907-156918`). Through those frames the buffer holds
//! `Pal_Init`, and the cartridge shows the backdrop alone. `Cutscene_Ending`
//! (`$8021`) fades out with `$FFFFED52` = `$0F` before its second tone loop,
//! and its own `VDP_EnableDisplay` (`$079C0A`) relights the screen after it.

use super::{pack_present, runtime_on};
use crate::RuntimeEvent;
use psiv_core::{CharId, Input, PresentationOp, SceneOp};

/// One traced tick: the records emitted, the display bit and the words.
struct Tick {
    ops: Vec<SceneOp>,
    display: bool,
    backdrop: Option<u16>,
    words: [u16; 64],
}

impl Tick {
    fn has(&self, pick: impl Fn(&SceneOp) -> bool) -> bool {
        self.ops.iter().any(pick)
    }
}

fn presentation(op: &SceneOp, pick: impl Fn(&PresentationOp) -> bool) -> bool {
    matches!(op, SceneOp::Presentation { op } if pick(op))
}

/// Runs `event` on map `$13`, closing every window at once, until `done`
/// holds for the ticks so far. `None` without the pack.
fn run(event: u16, done: impl Fn(&[Tick]) -> bool) -> Option<Vec<Tick>> {
    if !pack_present() {
        return None;
    }
    let mut runtime = runtime_on(0x013, [Some(CharId(0)), None, None, None, None], &[]);
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut ticks: Vec<Tick> = Vec::new();
    for _ in 0..200_000 {
        let events = runtime.tick(Input::Neutral);
        let mut ops = Vec::new();
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "{item:?}"
            );
            if matches!(item, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
            if let RuntimeEvent::ScenePresentation { op, .. } = item {
                ops.push(*op);
            }
        }
        let view = runtime.cram();
        ticks.push(Tick {
            ops,
            display: view.display,
            backdrop: view.shown_backdrop(),
            words: view.words,
        });
        if done(&ticks) {
            assert!(
                runtime.cram().missing.is_empty(),
                "{:x?}",
                runtime.cram().missing
            );
            return Some(ticks);
        }
        assert!(runtime.scene_active(), "event {event:#x} ended early");
    }
    panic!("event {event:#x} never got there");
}

fn enable_display(op: &SceneOp) -> bool {
    presentation(op, |op| matches!(op, PresentationOp::EnableDisplay))
}

/// The ending until the tick after its `VDP_EnableDisplay`.
fn ending() -> Option<Vec<Tick>> {
    run(0x8021, |ticks| {
        ticks
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|tick| tick.has(enable_display))
    })
}

fn first(ticks: &[Tick], from: usize, pick: impl Fn(&SceneOp) -> bool) -> usize {
    from + ticks[from..]
        .iter()
        .position(|tick| tick.has(&pick))
        .expect("the record is emitted")
}

fn variable_fade(op: &SceneOp, fade_in: bool, mode: Option<u8>) -> bool {
    presentation(op, |op| {
        matches!(*op, PresentationOp::VariablePaletteFade { fade_in: f, mode: m }
            if f == fade_in && mode.is_none_or(|mode| mode == m))
    })
}

/// Lashiec's forty dark frames show the backdrop alone, black, while the
/// buffer holds `Pal_Init`; the fade-in lights the frame after its wait.
#[test]
fn lashiec_draws_only_the_backdrop_between_its_fade_out_and_fade_in() {
    let lit = |ticks: &[Tick]| {
        let fade_ins = ticks
            .iter()
            .filter(|tick| tick.has(|op| *op == SceneOp::FadeIn))
            .count();
        fade_ins == 2 && ticks.last().is_some_and(|tick| tick.display)
    };
    let Some(ticks) = run(0x8016, lit) else {
        return;
    };
    // The scene's first `InitVRAMAndCRAM` and `Pal_FadeIn`, then the second
    // `InitVRAMAndCRAM`, after the explicit fade-out, and its `Pal_FadeIn`.
    let lit_once = first(&ticks, 0, |op| *op == SceneOp::FadeIn) + 1;
    let init = first(&ticks, lit_once, |op| *op == SceneOp::InitVramAndCram);
    let fade_in = first(&ticks, init, |op| *op == SceneOp::FadeIn);
    let dark = lit_once
        + ticks[lit_once..]
            .iter()
            .position(|tick| !tick.display)
            .unwrap();
    assert!(ticks[lit_once].display);
    assert_eq!(
        ticks[dark - 1].words,
        [0; 64],
        "the fade reached black first"
    );
    assert!(dark <= init);
    assert!(ticks[dark..=fade_in].iter().all(|tick| !tick.display));
    assert!(
        ticks[fade_in + 1].display,
        "lit the frame after the fade-in's wait"
    );
    // `Pal_Init` is in the buffer from the frame after the op until the
    // fade-in clears it: forty frames and more, each showing only the
    // backdrop, word 0, black.
    let written: Vec<&Tick> = ticks[init + 1..fade_in]
        .iter()
        .filter(|tick| tick.words != [0; 64])
        .collect();
    assert!(
        written.len() >= 40,
        "{} frames show a written palette",
        written.len()
    );
    for tick in written {
        assert_eq!(tick.backdrop, Some(0), "only the black backdrop shows");
    }
}

/// The ending's own `VDP_EnableDisplay` relights the screen the frame after
/// its wait; the `$0F` fade-out before the second tone loop had darkened it,
/// so the loop's whitening shows only on the backdrop.
#[test]
fn the_ending_s_bare_display_enable_relights_the_screen() {
    let Some(ticks) = ending() else {
        return;
    };
    let enable = ticks.len() - 2;
    let out = (0..enable)
        .rev()
        .find(|&at| ticks[at].has(|op| variable_fade(op, false, Some(0x0F))))
        .expect("the $0F fade-out precedes the enable");
    let tone = first(&ticks, out, |op| {
        presentation(op, |op| {
            matches!(op, PresentationOp::PaletteIncreaseTone { .. })
        })
    });
    assert!(!ticks[tone].display, "the tone loop runs dark");
    assert!(ticks[tone..=enable].iter().all(|tick| !tick.display));
    assert!(ticks[enable + 1].display);
    // Sixty frames of `Pal_IncreaseTone` from black step every component
    // fifteen times, capped at 7: the backdrop, all that shows, is white.
    assert_eq!(ticks[enable].backdrop, Some(0x0EEE));
}
