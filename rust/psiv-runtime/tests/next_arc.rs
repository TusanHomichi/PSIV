//! Headless coverage for the retail scene arc after Piata.
//!
//! These tests deliberately start the scenes through `Runtime::start_event`:
//! the trigger census proves which map entries dispatch them, while this file
//! proves the runtime can execute the resulting scene data, including map
//! recasts, battles and NPC movement landing in `FieldMap`.

use std::path::Path;

use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{Cell, Direction, Flag, Input, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime_at(map: u16) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    Runtime::new(
        data,
        map,
        Cell::new(1, 1),
        Direction::Down,
        StepFrames::default(),
    )
    .expect("runtime starts")
}

fn runtime_with_battles_at(map: u16) -> Runtime {
    let mut runtime = runtime_at(map);
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    runtime.enable_battles(&files).expect("battles enable");
    runtime
}

fn drive_scene(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..20_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            if matches!(item, RuntimeEvent::SceneDialogue { .. }) {
                runtime.dialogue_closed();
            }
            assert!(
                !matches!(item, RuntimeEvent::SceneBattleStarted { .. }),
                "event {event:#x} unexpectedly owns a battle"
            );
        }
        log.extend(events);
        if !runtime.scene_active() {
            return log;
        }
    }
    panic!(
        "event {event:#x} did not finish; tail: {:?}",
        &log[log.len().saturating_sub(8)..]
    );
}

fn drive_until_battle(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..20_000 {
        let events = runtime.tick(Input::Neutral);
        let battle_started = events
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { .. }));
        for item in &events {
            if matches!(item, RuntimeEvent::SceneDialogue { .. }) {
                runtime.dialogue_closed();
            }
        }
        log.extend(events);
        if battle_started {
            return log;
        }
    }
    panic!("event {event:#x} never requested its battle");
}

fn resolve_scene_battle(runtime: &mut Runtime) -> Outcome {
    let mut outcome = None;
    for _ in 0..400 {
        let events = runtime
            .battle_round(&RoundOrders::attack_all())
            .expect("battle round resolves");
        for event in events {
            if let BattleEvent::Ended { outcome: result } = event {
                outcome = Some(result);
            }
        }
        if outcome.is_some() {
            break;
        }
    }
    let outcome = outcome.expect("scene battle ends");
    runtime.finish_battle_for_outcome(outcome, 0);

    for _ in 0..100 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            if matches!(item, RuntimeEvent::SceneDialogue { .. }) {
                runtime.dialogue_closed();
            }
        }
        if !runtime.scene_active() {
            return outcome;
        }
    }
    panic!("scene did not consume BattleFinished");
}

#[test]
fn every_next_arc_scene_reaches_its_return_edge() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    // One entry per registered scene which does not own a battle. The map is
    // the retail map that supplies the scene's NPC indices when it needs one.
    for (name, map, event) in [
        ("ProfHolt", 0x2B, 0x8002),
        ("MeetingRune", 0x41, 0x8003),
        ("MeetingDorin", 0x43, 0x0032),
        ("Dorin", 0x43, 0x8004),
        ("RuneFlaeli", 0xD8, 0x0027),
        ("AlshlineFound", 0x4A, 0x0028),
        ("ZemaIgglanovaDefeated", 0x24, 0x8006),
        ("ZemaOldMan", 0x24, 0x008B),
        ("ZemaOldManAfterMission", 0x24, 0x008C),
        ("MeetingSaya", 0x3A, 0x000D),
        ("TonoeBasementDoor", 0x42, 0x0033),
    ] {
        let mut runtime = runtime_at(map);
        let log = drive_scene(&mut runtime, event);
        assert!(
            log.iter()
                .any(|item| matches!(item, RuntimeEvent::SceneEnded)),
            "{name} emits SceneEnded"
        );
    }
}

#[test]
fn dorin_motion_lands_in_the_live_field_map() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut runtime = runtime_at(0x43);
    let _ = drive_scene(&mut runtime, 0x8004);

    assert_eq!(runtime.map_id().0, 0x43, "Dorin refreshes TonoeGryzHouse");
    assert_eq!(runtime.map().npcs()[6].cell, Cell::new(0x1F, 0x31));
    assert_eq!(runtime.map().npcs()[0].cell, Cell::new(0x1E, 0x31));
    assert!(runtime.game().is_set(Flag::event(0x30)));
}

#[test]
fn the_arc_walks_from_holt_through_tonoe_to_birth_valley() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }
    let mut runtime = runtime_with_battles_at(0x2B);

    let _ = drive_scene(&mut runtime, 0x8002);
    assert_eq!(runtime.map_id().0, 0x24);
    assert!(runtime.game().is_set(Flag::event(0x10)));

    let _ = drive_scene(&mut runtime, 0x8003);
    assert!(runtime.game().is_set(Flag::event(0x11)));

    let _ = drive_scene(&mut runtime, 0x0032);
    assert!(runtime.game().is_set(Flag::event(0x36)));

    let _ = drive_scene(&mut runtime, 0x8004);
    assert_eq!(runtime.map_id().0, 0x43);
    assert!(runtime.game().is_set(Flag::event(0x30)));

    let _ = drive_scene(&mut runtime, 0x0027);
    assert!(runtime.game().is_set(Flag::event(0x13)));

    let _ = drive_scene(&mut runtime, 0x0028);
    assert!(runtime.game().is_set(Flag::event(0x32)));

    let _ = drive_until_battle(&mut runtime, 0x8005);
    assert_eq!(
        resolve_scene_battle(&mut runtime),
        Outcome::Victory,
        "headless arc wins its battle"
    );
    assert!(runtime.game().is_set(Flag::event(0x33)));

    let _ = drive_scene(&mut runtime, 0x8006);
    assert_eq!(runtime.map_id().0, 0x24);
    assert!(runtime.game().is_set(Flag::event(0x37)));
}

#[test]
fn servant_battle_uses_the_existing_battle_requested_path() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }
    let mut runtime = runtime_with_battles_at(0x24);
    let _ = drive_until_battle(&mut runtime, 0x008A);
    assert!(runtime.battle_active());
    let _ = resolve_scene_battle(&mut runtime);
    assert!(runtime.game().is_set(Flag::event(0xB2)));
}
