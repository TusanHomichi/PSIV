//! The display-enable bit (#653) against the routines' listings: the frame a
//! fade-out's `ClearSpriteTableBuf` makes the first dark one, the frame a
//! `VDP_EnableDisplay` makes the first lit one, and what the screen shows
//! between them.

use psiv_core::{PresentationOp, SceneOp};
use psiv_data::ScenePalettes;

use super::{cram, filled, op};
use crate::cram::Cram;

/// Applies `scene_op` on frame 0, then ticks: whether the display is on
/// after each of `frames` frames.
fn displays(cram: &mut Cram, scene_op: SceneOp, frames: u16) -> (Option<u16>, Vec<bool>) {
    let none = ScenePalettes::default();
    let answered = cram.apply(&scene_op, 0, &none, None, &mut 0);
    let mut on = vec![cram.view().display];
    for frame in 1..frames {
        cram.tick(frame, &mut 0);
        on.push(cram.view().display);
    }
    (answered, on)
}

fn dark(mut cram: Cram) -> Cram {
    cram.view.display = false;
    cram
}

/// `PalFadeOut_ClrSpriteTbl` over `$222`: passes on frames 0 (`$111`) and 1
/// (`$000`, `d7` zero), each with its one `VInt_Prepare`; then
/// `ClearSpriteTableBuf`, so frame 2 is the first with the display off.
#[test]
fn a_fade_out_turns_the_display_off_after_its_last_frame() {
    let mut lit = cram(filled(0x0222), filled(0));
    let (frames, on) = displays(&mut lit, SceneOp::FadeOut, 4);
    assert_eq!(frames, Some(2));
    assert_eq!(on, [true, true, false, false]);
}

/// `Pal_VariableFadeOut` with `$FFFFED52` = 3: the same two passes, four
/// frames each (`dbf d0` from 3), so the display goes off on frame 8, after
/// the last pass's waits, not at the pass.
#[test]
fn a_variable_fade_out_turns_it_off_after_its_waits() {
    let mut lit = cram(filled(0x0222), filled(0));
    let fade = op(PresentationOp::VariablePaletteFade {
        fade_in: false,
        mode: 3,
    });
    let (frames, on) = displays(&mut lit, fade, 10);
    assert_eq!(frames, Some(8));
    assert_eq!(on.iter().position(|on| !on), Some(8));
    assert!(on[8..].iter().all(|on| !on));
}

/// Negative control: `Pal_VariableFadeOut_Lines_1_2` ends in `rts`
/// (`ps4.asm:122147`), not `ClearSpriteTableBuf`, so the display stays on.
#[test]
fn a_two_line_fade_out_leaves_the_display_on() {
    let mut lit = cram(filled(0x0222), filled(0));
    let fade = op(PresentationOp::TwoLinePaletteFade {
        fade_in: false,
        mode: 0,
    });
    let (_, on) = displays(&mut lit, fade, 6);
    assert!(on.iter().all(|on| *on));
}

/// `Pal_FadeIn`, `Pal_VariableFadeIn` and a bare
/// `VDP_EnableDisplay` each spend one `VInt_Prepare` with the bit as it was,
/// then set it: frame 0 dark, frame 1 lit.
#[test]
fn each_display_enable_lights_the_frame_after_its_wait() {
    for enable in [
        SceneOp::FadeIn,
        op(PresentationOp::VariablePaletteFade {
            fade_in: true,
            mode: 2,
        }),
        op(PresentationOp::EnableDisplay),
    ] {
        let mut off = dark(cram(filled(0), filled(0x0222)));
        let (_, on) = displays(&mut off, enable, 3);
        assert_eq!(on, [false, true, true], "{enable:?}");
    }
}

/// Negative control: records that make no `VDP_EnableDisplay` call leave a
/// dark display dark, a palette write among them.
#[test]
fn other_records_leave_the_display_off() {
    for other in [
        SceneOp::DmaPlanes,
        SceneOp::InitVramAndCram,
        op(PresentationOp::SetPaletteWords {
            offset: 33,
            first: 0x0EEE,
            second: 0x0EEE,
        }),
        op(PresentationOp::TwoLinePaletteFade {
            fade_in: true,
            mode: 0,
        }),
    ] {
        let mut off = dark(cram(filled(0), filled(0x0222)));
        let (_, on) = displays(&mut off, other, 3);
        assert_eq!(on, [false, false, false], "{other:?}");
    }
}

/// The issue's case on made-up words: the fade-out ends, a palette is
/// written before the next fade-in, and the screen shows the backdrop alone
/// (word 0, still black) on every frame until the fade-in's wait is over.
#[test]
fn a_palette_written_while_dark_shows_only_the_backdrop() {
    let none = ScenePalettes::default();
    let mut cram = cram(filled(0x0222), filled(0));
    let _ = cram.apply(&SceneOp::FadeOut, 0, &none, None, &mut 0);
    cram.tick(1, &mut 0);
    cram.tick(2, &mut 0);
    let write = op(PresentationOp::SetPaletteWords {
        offset: 33,
        first: 0x0EEE,
        second: 0x00E0,
    });
    let _ = cram.apply(&write, 2, &none, None, &mut 0);
    assert_eq!(cram.view().words[33..35], [0x0EEE, 0x00E0]);
    assert_eq!(cram.view().shown_backdrop(), Some(0));
    cram.tick(3, &mut 0);
    assert_eq!(cram.view().shown_backdrop(), Some(0));
    let _ = cram.apply(&SceneOp::FadeIn, 4, &none, None, &mut 0);
    assert_eq!(cram.view().shown_backdrop(), Some(0), "the fade-in's wait");
    cram.tick(5, &mut 0);
    assert_eq!(cram.view().shown_backdrop(), None);
}

/// A scene's `RefreshMap` keeps the bit and a write still pending; a field
/// load (`enable_display`) sets it and drops the pending write.
#[test]
fn a_refresh_keeps_the_bit_and_a_field_load_sets_it() {
    let none = ScenePalettes::default();
    let mut fading = cram(filled(0x0222), filled(0));
    let _ = fading.apply(&SceneOp::FadeOut, 0, &none, None, &mut 0);
    let mut refreshed = Cram::default();
    refreshed.follow(&fading);
    assert!(refreshed.view().display);
    refreshed.tick(1, &mut 0);
    refreshed.tick(2, &mut 0);
    assert!(!refreshed.view().display, "the fade's write carried over");
    let mut field = Cram::default();
    field.follow(&fading);
    field.enable_display();
    field.tick(1, &mut 0);
    field.tick(2, &mut 0);
    assert!(field.view().display);
}

/// The inn's `Pal_FadeIn` runs after sixty `DMAPlane_A_VInt` frames and
/// its own display wait (`ps4.asm:136436-136453`). The resume takes over
/// pending display writes; neither an already lit scene return nor the
/// scene's delayed display-off may expose or hide the resumed fade.
#[test]
fn the_inn_resume_enables_after_its_hold_and_display_wait() {
    for was_on in [false, true] {
        let mut cram = cram(filled(0), filled(0));
        cram.view.display = was_on;
        cram.display_change = Some((2, false));
        cram.inn_resume_fade_in(filled(0x0222), 0, &mut 0);
        assert!(!cram.view().display);
        for frame in 1..=60 {
            cram.tick(frame, &mut 0);
            assert_eq!(cram.view().shown_backdrop(), Some(0), "frame {frame}");
        }
        cram.tick(61, &mut 0);
        assert!(cram.view().display, "the fade's first lit frame");
        assert_eq!(cram.view().words, filled(0x0111));
        cram.tick(62, &mut 0);
        assert!(cram.view().display);
        assert_eq!(cram.view().words, filled(0x0222));
    }
}
