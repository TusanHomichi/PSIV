//! `Cutscene_Ending`'s first text window through its first tone loop
//! (`$078F68..$078FCC`): `RunText3` returns with the window and Chaz's
//! portrait up, the sixty-frame `Pal_IncreaseTone` loop tones them with the
//! panels, and only the scene's `Window_Destroy` takes them down (#636).
//! The next text, `Event_RunDialogue4`'s, destroys its own window.

use super::{pack_present, runtime_on};
use crate::{Button, DialogueView, Pad, Runtime, RuntimeEvent};
use psiv_core::{CharId, DialogueWindow, Input, PresentationOp, SceneOp};

/// One frame of the ending as the session runs it: the window's input half,
/// the field and scene tick, the scene's windows opened, the window's own
/// half. Speak is pressed on the frame after a page turns dismissable, then
/// released.
fn frame(runtime: &mut Runtime, pad: Pad) -> Vec<RuntimeEvent> {
    runtime.dialogue_frame(pad);
    let events = runtime.tick(Input::Neutral);
    for event in &events {
        match event {
            RuntimeEvent::SceneFaulted { .. } => panic!("{event:?}"),
            RuntimeEvent::SceneDialogue { entry } => {
                let layout = runtime.panel_dialogue_mode();
                runtime.open_scene_dialogue(*entry, layout);
            }
            RuntimeEvent::SceneDialogueResume => {
                let layout = runtime.panel_dialogue_mode();
                assert!(runtime.resume_scene_dialogue(layout));
            }
            RuntimeEvent::SceneChoiceRequested => runtime.dialogue_choice(true),
            _ => {}
        }
    }
    runtime.dialogue_tick();
    events
}

fn destroys_window(events: &[RuntimeEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::Presentation {
                    op: PresentationOp::WindowDestroy { .. }
                },
                ..
            }
        )
    })
}

/// What the ending's first two texts left on screen.
struct Trace {
    /// The `EndingIntro` window's last page while it was open.
    last_open: DialogueView,
    /// The view on each frame from its close to the scene's `WindowDestroy`,
    /// that frame included.
    after_close: Vec<Option<DialogueView>>,
    /// The view on the frame after the next window (`Ending`) closed.
    after_next: Option<DialogueView>,
}

fn trace() -> Option<Trace> {
    if !pack_present() {
        return None;
    }
    let mut runtime = runtime_on(0x013, [Some(CharId(0)), None, None, None, None], &[]);
    assert!(runtime.start_event(0x8021), "the ending starts");
    let mut pad = Pad::NEUTRAL;
    let mut last_open = None;
    let mut after_close: Vec<Option<DialogueView>> = Vec::new();
    let mut destroyed = false;
    for _ in 0..20_000 {
        let was_open = runtime.dialogue_open();
        let events = frame(&mut runtime, pad);
        let window = runtime.scene_dialogue_window();
        let view = runtime.dialogue_view();
        pad = if view.as_ref().is_some_and(|view| view.dismissable) && pad == Pad::NEUTRAL {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        if !destroyed {
            if runtime.dialogue_open() {
                assert_eq!(window, Some(DialogueWindow::EndingIntro));
                last_open = view;
            } else if was_open || !after_close.is_empty() {
                after_close.push(view);
                destroyed = destroys_window(&events);
            }
        } else if was_open && !runtime.dialogue_open() {
            assert_eq!(window, Some(DialogueWindow::Ending));
            return Some(Trace {
                last_open: last_open.expect("the first text opened"),
                after_close,
                after_next: runtime.dialogue_view(),
            });
        }
    }
    panic!("the ending's second text never closed");
}

/// Acceptance for #636: from the frame the first text ends until the scene's
/// `WindowDestroy`, the window's last page and Chaz's portrait stay drawn,
/// with no arrow and nothing a press would advance, through the whole tone
/// loop (sixty frames).
#[test]
fn the_endings_first_window_stays_up_until_the_scene_destroys_it() {
    let Some(trace) = trace() else {
        return;
    };
    let (last, held) = trace
        .after_close
        .split_last()
        .expect("frames after the close");
    assert!(held.len() >= 60, "{} frames held", held.len());
    assert!(trace.last_open.portrait.is_some(), "Chaz's portrait is up");
    for (n, view) in held.iter().enumerate() {
        let view = view
            .as_ref()
            .unwrap_or_else(|| panic!("frame {n}: no window"));
        assert_eq!(view.lines, trace.last_open.lines, "frame {n}");
        assert_eq!(view.portrait, trace.last_open.portrait, "frame {n}");
        assert_eq!(view.revealed, view.total, "frame {n}");
        assert_eq!(view.open_cells, trace.last_open.open_cells, "frame {n}");
        assert!(!view.dismissable && !view.waiting, "frame {n}: {view:?}");
    }
    assert_eq!(*last, None, "WindowDestroy takes it down on its own frame");
}

/// Negative control: `Event_RunDialogue4` destroys its own window, so the
/// ending's next text leaves nothing drawn when it closes.
#[test]
fn the_endings_next_window_closes_with_its_text() {
    let Some(trace) = trace() else {
        return;
    };
    assert_eq!(trace.after_next, None);
}
