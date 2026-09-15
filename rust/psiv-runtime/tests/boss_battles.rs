//! Headless event-battle coverage against the extracted opening-act pack.

use std::path::Path;

use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{Cell, Direction, Flag, Input, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn runtime_with_battles() -> Runtime {
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let start = data.manifest().game_start.clone().expect("game start");
    let mut runtime = Runtime::new(
        data,
        start.map.id,
        Cell::new(start.x_cell as u16, start.y_cell as u16),
        Direction::Down,
        StepFrames::default(),
    )
    .expect("runtime starts");
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    runtime.enable_battles(&files).expect("battles enable");
    runtime
}

fn drive_scene(runtime: &mut Runtime, event: u16) -> Vec<RuntimeEvent> {
    assert!(runtime.start_event(event), "event {event:#x} starts");
    let mut log = Vec::new();
    for _ in 0..10_000 {
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
        log.extend(events);
        if !runtime.scene_active() {
            return log;
        }
    }
    panic!(
        "event {event:#x} did not finish; tail: {:?}",
        &log[log.len().saturating_sub(6)..]
    );
}

fn start_igglanova(runtime: &mut Runtime) -> Vec<BattleEvent> {
    assert!(runtime.start_event(0x6B), "Igglanova event starts");
    for _ in 0..100 {
        let events = runtime.tick(Input::Neutral);
        for item in &events {
            match item {
                RuntimeEvent::SceneBattleStarted { events, .. } => {
                    assert!(
                        runtime.battle_active(),
                        "battle owns the field after request"
                    );
                    assert!(
                        runtime.scene_active(),
                        "scene remains blocked during battle"
                    );
                    return events.clone();
                }
                RuntimeEvent::SceneBattleFailed { error, .. } => {
                    panic!("Igglanova battle failed to start: {error}");
                }
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume => {
                    runtime.dialogue_closed()
                }
                _ => {}
            }
        }
    }
    panic!("Igglanova scene never requested its battle");
}

fn resolve_with_attacks(runtime: &mut Runtime) -> Outcome {
    let mut outcome = None;
    for _ in 0..300 {
        let events = runtime
            .battle_round(&RoundOrders::attack_all())
            .expect("attack round resolves");
        for event in events {
            if let BattleEvent::Ended { outcome: result } = event {
                outcome = Some(result);
            }
        }
        if let Some(outcome) = outcome {
            return outcome;
        }
    }
    panic!("Igglanova battle did not end in 300 attack rounds");
}

#[test]
fn containers_unlock_igglanova_boss_and_resume_story_on_victory() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }
    let mut runtime = runtime_with_battles();
    let containers = drive_scene(&mut runtime, 0x0C);
    assert!(
        containers
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneEnded)),
        "containers scene ends"
    );
    assert!(
        runtime.game().is_set(Flag::event(0x0D)),
        "containers scene sets EventFlag 0x0D"
    );

    // Bring Alys into the party so the headless victory path tests the real
    // opening party shape rather than making Chaz solo a boss fight.
    let alys = drive_scene(&mut runtime, 0x03);
    assert!(
        alys.iter()
            .any(|event| matches!(event, RuntimeEvent::PartyChanged))
    );
    assert_eq!(
        runtime.battle_party().len(),
        2,
        "Alys joins before the boss"
    );

    let initial = start_igglanova(&mut runtime);
    assert!(
        !initial.is_empty(),
        "boss start emits battle timeline events"
    );

    let escape = runtime
        .battle_round(&RoundOrders::Run)
        .expect("boss escape attempt resolves");
    assert!(
        escape
            .iter()
            .any(|event| matches!(event, BattleEvent::EscapeFailed)),
        "Igglanova run chance 0xFE forbids escape"
    );
    assert!(
        runtime.battle_active(),
        "failed escape leaves the boss battle live"
    );

    let outcome = resolve_with_attacks(&mut runtime);
    assert_eq!(outcome, Outcome::Victory, "Alys and Chaz defeat Igglanova");
    runtime.finish_battle_for_outcome(outcome, 0);
    assert!(!runtime.battle_active());
    assert!(
        runtime.scene_active(),
        "scene waits for battle completion input"
    );

    let mut ended = false;
    for _ in 0..20 {
        for event in runtime.tick(Input::Neutral) {
            if matches!(event, RuntimeEvent::SceneEnded) {
                ended = true;
            }
        }
        if ended {
            break;
        }
    }
    assert!(ended, "Igglanova scene resumes and returns after victory");
    assert!(runtime.game().is_set(Flag::event(0x0B)));
    assert!(runtime.game().is_set(Flag::event(0x0D)));
}

#[test]
fn igglanova_defeat_ends_play_without_reviving_or_replaying_the_scene() {
    if !Path::new(PACK).join("battle").is_dir() {
        return;
    }
    let mut runtime = runtime_with_battles();
    let _ = start_igglanova(&mut runtime);
    let outcome = resolve_with_attacks(&mut runtime);
    assert_eq!(outcome, Outcome::Defeat);
    let money = runtime.game().money();
    runtime.finish_battle_for_outcome(outcome, 0);
    assert!(runtime.game_over());
    assert!(!runtime.scene_active());
    assert!(!runtime.battle_active());
    assert!(
        runtime.game().is_set(Flag::event(0x0B)),
        "defeat does not edit story guards"
    );
    assert_eq!(runtime.battle_party()[0].stats.curr_hp, 0);
    assert_ne!(runtime.battle_party()[0].stats.status & 0x44, 0);
    let at = runtime.state().cell();
    for _ in 0..120 {
        assert!(runtime.tick(Input::Direction(Direction::Down)).is_empty());
    }
    assert!(runtime.return_to_field().is_empty());
    assert_eq!(runtime.state().cell(), at);
    assert_eq!(runtime.game().money(), money);
}
