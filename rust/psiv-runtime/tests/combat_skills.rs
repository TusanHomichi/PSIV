//! Opening skills from the real pack, carried back to field and saved.
use psiv_core::battle::{BattleEvent, Command, FighterId, Outcome, RoundOrders};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

#[test]
fn opening_skills_survive_victory_and_save_continue() {
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
    if let Some(directory) = std::env::var_os("PSIV_SKILL_SMOKE_SAVE_DIR") {
        runtime.save_slot(Path::new(&directory), 0).unwrap();
    }
    assert_eq!(runtime.battle_skills().count(), 54);
    assert_eq!(runtime.battle_skills().filter(|s| s.supported()).count(), 5);
    assert!(runtime.battle_skills().any(|s| s.id == 1 && s.supported()));
    for skill in runtime
        .battle_party()
        .iter()
        .flat_map(|p| p.stats.skills)
        .filter(|s| *s != 0)
    {
        assert!(
            runtime
                .battle_skills()
                .find(|s| s.id == skill)
                .unwrap()
                .supported()
        );
    }
    runtime.set_rng_seed(0x0101_5678);
    runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .get(id(1))
            .unwrap()
            .stats
            .mental
            .battle,
        6
    );
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .get(id(6))
            .unwrap()
            .stats
            .element_factor(11),
        Some(2)
    );
    let timeline = runtime
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Skill {
                skill: 31,
                target: Some(id(6)),
            },
            Command::Skill {
                skill: 6,
                target: Some(id(7)),
            },
            Command::Skill {
                skill: 47,
                target: None,
            },
        ]))
        .unwrap();
    for (actor, skill, remaining, tp, dex) in
        [(1, 31, 2, 10, 13), (2, 6, 4, 40, 21), (3, 47, 4, 25, 13)]
    {
        assert!(timeline.events.iter().any(|e| matches!(e, BattleEvent::SkillUsed { actor: a, skill: s, remaining: r, .. } if *a == id(actor) && *s == skill && *r == remaining)), "{timeline:?}");
        let stats = &runtime
            .battle_roster()
            .unwrap()
            .get(id(actor))
            .unwrap()
            .stats;
        assert_eq!(stats.curr_tp, tp);
        assert_eq!(stats.dexterity.battle, dex);
    }
    assert!(
        timeline.events.contains(&BattleEvent::FellAsleep {
            actor: id(1),
            target: id(6)
        }),
        "{timeline:?}"
    );
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::SkillRejected { .. }))
    );
    for sound in &timeline.sounds {
        assert!(!matches!(
            timeline.events[sound.event_index],
            BattleEvent::SkillUsed { .. }
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
    for (character, uses, tp) in [(0, 2, 10), (1, 4, 40), (2, 4, 25)] {
        let stats = runtime.game().roster().get(CharId(character)).unwrap();
        assert_eq!(stats.curr_skill_uses[0], uses);
        assert_eq!(stats.curr_tp, tp);
        assert_eq!(stats.dexterity.battle, stats.dexterity.modified);
    }
    let snapshot = runtime.game().snapshot();
    let directory = std::env::temp_dir().join(format!("psiv-skill-save-{}", std::process::id()));
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
    // Re-entering combat must not replenish uses or preserve Vision's buff.
    loaded.start_battle(0x8A, loaded.battle_party()).unwrap();
    for (fighter, uses) in [(1, 2), (2, 4), (3, 4)] {
        let stats = &loaded
            .battle_roster()
            .unwrap()
            .get(id(fighter))
            .unwrap()
            .stats;
        assert_eq!(stats.curr_skill_uses[0], uses);
        assert_eq!(stats.dexterity.battle, stats.dexterity.modified);
    }
    std::fs::remove_dir_all(directory).unwrap();
}
