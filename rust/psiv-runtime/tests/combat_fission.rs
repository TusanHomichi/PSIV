//! First-boss Fission: live formation slots, repeat kills and save handoff.
use psiv_core::battle::{BattleEvent, Command, FighterId, ItemSource, Outcome, RoundOrders, Side};
use psiv_core::{
    Cell, CharId, Direction, GameState, Input, RetailLocation, RetailSave, StepFrames,
};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};
use std::path::Path;
const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

#[test]
fn igglanova_replaces_the_killed_neighbor_then_can_be_defeated_and_saved() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x13,
        Cell::new(48, 19),
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
    game.inventory_mut().add(139).unwrap();
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
    if let Some(dir) = std::env::var_os("PSIV_FISSION_SMOKE_SAVE_DIR") {
        runtime.save_slot(Path::new(&dir), 0).unwrap();
    }
    assert!(runtime.start_event(0x6B));
    for _ in 0..100 {
        for event in runtime.tick(Input::Neutral) {
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        if runtime.battle_active() {
            break;
        }
    }
    assert!(runtime.battle_active());
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .living(psiv_core::battle::Side::Enemy)
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![id(7)]
    );
    runtime.set_rng_seed(0x0101_5678);
    for expected_alive in [2, 3] {
        let events = runtime
            .battle_round(&RoundOrders::Commands(vec![Command::Defend; 3]))
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::EnemyReplenished { .. }))
                .count(),
            1
        );
        assert_eq!(
            runtime
                .battle_roster()
                .unwrap()
                .living(psiv_core::battle::Side::Enemy)
                .count(),
            expected_alive
        );
    }
    let timeline = runtime
        .battle_round_timeline(&RoundOrders::Commands(vec![
            Command::Defend,
            Command::Item {
                item: 139,
                source: ItemSource::Inventory(0),
                target: Some(id(6)),
            },
            Command::Defend,
        ]))
        .unwrap();
    assert!(
        timeline
            .events
            .contains(&BattleEvent::Died { fighter: id(6) })
    );
    assert!(timeline.events.contains(&BattleEvent::EnemyReplenished {
        actor: id(7),
        fighter: id(6),
        enemy_id: 9,
        name: "XANAFALGUE".into(),
        hp: 16
    }));
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. }))
    );
    assert_eq!(runtime.game().inventory().get(0), None);
    let battle = runtime.battle_roster().unwrap();
    assert_eq!(battle.get(id(6)).unwrap().stats.curr_hp, 16);
    assert_eq!(battle.get(id(7)).unwrap().stats.curr_hp, 300);
    assert_eq!(battle.get(id(8)).unwrap().stats.curr_hp, 16);
    let mut outcome = None;
    let mut reward = 0;
    for _ in 0..80 {
        let orders = RoundOrders::Commands(vec![
            Command::AttackTarget(id(7)),
            Command::Attack,
            Command::AttackTarget(id(7)),
        ]);
        for e in runtime.battle_round(&orders).unwrap() {
            match e {
                BattleEvent::Ended { outcome: result } => outcome = Some(result),
                BattleEvent::Rewarded {
                    experience_each, ..
                } => reward = experience_each,
                _ => {}
            }
        }
        if outcome.is_some() {
            break;
        }
    }
    assert_eq!(outcome, Some(Outcome::Victory));
    assert!(runtime.map().npcs()[..3].iter().all(|npc| npc.active));
    let party_before_return = runtime.members();
    let camera_before_return = *runtime.camera();
    runtime.finish_battle_for_outcome(Outcome::Victory, reward);
    assert_eq!(
        runtime.tick(Input::Neutral),
        vec![RuntimeEvent::MapRefreshed]
    );
    assert!(runtime.map().npcs()[..3].iter().all(|npc| !npc.active));
    assert!(runtime.map().npc_at(Cell::new(15, 10)).is_none());
    assert!(runtime.map().npc_at(Cell::new(16, 10)).is_none());
    assert_eq!(runtime.members(), party_before_return);
    assert_eq!(*runtime.camera(), camera_before_return);
    assert!(runtime.return_to_field().is_empty());
    for _ in 0..10000 {
        for e in runtime.tick(Input::Neutral) {
            if matches!(
                e,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                runtime.dialogue_closed();
            }
        }
        if !runtime.scene_active() {
            break;
        }
    }
    assert!(!runtime.scene_active());
    let saved = runtime.game().snapshot();
    let directory = std::env::temp_dir().join(format!("psiv-fission-save-{}", std::process::id()));
    runtime.save_slot(&directory, 0).unwrap();
    let mut continued = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        &directory,
        0,
        StepFrames::default(),
    )
    .unwrap();
    continued.enable_battles(&files).unwrap();
    assert_eq!(continued.game().snapshot(), saved);
    std::fs::remove_dir_all(directory).unwrap();
}

/// A durable three-member party at the pack's own stats, for the two
/// FloatMine2 formations below. The HP headroom is a constructed fixture, not
/// campaign state: FloatMine2 hits for 136 and Tower for 100, and the round
/// search needs the battle to outlive it.
fn durable_runtime(pack: &Path) -> Runtime {
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x2B,
        Cell::new(17, 52),
        Direction::Up,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        None,
        None,
    ]);
    for character in [1, 0, 2] {
        let stats = game.roster_mut().get_mut(CharId(character)).unwrap();
        stats.max_hp = 999;
        stats.curr_hp = 999;
    }
    let mut runtime = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x2B,
                char_x: 17 * 16,
                char_y: 52 * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    runtime.enable_battles(&files).unwrap();
    runtime
}

/// `EnemyAttackOffs` `$32` (`ps4.asm:19257`) sends 50 FloatMine2 to
/// `EnemyAttack_FloatMine` (`ps4.asm:22675`), whose arms cover `$14`, `$18`,
/// `$19` and `$1A` only. Its regular `$07` and `$17` therefore reach the
/// fall-through `loc_10406` (`ps4.asm:22781`) and the actor's turn ends with
/// nothing loaded, nothing drawn and nothing resolved.
///
/// Formation 263 of the pack is a real one — FloatMine2, Tower, FloatMine2, the
/// only kind of formation that carries the id (`2/504` in
/// `docs/battle/ENEMY_ABILITIES.md`) — and Tower's eight slots are all zero, so every
/// ability event in this battle belongs to a FloatMine2.
#[test]
fn floatmine2_formations_spend_fission2_and_waiting_turns_without_a_swing() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let mut runtime = durable_runtime(pack);
    runtime
        .start_battle_timeline(263, runtime.battle_party())
        .unwrap();
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .side(Side::Enemy)
            .map(|f| f.stats.enemy_id)
            .collect::<Vec<_>>(),
        vec![50, 39, 50]
    );
    runtime.set_rng_seed(0x0101_5678);
    let mut found = 0;
    let mut seen_waiting = 0;
    for round in 1..=8 {
        let timeline = runtime
            .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
            .unwrap();
        for (index, event) in timeline.events.iter().enumerate() {
            let BattleEvent::EnemyAbilityWasted {
                actor,
                ability,
                name,
            } = event
            else {
                continue;
            };
            let expected = match ability {
                7 => "FISSION",
                23 => "WAITING",
                other => panic!("round {round}: unexpected roll {other}: {event:?}"),
            };
            assert_eq!(name.as_str(), expected, "round {round}: {event:?}");
            assert!(
                actor.get() == 6 || actor.get() == 8,
                "round {round}: only the two FloatMine2 slots can roll it: {event:?}"
            );
            assert!(
                !timeline.events.iter().any(|e| matches!(
                    e,
                    BattleEvent::Attacked { actor: attacker, .. } if attacker == actor
                )),
                "round {round}: no physical swing may follow: {:?}",
                timeline.events
            );
            assert!(
                !timeline.events.iter().any(|e| matches!(
                    e,
                    BattleEvent::UnsupportedAbility { actor: rolled, .. } if rolled == actor
                )),
                "round {round}: the roll was proven, so nothing falls back: {:?}",
                timeline.events
            );
            assert!(
                !timeline.events.iter().any(
                    |e| matches!(e, BattleEvent::Resolved { actor: hitter, .. } if hitter == actor)
                ),
                "round {round}: the actor deals neither damage nor a miss: {:?}",
                timeline.events
            );
            // The presentation sidecar follows `Attacked`, so an empty turn
            // cannot have an attack animation either.
            assert!(
                !timeline.animations.iter().any(|a| a.actor == *actor),
                "round {round}: no attack animation: {:?}",
                timeline.animations
            );
            assert!(
                !timeline.sounds.iter().any(|s| s.event_index == index),
                "round {round}: retail writes no Sound_Index on this path: {:?}",
                timeline.sounds
            );
            found += usize::from(*ability == 7);
            seen_waiting += usize::from(*ability == 23);
        }
    }
    assert!(
        found > 0 && seen_waiting > 0,
        "the fixed seed must reach both real FloatMine2 rolls: {found} FISSION, {seen_waiting} WAITING"
    );
}

/// The same formation roster with 45 CommndBall in the middle — pack formation
/// 292 — rolls `$19` Detonation, one of the routine's *arms*. Its object is
/// untraced, so it keeps the ordinary fallback: an `UnsupportedAbility` notice
/// and a physical swing. The wasted-turn witness must not swallow an arm.
#[test]
fn a_float_mine_arm_on_a_real_formation_keeps_the_physical_fallback() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let mut runtime = durable_runtime(pack);
    runtime
        .start_battle_timeline(292, runtime.battle_party())
        .unwrap();
    assert_eq!(
        runtime
            .battle_roster()
            .unwrap()
            .side(Side::Enemy)
            .map(|f| f.stats.enemy_id)
            .collect::<Vec<_>>(),
        vec![50, 45, 50]
    );
    runtime.set_rng_seed(0x0101_5678);
    let timeline = runtime
        .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
        .unwrap();
    assert!(
        timeline.events.contains(&BattleEvent::UnsupportedAbility {
            actor: id(7),
            ability: 25,
        }),
        "CommndBall's whole list is `$19`: {:?}",
        timeline.events
    );
    assert!(
        timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { actor, .. } if *actor == id(7))),
        "{:?}",
        timeline.events
    );
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemyAbilityWasted { ability: 25, .. })),
        "{:?}",
        timeline.events
    );
}
