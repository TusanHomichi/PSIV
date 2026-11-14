//! The Le Roof loops, the two-line variable fades and the pass bound against
//! hand-worked cartridge arithmetic, on made-up palettes as in the parent.

use super::*;

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
    cram.program = Some(crate::cram::Program::Passes {
        kind: crate::cram::Pass::VariableIn,
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

/// Ticks `cram` from `elapsed` until its program ends, at most `limit`
/// ticks; returns the ticks taken, then runs two more (the final tick's
/// follower and one past it), which must leave the words as they are.
fn tick_to_end(cram: &mut Cram, limit: u32) -> u32 {
    let mut ticks = 0u32;
    while cram.running() {
        assert!(ticks < limit, "still running after {limit} ticks");
        cram.tick(ticks as u16, &mut 0);
        ticks += 1;
    }
    let words = cram.view().words;
    cram.tick(0, &mut 0);
    cram.tick(1, &mut 0);
    assert_eq!(cram.view().words, words, "an ended program writes nothing");
    ticks
}

/// Every pass program at the model's `u16::MAX` bound, for each first-frame
/// offset (`Pal_VariableFadeIn`'s one `VDP_EnableDisplay` frame, or none)
/// and each delay (`$ED52` 0, 2 and `$FF`): it runs to its end, through the
/// tick that would make `elapsed + 1` leave `u16` and the one after, without
/// overflowing, and ends inside the frames the op answers the scene runner.
#[test]
fn every_saturated_pass_program_ends_inside_its_frames() {
    for kind in [
        crate::cram::Pass::VariableIn,
        crate::cram::Pass::VariableOut,
        crate::cram::Pass::ToRed,
        crate::cram::Pass::FromRed,
    ] {
        for first in [0u16, 1] {
            for every in [1u16, 3, 256] {
                let mut cram = cram(filled(0), filled(0x0EEE));
                cram.program = Some(crate::cram::Program::Passes {
                    kind,
                    elapsed: 0,
                    first,
                    every,
                    count: u16::MAX,
                    words: 32,
                });
                let frames = crate::cram::passes_frames(first, every, u16::MAX);
                assert_eq!(frames, u16::MAX, "{kind:?} {first} {every}: saturated");
                // `cram.program` holds elapsed 0 unstepped: one tick per
                // frame, the last frame the scene waits being `frames - 1`.
                let ticks = tick_to_end(&mut cram, u32::from(u16::MAX) + 2);
                assert!(
                    ticks <= u32::from(frames),
                    "{kind:?} first {first} every {every}: {ticks} ticks, {frames} frames"
                );
            }
        }
    }
}

/// Unsaturated pass programs keep their last pass where it was: `first`
/// plus `every` per pass after the first, inside `first + every * count`.
#[test]
fn the_pass_bound_leaves_unsaturated_fades_alone() {
    assert_eq!(crate::cram::passes_last(0, 3, 15), 42);
    assert_eq!(crate::cram::passes_frames(0, 3, 15), 45);
    assert_eq!(crate::cram::passes_last(1, 2, 7), 13);
    assert_eq!(crate::cram::passes_frames(1, 2, 7), 15);
    assert_eq!(crate::cram::passes_last(0, 1, 8), 7);
    // Saturated: the last pass is held one frame inside the answer.
    assert_eq!(crate::cram::passes_last(0, 3, u16::MAX), u16::MAX - 1);
    assert_eq!(crate::cram::passes_last(1, 256, u16::MAX), u16::MAX - 1);
}

/// `Pal_VariableFadeToPal2` from a word whose carry keeps it from ever
/// matching (`$001` toward `$000`: each pass adds one to a nibble and the
/// `$F` carry walks round all four): the model stops at `u16::MAX` passes,
/// answers `u16::MAX` frames, and the program it starts ends inside them,
/// its final tick and the one after stepping without overflow.
#[test]
fn an_endless_two_line_fade_up_ends_inside_the_frames_it_answers() {
    let none = ScenePalettes::default();
    let mut live = filled(0);
    live[0] = 0x0001;
    let mut cram = cram(live, filled(0));
    let fade = op(PresentationOp::TwoLinePaletteFade {
        fade_in: true,
        mode: 2,
    });
    let frames = cram.apply(&fade, 0, &none, None, &mut 0);
    assert_eq!(frames, Some(u16::MAX));
    // The apply ran frame 0; the scene waits frames 1..u16::MAX - 1.
    let ticks = tick_to_end(&mut cram, u32::from(u16::MAX));
    assert!(ticks < u32::from(u16::MAX), "{ticks}");
}

/// A literal-writes program whose last write is at frame `u16::MAX` makes
/// that write and ends there, without building a frame past the bound.
#[test]
fn a_write_at_the_last_frame_ends_without_overflow() {
    let mut cram = cram(filled(0), filled(0));
    cram.program = Some(crate::cram::Program::Writes {
        elapsed: 0,
        writes: vec![(3, 0, vec![0x0002]), (u16::MAX, 1, vec![0x0004])],
    });
    let ticks = tick_to_end(&mut cram, u32::from(u16::MAX) + 2);
    assert_eq!(ticks, u32::from(u16::MAX) + 1);
    assert_eq!(cram.view().words[..2], [0x0002, 0x0004]);
}
