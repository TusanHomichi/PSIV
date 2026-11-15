//! Where each scene's display is off, read off its ops (#653).
//!
//! The runtime keeps VDP register 1's display bit: a whole-palette fade-out
//! ([`SceneOp::FadeOut`], [`SceneOp::InitVramAndCram`]'s, a
//! [`PresentationOp::VariablePaletteFade`] down) leaves it off, and
//! `VDP_EnableDisplay` ([`SceneOp::FadeIn`], [`SceneOp::FlightFadeIn`], a
//! variable fade up, [`PresentationOp::EnableDisplay`]) turns it on. While it
//! is off the screen is the backdrop alone, so a window drawn then is never
//! seen: a dialogue op met dark means the scene's fades are not its bytes'.
//!
//! The walk follows the ops in order. A `Return` or `End` ends an arm, and
//! the ops after one are another arm (a branch's, a menu's cancel leg),
//! entered lit. An arm that ends dark hands control back dark, unless it is
//! a cutscene's zero return, whose field reload fades in. The runtime turns
//! the display on when a scene ends (`Runtime::end_scene`), so those are
//! listed in [`ENDS_DARK`] with what the cartridge does there.

use super::SCENES;
use crate::{PresentationOp, Scene, SceneOp};

/// Scenes an arm of which ends with the display off, and why.
const ENDS_DARK: &[(&str, &str)] = &[
    (
        "Event_GirlsSneakingOut",
        "retail: PalFadeOut_ClrSpriteTbl, then Game_Mode_Routine $18 \
         (FieldRoutine_Shop) with the windows built dark (ps4.asm:146993-147044)",
    ),
    (
        "Cutscene_Ending",
        "retail: the last Pal_VariableFadeOut ($07A25A), then the game clears",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Display {
    Off,
    On,
}

/// What `op` does to the display, if anything.
fn writes(op: &SceneOp) -> Option<Display> {
    match op {
        SceneOp::FadeOut
        | SceneOp::InitVramAndCram
        | SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade { fade_in: false, .. },
        } => Some(Display::Off),
        SceneOp::FadeIn
        | SceneOp::FlightFadeIn { .. }
        | SceneOp::Presentation {
            op:
                PresentationOp::VariablePaletteFade { fade_in: true, .. }
                | PresentationOp::EnableDisplay,
        } => Some(Display::On),
        _ => None,
    }
}

fn draws_a_window(op: &SceneOp) -> bool {
    matches!(
        op,
        SceneOp::RunDialogue { .. }
            | SceneOp::RunDialogueResume
            | SceneOp::RunDialogueResumeWithWindow { .. }
            | SceneOp::DrawTextToPlane { .. }
    )
}

/// The scene's windows drawn dark, by op index, and whether an arm ends dark.
fn walk(scene: &Scene) -> (Vec<usize>, bool) {
    let cutscene = scene.event.0 & 0x8000 != 0;
    let mut display = Display::On;
    let mut dark_windows = Vec::new();
    let mut ends_dark = false;
    for (index, op) in scene.ops.iter().enumerate() {
        if let Some(written) = writes(op) {
            display = written;
        }
        if display == Display::Off && draws_a_window(op) {
            dark_windows.push(index);
        }
        let ends = matches!(op, SceneOp::Return { .. } | SceneOp::End);
        if ends || index + 1 == scene.ops.len() {
            let reloads = cutscene && matches!(op, SceneOp::Return { value: 0 });
            ends_dark |= display == Display::Off && !reloads;
            display = Display::On;
        }
    }
    (dark_windows, ends_dark)
}

fn problems(scenes: &[Scene], ends_dark: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for scene in scenes {
        let (windows, dark) = walk(scene);
        if !windows.is_empty() {
            problems.push(format!(
                "{}: ops {windows:?} draw a window with the display off",
                scene.name
            ));
        }
        let listed = ends_dark.iter().any(|(name, _)| *name == scene.name);
        if dark != listed {
            problems.push(format!(
                "{}: {} dark, and ENDS_DARK {} it",
                scene.name,
                if dark { "ends" } else { "never ends" },
                if listed { "lists" } else { "does not list" }
            ));
        }
    }
    problems
}

#[test]
fn no_scene_draws_a_window_or_ends_dark_unlisted() {
    let found = problems(SCENES, ENDS_DARK);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// Negative controls: a fade-in dropped before a window, an `EnableDisplay`
/// dropped, and a listed scene that no longer ends dark are each reported.
#[test]
fn a_dark_window_or_a_stale_listing_is_reported() {
    let find = |name: &str| *SCENES.iter().find(|scene| scene.name == name).unwrap();
    let without = |scene: Scene, pick: &dyn Fn(&SceneOp) -> bool| -> Scene {
        let at = scene.ops.iter().rposition(pick).unwrap();
        let mut ops = scene.ops.to_vec();
        ops.remove(at);
        Scene {
            ops: Box::leak(ops.into_boxed_slice()),
            ..scene
        }
    };
    // Lashiec's last fade-in lights its last dialogue.
    let lashiec = without(find("Cutscene_LashiecDefeated"), &|op| {
        *op == SceneOp::FadeIn
    });
    assert!(problems(&[lashiec], ENDS_DARK)[0].contains("draw a window"));
    // The ending's staff text is drawn after its own VDP_EnableDisplay.
    let ending = without(find("Cutscene_Ending"), &|op| {
        matches!(
            op,
            SceneOp::Presentation {
                op: PresentationOp::EnableDisplay
            }
        )
    });
    assert!(problems(&[ending], ENDS_DARK)[0].contains("draw a window"));
    // A listed scene that ends lit is reported, so the list only shrinks.
    let lit = find("Cutscene_Alshline");
    assert_eq!(
        problems(&[lit], &[("Cutscene_Alshline", "")]),
        ["Cutscene_Alshline: never ends dark, and ENDS_DARK lists it"]
    );
    // The guardian events stay lit: their retail tone loops do not clear
    // the display bit. Reintroducing the old fixed fade-out must be caught
    // for either event, independent of changes to the production scene ops.
    for name in ["Event_DeVarsDefeated", "Event_SaLewsDefeated"] {
        let scene = find(name);
        assert_eq!(walk(&scene), (vec![], false), "{name}");
        let mut ops = scene.ops.to_vec();
        ops.push(SceneOp::FadeOut);
        let dark = Scene {
            ops: Box::leak(ops.into_boxed_slice()),
            ..scene
        };
        assert_eq!(
            problems(&[dark], ENDS_DARK),
            [format!("{name}: ends dark, and ENDS_DARK does not list it")]
        );
    }
}
