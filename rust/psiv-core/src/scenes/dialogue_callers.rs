//! The dialogue-entry caller census: every `jsr`/`jmp` to the five
//! `Event_GetAndRunDialogue*` entries and their five `Event_RunDialogue*`
//! resume entries in the US image, and every inline `jsr GetDialogueByID`
//! inside a transcribed scene's bytes, as a row of [`DIALOGUE_CALLERS`]; and
//! the test that holds the registry to it.
//!
//! The rows are re-derived from the image by the census in
//! `dialogue_callers_image.rs`, which also reads each routine's exit from the
//! bytes and checks [`ROUTINE_WINDOWS`] against it, so neither table can rot.
//! The audit, the findings and the callers no scene holds yet are in
//! `docs/scenes/DIALOGUE_CALLERS.md`.

use std::collections::BTreeMap;

use super::SCENES;
use crate::scene::{DialogueId, DialogueSource, DialogueWindow, FifthOp, GuildOp, SceneOp};
use crate::scene_runner::Scene;

/// `Event_GetAndRunDialogue`: panels, both windows and the map chunks go.
pub(super) const D1: u32 = 0x5AC66;
/// `Event_GetAndRunDialogue2`: nothing is destroyed, the counts are zeroed.
pub(super) const D2: u32 = 0x5ACDC;
/// `Event_GetAndRunDialogue3`: as `D1` without `Panel_DestroyAll`.
pub(super) const D3: u32 = 0x5AD32;
/// `Event_GetAndRunDialogue4`: `RunText3`, no button wait, no panel destroy.
pub(super) const D4: u32 = 0x5ADA2;
/// `Event_GetAndRunDialogue5`: as `D3`, then `PalFadeOut_ClrSpriteTbl` and
/// `Panel_DestroyAll`.
pub(super) const D5: u32 = 0x5ADF8;
/// `GetDialogueByID` called by the scene's own window code, which exits its
/// own way: any window may stand for it.
pub(super) const INLINE: u32 = 0x59164;
/// `d0` is not a literal at the call (read off an object or out of a table).
pub(super) const DYN: u16 = 0xFFFF;

/// `Event_RunDialogue`, `D1`'s resume entry: its own body after `jsr
/// GetDialogueByID`, so it reopens `Saved_Dialogue_Addr` and exits as `D1`.
pub(super) const R1: u32 = D1 + 6;
/// `Event_RunDialogue2`, `D2`'s resume entry.
pub(super) const R2: u32 = D2 + 6;
/// `Event_RunDialogue3`, `D3`'s resume entry.
pub(super) const R3: u32 = D3 + 6;
/// `Event_RunDialogue4`, `D4`'s resume entry.
pub(super) const R4: u32 = D4 + 6;
/// `Event_RunDialogue5`, `D5`'s resume entry.
pub(super) const R5: u32 = D5 + 6;
/// A resume row's entry: the call reads no `d0`, it reopens
/// `Saved_Dialogue_Addr`.
pub(super) const RESUME: u16 = 0xFFFE;

/// Which [`DialogueWindow`] each routine is. Five routines, five variants;
/// `Cutscene5` (`Event_RunDialogue5`'s resume) and `EndingIntro` (an inline
/// `RunText3` window) belong to no `Get` entry.
pub(super) const ROUTINE_WINDOWS: &[(u32, DialogueWindow)] = &[
    (D1, DialogueWindow::Standard),
    (D2, DialogueWindow::Retained),
    (D3, DialogueWindow::Cutscene3),
    (D4, DialogueWindow::Ending),
    (D5, DialogueWindow::Cutscene),
];

/// Resume calls a scene's transcription skips, by call (or inline `popdlg`)
/// address, with the issue that transcribes them: the census does not ask the
/// scene for them, and fails when one stops being a resume call of the scene.
pub(super) const UNTRANSCRIBED_RESUMES: &[(&str, &[u32], u32)] = &[
    // `$075B10..$075D72`, Demi's BioPlant sequence.
    (
        "Cutscene_ZioDefeated",
        &[0x075BA0, 0x075C56, 0x075C94, 0x075CD4],
        414,
    ),
    // The tail after panel `$130`, built on #162's palette loops.
    ("Cutscene_Elsydeon", &[0x07892A], 414),
];

/// Which [`DialogueWindow`] a resume through each `Event_RunDialogue*` entry
/// draws in: its `Get` routine's exit, which it shares. `R5`'s is
/// `Cutscene5`, the port's name for `Cutscene` reached through the resume.
pub(super) const RESUME_WINDOWS: &[(u32, DialogueWindow)] = &[
    (R1, DialogueWindow::Standard),
    (R2, DialogueWindow::Retained),
    (R3, DialogueWindow::Cutscene3),
    (R4, DialogueWindow::Ending),
    (R5, DialogueWindow::Cutscene5),
];

pub(super) use super::dialogue_caller_rows::DIALOGUE_CALLERS;

pub(super) type Row = (&'static str, u32, u32, u16);

pub(super) fn routine_name(routine: u32) -> &'static str {
    match routine {
        D1 => "Event_GetAndRunDialogue",
        D2 => "Event_GetAndRunDialogue2",
        D3 => "Event_GetAndRunDialogue3",
        D4 => "Event_GetAndRunDialogue4",
        D5 => "Event_GetAndRunDialogue5",
        R1 => "Event_RunDialogue",
        R2 => "Event_RunDialogue2",
        R3 => "Event_RunDialogue3",
        R4 => "Event_RunDialogue4",
        R5 => "Event_RunDialogue5",
        _ => "GetDialogueByID (inline)",
    }
}

/// Each dialogue a scene opens: its window and its literal entry, `DYN` when
/// the entry is read off an object or out of a table. The table-driven ops
/// open `Event_GetAndRunDialogue`'s window (`Standard`) on the line they
/// compute: the guild's row line, the fifth-character row's `$5(a0)` lines.
///
/// A resume is `(window, RESUME)`, in the window it draws in: the one it
/// names, or for a bare `RunDialogueResume` the one the scene's last dialogue
/// left (the runner keeps it), in op order from `Standard`, the runner's
/// first.
fn dialogue_ops(scene: &Scene) -> Vec<(DialogueWindow, u16)> {
    let mut current = DialogueWindow::Standard;
    scene
        .ops
        .iter()
        .filter_map(|op| match op {
            // A system line is drawn from a string of the cartridge's own, not
            // out of a tree, so it is no `GetDialogueByID` caller.
            SceneOp::RunDialogue {
                source: DialogueSource::System(_),
                window,
            } => {
                current = *window;
                None
            }
            SceneOp::RunDialogue { source, window } => {
                current = *window;
                Some((
                    *window,
                    match source {
                        DialogueSource::Entry(DialogueId(entry)) => *entry,
                        DialogueSource::NpcDialogueId(_) | DialogueSource::System(_) => DYN,
                    },
                ))
            }
            SceneOp::RunDialogueResumeWithWindow { window } => {
                current = *window;
                Some((*window, RESUME))
            }
            SceneOp::RunDialogueResume => Some((current, RESUME)),
            SceneOp::Guild(GuildOp::RunLine { .. })
            | SceneOp::FifthCharacter(FifthOp::RunLine { .. } | FifthOp::RunLeaverLine) => {
                current = DialogueWindow::Standard;
                Some((DialogueWindow::Standard, DYN))
            }
            _ => None,
        })
        .collect()
}

/// Everything wrong between `scenes` and `table`; empty when they agree.
///
/// Per scene and window: the entries its rows for that window's routine give
/// must be exactly the entries it runs in that window (as multisets), except
/// that an entry an `INLINE` row gives may run in any window, and a routine
/// with a `DYN` row only needs its literal entries present (the port unrolls
/// the table `d0` came from). A `RunDialogue` in a window with no routine row
/// is an error, so a scene cannot claim a routine its bytes do not call.
fn caller_problems(
    scenes: &[Scene],
    table: &[Row],
    untranscribed: &[(&str, &[u32], u32)],
) -> Vec<String> {
    let mut problems = Vec::new();
    let window_of = |routine: u32| {
        ROUTINE_WINDOWS
            .iter()
            .chain(RESUME_WINDOWS)
            .find(|(r, _)| *r == routine)
            .map(|(_, w)| *w)
    };
    let mut seen = BTreeMap::new();
    for row in table {
        if let Some(other) = seen.insert((row.1, row.2, row.3), row.0) {
            problems.push(format!(
                "${:06X} {} entry {:#X} is claimed by both {other} and {}",
                row.2,
                routine_name(row.1),
                row.3,
                row.0
            ));
        }
        if !scenes.iter().any(|scene| scene.name == row.0) {
            problems.push(format!(
                "{} has rows in DIALOGUE_CALLERS (${:06X}) but is not in the registry",
                row.0, row.2
            ));
        }
    }
    for (name, calls, issue) in untranscribed {
        for at in *calls {
            if !table
                .iter()
                .any(|row| row.0 == *name && row.2 == *at && row.3 == RESUME)
            {
                problems.push(format!(
                    "UNTRANSCRIBED_RESUMES lists ${at:06X} for {name} (#{issue}), which is \
                     not one of its resume calls"
                ));
            }
        }
    }
    for scene in scenes {
        let skipped: &[u32] = untranscribed
            .iter()
            .find(|(name, ..)| *name == scene.name)
            .map_or(&[], |(_, calls, _)| *calls);
        let rows: Vec<&Row> = table
            .iter()
            .filter(|row| row.0 == scene.name && !(row.3 == RESUME && skipped.contains(&row.2)))
            .collect();
        let inline: Vec<u16> = rows
            .iter()
            .filter(|row| row.1 == INLINE)
            .map(|row| row.3)
            .collect();
        let ran = dialogue_ops(scene);
        let inline_resumes = inline.iter().filter(|e| **e == RESUME).count();
        let mut extra_resumes = 0;
        let mut windows: Vec<DialogueWindow> = ran.iter().map(|(w, _)| *w).collect();
        windows.extend(
            ROUTINE_WINDOWS
                .iter()
                .chain(RESUME_WINDOWS)
                .map(|(_, w)| *w),
        );
        windows.sort();
        windows.dedup();
        for window in windows {
            // Resumes: at least as many in this window as its bytes call the
            // resume entry whose exit it is; the rest are inline resumes,
            // counted after the loop.
            let resumes: Vec<&&Row> = rows
                .iter()
                .filter(|row| row.3 == RESUME && window_of(row.1) == Some(window))
                .collect();
            let resumed = ran
                .iter()
                .filter(|(w, e)| *w == window && *e == RESUME)
                .count();
            // A `Get` call with a computed entry may be unrolled into a branch
            // per entry, each with its own resumes (`Event_CatchingRocky`'s
            // per-map rows): the extra ones are those branches'.
            let unrolled = rows
                .iter()
                .any(|row| row.3 == DYN && window_of(row.1) == Some(window));
            if resumed > resumes.len() && !unrolled {
                extra_resumes += resumed - resumes.len();
            } else if resumed < resumes.len() {
                let at: Vec<String> = resumes
                    .iter()
                    .map(|row| format!("${:06X} {}", row.2, routine_name(row.1)))
                    .collect();
                problems.push(format!(
                    "{}: {resumed} resume(s) in DialogueWindow::{window:?}, its bytes resume \
                     there {} time(s) [{}]",
                    scene.name,
                    resumes.len(),
                    at.join(", ")
                ));
            }
            let mut want: Vec<(u16, u32, u32)> = rows
                .iter()
                .filter(|row| row.3 != RESUME && window_of(row.1) == Some(window))
                .map(|row| (row.3, row.1, row.2))
                .collect();
            let mut have: Vec<u16> = ran
                .iter()
                .filter(|(w, e)| *w == window && *e != RESUME)
                .map(|(_, e)| *e)
                .collect();
            let dynamic = want.iter().any(|(entry, _, _)| *entry == DYN);
            want.retain(|(entry, _, _)| *entry != DYN);
            if dynamic && have.is_empty() {
                problems.push(format!(
                    "{}: its bytes call the {window:?} routine with a computed entry, \
                     the scene runs no {window:?} dialogue",
                    scene.name
                ));
            }
            for (entry, routine, at) in &want {
                if let Some(i) = have.iter().position(|e| e == entry) {
                    if !dynamic {
                        have.swap_remove(i);
                    }
                } else {
                    problems.push(format!(
                        "{}: ${at:06X} runs entry {entry:#X} through {}, the scene has no \
                         RunDialogue {{ Entry({entry:#X}), DialogueWindow::{window:?} }}",
                        scene.name,
                        routine_name(*routine)
                    ));
                }
            }
            if dynamic {
                continue;
            }
            for entry in have {
                if entry == DYN || !inline.contains(&entry) {
                    problems.push(format!(
                        "{}: runs entry {entry:#X} in DialogueWindow::{window:?}, which no \
                         caller in its bytes asks for",
                        scene.name
                    ));
                }
            }
        }
        if extra_resumes != inline_resumes {
            problems.push(format!(
                "{}: {extra_resumes} resume(s) beyond its resume calls' windows, its bytes \
                 have {inline_resumes} inline resume(s) (`popdlg` into its own window code)",
                scene.name
            ));
        }
    }
    problems
}

#[test]
fn every_dialogue_entry_caller_runs_its_window() {
    let problems = caller_problems(SCENES, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The H22 shape: `RunDialogueResume` reopens `Saved_Dialogue_Addr`, so a
/// scene may only resume after it has run a dialogue itself. A scene whose
/// first dialogue op is a resume reads whatever ran last in play, or nothing
/// after a load.
#[test]
fn no_scene_resumes_before_it_opens_a_dialogue() {
    for scene in SCENES {
        let first = scene.ops.iter().find(|op| {
            matches!(
                op,
                SceneOp::RunDialogue { .. }
                    | SceneOp::RunDialogueResume
                    | SceneOp::RunDialogueResumeWithWindow { .. }
            )
        });
        assert!(
            !matches!(
                first,
                Some(SceneOp::RunDialogueResume | SceneOp::RunDialogueResumeWithWindow { .. })
            ),
            "{} resumes a dialogue it never opened",
            scene.name
        );
    }
}

/// The registry with `name`'s ops replaced by `ops`.
fn replaced(name: &str, ops: Vec<SceneOp>) -> Vec<Scene> {
    let ops: &'static [SceneOp] = Box::leak(ops.into_boxed_slice());
    assert!(
        SCENES.iter().any(|scene| scene.name == name),
        "{name} is not registered"
    );
    SCENES
        .iter()
        .map(|scene| Scene {
            name: scene.name,
            event: scene.event,
            ops: if scene.name == name { ops } else { scene.ops },
        })
        .collect()
}

fn ops_of(name: &str) -> Vec<SceneOp> {
    SCENES
        .iter()
        .find(|scene| scene.name == name)
        .map(|scene| scene.ops.to_vec())
        .unwrap_or_else(|| panic!("{name} is not registered"))
}

/// The registry with `name`'s ops rewritten by `edit`, which must change one.
fn edited(name: &str, edit: impl Fn(&SceneOp) -> SceneOp) -> Vec<Scene> {
    let before = ops_of(name);
    let after: Vec<SceneOp> = before.iter().map(edit).collect();
    assert_ne!(
        before, after,
        "the control's edit changed nothing in {name}"
    );
    replaced(name, after)
}

const fn run(entry: u16, window: DialogueWindow) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window,
    }
}

fn swap(from: SceneOp, to: SceneOp) -> impl Fn(&SceneOp) -> SceneOp {
    move |op| if *op == from { to } else { *op }
}

#[test]
fn control_a_scene_back_on_resume_is_reported() {
    let scenes = edited(
        "Event_ZioNurvus",
        swap(
            run(0x0B, DialogueWindow::Retained),
            SceneOp::RunDialogueResume,
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Event_ZioNurvus: $06F3D4 runs entry 0xB through Event_GetAndRunDialogue2, the \
             scene has no RunDialogue { Entry(0xB), DialogueWindow::Retained }",
            // and the resume stands where no resume call is.
            "Event_ZioNurvus: 1 resume(s) beyond its resume calls' windows, its bytes have 0 \
             inline resume(s) (`popdlg` into its own window code)"
        ]
    );
}

#[test]
fn control_a_wrong_variant_is_reported_both_ways() {
    // `Cutscene_MeetingKyra`'s `$28` back on the `Event_GetAndRunDialogue5`
    // window, as it was transcribed before this census.
    let scenes = edited(
        "Cutscene_MeetingKyra",
        swap(
            run(0x28, DialogueWindow::Cutscene3),
            run(0x28, DialogueWindow::Cutscene),
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    let reported = |needle: &str| problems.iter().any(|p| p.contains(needle));
    assert!(reported(
        "$0777D8 runs entry 0x28 through Event_GetAndRunDialogue3"
    ));
    assert!(reported(
        "runs entry 0x28 in DialogueWindow::Cutscene, which no"
    ));
}

#[test]
fn control_a_wrong_entry_is_reported() {
    let scenes = edited(
        "Event_Juza",
        swap(
            run(0x48, DialogueWindow::Retained),
            run(0x47, DialogueWindow::Retained),
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(problems.len(), 2, "{problems:#?}");
}

#[test]
fn control_a_duplicated_op_is_reported() {
    // Twice the one retained window `Event_DarkForce2` has: the multiset
    // comparison sees a count, not just a set.
    let mut ops = ops_of("Event_DarkForce2");
    ops.push(run(0x3A, DialogueWindow::Retained));
    let scenes = replaced("Event_DarkForce2", ops);
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Event_DarkForce2: runs entry 0x3A in DialogueWindow::Retained, which no caller \
             in its bytes asks for"
        ]
    );
}

#[test]
fn control_a_dropped_row_is_reported() {
    let table: Vec<Row> = DIALOGUE_CALLERS
        .iter()
        .copied()
        .filter(|row| row.2 != 0x06FB42)
        .collect();
    let problems = caller_problems(SCENES, &table, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Event_Juza: runs entry 0x48 in DialogueWindow::Retained, which no caller in its \
             bytes asks for"
        ]
    );
}

#[test]
fn control_a_caller_claimed_by_two_scenes_is_reported() {
    let mut table = DIALOGUE_CALLERS.to_vec();
    table.push(("Event_DarkForce2", D2, 0x06F3D4, 0x0B));
    let problems = caller_problems(SCENES, &table, UNTRANSCRIBED_RESUMES);
    assert!(
        problems[0].starts_with("$06F3D4 Event_GetAndRunDialogue2 entry 0xB is claimed by both"),
        "{problems:#?}"
    );
}

#[test]
fn control_a_row_for_an_unregistered_scene_is_reported() {
    let mut table = DIALOGUE_CALLERS.to_vec();
    table.push(("Event_NotYetTranscribed", D1, 0x073400, 0x33));
    let problems = caller_problems(SCENES, &table, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Event_NotYetTranscribed has rows in DIALOGUE_CALLERS ($073400) but is not in the registry"
        ]
    );
}

#[test]
fn control_a_dynamic_caller_with_no_window_is_reported() {
    // `Event_AlysFound` reads `d0` off Alys's object (`move.b $14(a4), d0`).
    let scenes = edited("Event_AlysFound", |op| match op {
        SceneOp::RunDialogue { source, .. } => SceneOp::RunDialogue {
            source: *source,
            window: DialogueWindow::Cutscene,
        },
        other => *other,
    });
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(problems[0].contains("computed entry"));
}

#[test]
fn control_an_inline_entry_does_not_excuse_another_entry() {
    // `Cutscene_CrashLaanding` runs `$03` from its own window code
    // (`$076622`); a `$04` in the same window is still unexplained.
    let scenes = edited(
        "Cutscene_CrashLaanding",
        swap(
            run(0x03, DialogueWindow::Cutscene),
            run(0x04, DialogueWindow::Cutscene),
        ),
    );
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Cutscene_CrashLaanding: runs entry 0x4 in DialogueWindow::Cutscene, which no \
             caller in its bytes asks for"
        ]
    );
}

#[test]
fn control_a_table_driven_line_is_a_window_the_census_counts() {
    // `Event_PickingFifthCharacter` opens its three `Event_GetAndRunDialogue`
    // lines through `FifthOp`; without them the scene runs no `Standard`
    // window for its computed callers (`$071F34`, `$071F44`, `$071F86`).
    let scenes = edited("Event_PickingFifthCharacter", |op| match op {
        SceneOp::FifthCharacter(FifthOp::RunLine { .. } | FifthOp::RunLeaverLine) => {
            SceneOp::Wait { ticks: 1 }
        }
        other => *other,
    });
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Event_PickingFifthCharacter: its bytes call the Standard routine with a computed \
             entry, the scene runs no Standard dialogue"
        ]
    );
}

const fn resume_in(window: DialogueWindow) -> SceneOp {
    SceneOp::RunDialogueResumeWithWindow { window }
}

/// The `n`th resume op of `name` replaced by `to`.
fn with_resume(name: &str, n: usize, to: Option<SceneOp>) -> Vec<Scene> {
    let mut ops = ops_of(name);
    let at = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| {
            matches!(
                op,
                SceneOp::RunDialogueResume | SceneOp::RunDialogueResumeWithWindow { .. }
            )
        })
        .nth(n)
        .map(|(i, _)| i)
        .unwrap_or_else(|| panic!("{name} has no resume {n}"));
    match to {
        Some(op) => ops[at] = op,
        None => {
            ops.remove(at);
        }
    }
    replaced(name, ops)
}

#[test]
fn control_a_resume_back_on_the_last_window_is_reported() {
    // `Cutscene_DemiRescue`'s `Event_RunDialogue2` as the bare resume it was:
    // it kept the `Get3` window before it.
    let scenes = with_resume("Cutscene_DemiRescue", 0, Some(SceneOp::RunDialogueResume));
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(problems.iter().any(|p| p.contains(
        "0 resume(s) in DialogueWindow::Retained, its bytes resume there 1 time(s) \
         [$074B10 Event_RunDialogue2]"
    )));
}

#[test]
fn control_a_dropped_resume_is_reported() {
    // `Cutscene_AlysWounded`'s third page after `$2C`, as it was.
    let scenes = with_resume("Cutscene_AlysWounded", 3, None);
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert!(problems[0].starts_with(
        "Cutscene_AlysWounded: 12 resume(s) in DialogueWindow::Standard, its bytes resume \
         there 13 time(s)"
    ));
}

#[test]
fn control_an_extra_resume_is_reported() {
    let mut ops = ops_of("Cutscene_MeetingKyra");
    let last = ops.len() - 1;
    ops.insert(last, resume_in(DialogueWindow::Cutscene5));
    let scenes = replaced("Cutscene_MeetingKyra", ops);
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Cutscene_MeetingKyra: 1 resume(s) beyond its resume calls' windows, its bytes \
             have 0 inline resume(s) (`popdlg` into its own window code)"
        ]
    );
}

#[test]
fn control_a_missing_inline_resume_is_reported() {
    // `Cutscene_Elsydeon`'s inline page at `$078740` (its second resume).
    let scenes = with_resume("Cutscene_Elsydeon", 1, None);
    let problems = caller_problems(&scenes, DIALOGUE_CALLERS, UNTRANSCRIBED_RESUMES);
    assert_eq!(
        problems,
        vec![
            "Cutscene_Elsydeon: 0 resume(s) beyond its resume calls' windows, its bytes have \
             1 inline resume(s) (`popdlg` into its own window code)"
        ]
    );
}

#[test]
fn control_a_stale_untranscribed_resume_is_reported() {
    let stale: &[(&str, &[u32], u32)] = &[("Cutscene_Elsydeon", &[0x07892A, 0x078000], 414)];
    let problems = caller_problems(SCENES, DIALOGUE_CALLERS, stale);
    assert!(
        problems.contains(
            &"UNTRANSCRIBED_RESUMES lists $078000 for Cutscene_Elsydeon (#414), which is not \
              one of its resume calls"
                .to_owned()
        ),
        "{problems:#?}"
    );
    // Without the list, ZioDefeated's skipped pages are reported.
    let problems = caller_problems(SCENES, DIALOGUE_CALLERS, &[]);
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("Cutscene_ZioDefeated: 0 resume(s) in DialogueWindow::Standard")),
        "{problems:#?}"
    );
}
