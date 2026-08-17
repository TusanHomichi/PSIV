//! Presentation-effect ordering at the runtime scene boundary.

use std::path::Path;

use psiv_core::{CharId, GameState, Input, RetailLocation, RetailSave, SceneOp, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};

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
            if let RuntimeEvent::ScenePresentation { op } = event {
                seen.push((tick, position, *op));
            }
            if matches!(event, RuntimeEvent::SceneDialogue { .. }) {
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
