//! Headless coverage for the retail scene arc after Piata: the campaign chain.
//!
//! This target carries one retail-shaped state from the title through every
//! registered scene to the `$8021` ending, relocating between maps where the
//! player's own route (a chest, a control key, a vehicle) is skipped, and
//! supplying those prerequisites as explicit fixtures. The per-scene checks
//! live in `next_arc_scenes.rs` and the arc's scripted battles in
//! `next_arc_battles.rs`.
//!
//! The chain deliberately starts its scenes through `Runtime::start_event`:
//! the trigger census proves which map entries dispatch them, while this file
//! proves the runtime can execute the resulting scene data, including map
//! recasts, battles and NPC movement landing in `FieldMap`.

use std::path::Path;

use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{
    Cell, CharId, Flag, GameState, Input, RetailLocation, RetailSave, SceneOp, StepFrames,
};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime_with_title_fixture() -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let mut game = GameState::new();
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    let mut runtime = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x13,
                char_x: 48 * 16,
                char_y: 19 * 16,
            },
        },
        StepFrames::default(),
    )
    .expect("title fixture starts");
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

fn relocate_with_edit<F>(runtime: &Runtime, map: u16, edit: F) -> Runtime
where
    F: FnOnce(&mut GameState),
{
    let mut game = GameState::from_snapshot(&runtime.game().snapshot());
    edit(&mut game);
    Runtime::from_save(
        runtime.data().clone(),
        RetailSave {
            snapshot: game.snapshot(),
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
    .expect("runtime relocates with edited persistent state")
}

fn relocate_with_edit_and_battles<F>(runtime: &Runtime, map: u16, edit: F) -> Runtime
where
    F: FnOnce(&mut GameState),
{
    let mut relocated = relocate_with_edit(runtime, map, edit);
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    relocated
        .enable_battles(&files)
        .expect("battles enable after edited relocation");
    relocated
}

fn harden_arc_battle_roster(game: &mut GameState) {
    // The arc test checks scene state transitions, not the balance curve. Give
    // every seated character deterministic late-game combat stats so a long
    // chain cannot die because the preceding synthetic walk left one member
    // at an early-game level or low HP.
    for raw in 0..11 {
        if let Some(stats) = game.roster_mut().get_mut(CharId(raw)) {
            stats.curr_hp = u16::MAX;
            stats.max_hp = u16::MAX;
            stats.curr_tp = u16::MAX;
            stats.max_tp = u16::MAX;
            stats.status = 0;
            stats.strength.battle = u8::MAX;
            stats.dexterity.battle = u8::MAX;
            stats.attack.derived = 1_000;
            stats.attack.battle = 1_000;
            stats.defence.derived = 1_000;
            stats.defence.battle = 1_000;
            stats.mental_defence.derived = 1_000;
            stats.mental_defence.battle = 1_000;
        }
    }
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
    for _ in 0..50_000 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            assert!(
                !matches!(item, RuntimeEvent::SceneFaulted { .. }),
                "event {event:#x} faulted: {item:?}; party={:?}; actors={:?}",
                runtime.game().party_members(),
                runtime.scene_actors()
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
            if matches!(
                item,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::WaitForStart
                }
            ) {
                runtime.ending_continue();
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
        if battle_started {
            return log;
        }
    }
    panic!("event {event:#x} never requested its battle");
}

fn use_psycho_wand(runtime: &mut Runtime) {
    use psiv_core::battle::{Command, ItemSource};
    let slot = runtime
        .game()
        .inventory()
        .slots()
        .iter()
        .position(|id| *id == 0x39)
        .expect("Psycho Wand is carried");
    let mut orders = vec![Command::Defend; 5];
    orders[0] = Command::Item {
        item: 0x39,
        source: ItemSource::Inventory(slot as u8),
        target: None,
    };
    let events = runtime
        .battle_round(&RoundOrders::Commands(orders))
        .unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemyStatsReloaded { enemy_id: 140, .. }))
    );
    assert_eq!(runtime.game().inventory().get(slot), Some(0x39));
}

fn resolve_scene_battle(runtime: &mut Runtime) -> Outcome {
    resolve_scene_battle_with_limit(runtime, 400)
}

fn resolve_scene_battle_with_limit(runtime: &mut Runtime, max_rounds: usize) -> Outcome {
    let mut outcome = None;
    let mut last_events = Vec::new();
    for _ in 0..max_rounds {
        let events = runtime
            .battle_round(&RoundOrders::attack_all())
            .expect("battle round resolves");
        last_events = events.clone();
        for event in events {
            if let BattleEvent::Ended { outcome: result } = event {
                outcome = Some(result);
            }
        }
        if outcome.is_some() {
            break;
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        panic!(
            "scene battle ends; active={} party_len={} last={:?}",
            runtime.battle_active(),
            runtime.battle_party().len(),
            last_events
        )
    });
    runtime.finish_battle_for_outcome(outcome, 0);

    for _ in 0..100 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
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
        if !runtime.scene_active() {
            return outcome;
        }
    }
    panic!("scene did not consume BattleFinished");
}

#[test]
fn synthetic_scene_chain_reaches_ending_with_explicit_battle_fixtures() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }

    // This is scene-graph coverage, not a playable campaign. Relocations skip
    // walking/encounters and explicit edits below supply keys and combat
    // stats. Native connected routes are recorded separately.
    let mut runtime = runtime_with_title_fixture();
    let _ = drive_scene(&mut runtime, 0x009F);
    assert!(runtime.game().is_set(Flag::event(0x07)));
    assert_eq!(runtime.game().party_members(), vec![CharId(0)]);
    let _ = drive_scene(&mut runtime, 0x00A0);
    assert!(runtime.game().is_set(Flag::event(0x15)));
    let _ = drive_scene(&mut runtime, 0x0003);
    assert!(runtime.game().is_set(Flag::event(0x08)));
    assert_eq!(runtime.game().party_members(), vec![CharId(1), CharId(0)]);

    runtime = relocate_with_battles(&runtime, 0x14);
    let _ = drive_scene(&mut runtime, 0x8001);
    assert!(runtime.game().is_set(Flag::event(0x09)));
    let _ = drive_scene(&mut runtime, 0x000F);
    assert!(runtime.game().is_set(Flag::event(0x0E)));
    runtime = relocate_with_battles(&runtime, 0x12);
    let _ = drive_scene(&mut runtime, 0x0004);
    assert!(runtime.game().is_set(Flag::event(0x0A)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(1), CharId(0), CharId(2)]
    );
    runtime = relocate_with_battles(&runtime, 0x17);
    let _ = drive_scene(&mut runtime, 0x000C);
    assert!(runtime.game().is_set(Flag::event(0x0D)));
    let _ = drive_until_battle(&mut runtime, 0x006B);
    assert_eq!(resolve_scene_battle(&mut runtime), Outcome::Victory);
    assert!(runtime.game().is_set(Flag::event(0x0B)));
    let _ = drive_scene(&mut runtime, 0x0025);
    assert!(runtime.game().is_set(Flag::event(0x0F)));
    runtime = relocate_with_battles(&runtime, 0x14);
    let _ = drive_scene(&mut runtime, 0x0026);
    assert!(runtime.game().is_set(Flag::event(0x0C)));
    let _ = drive_scene(&mut runtime, 0x009E);
    assert_eq!(runtime.map_id().0, 0x10);
    runtime = relocate_with_battles(&runtime, 0x02B);
    let _ = drive_scene(&mut runtime, 0x8002);
    assert!(runtime.game().is_set(Flag::event(0x10)));
    runtime = relocate_with_battles(&runtime, 0x40);
    let _ = drive_scene(&mut runtime, 0x8003);
    assert!(runtime.game().is_set(Flag::event(0x11)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(1), CharId(0), CharId(2), CharId(3)]
    );
    runtime = relocate_with_battles(&runtime, 0xD8);
    let _ = drive_scene(&mut runtime, 0x0027);
    assert!(runtime.game().is_set(Flag::event(0x13)));
    assert!(runtime.map().is_walkable(Cell::new(31, 31)));
    runtime = relocate_with_battles(&runtime, 0x43);
    let _ = drive_scene(&mut runtime, 0x0032);
    assert!(runtime.game().is_set(Flag::event(0x36)));
    let _ = drive_scene(&mut runtime, 0x8004);
    assert!(runtime.game().is_set(Flag::event(0x30)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(1), CharId(0), CharId(2), CharId(4)]
    );
    let _ = drive_scene(&mut runtime, 0x0028);
    assert!(runtime.game().is_set(Flag::event(0x32)));
    let _ = drive_until_battle(&mut runtime, 0x8005);
    assert_eq!(resolve_scene_battle(&mut runtime), Outcome::Victory);
    assert!(runtime.game().is_set(Flag::event(0x33)));
    let _ = drive_scene(&mut runtime, 0x8006);
    assert!(runtime.game().is_set(Flag::event(0x37)));
    runtime =
        relocate_with_edit_and_battles(&runtime, runtime.map_id().0, harden_arc_battle_roster);
    let _ = drive_until_battle(&mut runtime, 0x008A);
    assert_eq!(resolve_scene_battle(&mut runtime), Outcome::Victory);
    assert!(runtime.game().is_set(Flag::event(0xB2)));
    let _ = drive_scene(&mut runtime, 0x008B);
    assert!(runtime.game().is_set(Flag::event(0xB3)));
    let _ = drive_scene(&mut runtime, 0x008C);
    assert!(runtime.game().is_set(Flag::event(0xB7)));
    runtime = relocate_with_battles(&runtime, 0x03A);
    let _ = drive_scene(&mut runtime, 0x000D);
    assert!(runtime.game().is_set(Flag::event(0x12)));
    runtime = relocate_with_battles(&runtime, 0x042);
    let _ = drive_scene(&mut runtime, 0x0033);
    assert!(runtime.game().is_set(Flag::event(0x31)));
    runtime = relocate_with_battles(&runtime, 0x0A3);
    let _ = drive_scene(&mut runtime, 0x0012);
    assert!(runtime.game().is_set(Flag::temp(0x08)));
    runtime = relocate_with_battles(&runtime, 0x063);
    let _ = drive_scene(&mut runtime, 0x0023);
    assert!(runtime.game().is_set(Flag::event(0x46)));
    runtime = relocate_with_battles(&runtime, 0x05E);
    let _ = drive_scene(&mut runtime, 0x003B);
    let _ = drive_scene(&mut runtime, 0x003C);
    runtime = relocate_with_edit_and_battles(&runtime, 0x0AC, |game| {
        game.inventory_mut()
            .add(0x99)
            .expect("control key fits inventory");
        game.set(Flag::chest(0x0A)).expect("control-key chest flag");
        game.set(Flag::chest(0x09)).expect("psycho-wand chest flag");
    });

    let _ = drive_scene(&mut runtime, 0x8007);
    assert!(runtime.game().is_set(Flag::event(0x34)));
    assert!(runtime.game().is_set(Flag::event(0x35)));
    assert_eq!(
        runtime.game().party_members(),
        vec![CharId(1), CharId(0), CharId(2), CharId(4), CharId(5)]
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
        Outcome::ScriptedExit,
        "the first Zio encounter exits from its Black Wave object"
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

    // The synthetic event dispatch skips the tower entrance. Carry its
    // on-foot state explicitly, otherwise this fights the tower boss in a
    // Land Rover and hides the resulting defeat behind the next cutscene.
    runtime
        .set_vehicle_index(0)
        .expect("dismount before Ladea Tower");
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
    assert_eq!(resolve_scene_battle(&mut runtime), Outcome::Victory);

    // Direct event dispatch skipped the chest interaction that gives the
    // wand. Supply that explicit fixture prerequisite before the Zio fight.
    runtime = relocate_with_edit_and_battles(&runtime, runtime.map_id().0, |game| {
        game.inventory_mut()
            .add(0x39)
            .expect("Psycho Wand chest item");
    });

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
    use_psycho_wand(&mut runtime);
    assert_eq!(resolve_scene_battle(&mut runtime), Outcome::Victory);

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

    // Wave 5 ends here. From this point the test keeps one retail-shaped
    // state alive through every newly registered Dezo scene. The tower chest
    // flags and Eclipse Torch are player-controlled prerequisites, so seed
    // those exact persistent bits before entering the first trigger.
    assert_eq!(
        post_zio.game().party_members(),
        vec![CharId(0), CharId(5), CharId(3), CharId(7), CharId(8)]
    );
    post_zio = relocate_with_edit_and_battles(&post_zio, 0x001, |game| {
        game.set(Flag::event(0xD5)).expect("strength chest flag");
        game.set(Flag::event(0xD3)).expect("courage chest flag");
        game.inventory_mut()
            .add(0x8E)
            .expect("Eclipse Torch fits inventory");
        harden_arc_battle_roster(game);
    });

    let _ = drive_scene(&mut post_zio, 0x0048);
    assert!(post_zio.game().is_set(Flag::event(0xD1)));

    let _ = drive_scene(&mut post_zio, 0x801C);
    assert_eq!(post_zio.map_id().0, 0x18D);
    assert!(post_zio.game().is_set(Flag::event(0xD6)));
    assert!(post_zio.game().is_set(Flag::event(0xD7)));

    post_zio = relocate_with_battles(&post_zio, 0x001);
    let trees = drive_until_battle(&mut post_zio, 0x004C);
    assert!(
        trees
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0A, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    post_zio = relocate_with_edit_and_battles(&post_zio, 0x001, |game| {
        game.set(Flag::event(0x94)).expect("RajaSick prerequisite");
    });
    let saving_kyra = drive_until_battle(&mut post_zio, 0x004D);
    assert!(post_zio.game().is_set(Flag::event(0x95)));
    assert!(
        saving_kyra
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0A, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let _ = drive_scene(&mut post_zio, 0x8013);
    assert_eq!(post_zio.map_id().0, 0x001);
    assert!(post_zio.game().is_set(Flag::event(0xA0)));
    assert_eq!(post_zio.game().party_slot(4), Some(CharId(9)));
    assert_eq!(post_zio.vehicle_index(), Some(2));

    let _ = drive_scene(&mut post_zio, 0x0047);
    assert!(post_zio.game().is_set(Flag::event(0x9C)));
    assert_eq!(post_zio.vehicle_index(), None);

    let dark_force_2 = drive_until_battle(&mut post_zio, 0x004E);
    assert!(post_zio.game().is_set(Flag::event(0x9E)));
    assert!(
        dark_force_2
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x11, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let _ = drive_scene(&mut post_zio, 0x8014);
    assert_eq!(post_zio.map_id().0, 0x16F);
    assert!(post_zio.game().is_set(Flag::event(0x97)));

    let _ = drive_scene(&mut post_zio, 0x8017);
    assert_eq!(post_zio.map_id().0, 0x001);
    assert!(post_zio.game().is_set(Flag::event(0xA1)));
    assert_eq!(post_zio.game().party_slot(3), Some(CharId(7)));
    assert!(!post_zio.game().party_members().contains(&CharId(9)));

    let _ = drive_scene(&mut post_zio, 0x8019);
    assert_eq!(post_zio.map_id().0, 0x000);
    assert!(post_zio.game().is_set(Flag::event(0xC1)));
    assert_eq!(post_zio.game().party_slot(4), Some(CharId(10)));

    post_zio = relocate_with_edit_and_battles(&post_zio, 0x000, |game| {
        game.set(Flag::chest(0x0D)).expect("Aero Prism chest flag");
    });
    let aero_prism = drive_until_battle(&mut post_zio, 0x801A);
    assert!(post_zio.game().is_set(Flag::event(0xC5)));
    assert!(!post_zio.game().party_members().contains(&CharId(10)));
    assert!(
        aero_prism
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x12, .. }))
    );
    assert_eq!(
        resolve_scene_battle_with_limit(&mut post_zio, 5_000),
        Outcome::Victory
    );

    let _ = drive_scene(&mut post_zio, 0x0050);
    assert!(post_zio.game().is_set(Flag::event(0xC6)));
    let reshel = drive_until_battle(&mut post_zio, 0x0053);
    assert!(post_zio.game().is_set(Flag::event(0x8B)));
    assert!(
        reshel
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0D, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let clm_forced = drive_until_battle(&mut post_zio, 0x0054);
    assert!(post_zio.game().is_set(Flag::event(0x92)));
    assert!(
        clm_forced
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0B, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);
    let _ = drive_scene(&mut post_zio, 0x0055);
    assert!(post_zio.game().is_set(Flag::event(0xA4)));

    let d_elm_lars = drive_until_battle(&mut post_zio, 0x0056);
    assert!(post_zio.game().is_set(Flag::event(0x93)));
    assert!(
        d_elm_lars
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0C, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);
    let _ = drive_scene(&mut post_zio, 0x0057);
    assert!(post_zio.game().is_set(Flag::event(0xA5)));

    let _ = drive_scene(&mut post_zio, 0x8015);
    assert_eq!(post_zio.map_id().0, 0x18D);
    assert!(post_zio.game().is_set(Flag::event(0x99)));
    let _ = drive_scene(&mut post_zio, 0x0058);
    assert!(post_zio.game().is_set(Flag::event(0x9F)));

    let xe_a_thoul = drive_until_battle(&mut post_zio, 0x0059);
    assert!(post_zio.game().is_set(Flag::event(0x9A)));
    assert!(
        xe_a_thoul
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0E, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let air_fake_chest = drive_until_battle(&mut post_zio, 0x005A);
    assert!(post_zio.game().is_set(Flag::event(0xA6)));
    assert!(!post_zio.game().inventory().slots().contains(&0x8E));
    assert!(
        air_fake_chest
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x0F, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let lashiec_appears = drive_until_battle(&mut post_zio, 0x005D);
    assert!(post_zio.game().is_set(Flag::event(0x9B)));
    assert!(
        lashiec_appears
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x10, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let _ = drive_scene(&mut post_zio, 0x8016);
    assert_eq!(post_zio.map_id().0, 0x162);
    assert!(post_zio.game().inventory().slots().contains(&0x8E));
    let _ = drive_scene(&mut post_zio, 0x8018);
    assert_eq!(post_zio.map_id().0, 0x0BF);
    assert!(post_zio.game().is_set(Flag::event(0x9D)));
    assert!(post_zio.game().is_set(Flag::event(0xC0)));
    assert!(post_zio.game().inventory().slots().contains(&0x98));

    post_zio = relocate_with_battles(&post_zio, 0x0F6);
    let _ = drive_scene(&mut post_zio, 0x005E);
    assert!(post_zio.game().is_set(Flag::event(0xE2)));
    post_zio = relocate_with_battles(&post_zio, 0x0FB);
    let _ = drive_scene(&mut post_zio, 0x005F);
    assert!(post_zio.game().is_set(Flag::event(0xE3)));

    let de_vars = drive_until_battle(&mut post_zio, 0x0060);
    assert!(post_zio.game().is_set(Flag::event(0xD4)));
    assert!(
        de_vars
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x16, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);
    let sa_lews = drive_until_battle(&mut post_zio, 0x0061);
    assert!(post_zio.game().is_set(Flag::event(0xD2)));
    assert!(
        sa_lews
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x17, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    post_zio = relocate_with_edit_and_battles(&post_zio, 0x0FB, |game| {
        game.set(Flag::event(0xE4))
            .expect("Alys fight prerequisite");
    });
    let _ = drive_scene(&mut post_zio, 0x0063);
    assert!(post_zio.game().is_set(Flag::event(0xE1)));
    post_zio = relocate_with_edit_and_battles(&post_zio, 0x0FB, |game| {
        game.clear(Flag::event(0xE4)).expect("Alys fight clears");
    });
    assert!(post_zio.game().is_clear(Flag::event(0xE4)));

    let _ = drive_scene(&mut post_zio, 0x0064);
    assert!(post_zio.game().is_set(Flag::event(0xE5)));
    let _ = drive_scene(&mut post_zio, 0x0065);
    assert!(post_zio.game().is_set(Flag::event(0xE6)));

    // The retail Alys interaction at EventPtrs[$62] and its battle epilogue
    // are outside this RunEvents delta. The trigger's post-battle state is
    // represented explicitly here: AlysFight is clear, ReFaze is set.
    post_zio = relocate_with_battles(&post_zio, 0x0FE);
    let _ = drive_scene(&mut post_zio, 0x0069);
    assert!(post_zio.game().is_set(Flag::event(0xE2)));
    assert_eq!(post_zio.map_id().0, 0x0FE);
    let _ = drive_scene(&mut post_zio, 0x006A);
    assert!(post_zio.game().is_set(Flag::event(0xE7)));

    let profound = drive_until_battle(&mut post_zio, 0x8020);
    assert!(post_zio.game().is_set(Flag::event(0xE8)));
    assert!(
        profound
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneBattleStarted { index: 0x1A, .. }))
    );
    assert_eq!(resolve_scene_battle(&mut post_zio), Outcome::Victory);

    let ending = drive_scene(&mut post_zio, 0x8021);
    assert!(
        ending
            .iter()
            .any(|item| matches!(item, RuntimeEvent::SceneEnded)),
        "ending returns to the cleared-game loop"
    );
    assert!(post_zio.game().is_set(Flag::event(0xE8)));
    assert_eq!(post_zio.map_id().0, 0x07B);
    assert!(post_zio.game_cleared());
    assert!(ending.iter().any(|item| {
        matches!(
            item,
            RuntimeEvent::ScenePresentation {
                op: SceneOp::PanelCreate { id: 0x17E }
            }
        )
    }));
}
