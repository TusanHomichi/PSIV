//! Field inventory recovery uses field RNG and the cartridge's target masks.
use psiv_core::battle::Lcg41;
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames, field_healing};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{CampUseResult, Runtime};
use std::path::Path;

#[test]
fn field_recovery_rolls_through_the_first_empty_slot_and_obeys_target_rules() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").exists() {
        return;
    }
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new_game(data.clone(), StepFrames::default()).unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(1)),
        Some(CharId(6)),
        None,
        None,
    ]);
    for (id, hp, max_hp, status) in [(0, 1, 400, 0x1B), (1, 0, 53, 5), (6, 0, 999, 0x51)] {
        let stats = game.roster_mut().get_mut(CharId(id)).unwrap();
        stats.curr_hp = hp;
        stats.max_hp = max_hp;
        stats.status = status;
    }
    assert!(game.roster().get(CharId(6)).unwrap().is_android());
    for item in [131, 125, 128, 129, 134, 130, 130, 125, 144, 144] {
        game.inventory_mut().add(item).unwrap();
    }
    let mut rt = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x13,
                char_x: 768,
                char_y: 304,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    rt.set_rng_seed(0x0101_5678);
    let mut reference = Lcg41::new(0x0101_5678);
    let star = field_healing(64, 64, &mut reference);
    for _ in 1..4 {
        let _ = field_healing(64, 64, &mut reference);
    }
    let mono = field_healing(24, 24, &mut reference);
    assert!(matches!(rt.use_camp_item(0, 0), CampUseResult::Used { amount, .. } if amount == star));
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().status, 0x0B);
    assert_eq!(rt.game().roster().get(CharId(1)).unwrap().status, 5);
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().status, 0x51);
    assert!(matches!(rt.use_camp_item(0, 0), CampUseResult::Used { amount, .. } if amount == mono));
    assert_eq!(
        rt.game().roster().get(CharId(0)).unwrap().curr_hp,
        1 + star + mono
    );
    assert!(matches!(
        rt.use_camp_item(0, 0),
        CampUseResult::Used { amount: 0, .. }
    ));
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().status, 0x0A);
    rt.use_camp_item(0, 0);
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().status, 8);
    rt.use_camp_item(0, 0);
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().status, 0);
    assert_eq!(rt.game().roster().get(CharId(0)).unwrap().curr_hp, 400);
    rt.use_camp_item(0, 1);
    assert_eq!(rt.game().roster().get(CharId(1)).unwrap().curr_hp, 13);
    assert_eq!(rt.game().roster().get(CharId(1)).unwrap().status, 0);
    assert!(matches!(
        rt.use_camp_item(0, 0),
        CampUseResult::NoEffect { .. }
    ));
    assert!(matches!(
        rt.use_camp_item(0, 2),
        CampUseResult::NoEffect { .. }
    ));
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().curr_hp, 0);
    rt.use_camp_item(0, 2);
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().curr_hp, 999);
    assert_eq!(rt.game().roster().get(CharId(6)).unwrap().status, 0);
    assert!(matches!(
        rt.use_camp_item(0, 0),
        CampUseResult::NoEffect { .. }
    ));
    assert!(rt.game().inventory().slots().iter().all(|id| *id == 0));
}
