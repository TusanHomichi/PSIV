//! When the Le Roof scenes reach their retail calls (#595): counted frames
//! from the `Event_Index` write to each fade, `DMAPlanes_VInt`, the first
//! colour-loop pass and the fade-in, and the 84-frame `Pal_VariableFadeOut`
//! at `$07845A`. The loops' palette words are `palette.rs`'s Le Roof tests.

use super::palette::{LeRoofTick, le_roof_emitted, le_roof_run, le_roof_run_from, pack_present};
use crate::RuntimeEvent;
use psiv_core::{Flag, PresentationOp, SceneOp};

/// The ticks the fade-out, `DmaPlanes`, the fade up, the first colour loop
/// and the fade-in are first emitted on, counted from the event's start.
/// `ticks[0]` is the frame the mode dispatcher spends entering event mode,
/// retail's frame that writes `Event_Index` (tape 02 measures the boundary,
/// `Runtime::scene_warmup`), so `ticks[n]` comes after `n` counted frames,
/// as retail's code does after `n` `VInt_Prepare` calls from that write.
fn le_roof_marks(ticks: &[LeRoofTick]) -> [usize; 5] {
    let fade_out = le_roof_emitted(ticks, 0, |op| {
        matches!(
            op,
            PresentationOp::TwoLinePaletteFade { fade_in: false, .. }
        )
    });
    let dma = fade_out
        + ticks[fade_out..]
            .iter()
            .position(|tick| {
                tick.events.iter().any(|event| {
                    matches!(
                        event,
                        RuntimeEvent::ScenePresentation {
                            op: SceneOp::DmaPlanes,
                            ..
                        }
                    )
                })
            })
            .expect("DmaPlanes");
    let fade_up = le_roof_emitted(ticks, dma, |op| {
        matches!(op, PresentationOp::TwoLinePaletteFade { fade_in: true, .. })
    });
    let first = le_roof_emitted(ticks, fade_up, |op| {
        matches!(op, PresentationOp::CounterPaletteCycle { frames: 180, .. })
    });
    let grow = le_roof_emitted(ticks, first, |op| {
        matches!(op, PresentationOp::PaletteGrowIn { .. })
    });
    [fade_out, dma, fade_up, first, grow]
}

/// Acceptance for #595: from the retail capture's start, each scene enters
/// its fade-out, its `DMAPlanes_VInt`, its fade up, its first colour-loop
/// pass and its fade-in after as many frames as retail counts from the
/// `Event_Index` write. Retail (oracle PC trace, tape 28's schedule, the
/// `VInt_Prepare` calls before each entry): `Event_MeetingLeRoof` one frame
/// to `Event_MoveCharacter`, 16 of walk, 60, 42 of fade, 60, then
/// `DMAPlanes_VInt` at 179 and `Pal_VariableFadeToPal2` at 180, the loop's
/// first pass at 225 and `loc_705D6` at 405; `Cutscene_LeRoofAgain` 8 of
/// walk, so 9, 111, 112, 157 and 337. Negative control: from #591's start,
/// one row nearer, `Event_MeetingLeRoof` arrives 8 frames early and
/// `Cutscene_LeRoofAgain`, with no walk left, 7.
#[test]
fn le_roof_scenes_reach_their_loops_on_the_retail_frame() {
    if !pack_present() {
        return;
    }
    let none = |_: &RuntimeEvent| false;
    let meeting = le_roof_marks(&le_roof_run(0x0048, &[], none));
    assert_eq!(meeting, [77, 179, 180, 225, 405]);
    let flags = [Flag::event(0xD5), Flag::event(0xD3)];
    let grown = |event: &RuntimeEvent| {
        matches!(
            event,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::Presentation {
                    op: PresentationOp::PaletteGrowIn { .. }
                },
                ..
            }
        )
    };
    let again = le_roof_marks(&le_roof_run(0x801C, &flags, grown));
    assert_eq!(again, [9, 111, 112, 157, 337]);

    let near = (0x1F0, 0x200);
    let meeting = le_roof_marks(&le_roof_run_from(near, 0x0048, &[], none));
    assert_eq!(meeting[3..], [217, 397]);
    let again = le_roof_marks(&le_roof_run_from(near, 0x801C, &flags, grown));
    assert_eq!(again[3..], [150, 330]);
}

/// `$07845A`: `Cutscene_LeRoofAgain` fades panel `$11B` out with
/// `Pal_VariableFadeOut` at `$ED52` 5, which the retail capture spends 84
/// counted frames on (fourteen passes of six) before `$07846C`'s
/// `DoMapUpdateLoop`. The runtime measures the same from the panel's
/// palette and leaves it black.
#[test]
fn le_roof_again_fades_its_panel_out_over_the_retail_frames() {
    if !pack_present() {
        return;
    }
    let flags = [Flag::event(0xD5), Flag::event(0xD3)];
    let ticks = le_roof_run(0x801C, &flags, |event| {
        matches!(
            event,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::PanelCreate { id: 0x125 },
                ..
            }
        )
    });
    let fade = le_roof_emitted(&ticks, 0, |op| {
        matches!(
            op,
            PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 5
            }
        )
    });
    let reinit = (fade..ticks.len())
        .find(|&at| {
            ticks[at].events.iter().any(|event| {
                matches!(
                    event,
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::InitVramAndCram,
                        ..
                    }
                )
            })
        })
        .expect("the second InitVRAMAndCRAM");
    assert_eq!(reinit - fade, 84 + 60);
    assert!(
        ticks[fade].words.iter().any(|&word| word != 0),
        "lit at the start"
    );
    assert_eq!(ticks[fade + 83].words, [0; 64], "black on the last pass");
}
