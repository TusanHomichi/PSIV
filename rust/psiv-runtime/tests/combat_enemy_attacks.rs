//! Real pack enemy dispatch, presentation cues and persistent battle handoff.
use psiv_core::battle::{BattleEvent, Command, FighterId, Outcome, RoundOrders};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn flattrplnt_acid_breath_has_its_own_turn_sound_cues_and_save_handoff() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
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
    // Constructed durability fixture, unrelated to the connected native save.
    for character in [1, 0, 2] {
        let stats = game.roster_mut().get_mut(CharId(character)).unwrap();
        stats.max_hp = 400;
        stats.curr_hp = 400;
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
    if let Some(dir) = std::env::var_os("PSIV_ENEMY_ATTACK_SAVE_DIR") {
        runtime.save_slot(Path::new(&dir), 0).unwrap();
    }
    runtime
        .start_battle_timeline(150, runtime.battle_party())
        .unwrap();
    runtime.set_rng_seed(0x0101_5678);
    let mut found = false;
    for _ in 0..10 {
        let timeline = runtime
            .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
            .unwrap();
        assert!(
            !timeline
                .events
                .iter()
                .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. }))
        );
        if let Some((index, actor)) =
            timeline
                .events
                .iter()
                .enumerate()
                .find_map(|(i, e)| match e {
                    BattleEvent::EnemySkillUsed {
                        actor,
                        skill: 51,
                        name,
                    } => {
                        assert_eq!(name, "ACIDBREATH");
                        Some((i, *actor))
                    }
                    _ => None,
                })
        {
            assert!(
                matches!(timeline.events[index + 1], BattleEvent::Resolved { actor: caster, damage: Some(_), .. } if caster == actor)
            );
            assert!(!timeline.events.iter().any(
                |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if *attacker == actor)
            ));
            assert!(
                timeline
                    .sounds
                    .iter()
                    .any(|s| s.event_index == index && s.id == 0xD5)
            );
            assert!(
                timeline
                    .sounds
                    .iter()
                    .any(|s| s.event_index == index + 1 && s.id == 0xD8)
            );
            assert!(
                timeline.animations.is_empty(),
                "do not substitute the plain attack animation"
            );
            found = true;
            break;
        }
    }
    assert!(found, "fixed seed must exercise the real ability record");
    let purse = runtime.game().money();
    let mut won = false;
    for _ in 0..12 {
        let events = runtime.battle_round(&RoundOrders::attack_all()).unwrap();
        if events.contains(&BattleEvent::Ended {
            outcome: Outcome::Victory,
        }) {
            let exp = events
                .iter()
                .find_map(|e| match e {
                    BattleEvent::Rewarded {
                        experience_each, ..
                    } => Some(*experience_each),
                    _ => None,
                })
                .unwrap();
            runtime.finish_battle_for_outcome(Outcome::Victory, exp);
            runtime.return_to_field();
            won = true;
            break;
        }
    }
    assert!(won);
    assert_eq!(runtime.game().money(), purse + 30);
    let dir = std::env::temp_dir().join(format!("psiv-acid-save-{}", std::process::id()));
    runtime.save_slot(&dir, 0).unwrap();
    let mut loaded = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        &dir,
        0,
        StepFrames::default(),
    )
    .unwrap();
    loaded.enable_battles(&files).unwrap();
    assert_eq!(loaded.game().snapshot(), runtime.game().snapshot());
    std::fs::remove_dir_all(dir).unwrap();
}

/// `EnemyAttackOffs` (`ps4.asm:19206`) sends 76 FlyScreamr and 85 Piercer down
/// two different `$33` arms: `$4C` is `EnemyAttack_FlattrPlnt`
/// (`ps4.asm:21778`), `$55` is `EnemyAttack_Piercer` (`ps4.asm:21518`). Both
/// roll the real ability record from formation `$D5` (one FlyScreamr) and
/// `$129` (two Piercers), and both must resolve it instead of announcing
/// `UnsupportedAbility` and swinging.
#[test]
fn newly_supported_acid_breath_carriers_resolve_in_their_real_formations() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    // Fixed seeds, first round each; the loop below tolerates a later one.
    for (formation, carrier, seed) in [
        (0xD5u16, 76u16, 0x9E37_79B9u32),
        (0x129u16, 85u16, 0x0101_5678u32),
    ] {
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
        // Constructed durability fixture, unrelated to the connected native
        // save: headroom for the round search, so no member's death ends the
        // battle before the carrier's `$33` roll shows up.
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
            .start_battle_timeline(formation, runtime.battle_party())
            .unwrap();
        runtime.set_rng_seed(seed);
        let mut found = false;
        for _ in 0..4 {
            let timeline = runtime
                .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
                .unwrap();
            assert!(
                !timeline
                    .events
                    .iter()
                    .any(|e| matches!(e, BattleEvent::UnsupportedAbility { ability: 51, .. })),
                "carrier {carrier}: {:?}",
                timeline.events
            );
            let Some((index, actor)) =
                timeline
                    .events
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| match e {
                        BattleEvent::EnemySkillUsed {
                            actor,
                            skill: 51,
                            name,
                        } => {
                            assert_eq!(name, "ACIDBREATH");
                            Some((i, *actor))
                        }
                        _ => None,
                    })
            else {
                continue;
            };
            assert!(
                matches!(timeline.events[index + 1], BattleEvent::Resolved { actor: caster, damage: Some(_), .. } if caster == actor),
                "carrier {carrier}: {:?}",
                timeline.events
            );
            assert!(
                !timeline.events.iter().any(
                    |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if *attacker == actor)
                ),
                "carrier {carrier}: the ability replaces the swing: {:?}",
                timeline.events
            );
            assert!(
                timeline
                    .sounds
                    .iter()
                    .any(|s| s.event_index == index && s.id == 0xD5),
                "carrier {carrier}: MoleAttack starts the wind-up"
            );
            assert!(
                timeline
                    .sounds
                    .iter()
                    .any(|s| s.event_index == index + 1 && s.id == 0xD8),
                "carrier {carrier}: EnemyAttack4 precedes the damage reaction"
            );
            assert!(
                !timeline.animations.iter().any(|a| a.actor == actor),
                "carrier {carrier}: do not substitute the plain attack animation"
            );
            found = true;
            break;
        }
        assert!(
            found,
            "carrier {carrier} must resolve its own $33 in formation {formation:#x}"
        );
    }
}

#[test]
fn carrion_crawler_thread_replaces_its_attack_in_the_real_formation() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
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
    // Original Raja is sturdy enough to observe the four crawlers without
    // fabricated stats or consuming the connected campaign save.
    game.set_party([Some(CharId(8)), None, None, None, None]);
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
    rt.set_rng_seed(0x1234_5678);
    rt.start_battle(0x9b, rt.battle_party()).unwrap();
    let mut seen = 0;
    let mut lowered = false;
    let mut casts = 0;
    for _ in 0..8 {
        let timeline = rt
            // DEFEND grants physical resistance and can make THREAD miss.
            // Spend Raja's learned ANTI on himself instead; no invented wait.
            .battle_round_timeline(&RoundOrders::Commands(vec![Command::Technique {
                technique: 34,
                target: psiv_core::battle::FighterId::new(1),
            }]))
            .unwrap();
        casts += timeline
            .events
            .iter()
            .filter(|e| matches!(e, BattleEvent::TechniqueUsed { technique: 34, .. }))
            .count() as u16;
        assert!(
            !timeline
                .events
                .iter()
                .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. })),
            "{:?}",
            timeline.events
        );
        for (index, event) in timeline.events.iter().enumerate() {
            if let BattleEvent::EnemySkillUsed {
                actor, skill: 16, ..
            } = event
            {
                seen += 1;
                assert!(
                    timeline
                        .sounds
                        .iter()
                        .any(|s| s.event_index == index && s.id == 0xda)
                );
                assert!(
                    !timeline.events.iter().any(
                        |e| matches!(e,BattleEvent::Attacked{actor:attacker,..} if attacker==actor)
                    ),
                    "THREAD must not also attack: {:?}",
                    timeline.events
                );
            }
            if matches!(
                event,
                BattleEvent::StatChanged {
                    stat: psiv_core::battle::TechniqueStat::Agility,
                    value: 1,
                    ..
                }
            ) {
                lowered = true;
            }
        }
        if seen > 0 && lowered {
            break;
        }
    }
    assert!(seen > 0 && lowered);
    let mut escaped = false;
    for _ in 0..30 {
        if rt
            .battle_round(&RoundOrders::Run)
            .unwrap()
            .contains(&BattleEvent::Ended {
                outcome: Outcome::Escaped,
            })
        {
            escaped = true;
            break;
        }
    }
    assert!(escaped);
    let before_money = rt.game().money();
    rt.finish_battle_for_outcome(Outcome::Escaped, 0);
    assert_eq!(rt.game().money(), before_money);
    assert_eq!(
        rt.game().roster().get(CharId(8)).unwrap().curr_tp,
        170 - casts * 2
    );
    let before = rt.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-thread-save-{}", std::process::id()));
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

/// FLAME BOLT `$02` is real-pack reachable: `generated/enemies.json` gives 0
/// Helex `$02` in all eight regular slots, and formation `$5E`/94 is two of
/// them. The ability must resolve through
/// `enemy_damage::resolve_damage_skill` — one `Resolved` with damage and no
/// physical swing — rather than announcing `UnsupportedAbility` and swinging.
#[test]
fn helex_flame_bolt_resolves_in_its_real_formation() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
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
    // Constructed durability fixture, unrelated to the connected native save:
    // two Helex cannot end the battle before their `$02` roll shows up.
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
        .start_battle_timeline(0x5E, runtime.battle_party())
        .unwrap();
    runtime.set_rng_seed(0x1234_5678);
    let mut found = false;
    for _ in 0..4 {
        let timeline = runtime
            .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
            .unwrap();
        assert!(
            !timeline
                .events
                .iter()
                .any(|e| matches!(e, BattleEvent::UnsupportedAbility { ability: 2, .. })),
            "{:?}",
            timeline.events
        );
        let Some((index, actor)) = timeline
            .events
            .iter()
            .enumerate()
            .find_map(|(i, e)| match e {
                BattleEvent::EnemySkillUsed {
                    actor,
                    skill: 2,
                    name,
                } => {
                    assert_eq!(name, "FLAME BOLT");
                    Some((i, *actor))
                }
                _ => None,
            })
        else {
            continue;
        };
        assert!(
            matches!(timeline.events[index + 1], BattleEvent::Resolved { actor: caster, damage: Some(_), .. } if caster == actor),
            "{:?}",
            timeline.events
        );
        assert!(
            !timeline.events.iter().any(
                |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if *attacker == actor)
            ),
            "the ability replaces the swing: {:?}",
            timeline.events
        );
        assert!(
            timeline
                .sounds
                .iter()
                .any(|s| s.event_index == index && s.id == 0xD7),
            "EnemyAttack3 starts BattleObj_HelexFlameBolt"
        );
        assert!(
            timeline
                .sounds
                .iter()
                .any(|s| s.event_index == index + 1 && s.id == 0xC2),
            "FireBreath precedes the damage reaction"
        );
        assert!(
            !timeline.animations.iter().any(|a| a.actor == actor),
            "do not substitute the plain attack animation"
        );
        found = true;
        break;
    }
    assert!(found, "fixed seed must resolve a real FLAME BOLT");
    let escaped = (0..10).any(|_| {
        runtime
            .battle_round(&RoundOrders::Run)
            .unwrap()
            .contains(&BattleEvent::Ended {
                outcome: Outcome::Escaped,
            })
    });
    assert!(escaped);
    let before = runtime.game().snapshot();
    let dir = std::env::temp_dir().join(format!("psiv-flame-save-{}", std::process::id()));
    runtime.save_slot(&dir, 0).unwrap();
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

/// The Motavia single-target routes are real-pack reachable.
/// `generated/enemies.json` gives 81 DesrtLeach `$37` SAND STORM in its last
/// three regular slots and formation `$53` is one of them; 99 TechUser has
/// `$40` WAT in slots 1-4 and `$44` FOI in 5-8, and formation `$2A` is two of
/// them. Each must resolve through `enemy_damage::resolve_damage_skill` — one
/// `Resolved` with damage, the object's wind-up cue, no `UnsupportedAbility`
/// for the ability and no physical swing by that caster.
#[test]
fn motavia_single_target_abilities_resolve_in_their_real_formations() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    for (formation, carrier, abilities, seed) in [
        (0x53u16, 81u16, &[55u8][..], 0x9E37_79B9u32),
        (0x2Au16, 99u16, &[64u8, 68][..], 0x0101_5678u32),
    ] {
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
        // Constructed durability fixture, unrelated to the connected native
        // save: headroom for the round search, so no member's death ends the
        // battle before the carrier's roll shows up. DesrtLeach's SAND STORM
        // is the strongest of these records.
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
            .start_battle_timeline(formation, runtime.battle_party())
            .unwrap();
        runtime.set_rng_seed(seed);
        let mut found = false;
        for _ in 0..8 {
            let timeline = runtime
                .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
                .unwrap();
            for ability in abilities {
                assert!(
                    !timeline.events.iter().any(|e| matches!(
                        e,
                        BattleEvent::UnsupportedAbility { ability: reported, .. }
                            if reported == ability
                    )),
                    "carrier {carrier} formation {formation:#x} ability {ability}: {:?}",
                    timeline.events
                );
            }
            // Every occurrence is checked, not just the first: a later round
            // may roll the other ability of the same carrier's list.
            for (index, event) in timeline.events.iter().enumerate() {
                let BattleEvent::EnemySkillUsed { actor, skill, .. } = event else {
                    continue;
                };
                if !abilities.contains(skill) {
                    continue;
                }
                let (actor, skill) = (*actor, *skill);
                found = true;
                assert!(
                    matches!(timeline.events[index + 1], BattleEvent::Resolved { actor: caster, damage: Some(_), .. } if caster == actor),
                    "carrier {carrier} ability {skill}: {:?}",
                    timeline.events
                );
                assert!(
                    !timeline.events.iter().any(
                        |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if *attacker == actor)
                    ),
                    "carrier {carrier} ability {skill}: the ability replaces the swing: {:?}",
                    timeline.events
                );
                assert!(
                    timeline
                        .sounds
                        .iter()
                        .any(|s| s.event_index == index && s.id == 0xD5),
                    "carrier {carrier} ability {skill}: MoleAttack starts every one of these wind-ups"
                );
                if skill == 68 {
                    assert!(
                        timeline
                            .sounds
                            .iter()
                            .any(|s| s.event_index == index + 1 && s.id == 0xBB),
                        "FOI's chain writes TechCast as the request phase starts"
                    );
                }
                assert!(
                    !timeline.animations.iter().any(|a| a.actor == actor),
                    "carrier {carrier} ability {skill}: do not substitute the plain attack animation"
                );
            }
        }
        assert!(
            found,
            "carrier {carrier} must roll its own ability in formation {formation:#x}"
        );
    }
}

/// The Motavia all-party routes are real-pack reachable. `generated/enemies.json`
/// gives 15 Fanbite `$08` SPIRAL BLD in regular slots 7-8 and formation `$37` is
/// two of them; 80 SandWorm has `$38` EARTHQUAKE in slots 6-8 and formation
/// `$3B` is one of them. Both must resolve through
/// `enemy_damage::resolve_damage_skill`'s all-party class: one `Resolved` on
/// every living party member, in slot order, after one `EnemySkillUsed`, with
/// no `UnsupportedAbility` for the ability and no physical swing by that caster.
/// 149 KingRappy's `$38` is the third pair of the class; its only formation is a
/// boss one (`EnemyAttack_KingRappy`, `ps4.asm:19596`), and boss formations are
/// started by scenes, so it is covered by the core tests alone.
#[test]
fn motavia_all_party_abilities_resolve_in_their_real_formations() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    for (formation, carrier, ability, display, seed) in [
        (0x37u16, 15u16, 8u8, "SPIRAL BLD", 0x9E37_79B9u32),
        (0x3Bu16, 80u16, 56u8, "EARTHQUAKE", 0x0101_5678u32),
    ] {
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
        // Constructed durability fixture, unrelated to the connected native
        // save: SPIRAL BLD and EARTHQUAKE take more than a hundred hit points
        // off each member per use, so a realistic defense would let the round
        // search lose a member — and with it a target of the whole-party case —
        // before the ability roll showed up. `battle_party` hands these stats to
        // the battle unchanged, and a defense word this large clamps both
        // records to `loc_266C`'s minimum damage, so every member is still in
        // the fight for the whole search. The numbers themselves are pinned by
        // the core tests.
        for character in [1, 0, 2] {
            let stats = game.roster_mut().get_mut(CharId(character)).unwrap();
            stats.max_hp = 999;
            stats.curr_hp = 999;
            stats.defence.battle = 5000;
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
            .start_battle_timeline(formation, runtime.battle_party())
            .unwrap();
        runtime.set_rng_seed(seed);
        let mut found = false;
        for _ in 0..12 {
            let timeline = runtime
                .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
                .unwrap();
            assert!(
                !timeline.events.iter().any(|e| matches!(
                    e,
                    BattleEvent::UnsupportedAbility { ability: reported, .. }
                        if *reported == ability
                )),
                "carrier {carrier} formation {formation:#x} ability {ability}: {:?}",
                timeline.events
            );
            // Every occurrence is checked, not just the first.
            for (index, event) in timeline.events.iter().enumerate() {
                let BattleEvent::EnemySkillUsed { actor, skill, name } = event else {
                    continue;
                };
                if *skill != ability {
                    continue;
                }
                let (actor, skill) = (*actor, *skill);
                assert_eq!(name, display, "carrier {carrier}");
                found = true;
                // The class puts one `Resolved` on every living party member,
                // in `Battle_UpdateFighters` order, and a `Died` between them
                // would belong to a member that just emptied its HP.
                let mut hits = Vec::new();
                for (offset, event) in timeline.events[index + 1..].iter().enumerate() {
                    match event {
                        BattleEvent::Resolved {
                            actor: caster,
                            target,
                            damage: Some(damage),
                            remaining_hp,
                            ..
                        } if *caster == actor => {
                            assert!(*damage > 0, "carrier {carrier}");
                            assert!(
                                u32::from(*remaining_hp) + u32::from(*damage) <= 999,
                                "carrier {carrier}: hit points cannot grow"
                            );
                            hits.push((index + 1 + offset, *target));
                        }
                        BattleEvent::Died { .. } => {}
                        _ => break,
                    }
                }
                assert_eq!(
                    hits.iter().map(|(_, target)| *target).collect::<Vec<_>>(),
                    vec![
                        FighterId::new(1).unwrap(),
                        FighterId::new(2).unwrap(),
                        FighterId::new(3).unwrap()
                    ],
                    "carrier {carrier} ability {skill}: the whole party, in slot order: {:?}",
                    timeline.events
                );
                assert!(
                    !timeline.events.iter().any(
                        |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if *attacker == actor)
                    ),
                    "carrier {carrier} ability {skill}: the ability replaces the swing: {:?}",
                    timeline.events
                );
                assert!(
                    timeline
                        .sounds
                        .iter()
                        .any(|s| s.event_index == index && s.id == wind_up_sound(skill)),
                    "carrier {carrier} ability {skill}: the object's wind-up cue is mapped"
                );
                if skill == 8 {
                    // Fanbite's object flinches each member it passes with
                    // EnemyAttack1 (`ps4.asm:29314`); the port attaches that cue
                    // to the member's own `Resolved`.
                    for (hit_index, target) in &hits {
                        assert!(
                            timeline
                                .sounds
                                .iter()
                                .any(|s| s.event_index == *hit_index && s.id == 0xBA),
                            "carrier {carrier} ability {skill}: member {target:?} keeps its flinch cue"
                        );
                    }
                }
                assert!(
                    !timeline.animations.iter().any(|a| a.actor == actor),
                    "carrier {carrier} ability {skill}: do not substitute the plain attack animation"
                );
            }
        }
        assert!(
            found,
            "carrier {carrier} must roll its own ability in formation {formation:#x}"
        );
    }
}

/// The wind-up sound each all-party chain writes into `Sound_Index`:
/// `SFXID_Slasher` `$B7` for `BattleObj_LocustaSpiralBld` (`ps4.asm:29206`),
/// `SFXID_GraveOpening` `$DD` for `BattleObj_Earthquake` (`ps4.asm:47917`).
fn wind_up_sound(skill: u8) -> u8 {
    match skill {
        8 => 0xB7,
        56 => 0xDD,
        other => panic!("no all-party cue for {other}"),
    }
}
