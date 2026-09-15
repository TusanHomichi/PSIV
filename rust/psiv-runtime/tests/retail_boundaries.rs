//! Headless proofs for the deterministic retail boundary scenes.

use std::path::Path;

use psiv_core::{CharId, Flag, GameState, Input, RetailLocation, RetailSave, SceneOp, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime_with_party(map: u16, party: [Option<CharId>; 5]) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut game = GameState::new();
    game.set_party(party);
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
    .expect("boundary runtime starts")
}

fn drive_scene(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..50_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "{item:?}"
            );
            if matches!(item, RuntimeEvent::SceneChoiceRequested) {
                runtime.dialogue_choice(true);
            }
            if matches!(
                item,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        log.extend(events);
        if !runtime.scene_active() {
            return log;
        }
    }
    panic!("event {event:#x} did not finish");
}

#[test]
fn raja_sick_transcription_reaches_the_retail_handoff() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    let mut runtime = runtime_with_party(
        0x14C,
        [
            Some(CharId(0)),
            Some(CharId(3)),
            Some(CharId(5)),
            Some(CharId(7)),
            Some(CharId(8)),
        ],
    );
    let log = drive_scene(&mut runtime, 0x8012);

    assert!(runtime.game().is_set(Flag::event(0x94)));
    assert_eq!(runtime.map_id().0, 0x134);
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(0), CharId(3), CharId(5), CharId(7)]
    );
    assert!(log.iter().any(|item| {
        matches!(
            item,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::Presentation {
                    op: psiv_core::PresentationOp::RajaSickArrangeParty { .. }
                }
            }
        )
    }));
}

#[test]
fn rykros_transcription_reaches_the_tower_flag() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    let mut runtime = runtime_with_party(0x002, [Some(CharId(0)), None, None, None, None]);
    let log = drive_scene(&mut runtime, 0x801B);

    assert!(runtime.game().is_set(Flag::event(0xD0)));
    assert!(log.iter().any(|item| {
        matches!(
            item,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::PanelCreate { id: 0x18F }
            }
        )
    }));
    assert!(log.iter().any(|item| {
        matches!(
            item,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::Presentation {
                    op: psiv_core::PresentationOp::RykrosPaletteCycle { .. }
                }
            }
        )
    }));
}
