//! Which variable palette fade each scene's retail bytes call (#596).
//!
//! Six library routines fade the palette in passes of `$FFFFED52 + 1`
//! frames, and each has its own record:
//!
//! | Routine | Record |
//! | --- | --- |
//! | `Pal_VariableFadeOut` (`$05B168`) | [`PresentationOp::VariablePaletteFade`], `fade_in: false` |
//! | `Pal_VariableFadeIn` (`$05B1B8`) | [`PresentationOp::VariablePaletteFade`], `fade_in: true` |
//! | `Pal_VariableFadeOut_Lines_1_2` (`$05B222`) | [`PresentationOp::TwoLinePaletteFade`], `fade_in: false` |
//! | `Pal_VariableFadeToPal2` (`$05B26E`) | [`PresentationOp::TwoLinePaletteFade`], `fade_in: true` |
//! | `Pal_VariableFadeToRed` (`$05B2BE`) | [`PresentationOp::FadeToRed`] |
//! | `Pal_VariableFadeFromRed` (`$05B30E`) | [`PresentationOp::FadeFromRed`] |
//!
//! They differ in span (64 or 32 words), direction and termination, so one
//! transcribed as another fades the wrong words for the wrong number of
//! frames. [`CALLS`] lists every `jsr` to them inside a scene's documented
//! ranges, with the `move.b #mode, ($FFFFED52).w` the bytes set before it.
//! The image check reads the table off the US image, and skips without it.
//! The table check needs no image: each scene's fade records, in op order,
//! must be its calls. [`KNOWN`] lists the scenes that do not match yet,
//! with the issue that fixes them. A listed scene that matches fails too,
//! so the list only shrinks.

use super::SCENES;
use super::dialogue_callers_image::rom;
use super::documented_ranges::documented_ranges;
use crate::{PresentationOp, SceneOp};

/// A variable fade routine, by what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fade {
    /// `Pal_VariableFadeOut` / `Pal_VariableFadeIn`: all 64 words.
    Variable { fade_in: bool },
    /// `Pal_VariableFadeOut_Lines_1_2` / `Pal_VariableFadeToPal2`: words 0-31.
    TwoLine { fade_in: bool },
    /// `Pal_VariableFadeToRed`.
    ToRed,
    /// `Pal_VariableFadeFromRed`.
    FromRed,
}

const VAR_OUT: Fade = Fade::Variable { fade_in: false };
const VAR_IN: Fade = Fade::Variable { fade_in: true };
const TWO_OUT: Fade = Fade::TwoLine { fade_in: false };
const TWO_IN: Fade = Fade::TwoLine { fade_in: true };

/// The routines' US entry points.
const ROUTINES: &[(u32, Fade)] = &[
    (0x05_B168, VAR_OUT),
    (0x05_B1B8, VAR_IN),
    (0x05_B222, TWO_OUT),
    (0x05_B26E, TWO_IN),
    (0x05_B2BE, Fade::ToRed),
    (0x05_B30E, Fade::FromRed),
];

/// One fade call in a scene's bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Call {
    scene: &'static str,
    /// The `jsr`.
    at: u32,
    fade: Fade,
    /// The `$FFFFED52` value set before it.
    mode: u8,
}

const fn call(scene: &'static str, at: u32, fade: Fade, mode: u8) -> Call {
    Call {
        scene,
        at,
        fade,
        mode,
    }
}

/// Every fade call in the scenes' documented bytes, by scene in registry
/// order, then by address.
const CALLS: &[Call] = &[
    call("Event_BioPlantAlarm", 0x06_C28A, Fade::ToRed, 3),
    call("Event_BioPlantAlarm", 0x06_C296, Fade::FromRed, 3),
    call("Cutscene_AlysWounded", 0x07_4BD6, VAR_OUT, 3),
    call("Cutscene_PsycoWand", 0x07_5428, VAR_OUT, 5),
    call("Cutscene_PsycoWand", 0x07_5500, VAR_OUT, 9),
    call("Cutscene_PsycoWand", 0x07_55D2, VAR_OUT, 9),
    call("Cutscene_PsycoWand", 0x07_5772, VAR_OUT, 9),
    call("Cutscene_CrashLaanding", 0x07_6710, VAR_OUT, 9),
    call("Event_VahalFortTerminal", 0x06_CDE2, Fade::ToRed, 3),
    call("Event_VahalFortTerminal", 0x06_CDF6, Fade::FromRed, 3),
    call("Event_WeaponPlantTerminal", 0x06_CE26, Fade::ToRed, 3),
    call("Event_WeaponPlantTerminal", 0x06_CE3A, Fade::FromRed, 3),
    call("Event_Juza", 0x06_FB3A, TWO_OUT, 2),
    call("Event_MeetingLeRoof", 0x07_049E, TWO_OUT, 2),
    call("Event_MeetingLeRoof", 0x07_052E, TWO_IN, 2),
    call("Event_MeetingLeRoof", 0x07_0590, TWO_OUT, 2),
    call("Event_MeetingLeRoof", 0x07_05BE, TWO_IN, 2),
    call("Cutscene_LeRoofAgain", 0x07_835E, TWO_OUT, 2),
    call("Cutscene_LeRoofAgain", 0x07_83E8, TWO_IN, 2),
    call("Cutscene_LeRoofAgain", 0x07_8460, VAR_OUT, 5),
    call("Cutscene_LutzRevelation", 0x07_78C6, VAR_OUT, 2),
    call("Cutscene_LutzRevelation", 0x07_7902, VAR_IN, 2),
    call("Event_ReFaze", 0x07_15DA, TWO_OUT, 5),
    call("Event_ReFaze", 0x07_172C, TWO_IN, 2),
    call("Cutscene_ProfoundDarkness", 0x07_8D3E, VAR_OUT, 1),
    call("Cutscene_ProfoundDarkness", 0x07_8E64, VAR_IN, 7),
    call("Cutscene_Ending", 0x07_9856, VAR_OUT, 7),
    call("Cutscene_Ending", 0x07_989E, VAR_IN, 1),
    call("Cutscene_Ending", 0x07_98C2, VAR_OUT, 1),
    call("Cutscene_Ending", 0x07_9918, VAR_IN, 1),
    call("Cutscene_Ending", 0x07_9942, VAR_OUT, 1),
    call("Cutscene_Ending", 0x07_9998, VAR_IN, 1),
    call("Cutscene_Ending", 0x07_99C2, VAR_OUT, 1),
    call("Cutscene_Ending", 0x07_9A18, VAR_IN, 1),
    call("Cutscene_Ending", 0x07_9A42, VAR_OUT, 1),
    call("Cutscene_Ending", 0x07_9A98, VAR_IN, 1),
    call("Cutscene_Ending", 0x07_9AC2, VAR_OUT, 1),
    call("Cutscene_Ending", 0x07_9B1E, VAR_IN, 1),
    call("Cutscene_Ending", 0x07_9B48, VAR_OUT, 1),
    call("Cutscene_Ending", 0x07_9B9E, VAR_IN, 5),
    call("Cutscene_Ending", 0x07_9BBE, VAR_OUT, 15),
    call("Cutscene_Ending", 0x07_A25A, VAR_OUT, 3),
    call("Event_Phonon", 0x07_232C, VAR_OUT, 5),
    call("Event_Phonon", 0x07_23B6, VAR_IN, 2),
    call("Event_Phonon", 0x07_23C2, VAR_OUT, 0),
    call("Event_Phonon", 0x07_23E4, VAR_IN, 0),
];

/// Scenes whose fade records do not match their calls yet, and the issue
/// that fixes each.
const KNOWN: &[(&str, u32)] = &[];

/// The fade record an op is, and its mode, if it is one.
fn recorded(op: &SceneOp) -> Option<(Fade, u8)> {
    let SceneOp::Presentation { op } = op else {
        return None;
    };
    Some(match *op {
        PresentationOp::VariablePaletteFade { fade_in, mode } => (Fade::Variable { fade_in }, mode),
        PresentationOp::TwoLinePaletteFade { fade_in, mode } => (Fade::TwoLine { fade_in }, mode),
        PresentationOp::FadeToRed { lines } => (Fade::ToRed, lines),
        PresentationOp::FadeFromRed { lines } => (Fade::FromRed, lines),
        _ => return None,
    })
}

/// Each scene whose fade records are not its [`CALLS`], by name.
fn mismatches(scenes: &[crate::Scene]) -> Vec<(&'static str, String)> {
    let mut problems = Vec::new();
    for scene in scenes {
        let native: Vec<(Fade, u8)> = scene.ops.iter().filter_map(recorded).collect();
        let calls: Vec<&Call> = CALLS.iter().filter(|c| c.scene == scene.name).collect();
        let expected: Vec<(Fade, u8)> = calls.iter().map(|c| (c.fade, c.mode)).collect();
        if native != expected {
            let at: Vec<String> = calls
                .iter()
                .map(|c| format!("${:06X} {:?} mode {}", c.at, c.fade, c.mode))
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
fn report(scenes: &[crate::Scene], known: &[(&str, u32)]) -> Vec<String> {
    let found = mismatches(scenes);
    let mut problems: Vec<String> = found
        .iter()
        .filter(|(name, _)| !known.iter().any(|(known, _)| known == name))
        .map(|(_, problem)| problem.clone())
        .collect();
    for (known, issue) in known {
        if !found.iter().any(|(name, _)| name == known) {
            problems.push(format!(
                "{known} now records the fades its bytes call: remove it from KNOWN (#{issue})"
            ));
        }
    }
    problems
}

#[test]
fn every_scene_records_the_fades_its_bytes_call() {
    let problems = report(SCENES, KNOWN);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Negative control: an exception whose scene already matches is reported,
/// so the list only shrinks.
#[test]
fn a_listed_scene_that_matches_is_reported() {
    assert_eq!(
        report(SCENES, &[("Cutscene_AlysWounded", 610)]),
        ["Cutscene_AlysWounded now records the fades its bytes call: remove it from KNOWN (#610)"]
    );
}

/// The fade calls in `ranges` of the image, in address order: every
/// `jsr (routine).l`, with the mode of the last `move.b #n, ($FFFFED52).w`
/// before it in the same range.
fn retail_calls(rom: &[u8], scene: &'static str, ranges: &[(usize, usize)]) -> Vec<Call> {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable();
    ranges.dedup();
    let mut calls = Vec::new();
    for (start, end) in ranges {
        let mut mode = None;
        for at in (start..end.min(rom.len().saturating_sub(5))).step_by(2) {
            let bytes = &rom[at..at + 6];
            // move.b #n, ($FFFFED52).w
            if bytes[..3] == [0x11, 0xFC, 0x00] && bytes[4..] == [0xED, 0x52] {
                mode = Some(bytes[3]);
            }
            if bytes[..2] == [0x4E, 0xB9] {
                let target = u32::from_be_bytes(bytes[2..].try_into().unwrap());
                if let Some(&(_, fade)) = ROUTINES.iter().find(|(addr, _)| *addr == target) {
                    let mode = mode
                        .unwrap_or_else(|| panic!("{scene}: ${at:06X} sets no $FFFFED52 first"));
                    calls.push(call(scene, at as u32, fade, mode));
                }
            }
        }
    }
    calls
}

#[test]
fn the_calls_are_every_fade_in_the_scenes_bytes() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let mut read = Vec::new();
    for scene in SCENES {
        if let Some(ranges) = ranges.get(scene.name) {
            read.extend(retail_calls(rom, scene.name, ranges));
        }
    }
    let missing: Vec<String> = read
        .iter()
        .filter(|call| !CALLS.contains(call))
        .map(|call| format!("missing from CALLS: {call:?}"))
        .collect();
    let extra: Vec<String> = CALLS
        .iter()
        .filter(|call| !read.contains(call))
        .map(|call| format!("not in the image: {call:?}"))
        .collect();
    let problems = [missing, extra].concat();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert_eq!(CALLS, read.as_slice(), "CALLS is out of order");
}

/// The scenes #596 and #610 fixed, as each was or could be mistranscribed:
/// every fade record swapped for a red fade, for the other span, for the
/// other direction, or given another mode, is reported.
#[test]
fn a_wrong_fade_record_is_reported() {
    let swaps: [fn(PresentationOp) -> Option<PresentationOp>; 4] = [
        |op| match op {
            PresentationOp::TwoLinePaletteFade { fade_in, mode }
            | PresentationOp::VariablePaletteFade { fade_in, mode } => Some(if fade_in {
                PresentationOp::FadeFromRed { lines: mode }
            } else {
                PresentationOp::FadeToRed { lines: mode }
            }),
            _ => None,
        },
        |op| match op {
            PresentationOp::TwoLinePaletteFade { fade_in, mode } => {
                Some(PresentationOp::VariablePaletteFade { fade_in, mode })
            }
            PresentationOp::VariablePaletteFade { fade_in, mode } => {
                Some(PresentationOp::TwoLinePaletteFade { fade_in, mode })
            }
            _ => None,
        },
        |op| match op {
            PresentationOp::TwoLinePaletteFade { fade_in, mode } => {
                Some(PresentationOp::TwoLinePaletteFade {
                    fade_in: !fade_in,
                    mode,
                })
            }
            _ => None,
        },
        |op| match op {
            PresentationOp::TwoLinePaletteFade { fade_in, mode } => {
                Some(PresentationOp::TwoLinePaletteFade {
                    fade_in,
                    mode: mode ^ 1,
                })
            }
            PresentationOp::VariablePaletteFade { fade_in, mode } => {
                Some(PresentationOp::VariablePaletteFade {
                    fade_in,
                    mode: mode ^ 1,
                })
            }
            _ => None,
        },
    ];
    for name in [
        "Event_Juza",
        "Event_ReFaze",
        "Cutscene_CrashLaanding",
        "Cutscene_Ending",
        "Cutscene_AlysWounded",
        "Cutscene_PsycoWand",
        "Cutscene_LutzRevelation",
        "Cutscene_ProfoundDarkness",
        "Cutscene_LeRoofAgain",
    ] {
        let scene = SCENES.iter().find(|scene| scene.name == name).unwrap();
        assert!(mismatches(std::slice::from_ref(scene)).is_empty(), "{name}");
        let mut swapped = 0;
        for (index, op) in scene.ops.iter().enumerate() {
            let SceneOp::Presentation { op } = *op else {
                continue;
            };
            for swap in swaps {
                let Some(wrong) = swap(op) else {
                    continue;
                };
                let mut ops = scene.ops.to_vec();
                ops[index] = SceneOp::Presentation { op: wrong };
                let altered = crate::Scene {
                    ops: Box::leak(ops.into_boxed_slice()),
                    ..*scene
                };
                assert_eq!(
                    mismatches(&[altered]).len(),
                    1,
                    "{name} op {index} as {wrong:?} passed"
                );
                swapped += 1;
            }
        }
        assert!(swapped > 0, "{name}: no fade record to swap");
    }
}
