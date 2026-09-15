//! Dorin's FF answers return normally; F7 yields keep all four resumed sections.
use psiv_core::{
    CharId, Direction, Flag, GameState, Input, RetailLocation, RetailSave, StepFrames,
};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};
use std::path::Path;

fn fixture() -> Option<Runtime> {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").is_file() {
        return None;
    }
    let data = GameData::load(pack).unwrap();
    let mut initial = Runtime::new_game(data.clone(), StepFrames::default()).unwrap();
    initial
        .enable_battles(&BattleFiles::load(pack).unwrap())
        .unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(3)),
        None,
    ]);
    game.set(Flag::event(0x11)).unwrap();
    Some(
        Runtime::from_save(
            data,
            RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: 0x43,
                    char_x: 30 * 16,
                    char_y: 32 * 16,
                },
            },
            StepFrames::default(),
        )
        .unwrap(),
    )
}

#[test]
fn ordinary_answers_do_not_start_the_punch_or_set_the_dorin_flag() {
    let Some(mut rt) = fixture() else {
        return;
    };
    let before = rt.members();
    assert!(rt.start_event(0x32));
    let mut ended = false;
    for _ in 0..100 {
        for event in rt.tick(Input::Neutral) {
            match event {
                RuntimeEvent::SceneDialogue { entry: 0x18 } => rt.dialogue_ended(),
                RuntimeEvent::SceneDialogueResume => panic!("FF has no saved cursor"),
                RuntimeEvent::SceneEnded => ended = true,
                RuntimeEvent::SceneFaulted { .. } => panic!("{event:?}"),
                _ => {}
            }
        }
        if ended {
            break;
        }
    }
    assert!(ended);
    assert!(rt.game().is_clear(Flag::event(0x36)));
    assert_eq!(rt.members(), before);
}

#[test]
fn the_punch_moves_alys_and_resumes_every_section_before_the_followon_flag() {
    let Some(mut rt) = fixture() else {
        return;
    };
    let dorin_at = rt.map().npcs()[0].cell;
    assert!(rt.start_event(0x32));
    let mut resumes = 0;
    let mut ended = false;
    for _ in 0..2000 {
        for event in rt.tick(Input::Neutral) {
            match event {
                RuntimeEvent::SceneDialogue { entry: 0x18 } => rt.dialogue_closed(),
                RuntimeEvent::SceneDialogueResume => {
                    resumes += 1;
                    assert!(rt.game().is_clear(Flag::event(0x36)));
                    assert_eq!(
                        rt.map().npcs()[0].cell,
                        dorin_at,
                        "Dorin stays in his chair"
                    );
                    if resumes == 4 {
                        rt.dialogue_ended();
                    } else {
                        rt.dialogue_closed();
                    }
                }
                RuntimeEvent::SceneEnded => ended = true,
                RuntimeEvent::SceneFaulted { .. } => panic!("{event:?}"),
                _ => {}
            }
        }
        if ended {
            break;
        }
    }
    assert!(ended);
    assert_eq!(resumes, 4);
    assert!(rt.game().is_set(Flag::event(0x36)));
    assert_eq!(rt.state().cell(), psiv_core::Cell::new(32, 31));
    assert_eq!(rt.map().npcs()[0].cell, dorin_at);
    assert_eq!(rt.map().npcs()[0].facing, Direction::Down);
}
