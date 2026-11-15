//! Event-to-session coverage, separate from cartridge/core replay evidence.
use super::*;
use psiv_core::battle::{Command, status};
use psiv_core::{Cell, CharId, Direction, StepFrames};

fn runtime() -> Option<Runtime> {
    let pack = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    let mut runtime = Runtime::new(
        psiv_data::GameData::load(pack).expect("decoded runtime pack"),
        0x13,
        Cell { x: 48, y: 19 },
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    runtime
        .enable_battles(&psiv_data::BattleFiles::load(pack).unwrap())
        .unwrap();
    runtime.game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        None,
        None,
    ]);
    Some(runtime)
}

fn mode(runtime: &mut Runtime) -> BattleMode {
    runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
    BattleMode::begin(
        runtime,
        BattleTimeline {
            events: Vec::new(),
            sounds: Vec::new(),
            animations: Vec::new(),
        },
        BattleStart::Encounter(0x8A),
    )
}

#[test]
fn tp_event_updates_only_its_recipient_at_the_played_beat() {
    let Some(mut runtime) = runtime() else { return };
    let mut mode = mode(&mut runtime);
    let before = mode.party.clone();
    let target = FighterId::new(2).unwrap();
    mode.update_live_party(&BattleEvent::TpRestored {
        actor: FighterId::new(1).unwrap(),
        target,
        amount: 7,
        remaining_tp: 17,
    });
    for (old, new) in before.iter().zip(&mode.party) {
        assert_eq!(new.hp, old.hp);
        assert_eq!(new.status, old.status);
        assert_eq!(new.tp, if new.fighter == 2 { 17 } else { old.tp });
    }
}

#[test]
fn session_round_applies_arows_global_event_to_off_party_records() {
    let Some(mut runtime) = runtime() else { return };
    let mut off_party = runtime.game.roster().get(CharId(0)).unwrap().clone();
    off_party.status = status::ASLEEP | status::ASLEEP_2 | status::ANDROID_DEAD;
    off_party.agility.battle = 1;
    runtime
        .game
        .roster_mut()
        .seat(CharId(10), off_party)
        .unwrap();
    let actor = runtime.game.roster_mut().get_mut(CharId(1)).unwrap();
    actor.techniques[0] = 38;
    actor.curr_tp = 99;
    let mut mode = mode(&mut runtime);
    mode.run_round(
        &mut runtime,
        &RoundOrders::Commands(vec![
            Command::Technique {
                technique: 38,
                target: None,
            },
            Command::Defend,
            Command::Defend,
            Command::Defend,
            Command::Defend,
        ]),
    );
    assert!(mode.fault.is_none());
    let off_party = runtime.game.roster().get(CharId(10)).unwrap();
    assert_eq!(off_party.status, status::ASLEEP_2 | status::ANDROID_DEAD);
    assert_eq!(off_party.agility.battle, 1);
}

/// Tape 45's roll is on 7875 (shown from 7876); tape 07's is on 24840
/// (shown from 24841). These are video-frame offsets from those first views,
/// not the old 36-frame narration dwell. Both spend fourteen seed steps.
#[test]
fn opening_names_options_and_seed_follow_the_two_retail_tapes() {
    for (size, full_frame, lag_frames) in [(1, 15, vec![1]), (3, 16, vec![1, 13])] {
        let Some(mut runtime) = runtime() else { return };
        if size == 1 {
            runtime
                .game
                .set_party([Some(CharId(0)), None, None, None, None]);
        }
        runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
        let timeline = BattleTimeline {
            events: vec![BattleEvent::Started {
                priority: Priority::Normal,
                enemies: vec![FighterId::new(6).unwrap(), FighterId::new(7).unwrap()],
            }],
            sounds: Vec::new(),
            animations: Vec::new(),
        };
        let mut mode = BattleMode::begin(&runtime, timeline, BattleStart::Encounter(0x8A));
        let initial_count = runtime.frames;
        let mut expected_seed = runtime.rng.clone();
        let start = mode.start_frame(&runtime);
        assert!(start.advance_enemy_overlays);
        let first = start.view.unwrap();
        assert_eq!(
            first.opening_name_width,
            Some(if size == 1 { 2 } else { 0 })
        );
        assert!(first.opening_first_upload);
        assert!(first.menu.is_none());
        assert!(!first.ready);
        let mut counted = 0;
        for elapsed in 1..=full_frame {
            let frame = mode.frame(&mut runtime, Pad::new(Button::Speak));
            assert_eq!(frame.advance_enemy_overlays, !lag_frames.contains(&elapsed));
            let view = frame.view.unwrap();
            assert!(!view.opening_first_upload);
            if !lag_frames.contains(&elapsed) {
                counted += 1;
                expected_seed.step();
            }
            assert_eq!(runtime.frames, initial_count.wrapping_add(counted));
            assert_eq!(runtime.rng_seed(), expected_seed.seed());
            if elapsed < full_frame {
                assert!(!view.ready, "opening input is ignored at {elapsed}");
                assert!(mode.window.is_none());
            } else {
                assert!(view.ready);
                assert_eq!(view.opening_name_width, None);
                assert_eq!(
                    view.menu,
                    Some(MenuView::Top {
                        cursor: 0,
                        width: None,
                        full_width: 8
                    })
                );
                assert!(
                    !view.cursor_red,
                    "first full frame precedes the cursor routine"
                );
            }
            // The first six distinct writes are 2, 4, 6, 8, 10, 12 cells;
            // the first VInt repeats the two-cell window rather than growing it.
            if elapsed <= 6 {
                assert_eq!(
                    view.opening_name_width,
                    Some([2, 4, 6, 8, 10, 12][elapsed as usize - 1])
                );
            }
            if elapsed == 12 || (size == 3 && elapsed == 13) {
                assert_eq!(
                    view.menu,
                    Some(MenuView::Top {
                        cursor: 0,
                        width: Some(2),
                        full_width: 8
                    })
                );
            }
        }
        assert_eq!(counted, 14);
    }
}

#[test]
fn special_priorities_do_not_use_the_normal_options_opening() {
    for priority in [Priority::Preemptive, Priority::Ambush] {
        let Some(mut runtime) = runtime() else { return };
        runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
        let mode = BattleMode::begin(
            &runtime,
            BattleTimeline {
                events: vec![BattleEvent::Started {
                    priority,
                    enemies: vec![FighterId::new(6).unwrap(), FighterId::new(7).unwrap()],
                }],
                sounds: Vec::new(),
                animations: Vec::new(),
            },
            BattleStart::Encounter(0x8A),
        );
        assert!(
            mode.opening.is_none(),
            "{priority:?} has its own retail routine"
        );
        assert_eq!(mode.current.unwrap().remaining, BATTLE_DWELL_FRAMES);
    }
}

/// The shell must receive the same clock contract for every entry route,
/// including a display frame that does not run GameMode_Battle.
#[test]
fn opening_overlay_clock_holds_across_encounter_event_and_debug_entry() {
    for entry in ["encounter", "event", "debug"] {
        let Some(mut runtime) = runtime() else { return };
        // This seed gives an ordinary priority roll in the encounter routes.
        runtime.rng = psiv_core::battle::Lcg41::new(0);
        let mut session = Session::new(runtime);
        let start = match entry {
            "encounter" => session
                .begin_battle(&[RuntimeEvent::EncounterBattleReady { formation: 0x8A }])
                .unwrap(),
            "event" => {
                // Scene entry has already created the fight and carries its timeline.
                session
                    .runtime
                    .start_battle(0x8A, session.runtime.battle_party())
                    .unwrap();
                session
                    .begin_battle(&[RuntimeEvent::SceneBattleStarted {
                        index: 7,
                        events: vec![BattleEvent::Started {
                            priority: Priority::Normal,
                            enemies: vec![FighterId::new(6).unwrap(), FighterId::new(7).unwrap()],
                        }],
                        sounds: Vec::new(),
                        animations: Vec::new(),
                    }])
                    .unwrap()
            }
            "debug" => session.debug_battle(0x8A),
            _ => unreachable!(),
        };
        assert!(
            start.advance_enemy_overlays,
            "{entry}: initial battle-loop call"
        );
        assert!(start.view.unwrap().opening_first_upload, "{entry}");
        let mut overlay_calls = 1;
        for elapsed in 1..=17 {
            let count = session.main_frame_count();
            let frame = session.frame(Pad::NEUTRAL).battle.unwrap();
            let held = elapsed == 1 || elapsed == 13;
            assert_eq!(
                frame.advance_enemy_overlays, !held,
                "{entry} frame {elapsed}"
            );
            assert_eq!(
                session.main_frame_count().wrapping_sub(count),
                u16::from(!held)
            );
            overlay_calls += usize::from(frame.advance_enemy_overlays);
            if elapsed == 16 {
                assert!(frame.view.unwrap().ready, "{entry}: full options");
                assert_eq!(
                    overlay_calls, 15,
                    "{entry}: no overlay calls during either hold"
                );
            }
        }
    }
}

/// These sizes exercise the documented approximation, not new retail evidence.
#[test]
fn opening_overlay_clock_holds_for_every_supported_party_size() {
    for (size, mounted) in [
        (1, false),
        (2, false),
        (3, false),
        (4, false),
        (5, false),
        (1, true),
    ] {
        let Some(mut runtime) = runtime() else { return };
        let record = runtime.game.roster().get(CharId(0)).unwrap().clone();
        for id in 0..5 {
            runtime
                .game
                .roster_mut()
                .seat(CharId(id), record.clone())
                .unwrap();
        }
        runtime.game.set_party(std::array::from_fn(|i| {
            (i < size).then(|| CharId(u8::try_from(i).unwrap()))
        }));
        if mounted {
            runtime.set_vehicle_index(2).unwrap();
            assert_eq!(runtime.battle_party().len(), 1);
        }
        runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
        let mut mode = BattleMode::begin(
            &runtime,
            BattleTimeline {
                events: vec![BattleEvent::Started {
                    priority: Priority::Normal,
                    enemies: vec![FighterId::new(6).unwrap()],
                }],
                sounds: Vec::new(),
                animations: Vec::new(),
            },
            BattleStart::Encounter(0x8A),
        );
        assert!(mode.start_frame(&runtime).advance_enemy_overlays);
        let options_lag = (size - 1) / 2;
        let mut calls = 1;
        let full_frame = 15 + options_lag + usize::from(mounted);
        for elapsed in 1..=full_frame {
            let frame = mode.frame(&mut runtime, Pad::NEUTRAL);
            let held = elapsed == 1 || (13..13 + options_lag).contains(&elapsed);
            assert_eq!(
                frame.advance_enemy_overlays, !held,
                "party {size}, frame {elapsed}"
            );
            calls += usize::from(frame.advance_enemy_overlays);
            let view = frame.view.unwrap();
            assert_eq!(view.ready, elapsed == full_frame);
            if elapsed == full_frame {
                assert!(
                    matches!(view.menu, Some(MenuView::Top { full_width, .. }) if full_width == if mounted { 9 } else { 8 })
                );
            }
        }
        assert_eq!(
            calls,
            15 + usize::from(mounted),
            "party {size}, mounted {mounted}: one initial call plus the counted calls"
        );
    }
}
