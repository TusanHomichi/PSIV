//! Displayed battle winnings reach the persistent purse exactly once.
use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn fixture(money: u32) -> Option<Runtime> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").exists() {
        return None;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial =
        Runtime::new_game(GameData::load(pack).unwrap(), StepFrames::default()).unwrap();
    initial.enable_battles(&files).unwrap();
    let mut snapshot = initial.game().snapshot();
    snapshot.money = money;
    snapshot.party = [1, 0, 2, 0xFF, 0xFF];
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot,
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x15,
                char_x: 768,
                char_y: 160,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    rt.set_rng_seed(0x0101_5678);
    rt.start_battle_timeline(0x8A, rt.battle_party()).unwrap();
    Some(rt)
}

fn win(rt: &mut Runtime) -> (u16, u16) {
    for _ in 0..100 {
        for event in rt.battle_round(&RoundOrders::attack_all()).unwrap() {
            if let BattleEvent::Rewarded {
                experience_each,
                meseta,
                ..
            } = event
            {
                assert!(meseta > 0);
                return (experience_each, meseta);
            }
        }
    }
    panic!("opening party failed to defeat two ZoranBults");
}

#[test]
fn victory_pays_the_displayed_meseta_once_and_the_save_keeps_it() {
    let Some(mut rt) = fixture(500) else { return };
    let (experience, meseta) = win(&mut rt);
    assert_eq!(
        rt.game().money(),
        500,
        "payout waits for results confirmation"
    );
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    let expected = 500 + u32::from(meseta);
    assert_eq!(rt.game().money(), expected);
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    assert_eq!(
        rt.game().money(),
        expected,
        "a duplicate close cannot pay twice"
    );
    let dir = std::env::temp_dir().join(format!("psiv-reward-save-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let restored = Runtime::load_slot(
        GameData::load(Path::new(PACK)).unwrap(),
        &dir,
        0,
        StepFrames::default(),
    )
    .unwrap();
    assert_eq!(restored.game().money(), expected);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn victory_caps_the_purse_at_the_retail_limit() {
    let Some(mut rt) = fixture(9_999_998) else {
        return;
    };
    let (experience, meseta) = win(&mut rt);
    assert!(meseta > 1);
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    assert_eq!(rt.game().money(), 9_999_999);
}

#[test]
fn battle_return_retains_live_object_positions_facing_and_camera() {
    let Some(mut rt) = fixture(500) else { return };
    let npc = rt.map().npcs()[0];
    rt.set_npc_pixel_position(
        0,
        i32::from(npc.cell.x) * 16 + 3,
        (i32::from(npc.cell.y) - 1) * 16 + 5,
    )
    .unwrap();
    rt.face_npc(0, psiv_core::Direction::Right);
    rt.set_camera(256, 112);
    let objects = rt.map().npcs().to_vec();
    let party = rt.members();
    let camera = *rt.camera();
    let (experience, _) = win(&mut rt);
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    assert_eq!(
        rt.return_to_field(),
        vec![psiv_runtime::RuntimeEvent::MapRefreshed]
    );
    assert_eq!(rt.map().npcs(), objects);
    assert_eq!(rt.members(), party);
    assert_eq!(*rt.camera(), camera);
    assert!(rt.return_to_field().is_empty());
}

#[test]
fn escaping_after_a_kill_does_not_pay_the_partial_pool() {
    let Some(mut rt) = fixture(500) else { return };
    let first = rt.battle_round(&RoundOrders::attack_all()).unwrap();
    assert!(first.iter().any(|e| matches!(e, BattleEvent::Died { .. })));
    assert!(!first.iter().any(|e| matches!(e, BattleEvent::Ended { .. })));
    for _ in 0..100 {
        let events = rt.battle_round(&RoundOrders::Run).unwrap();
        if events.contains(&BattleEvent::Ended {
            outcome: Outcome::Escaped,
        }) {
            rt.finish_battle_for_outcome(Outcome::Escaped, 0);
            assert_eq!(rt.game().money(), 500);
            return;
        }
    }
    panic!("opening party did not escape");
}
