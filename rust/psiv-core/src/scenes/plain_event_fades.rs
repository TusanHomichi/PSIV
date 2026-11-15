//! A plain event leaves the field its palette.
//!
//! An event below `$8000` returns straight to the field: the cutscene
//! return's reload and its `Pal_FadeIn` run only for `$8000` and up. Since
//! #637 a [`SceneOp::FadeOut`] or [`SceneOp::InitVramAndCram`] takes the CRAM
//! model to black, so a plain event that fades out and never fades back in
//! hands the player a black field. Each one's last fade-out must have a
//! [`SceneOp::FadeIn`] after it in op order, unless [`CONTINUED`] names the
//! caller that fades the palette back in. The check reads op order, not
//! branches: a scene that fades in on one arm only passes it.

use super::SCENES;
use crate::SceneOp;

/// Plain events whose caller fades the palette back in after them, and where.
const CONTINUED: &[(&str, &str)] = &[(
    "Event_GirlsSneakingOut",
    "the Aiedo inn's resume, `Session::finish_aiedo_stay` (`ps4.asm:136436-136453`)",
)];

/// Whether `ops` end with the palette faded out: some fade-out with no
/// fade-in after it.
fn ends_dark(ops: &[SceneOp]) -> bool {
    let last_out = ops
        .iter()
        .rposition(|op| matches!(op, SceneOp::FadeOut | SceneOp::InitVramAndCram));
    let last_in = ops.iter().rposition(|op| matches!(op, SceneOp::FadeIn));
    match (last_out, last_in) {
        (Some(out), Some(fade_in)) => fade_in < out,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// The plain events in `scenes` that end dark and `continued` does not list,
/// and every listed one that does not end dark.
fn report(scenes: &[crate::Scene], continued: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for scene in scenes.iter().filter(|scene| !scene.event.is_cutscene()) {
        let listed = continued.iter().any(|(name, _)| *name == scene.name);
        match (ends_dark(scene.ops), listed) {
            (true, false) => problems.push(format!(
                "{} fades out and returns to the field without fading back in",
                scene.name
            )),
            (false, true) => problems.push(format!(
                "{} fades back in itself: remove it from CONTINUED",
                scene.name
            )),
            _ => {}
        }
    }
    problems
}

#[test]
fn every_plain_event_leaves_the_field_its_palette() {
    let problems = report(SCENES, CONTINUED);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Negative controls: `Event_DeVarsDefeated` with the `FadeOut` it once
/// recorded, and every plain event that fades back in with a `FadeOut` after
/// its last op, are reported; a listed scene that fades back in is too.
#[test]
fn a_plain_event_left_dark_is_reported() {
    let find = |name: &str| SCENES.iter().find(|scene| scene.name == name).unwrap();
    let de_vars = find("Event_DeVarsDefeated");
    let mut ops = de_vars.ops.to_vec();
    ops.insert(1, SceneOp::FadeOut);
    let altered = crate::Scene {
        ops: Box::leak(ops.into_boxed_slice()),
        ..*de_vars
    };
    assert_eq!(
        report(std::slice::from_ref(&altered), &[]),
        ["Event_DeVarsDefeated fades out and returns to the field without fading back in"]
    );

    let mut checked = 0;
    for scene in SCENES.iter().filter(|scene| !scene.event.is_cutscene()) {
        if !scene.ops.iter().any(|op| matches!(op, SceneOp::FadeIn)) {
            continue;
        }
        let mut ops = scene.ops.to_vec();
        ops.push(SceneOp::FadeOut);
        let altered = crate::Scene {
            ops: Box::leak(ops.into_boxed_slice()),
            ..*scene
        };
        let listed: Vec<_> = CONTINUED
            .iter()
            .copied()
            .filter(|(name, _)| *name != scene.name)
            .collect();
        assert_eq!(report(&[altered], &listed).len(), 1, "{}", scene.name);
        checked += 1;
    }
    assert!(checked > 10, "only {checked} plain events checked");

    let elevator = find("Event_RidingElevator");
    assert_eq!(
        report(
            std::slice::from_ref(elevator),
            &[("Event_RidingElevator", "nowhere")]
        ),
        ["Event_RidingElevator fades back in itself: remove it from CONTINUED"]
    );
}
