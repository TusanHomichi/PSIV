//! The resume half of the image census: each `Event_RunDialogue*` entry is
//! its `Get` routine's body after `jsr GetDialogueByID`, and the window
//! [`RESUME_WINDOWS`] gives it is that routine's exit
//! (`docs/scenes/DIALOGUE_CALLERS.md`, "The resume census"). The callers
//! themselves are re-derived with the rest in `dialogue_callers_image.rs`.

use super::dialogue_callers::{RESUME_WINDOWS, ROUTINE_WINDOWS, routine_name};
use super::dialogue_callers_image::{GET_DIALOGUE_BY_ID, RESUMES, ROUTINES, rom};
use crate::scene::DialogueWindow;

/// The exit a window stands for: `Cutscene5` is `Cutscene`'s, reached
/// through `Event_RunDialogue5`.
fn exit_of(window: DialogueWindow) -> DialogueWindow {
    match window {
        DialogueWindow::Cutscene5 => DialogueWindow::Cutscene,
        other => other,
    }
}

/// Every resume entry that is not its `Get` routine's body after `jsr
/// GetDialogueByID`, or whose window is not that routine's exit.
fn resume_problems(
    rom: &[u8],
    gets: &[(u32, DialogueWindow)],
    resumes: &[(u32, DialogueWindow)],
) -> Vec<String> {
    let mut problems = Vec::new();
    for (get, resume) in ROUTINES.into_iter().zip(RESUMES) {
        let label = routine_name(resume);
        if rom[get as usize..resume as usize] != GET_DIALOGUE_BY_ID {
            problems.push(format!(
                "{label}: ${resume:06X} is not {}'s body after jsr GetDialogueByID",
                routine_name(get)
            ));
        }
        let window = |table: &[(u32, DialogueWindow)], routine| {
            table.iter().find(|(r, _)| *r == routine).map(|(_, w)| *w)
        };
        match (window(gets, get), window(resumes, resume)) {
            (Some(g), Some(r)) if exit_of(g) == exit_of(r) => {}
            (g, r) => problems.push(format!(
                "{label}: RESUME_WINDOWS gives it {r:?}, but it exits as {} ({g:?})",
                routine_name(get)
            )),
        }
    }
    problems
}

#[test]
fn each_resume_entry_exits_as_its_routine() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = resume_problems(rom, ROUTINE_WINDOWS, RESUME_WINDOWS);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn control_a_resume_on_another_routine_s_window_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let mut windows = RESUME_WINDOWS.to_vec();
    windows[1].1 = DialogueWindow::Standard;
    let problems = resume_problems(rom, ROUTINE_WINDOWS, &windows);
    assert_eq!(
        problems,
        vec![
            "Event_RunDialogue2: RESUME_WINDOWS gives it Some(Standard), but it exits as \
              Event_GetAndRunDialogue2 (Some(Retained))"
        ]
    );
}
