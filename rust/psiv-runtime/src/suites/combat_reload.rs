//! Real-pack reloads: the inline formations Fusion and COMBINE copy, and the
//! corrected COMBINE arm in an Air Castle formation
//! (`docs/battle/ENEMY_FUSION.md`, `docs/battle/ENEMY_ABILITIES_AIR_CASTLE.md`).
use super::combat_enemy_effects::runtime;
use crate::encounters::battle_data;
use psiv_core::battle::{BattleEvent, Command, RoundOrders};
use psiv_data::BattleFiles;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn the_pack_carries_both_inline_formations_into_the_battle_data() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    assert!(
        !files.formations.inline_formations.is_empty(),
        "battle/formations.json has no inline_formations: the pack predates them; \
         rebuild it with `python3 -m psiv_tools pack`"
    );
    let data = battle_data(&files).unwrap();
    for (label, enemy) in [("loc_1A2F4", 36), ("loc_23D00", 87)] {
        let record = data.inline_formation(label).unwrap();
        let seated: Vec<_> = record
            .enemies
            .iter()
            .map(|e| (e.slot, e.enemy_id, e.position))
            .collect();
        assert_eq!(seated, vec![(1, enemy, 0x14)], "{label}");
        assert_eq!(record.run_chance, 0, "{label}");
    }
}

#[test]
fn a_blade_right_beside_two_frost_sabers_never_combines() {
    // Formation $175 (group 52, the Air Castle): BladeRight with two
    // FrostSabers and no HakenLeft. `EnemyAI_HakenLeftExists` wants exactly
    // one HakenLeft, so the BladeRight breathes fire or swings - the arm the
    // port used to fire here is gone.
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    for seed in [0x0BAD_F00D_u32, 0x1234_5678, 0x0F0F_0F0F] {
        let mut rt = runtime(pack, &files);
        rt.set_rng_seed(seed);
        rt.start_battle(0x175, rt.battle_party()).unwrap();
        for _ in 0..4 {
            let timeline = rt
                .battle_round_timeline(&RoundOrders::Commands(vec![Command::Defend; 3]))
                .unwrap();
            for event in &timeline.events {
                assert!(
                    !matches!(
                        event,
                        BattleEvent::UnsupportedAbility { .. } | BattleEvent::EnemiesFused { .. }
                    ),
                    "seed {seed:#x}: {event:?}"
                );
            }
            if rt.battle_roster().is_none() {
                break;
            }
        }
    }
}
