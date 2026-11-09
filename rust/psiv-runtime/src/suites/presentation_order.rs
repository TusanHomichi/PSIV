//! Presentation-effect ordering at the runtime scene boundary.

use std::path::Path;

use crate::{Runtime, RuntimeEvent};
use psiv_core::{CharId, GameState, Input, RetailLocation, RetailSave, SceneOp, StepFrames};
use psiv_data::GameData;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime_with_four_party_at(map: u16) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut game = GameState::new();
    for (slot, id) in [CharId(0), CharId(1), CharId(2), CharId(4)]
        .into_iter()
        .enumerate()
    {
        game.set_party_slot(slot, Some(id))
            .expect("party slot exists");
    }
    Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: 16,
                char_y: 16,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime with seeded party starts")
}

#[test]
fn presentation_events_keep_scene_order_and_tick_boundaries() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    let mut runtime = runtime_with_four_party_at(0xAC);
    assert!(runtime.start_event(0x8007));
    let mut seen = Vec::new();
    for (tick, _) in (0..20_000).enumerate() {
        let events = runtime.tick(Input::Neutral);
        for (position, event) in events.iter().enumerate() {
            if let RuntimeEvent::ScenePresentation { op, .. } = event {
                seen.push((tick, position, *op));
            }
            if matches!(event, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        if !runtime.scene_active() {
            break;
        }
    }

    assert!(
        !seen.is_empty(),
        "the scene surfaced no presentation effects"
    );
    assert!(matches!(seen[0].2, SceneOp::InitVramAndCram));
    assert!(matches!(seen[1].2, SceneOp::FadeIn));
    assert!(matches!(
        seen[2].2,
        SceneOp::SetRenderSpritesInCutscene { enabled: true }
    ));
    assert!(seen.windows(2).all(|pair| {
        pair[0].0 <= pair[1].0 && (pair[0].0 != pair[1].0 || pair[0].1 < pair[1].1)
    }));
}

#[test]
fn panel_op_snapshot_survives_a_later_dialogue_close() {
    use crate::{Button, DialogueSignal, Pad};
    use psiv_core::{EventIndex, SceneRunner};
    use psiv_data::{DialogueEntry, Segment};

    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    static OPS: &[SceneOp] = &[
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::WaitFrames { frames: 100 },
    ];
    let mut runtime = runtime_with_four_party_at(0xAC);
    let entry = DialogueEntry {
        id: 0,
        text: String::new(),
        segments: vec![Segment::Text("x".into())],
        pages: Vec::new(),
    };
    assert!(runtime.dialogue.open_system_entry(&entry, &runtime.game));
    for _ in 0..100 {
        runtime.dialogue_frame(Pad::NEUTRAL);
        runtime.dialogue_tick();
        if runtime.dialogue_view().is_some_and(|view| view.dismissable) {
            break;
        }
    }
    assert!(runtime.dialogue_view().is_some_and(|view| view.dismissable));
    runtime.scene = Some(SceneRunner::new(OPS, Vec::new(), StepFrames::default()));
    runtime.scene_event = EventIndex(0x8007);
    // Deliberately exercise the runtime presentation seam with a scene tick
    // followed by a close, rather than claiming a connected campaign route.
    let events = runtime.tick(Input::Neutral);
    let signals = runtime.dialogue_frame(Pad::new(Button::Speak));
    assert!(
        signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::Closed { .. }))
    );
    let captured = events
        .iter()
        .find_map(|event| match event {
            RuntimeEvent::ScenePresentation {
                op: SceneOp::SetRenderSpritesInCutscene { enabled: true },
                panel_dialogue_mode,
            } => Some(*panel_dialogue_mode),
            _ => None,
        })
        .expect("panel write precedes the window's close on this frame");
    assert!(captured, "the op hides field sprites at its own position");
    assert!(
        !runtime.panel_dialogue_mode(),
        "the later close clears the live byte"
    );
}

#[test]
fn session_input_close_precedes_a_same_frame_panel_op() {
    use crate::{Button, DialogueSignal, Pad};
    use psiv_core::{EventIndex, SceneRunner};
    use psiv_data::{DialogueEntry, Segment};

    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut runtime = runtime_with_four_party_at(0xAC);
    let entry = DialogueEntry {
        id: 0,
        text: String::new(),
        segments: vec![Segment::Text("x".into())],
        pages: Vec::new(),
    };
    assert!(runtime.dialogue.open_system_entry(&entry, &runtime.game));
    // Session closes in the input half, before the scene writes again.
    runtime.scene_event = EventIndex(0x8007);
    static LOOP: &[SceneOp] = &[
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::Jump { to: 0 },
    ];
    runtime.scene = Some(SceneRunner::new(LOOP, Vec::new(), StepFrames::default()));
    let mut session = crate::Session::new(runtime);
    for tick in 0..100 {
        let pad = if tick % 2 == 0 {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        let frame = session.frame(pad);
        if frame
            .signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::Closed { .. }))
        {
            assert!(frame.events.iter().any(|event| matches!(
                event,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::SetRenderSpritesInCutscene { enabled: true },
                    panel_dialogue_mode: true,
                }
            )));
            assert!(session.runtime().panel_dialogue_mode());
            return;
        }
    }
    panic!("Session never closed the fixture window");
}
