//! Original RIMIT record through the live queue and saved resource balance.
use psiv_core::battle::{BattleEvent, Command, Outcome, RoundOrders};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

fn rimit_fixture() -> Option<Runtime> {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").exists() {
        return None;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x47,
        Cell::new(30, 45),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(8)),
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(4)),
        Some(CharId(3)),
    ]);
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x47,
                map_index_2: 0x42,
                char_x: 480,
                char_y: 720,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    Some(rt)
}

#[test]
fn sealed_rimit_pays_tp_without_sleep_or_spell_sound() {
    let Some(mut rt) = rimit_fixture() else {
        return;
    };
    let mut party = rt.battle_party();
    // Model a command already chosen before a seal lands. No native menu
    // bypass is claimed: the fixture exercises the runtime execution boundary.
    party[0].stats.status |= psiv_core::battle::status::TECH_SEALED;
    let before_tp = party[0].stats.curr_tp;
    rt.set_rng_seed(0x1234_5678);
    rt.start_battle(0x8a, party).unwrap();
    let turn = rt
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Technique {
                technique: 23,
                target: None,
            },
            Command::Defend,
            Command::Defend,
            Command::Defend,
            Command::Defend,
        ]))
        .unwrap();
    let cast_index = turn
        .events
        .iter()
        .position(|event| {
            matches!(event,
        BattleEvent::TechniqueUsed { technique: 23, remaining_tp, .. }
            if *remaining_tp == before_tp - 10)
        })
        .expect("late seal still pays original 10 TP");
    assert!(matches!(
        turn.events.get(cast_index + 1),
        Some(BattleEvent::TechniqueRejected {
            technique: 23,
            reason: psiv_core::battle::TechniqueRejection::Sealed,
            ..
        })
    ));
    assert!(
        !turn
            .sounds
            .iter()
            .any(|sound| sound.event_index == cast_index)
    );
    assert!(
        !turn
            .events
            .iter()
            .any(|event| matches!(event, BattleEvent::FellAsleep { .. }))
    );
}

#[test]
fn original_raja_rimit_sleeps_enemies_and_preserves_paid_tp_through_victory_save() {
    let Some(mut rt) = rimit_fixture() else {
        return;
    };
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if let Some(directory) = std::env::var_os("PSIV_RIMIT_SMOKE_SAVE_DIR") {
        rt.save_slot(Path::new(&directory), 0).unwrap();
    }
    rt.set_rng_seed(0x1234_5678);
    rt.start_battle(0x8a, rt.battle_party()).unwrap();
    let turn = rt
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Technique {
                technique: 23,
                target: None,
            },
            Command::Defend,
            Command::Defend,
            Command::Defend,
            Command::Defend,
        ]))
        .unwrap();
    assert!(
        turn.events
            .iter()
            .any(|e| matches!(e, BattleEvent::FellAsleep { .. })),
        "{:?}",
        turn.events
    );
    assert!(turn.sounds.iter().any(|s| s.id == 0xcb));
    let mut events = turn.events;
    for _ in 0..20 {
        if events
            .iter()
            .any(|e| matches!(e, BattleEvent::Ended { .. }))
        {
            break;
        }
        events.extend(rt.battle_round(&RoundOrders::attack_all()).unwrap());
    }
    assert!(
        events.contains(&BattleEvent::Ended {
            outcome: Outcome::Victory
        }),
        "{events:?}"
    );
    let experience = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::Rewarded {
                experience_each, ..
            } => Some(*experience_each),
            _ => None,
        })
        .unwrap();
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    assert_eq!(rt.game().roster().get(CharId(8)).unwrap().curr_tp, 160);
    let before = rt.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-rimit-save-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let loaded = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        &dir,
        0,
        StepFrames::default(),
    )
    .unwrap();
    assert_eq!(loaded.game().snapshot(), before);
    std::fs::remove_dir_all(dir).unwrap();
}
