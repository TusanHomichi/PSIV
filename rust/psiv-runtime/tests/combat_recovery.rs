//! Original recovery records through battle commands, rewards and saved roster.
use psiv_core::battle::{BattleEvent, Command, FighterId, Outcome, RoundOrders, level_up, status};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, battle_data};
use std::path::Path;
const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn original_recovery_techniques_cure_revive_and_keep_spent_resources_after_save() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").exists() {
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let data = battle_data(&files).unwrap();
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
        None,
        None,
    ]);
    // Isolated injured fixture: every level/stat/learned slot comes from the
    // original table. This is not a campaign save or an earned route receipt.
    for (who, level) in [(8, 31), (0, 8), (2, 8)] {
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
        stats.curr_hp = stats.max_hp;
        stats.curr_tp = stats.max_tp;
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.curr_hp = chaz.max_hp / 2;
    chaz.status = status::POISONED | status::PARALYZED;
    let hahn = game.roster_mut().get_mut(CharId(2)).unwrap();
    hahn.curr_hp = 0;
    hahn.status = status::DEAD | status::TECH_SEALED;
    let hahn_quarter = hahn.max_hp / 4;
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
    if let Some(directory) = std::env::var_os("PSIV_RECOVERY_SMOKE_SAVE_DIR") {
        rt.save_slot(Path::new(&directory), 0).unwrap();
    }
    let before_tp = rt.game().roster().get(CharId(8)).unwrap().curr_tp;
    rt.set_rng_seed(0x1234_5678);
    rt.start_battle(0x8A, rt.battle_party()).unwrap();
    let mut events = Vec::new();
    for (tech, target) in [(34, 2), (35, 2), (36, 3), (37, 3)] {
        let turn = rt
            .battle_round(&RoundOrders::Commands(vec![
                Command::Technique {
                    technique: tech,
                    target: FighterId::new(target),
                },
                Command::Defend,
                Command::Defend,
            ]))
            .unwrap();
        assert!(
            turn.iter()
                .any(|e| matches!(e,BattleEvent::TechniqueUsed {technique,..} if *technique==tech)),
            "{turn:?}"
        );
        if tech == 36 {
            assert!(turn.iter().any(|e|matches!(e,BattleEvent::Revived {remaining_hp,..} if *remaining_hp==hahn_quarter)),"{turn:?}");
        }
        events.extend(turn);
    }
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
    rt.finish_battle_for_outcome(Outcome::Victory, 8);
    assert_eq!(
        rt.game().roster().get(CharId(8)).unwrap().curr_tp,
        before_tp - 55
    );
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().status, 0);
    let hahn = rt.game().roster().get(CharId(2)).unwrap();
    assert!(hahn.curr_hp > 0);
    assert_eq!(hahn.status, status::TECH_SEALED);
    let before = rt.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-battle-cures-{}", std::process::id()));
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
