//! Real item records, both hands, and save/reload persistence.
use psiv_core::battle::EquipSlot;
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{CampEquipResult, Runtime};
use std::path::Path;

#[test]
fn alys_can_equip_two_slashers_and_keep_them_after_reload() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    let mut game = GameState::new();
    game.set_party([Some(CharId(1)), None, None, None, None]);
    game.inventory_mut().add(9).unwrap();
    game.inventory_mut().add(9).unwrap();
    let location = RetailLocation {
        world_index: 0,
        map_index_2: 0,
        map_index: 0x26,
        char_x: 31 * 16,
        char_y: 39 * 16,
    };
    let mut runtime = Runtime::from_save(
        data.clone(),
        RetailSave {
            snapshot: game.snapshot(),
            location,
        },
        StepFrames::default(),
    )
    .unwrap();
    runtime.enable_battles(&files).unwrap();
    for hand in [EquipSlot::RightHand, EquipSlot::LeftHand] {
        let slot = runtime
            .game()
            .inventory()
            .slots()
            .iter()
            .position(|id| *id == 9)
            .unwrap();
        assert!(runtime.camp_equipment_hand_choice(slot));
        assert!(matches!(
            runtime.equip_camp_item_in_hand(0, slot, hand),
            CampEquipResult::Equipped { .. }
        ));
    }
    let stats = runtime.game().roster().get(CharId(1)).unwrap();
    assert_eq!(&stats.equipment[..2], &[9, 9]);
    assert_eq!(
        runtime
            .game()
            .inventory()
            .slots()
            .iter()
            .filter(|id| **id == 9)
            .count(),
        0
    );
    let attack = stats.attack.derived;
    let save = RetailSave {
        snapshot: runtime.game().snapshot(),
        location,
    };
    let mut restored = Runtime::from_save(data, save, StepFrames::default()).unwrap();
    restored.enable_battles(&files).unwrap();
    let stats = restored.game().roster().get(CharId(1)).unwrap();
    assert_eq!(&stats.equipment[..2], &[9, 9]);
    assert_eq!(stats.attack.derived, attack);
}
