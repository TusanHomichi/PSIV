//! Timing of the Le Roof scenes' palette loops (#591): `Event_MeetingLeRoof`
//! (`$0048`) and `Cutscene_LeRoofAgain` (`$801C`). Records:
//! `docs/scenes/51_MeetingLeRoof.md`, `52_LeRoofAgain.md`. The palette words
//! the loops write are `psiv-runtime`'s CRAM tests.

mod common;

use common::map_with;
use psiv_core::{
    DialogueId, EventIndex, Flag, GameState, PresentationOp, SceneEffect, SceneInput, SceneOp,
    SceneRunner, StepFrames, scene_for,
};

/// Runs `ops` with no cast, closing each dialogue at once, and writes one
/// mark per effect and, after each tick's marks, `+` when the tick runs map
/// updates and `.` when it does not, runs of either compressed to `+N`/`.N`.
/// A two-line fade's length is the runtime's to measure from the palette;
/// the trace releases it at [`FADE_UP`] frames, the length the runtime
/// gives Le Roof's line (`psiv-runtime`'s palette suite checks the words).
/// Marks: `U` `Pal_VariableFadeToPal2`, `R` the red fade back, `C<n>` a colour loop of `n` passes, `G<n>`
/// the fade-in of `n` passes, `D<id>` a dialogue, `|` a resume, `S<id>` a
/// sound, `O`/`I` `FadeOut`/`FadeIn`, `V` `InitVRAMAndCRAM`, `P` a panel, `M`
/// the map reload, `F` a flag. Stops at the first flag.
fn trace(ops: &'static [SceneOp]) -> String {
    let mut state = GameState::new();
    let map = map_with(&["........"; 8], vec![], vec![]);
    let mut runner = SceneRunner::new(ops, vec![], StepFrames::default());
    let mut marks: Vec<String> = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..5_000 {
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        let mut flag = false;
        for effect in &effects {
            let mark = match effect {
                SceneEffect::DialogueOpen(DialogueId(id)) => {
                    input = SceneInput::DialogueClosed;
                    format!("D{id:X}")
                }
                SceneEffect::DialogueResume => {
                    input = SceneInput::DialogueClosed;
                    "|".to_owned()
                }
                SceneEffect::FlagChanged { .. } => {
                    flag = true;
                    "F".to_owned()
                }
                SceneEffect::Presentation { op } => match op {
                    SceneOp::Presentation { op } => match op {
                        PresentationOp::FadeFromRed { .. } => "R".to_owned(),
                        PresentationOp::TwoLinePaletteFade { fade_in: true, .. } => "U".to_owned(),
                        PresentationOp::CounterPaletteCycle { frames, .. } => format!("C{frames}"),
                        PresentationOp::PaletteGrowIn { passes, .. } => format!("G{passes}"),
                        PresentationOp::ReloadMapChunks => "M".to_owned(),
                        other => panic!("unexpected record {other:?}"),
                    },
                    SceneOp::PlaySound { id } => format!("S{id:02X}"),
                    SceneOp::FadeOut => "O".to_owned(),
                    SceneOp::FadeIn => "I".to_owned(),
                    SceneOp::InitVramAndCram => "V".to_owned(),
                    SceneOp::PanelCreate { .. } => "P".to_owned(),
                    SceneOp::DmaPlanes => continue,
                    other => panic!("unexpected op {other:?}"),
                },
                _ => continue,
            };
            marks.push(mark);
            if flag {
                break;
            }
        }
        if flag {
            break;
        }
        if runner.awaits_palette() {
            runner.release_palette(FADE_UP);
        }
        marks.push(if runner.runs_map_updates() { "+" } else { "." }.to_owned());
    }
    // Compress runs of `+` or `.`.
    let mut out = String::new();
    let mut run: Option<(String, usize)> = None;
    for mark in marks {
        if mark == "+" || mark == "." {
            match &mut run {
                Some((kind, n)) if *kind == mark => *n += 1,
                _ => {
                    if let Some((kind, n)) = run.take() {
                        out.push_str(&format!("{kind}{n} "));
                    }
                    run = Some((mark, 1));
                }
            }
        } else {
            if let Some((kind, n)) = run.take() {
                out.push_str(&format!("{kind}{n} "));
            }
            out.push_str(&mark);
            out.push(' ');
        }
    }
    if let Some((kind, n)) = run {
        out.push_str(&format!("{kind}{n} "));
    }
    out.trim_end().to_owned()
}

/// `Pal_VariableFadeToPal2` from black to Le Roof's line (largest nibble
/// `$E`): fifteen passes, the last changing nothing, of three frames
/// (`$ED52` 2).
const FADE_UP: u16 = 15 * 3;

/// The ops from `first` on: each scene's fade up to Le Roof's palette, past
/// the actor walk the trace has no cast for.
fn from(event: u16, first: usize) -> &'static [SceneOp] {
    let ops = scene_for(EventIndex(event)).expect("registered").ops;
    assert!(
        matches!(
            ops[first],
            SceneOp::Presentation {
                op: PresentationOp::TwoLinePaletteFade {
                    fade_in: true,
                    mode: 2
                }
            }
        ),
        "op {first} of {event:#x} is the fade up"
    );
    &ops[first..]
}

/// `$07052E..`: the fade up to Le Roof's palette, then the 180-pass loop,
/// `bsr loc_705D6` (71), the first 120, dialogue `0`, the second 120, all
/// `VInt_Prepare` only: no tick of the 491 updates the map. Then the stop,
/// the map reload, the tower music and `$D1`.
#[test]
fn meeting_le_roof_runs_its_colour_loops_without_map_updates() {
    let ops = scene_for(EventIndex(0x0048)).unwrap().ops;
    assert!(matches!(ops[9], SceneOp::PlaySound { id: 0xA1 }));
    assert_eq!(
        trace(from(0x0048, 10)),
        "U .45 C180 .180 G71 .71 C120 .120 D0 .1 C120 .120 SFB M S9A F"
    );
}

/// `$0783E2..$07847A`: the same loops (180, `jsr loc_705D6` at `$078406`,
/// 120) before dialogue `$0B`, then after the panel's resume the fade-out and
/// `$07846C`'s `DoMapUpdateLoop` of 60, which does update the map.
#[test]
fn le_roof_again_runs_its_loops_then_a_map_update_wait() {
    let trace = trace(from(0x801C, 9));
    assert!(
        trace.starts_with("U .45 C180 .180 G71 .71 C120 .120 DB .1 V I P | .1 O +60 V I P DC .1 F"),
        "{trace}"
    );
}

/// Negative controls: the transcription before #591, a red fade back,
/// `Wait`s of 180 and 120 with no fade-in and no wait after the fade-out,
/// gives a different trace on every count: an eight-stage red fade, map
/// updates in the colour loops, 71 frames short, and the panels back to
/// back.
#[test]
fn the_old_le_roof_policy_is_rejected() {
    static OLD: &[SceneOp] = &[
        SceneOp::Presentation {
            op: PresentationOp::FadeFromRed { lines: 2 },
        },
        SceneOp::Wait { ticks: 180 },
        SceneOp::Wait { ticks: 120 },
        SceneOp::RunDialogue {
            source: psiv_core::DialogueSource::Entry(DialogueId(0x0B)),
            window: psiv_core::DialogueWindow::Standard,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x11B },
        SceneOp::RunDialogueResumeWithWindow {
            window: psiv_core::DialogueWindow::Cutscene3,
        },
        SceneOp::FadeOut,
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x125 },
        SceneOp::RunDialogue {
            source: psiv_core::DialogueSource::Entry(DialogueId(0x0C)),
            window: psiv_core::DialogueWindow::Cutscene,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xD6),
            value: true,
        },
    ];
    let old = trace(OLD);
    assert_eq!(old, "R .24 +300 DB .1 V I P | .1 O V I P DC .1 F");
    assert!(!trace(from(0x801C, 9)).starts_with(&old));
}

/// The ops that put the galaxy picture on plane A (#594): `NemDecomp` of
/// `loc_1DB2C8` to tile `$260`, `EniDecomp` of `loc_1DDE0A` to `RAM_Start`,
/// then the DMA that shows it, in that order and with nothing drawn between.
/// Both scenes run the same block (`$0704AC..$07051F`, `$07836C..$0783D9`);
/// `Cutscene_LeRoofAgain` once had no `LoadSceneAsset`, so it drew nothing.
fn picture_ops(event: u16) -> Vec<&'static SceneOp> {
    let ops = scene_for(EventIndex(event)).expect("registered").ops;
    let art = ops
        .iter()
        .position(|op| {
            matches!(
                op,
                SceneOp::LoadArt {
                    rom_addr: 0x001D_B2C8,
                    tile: 0x260
                }
            )
        })
        .expect("the picture's art");
    let dma = art
        + ops[art..]
            .iter()
            .position(|op| matches!(op, SceneOp::DmaPlanes))
            .expect("a DMA after the art");
    ops[art..=dma].iter().collect()
}

#[test]
fn both_le_roof_scenes_load_the_picture_before_their_dma() {
    for event in [0x0048, 0x801C] {
        let ops = picture_ops(event);
        let assets: Vec<_> = ops
            .iter()
            .filter_map(|op| match op {
                SceneOp::Presentation {
                    op:
                        PresentationOp::LoadSceneAsset {
                            source_rom_addr,
                            destination_ram,
                        },
                } => Some((*source_rom_addr, *destination_ram)),
                _ => None,
            })
            .collect();
        assert_eq!(assets, [(0x001D_DE0A, 0xFFFF_0000)], "{event:#x}");
        assert!(
            !ops.iter().any(|op| matches!(
                op,
                SceneOp::PanelCreate { .. } | SceneOp::InitVramAndCram | SceneOp::LoadArt { .. }
            ) && !std::ptr::eq(*op, ops[0])),
            "{event:#x}: nothing else draws between the art and the DMA"
        );
    }
    assert_eq!(picture_ops(0x0048), picture_ops(0x801C));
}
