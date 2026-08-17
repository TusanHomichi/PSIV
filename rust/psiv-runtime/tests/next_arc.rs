//! Headless coverage for the retail scene arc after Piata.
//!
//! These tests deliberately start the scenes through `Runtime::start_event`:
//! the trigger census proves which map entries dispatch them, while this file
//! proves the runtime can execute the resulting scene data, including map
//! recasts, battles and NPC movement landing in `FieldMap`.

use std::path::Path;

use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{
    Cell, CharId, Direction, Flag, GameState, Input, RetailLocation, RetailSave, SceneOp,
    StepFrames,
};
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

fn runtime_with_four_party_and_battles_at(map: u16) -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut game = GameState::new();
    for (slot, id) in [CharId(0), CharId(1), CharId(2), CharId(4)]
        .into_iter()
        .enumerate()
    {
        game.set_party_slot(slot, Some(id))
            .expect("party slot exists");
    }
    game.inventory_mut()
        .add(0x99)
        .expect("control key fits inventory");
    game.set(Flag::chest(0x0A)).expect("control-key chest flag");
    game.set(Flag::chest(0x09)).expect("psycho-wand chest flag");
    let mut runtime = Runtime::from_save(
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
    .expect("runtime with seeded party starts");
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    runtime.enable_battles(&files).expect("battles enable");
    runtime
}

fn relocate_with_state(runtime: &Runtime, map: u16) -> Runtime {
    Runtime::from_save(
        runtime.data().clone(),
        RetailSave {
            snapshot: runtime.game().snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: 0x1E0,
                char_y: 0x120,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime relocates with persistent state")
}

fn relocate_with_battles(runtime: &Runtime, map: u16) -> Runtime {
    let mut relocated = relocate_with_state(runtime, map);
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    relocated
        .enable_battles(&files)
        .expect("battles enable after relocation");
    relocated
}

fn drive_scene(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..20_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "event {event:#x} faulted: {item:?}"
            );
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
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "event {event:#x} faulted: {item:?}"
            );
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
        ("BioPlantAlarm", 0xA3, 0x0012),
        ("GirlsSneakingOut", 0x63, 0x0023),
        ("ChazHouse", 0x5E, 0x003B),
        ("LeavingChazHouse", 0x54, 0x003C),
        ("MeetingRika", 0xAC, 0x8007),
    ] {
        let mut runtime = if event == 0x8007 {
            runtime_with_four_party_at(map)
        } else {
            runtime_at(map)
        };
        let log = drive_scene(&mut runtime, event);
        assert!(
            log.iter()
                .any(|item| matches!(item, RuntimeEvent::SceneEnded)),
            "{name} emits SceneEnded"
        );
    }
}

#[test]
fn the_followup_chain_carries_alarm_through_rika_recruitment() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    // The alarm fires in BioPlant Part2 and the Rika trigger is in its later
    // B4 Part2 room. The map traversal between them is player-controlled, so
    // this single runtime starts at the latter room after the retail walk.
    let mut runtime = runtime_with_four_party_at(0xAC);
    let initial_inventory = *runtime.game().inventory().slots();
    let _ = drive_scene(&mut runtime, 0x0012);
    assert!(runtime.game().is_set(Flag::temp(0x08)));
    assert!(runtime.game().is_clear(Flag::event(0x34)));

    let log = drive_scene(&mut runtime, 0x8007);
    assert_eq!(
        runtime.map_id().0,
        0x00,
        "Rika tail: {:?}",
        &log[log.len().saturating_sub(12)..]
    );
    assert!(runtime.game().is_set(Flag::event(0x34)));
    assert!(runtime.game().is_set(Flag::event(0x35)));
    assert_eq!(runtime.game().party_slot(4).map(|id| id.0), Some(5));
    assert_eq!(*runtime.game().inventory().slots(), initial_inventory);
}

#[test]
fn the_retail_chain_runs_from_rika_to_zio_defeat() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }

    let mut runtime = runtime_with_four_party_and_battles_at(0xAC);

    let _ = drive_scene(&mut runtime, 0x8007);
    assert!(runtime.game().is_set(Flag::event(0x34)));
    assert!(runtime.game().is_set(Flag::event(0x35)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(0), CharId(1), CharId(2), CharId(4), CharId(5)]
    );
    assert_eq!(runtime.game().inventory().get(0), Some(0x99));

    let demi_rescue = drive_until_battle(&mut runtime, 0x8008);
    assert!(
        demi_rescue
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 4, .. }))
    );
    assert_eq!(
        resolve_scene_battle(&mut runtime),
        Outcome::Defeat,
        "the first Zio encounter is the cartridge's invulnerable loss"
    );
    assert!(runtime.game().is_set(Flag::event(0x42)));
    assert!(runtime.game().party_members().iter().all(|id| {
        runtime
            .game()
            .roster()
            .get(*id)
            .is_some_and(|stats| stats.curr_hp > 0)
    }));

    let _ = drive_scene(&mut runtime, 0x8009);
    assert!(runtime.game().is_set(Flag::event(0x47)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(0), CharId(4), CharId(5), CharId(6)]
    );
    assert_eq!(runtime.game().roster().get(CharId(1)).unwrap().status, 0);
    assert_eq!(runtime.game().roster().get(CharId(2)).unwrap().status, 0);

    let _ = drive_scene(&mut runtime, 0x002B);
    assert_eq!(runtime.game().vehicle_index(), 1);
    assert!(runtime.game().is_set(Flag::event(0x44)));
    assert!(!runtime.game().inventory().slots().contains(&0x99));
    assert!(runtime.game().inventory().slots().contains(&0x96));

    let _ = drive_scene(&mut runtime, 0x0006);
    assert!(runtime.game().is_set(Flag::event(0x43)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(0), CharId(4), CharId(5), CharId(6)]
    );

    let _ = drive_scene(&mut runtime, 0x002E);
    assert!(runtime.game().is_set(Flag::event(0x62)));
    assert_eq!(runtime.game().party_slot(4), Some(CharId(3)));
    assert_eq!(
        runtime.game().roster().get(CharId(3)).unwrap().equipment,
        [0x37, 0x00, 0x36, 0x38]
    );
    assert_eq!(
        runtime.game().roster().get(CharId(3)).unwrap().curr_hp,
        runtime.game().roster().get(CharId(3)).unwrap().max_hp
    );

    let psycho_chest = drive_until_battle(&mut runtime, 0x002F);
    assert!(
        psycho_chest
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 5, .. }))
    );
    assert!(runtime.game().is_set(Flag::event(0x69)));
    let _ = resolve_scene_battle(&mut runtime);

    let _ = drive_scene(&mut runtime, 0x800A);
    assert!(runtime.game().is_set(Flag::event(0x63)));
    assert!(runtime.game().is_set(Flag::event(0x67)));
    assert_eq!(runtime.map_id().0, 0x39);
    assert!(runtime.game().party_members().iter().all(|id| {
        runtime
            .game()
            .roster()
            .get(*id)
            .is_some_and(|stats| stats.curr_hp == stats.max_hp)
    }));

    // The route from Krup into Nurvus is on foot; the player dismounts the
    // Land Rover before the event battle, as the cartridge's field handoff
    // requires. Keep that player-controlled edge explicit in the arc test.
    runtime
        .set_vehicle_index(0)
        .expect("dismount before entering Nurvus");
    let nurvus = drive_until_battle(&mut runtime, 0x0034);
    assert!(
        nurvus
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 6, .. }))
    );
    assert!(runtime.game().is_set(Flag::event(0x65)));
    let _ = resolve_scene_battle(&mut runtime);

    let _ = drive_scene(&mut runtime, 0x800B);
    assert!(runtime.game().is_set(Flag::event(0x68)));
    assert!(runtime.game().is_set(Flag::event(0x66)));
    assert!(runtime.game().is_set(Flag::event(0x61)));
    assert_eq!(runtime.map_id().0, 0x00);
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(0), CharId(5), CharId(3)]
    );
    assert!(runtime.game().roster().get(CharId(0)).unwrap().curr_hp > 0);

    // The player walks from Motavia to Mota Spaceport before the next scene.
    // Re-enter through the same decoded snapshot so this is one persistent arc
    // test rather than a second synthetic game state.
    let mut post_zio = relocate_with_battles(&runtime, 0x0BF);
    let _ = drive_scene(&mut post_zio, 0x800D);
    assert_eq!(post_zio.map_id().0, 0x18D);
    assert_eq!(
        post_zio.game().party_members(),
        vec![CharId(0), CharId(5), CharId(3)]
    );
    assert!(post_zio.game().is_set(Flag::event(0x68)));

    // The player takes the elevator to Zelan F1, where the retail cutscene
    // promotes the map's Wren object into party slot 4 (zero-based slot 3).
    post_zio = relocate_with_battles(&post_zio, 0x18E);
    let _ = drive_scene(&mut post_zio, 0x800C);
    assert!(post_zio.game().is_set(Flag::event(0x70)));
    assert_eq!(
        post_zio.game().party_members(),
        vec![CharId(0), CharId(5), CharId(3), CharId(7)]
    );
    assert!(
        !post_zio.map().npcs()[0].active,
        "Wren's map object is gone"
    );

    // The next trip is the player-selected Zelan -> Kuran route. The scene
    // owns the Chaos Sorcerer battle and must set its flag before handing off.
    post_zio = relocate_with_battles(&post_zio, 0x18D);
    let sabotage = drive_until_battle(&mut post_zio, 0x800E);
    assert!(
        sabotage
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 8, .. }))
    );
    assert!(post_zio.game().is_set(Flag::event(0x71)));
    let _ = resolve_scene_battle(&mut post_zio);
    assert_eq!(post_zio.map_id().0, 0x18C);

    let _ = drive_scene(&mut post_zio, 0x800F);
    assert_eq!(post_zio.map_id().0, 0x14C);
    assert!(post_zio.game().is_set(Flag::event(0x85)));
    assert!(post_zio.game().is_set(Flag::event(0x88)));
    assert_eq!(
        post_zio.game().party_members(),
        vec![CharId(0), CharId(5), CharId(3), CharId(7), CharId(8)]
    );

    // Landale is reached on the player-controlled Dezolis walk. It is a
    // separate retail dispatch, so preserve the state and enter at Hangar.
    let mut landale = relocate_with_state(&post_zio, 0x15F);
    let _ = drive_scene(&mut landale, 0x8010);
    assert_eq!(landale.map_id().0, 0x001);
    assert!(landale.game().is_set(Flag::event(0x82)));

    // Kuran's three compact event bodies are the next tractable Dezo/Kuran
    // slice. The final one hands off to event battle 9.
    post_zio = relocate_with_battles(&landale, 0x190);
    let _ = drive_scene(&mut post_zio, 0x003D);
    assert!(post_zio.game().is_set(Flag::event(0x86)));
    let _ = drive_scene(&mut post_zio, 0x003E);
    assert!(post_zio.game().is_set(Flag::event(0x87)));
    let dark_force = drive_until_battle(&mut post_zio, 0x003F);
    assert!(
        dark_force
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 9, .. }))
    );
    assert!(post_zio.game().is_set(Flag::event(0x83)));
    let _ = resolve_scene_battle(&mut post_zio);

    let _ = drive_scene(&mut post_zio, 0x8011);
    assert_eq!(post_zio.map_id().0, 0x18E);
    assert!(post_zio.game().is_set(Flag::event(0x89)));
    assert!(post_zio.game().inventory().slots().contains(&0x97));
    assert!(!post_zio.game().inventory().slots().contains(&0x9A));
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
    assert!(
        seen.windows(2).all(|pair| {
            pair[0].0 <= pair[1].0 && (pair[0].0 != pair[1].0 || pair[0].1 < pair[1].1)
        }),
        "presentation effects lost runtime tick/order alignment"
    );
}

#[test]
fn the_shop_and_house_followups_write_their_retail_flags() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }

    let mut girls = runtime_at(0x63);
    let girls_inventory = *girls.game().inventory().slots();
    let _ = drive_scene(&mut girls, 0x0023);
    assert!(girls.game().is_set(Flag::event(0x46)));
    assert_eq!(*girls.game().inventory().slots(), girls_inventory);

    let mut house = runtime_at(0x5E);
    let house_inventory = *house.game().inventory().slots();
    let _ = drive_scene(&mut house, 0x003B);
    assert_eq!(house.map_id().0, 0x5E);
    assert!(house.game().is_set(Flag::temp(0x18)));
    assert_eq!(*house.game().inventory().slots(), house_inventory);

    let _ = drive_scene(&mut house, 0x003C);
    assert!(house.game().is_clear(Flag::temp(0x18)));
    assert_eq!(*house.game().inventory().slots(), house_inventory);
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
