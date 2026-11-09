//! The boarding-sound and saved-sound-word tests of `scene_runtime.rs`.
use super::*;
use psiv_core::{Cell, Direction, MapId, StepFrames};
use std::path::Path;

#[test]
fn all_three_boarding_frames_skip_same_track_and_emit_both_writes_on_mismatch() {
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = psiv_data::GameData::load(&pack).expect("pack loads");
    for event in [9, 10, 11] {
        for saved in [0x8D, 0x84] {
            let mut runtime = Runtime::new(
                data.clone(),
                0,
                Cell::new(114, 177),
                Direction::Down,
                StepFrames::default(),
            )
            .expect("field runtime");
            runtime.saved_sound_index = saved;
            assert!(runtime.start_event(event));
            let mut sound_writes = Vec::new();
            let mut retained = 0;
            for _ in 0..8 {
                for emitted in runtime.tick(Input::Neutral) {
                    match emitted {
                        RuntimeEvent::ScenePresentation {
                            op: SceneOp::PlaySound { id },
                            ..
                        } => sound_writes.push(SceneOp::PlaySound { id }),
                        RuntimeEvent::ScenePresentation {
                            op: SceneOp::SetSavedMusic { id },
                            ..
                        } => sound_writes.push(SceneOp::SetSavedMusic { id }),
                        RuntimeEvent::SceneMusicRetained => retained += 1,
                        _ => {}
                    }
                }
                if !runtime.scene_active() {
                    break;
                }
            }
            assert!(!runtime.scene_active(), "event {event} completed");
            let expected = if saved == 0x8D {
                vec![]
            } else {
                vec![
                    SceneOp::PlaySound { id: 0x8D },
                    SceneOp::SetSavedMusic { id: 0x8D },
                ]
            };
            assert_eq!(sound_writes, expected, "event {event}, saved {saved:#04x}");
            assert_eq!(retained, 1, "event {event} retains music at scene end");
            assert_eq!(runtime.saved_sound_index(), 0x8D);
        }
    }
}

#[test]
fn saved_sound_word_resets_at_start_and_follows_field_but_not_refresh_loads() {
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = psiv_data::GameData::load(&pack).expect("pack loads");
    let map_music = data.map(psiv_data::MapId(0)).unwrap().music.id;
    assert_ne!(map_music, 0);
    assert_ne!(map_music, 0x8D);
    let new_game = Runtime::new_game(data.clone(), StepFrames::default()).unwrap();
    assert_eq!(new_game.saved_sound_index(), 0, "START clears $ECEC");

    let mut runtime = Runtime::new(
        data,
        0,
        Cell::new(114, 177),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    assert_eq!(runtime.saved_sound_index(), map_music, "field entry");
    runtime.saved_sound_index = 0x8D;
    let refreshed = runtime
        .change_map_refresh(MapId(0), Cell::new(114, 177), Direction::Down, 0, 0)
        .unwrap();
    assert_eq!(runtime.saved_sound_index(), 0x8D, "RefreshMap keeps word");
    assert_eq!(refreshed.music_change(), None, "RefreshMap raises no edge");
    runtime.map_load_flags |= crate::map_change::LOAD_FLAG_AFTER_BATTLE;
    let after_battle = runtime
        .change_map(MapId(0), Cell::new(114, 177), Direction::Down)
        .unwrap();
    assert_eq!(runtime.saved_sound_index(), 0x8D, "battle keep bit");
    assert_eq!(
        after_battle.music_change(),
        None,
        "battle return raises no edge"
    );
    let field = runtime
        .change_map(MapId(0), Cell::new(114, 177), Direction::Down)
        .unwrap();
    assert_eq!(
        runtime.saved_sound_index(),
        map_music,
        "ordinary field load"
    );
    assert_eq!(
        field.music_change(),
        Some(map_music),
        "the write raises the edge"
    );
    // The same track again: the raw id equals the saved word, so no write
    // and no edge (`$051800`'s `cmp.b`), and no replay of the track.
    let again = runtime
        .change_map(MapId(0), Cell::new(114, 177), Direction::Down)
        .unwrap();
    assert_eq!(again.music_change(), None, "raw id equal to the saved word");
    assert_eq!(runtime.saved_sound_index(), map_music);
}
