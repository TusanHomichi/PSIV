//! First-boss Fission: live formation slots, repeat kills and save handoff.
use psiv_core::battle::{BattleEvent, Command, FighterId, ItemSource, Outcome, RoundOrders};
use psiv_core::{
    Cell, CharId, Direction, GameState, Input, RetailLocation, RetailSave, StepFrames,
};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};
use std::path::Path;
const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

#[test]
fn igglanova_replaces_the_killed_neighbor_then_can_be_defeated_and_saved() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x13,
        Cell::new(48, 19),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(1)),
        Some(CharId(2)),
        None,
        None,
    ]);
    game.inventory_mut().add(139).unwrap();
    let mut runtime = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x17,
                char_x: 0x1E0,
                char_y: 0x120,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    runtime.enable_battles(&files).unwrap();
    if let Some(dir) = std::env::var_os("PSIV_FISSION_SMOKE_SAVE_DIR") {
        runtime.save_slot(Path::new(&dir), 0).unwrap();
    }
    assert!(runtime.start_event(0x6B));
    for _ in 0..100 {
        for event in runtime.tick(Input::Neutral) {
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        if runtime.battle_active() {
            break;
        }
    }
    assert!(runtime.battle_active());
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .living(psiv_core::battle::Side::Enemy)
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![id(7)]
    );
    runtime.set_rng_seed(0x0101_5678);
    for expected_alive in [2, 3] {
        let events = runtime
            .battle_round(&RoundOrders::Commands(vec![Command::Defend; 3]))
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::EnemyReplenished { .. }))
                .count(),
            1
        );
        assert_eq!(
            runtime
                .battle_roster()
                .unwrap()
                .living(psiv_core::battle::Side::Enemy)
                .count(),
            expected_alive
        );
    }
    let timeline = runtime
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Defend,
            Command::Item {
                item: 139,
                source: ItemSource::Inventory(0),
                target: Some(id(6)),
            },
            Command::Defend,
        ]))
        .unwrap();
    assert!(
        timeline
            .events
            .contains(&BattleEvent::Died { fighter: id(6) })
    );
    assert!(timeline.events.contains(&BattleEvent::EnemyReplenished {
        actor: id(7),
        fighter: id(6),
        enemy_id: 9,
        name: "XANAFALGUE".into(),
        hp: 16
    }));
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. }))
    );
    assert_eq!(runtime.game().inventory().get(0), None);
    let battle = runtime.battle_roster().unwrap();
    assert_eq!(battle.get(id(6)).unwrap().stats.curr_hp, 16);
    assert_eq!(battle.get(id(7)).unwrap().stats.curr_hp, 300);
    assert_eq!(battle.get(id(8)).unwrap().stats.curr_hp, 16);
    let mut outcome = None;
    let mut reward = 0;
    for _ in 0..80 {
        let orders = RoundOrders::Commands(vec![
            Command::AttackTarget(id(7)),
            Command::Attack,
            Command::AttackTarget(id(7)),
        ]);
        for e in runtime.battle_round(&orders).unwrap() {
            match e {
                BattleEvent::Ended { outcome: result } => outcome = Some(result),
                BattleEvent::Rewarded {
                    experience_each, ..
                } => reward = experience_each,
                _ => {}
            }
        }
        if outcome.is_some() {
            break;
        }
    }
    assert_eq!(outcome, Some(Outcome::Victory));
    assert!(runtime.map().npcs()[..3].iter().all(|npc| npc.active));
    let party_before_return = runtime.members();
    let camera_before_return = *runtime.camera();
    runtime.finish_battle_for_outcome(Outcome::Victory, reward);
    assert_eq!(
        runtime.tick(Input::Neutral),
        vec![RuntimeEvent::MapRefreshed]
    );
    assert!(runtime.map().npcs()[..3].iter().all(|npc| !npc.active));
    assert!(runtime.map().npc_at(Cell::new(15, 10)).is_none());
    assert!(runtime.map().npc_at(Cell::new(16, 10)).is_none());
    assert_eq!(runtime.members(), party_before_return);
    assert_eq!(*runtime.camera(), camera_before_return);
    assert!(runtime.return_to_field().is_empty());
    for _ in 0..10000 {
        for e in runtime.tick(Input::Neutral) {
            if matches!(
                e,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        if !runtime.scene_active() {
            break;
        }
    }
    assert!(!runtime.scene_active());
    let saved = runtime.game().snapshot();
    let directory = std::env::temp_dir().join(format!("psiv-fission-save-{}", std::process::id()));
    runtime.save_slot(&directory, 0).unwrap();
    let mut continued = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        &directory,
        0,
        StepFrames::default(),
    )
    .unwrap();
    continued.enable_battles(&files).unwrap();
    assert_eq!(continued.game().snapshot(), saved);
    std::fs::remove_dir_all(directory).unwrap();
}
