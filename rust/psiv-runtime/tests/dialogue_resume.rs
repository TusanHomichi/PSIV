//! Scene dialogue returns to actor staging and waits for each resumed chunk.
use psiv_core::{CharId, Flag, GameState, Input, RetailLocation, RetailSave, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn after_igglanova_waits_for_both_resumed_chunks_before_releasing_the_field() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        return;
    }
    let mut game = GameState::new();
    game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        None,
        None,
    ]);
    // This fixture starts after victory, at the actual staging coordinates.
    game.set(Flag::event(7)).unwrap();
    game.set(Flag::event(11)).unwrap();
    game.set(Flag::event(13)).unwrap();
    game.set(Flag::event(14)).unwrap();
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x17,
                char_x: 256,
                char_y: 288,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    if let Some(dir) = std::env::var_os("PSIV_DIALOGUE_SMOKE_SAVE_DIR") {
        rt.save_slot(Path::new(&dir), 0).unwrap();
    }
    assert!(rt.start_event(0x25));
    let mut opened = false;
    for _ in 0..100 {
        if rt
            .tick(Input::Neutral)
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneDialogue { entry: 18 }))
        {
            opened = true;
            break;
        }
    }
    assert!(opened, "initial scene dialogue must be presented");
    for resume in 0..2 {
        rt.dialogue_closed();
        let mut saw_resume = false;
        for _ in 0..2000 {
            let events = rt.tick(Input::Neutral);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, RuntimeEvent::SceneFaulted { .. }))
            );
            if events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneDialogueResume))
            {
                saw_resume = true;
                break;
            }
        }
        assert!(saw_resume, "resume {resume} was never presented");
        let slot = if resume == 0 { 1 } else { 2 };
        let staged = *rt.scene_party_actor(slot).unwrap();
        for _ in 0..100 {
            let events = rt.tick(Input::Neutral);
            assert!(!events.iter().any(|e| matches!(
                e,
                RuntimeEvent::SceneDialogueResume | RuntimeEvent::SceneEnded
            )));
            assert!(rt.scene_active());
            assert!(!rt.game().is_set(Flag::event(15)));
            assert_eq!(rt.scene_party_actor(slot).unwrap(), &staged);
        }
    }
    rt.dialogue_closed();
    for _ in 0..2000 {
        rt.tick(Input::Neutral);
        if !rt.scene_active() {
            break;
        }
    }
    assert!(!rt.scene_active());
    assert!(rt.game().is_set(Flag::event(15)));
    assert_eq!(rt.game().party_members(), [CharId(1), CharId(0), CharId(2)]);
}
