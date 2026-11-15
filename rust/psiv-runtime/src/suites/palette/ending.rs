//! `Cutscene_Ending`'s three sixty-frame `Pal_IncreaseTone` loops
//! (`$078F9C`, `$079BD2`, `$07A0FA`) on the pack: what `Runtime::cram`
//! holds on every frame of each, checked by hand with [`check_tone`].

use std::path::Path;

use super::{Frame, check_tone, pack_present, runtime_on};
use crate::RuntimeEvent;
use psiv_core::{CharId, Input, PresentationOp, SceneOp, palette};
use psiv_data::GameData;

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
/// loop's native capture pairs with tape 45 (`ending-tone-loop-*` in
/// tools/certify.py). The third starts from what the staff roll's restore
/// and the credits' ramp leave
/// ([`the_staff_roll_and_credits_ramp_reach_the_third_loop`]), not the
/// second's white.
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
        assert_ne!(tone.start, [0x0EEE; 64], "loop {n}'s start");
    }
}

/// Negative controls for the ending check: an untoned loop and one that
/// steps every frame instead of every fourth both fail it, on all three.
#[test]
fn the_ending_tone_check_catches_an_untoned_or_misstepped_model() {
    let Some(loops) = ending_loops() else {
        return;
    };
    for (n, tone) in loops.iter().enumerate() {
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

/// The ending's ticks from its start to the third tone loop's record.
/// `None` without the pack.
fn ending_to_third_loop() -> Option<Vec<Frame>> {
    if !pack_present() {
        return None;
    }
    let mut runtime = runtime_on(0x013, [Some(CharId(0)), None, None, None, None], &[]);
    assert!(runtime.start_event(0x8021), "the ending starts");
    let mut frames = Vec::new();
    let mut tones = 0;
    for _ in 0..200_000 {
        let events = runtime.tick(Input::Neutral);
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
        }
        tones += events.iter().filter(|e| emits(e, is_tone)).count();
        frames.push(Frame {
            count: runtime.frames,
            events,
            words: runtime.cram().words,
            beams: Vec::new(),
        });
        if tones == 3 {
            return Some(frames);
        }
        assert!(runtime.scene_active(), "the ending ended before its loops");
    }
    panic!("the ending's third loop never ran");
}

fn is_tone(op: &SceneOp) -> bool {
    matches!(
        op,
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone { .. }
        }
    )
}

fn emits(event: &RuntimeEvent, test: impl Fn(&SceneOp) -> bool) -> bool {
    matches!(event, RuntimeEvent::ScenePresentation { op, .. } if test(op))
}

/// The tick that emitted the first record `test` matches at or after `from`.
fn tick_of(frames: &[Frame], from: usize, test: impl Fn(&SceneOp) -> bool) -> usize {
    (from..frames.len())
        .find(|&k| frames[k].events.iter().any(|e| emits(e, &test)))
        .expect("the record runs")
}

/// The tick of the first presentation record `test` matches at or after
/// `from`.
fn presentation_at(frames: &[Frame], from: usize, test: fn(&PresentationOp) -> bool) -> usize {
    tick_of(
        frames,
        from,
        |op| matches!(op, SceneOp::Presentation { op } if test(op)),
    )
}

/// The first stepping frame of a loop whose first frame has `count`: the
/// next multiple of eight, worked by hand as `(8 - count % 8) % 8`.
fn first_eighth(count: u16) -> usize {
    usize::from((8 - count % 8) % 8)
}

/// Acceptance on the pack, separately from any visual claim: the staff
/// roll's fade out of word `$2F` from the second loop's white, 180 frames,
/// the staff-roll music, 180 frames, the fade back in, the sixty-frame
/// restore toward the cleared copy, the credits' ramp from the pack's two
/// tables, and the third loop's input: black but for the ramp's last
/// words. Each record's tick is worked by hand from the one before it: a
/// staff loop spends up to its terminal stepping frame (`first + 48` after
/// six stores), the ramp through its fourteenth (`first + 105`). The plane
/// clear and DMA spend no frame natively yet: the helper frames are #608's.
#[test]
fn the_staff_roll_and_credits_ramp_reach_the_third_loop() {
    let Some(frames) = ending_to_third_loop() else {
        return;
    };
    let out = presentation_at(&frames, 0, |op| {
        matches!(op, PresentationOp::EndingStaffRollFadeOut { .. })
    });
    let music = tick_of(&frames, out, |op| {
        matches!(op, SceneOp::PlaySound { id: 0xAE })
    });
    let fade_in = presentation_at(&frames, music, |op| {
        matches!(op, PresentationOp::EndingStaffRollFadeIn { .. })
    });
    let restore = presentation_at(&frames, fade_in, |op| {
        matches!(op, PresentationOp::PaletteRestoreTone { frames: 60 })
    });
    let ramp = presentation_at(&frames, restore, |op| {
        matches!(op, PresentationOp::EndingCreditsPaletteRamp { .. })
    });
    let third = frames.len() - 1;
    // The second loop's white, then the fade out's six stores, each on its
    // stepping frame and not the frame before.
    assert_eq!(frames[out - 1].words, [0x0EEE; 64]);
    let first = first_eighth(frames[out].count);
    let down = [0x0CCC, 0x0AAA, 0x0888, 0x0666, 0x0444, 0x0222];
    for (n, &word) in down.iter().enumerate() {
        let at = out + first + 8 * n;
        let before = if n == 0 { 0x0EEE } else { down[n - 1] };
        assert_eq!(frames[at - 1].words[0x2F], before, "before store {n}");
        assert_eq!(frames[at].words[0x2F], word, "store {n}");
    }
    assert_eq!(
        music - out,
        first + 48 + 180,
        "the wait follows the zero test"
    );
    assert_eq!(
        frames[music - 1].words[0x2F],
        0x0222,
        "zero is never stored"
    );
    assert_eq!(fade_in - music, 180, "the second wait");
    let first = first_eighth(frames[fade_in].count);
    assert_eq!(
        restore - fade_in,
        first + 48,
        "six stores, then $1110 unstored"
    );
    assert_eq!(frames[restore - 1].words, [0x0EEE; 64]);
    // Sixty restore frames take every component down a level on the
    // fifteen whose count is a multiple of four: seven reach black.
    assert_eq!(frames[restore + 59].words, [0; 64]);
    let first = first_eighth(frames[ramp].count);
    assert_eq!(third - ramp, first + 105, "fourteen steps");
    let data = GameData::load(Path::new(super::PACK)).expect("pack loads");
    let primary = data
        .scene_palettes()
        .words(0x0007_A274)
        .expect("primary table");
    let secondary = data
        .scene_palettes()
        .words(0x0007_A2A0)
        .expect("secondary table");
    assert_eq!((primary.len(), secondary.len()), (22, 10));
    // Step 0 copies primary words 14-21 to words 18-25 on its frame.
    assert_eq!(frames[ramp + first - 1].words[18..26], [0; 8]);
    assert_eq!(frames[ramp + first].words[18..26], primary[14..22]);
    // The third loop's input: step 13's primary words 1-8 and secondary
    // words 8-9, over the restore's black.
    let mut input = [0; 64];
    input[18..26].copy_from_slice(&primary[1..9]);
    input[30..32].copy_from_slice(&secondary[8..10]);
    assert_eq!(frames[third - 1].words, input);
    assert_ne!(input, [0x0EEE; 64], "not the stale white");
}
