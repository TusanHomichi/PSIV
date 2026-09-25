//! The retail scene arc's scene-owned battles.
//!
//! A scene hands the fight to the battle engine and takes it back: the first
//! Zio encounter runs its five scripted stages and exits from its own Black
//! Wave object, and an ordinary story battle dispatches through the existing
//! battle-requested path. The scenes that own them are covered per scene in
//! `next_arc_scenes.rs`, and the whole chain in `next_arc.rs`.

use std::path::Path;

use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{
    Cell, CharId, Direction, Flag, GameState, Input, RetailLocation, RetailSave, StepFrames,
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
fn first_zio_runs_five_stages_and_returns_without_killing_or_rewarding_party() {
    use psiv_core::battle::{Command, FirstZioAction, Side};
    if !Path::new(PACK).join("battle").is_dir() {
        return;
    }
    let mut runtime = runtime_with_battles_at(0xAC);
    runtime = relocate_with_edit_and_battles(&runtime, 0xAC, |game| {
        game.set_party([
            Some(CharId(0)),
            Some(CharId(4)),
            Some(CharId(1)),
            Some(CharId(2)),
            Some(CharId(5)),
        ]);
        game.set_money(1234);
    });
    let start = drive_until_battle(&mut runtime, 0x8008);
    assert!(
        start
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneBattleStarted { index: 4, .. }))
    );
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .side(Side::Enemy)
            .next()
            .unwrap()
            .stats
            .enemy_id,
        152
    );
    let before = runtime
        .battle_roster()
        .unwrap()
        .side(Side::Party)
        .map(|f| (f.id, f.stats.curr_hp, f.stats.curr_tp, f.stats.status))
        .collect::<Vec<_>>();
    for (round, action) in [
        FirstZioAction::MagicBarrier,
        FirstZioAction::Invocation,
        FirstZioAction::Pause,
        FirstZioAction::Nightmare,
        FirstZioAction::BlackWave,
    ]
    .into_iter()
    .enumerate()
    {
        let events = runtime
            .battle_round(&RoundOrders::Commands(vec![Command::Defend; 5]))
            .unwrap();
        let stage = events
            .iter()
            .filter_map(|e| match e {
                BattleEvent::FirstZioAction { action, target, .. } => Some((*action, *target)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            stage,
            vec![(action, if round == 4 { Some(before[2].0) } else { None })]
        );
        assert!(!events.iter().any(|e| matches!(
            e,
            BattleEvent::UnsupportedAbility { .. }
                | BattleEvent::Resolved { .. }
                | BattleEvent::Died { .. }
                | BattleEvent::Rewarded { .. }
        )));
        let after = runtime
            .battle_roster()
            .unwrap()
            .side(Side::Party)
            .map(|f| (f.id, f.stats.curr_hp, f.stats.curr_tp, f.stats.status))
            .collect::<Vec<_>>();
        assert_eq!(after, before);
        if round == 4 {
            assert!(matches!(
                events.last(),
                Some(BattleEvent::Ended {
                    outcome: Outcome::ScriptedExit
                })
            ));
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, BattleEvent::RoundEnded { .. }))
            );
        } else {
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, BattleEvent::Ended { .. }))
            );
        }
    }
    assert!(
        runtime
            .battle_round(&RoundOrders::attack_all())
            .unwrap()
            .is_empty()
    );
    assert!(
        runtime
            .finish_battle_for_outcome(Outcome::ScriptedExit, 0)
            .is_empty()
    );
    assert!(!runtime.game_over());
    assert_eq!(runtime.game().money(), 1234);
    for _ in 0..100 {
        runtime.tick(Input::Neutral);
        if !runtime.scene_active() {
            break;
        }
    }
    assert!(!runtime.scene_active());
    assert!(runtime.game().is_set(Flag::event(0x42)));
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
