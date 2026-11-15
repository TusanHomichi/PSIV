//! `Cutscene_Ending`'s three sixty-frame `Pal_IncreaseTone` loops
//! (`$078F9C`, `$079BD2`, `$07A0FA`) on the pack: what `Runtime::cram`
//! holds on every frame of each, checked by hand with [`check_tone`].

use super::{Frame, check_tone, pack_present, runtime_on};
use crate::RuntimeEvent;
use psiv_core::{CharId, Input, PresentationOp, SceneOp, palette};

/// Frames each loop runs: `moveq #$3B, d7` / `dbra`.
const LOOP: usize = 60;

/// One traced loop: the palette the tick before its record, then the
/// sixty ticks from the record on.
struct ToneLoop {
    start: [u16; 64],
    frames: Vec<Frame>,
}

/// Runs `$8021` on map `$13` (where the certify fixture leaves Chaz),
/// closing every window at once, until the third loop's sixty frames are
/// traced. `None` without the pack.
fn ending_loops() -> Option<Vec<ToneLoop>> {
    if !pack_present() {
        return None;
    }
    let mut runtime = runtime_on(0x013, [Some(CharId(0)), None, None, None, None], &[]);
    assert!(runtime.start_event(0x8021), "the ending starts");
    let mut loops: Vec<ToneLoop> = Vec::new();
    let mut before = runtime.cram().words;
    for _ in 0..200_000 {
        let events = runtime.tick(Input::Neutral);
        let mut record = false;
        for event in &events {
            assert!(
                !matches!(event, RuntimeEvent::SceneFaulted { .. }),
                "{event:?}"
            );
            if matches!(event, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
            record |= matches!(
                event,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::Presentation {
                        op: PresentationOp::PaletteIncreaseTone { .. }
                    },
                    ..
                }
            );
        }
        if record {
            loops.push(ToneLoop {
                start: before,
                frames: Vec::new(),
            });
        }
        if let Some(open) = loops.last_mut().filter(|open| open.frames.len() < LOOP) {
            open.frames.push(Frame {
                count: runtime.frames,
                events,
                words: runtime.cram().words,
                beams: Vec::new(),
            });
            if loops.len() == 3 && loops[2].frames.len() == LOOP {
                return Some(loops);
            }
        }
        before = runtime.cram().words;
        assert!(runtime.scene_active(), "the ending ended before its loops");
    }
    panic!("the ending's third loop never ran");
}

/// Acceptance for the ending's loops: sixty frames of `Pal_IncreaseTone`
/// over all four lines, none held, each component `min(level + s, 7)` after
/// `s` phase frames, so every word is white by the loop's end. The first
/// loop's native capture pairs with tape 38 (`ending-tone-loop-*` in
/// tools/certify.py). The third starts white: nothing between the second
/// and it writes the buffer, since the staff roll's and the credits'
/// palette steps are not modelled yet
/// ([#635](https://github.com/TusanHomichi/PSIV/issues/635)).
#[test]
fn the_ending_tone_loops_whiten_every_line() {
    let Some(loops) = ending_loops() else {
        return;
    };
    for (n, tone) in loops.iter().enumerate() {
        let (failures, [up, down]) = check_tone(&tone.start, None, LOOP, &tone.frames);
        assert_eq!(failures, Vec::<String>::new(), "loop {n}");
        // Every fourth frame of sixty steps: fifteen, none restored.
        assert_eq!((up, down), (15, 0), "loop {n}");
        assert_eq!(
            tone.frames[LOOP - 1].words,
            [0x0EEE; 64],
            "loop {n} ends white"
        );
        assert_eq!(tone.start == [0x0EEE; 64], n == 2, "loop {n}'s start");
    }
}

/// Negative controls for the ending check: an untoned loop and one that
/// steps every frame instead of every fourth both fail it. The third loop
/// starts white (#635), so neither control could fail there.
#[test]
fn the_ending_tone_check_catches_an_untoned_or_misstepped_model() {
    let Some(loops) = ending_loops() else {
        return;
    };
    for (n, tone) in loops.iter().enumerate().take(2) {
        let untoned: Vec<Frame> = tone
            .frames
            .iter()
            .map(|frame| Frame {
                count: frame.count,
                events: Vec::new(),
                words: tone.start,
                beams: Vec::new(),
            })
            .collect();
        let (failures, _) = check_tone(&tone.start, None, LOOP, &untoned);
        assert!(
            failures.iter().any(|f| f.starts_with("frame ")),
            "loop {n} untoned"
        );
        let mut words = tone.start;
        let every_frame: Vec<Frame> = tone
            .frames
            .iter()
            .map(|frame| {
                words = words.map(palette::increase_tone);
                Frame {
                    count: frame.count,
                    events: Vec::new(),
                    words,
                    beams: Vec::new(),
                }
            })
            .collect();
        let (failures, _) = check_tone(&tone.start, None, LOOP, &every_frame);
        assert!(
            failures.iter().any(|f| f.starts_with("frame ")),
            "loop {n} every frame"
        );
    }
}
