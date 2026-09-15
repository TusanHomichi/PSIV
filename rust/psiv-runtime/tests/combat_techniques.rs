//! Real JSON -> native commands -> victory -> field -> save/continue.
use psiv_core::battle::{BattleEvent, Command, FighterId, Outcome, RoundOrders};
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

#[test]
fn early_party_techniques_survive_victory_and_save_continue() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x13,
        psiv_core::Cell { x: 48, y: 19 },
        psiv_core::Direction::Down,
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
    game.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 20;
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
    // Optional reproducible Godot fixture. Isolated directory only; the test
    // never touches the player's normal save location.
    if let Some(directory) = std::env::var_os("PSIV_COMBAT_SMOKE_SAVE_DIR") {
        runtime.save_slot(Path::new(&directory), 0).unwrap();
    }
    for tech in runtime
        .battle_party()
        .iter()
        .flat_map(|p| p.stats.techniques)
        .filter(|t| *t != 0)
    {
        assert!(
            runtime
                .battle_techniques()
                .find(|t| t.id == tech)
                .unwrap()
                .supported(),
            "starting technique {tech}"
        );
    }
    runtime.set_rng_seed(0x1234_5678);
    runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
    let timeline = runtime
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Technique {
                technique: 24,
                target: Some(id(1)),
            },
            Command::Technique {
                technique: 1,
                target: Some(id(6)),
            },
            Command::Technique {
                technique: 20,
                target: None,
            },
        ]))
        .unwrap();
    for (actor, technique, tp) in [(1, 24, 7), (2, 1, 37), (3, 20, 20)] {
        assert!(timeline.events.iter().any(|event| matches!(event, BattleEvent::TechniqueUsed { actor: a, technique: t, remaining_tp, .. } if *a == id(actor) && *t == technique && *remaining_tp == tp)), "{timeline:?}");
    }
    assert!(timeline.events.iter().any(|e| matches!(e, BattleEvent::Healed { target, amount, .. } if *target == id(1) && *amount > 0)));
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::TechniqueRejected { .. }))
    );
    // Technique events must never borrow physical weapon sounds/animations.
    for sound in &timeline.sounds {
        assert!(!matches!(
            timeline.events[sound.event_index],
            BattleEvent::TechniqueUsed { .. }
        ));
    }
    let mut all_events = timeline.events;
    for _ in 0..20 {
        if all_events
            .iter()
            .any(|e| matches!(e, BattleEvent::Ended { .. }))
        {
            break;
        }
        all_events.extend(runtime.battle_round(&RoundOrders::attack_all()).unwrap());
    }
    assert!(
        all_events.contains(&BattleEvent::Ended {
            outcome: Outcome::Victory
        }),
        "{all_events:?}"
    );
    let reward = all_events
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
    for (character, tp) in [(0, 7), (1, 37), (2, 20)] {
        assert_eq!(
            runtime
                .game()
                .roster()
                .get(CharId(character))
                .unwrap()
                .curr_tp,
            tp
        );
    }
    let snapshot = runtime.game().snapshot();
    let directory =
        std::env::temp_dir().join(format!("psiv-technique-save-{}", std::process::id()));
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
