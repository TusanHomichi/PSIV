//! #659: fixed fades at their panel, window and credits boundaries.
use super::ending::ENDING;
use crate::{DialogueId, DialogueSource, DialogueWindow, PresentationOp, SceneOp};

/// Check ordered neighborhoods, not just the fade count. A census alone
/// cannot detect a fade moved across a panel, window or credits transition.
fn problems(ops: &[SceneOp]) -> Vec<&'static str> {
    use SceneOp::*;
    let mut problems = Vec::new();
    let mut check = |name, sequence: &[SceneOp]| {
        if !ops.windows(sequence.len()).any(|w| w == sequence) {
            problems.push(name);
        }
    };
    check(
        "entry fade before the retained intro window",
        &[
            PlaySound { id: 0xAD },
            InitVramAndCram,
            FadeIn,
            SetRenderSpritesInCutscene { enabled: true },
            RunDialogue {
                source: DialogueSource::Entry(DialogueId(9)),
                window: DialogueWindow::EndingIntro,
            },
        ],
    );
    check(
        "intro teardown before the next Init",
        &[
            Presentation {
                op: PresentationOp::WindowDestroy { render_mode: 2 },
            },
            Presentation {
                op: PresentationOp::ReloadMapChunks,
            },
            WaitFrames { frames: 120 },
            InitVramAndCram,
            PlaySound { id: 0xAF },
            FadeIn,
            PanelCreate { id: 0x147 },
        ],
    );
    check(
        "next panel uploaded before fade-in",
        &[
            RunDialogue {
                source: DialogueSource::Entry(DialogueId(0x0A)),
                window: DialogueWindow::Ending,
            },
            InitVramAndCram,
            PanelDestroyAll,
            WaitFrames { frames: 120 },
            PanelCreate { id: 0x14C },
            DmaPlanes,
            FadeIn,
            WaitFrames { frames: 120 },
            PanelCreate { id: 0x14D },
        ],
    );
    check(
        "final Start still gates clearing the game",
        &[WaitForStart, MarkGameCleared],
    );
    let fourth = ops.iter().position(|op| {
        matches!(
            op,
            Presentation {
                op: PresentationOp::DmaPlanesLoop { frames: 60 }
            }
        )
    });
    if !fourth.is_some_and(|i| {
        matches!(
            ops.get(i + 1..i + 5),
            Some([
                InitVramAndCram,
                Presentation {
                    op: PresentationOp::PaletteCopy { .. }
                },
                PanelCreate { id: 0x171 },
                DmaPlanes,
            ])
        )
    }) {
        problems.push("panel 171 follows the fourth Init");
    }
    let credits = ops.iter().position(|op| {
        matches!(
            op,
            Presentation {
                op: PresentationOp::EndingCreditsAssets { .. }
            }
        )
    });
    if !credits.is_some_and(|i| {
        matches!(
            ops.get(i.saturating_sub(3)..i + 3),
            Some([
                Presentation {
                    op: PresentationOp::PaletteRestoreTone { frames: 60 }
                },
                WaitFrames { frames: 60 },
                InitVramAndCram,
                Presentation {
                    op: PresentationOp::EndingCreditsAssets { .. }
                },
                FadeIn,
                Presentation {
                    op: PresentationOp::EndingCreditsStage { stage: 1, .. }
                },
            ])
        )
    }) {
        problems.push("credits Init after restore, fade-in after assets and before stage 1");
    }
    problems
}

#[test]
fn ending_fixed_fades_stay_at_their_retail_boundaries() {
    assert_eq!(problems(ENDING.ops), Vec::<&str>::new());
}

#[test]
fn reverted_or_misplaced_credits_fades_fail_the_boundary_check() {
    let assets = ENDING
        .ops
        .iter()
        .position(|op| {
            matches!(
                op,
                SceneOp::Presentation {
                    op: PresentationOp::EndingCreditsAssets { .. }
                }
            )
        })
        .unwrap();
    let mut reverted = ENDING.ops.to_vec();
    reverted.remove(assets + 1); // the credits fade-in
    reverted.remove(assets - 1); // its Init
    assert_eq!(
        problems(&reverted),
        ["credits Init after restore, fade-in after assets and before stage 1"]
    );
    // Moving either fade preserves the census but violates the boundary.
    for at in [assets - 1, assets + 1] {
        let mut moved = ENDING.ops.to_vec();
        moved.swap(at, at + 1);
        assert_eq!(
            problems(&moved),
            ["credits Init after restore, fade-in after assets and before stage 1"]
        );
    }
}
