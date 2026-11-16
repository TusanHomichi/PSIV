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
/// loop's native capture pairs with tape 38 (`ending-tone-loop-*` in
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

/// The ending's second `InitVRAMAndCRAM` (`$078FE6`), after the first loop
/// left every word white (#637): the palette the tick before its fade, then
/// each tick from the fade's record on, and the ticks its `InitVramAndCram`
/// and the `Pal_FadeIn` after it were emitted on, counted from the fade.
struct InitFade {
    before: [u16; 64],
    words: Vec<[u16; 64]>,
    init_at: usize,
    fade_in_at: usize,
    saved: [u16; 64],
}

/// Ticks traced from the fade-out's record.
const INIT_TRACE: usize = 32;

fn ending_init_fade() -> Option<InitFade> {
    if !pack_present() {
        return None;
    }
    let mut runtime = runtime_on(0x013, [Some(CharId(0)), None, None, None, None], &[]);
    assert!(runtime.start_event(0x8021), "the ending starts");
    let mut tones = 0;
    let mut before = runtime.cram().words;
    let mut traced: Option<InitFade> = None;
    for _ in 0..20_000 {
        let events = runtime.tick(Input::Neutral);
        let emitted = |want: &dyn Fn(&SceneOp) -> bool| {
            events.iter().any(
                |event| matches!(event, RuntimeEvent::ScenePresentation { op, .. } if want(op)),
            )
        };
        let tone = emitted(&|op| {
            matches!(
                op,
                SceneOp::Presentation {
                    op: PresentationOp::PaletteIncreaseTone { .. }
                }
            )
        });
        let fade_out = emitted(&|op| matches!(op, SceneOp::FadeOut));
        let init = emitted(&|op| matches!(op, SceneOp::InitVramAndCram));
        let fade_in = emitted(&|op| matches!(op, SceneOp::FadeIn));
        for event in &events {
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        tones += usize::from(tone);
        if tones == 1 && fade_out && traced.is_none() {
            traced = Some(InitFade {
                before,
                words: Vec::new(),
                init_at: usize::MAX,
                fade_in_at: usize::MAX,
                saved: [0; 64],
            });
        }
        if let Some(fade) = traced.as_mut() {
            let at = fade.words.len();
            if init && fade.init_at == usize::MAX {
                fade.init_at = at;
            }
            if fade_in && fade.fade_in_at == usize::MAX {
                fade.fade_in_at = at;
                fade.saved = *runtime.effects.cram.saved();
            }
            fade.words.push(runtime.cram().words);
            if fade.words.len() == INIT_TRACE {
                return traced;
            }
        }
        before = runtime.cram().words;
    }
    panic!("the ending's second InitVRAMAndCRAM never ran");
}

/// What a correct trace holds, worked by hand from `PalFadeOut_ClrSpriteTbl`
/// (`$04223E`: every nibble down one a pass, one VBlank a pass) and
/// `Pal_FadeIn` (`$0421D4`: copy, clear, `VDP_EnableDisplay`'s frame, then
/// every nibble up one toward the copy a pass): each mismatch, described.
fn check_init_fade(fade: &InitFade, pal_init: &[u16; 64]) -> Vec<String> {
    let mut failures = Vec::new();
    if fade.before != [0x0EEE; 64] {
        failures.push("the fade does not start from white".to_owned());
    }
    // $EEE has fourteen levels: pass k (from 1) leaves k fewer, on tick k - 1.
    for (tick, words) in fade.words.iter().enumerate().take(14) {
        let level = 0x0E - tick as u16 - 1;
        let want = level * 0x0111;
        if *words != [want; 64] {
            failures.push(format!("tick {tick}: {:#06x}, not {want:#06x}", words[0]));
        }
    }
    // `DMAPlanes_VInt`'s frame, then `Pal_Init` copied and cleared by the
    // fade-in before the next VBlank: black on both.
    if fade.init_at != 14 {
        failures.push(format!("InitVramAndCram on tick {}, not 14", fade.init_at));
    }
    if fade.fade_in_at != 15 {
        failures.push(format!("FadeIn on tick {}, not 15", fade.fade_in_at));
    }
    for tick in [14, 15] {
        if fade.words.get(tick) != Some(&[0; 64]) {
            failures.push(format!("tick {tick} is not black"));
        }
    }
    if fade.saved != *pal_init {
        failures.push("the fade-in did not copy Pal_Init".to_owned());
    }
    // Pass p (from 1), shown on tick 15 + p: each nibble min(target, p).
    for pass in 1..=15u16 {
        let want = pal_init.map(|word| {
            (0..4).fold(0u16, |acc, nibble| {
                let target = (word >> (4 * nibble)) & 0xF;
                acc | (target.min(pass) << (4 * nibble))
            })
        });
        let tick = 15 + usize::from(pass);
        if fade.words.get(tick) != Some(&want) {
            failures.push(format!("fade-in pass {pass} (tick {tick}) differs"));
        }
    }
    failures
}

fn pal_init() -> [u16; 64] {
    let data = psiv_data::GameData::load(std::path::Path::new(super::PACK)).expect("pack loads");
    let words = data
        .scene_palettes()
        .words(0x0009_F2BC)
        .expect("the pack carries Pal_Init");
    words[..64].try_into().unwrap()
}

/// Acceptance for #637: `InitVRAMAndCRAM`'s `PalFadeOut_ClrSpriteTbl` steps
/// the live palette from white to black one level a frame over fourteen
/// frames (the oracle's frames 10086-10099 on tape 38), spends
/// `DMAPlanes_VInt`'s frame, and the `Pal_FadeIn` after it ramps `Pal_Init`
/// up from black.
#[test]
fn the_endings_init_fades_out_a_level_a_frame() {
    let Some(fade) = ending_init_fade() else {
        return;
    };
    assert_eq!(check_init_fade(&fade, &pal_init()), Vec::<String>::new());
}

/// Negative controls: the model before #637 (white held, then black at
/// once), a fade at two frames a pass, and a fade-in that never ran each fail
/// the check.
#[test]
fn the_init_fade_check_catches_a_missing_or_slow_fade() {
    let Some(fade) = ending_init_fade() else {
        return;
    };
    let pal_init = pal_init();
    let mut instant = fade.words.clone();
    for (tick, words) in instant.iter_mut().enumerate() {
        *words = if tick == 0 { [0x0EEE; 64] } else { [0; 64] };
    }
    let mut slow = fade.words.clone();
    for (tick, words) in slow.iter_mut().enumerate().take(14) {
        *words = [(0x0E - (tick as u16 / 2) - 1) * 0x0111; 64];
    }
    let mut unlit = fade.words.clone();
    for words in unlit.iter_mut().skip(16) {
        *words = [0; 64];
    }
    for (name, words) in [("instant", instant), ("slow", slow), ("unlit", unlit)] {
        let wrong = InitFade {
            words,
            before: fade.before,
            init_at: fade.init_at,
            fade_in_at: fade.fade_in_at,
            saved: fade.saved,
        };
        assert!(
            !check_init_fade(&wrong, &pal_init).is_empty(),
            "{name} passed"
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
        let calls = runtime.map_updates().calls;
        let events = runtime.tick(Input::Neutral);
        if events
            .iter()
            .any(|event| emits(event, |op| matches!(op, SceneOp::DmaPlanes)))
        {
            assert_eq!(
                runtime.map_updates().calls,
                calls,
                "DMA spends no map update"
            );
        }
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
/// tables, and the third loop's input: the credits entry palette with the
/// ramp's last words. Each record's tick is worked by hand from the one
/// before it: a staff loop spends up to its terminal stepping frame (`first + 48` after
/// six stores), the ramp through its fourteenth (`first + 105`). The plane
/// clear spends no frame; `DMAPlanes_VInt` (`$079CF4`) spends one bare frame
/// between the staff fade-in and the sixty-frame restore.
#[test]
fn the_staff_roll_and_credits_ramp_reach_the_third_loop() {
    let Some(frames) = ending_to_third_loop() else {
        return;
    };
    let events: Vec<_> = frames
        .iter()
        .flat_map(|frame| frame.events.clone())
        .collect();
    use SceneOp::{FadeIn as In, InitVramAndCram as Init};
    super::super::fixed_fades::assert_fades(
        &events,
        &[Init, In, Init, In, Init, In, Init, Init, In],
    );
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
    let dma = tick_of(&frames, fade_in, |op| matches!(op, SceneOp::DmaPlanes));
    assert_eq!(dma - fade_in, first + 48, "six stores, then $1110 unstored");
    assert_eq!(restore - dma, 1, "the DMA VInt before the restore");
    let credits_init = tick_of(&frames, restore, |op| {
        matches!(op, SceneOp::InitVramAndCram)
    });
    let credits_in = tick_of(&frames, credits_init, |op| matches!(op, SceneOp::FadeIn));
    assert_eq!(
        credits_init - restore,
        61,
        "sixty restore frames, then one black Out pass"
    );
    assert_eq!(credits_in - credits_init, 1, "Init spends its DMA frame");
    let data = GameData::load(Path::new(super::PACK)).expect("pack loads");
    // Credits palette uploads remain #640. Until then the fade's input is
    // Pal_Init, not the preceding staff restore's black (#659).
    let init = data.scene_palettes().words(0x09_F2BC).expect("Pal_Init");
    let largest = init
        .iter()
        .flat_map(|w| [w & 15, (w >> 4) & 15, (w >> 8) & 15])
        .max()
        .unwrap();
    assert_eq!(
        ramp - credits_in,
        usize::from(largest) + 2,
        "display frame, changing passes, final pass"
    );
    assert_eq!(frames[credits_in].words, [0; 64], "fade-in display frame");
    // Hand-worked nibble ramps: pass n adds one to each component until
    // its target, then one unchanged pass. No palette-model helper here.
    for pass in 1..=usize::from(largest) + 1 {
        let expected: Vec<u16> = init
            .iter()
            .map(|word| {
                [0, 4, 8]
                    .into_iter()
                    .map(|shift| ((word >> shift) & 15).min(pass as u16) << shift)
                    .sum()
            })
            .collect();
        assert_eq!(
            frames[credits_in + pass].words.as_slice(),
            expected,
            "credits In pass {pass}"
        );
    }
    assert_eq!(frames[ramp - 1].words, init);
    eprintln!(
        "Ending state: staff restore {restore}, credits Init {credits_init}, In {credits_in}, ramp {ramp}, third tone {third}; fixed In {} frames",
        ramp - credits_in
    );
    assert_eq!(frames[restore - 1].words, [0x0EEE; 64]);
    // Sixty restore frames take every component down a level on the
    // fifteen whose count is a multiple of four: seven reach black.
    assert_eq!(frames[restore + 59].words, [0; 64]);
    let first = first_eighth(frames[ramp].count);
    assert_eq!(third - ramp, first + 105, "fourteen steps");
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
    assert_eq!(frames[ramp + first - 1].words[18..26], init[18..26]);
    assert_eq!(frames[ramp + first].words[18..26], primary[14..22]);
    // The third loop's input: step 13's primary words 1-8 and secondary
    // words 8-9, over the credits entry's provisional Pal_Init.
    let mut input: [u16; 64] = init.try_into().unwrap();
    input[18..26].copy_from_slice(&primary[1..9]);
    input[30..32].copy_from_slice(&secondary[8..10]);
    assert_eq!(frames[third - 1].words, input);
    assert_ne!(input, [0x0EEE; 64], "not the stale white");
}
