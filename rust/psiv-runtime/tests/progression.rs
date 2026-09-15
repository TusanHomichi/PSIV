//! Original level records must reach persistent learned slots after victory.
use psiv_core::battle::{BattleEvent, Outcome, RoundOrders, level_up};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, battle_data};
use std::path::Path;
const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn fixture() -> Option<(Runtime, BattleFiles)> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").exists() {
        return None;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut rt = Runtime::new(
        GameData::load(pack).unwrap(),
        0x47,
        Cell::new(30, 45),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    Some((rt, files))
}

fn from_game(game: GameState, files: &BattleFiles) -> Runtime {
    let mut rt = Runtime::from_save(
        GameData::load(Path::new(PACK)).unwrap(),
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
    rt.enable_battles(files).unwrap();
    rt
}

#[test]
fn victory_teaches_chaz_tsu_and_hahn_wat_and_save_continue_retains_them() {
    let Some((initial, files)) = fixture() else {
        return;
    };
    let data = battle_data(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(4)),
        None,
        None,
    ]);
    for (who, level) in [(0, 3), (2, 2)] {
        let stats = game.roster_mut().get_mut(CharId(who)).unwrap();
        while stats.level < level {
            stats.experience = data
                .level_table(who)
                .unwrap()
                .next_after(stats.level)
                .unwrap()
                .experience_required;
            level_up(who, stats, &data).unwrap().unwrap();
        }
        stats.experience = data
            .level_table(who)
            .unwrap()
            .next_after(level)
            .unwrap()
            .experience_required
            - 8;
        stats.curr_hp = stats.max_hp;
        stats.curr_tp = stats.max_tp;
        stats.curr_skill_uses[0] = 1;
    }
    let mut rt = from_game(game, &files);
    if let Some(directory) = std::env::var_os("PSIV_PROGRESSION_SMOKE_SAVE_DIR") {
        rt.save_slot(Path::new(&directory), 0).unwrap();
    }
    rt.set_rng_seed(0x1234_5678);
    rt.start_battle(0x8A, rt.battle_party()).unwrap();
    let mut events = Vec::new();
    for _ in 0..20 {
        events.extend(rt.battle_round(&RoundOrders::attack_all()).unwrap());
        if events
            .iter()
            .any(|e| matches!(e, BattleEvent::Ended { .. }))
        {
            break;
        }
    }
    assert!(events.contains(&BattleEvent::Ended {
        outcome: Outcome::Victory
    }));
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::Rewarded {
            experience_each: 8,
            ..
        }
    )));
    let events = rt.finish_battle_for_outcome(Outcome::Victory, 8);
    for (who, tech, name, level) in [(0, 7, "TSU", 4), (2, 4, "WAT", 3)] {
        assert!(
            events.contains(&BattleEvent::LearnedAbility {
                character: who,
                name: name.into()
            }),
            "{events:?}"
        );
        let stats = rt.game().roster().get(CharId(who)).unwrap();
        assert_eq!(stats.level, level);
        assert!(stats.techniques.contains(&tech));
        assert_eq!(stats.curr_skill_uses[0], 1, "existing uses do not refill");
    }
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().curr_tp, 17);
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().max_tp, 19);
    let snapshot = rt.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-learn-save-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let mut loaded = Runtime::load_slot(
        GameData::load(Path::new(PACK)).unwrap(),
        &dir,
        0,
        StepFrames::default(),
    )
    .unwrap();
    loaded.enable_battles(&files).unwrap();
    assert_eq!(loaded.game().snapshot(), snapshot);
    assert!(loaded.repair_legacy_progression().unwrap().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_repair_only_restores_earned_abilities_and_maxima_and_is_idempotent() {
    let Some((initial, files)) = fixture() else {
        return;
    };
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.roster_mut().get_mut(CharId(0)).unwrap().level = 4;
    game.roster_mut().get_mut(CharId(2)).unwrap().level = 3;
    game.roster_mut()
        .get_mut(CharId(0))
        .unwrap()
        .curr_skill_uses[0] = 1;
    let mut expected = GameState::from_snapshot(&game.snapshot());
    let chaz = expected.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.techniques[1] = 7;
    chaz.max_skill_uses[0] = 5;
    expected.roster_mut().get_mut(CharId(2)).unwrap().techniques[2] = 4;
    let mut rt = from_game(game, &files);
    let repairs = rt.repair_legacy_progression().unwrap();
    assert_eq!(repairs.len(), 2);
    assert_eq!(rt.game().snapshot(), expected.snapshot());
    assert!(rt.repair_legacy_progression().unwrap().is_empty());
}

#[test]
fn failed_legacy_repair_does_not_partially_edit_the_roster() {
    let Some((initial, files)) = fixture() else {
        return;
    };
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.roster_mut().get_mut(CharId(0)).unwrap().level = 4;
    let hahn = game.roster_mut().get_mut(CharId(2)).unwrap();
    hahn.level = 3;
    hahn.techniques = [24; 16];
    let before = game.snapshot();
    let mut rt = from_game(game, &files);
    assert!(rt.repair_legacy_progression().is_err());
    assert_eq!(rt.game().snapshot(), before);
}
