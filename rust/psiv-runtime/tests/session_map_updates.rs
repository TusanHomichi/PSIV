//! Map updates through Session::start and pad-only Session::frame.
//! Retail citations and fixture limits: docs/field/MAP_UPDATES.md.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use psiv_core::battle::{Lcg41, Rng2, Rolls};
use psiv_core::{
    CameraPlane, Cell, CharId, Flag, GameState, RetailLocation, RetailSave, RetailSlot,
};
use psiv_data::{BattleFiles, CellPos, GameData, MapId, UpdateProgram};
use psiv_runtime::{Button, Pad, RuntimeEvent, Session};

fn pack_path() -> PathBuf {
    std::env::var_os("PSIV_RUNTIME_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/x41-pack"))
}

fn pack() -> &'static (GameData, BattleFiles) {
    static PACK: OnceLock<(GameData, BattleFiles)> = OnceLock::new();
    PACK.get_or_init(|| {
        let path = control_pack(&pack_path());
        (
            GameData::load(&path).expect("x41 pack loads"),
            BattleFiles::load(&path).expect("battles load"),
        )
    })
}

// Opt-in mutation controls affect only this test binary's copied map JSON.
// Commands expecting failure are recorded in the ledger; the source pack is
// never edited and the session has no mutation/input seam after construction.
fn control_pack(source: &Path) -> PathBuf {
    let Ok(control) = std::env::var("PSIV_X41_NEGATIVE_CONTROL") else {
        return source.to_owned();
    };
    assert!(matches!(control.as_str(), "omit_dispatch" | "water_phase"));
    let target = std::env::temp_dir().join(format!("psiv-x41-{control}-{}", std::process::id()));
    std::fs::create_dir_all(target.join("maps")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(source.join("manifest.json")).unwrap())
            .unwrap();
    let id = if control == "omit_dispatch" { 398 } else { 84 };
    let map_name = manifest["maps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == id)
        .unwrap()["json"]
        .as_str()
        .unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "maps" {
            std::os::unix::fs::symlink(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
    for entry in std::fs::read_dir(source.join("maps")).unwrap() {
        let entry = entry.unwrap();
        let name = PathBuf::from("maps").join(entry.file_name());
        if name != Path::new(map_name) {
            std::os::unix::fs::symlink(entry.path(), target.join(name)).unwrap();
        }
    }
    let mut map: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(source.join(map_name)).unwrap()).unwrap();
    if control == "omit_dispatch" {
        map.as_object_mut().unwrap().remove("map_updates");
    } else {
        map["map_updates"][0]["program"]["cycles"][0]["frames"]
            .as_array_mut()
            .unwrap()
            .rotate_left(1);
    }
    std::fs::write(
        target.join(map_name),
        serde_json::to_vec_pretty(&map).unwrap(),
    )
    .unwrap();
    eprintln!("negative control {control}: {}", target.display());
    target
}

fn fresh_game() -> GameState {
    let session = Session::start(pack().0.clone())
        .new_game()
        .expect("START initializer");
    let mut game = GameState::from_snapshot(&session.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    game
}

fn ground(map: u16, y: Option<u16>) -> Cell {
    let record = pack().0.map(MapId(map)).unwrap();
    for row in 1..record.dimensions.height_cells {
        if y.is_some_and(|y| row != u32::from(y)) {
            continue;
        }
        for x in 0..record.dimensions.width_cells {
            let pos = CellPos::new(x, row);
            if record.collision_at(pos) == Some(psiv_data::CollisionType::Normal)
                && record.warps_at(pos).next().is_none()
                && !record.npcs.iter().any(|n| n.x_cell == x && n.y_cell == row)
            {
                return Cell::new(x as u16, row as u16);
            }
        }
    }
    panic!("map {map:#x} has no ground at {y:?}");
}

fn session_at(map: u16, cell: Cell, game: &GameState) -> Session {
    Session::start(pack().0.clone())
        .with_battles(pack().1.clone())
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            // The existing save boundary stores cell-scaled Y. Field rules below
            // read PixelPos::from_cell, including the cartridge's -16 correction.
            location: RetailLocation {
                map_index: map,
                char_x: cell.x * 16,
                char_y: cell.y * 16,
                ..RetailLocation::default()
            },
        })
        .expect("session starts")
}

fn tick(session: &mut Session, pad: Pad) {
    let frame = session.frame(pad);
    assert!(
        !frame.events.iter().any(|e| matches!(
            e,
            RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::WarpUnmapped { .. }
        )),
        "fault: {:?}",
        frame.events
    );
}

#[test]
fn zelan_canceller_requires_the_chest_bit_not_inventory_or_temporary_flags() {
    for (bank, positive) in [(0, true), (1, false), (2, false)] {
        let mut game = fresh_game();
        let _ = game.clear(Flag::event(0x72));
        let _ = game.clear(Flag::chest(0x0B));
        let _ = game.set(match bank {
            0 => Flag::chest(0x0B),
            1 => Flag::temp(0x0B),
            _ => Flag::chest(0x0A),
        });
        let mut session = session_at(398, ground(398, None), &game);
        assert!(
            session.runtime().game().is_clear(Flag::event(0x72)),
            "not a map-load effect"
        );
        tick(&mut session, Pad::NEUTRAL);
        assert_eq!(
            session.runtime().game().is_set(Flag::event(0x72)),
            positive,
            "$2D condition, bank case {bank}"
        );
        assert_eq!(session.runtime().map_updates().last_frame, Some(1));
    }
}

#[test]
fn aiedo_water_follows_raw_table_columns_and_absolute_eight_frame_clock() {
    let rom = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Phantasy Star IV (USA).md"),
    )
    .unwrap();
    let mut session = session_at(0x54, ground(0x54, None), &fresh_game());
    if let Some(directory) = std::env::var_os("PSIV_X41_NATIVE_SAVE") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let fixture = session_at(0x54, Cell::new(34, 34), &fresh_game());
        std::fs::write(
            directory.join("slot_1.sram"),
            fixture.runtime().slot_bytes(0).unwrap(),
        )
        .unwrap();
    }
    let mut expected = session.runtime().map_updates().palette[26..30].to_vec();
    for frame in 1..=64 {
        tick(&mut session, Pad::NEUTRAL);
        if frame % 8 == 0 {
            for (color, word) in expected.iter_mut().enumerate() {
                let at = 0x54B0C + ((frame & 0x18) >> 2) + color * 8;
                *word = u16::from_be_bytes([rom[at], rom[at + 1]]);
            }
        }
        assert_eq!(
            &session.runtime().map_updates().palette[26..30],
            expected,
            "water frame {frame}"
        );
    }
}

#[test]
fn canceller_chest_input_defers_the_story_write_until_its_window_closes() {
    let mut game = fresh_game();
    let _ = game.clear(Flag::chest(0x0B));
    let _ = game.clear(Flag::event(0x72));
    let mut session = session_at(398, Cell::new(49, 50), &game);
    tick(&mut session, Pad::new(Button::Up));
    tick(&mut session, Pad::NEUTRAL);
    tick(&mut session, Pad::new(Button::Speak));
    assert!(
        session.runtime().loot_state().is_some(),
        "the ordinary chest window"
    );
    assert!(session.runtime().game().is_set(Flag::chest(0x0B)));
    assert!(session.runtime().game().is_clear(Flag::event(0x72)));
    for _ in 0..16 {
        tick(&mut session, Pad::NEUTRAL);
    }
    assert!(
        session.runtime().game().is_clear(Flag::event(0x72)),
        "loot owns these frames"
    );
    tick(&mut session, Pad::new(Button::Speak));
    tick(&mut session, Pad::NEUTRAL);
    assert!(session.runtime().loot_state().is_none());
    assert!(session.runtime().game().is_set(Flag::event(0x72)));
}

#[test]
fn menu_and_dialogue_frames_park_updates_but_not_the_absolute_clock() {
    let mut session = session_at(0x54, ground(0x54, None), &fresh_game());
    for _ in 0..8 {
        tick(&mut session, Pad::NEUTRAL);
    }
    let before = session.runtime().map_updates().clone();
    tick(&mut session, Pad::new(Button::Camp));
    assert!(session.camp_view().is_some());
    for _ in 0..16 {
        tick(&mut session, Pad::NEUTRAL);
    }
    assert_eq!(session.runtime().map_updates(), &before);
    tick(&mut session, Pad::new(Button::Cancel));
    tick(&mut session, Pad::NEUTRAL);
    assert!(session.runtime().map_updates().last_frame.unwrap() > 8);
    tick(&mut session, Pad::new(Button::Speak));
    assert!(
        session.runtime().dialogue_open(),
        "ordinary nothing-here window"
    );
    let before = session.runtime().map_updates().clone();
    for _ in 0..40 {
        tick(&mut session, Pad::NEUTRAL);
    }
    assert_eq!(session.runtime().map_updates(), &before);
}

#[test]
fn tower_chest_conjunctions_set_story_flags_only_when_all_bits_hold() {
    for (map, required, event) in [
        (246, vec![0xA1, 0xA2, 0xA3], 0xD5),
        (251, vec![0xA4, 0xA5], 0xD3),
    ] {
        for missing in 0..=required.len() {
            let mut game = fresh_game();
            let _ = game.clear(Flag::event(event));
            for (i, &chest) in required.iter().enumerate() {
                let _ = game.write(Flag::chest(chest), i != missing);
            }
            let mut session = session_at(map, ground(map, None), &game);
            tick(&mut session, Pad::NEUTRAL);
            // Entry events own their dispatch frame before RunMapUpdates.
            // The tower's ensuing MoveCamera/Wait loop does call the table.
            for _ in 0..3 {
                if session.runtime().map_updates().last_frame.is_some() {
                    break;
                }
                tick(&mut session, Pad::NEUTRAL);
            }
            assert_eq!(
                session.runtime().game().is_set(Flag::event(event)),
                missing == required.len(),
                "map {map}, missing {missing}, cell {:?}, scene {}, updates {:?}",
                session.runtime().state().cell(),
                session.runtime().scene_active(),
                session.runtime().map_updates()
            );
        }
    }
}

#[test]
fn unused_chest_clear_and_music_writes_reach_persistent_state() {
    let mut game = fresh_game();
    let _ = game.set(Flag::chest(0xA9));
    let mut chest = session_at(198, ground(198, None), &game);
    tick(&mut chest, Pad::NEUTRAL);
    assert!(chest.runtime().game().is_clear(Flag::chest(0xA9)));
    let mut music = session_at(140, ground(140, None), &game);
    tick(&mut music, Pad::NEUTRAL);
    let slot = RetailSlot::from_bytes(&music.runtime().slot_bytes(0).unwrap(), 0)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(slot.location.map_index_2, 0xFFFF);
    assert_eq!(music.runtime().saved_sound_index(), 0xA7);
}

#[test]
fn camera_damping_runs_between_driver_latch_and_camera_commit() {
    let mut session = session_at(132, Cell::new(30, 20), &fresh_game());
    tick(&mut session, Pad::new(Button::Right));
    let camera = session.runtime().camera();
    let fg = camera.raw_step_on(CameraPlane::Foreground).0;
    let bg = camera.raw_step_on(CameraPlane::Background).0;
    assert!(fg > 0, "the pad moved the driver past the camera threshold");
    assert_eq!(bg, fg - (fg >> 4), "$05 signed 32-bit BG step");
}

#[test]
fn tunnels_and_sine_scroll_emit_exact_sparse_presentation_writes() {
    let mut tunnels = session_at(134, ground(134, None), &fresh_game());
    tick(&mut tunnels, Pad::NEUTRAL);
    let view = tunnels.runtime().map_updates();
    assert_eq!(view.scroll_writes.len(), 64);
    assert_eq!(view.scroll_writes[0].offset, 0);
    assert_eq!(view.scroll_writes[31].offset, 31 * 32);
    assert_eq!(view.scroll_writes[32].offset, 2);
    let mut sine = session_at(409, ground(409, None), &fresh_game());
    tick(&mut sine, Pad::NEUTRAL);
    let view = sine.runtime().map_updates();
    assert_eq!(view.scroll_writes.len(), 512);
    let record = sine.runtime().map_record().unwrap();
    let UpdateProgram::Scroll { samples, .. } = &record.map_updates.as_ref().unwrap()[0].program
    else {
        panic!("scroll recipe")
    };
    for row in 0..256 {
        assert_eq!(
            view.scroll_writes[row * 2 + 1].word,
            u16::from(samples[(1 + row * 0x60) & 255] >> 3)
        );
    }
}

#[test]
fn rykros_countdown_and_single_rng2_reload_share_the_field_seed() {
    let mut session = session_at(2, ground(2, None), &fresh_game());
    for frame in 1..=272 {
        let seed = session.runtime().rng_seed();
        let mut expected_rng = Lcg41::new(seed);
        expected_rng.step();
        expected_rng.step();
        let expected_reload = if frame == 272 {
            Some(Rng2::with_surrogate(&mut expected_rng, frame).next_roll() as u8)
        } else {
            None
        };
        tick(&mut session, Pad::NEUTRAL);
        assert_eq!(
            session.runtime().rng_seed(),
            expected_rng.seed(),
            "draw count/stream on frame {frame}"
        );
        let counters = session.runtime().map_updates().counters;
        if frame == 1 {
            assert_eq!(counters[0], 255, "initial zero byte underflows");
        }
        if frame == 256 {
            assert_eq!(counters[1], 1);
        }
        if let Some(reload) = expected_reload {
            assert_eq!(counters[..2], [reload, 0]);
        }
    }
}

#[test]
fn positional_encounter_override_preserves_the_unsigned_inclusive_boundary() {
    for (cell_y, enabled) in [(23, false), (24, true)] {
        let mut session = session_at(415, ground(415, Some(cell_y)), &fresh_game());
        tick(&mut session, Pad::NEUTRAL);
        assert_eq!(
            session.runtime().map_updates().random_battles,
            Some(enabled),
            "curr_y word {}, cell {:?}, actor {:?}",
            (cell_y - 1) * 16,
            session.runtime().state().cell(),
            session.runtime().scene_party_actor(0)
        );
    }
}

#[test]
fn missing_dma_and_conveyor_buffers_are_reported_without_invented_effects() {
    for (map, index) in [(202, 0x11), (199, 0x12), (412, 0x22)] {
        let mut session = session_at(map, ground(map, None), &fresh_game());
        tick(&mut session, Pad::NEUTRAL);
        let skipped = &session.runtime().map_updates().unsupported;
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].index, index);
        assert!(!skipped[0].missing.is_empty());
    }
}
