//! Window exits exercised through the runtime's input/scene/window frame order.
//! The core caller census ties these window kinds to the US image and docs.

use std::path::Path;

use psiv_data::BattleFiles;

use super::palette::{pack_present, runtime_on};
use crate::dialogue::glue::SceneDialogueOpen;
use crate::{Button, DialogueSignal, DialogueView, Pad, Runtime, RuntimeEvent};
use psiv_core::{
    CharId, DialogueId, DialogueSource, DialogueWindow, EventIndex, Input, SCENES, Scene, SceneOp,
};

fn frame(runtime: &mut Runtime, pad: Pad) -> Vec<RuntimeEvent> {
    let signals = runtime.dialogue_frame(pad);
    assert!(
        !signals
            .iter()
            .any(|s| matches!(s, DialogueSignal::Fault(_))),
        "{signals:?}"
    );
    let events = runtime.tick(Input::Neutral);
    for event in &events {
        match event {
            RuntimeEvent::SceneFaulted { .. } | RuntimeEvent::SceneBattleFailed { .. } => {
                panic!("{event:?}");
            }
            RuntimeEvent::SceneDialogue { entry } => {
                let layout = runtime.panel_dialogue_mode();
                assert_eq!(
                    runtime.open_scene_dialogue(*entry, layout),
                    SceneDialogueOpen::Opened
                );
            }
            RuntimeEvent::SceneDialogueResume => {
                let layout = runtime.panel_dialogue_mode();
                assert!(runtime.resume_scene_dialogue(layout));
            }
            _ => {}
        }
    }
    runtime.dialogue_tick();
    events
}

fn next_pad(runtime: &Runtime, previous: Pad) -> Pad {
    if previous == Pad::NEUTRAL && runtime.dialogue_view().is_some_and(|v| v.dismissable) {
        Pad::new(Button::Speak)
    } else {
        Pad::NEUTRAL
    }
}

fn assert_held(last: &DialogueView, view: Option<DialogueView>) {
    let held = view.expect("the last page stays drawn");
    assert!(last.portrait.is_some(), "the test must cover a portrait");
    assert_eq!(held.lines, last.lines);
    assert_eq!(held.portrait, last.portrait);
    assert_eq!(held.open_cells, last.open_cells);
    assert_eq!(held.revealed, held.total);
    assert!(!held.waiting && !held.dismissable);
}

/// Isolate a caller's window exit from its other scene requirements. Use real
/// packed text and presses, then eight frames of scene activity. Resumes use
/// AfterIgglanova's first `$F7` and saved cursor, never a new literal entry.
fn check_exit(window: DialogueWindow, resume: bool) {
    let (map, entry) = if resume { (0x17, 18) } else { (0x187, 0x3C) };
    let mut runtime = runtime_on(map, [Some(CharId(0)), None, None, None, None], &[]);
    let mut ops = vec![SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: if resume {
            DialogueWindow::Standard
        } else {
            window
        },
    }];
    if resume {
        ops.push(SceneOp::RunDialogueResumeWithWindow { window });
    }
    ops.extend([
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::Return { value: 1 },
    ]);
    let scene = Box::leak(Box::new(Scene {
        name: "window exit fixture",
        event: EventIndex(0x5A),
        ops: Box::leak(ops.into_boxed_slice()),
    }));
    assert!(runtime.install_runner(scene, scene.event));
    let expected = matches!(
        window,
        DialogueWindow::Retained | DialogueWindow::EndingIntro
    );
    let mut pad = Pad::NEUTRAL;
    let mut last = None;
    let mut held_frames = 0;
    for _ in 0..10_000 {
        let was_open = runtime.dialogue_open();
        let before = runtime.dialogue_view();
        frame(&mut runtime, pad);
        if was_open && !runtime.dialogue_open() {
            last = before;
        }
        if let Some(last) = &last {
            if !runtime.scene_active() {
                assert_eq!(runtime.dialogue_view(), None, "scene end releases the page");
                assert_eq!(held_frames, 8, "every intervening frame checked");
                return;
            }
            if expected {
                assert_held(last, runtime.dialogue_view());
            } else {
                assert_eq!(
                    runtime.dialogue_view(),
                    None,
                    "{window:?} closes with its text"
                );
            }
            held_frames += 1;
        }
        // Continue pressing even after the text ends: a held page is inert.
        pad = if last.is_some() {
            if pad == Pad::NEUTRAL {
                Pad::new(Button::Speak)
            } else {
                Pad::NEUTRAL
            }
        } else {
            next_pad(&runtime, pad)
        };
    }
    panic!("{window:?}, resume={resume}: scene never finished");
}

#[test]
fn every_retained_caller_holds_its_page_including_the_four_resumes() {
    if !pack_present() {
        return;
    }
    let docs = include_str!("../../../../docs/scenes/DIALOGUE_CALLERS.md");
    let mut entries = 0;
    let mut resumes = 0;
    for scene in SCENES {
        for op in scene.ops {
            let resume = match op {
                SceneOp::RunDialogue {
                    window: DialogueWindow::Retained,
                    ..
                } => false,
                SceneOp::RunDialogueResumeWithWindow {
                    window: DialogueWindow::Retained,
                } => true,
                _ => continue,
            };
            let routine = if resume { "R2" } else { "D2" };
            assert!(
                docs.contains(&format!("| `{}` | {routine} (`Retained`) |", scene.name)),
                "{} missing from census",
                scene.name
            );
            check_exit(DialogueWindow::Retained, resume);
            if resume {
                resumes += 1;
            } else {
                entries += 1;
            }
        }
    }
    assert_eq!((entries, resumes), (14, 4));
}

#[test]
fn the_other_routine_exits_close_at_the_press_and_inline_ending_still_holds() {
    if !pack_present() {
        return;
    }
    for window in [
        DialogueWindow::Standard,
        DialogueWindow::Cutscene,
        DialogueWindow::Cutscene3,
        DialogueWindow::Cutscene5,
        DialogueWindow::Ending,
        DialogueWindow::EndingIntro,
    ] {
        check_exit(window, false);
        check_exit(window, true);
    }
}

#[test]
fn air_castle_removes_the_torch_and_runs_all_five_records_on_the_close_frame() {
    if !pack_present() {
        return;
    }
    let mut runtime = runtime_on(0x187, [Some(CharId(0)), None, None, None, None], &[]);
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    runtime
        .enable_battles(&BattleFiles::load(&pack).expect("battle files"))
        .expect("battles enabled");
    runtime
        .game
        .inventory_mut()
        .add(0x8E)
        .expect("Eclipse Torch");
    assert!(runtime.start_event(0x5A));
    let mut pad = Pad::NEUTRAL;
    for _ in 0..10_000 {
        let was_open = runtime.dialogue_open();
        let last = runtime.dialogue_view();
        let events = frame(&mut runtime, pad);
        if was_open && !runtime.dialogue_open() {
            assert!(events.iter().any(|e| matches!(
                e,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::RemoveItem { .. },
                    ..
                }
            )));
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(
                        e,
                        RuntimeEvent::ScenePresentation {
                            op: SceneOp::ObjectAnimation { .. },
                            ..
                        }
                    ))
                    .count(),
                5
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, RuntimeEvent::EventBattleEntered { index: 0xF }))
            );
            assert!(!runtime.game.inventory().get_item(0x8E));
            let last = last.expect("last open page");
            assert_held(&last, runtime.dialogue_view());
            // The actual scene keeps running through the event battle's
            // entry. Presses during that handoff cannot advance the page.
            for tick in 0..128 {
                let pad = if tick % 2 == 0 {
                    Pad::new(Button::Speak)
                } else {
                    Pad::NEUTRAL
                };
                let events = frame(&mut runtime, pad);
                assert!(runtime.scene_active());
                assert_held(&last, runtime.dialogue_view());
                if events.iter().any(|event| {
                    matches!(event, RuntimeEvent::SceneBattleStarted { index: 0xF, .. })
                }) {
                    return;
                }
            }
            panic!("Air Castle's battle entry never finished");
        }
        pad = next_pad(&runtime, pad);
    }
    panic!("Air Castle never reached its battle");
}
