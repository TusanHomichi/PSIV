//! Which one-frame-a-pass palette fades each scene's retail bytes call (#637).
//!
//! Three library routines fade the whole palette one pass a frame, and each
//! has its own op, which the runtime's CRAM model runs with its frames:
//!
//! | Routine | Op |
//! | --- | --- |
//! | `InitVRAMAndCRAM` (`$05A658`) | [`SceneOp::InitVramAndCram`] |
//! | `Pal_FadeIn` (`$0421D4`) | [`SceneOp::FadeIn`] |
//! | `PalFadeOut_ClrSpriteTbl` (`$04223E`) | [`SceneOp::FadeOut`] |
//!
//! A scene that records one where its bytes call none spends frames and
//! draws a fade the cartridge does not, and one it leaves out the reverse.
//! The check reads every `jsr`/`jmp` to them inside a scene's documented
//! ranges off the US image, in address order, and each scene's ops of the
//! three kinds, in op order, must be those calls. It skips without the
//! image. [`KNOWN`] lists the scenes that do not match yet, with the issue
//! that fixes them; a listed scene that matches fails too, so the list only
//! shrinks. The variable fades have their own check, `palette_fades_image`.

use super::SCENES;
use super::dialogue_callers_image::rom;
use super::documented_ranges::{Ranges, documented_ranges};
use crate::SceneOp;

/// A fixed fade routine, by its op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fixed {
    Init,
    In,
    Out,
}

/// The routines' US entry points.
const ROUTINES: &[(u32, Fixed)] = &[
    (0x05_A658, Fixed::Init),
    (0x04_21D4, Fixed::In),
    (0x04_223E, Fixed::Out),
];

/// Scenes whose ops do not match their calls yet, and the issue that fixes
/// each.
const KNOWN: &[(&str, u32)] = &[
    ("Cutscene_Dorin", 652),
    ("Cutscene_AlysWounded", 652),
    ("Cutscene_ZioDefeated", 652),
    ("Cutscene_InsideSpaceship", 652),
    ("Cutscene_SpaceshipSabotage", 652),
    ("Cutscene_CrashLaanding", 652),
    ("Cutscene_DarkForce1Defeated", 652),
    ("Cutscene_LeRoofAgain", 652),
    ("Cutscene_FindingAirCastle", 652),
    ("Cutscene_LashiecDefeated", 652),
    ("Event_DeVarsDefeated", 652),
    ("Event_SaLewsDefeated", 652),
    ("Cutscene_Ending", 652),
];

/// The fixed fade an op is, if it is one.
fn recorded(op: &SceneOp) -> Option<Fixed> {
    match op {
        SceneOp::InitVramAndCram => Some(Fixed::Init),
        SceneOp::FadeIn => Some(Fixed::In),
        SceneOp::FadeOut => Some(Fixed::Out),
        _ => None,
    }
}

/// The calls in `ranges` of the image, in address order: every `jsr
/// (routine).l` and `jmp (routine).l`, with its address.
fn retail_calls(rom: &[u8], ranges: &[(usize, usize)]) -> Vec<(usize, Fixed)> {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable();
    ranges.dedup();
    let mut calls = Vec::new();
    for (start, end) in ranges {
        for at in (start..end.min(rom.len().saturating_sub(5))).step_by(2) {
            let bytes = &rom[at..at + 6];
            if bytes[..2] == [0x4E, 0xB9] || bytes[..2] == [0x4E, 0xF9] {
                let target = u32::from_be_bytes(bytes[2..].try_into().unwrap());
                if let Some(&(_, fixed)) = ROUTINES.iter().find(|(addr, _)| *addr == target) {
                    calls.push((at, fixed));
                }
            }
        }
    }
    calls
}

/// Each scene with documented ranges whose ops are not its calls, by name.
fn mismatches(rom: &[u8], ranges: &Ranges, scenes: &[crate::Scene]) -> Vec<(&'static str, String)> {
    let mut problems = Vec::new();
    for scene in scenes {
        let Some(ranges) = ranges.get(scene.name) else {
            continue;
        };
        let native: Vec<Fixed> = scene.ops.iter().filter_map(recorded).collect();
        let calls = retail_calls(rom, ranges);
        if calls
            .iter()
            .map(|(_, fixed)| *fixed)
            .ne(native.iter().copied())
        {
            let at: Vec<String> = calls
                .iter()
                .map(|(at, fixed)| format!("${at:06X} {fixed:?}"))
                .collect();
            problems.push((
                scene.name,
                format!(
                    "{}: retail calls [{}], the scene records {native:?}",
                    scene.name,
                    at.join(", ")
                ),
            ));
        }
    }
    problems
}

/// Every mismatch `known` does not list, and every scene `known` lists that
/// matches.
fn report(
    rom: &[u8],
    ranges: &Ranges,
    scenes: &[crate::Scene],
    known: &[(&str, u32)],
) -> Vec<String> {
    let found = mismatches(rom, ranges, scenes);
    let mut problems: Vec<String> = found
        .iter()
        .filter(|(name, _)| !known.iter().any(|(known, _)| known == name))
        .map(|(_, problem)| problem.clone())
        .collect();
    for (known, issue) in known {
        if scenes.iter().any(|scene| scene.name == *known)
            && !found.iter().any(|(name, _)| name == known)
        {
            problems.push(format!(
                "{known} now records the fades its bytes call: remove it from KNOWN (#{issue})"
            ));
        }
    }
    problems
}

#[test]
fn every_scene_records_the_fixed_fades_its_bytes_call() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = report(rom, &documented_ranges(), SCENES, KNOWN);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Negative control: an exception whose scene already matches is reported,
/// so the list only shrinks.
#[test]
fn a_listed_scene_that_matches_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let scene = SCENES
        .iter()
        .find(|scene| scene.name == "Cutscene_ProfoundDarkness")
        .unwrap();
    assert_eq!(
        report(
            rom,
            &ranges,
            std::slice::from_ref(scene),
            &[("Cutscene_ProfoundDarkness", 1)]
        ),
        [
            "Cutscene_ProfoundDarkness now records the fades its bytes call: remove it from KNOWN (#1)"
        ]
    );
}

/// Negative controls: in every matching scene that records them, each op of
/// the three dropped, swapped for each of the other two, or doubled is
/// reported.
#[test]
fn a_wrong_fixed_fade_op_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let kinds = [SceneOp::InitVramAndCram, SceneOp::FadeIn, SceneOp::FadeOut];
    let mut altered_count = 0;
    for scene in SCENES {
        if KNOWN.iter().any(|(name, _)| *name == scene.name) || !ranges.contains_key(scene.name) {
            continue;
        }
        for (index, op) in scene.ops.iter().enumerate() {
            if recorded(op).is_none() {
                continue;
            }
            let mut variants: Vec<Vec<SceneOp>> = Vec::new();
            let mut dropped = scene.ops.to_vec();
            dropped.remove(index);
            variants.push(dropped);
            let mut doubled = scene.ops.to_vec();
            doubled.insert(index, *op);
            variants.push(doubled);
            for other in kinds.iter().filter(|kind| recorded(kind) != recorded(op)) {
                let mut swapped = scene.ops.to_vec();
                swapped[index] = *other;
                variants.push(swapped);
            }
            for ops in variants {
                let altered = crate::Scene {
                    ops: Box::leak(ops.into_boxed_slice()),
                    ..*scene
                };
                assert_eq!(
                    mismatches(rom, &ranges, &[altered]).len(),
                    1,
                    "{} op {index} altered passed",
                    scene.name
                );
                altered_count += 1;
            }
        }
    }
    assert!(
        altered_count > 100,
        "only {altered_count} alterations checked"
    );
}
