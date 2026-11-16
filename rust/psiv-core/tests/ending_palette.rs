//! `Cutscene_Ending`'s eighth-frame palette loops (#638): the staff roll's
//! word fade out and back in (`loc_79C8E`, `loc_79CCA`), the credits' ramp
//! (`loc_7A0A6`), their place in the scene stream and the palette wait the
//! runner holds them on. Every expected word and frame is worked by hand from
//! the instruction listing in `docs/scenes/89_Ending.md`, on made-up tables;
//! none is copied from the ROM or produced by the code under test.

mod common;

use common::map_with;
use psiv_core::palette::{
    RampStep, WordLoop, credits_ramp_frames, credits_ramp_rows, eighth_phase, staff_roll_fade_in,
    staff_roll_fade_out,
};
use psiv_core::{
    EventIndex, GameState, PresentationOp, SceneEffect, SceneInput, SceneOp, SceneRunner,
    StepFrames, scene_for,
};

/// The first frame of a loop whose `Main_Frame_Count & 7` is zero, for a
/// loop starting at count `0..8`: worked by hand, `(8 - count) mod 8`.
const FIRST: [u16; 8] = [0, 7, 6, 5, 4, 3, 2, 1];

/// `$0EEE` less `$222` six times; a seventh would be zero.
const DOWN_FROM_WHITE: [u16; 6] = [0x0CCC, 0x0AAA, 0x0888, 0x0666, 0x0444, 0x0222];
/// `$0222` plus `$222` six times; a seventh, `$1110`, sets bit `$C`.
const UP_FROM_0222: [u16; 6] = [0x0444, 0x0666, 0x0888, 0x0AAA, 0x0CCC, 0x0EEE];

fn stepped(first: u16, words: &[u16]) -> Vec<(u16, u16)> {
    words
        .iter()
        .enumerate()
        .map(|(n, &word)| (first + 8 * n as u16, word))
        .collect()
}

#[test]
fn the_phase_is_the_low_three_bits() {
    let hits: Vec<u16> = (0..20).filter(|&c| eighth_phase(c)).collect();
    assert_eq!(hits, [0, 8, 16]);
    // The staff roll reads the low byte, the credits the word: the same.
    assert!(eighth_phase(0xFF08) && eighth_phase(0x0108) && !eighth_phase(0xFF0C));
}

/// Acceptance: from white, at every phase, six stores on the stepping
/// frames, the zero result unstored, and the loop's frames end on the
/// stepping frame that tests it.
#[test]
fn the_staff_fade_out_steps_every_eighth_frame_and_never_stores_zero() {
    for (start, &first) in FIRST.iter().enumerate() {
        let start = start as u16;
        for count in [start, start + 0x100, start.wrapping_sub(8)] {
            let fade = staff_roll_fade_out(0x0EEE, count, 0x222);
            assert_eq!(
                fade,
                WordLoop {
                    writes: stepped(first, &DOWN_FROM_WHITE),
                    frames: first + 48,
                },
                "count {count:#06X}"
            );
        }
    }
}

/// `subi.w` is word arithmetic: below `$222` the word borrows through bit
/// 15 and is stored; an odd word never reaches zero, so the cartridge
/// spins, and the model cuts the loop at `u16::MAX` frames.
#[test]
fn the_staff_fade_out_borrows_at_word_width() {
    let fade = staff_roll_fade_out(0x0001, 0, 0x222);
    assert_eq!(fade.writes[..2], [(0, 0xFDDF), (8, 0xFBBD)]);
    assert_eq!(fade.frames, u16::MAX);
    // Frames 0, 8, ..., 65528: 8192 stores before the cut.
    assert_eq!(fade.writes.len(), 8192);
    assert_eq!(fade.writes.last().map(|w| w.0), Some(65528));
    // One step from zero: the loop ends on its first stepping frame.
    assert_eq!(
        staff_roll_fade_out(0x0222, 3, 0x222),
        WordLoop {
            writes: Vec::new(),
            frames: 5,
        }
    );
}

/// The fade back in from the fade out's last store, at two phases: six
/// stores up to white, `$1110` unstored.
#[test]
fn the_staff_fade_in_stops_unstored_on_bit_c() {
    for (count, first) in [(0, 0), (0x0165, 3)] {
        assert_eq!(
            staff_roll_fade_in(0x0222, count, 0x222, 0x0C),
            WordLoop {
                writes: stepped(first, &UP_FROM_0222),
                frames: first + 48,
            },
            "count {count:#06X}"
        );
    }
    // The test is bit $C of the sum, not a bound: $2000 climbs through
    // $2222..$2EEE (seven stores, none with bit $C) and ends on $3110.
    let high = staff_roll_fade_in(0x2000, 0, 0x222, 0x0C);
    assert_eq!(
        high.writes,
        stepped(0, &[0x2222, 0x2444, 0x2666, 0x2888, 0x2AAA, 0x2CCC, 0x2EEE])
    );
    assert_eq!(high.frames, 56);
    // A sum with bit $C at once: no store, no frame.
    assert_eq!(
        staff_roll_fade_in(0x0F00, 8, 0x222, 0x0C),
        WordLoop {
            writes: Vec::new(),
            frames: 0,
        }
    );
    // A bit the word cannot reach never ends the loop.
    assert_eq!(staff_roll_fade_in(0, 0, 0x222, 16).frames, u16::MAX);
}

/// Fourteen steps on the stepping frames, the loop spending through the
/// last one's frame: `first + 8 * 13 + 1`.
#[test]
fn the_credits_ramp_spends_through_its_fourteenth_stepping_frame() {
    for (count, first) in [(0u16, 0u16), (3, 5), (8, 0), (0xFFFC, 4), (0xFFFF, 1)] {
        let (at, frames) = credits_ramp_frames(count, 14);
        let expected: Vec<u16> = (0..14).map(|n| first + 8 * n).collect();
        assert_eq!(at, expected, "count {count:#06X}");
        assert_eq!(frames, first + 105, "count {count:#06X}");
    }
    assert_eq!(credits_ramp_frames(5, 0), (Vec::new(), 0));
}

/// Made-up tables: primary words `$100..$115` (22, the deepest read being
/// words 14-21) and secondary `$200..$209` (10, deepest 8-9).
fn tables() -> (Vec<u16>, Vec<u16>) {
    ((0x100..0x116).collect(), (0x200..0x20A).collect())
}

#[test]
fn the_credits_ramp_reads_primary_from_d5_and_secondary_from_step_nine() {
    let (primary, secondary) = tables();
    let rows = credits_ramp_rows(&primary, 8, &secondary, 2, 9, 14).expect("all reads fit");
    assert_eq!(rows.len(), 14);
    // Step 0: d5 = 14, words 14..22; no secondary (d6 - 9 < 0).
    assert_eq!(
        rows[0],
        RampStep {
            primary: (0x10E..0x116).collect(),
            secondary: None,
        }
    );
    // Step 8: d5 = 6; still none.
    assert_eq!(rows[8].primary, (0x106..0x10E).collect::<Vec<_>>());
    assert_eq!(rows[8].secondary, None);
    // Step 9: d5 = 5, and byte 0 of the secondary table.
    assert_eq!(rows[9].primary, (0x105..0x10D).collect::<Vec<_>>());
    assert_eq!(rows[9].secondary, Some(vec![0x200, 0x201]));
    // Step 13: d5 = 1, and byte 16: words 8-9.
    assert_eq!(rows[13].primary, (0x101..0x109).collect::<Vec<_>>());
    assert_eq!(rows[13].secondary, Some(vec![0x208, 0x209]));
    // Each step reads one word earlier, each late step two words on.
    for (d6, row) in rows.iter().enumerate() {
        let d5 = 14 - d6 as u16;
        assert_eq!(row.primary[0], 0x100 + d5, "step {d6}");
        if d6 >= 9 {
            let late = 2 * (d6 as u16 - 9);
            assert_eq!(row.secondary, Some(vec![0x200 + late, 0x201 + late]));
        }
    }
    // A table one word short of a read the cartridge makes.
    assert_eq!(
        credits_ramp_rows(&primary[..21], 8, &secondary, 2, 9, 14),
        None
    );
    assert_eq!(
        credits_ramp_rows(&primary, 8, &secondary[..9], 2, 9, 14),
        None
    );
}

fn presentation(op: &SceneOp) -> Option<PresentationOp> {
    match op {
        SceneOp::Presentation { op } => Some(*op),
        _ => None,
    }
}

/// The staff roll's records in retail order (`$079C8E..$079D0B`): the fade
/// out, 180 frames, the staff-roll music, 180 frames, the fade in, the plane
/// clear and DMA, the sixty-frame restore; then the credits Init.
#[test]
fn the_staff_roll_runs_its_loops_waits_and_music_in_retail_order() {
    let scene = scene_for(EventIndex(0x8021)).expect("the ending is registered");
    let at = scene
        .ops
        .iter()
        .position(|op| {
            matches!(
                presentation(op),
                Some(PresentationOp::EndingStaffRollFadeOut { .. })
            )
        })
        .expect("the staff fade out");
    let expected = [
        SceneOp::Presentation {
            op: PresentationOp::EndingStaffRollFadeOut {
                palette_offset: 0x5E,
                step: 0x222,
            },
        },
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::PlaySound { id: 0xAE },
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::Presentation {
            op: PresentationOp::EndingStaffRollFadeIn {
                palette_offset: 0x5E,
                step: 0x222,
                target_bit: 0x0C,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::ClearPlanes,
        },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::PaletteRestoreTone { frames: 60 },
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::InitVramAndCram,
    ];
    assert_eq!(scene.ops[at..at + expected.len()], expected);
    assert!(matches!(
        presentation(&scene.ops[at + expected.len()]),
        Some(PresentationOp::EndingCreditsAssets { .. })
    ));
    // The ramp's record, then the third tone loop and its wait.
    let ramp = scene
        .ops
        .iter()
        .position(|op| {
            matches!(
                presentation(op),
                Some(PresentationOp::EndingCreditsPaletteRamp { .. })
            )
        })
        .expect("the credits ramp");
    assert_eq!(
        scene.ops[ramp..ramp + 3],
        [
            SceneOp::Presentation {
                op: PresentationOp::EndingCreditsPaletteRamp {
                    primary_rom_addr: 0x0007_A274,
                    primary_ram: 0xFFFF_FB24,
                    primary_words: 8,
                    secondary_rom_addr: 0x0007_A2A0,
                    secondary_ram: 0xFFFF_FB3C,
                    secondary_words: 2,
                    secondary_from: 9,
                    steps: 14,
                },
            },
            SceneOp::Presentation {
                op: PresentationOp::PaletteIncreaseTone {
                    frames: 60,
                    hold: None,
                },
            },
            SceneOp::WaitFrames { frames: 60 },
        ]
    );
}

static FADE_THEN_SOUND: [SceneOp; 3] = [
    SceneOp::Presentation {
        op: PresentationOp::EndingStaffRollFadeOut {
            palette_offset: 0x5E,
            step: 0x222,
        },
    },
    SceneOp::PlaySound { id: 0xAE },
    SceneOp::End,
];

/// Each loop holds the scene until the runtime answers with its frames; the
/// next record runs that many ticks after the loop's own.
#[test]
fn the_ending_loops_hold_the_scene_for_the_frames_the_runtime_answers() {
    let map = map_with(&["........"; 8], vec![], vec![]);
    let mut state = GameState::default();
    let mut runner = SceneRunner::new(&FADE_THEN_SOUND, Vec::new(), StepFrames::new(8).unwrap());
    let first = runner.tick(&map, &mut state, SceneInput::None);
    assert!(matches!(
        first.as_slice(),
        [SceneEffect::Presentation {
            op: SceneOp::Presentation {
                op: PresentationOp::EndingStaffRollFadeOut { .. }
            }
        }]
    ));
    assert!(runner.awaits_palette());
    // Unanswered, the scene stays on the loop.
    let held = runner.tick(&map, &mut state, SceneInput::None);
    assert!(held.is_empty() && runner.awaits_palette());
    runner.release_palette(48);
    let mut sound_at = None;
    for tick in 1..=60 {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::PlaySound { id: 0xAE }
                }
            )
        }) {
            sound_at = Some(tick);
            break;
        }
    }
    assert_eq!(sound_at, Some(48), "48 ticks after the answer");
}
