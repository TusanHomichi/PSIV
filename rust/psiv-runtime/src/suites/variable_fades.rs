//! The variable fades on the pack (`Pal_VariableFadeOut`/`In`, `$05B168`/
//! `$05B1B8`): each blocks its scene for the frames its passes spend on the
//! palette it fades, which `Runtime::cram` holds. Lengths are worked from
//! [`psiv_core::palette`]'s pass counts over the words the trace saw.

use super::palette::{emitted, next_event, pack_present, runtime_at, runtime_on, trace};
use crate::RuntimeEvent;
use psiv_core::{CharId, PresentationOp, SceneOp, palette};

/// `Pal_VariableFadeOut` with `$ED52` = 5 in `Event_Phonon` blocks for its
/// passes: the largest nibble of the palette it fades, six frames each.
#[test]
fn a_variable_fade_blocks_for_the_palette_it_fades() {
    if !pack_present() {
        return;
    }
    let party = [
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(6)),
        None,
        None,
    ];
    let mut runtime = runtime_on(0x0BD, party, &[]);
    let (_, frames) = trace(&mut runtime, 0x6C);
    let fade = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade { fade_in: false, .. }
        )
    });
    let before = frames[fade - 1].words;
    let largest = before
        .iter()
        .flat_map(|word| [word & 0xF, (word >> 4) & 0xF, (word >> 8) & 0xF, word >> 12])
        .max()
        .unwrap()
        .max(1);
    // `$ED52` is 5 here: a pass, then six `VInt_Prepare`s (`loc_5B1A4`).
    let every = 6;
    let spent = usize::from(largest) * every;
    // Each pass writes the buffer before its wait (`ps4.asm:122042-122066`),
    // so the last pass turns it black at the start of its six frames, and
    // the pass before leaves a nibble lit.
    assert_eq!(frames[fade + spent - every].words, [0; 64]);
    assert_ne!(frames[fade + spent - every - 1].words, [0; 64]);
    // The scene's next ops are `wait(40)` (no event) and a sound: the sound
    // lands the fade's frames and forty more after it.
    let next = next_event(&frames, fade, |event| {
        matches!(event, RuntimeEvent::ScenePresentation { .. })
    });
    assert_eq!(next - fade, spent + 40);
}

/// `Cutscene_LutzRevelation` (#610): `Pal_VariableFadeOut` at mode 2
/// (`$0778C6`) takes the Inner Sanctuary to black, three frames a pass;
/// `RefreshMap` loads B1 under it, and `Pal_VariableFadeIn` at mode 2
/// (`$077902`) brings B1's palette up from black.
#[test]
fn lutz_revelation_fades_out_and_up_into_the_next_floor() {
    if !pack_present() {
        return;
    }
    let party = [Some(CharId(0)), Some(CharId(9)), None, None, None];
    let mut runtime = runtime_at(0x16E, (0x1F0, 0x1E0), party, &[]);
    let (_, frames) = trace(&mut runtime, 0x8014);
    let out = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 2
            }
        )
    });
    let lit = frames[out - 1].words;
    assert_ne!(lit, [0; 64]);
    let spent = usize::from(palette::variable_fade_out_frames(&lit, 2));
    assert_ne!(frames[out + spent - 4].words, [0; 64]);
    let fade_in = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 2
            }
        )
    });
    assert!(fade_in > out + spent);
    // The map load's tick stays black too: retail's `RefreshMap` returns with
    // no VBlank before the fade up clears the buffer (#612).
    assert!(
        frames[fade_in - 1]
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::MapChanged { .. })),
        "the load is the tick before the fade up"
    );
    for (at, frame) in frames
        .iter()
        .enumerate()
        .take(fade_in + 1)
        .skip(out + spent - 1)
    {
        assert_eq!(frame.words, [0; 64], "tick {at}: black until the fade up");
    }
    let b1 = frames[fade_in + 100].words;
    assert_ne!(b1, [0; 64]);
    let up = usize::from(palette::variable_fade_in_frames(&b1, 2));
    assert_eq!(frames[fade_in + up - 1].words, b1);
    assert_ne!(frames[fade_in + up - 7].words, b1);
}

/// `Cutscene_ProfoundDarkness` (#610): `Pal_VariableFadeOut` at mode 1
/// (`$078D3E`) saves the palette and takes it to black, two frames a pass;
/// `ClearPlaneABuf` leaves both buffers alone; the 64 words come back from
/// `Palette_Table_Buffer_2`, and `Pal_VariableFadeIn` at mode 7 (`$078E64`)
/// fades up to them, eight frames a pass after its display frame. The stop
/// follows the fade.
#[test]
fn profound_darkness_fades_back_up_to_the_palette_it_left() {
    if !pack_present() {
        return;
    }
    let party = [
        Some(CharId(0)),
        Some(CharId(3)),
        Some(CharId(5)),
        Some(CharId(8)),
        Some(CharId(9)),
    ];
    let mut runtime = runtime_at(0x108, (0x100, 0x160), party, &[]);
    let (_, frames) = trace(&mut runtime, 0x8020);
    let out = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1
            }
        )
    });
    let lit = frames[out - 1].words;
    assert_ne!(lit, [0; 64]);
    let spent = usize::from(palette::variable_fade_out_frames(&lit, 1));
    assert_eq!(frames[out + spent - 1].words, [0; 64]);
    assert_ne!(frames[out + spent - 3].words, [0; 64]);
    let fade_in = emitted(&frames, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 7
            }
        )
    });
    assert_eq!(frames[fade_in].words, [0; 64]);
    let up = usize::from(palette::variable_fade_in_frames(&lit, 7));
    assert_eq!(frames[fade_in + up - 1].words, lit);
    let stop = next_event(&frames, fade_in, |event| {
        matches!(
            event,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::PlaySound { id: 0xFE },
                ..
            }
        )
    });
    assert_eq!(stop - fade_in, up);
}
