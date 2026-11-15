//! `Cutscene_Ending`'s eighth-frame loops on the CRAM model (#638): every
//! word on every frame against hand-worked cartridge arithmetic, on made-up
//! palettes and tables as in the parent, with controls that a dropped write
//! or a stored terminal word fails.

use super::*;

/// `Palette_Table_Buffer+$5E`: word `$2F`.
const SLOT: usize = 0x2F;
/// Words every other slot holds: none of these loops may touch them.
const OTHER: u16 = 0x0246;

/// The first stepping frame for a loop starting at count `0..8`, by hand.
const FIRST: [u16; 8] = [0, 7, 6, 5, 4, 3, 2, 1];
const DOWN_FROM_WHITE: [u16; 6] = [0x0CCC, 0x0AAA, 0x0888, 0x0666, 0x0444, 0x0222];
const UP_FROM_0222: [u16; 6] = [0x0444, 0x0666, 0x0888, 0x0AAA, 0x0CCC, 0x0EEE];

const FADE_OUT: PresentationOp = PresentationOp::EndingStaffRollFadeOut {
    palette_offset: 0x5E,
    step: 0x222,
};
const FADE_IN: PresentationOp = PresentationOp::EndingStaffRollFadeIn {
    palette_offset: 0x5E,
    step: 0x222,
    target_bit: 0x0C,
};

fn staff_cram(word: u16) -> Cram {
    let mut live = filled(OTHER);
    live[SLOT] = word;
    cram(live, filled(0))
}

/// Applies `scene_op` on frame `start`, then ticks through `ticks` frames in
/// all; the answered frame count and the words after each frame.
fn trace(
    cram: &mut Cram,
    scene_op: PresentationOp,
    start: u16,
    ticks: u16,
    palettes: &ScenePalettes,
) -> (Option<u16>, Vec<[u16; 64]>) {
    let answered = cram.apply(&op(scene_op), start, palettes, None, &mut 0);
    let mut words = vec![cram.view().words];
    for k in 1..ticks {
        cram.tick(start.wrapping_add(k), &mut 0);
        words.push(cram.view().words);
    }
    (answered, words)
}

/// What a staff word loop's trace must show: `from` until the first
/// stepping frame, each of `stores` from its frame on (every eighth frame
/// from `first`), the last kept after the loop, and every other word
/// untouched. The failures, empty when it does.
fn check_word_loop(trace: &[[u16; 64]], from: u16, first: u16, stores: &[u16]) -> Vec<String> {
    let mut failures = Vec::new();
    for (k, words) in trace.iter().enumerate() {
        let k = k as u16;
        let made = if k < first {
            0
        } else {
            (usize::from((k - first) / 8) + 1).min(stores.len())
        };
        let expected = if made == 0 { from } else { stores[made - 1] };
        if words[SLOT] != expected {
            failures.push(format!(
                "frame {k}: {:#06X}, not {expected:#06X}",
                words[SLOT]
            ));
        }
        if let Some(slot) = (0..64).find(|&s| s != SLOT && words[s] != OTHER) {
            failures.push(format!("frame {k}: word {slot} changed"));
        }
    }
    failures
}

/// Acceptance: the fade out from white at all eight phases: six stores on
/// the stepping frames, `$0222` kept (the zero step stores nothing), every
/// other word unchanged, and the frames answered end on the stepping frame
/// that tests zero.
#[test]
fn the_staff_fade_out_writes_word_2f_on_every_eighth_frame_at_every_phase() {
    let none = ScenePalettes::default();
    for (start, &first) in FIRST.iter().enumerate() {
        let mut cram = staff_cram(0x0EEE);
        let (answered, words) = trace(&mut cram, FADE_OUT, start as u16, 80, &none);
        assert_eq!(answered, Some(first + 48), "start {start}");
        assert_eq!(
            check_word_loop(&words, 0x0EEE, first, &DOWN_FROM_WHITE),
            Vec::<String>::new(),
            "start {start}"
        );
        assert!(!cram.running(), "start {start}: the writes are done");
        assert_eq!(cram.view().words[SLOT], 0x0222);
    }
}

/// The fade back in from the fade out's `$0222`, at all eight phases: six
/// stores up to white, `$1110` (bit `$C`) never stored.
#[test]
fn the_staff_fade_in_writes_back_to_white_and_stops_before_bit_c() {
    let none = ScenePalettes::default();
    for (start, &first) in FIRST.iter().enumerate() {
        // A count past a wrap, to show only the low bits matter.
        let count = 0xFFF8u16.wrapping_add(start as u16);
        let mut cram = staff_cram(0x0222);
        let (answered, words) = trace(&mut cram, FADE_IN, count, 80, &none);
        assert_eq!(answered, Some(first + 48), "start {start}");
        assert_eq!(
            check_word_loop(&words, 0x0222, first, &UP_FROM_0222),
            Vec::<String>::new(),
            "start {start}"
        );
        assert_eq!(cram.view().words[SLOT], 0x0EEE);
    }
}

/// Negative controls: a model that drops a store, stores the terminal
/// word, steps a frame early or touches another word fails the check.
#[test]
fn the_staff_loop_check_catches_a_dropped_or_terminal_store() {
    let none = ScenePalettes::default();
    let mut cram = staff_cram(0x0EEE);
    let (_, words) = trace(&mut cram, FADE_OUT, 3, 80, &none);
    let first = FIRST[3];
    assert!(check_word_loop(&words, 0x0EEE, first, &DOWN_FROM_WHITE).is_empty());
    // The third store dropped: frames 21..28 keep the second.
    let mut dropped = words.clone();
    for frame in &mut dropped[usize::from(first + 16)..usize::from(first + 24)] {
        frame[SLOT] = 0x0AAA;
    }
    assert!(!check_word_loop(&dropped, 0x0EEE, first, &DOWN_FROM_WHITE).is_empty());
    // The zero result stored on the terminal frame.
    let mut terminal = words.clone();
    for frame in &mut terminal[usize::from(first + 48)..] {
        frame[SLOT] = 0;
    }
    assert!(!check_word_loop(&terminal, 0x0EEE, first, &DOWN_FROM_WHITE).is_empty());
    // A first store a frame early.
    let mut early = words.clone();
    early[usize::from(first - 1)][SLOT] = 0x0CCC;
    assert!(!check_word_loop(&early, 0x0EEE, first, &DOWN_FROM_WHITE).is_empty());
    // Another word written.
    let mut stray = words;
    stray[10][SLOT + 1] = 0;
    assert!(!check_word_loop(&stray, 0x0EEE, first, &DOWN_FROM_WHITE).is_empty());
}

/// Made-up ramp tables: primary `$100..$115`, secondary `$200..$209`.
fn ramp_palettes(primary: u16, secondary: u16) -> ScenePalettes {
    let mut words = BTreeMap::new();
    words.insert(0x0007_A274, (0x100..0x100 + primary).collect());
    words.insert(0x0007_A2A0, (0x200..0x200 + secondary).collect());
    ScenePalettes::from_parts(words, BTreeMap::new())
}

const RAMP: PresentationOp = PresentationOp::EndingCreditsPaletteRamp {
    primary_rom_addr: 0x0007_A274,
    primary_ram: 0xFFFF_FB24,
    primary_words: 8,
    secondary_rom_addr: 0x0007_A2A0,
    secondary_ram: 0xFFFF_FB3C,
    secondary_words: 2,
    secondary_from: 9,
    steps: 14,
};

/// The words the ramp leaves after `made` steps, by hand: words 18-25 hold
/// primary words `15 - made` on (step `made - 1` read `d5 = 15 - made`),
/// words 30-31 secondary words `2 * (made - 10)` on once a tenth step ran;
/// everything else is `OTHER`.
fn ramp_after(made: usize) -> [u16; 64] {
    let mut words = filled(OTHER);
    if made > 0 {
        let d5 = 15 - made as u16;
        for n in 0..8 {
            words[18 + n] = 0x100 + d5 + n as u16;
        }
    }
    if made >= 10 {
        let late = 2 * (made as u16 - 10);
        words[30] = 0x200 + late;
        words[31] = 0x201 + late;
    }
    words
}

/// Acceptance: fourteen steps on the stepping frames, at three phases,
/// every word on every frame, the loop's frames through the last step's.
#[test]
fn the_credits_ramp_writes_lines_one_words_2_to_9_then_14_and_15() {
    let palettes = ramp_palettes(22, 10);
    for (start, first) in [(0u16, 0u16), (3, 5), (0xFFFF, 1)] {
        let mut cram = cram(filled(OTHER), filled(0));
        let (answered, words) = trace(&mut cram, RAMP, start, 130, &palettes);
        assert_eq!(answered, Some(first + 105), "start {start:#06X}");
        for (k, frame) in words.iter().enumerate() {
            let k = k as u16;
            let made = if k < first {
                0
            } else {
                (usize::from((k - first) / 8) + 1).min(14)
            };
            assert_eq!(*frame, ramp_after(made), "start {start:#06X}, frame {k}");
        }
        // Spot checks, by hand: the first step's words and the last's.
        assert_eq!(
            words[usize::from(first)][18..26],
            [0x10E, 0x10F, 0x110, 0x111, 0x112, 0x113, 0x114, 0x115]
        );
        assert_eq!(
            words[usize::from(first + 104)][18..32],
            [
                0x101, 0x102, 0x103, 0x104, 0x105, 0x106, 0x107, 0x108, OTHER, OTHER, OTHER, OTHER,
                0x208, 0x209
            ]
        );
        assert!(cram.view().missing.is_empty());
    }
}

/// Negative controls: the hand-worked frames fail a ramp that skips its
/// secondary copies or reads the primary table from `d6` instead of `d5`.
#[test]
fn the_credits_ramp_check_catches_a_skipped_or_misread_copy() {
    let palettes = ramp_palettes(22, 10);
    let mut cram = cram(filled(OTHER), filled(0));
    let (_, words) = trace(&mut cram, RAMP, 0, 110, &palettes);
    let last = words[104];
    assert_eq!(last, ramp_after(14));
    let mut skipped = last;
    skipped[30] = OTHER;
    skipped[31] = OTHER;
    assert_ne!(skipped, ramp_after(14));
    let mut misread = last;
    for n in 0..8 {
        misread[18 + n] = 0x100 + 13 + n as u16;
    }
    assert_ne!(misread, ramp_after(14));
}

/// A table the pack lacks, or carries short, is listed missing and written
/// nowhere, and the loop still answers the frames it spends.
#[test]
fn the_credits_ramp_without_its_tables_writes_nothing_and_still_releases() {
    for (palettes, missing) in [
        (ScenePalettes::default(), vec![0x0007_A274, 0x0007_A2A0]),
        (ramp_palettes(21, 10), vec![0x0007_A274]),
        (ramp_palettes(22, 9), vec![0x0007_A2A0]),
    ] {
        let mut cram = cram(filled(OTHER), filled(0));
        let (answered, words) = trace(&mut cram, RAMP, 2, 120, &palettes);
        assert_eq!(answered, Some(6 + 105));
        assert!(words.iter().all(|frame| *frame == filled(OTHER)));
        assert_eq!(cram.view().missing, missing);
    }
}
