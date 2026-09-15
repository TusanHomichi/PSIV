//! Original Gryz records: instant-death commands, rewards and saved resources.
use psiv_core::battle::{BattleEvent, Command, FighterId, Outcome, RoundOrders};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn gryz_original_crash_and_brose_records_consume_resources_and_survive_save() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").exists() {
        return;
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
    game.set_party([Some(CharId(4)), None, None, None, None]);
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
    assert_eq!(rt.battle_techniques().filter(|t| t.supported()).count(), 35);
    assert_eq!(rt.battle_skills().filter(|s| s.supported()).count(), 5);
    if let Some(directory) = std::env::var_os("PSIV_DEATH_SMOKE_SAVE_DIR") {
        rt.save_slot(Path::new(&directory), 0).unwrap();
    }
    let before_money = rt.game().money();
    rt.set_rng_seed(0x1234_5678);
    rt.start_battle(0x8A, rt.battle_party()).unwrap();
    let mut events = Vec::new();
    for command in [
        Command::Skill {
            skill: 34,
            target: FighterId::new(6),
        },
        Command::Technique {
            technique: 17,
            target: None,
        },
    ] {
        let timeline = rt
            .battle_round_timeline(&RoundOrders::Commands(vec![command]))
            .unwrap();
        if timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::TechniqueUsed { technique: 17, .. }))
        {
            assert!(timeline.sounds.iter().any(|sound| sound.id == 0xCB
                && matches!(
                    timeline.events[sound.event_index],
                    BattleEvent::TechniqueUsed { technique: 17, .. }
                )));
        }
        for sound in &timeline.sounds {
            if matches!(
                timeline.events[sound.event_index],
                BattleEvent::TechniqueUsed { technique: 17, .. }
            ) {
                assert_eq!(sound.id, 0xCB);
            }
        }
        events.extend(timeline.events);
    }
    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::SkillUsed {
                skill: 34,
                remaining: 6,
                ..
            }
        )),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::TechniqueUsed {
                technique: 17,
                remaining_tp: 4,
                ..
            }
        )),
        "{events:?}"
    );
    // The fixed seed misses both initial effects. Spend further *real* CRASH
    // uses until one succeeds; never turn a probabilistic skill into a sure kill.
    for _ in 0..6 {
        let turn = rt
            .battle_round(&RoundOrders::Commands(vec![Command::Skill {
                skill: 34,
                target: FighterId::new(6),
            }]))
            .unwrap();
        let killed = turn.iter().any(|e| matches!(e, BattleEvent::Died { .. }));
        events.extend(turn);
        if killed {
            break;
        }
    }
    assert!(
        events.iter().any(|e| matches!(e, BattleEvent::Died { .. })),
        "{events:?}"
    );
    let crash_uses = events
        .iter()
        .filter(|e| matches!(e, BattleEvent::SkillUsed { skill: 34, .. }))
        .count() as u8;
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
    let rewards: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let BattleEvent::Rewarded {
                experience_each,
                meseta,
                ..
            } = e
            {
                Some((*experience_each, *meseta))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(rewards, vec![(24, 6)]);
    rt.finish_battle_for_outcome(Outcome::Victory, 24);
    assert_eq!(rt.game().money(), before_money + 6);
    let gryz = rt.game().roster().get(CharId(4)).unwrap();
    assert_eq!(gryz.curr_tp, 4);
    assert_eq!(gryz.curr_skill_uses[0], 7 - crash_uses);
    let before = rt.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-death-save-{}", std::process::id()));
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
