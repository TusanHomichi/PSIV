//! JSON item-use records -> live shared inventory -> victory/save/continue.
use psiv_core::battle::{
    BattleEvent, Command, FighterId, ItemSource, Outcome, RoundOrders, status,
};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

#[test]
fn healing_cure_and_damage_items_persist_through_victory_and_continue() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x13,
        Cell { x: 48, y: 19 },
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
    let hahn = game.roster_mut().get_mut(CharId(2)).unwrap();
    hahn.curr_hp = 1;
    hahn.status = status::POISONED;
    for item in [125, 128, 139, 130, 144] {
        game.inventory_mut().add(item).unwrap();
    }
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
    if let Some(directory) = std::env::var_os("PSIV_ITEM_SMOKE_SAVE_DIR") {
        runtime.save_slot(Path::new(&directory), 0).unwrap();
    }
    assert_eq!(runtime.battle_items().count(), 160);
    assert_eq!(
        runtime
            .battle_items()
            .filter(|item| item.supported())
            .count(),
        26
    );
    assert!(
        runtime
            .battle_items()
            .find(|item| item.id == 144)
            .unwrap()
            .consumable
    );
    runtime.set_rng_seed(0x0101_5678);
    runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
    let timeline = runtime
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Item {
                item: 139,
                source: ItemSource::Inventory(2),
                target: Some(id(7)),
            },
            Command::Item {
                item: 125,
                source: ItemSource::Inventory(0),
                target: Some(id(3)),
            },
            Command::Item {
                item: 128,
                source: ItemSource::Inventory(1),
                target: Some(id(3)),
            },
        ]))
        .unwrap();
    for item in [125, 128, 139] {
        assert!(timeline.events.iter().any(|e| matches!(e, BattleEvent::ItemUsed { item: actual, consumed: true, .. } if *actual == item)), "{timeline:?}");
    }
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::ItemRejected { .. })),
        "{timeline:?}"
    );
    assert!(timeline.events.iter().any(|e| matches!(e, BattleEvent::Healed { target, amount, .. } if *target == id(3) && *amount > 0)), "{timeline:?}");
    let hahn = &runtime.battle_roster().unwrap().get(id(3)).unwrap().stats;
    assert_eq!(hahn.status & status::POISONED, 0);
    for (actor, tp) in [(1, 10), (2, 40), (3, 25)] {
        assert_eq!(
            runtime
                .battle_roster()
                .unwrap()
                .get(id(actor))
                .unwrap()
                .stats
                .curr_tp,
            tp
        );
    }
    assert_eq!(
        &runtime.game().inventory().slots()[..5],
        &[0, 0, 0, 130, 144]
    );
    for sound in &timeline.sounds {
        assert!(!matches!(
            timeline.events[sound.event_index],
            BattleEvent::ItemUsed { .. }
        ));
    }
    let mut events = timeline.events;
    for _ in 0..20 {
        if events
            .iter()
            .any(|e| matches!(e, BattleEvent::Ended { .. }))
        {
            break;
        }
        events.extend(runtime.battle_round(&RoundOrders::attack_all()).unwrap());
    }
    assert!(
        events.contains(&BattleEvent::Ended {
            outcome: Outcome::Victory
        }),
        "{events:?}"
    );
    let reward = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::Rewarded {
                experience_each, ..
            } => Some(*experience_each),
            _ => None,
        })
        .unwrap();
    runtime.finish_battle_for_outcome(Outcome::Victory, reward);
    assert!(!runtime.battle_active());
    assert_eq!(
        &runtime.game().inventory().slots()[..5],
        &[0, 0, 0, 130, 144]
    );
    assert_eq!(
        runtime.game().roster().get(CharId(2)).unwrap().status & status::POISONED,
        0
    );
    let snapshot = runtime.game().snapshot();
    let directory = std::env::temp_dir().join(format!("psiv-item-save-{}", std::process::id()));
    runtime.save_slot(&directory, 0).unwrap();
    let mut loaded = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        &directory,
        0,
        StepFrames::default(),
    )
    .unwrap();
    loaded.enable_battles(&files).unwrap();
    assert_eq!(loaded.game().snapshot(), snapshot);
    std::fs::remove_dir_all(directory).unwrap();
}
