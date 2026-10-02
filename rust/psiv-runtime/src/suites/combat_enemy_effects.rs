//! Real-pack status and stat abilities of the Motavia arc: VOICE's sleep and
//! the Zol slugs' Fusion, through a live formation, and what the field carries
//! out of the battle (`docs/battle/ENEMY_EFFECT_ABILITIES.md`,
//! `docs/battle/ENEMY_FUSION.md`).
use crate::Runtime;
use psiv_core::battle::{BattleEvent, Command, Outcome, RoundOrders, Side, status};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// A runtime on a constructed durability fixture: Chaz, Alys and Hahn with 999
/// HP, unrelated to any native save, so that a formation's opening rounds can be
/// watched.
fn runtime(pack: &Path, files: &BattleFiles) -> Runtime {
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x2B,
        Cell::new(17, 52),
        Direction::Up,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(files).unwrap();
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
        assert_eq!(stats.status, 0, "the fixture starts with no ailment");
    }
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x2B,
                map_index_2: 0,
                char_x: 17 * 16,
                char_y: 52 * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(files).unwrap();
    rt
}

fn leave(rt: &mut Runtime) {
    let mut escaped = false;
    for _ in 0..40 {
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
    rt.finish_battle_for_outcome(Outcome::Escaped, 0);
    rt.return_to_field();
}

#[test]
fn flyscreamr_voice_sleeps_a_member_and_the_battle_exit_wakes_every_sleeper() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    // Pack formation 0xD7 is three FlyScreamr (enemy 76), whose regular list
    // holds `$34` VOICE twice and `$33` ACIDBREATH three times.
    let mut slept = false;
    'seeds: for seed in 0..64u32 {
        let mut rt = runtime(pack, &files);
        rt.set_rng_seed(0x0BAD_F00D ^ seed.wrapping_mul(0x9E37_79B1));
        rt.start_battle(0xD7, rt.battle_party()).unwrap();
        for _ in 0..4 {
            let timeline = rt
                .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
                .unwrap();
            assert!(
                !timeline
                    .events
                    .iter()
                    .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. })),
                "{:?}",
                timeline.events
            );
            let Some(index) = timeline.events.iter().position(|e| {
                matches!(e, BattleEvent::EnemySkillUsed { skill: 52, name, .. } if name == "VOICE")
            }) else {
                if rt.battle_roster().is_none() {
                    continue 'seeds;
                }
                continue;
            };
            let BattleEvent::EnemySkillUsed { actor, .. } = &timeline.events[index] else {
                unreachable!()
            };
            assert!(
                !timeline.events.iter().any(
                    |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if attacker == actor)
                ),
                "VOICE must not also swing: {:?}",
                timeline.events
            );
            if !timeline.events.iter().any(|e| {
                matches!(e, BattleEvent::StatusInflicted { status: inflicted, .. }
                         if *inflicted == status::ASLEEP)
            }) {
                continue;
            }
            // The cast landed. Whatever the round-end roll did, nobody carries
            // the sleep or a seal out of the battle.
            let party = rt
                .battle_roster()
                .unwrap()
                .side(Side::Party)
                .filter_map(|f| f.character)
                .collect::<Vec<_>>();
            leave(&mut rt);
            for character in party {
                let carried = rt.game().roster().get(CharId(character)).unwrap();
                assert_eq!(
                    carried.status & (status::ASLEEP | status::TECH_SEALED),
                    0,
                    "Battle_LastMessage clears bits 3 and 4"
                );
            }
            slept = true;
            break 'seeds;
        }
    }
    assert!(slept, "some seed's FlyScreamr must land a VOICE");
}

#[test]
fn two_zol_slugs_fuse_into_one_metaslug_in_the_real_formation() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut rt = runtime(pack, &files);
    rt.set_rng_seed(0x0BAD_F00D);
    // Pack formation 0xD2 is two ZolSlug (enemy 34); with exactly two on the
    // field every slug turn is Fusion.
    rt.start_battle(0xD2, rt.battle_party()).unwrap();
    let timeline = rt
        .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
        .unwrap();
    assert!(
        !timeline
            .events
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. })),
        "{:?}",
        timeline.events
    );
    let fused = timeline
        .events
        .iter()
        .find_map(|e| match e {
            BattleEvent::EnemiesFused {
                removed,
                fighter,
                enemy_id,
                hp,
                ..
            } => Some((removed.len(), fighter.get(), *enemy_id, *hp)),
            _ => None,
        })
        .expect("a slug's turn is Fusion");
    assert_eq!(
        fused,
        (2, 6, 36, 239),
        "both slots cleared, a MetaSlug seated"
    );
    let roster = rt.battle_roster().unwrap();
    let enemies: Vec<_> = roster
        .side(Side::Enemy)
        .map(|f| (f.id.get(), f.stats.enemy_id, f.stats.curr_hp))
        .collect();
    assert_eq!(enemies, vec![(6, 36, 239)], "the formation is one MetaSlug");
    leave(&mut rt);
}
