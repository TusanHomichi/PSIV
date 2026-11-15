use super::*;
use crate::dialogue::{DialogueSignal, Opening, TextFlow};
use crate::pad::{Button, Pad};
use psiv_core::{
    CharId, DialogueId, DialogueSource, EventIndex, Flag, Input, PresentationOp, Scene,
};
use psiv_data::{Ctrl, DialogueEntry, Segment};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn pack_present() -> bool {
    std::path::Path::new(PACK).join("manifest.json").is_file()
}

fn alshline() -> Runtime {
    use psiv_core::{GameState, RetailLocation, RetailSave, StepFrames};
    let data = psiv_data::GameData::load(std::path::Path::new(PACK)).expect("pack loads");
    let initial = crate::Session::start(data.clone())
        .field()
        .expect("pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    game.set(Flag::event(0x32)).unwrap();
    Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x24,
                char_x: 31 * 16,
                char_y: 49 * 16,
            },
        },
        StepFrames::default(),
    )
    .expect("Alshline fixture")
}

fn is_op(event: &RuntimeEvent, wanted: SceneOp) -> bool {
    matches!(event, RuntimeEvent::ScenePresentation { op, .. } if *op == wanted)
}

/// The real input close, F7 and FF, with each fading window and a Standard
/// negative control. A maximal nibble of six means six one-level passes,
/// including the pass that reaches black.
#[test]
fn close_waits_for_every_fade_level_before_destroying_panels_and_returning() {
    if !pack_present() {
        return;
    }
    for window in [
        DialogueWindow::Cutscene,
        DialogueWindow::Cutscene5,
        DialogueWindow::Standard,
    ] {
        for suspended in [false, true] {
            let mut rt = alshline();
            let scene = Box::leak(Box::new(Scene {
                name: "test dialogue return",
                event: EventIndex(0x8FF0),
                ops: Box::leak(Box::new([
                    SceneOp::RunDialogue {
                        source: DialogueSource::Entry(DialogueId(0)),
                        window,
                    },
                    SceneOp::PlaySound { id: 0x81 },
                    SceneOp::WaitFrames { frames: 100 },
                ])),
            }));
            assert!(rt.install_runner(scene, scene.event));
            assert!((0..4).any(|_| {
                rt.tick(Input::Neutral)
                    .iter()
                    .any(|e| matches!(e, RuntimeEvent::SceneDialogue { .. }))
            }));
            let mut segments = vec![Segment::Text("x".into())];
            if suspended {
                segments.push(Segment::Control(Ctrl::Close {
                    code: 0xF7,
                    operands: vec![],
                }));
                segments.push(Segment::Text("y".into()));
            }
            let entry = DialogueEntry {
                id: 0,
                text: String::new(),
                segments,
                pages: vec![],
            };
            let Opening::Window(flow) = TextFlow::open_at(&entry, &rt.game) else {
                panic!("window")
            };
            rt.dialogue.scene_dialogue = true;
            rt.dialogue.start(Opening::Window(flow), "test");
            rt.dialogue.open_cells = rt.dialogue.full_cells();
            rt.dialogue.revealed = 1;
            let (cram, cycle) = rt.effects.cram_and_cycle_index();
            cram.apply(
                &SceneOp::Presentation {
                    op: PresentationOp::PaletteCopy {
                        source: psiv_core::PaletteSource::Fill(0x642),
                        destination: psiv_core::PaletteBuffer::Live,
                        start: 0,
                        words: 64,
                    },
                },
                rt.frames,
                rt.data.scene_palettes(),
                None,
                cycle,
            );
            rt.scene_panel_sprites = true;
            let signals = rt.dialogue_frame(Pad::new(Button::Speak));
            assert!(signals.contains(&DialogueSignal::Closed { suspended }));
            assert!(!rt.dialogue_open(), "windows are destroyed before fading");
            if window == DialogueWindow::Standard {
                assert!(
                    rt.dialogue_close.is_none(),
                    "negative control: Standard never fades"
                );
                assert!(
                    rt.tick(Input::Neutral)
                        .iter()
                        .any(|e| is_op(e, SceneOp::PlaySound { id: 0x81 }))
                );
                assert_eq!(rt.cram().words, [0x642; 64]);
                assert!(rt.cram().display);
                continue;
            }
            for expected in [0x531, 0x420, 0x310, 0x200, 0x100, 0] {
                let events = rt.tick(Input::Neutral);
                assert!(
                    events.is_empty(),
                    "the scene and panel teardown wait: {events:?}"
                );
                assert_eq!(rt.cram().words, [expected; 64]);
                assert!(rt.cram().display, "display stays on through the passes");
                assert!(rt.scene_panel_sprites);
            }
            let events = rt.tick(Input::Neutral);
            assert!(!rt.cram().display);
            assert!(!rt.scene_panel_sprites);
            assert!(is_op(&events[0], SceneOp::PanelDestroyAll));
            assert!(
                events
                    .iter()
                    .skip(1)
                    .any(|e| is_op(e, SceneOp::PlaySound { id: 0x81 }))
            );
        }
    }
}

/// `$0743EA` returns through RunDialogue5; RefreshMap and Igglanova's
/// construction must stay dark until `$074494`'s own VDP_EnableDisplay.
/// Replacing that resume's window with Standard is the negative control.
fn alshline_dark_until_enable(mutate: bool) -> bool {
    let mut rt = alshline();
    let mut scene = *psiv_core::scene_for(EventIndex(0x8005)).unwrap();
    if mutate {
        let mut ops = scene.ops.to_vec();
        let op = ops
            .iter_mut()
            .find(|op| {
                matches!(
                    op,
                    SceneOp::RunDialogueResumeWithWindow {
                        window: DialogueWindow::Cutscene5
                    }
                )
            })
            .unwrap();
        *op = SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Standard,
        };
        scene.ops = Box::leak(ops.into_boxed_slice());
    }
    let scene = Box::leak(Box::new(scene));
    assert!(rt.install_runner(scene, scene.event));
    let mut resumed = false;
    let mut returned = false;
    let mut dark = true;
    let mut built = false;
    let mut enabled = false;
    for _ in 0..10_000 {
        let events = rt.tick(Input::Neutral);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneFaulted { .. }))
        );
        for event in &events {
            if matches!(event, RuntimeEvent::SceneDialogueResume) {
                resumed = true;
                rt.dialogue_closed();
            } else if matches!(event, RuntimeEvent::SceneDialogue { .. }) {
                rt.dialogue_closed();
            }
            if resumed && matches!(event, RuntimeEvent::MapChanged { .. }) {
                returned = true;
            }
            if returned
                && matches!(
                    event,
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::CreateFieldObject {
                            object_id: 0x188,
                            ..
                        },
                        ..
                    }
                )
            {
                built = true;
            }
            if returned
                && is_op(
                    event,
                    SceneOp::Presentation {
                        op: PresentationOp::EnableDisplay,
                    },
                )
            {
                enabled = true;
            }
        }
        if enabled && rt.cram().display {
            assert!(built);
            assert!(rt.cram().missing.is_empty());
            return dark;
        }
        if returned {
            dark &= !rt.cram().display;
        }
    }
    panic!("Alshline never enabled its display");
}

#[test]
fn alshline_load_and_igglanova_build_stay_dark_until_074494() {
    if !pack_present() {
        return;
    }
    assert!(alshline_dark_until_enable(false));
    assert!(
        !alshline_dark_until_enable(true),
        "negative control must catch a missing RunDialogue5 fade"
    );
}
