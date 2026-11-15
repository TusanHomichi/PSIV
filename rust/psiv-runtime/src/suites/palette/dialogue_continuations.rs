//! The callers of Get5/Run5 own the palette reload and display enable after
//! their shared close. Exercise actual scene records with either packed
//! dialogue plus Speak, or the headless acknowledgement path.
use super::{pack_present, runtime_at};
use crate::dialogue::DialogueSignal;
use crate::pad::{Button, Pad};
use crate::{Runtime, RuntimeEvent, SceneDialogueOpen};
use psiv_core::{CharId, Flag, Input, Scene, SceneOp};

fn runtime(map: u16) -> Runtime {
    runtime_at(
        map,
        (16, 16),
        [
            Some(CharId(0)),
            Some(CharId(1)),
            Some(CharId(2)),
            Some(CharId(3)),
            None,
        ],
        &[],
    )
}

fn tick(rt: &mut Runtime, headless: bool, frame: usize) -> Vec<RuntimeEvent> {
    let was_open = rt.dialogue_open();
    if !headless {
        let pad = if frame % 3 == 0 {
            Pad::NEUTRAL
        } else {
            Pad::new(Button::Speak)
        };
        let signals = rt.dialogue_frame(pad);
        assert!(
            !signals
                .iter()
                .any(|s| matches!(s, DialogueSignal::Fault(_))),
            "{signals:?}"
        );
    }
    if !was_open {
        rt.set_field_suspended(false);
    }
    let events = rt.tick(Input::Neutral);
    for event in &events {
        match event {
            RuntimeEvent::SceneFaulted { .. } | RuntimeEvent::MapRefreshFailed { .. } => {
                panic!("{event:?}")
            }
            RuntimeEvent::SceneDialogue { entry } => {
                if headless {
                    rt.dialogue_closed();
                } else {
                    let layout = rt.panel_dialogue_mode();
                    assert_eq!(
                        rt.open_scene_dialogue(*entry, layout),
                        SceneDialogueOpen::Opened
                    );
                }
            }
            RuntimeEvent::SceneDialogueResume => {
                if headless {
                    rt.dialogue_closed();
                } else {
                    let layout = rt.panel_dialogue_mode();
                    assert!(rt.resume_scene_dialogue(layout));
                }
            }
            _ => {}
        }
    }
    if !headless {
        let signals = rt.dialogue_tick();
        assert!(
            !signals
                .iter()
                .any(|s| matches!(s, DialogueSignal::Fault(_))),
            "{signals:?}"
        );
    }
    events
}

fn op(event: &RuntimeEvent) -> Option<SceneOp> {
    if let RuntimeEvent::ScenePresentation { op, .. } = event {
        Some(*op)
    } else {
        None
    }
}

/// The source calls RefreshMap at :153924 and :153982, which reloads all
/// 64 palette words through loc_53F14 before Pal_FadeIn. The field and text
/// ink must reach that palette after each Get5/Run5 return, not fade to zero.
fn rika(headless: bool, omit_reload: Option<usize>) -> bool {
    let mut rt = runtime(0xAC);
    let scene = *psiv_core::scene_for(psiv_core::EventIndex(0x8007)).unwrap();
    let mut reload = 0;
    let ops = scene
        .ops
        .iter()
        .copied()
        .filter(|op| {
            if *op != SceneOp::ReloadMapPalette {
                return true;
            }
            let keep = omit_reload != Some(reload);
            reload += 1;
            keep
        })
        .collect::<Vec<_>>();
    assert_eq!(reload, 2);
    let scene = Box::leak(Box::new(Scene {
        ops: Box::leak(ops.into_boxed_slice()),
        ..scene
    }));
    assert!(rt.install_runner(scene, scene.event));
    let mut pending_close = false;
    let mut expected = None;
    let mut reached = 0;
    let mut correct = true;
    for frame in 0..100_000 {
        let closing = rt.dialogue_close.is_some();
        let events = tick(&mut rt, headless, frame);
        pending_close |= closing || rt.dialogue_close.is_some();
        for event in &events {
            if op(event) == Some(SceneOp::FadeIn) && pending_close {
                let palette = rt
                    .data
                    .map(psiv_data::MapId(rt.map_id().0))
                    .unwrap()
                    .map_update_palette
                    .as_ref()
                    .unwrap();
                let target: [u16; 64] = palette.as_slice().try_into().unwrap();
                assert_ne!(
                    target[crate::cram::TEXT_COLOUR_SLOT],
                    0,
                    "text target is visible"
                );
                correct &= rt.effects.cram.saved() == &target;
                expected = Some(target);
                pending_close = false;
            }
        }
        if let Some(target) = expected {
            if rt.cram().words == target && rt.cram().display {
                reached += 1;
                expected = None;
                if reached == 2 {
                    return correct;
                }
            } else if !correct {
                return false;
            }
        }
    }
    panic!("MeetingRika did not reveal both continuation palettes");
}

#[test]
fn meeting_rika_restores_field_and_window_colours_after_both_closes() {
    if !pack_present() {
        return;
    }
    for headless in [false, true] {
        assert!(rika(headless, None));
        for omitted in 0..2 {
            assert!(
                !rika(headless, Some(omitted)),
                "negative control: omitted reload {omitted}"
            );
        }
    }
}

#[test]
fn saya_text_and_dorin_departure_are_lit_after_their_close() {
    if !pack_present() {
        return;
    }
    for headless in [false, true] {
        let mut rt = runtime(0x8B);
        let _ = rt.game.clear(Flag::event(0x12));
        assert!(rt.start_event(0x8009));
        let mut saya = false;
        let mut resumes = 0;
        for frame in 0..100_000 {
            let events = tick(&mut rt, headless, frame);
            if events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneDialogue { entry: 0x2C }))
            {
                saya = true;
            }
            if saya {
                assert!(rt.cram().display, "Saya's dialogue must be visible");
                assert_ne!(rt.cram().words[crate::cram::TEXT_COLOUR_SLOT], 0);
                resumes += events
                    .iter()
                    .filter(|e| matches!(e, RuntimeEvent::SceneDialogueResume))
                    .count();
                if resumes == 3 {
                    break;
                }
            }
        }
        assert!(saya && resumes == 3);
        let mut rt = runtime(0x43);
        assert!(rt.start_event(0x8004));
        let mut departed = false;
        for frame in 0..100_000 {
            let events = tick(&mut rt, headless, frame);
            if events
                .iter()
                .filter_map(op)
                .any(|op| matches!(op, SceneOp::PlaySound { id: 0x81 }))
            {
                assert!(rt.cram().display, "Dorin's departure is visible");
                assert!(rt.cram().words.iter().any(|w| *w != 0));
                departed = true;
                break;
            }
        }
        assert!(departed);
    }
}

#[test]
fn crash_landings_inline_window_does_not_fade_panel_86_to_black() {
    if !pack_present() {
        return;
    }
    for headless in [false, true] {
        let mut rt = runtime(0x18C);
        assert!(rt.start_event(0x800F));
        let mut shown = false;
        for frame in 0..100_000 {
            let events = tick(&mut rt, headless, frame);
            if events
                .iter()
                .filter_map(op)
                .any(|op| op == SceneOp::PanelCreate { id: 0x86 })
            {
                assert!(rt.cram().display);
                assert!(rt.cram().words.iter().any(|w| *w != 0));
                assert!(rt.dialogue_close.is_none());
                shown = true;
                break;
            }
        }
        assert!(shown);
    }
}

#[test]
fn alshline_copies_its_saved_palette_back_before_the_next_fade_in() {
    if !pack_present() {
        return;
    }
    let mut rt = runtime(0x24);
    assert!(rt.start_event(0x8005));
    let mut before_close = None;
    for frame in 0..10_000 {
        let events = tick(&mut rt, true, frame);
        if events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneDialogue { entry: 0x67 }))
        {
            let palette = rt.cram().words;
            assert!(palette.iter().any(|word| *word != 0));
            before_close = Some(palette);
        }
        if events.iter().filter_map(op).any(|op| {
            matches!(
                op,
                SceneOp::Presentation {
                    op: psiv_core::PresentationOp::PaletteCopy {
                        source: psiv_core::PaletteSource::Buffer(psiv_core::PaletteBuffer::Saved),
                        destination: psiv_core::PaletteBuffer::Live,
                        words: 64,
                        ..
                    }
                }
            )
        }) {
            assert_eq!(rt.cram().words, before_close.expect("Get5 closed first"));
            assert!(!rt.cram().display, "the copy does not enable the display");
            return;
        }
    }
    panic!("Alshline did not restore its saved palette");
}
