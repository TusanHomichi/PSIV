//! The CRAM model against hand-worked cartridge arithmetic. Every expected
//! word below is worked from the instruction listing the op cites, on short
//! made-up palettes, never copied from the ROM or produced by the code under
//! test. The pack-backed scene runs are `crate::suites::palette`.

use std::collections::BTreeMap;

use psiv_core::{PaletteBuffer, PaletteHold, PaletteSource, PresentationOp, SceneOp};
use psiv_data::{PanelPalette, ScenePalettes};

use super::{Cram, CramView};

const LINE: usize = 16;

/// A CRAM with `live` and `saved` buffers, no program.
fn cram(live: [u16; 64], saved: [u16; 64]) -> Cram {
    Cram {
        view: CramView {
            words: live,
            ..CramView::default()
        },
        saved,
        program: None,
        queued: None,
    }
}

fn filled(word: u16) -> [u16; 64] {
    [word; 64]
}

fn op(op: PresentationOp) -> SceneOp {
    SceneOp::Presentation { op }
}

/// Runs `op` the way a scene does: applied on frame `start`, then `frames - 1`
/// more ticks, the words recorded after each frame.
fn run(
    cram: &mut Cram,
    scene_op: SceneOp,
    start: u16,
    frames: u16,
    palettes: &ScenePalettes,
) -> Vec<[u16; 64]> {
    let mut trace = Vec::new();
    let _ = cram.apply(&scene_op, start, palettes, None, &mut 0);
    trace.push(cram.view().words);
    for k in 1..frames {
        cram.tick(start.wrapping_add(k), &mut 0);
        trace.push(cram.view().words);
    }
    trace
}

/// The frames of a trace on which any word changed.
fn changes(before: [u16; 64], trace: &[[u16; 64]]) -> Vec<usize> {
    let mut last = before;
    let mut out = Vec::new();
    for (k, words) in trace.iter().enumerate() {
        if *words != last {
            out.push(k);
        }
        last = *words;
    }
    out
}

fn tone(frames: u16, hold: Option<PaletteHold>) -> SceneOp {
    op(PresentationOp::PaletteIncreaseTone { frames, hold })
}

/// Acceptance: a 30-frame tone loop steps on every frame whose
/// `Main_Frame_Count & 3` is zero. Starting at count 0, 1, 2, 3 those are
/// k = 0,4..28 (8); 3,7..27 (7); 2,6..26 (7); 1,5..29 (8).
const PHASE_HITS: [&[usize]; 4] = [
    &[0, 4, 8, 12, 16, 20, 24, 28],
    &[3, 7, 11, 15, 19, 23, 27],
    &[2, 6, 10, 14, 18, 22, 26],
    &[1, 5, 9, 13, 17, 21, 25, 29],
];

/// The tone from black: each step is visible until white, seven steps, so
/// the first seven phase frames are exactly the frames the palette changes.
#[test]
fn a_thirty_frame_tone_steps_on_the_phase_frames() {
    let none = ScenePalettes::default();
    for start in 0u16..4 {
        let words = filled(0x0000);
        let mut cram = cram(words, words);
        let trace = run(&mut cram, tone(30, None), start, 30, &none);
        let hits = PHASE_HITS[usize::from(start)];
        assert_eq!(changes(words, &trace), hits[..7], "start {start}");
        assert_eq!(cram.view().words, filled(0x0EEE), "start {start}");
        assert!(!cram.running());
    }
}

/// Seven or eight steps, told apart on a word whose restore keeps moving for
/// more than eight: `$00EC` toward `$00EE` (`loc_58666` by hand: red C, A, 8,
/// 6, 4, 2, 0 over six steps to `$00E0`; step 7 borrows, `$00E0 - 2 = $00DE`,
/// green `$C0` differs from `$E0`, `- $20 = $00BE`; step 8: red E matches,
/// green `$A0` differs, `$009E`). A 30-frame loop from count 1 or 2 ends at
/// `$00BE`, from 0 or 3 at `$009E`.
#[test]
fn a_thirty_frame_loop_takes_seven_or_eight_steps_by_phase() {
    let none = ScenePalettes::default();
    let expected = [0x009E, 0x00BE, 0x00BE, 0x009E];
    for start in 0u16..4 {
        let mut cram = cram(filled(0x00EC), filled(0x00EE));
        let restore = op(PresentationOp::PaletteRestoreTone { frames: 30 });
        let trace = run(&mut cram, restore, start, 30, &none);
        let hits = PHASE_HITS[usize::from(start)];
        assert_eq!(changes(filled(0x00EC), &trace), hits, "start {start}");
        assert_eq!(
            cram.view().words[0],
            expected[usize::from(start)],
            "start {start}"
        );
    }
}

/// The step the tone applies is `Pal_IncreaseTone`'s: one level per
/// component, `$E` the ceiling, `$EEE` skipped, worked by hand.
#[test]
fn one_tone_step_matches_the_routine() {
    let none = ScenePalettes::default();
    let mut words = filled(0x0000);
    words[0] = 0x0642; // -> $864
    words[1] = 0x00EC; // red $C -> $E, green $E stays, blue 0 -> 2: $2EE
    words[2] = 0x0EEE; // skipped
    let mut cram = cram(words, words);
    let _ = cram.apply(&tone(1, None), 0, &none, None, &mut 0);
    assert_eq!(cram.view().words[..3], [0x0864, 0x02EE, 0x0EEE]);
    assert_eq!(cram.view().words[3], 0x0222);
    // Off phase: nothing.
    let mut cram = super::Cram {
        program: None,
        ..cram.clone()
    };
    let before = cram.view().words;
    let _ = cram.apply(&tone(1, None), 1, &none, None, &mut 0);
    assert_eq!(cram.view().words, before);
}

fn hold_palettes() -> ScenePalettes {
    let mut words = BTreeMap::new();
    words.insert(0x0029_6300, (0..16).map(|i| 0x0100 + i).collect());
    ScenePalettes::from_parts(words, BTreeMap::new())
}

const HOLD: PaletteHold = PaletteHold {
    source_rom_addr: 0x0029_6300,
    destination_ram: 0xFFFF_FB40,
    words: 16,
};

/// The hold is copied after the frame's tone step, every frame: line 2
/// equals the held words on every frame, the phase frames included (where a
/// copy before the step would leave them one level up).
#[test]
fn the_hold_is_written_after_the_step_every_frame() {
    let palettes = hold_palettes();
    let held: Vec<u16> = (0..16).map(|i| 0x0100 + i).collect();
    let words = filled(0x0222);
    let mut cram = cram(words, words);
    let trace = run(&mut cram, tone(8, Some(HOLD)), 0, 8, &palettes);
    for (k, frame) in trace.iter().enumerate() {
        assert_eq!(frame[2 * LINE..3 * LINE], held[..], "frame {k}");
    }
    // The other lines took the frame-0 and frame-4 steps: $222 -> $444 -> $666.
    assert_eq!(trace[0][0], 0x0444);
    assert_eq!(trace[7][0], 0x0666);
    assert_eq!(trace[7][63], 0x0666);
}

/// `loc_58666` steps every component toward `Palette_Table_Buffer_2`, the
/// saved buffer, with the whole-word borrow.
#[test]
fn the_restore_steps_toward_the_saved_buffer() {
    let none = ScenePalettes::default();
    let mut live = filled(0x0EEE);
    let mut saved = filled(0x0246);
    live[5] = 0x0000;
    saved[5] = 0x0002;
    let mut cram = cram(live, saved);
    let restore = op(PresentationOp::PaletteRestoreTone { frames: 40 });
    let trace = run(&mut cram, restore, 0, 40, &none);
    // One step: $EEE -> $CCC.
    assert_eq!(trace[0][0], 0x0CCC);
    // Red 6 is reached after 4 steps, green 4 after 5, blue 2 after 6;
    // ten phase frames in forty, so the restore finishes on frame 20.
    assert_eq!(trace[39][0], 0x0246);
    assert_eq!(trace[20][0], 0x0246);
    assert_eq!(trace[16][0], 0x0446);
    // The borrow: $0000 toward $0002 is $FDDE after one step (hand-worked in
    // `psiv_core::palette`'s tests), not a clamp at zero.
    assert_eq!(trace[0][5], 0xFDDE);
}

/// Recovery's flashes step only the first two lines and only red and green;
/// four up and four back return every word to the saved copy.
#[test]
fn the_recovery_flash_steps_red_and_green_of_two_lines() {
    let none = ScenePalettes::default();
    let mut live = filled(0x0246);
    live[40] = 0x0000;
    let mut cram = cram(live, live);
    let up = op(PresentationOp::PaletteToneFlash {
        brighten: true,
        words: 32,
        frames: 16,
    });
    let trace = run(&mut cram, up, 0, 16, &none);
    // Four phase frames: red 6 -> E (capped after 4), green 4 -> C, blue kept.
    assert_eq!(trace[15][0], 0x02CE);
    assert_eq!(trace[15][31], 0x02CE);
    assert_eq!(trace[15][32], 0x0246, "line 2 is not stepped");
    assert_eq!(trace[15][40], 0x0000);
    let down = op(PresentationOp::PaletteToneFlash {
        brighten: false,
        words: 32,
        frames: 16,
    });
    let trace = run(&mut cram, down, 16, 16, &none);
    assert_eq!(trace[15][..32], [0x0246; 32]);
}

fn copy(source: PaletteSource, destination: PaletteBuffer, start: u8, words: u8) -> SceneOp {
    op(PresentationOp::PaletteCopy {
        source,
        destination,
        start,
        words,
    })
}

/// The recovery's `trap #1` copies 32 words: lines 2 and 3 of the copy keep
/// what they held.
#[test]
fn the_recovery_copy_leaves_the_last_two_saved_lines() {
    let none = ScenePalettes::default();
    let live = filled(0x0246);
    let saved = filled(0x0EEE);
    let mut cram = cram(live, saved);
    let recovery = psiv_core::scene_for(psiv_core::EventIndex(0x0021)).expect("Event_Recovery");
    let _ = cram.apply(&recovery.ops[0], 0, &none, None, &mut 0);
    assert_eq!(cram.saved()[..32], [0x0246; 32]);
    assert_eq!(cram.saved()[32..], [0x0EEE; 32]);
}

#[test]
fn copies_fills_and_rom_loads_land_in_their_buffer() {
    let mut words = BTreeMap::new();
    words.insert(0x0002_EEA4, vec![0x0002, 0x0004, 0x0006]);
    let palettes = ScenePalettes::from_parts(words, BTreeMap::new());
    let mut cram = cram(filled(0x0246), filled(0x0000));
    // Saved -> live, all 64.
    let _ = cram.apply(
        &copy(
            PaletteSource::Buffer(PaletteBuffer::Saved),
            PaletteBuffer::Live,
            0,
            64,
        ),
        0,
        &palettes,
        None,
        &mut 0,
    );
    assert_eq!(cram.view().words, filled(0));
    // The ending's fifteen zeros at `$FFFFFB40` after a $EEE wash.
    let _ = cram.apply(
        &copy(PaletteSource::Fill(0x0EEE), PaletteBuffer::Live, 0, 64),
        0,
        &palettes,
        None,
        &mut 0,
    );
    let _ = cram.apply(
        &copy(PaletteSource::Fill(0), PaletteBuffer::Live, 32, 15),
        0,
        &palettes,
        None,
        &mut 0,
    );
    assert_eq!(cram.view().words[31], 0x0EEE);
    assert_eq!(cram.view().words[32..47], [0; 15]);
    assert_eq!(cram.view().words[47], 0x0EEE);
    // A ROM palette into the saved buffer.
    let _ = cram.apply(
        &copy(PaletteSource::Rom(0x0002_EEA4), PaletteBuffer::Saved, 0, 3),
        0,
        &palettes,
        None,
        &mut 0,
    );
    assert_eq!(cram.saved()[..4], [0x0002, 0x0004, 0x0006, 0x0000]);
    // A palette the pack lacks is listed, and nothing is written.
    let before = cram.view().words;
    let _ = cram.apply(
        &copy(PaletteSource::Rom(0x0012_3456), PaletteBuffer::Live, 0, 16),
        0,
        &palettes,
        None,
        &mut 0,
    );
    assert_eq!(cram.view().words, before);
    assert_eq!(cram.view().missing, vec![0x0012_3456]);
}

#[test]
fn single_word_writes() {
    let none = ScenePalettes::default();
    let mut cram = cram(filled(0x0000), filled(0x0000));
    let revision = cram.view().revision;
    let _ = cram.apply(
        &op(PresentationOp::SetPaletteWords {
            offset: 0x0C,
            first: 0x000E,
            second: 0x00E0,
        }),
        0,
        &none,
        None,
        &mut 0,
    );
    assert_eq!(cram.view().words[12..14], [0x000E, 0x00E0]);
    assert_ne!(cram.view().revision, revision);
    // `addi.w #$220` wraps as a word: $FF00 + $220 = $0120.
    cram.write(13, 0xFF00);
    let _ = cram.apply(
        &op(PresentationOp::AddPaletteWord {
            offset: 0x0D,
            add: 0x0220,
        }),
        0,
        &none,
        None,
        &mut 0,
    );
    assert_eq!(cram.view().words[13], 0x0120);
}

/// `loc_78296`: nine words at `destination_ram` per frame, the next after the
/// frame's corrected `dbf` wait.
#[test]
fn the_rykros_cycle_writes_a_frame_then_waits_its_delay() {
    let mut words = BTreeMap::new();
    words.insert(0x0007_82DA, (0..54u16).collect());
    let palettes = ScenePalettes::from_parts(words, BTreeMap::new());
    let mut cram = cram(filled(0xFFFF), filled(0));
    const DELAYS: &[u8] = &[2, 3];
    let cycle = op(PresentationOp::RykrosPaletteCycle {
        source_rom_addr: 0x0007_82DA,
        destination_ram: 0xFFFF_FB2E,
        words_per_frame: 9,
        delays: DELAYS,
    });
    let trace = run(&mut cram, cycle, 0, 5, &palettes);
    // `$FFFFFB2E` is word 23: frame 0 writes table words 0..9 at 23..32.
    let first: Vec<u16> = (0..9).collect();
    let second: Vec<u16> = (9..18).collect();
    assert_eq!(trace[0][23..32], first[..]);
    assert_eq!(trace[1][23..32], first[..]);
    assert_eq!(trace[2][23..32], second[..]);
    assert_eq!(trace[0][22], 0xFFFF);
    assert_eq!(trace[0][32], 0xFFFF);
}

/// `loc_6ED4E`: words 49..51 of `Palette_Line_4` from table words `n`,
/// `n + 7`, `n + 14`, every `frame_delay` frames.
#[test]
fn the_zio_ramp_writes_three_interleaved_words() {
    let mut words = BTreeMap::new();
    words.insert(0x0006_EE16, (100..121u16).collect());
    let palettes = ScenePalettes::from_parts(words, BTreeMap::new());
    let mut cram = cram(filled(0), filled(0));
    let ramp = op(PresentationOp::PaletteRampFromTable {
        source_rom_addr: 0x0006_EE16,
        destination_ram: 0xFFFF_FB60,
        steps: 7,
        frame_delay: 30,
    });
    let trace = run(&mut cram, ramp, 0, 210, &palettes);
    assert_eq!(trace[0][48..52], [0, 100, 107, 114]);
    assert_eq!(trace[29][49..52], [100, 107, 114]);
    assert_eq!(trace[30][49..52], [101, 108, 115]);
    assert_eq!(trace[180][49..52], [106, 113, 120]);
}

/// `loc_6E842`: a pass writes the first non-negative word and the word
/// `second_skip` bytes past the advanced pointer to the two words at
/// `offset`.
#[test]
fn the_engine_light_cycle_writes_pairs() {
    let mut words = BTreeMap::new();
    // Made-up table: 1, 2, $8000 (wraps to the start), 3, 4.
    words.insert(0x0006_E908, vec![1, 2, 0x8000, 3, 4]);
    let palettes = ScenePalettes::from_parts(words, BTreeMap::new());
    let mut cram = cram(filled(0), filled(0));
    let cycle = op(PresentationOp::PaletteCycleFromTable {
        source_rom_addr: 0x0006_E908,
        offset: 0x1C,
        second_skip: 2,
        steps: 3,
        frame_delay: 8,
    });
    let trace = run(&mut cram, cycle, 0, 24, &palettes);
    // Pass 1: d0 = word 0 = 1, the pointer now at word 1; the second word is
    // two bytes past it, word 2 = $8000.
    assert_eq!(trace[0][28..30], [1, 0x8000]);
    // Pass 2: d0 = word 1 = 2, second = word 3 = 3.
    assert_eq!(trace[8][28..30], [2, 3]);
    // Pass 3: word 2 is negative, the pointer goes back: d0 = 1, second $8000.
    assert_eq!(trace[16][28..30], [1, 0x8000]);
}

/// `Pal_VariableFadeOut`, `$ED52` = 1: the copy to the saved buffer, then a
/// pass every two frames until black; the op answers with the frames.
#[test]
fn a_variable_fade_out_answers_its_length() {
    let none = ScenePalettes::default();
    let mut live = filled(0);
    live[0] = 0x0246;
    let mut cram = cram(live, filled(0x0EEE));
    let fade = op(PresentationOp::VariablePaletteFade {
        fade_in: false,
        mode: 1,
    });
    // Largest nibble 6: six passes of two frames.
    assert_eq!(cram.apply(&fade, 0, &none, None, &mut 0), Some(12));
    assert_eq!(cram.saved()[0], 0x0246);
    assert_eq!(cram.view().words[0], 0x0135);
    for k in 1..12u16 {
        cram.tick(k, &mut 0);
    }
    assert_eq!(cram.view().words[0], 0x0000);
    assert!(!cram.running());
}

/// `Pal_VariableFadeIn`, `$ED52` = 1: copy, clear, one display-enable frame,
/// then passes up to the copy and one that changes nothing.
#[test]
fn a_variable_fade_in_answers_its_length() {
    let none = ScenePalettes::default();
    let mut live = filled(0);
    live[0] = 0x0246;
    let mut cram = cram(live, filled(0));
    let fade = op(PresentationOp::VariablePaletteFade {
        fade_in: true,
        mode: 1,
    });
    // 1 + (6 + 1) * 2.
    assert_eq!(cram.apply(&fade, 0, &none, None, &mut 0), Some(15));
    assert_eq!(cram.view().words[0], 0x0000);
    cram.tick(1, &mut 0);
    assert_eq!(cram.view().words[0], 0x0111);
    for k in 2..15u16 {
        cram.tick(k, &mut 0);
    }
    assert_eq!(cram.view().words[0], 0x0246);
}

/// `Pal_VariableFadeToRed`/`FromRed`: eight stages of `delay + 1` frames.
#[test]
fn the_red_fades_take_green_and_blue_down_and_back() {
    let none = ScenePalettes::default();
    let mut cram = cram(filled(0x0EEE), filled(0));
    let trace = run(
        &mut cram,
        op(PresentationOp::FadeToRed { lines: 1 }),
        0,
        16,
        &none,
    );
    assert_eq!(trace[0][0], 0x0CCE);
    assert_eq!(trace[15][0], 0x000E);
    assert_eq!(cram.saved()[0], 0x0EEE);
    let trace = run(
        &mut cram,
        op(PresentationOp::FadeFromRed { lines: 1 }),
        16,
        16,
        &none,
    );
    assert_eq!(trace[0][0], 0x022E);
    assert_eq!(trace[15][0], 0x0EEE);
}

/// `InitVRAMAndCRAM`, `LoadPalette` and `Panel_Create` read the pack.
#[test]
fn scene_loads_read_the_pack() {
    let mut words = BTreeMap::new();
    words.insert(0x0009_F2BC, vec![0x0002; 64]);
    words.insert(0x001D_E0B8, vec![0x0004; 16]);
    words.insert(0x0009_F37C, vec![0x0006; 16]);
    words.insert(0x0009_F39C, vec![0x0008; 16]);
    let mut panels = BTreeMap::new();
    panels.insert(
        0x18B,
        vec![
            PanelPalette {
                art_tile: 0x8100,
                rom_addr: 0x0009_F37C,
            },
            PanelPalette {
                art_tile: 0xE100,
                rom_addr: 0x0009_F39C,
            },
        ],
    );
    let palettes = ScenePalettes::from_parts(words, panels);
    let mut cram = cram(filled(0x0246), filled(0));
    let _ = cram.apply(&SceneOp::InitVramAndCram, 0, &palettes, None, &mut 0);
    assert_eq!(cram.view().words, filled(0x0002));
    assert_eq!(cram.saved(), &filled(0x0246));
    let _ = cram.apply(
        &SceneOp::LoadPalette {
            rom_addr: 0x001D_E0B8,
            words: 16,
            start: 48,
        },
        0,
        &palettes,
        None,
        &mut 0,
    );
    assert_eq!(cram.view().words[48..], [0x0004; 16]);
    let _ = cram.apply(
        &SceneOp::PanelCreate { id: 0x18B },
        0,
        &palettes,
        None,
        &mut 0,
    );
    // $8100: line 0; $E100: `$E1 & $70` = $60 bytes, line 3.
    assert_eq!(cram.view().words[..16], [0x0006; 16]);
    assert_eq!(cram.view().words[16..32], [0x0002; 16]);
    assert_eq!(cram.view().words[48..], [0x0008; 16]);
    // A fade copies the live words to the saved buffer and leaves them.
    let live = cram.view().words;
    let _ = cram.apply(&SceneOp::FadeOut, 0, &palettes, None, &mut 0);
    assert_eq!(cram.saved(), &live);
    assert_eq!(cram.view().words, live);
}

#[test]
fn the_elsydeon_break_tones_to_white_and_back_in_forty_frames() {
    // `TextAction_ElsydeonBroken`: the buffer to the copy, twenty frames of
    // `Pal_IncreaseTone`, twenty of `loc_58666`. From frame 0 the steps land
    // on frames 0, 4, 8, 12, 16 (up) and 20, 24, 28, 32, 36 (back): a level-1
    // word (`$222`) rises five levels to `$CCC` and returns.
    let mut cram = cram(filled(0x0222), filled(0));
    cram.tone_and_back(20, 0, &mut 0);
    assert_eq!(
        cram.saved(),
        &filled(0x0222),
        "the whole buffer is copied first"
    );
    assert_eq!(cram.view().words, filled(0x0444), "frame 0 steps at once");
    for frame in 1..20 {
        cram.tick(frame, &mut 0);
    }
    assert_eq!(cram.view().words, filled(0x0CCC), "the top, after frame 19");
    cram.tick(20, &mut 0);
    assert_eq!(cram.view().words, filled(0x0AAA), "frame 20 starts back");
    for frame in 21..40 {
        cram.tick(frame, &mut 0);
    }
    assert_eq!(cram.view().words, filled(0x0222), "back on the copy");
    assert!(!cram.running(), "both loops are done");
    cram.tick(40, &mut 0);
    assert_eq!(cram.view().words, filled(0x0222));
}

/// The literal boarding copy crosses the live/saved boundary. The source
/// words are made up so this check needs no pack and catches clipping at 64.
#[test]
fn boarding_palette_copy_writes_line_three_and_the_adjacent_saved_word() {
    let live = std::array::from_fn(|n| n as u16);
    let saved = filled(0x0EEE);
    let values: Vec<u16> = (100..117).collect();
    let palettes = ScenePalettes::from_parts(
        BTreeMap::from([(0x0029_754E, values.clone())]),
        BTreeMap::new(),
    );
    let mut cram = cram(live, saved);
    let copy = op(PresentationOp::PaletteCopy {
        source: PaletteSource::Rom(0x0029_754E),
        destination: PaletteBuffer::Live,
        start: 48,
        words: 17,
    });
    let _ = cram.apply(&copy, 0, &palettes, None, &mut 0);
    let mut expected = live;
    expected[48..].copy_from_slice(&values[..16]);
    assert_eq!(cram.view().words, expected);
    let mut expected_saved = saved;
    expected_saved[0] = values[16];
    assert_eq!(cram.saved(), &expected_saved);
    assert!(cram.view().missing.is_empty());
    let revision = cram.view().revision;
    let _ = cram.apply(&copy, 1, &palettes, None, &mut 0);
    assert_eq!(
        cram.view().revision,
        revision,
        "identical copy changes no live word"
    );
}

/// Made-up Le Roof tables: the ramp `1..=9` and the cycle's two series
/// `100..=110` and `111..=121`.
fn le_roof_palettes() -> ScenePalettes {
    let mut words = BTreeMap::new();
    words.insert(0x0007_0626, (1..=9u16).collect());
    words.insert(0x0007_0672, (100..122u16).collect());
    ScenePalettes::from_parts(words, BTreeMap::new())
}

const LE_ROOF_CYCLE: psiv_core::CounterCycle = psiv_core::CounterCycle {
    source_rom_addr: 0x0007_0672,
    offset: 11,
    entries: 11,
};

/// Applies `scene_op` on frame `start` and ticks `frames - 1` more, sharing
/// one `$FFFFECF3` byte across the calls as the runtime does.
fn run_counted(
    cram: &mut Cram,
    scene_op: &SceneOp,
    start: u16,
    frames: u16,
    palettes: &ScenePalettes,
    index: &mut u8,
) -> Vec<[u16; 64]> {
    let _ = cram.apply(scene_op, start, palettes, None, index);
    let mut trace = vec![cram.view().words];
    for k in 1..frames {
        cram.tick(start.wrapping_add(k), index);
        trace.push(cram.view().words);
    }
    trace
}

/// `loc_705D6` then a 120-pass `loc_70638` loop, from frame 1: each pass
/// blacks words 2-10 and lights the ramp's last `d0 + 1` words over them
/// (`d0` stepping after the passes whose `d7`, 70 down, is a multiple of
/// 8), then the cycle writes words 11 and 12 on frames whose count is a
/// multiple of 4, its index carried from one loop to the next.
#[test]
fn the_le_roof_fade_in_lights_the_ramp_and_runs_the_cycle() {
    let palettes = le_roof_palettes();
    let mut cram = cram(filled(0x0EEE), filled(0));
    let mut index = 0;
    let grow = op(PresentationOp::PaletteGrowIn {
        source_rom_addr: 0x0007_0626,
        offset: 2,
        words: 9,
        passes: 71,
        cycle: LE_ROOF_CYCLE,
    });
    let trace = run_counted(&mut cram, &grow, 1, 72, &palettes, &mut index);
    // Pass 0, frame 1: one word lit, the rest black; frame 1 writes no cycle.
    assert_eq!(trace[0][2..11], [9, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(trace[0][11..13], [0x0EEE, 0x0EEE]);
    assert_eq!(trace[0][..2], [0x0EEE, 0x0EEE]);
    // Frame 4 (pass 3): the cycle's first write, index 0.
    assert_eq!(trace[2][11..13], [0x0EEE, 0x0EEE]);
    assert_eq!(trace[3][11..13], [100, 111]);
    // d7 = 64 on pass 6: pass 7 lights two.
    assert_eq!(trace[6][2..11], [9, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(trace[7][2..11], [8, 9, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(trace[7][11..13], [101, 112]);
    // Pass 63 on: all nine.
    assert_eq!(trace[62][2..11], [2, 3, 4, 5, 6, 7, 8, 9, 0]);
    assert_eq!(trace[63][2..11], [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(trace[70][2..11], [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    // Seventeen phase frames (4, 8, .., 68): eleven writes, a restart at
    // 11, six more: the index is 6, and the last write was entry 5.
    assert_eq!(index, 6);
    assert_eq!(trace[67][11..13], [105, 116]);
    // The program ended with pass 70: frame 72 changes nothing.
    assert_eq!(trace[71], trace[70]);
    assert!(!cram.running());
    assert_eq!(trace[70][13..], filled(0x0EEE)[13..]);

    let cycle = op(PresentationOp::CounterPaletteCycle {
        cycle: LE_ROOF_CYCLE,
        frames: 120,
    });
    let trace = run_counted(&mut cram, &cycle, 72, 121, &palettes, &mut index);
    // Frame 72 is a phase frame: entry 6 at once, the ramp left lit.
    assert_eq!(trace[0][11..13], [106, 117]);
    assert_eq!(trace[0][2..11], [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(trace[4][11..13], [107, 118]);
    // 30 phase frames in 120 (72, 76, .., 188): from 6, five to the
    // restart, eleven, eleven, three: index 3.
    assert_eq!(index, 3);
    assert_eq!(trace[119][11..13], [102, 113]);
    assert_eq!(trace[120], trace[119], "120 frames, then nothing");
    assert!(!cram.running());
}

/// Negative control: without the pack's tables the loops write nothing and
/// list the address, and a scratch `$FFFFECF3` left past the series
/// restarts at entry 0 rather than reading past it.
#[test]
fn the_le_roof_loops_refuse_a_missing_table_and_restart_a_stale_index() {
    let none = ScenePalettes::default();
    let mut cram = cram(filled(0x0EEE), filled(0));
    let cycle = op(PresentationOp::CounterPaletteCycle {
        cycle: LE_ROOF_CYCLE,
        frames: 4,
    });
    let mut index = 0;
    let trace = run_counted(&mut cram, &cycle, 0, 4, &none, &mut index);
    assert_eq!(trace[3], filled(0x0EEE));
    assert_eq!(cram.view().missing, [0x0007_0672]);
    assert_eq!(index, 0);

    let palettes = le_roof_palettes();
    let mut cram = super::Cram::default();
    let mut index = 0xFF;
    let trace = run_counted(&mut cram, &cycle, 0, 1, &palettes, &mut index);
    assert_eq!(trace[0][11..13], [100, 111]);
    assert_eq!(index, 1);
}

/// `Pal_VariableFadeOut_Lines_1_2`, `$ED52` = 1: words 0-31 copied to the
/// saved buffer (`moveq #$1F, d7` / `trap #1`), then a pass every two frames
/// until those 32 are black; words 32-63 and the saved buffer's are left.
#[test]
fn a_two_line_fade_out_steps_words_0_to_31_only() {
    let none = ScenePalettes::default();
    let mut live = filled(0);
    live[0] = 0x0246;
    live[31] = 0x0002;
    live[40] = 0x0EEE;
    let mut cram = cram(live, filled(0x0111));
    let fade = op(PresentationOp::TwoLinePaletteFade {
        fade_in: false,
        mode: 1,
    });
    // Largest nibble 6: six passes of two frames.
    assert_eq!(cram.apply(&fade, 0, &none, None, &mut 0), Some(12));
    assert_eq!(cram.saved()[0], 0x0246);
    assert_eq!(cram.saved()[31], 0x0002);
    assert_eq!(cram.saved()[40], 0x0111, "the copy is 32 words");
    assert_eq!(cram.view().words[0], 0x0135, "pass 1 on the op's frame");
    for k in 1..12 {
        cram.tick(k, &mut 0);
    }
    assert_eq!(cram.view().words[..32], [0; 32]);
    assert_eq!(cram.view().words[40], 0x0EEE);
}

/// `Pal_VariableFadeToPal2`, `$ED52` = 1: no copy and no clear; words 0-31
/// step up from where they are toward the saved buffer, and the pass that
/// changes nothing still waits its two frames.
#[test]
fn a_two_line_fade_up_starts_from_the_live_words() {
    let none = ScenePalettes::default();
    let mut live = filled(0);
    live[0] = 0x0002;
    live[40] = 0x0EEE;
    let mut saved = filled(0);
    saved[0] = 0x0024;
    let mut cram = cram(live, saved);
    let fade = op(PresentationOp::TwoLinePaletteFade {
        fade_in: true,
        mode: 1,
    });
    // $002 -> $013 -> $024, then a pass that changes nothing: three passes.
    assert_eq!(cram.apply(&fade, 0, &none, None, &mut 0), Some(6));
    assert_eq!(cram.view().words[0], 0x0013);
    cram.tick(1, &mut 0);
    assert_eq!(cram.view().words[0], 0x0013);
    cram.tick(2, &mut 0);
    assert_eq!(cram.view().words[0], 0x0024);
    for k in 3..6 {
        cram.tick(k, &mut 0);
    }
    assert_eq!(cram.view().words[0], 0x0024);
    assert_eq!(cram.view().words[40], 0x0EEE, "word 40 is past the span");
    assert_eq!(cram.saved()[0], 0x0024, "nothing is copied");
}

/// A fade the model bounds at `u16::MAX` passes (one the cartridge would
/// spin in forever) steps without overflowing its frame arithmetic; the
/// next-arc chain once ran Le Roof's fade up on a map whose own palette
/// program left a nibble it could not reach.
#[test]
fn a_bounded_endless_fade_steps_without_overflow() {
    let mut cram = cram(filled(0), filled(0x0EEE));
    cram.program = Some(super::Program::Passes {
        kind: super::Pass::VariableIn,
        elapsed: 0,
        first: 0,
        every: 3,
        count: u16::MAX,
        words: 32,
    });
    for k in 0..10 {
        cram.tick(k, &mut 0);
    }
    assert!(cram.running());
}
