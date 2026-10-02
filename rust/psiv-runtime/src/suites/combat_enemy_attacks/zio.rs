//! The Zio-arc abilities: `$21` FIREBREATH, `$47` ZAN and `$4D` CORRSION in
//! their real formations.
//!
//! A child module rather than more of `combat_enemy_attacks.rs`: that file sits
//! at the repository's 1,000-line limit, so the newest suite of the same kind
//! lives beside it under the same test path prefix
//! (`suites::combat_enemy_attacks::zio::`).

use super::PACK;
use crate::Runtime;
use psiv_core::battle::{BattleEvent, Command, FighterId, RoundOrders};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use std::path::Path;

/// The Zio-arc routes are real-pack reachable, on the maps the campaign runner
/// walks (`docs/campaign/RUNNER_LOG.md` H15/H16). `generated/enemies.json` gives
/// 83 Ripper `$21` FIREBREATH in regular slots 6-8, and formation `$DB` is two of
/// them (the Zio Fort's, `$082`); 100 TechMaster `$47` ZAN in slot 8, and
/// formation `$110` is two of them (Nurvus `$0CC`); 106 Haunt `$4D` CORRSION in
/// slot 8, and formation `$FB` is two of them (Ladea Tower `$8E`). Each must
/// resolve through `enemy_damage::resolve_damage_skill`: one `EnemySkillUsed`
/// with the record's own display name, then one `Resolved` with damage per
/// living party slot in slot order (ZAN's and CORRSION's all-party class) or one
/// against the drawn target (FIREBREATH's single-target class), and no physical
/// swing by that caster.
///
/// The runtime's *sound* cue for these four abilities is the presentation
/// layer's (`psiv-runtime/src/battle_interim.rs`), so this test asserts nothing
/// about `timeline.sounds`: a cue added later is not a regression here, and the
/// missing one is a gap the feature map's row owns.
#[test]
fn zio_arc_abilities_resolve_in_their_real_formations() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    for (formation, carrier, ability, display, party_wide, seed) in [
        (0xDBu16, 83u16, 33u8, "FIREBREATH", false, 0x9E37_79B9u32),
        (0x110u16, 100u16, 71u8, "ZAN", true, 0x0101_5678u32),
        (0xFBu16, 106u16, 77u8, "CORRSION", true, 0x2718_2818u32),
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
        // battle before the carrier's roll shows up. The three records resist
        // with `$07` (magic defense) and TechMaster's plain attack with `$06`
        // (defense), so both words are pinned high; `battle_party` hands these
        // stats to the battle unchanged and a resistance this large clamps every
        // record to `loc_266C`'s minimum damage. The numbers themselves are
        // pinned by the core tests.
        for character in [1, 0, 2] {
            let stats = game.roster_mut().get_mut(CharId(character)).unwrap();
            stats.max_hp = 999;
            stats.curr_hp = 999;
            stats.defence.battle = 5000;
            stats.mental_defence.battle = 5000;
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
            // Every occurrence is checked, not just the first: a later round may
            // roll the ability again.
            for (index, event) in timeline.events.iter().enumerate() {
                let BattleEvent::EnemySkillUsed { actor, skill, name } = event else {
                    continue;
                };
                if *skill != ability {
                    continue;
                }
                let actor = *actor;
                assert_eq!(name, display, "carrier {carrier}");
                found = true;
                let mut hits = Vec::new();
                for event in &timeline.events[index + 1..] {
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
                            hits.push(*target);
                        }
                        BattleEvent::Died { .. } => {}
                        _ => break,
                    }
                }
                if party_wide {
                    assert_eq!(
                        hits,
                        vec![
                            FighterId::new(1).unwrap(),
                            FighterId::new(2).unwrap(),
                            FighterId::new(3).unwrap()
                        ],
                        "carrier {carrier} ability {skill}: the whole party, in slot order: {:?}",
                        timeline.events
                    );
                } else {
                    assert_eq!(
                        hits.len(),
                        1,
                        "carrier {carrier} ability {skill}: the single-target class resolves once: {:?}",
                        timeline.events
                    );
                }
                assert!(
                    !timeline.events.iter().any(
                        |e| matches!(e, BattleEvent::Attacked { actor: attacker, .. } if *attacker == actor)
                    ),
                    "carrier {carrier} ability {skill}: the ability replaces the swing: {:?}",
                    timeline.events
                );
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
