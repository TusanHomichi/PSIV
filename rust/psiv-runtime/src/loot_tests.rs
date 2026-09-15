//! Real-pack field input and transaction boundaries, including story loot.
use super::*;
use psiv_core::{ChestOutcome, Inventory};
use std::path::Path;
const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn fixture(map: u16, x: u16, y: u16) -> Option<Runtime> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").exists() {
        return None;
    }
    let mut rt = Runtime::new(
        GameData::load(pack).unwrap(),
        map,
        Cell::new(x, y),
        Direction::Up,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&psiv_data::BattleFiles::load(pack).unwrap())
        .unwrap();
    rt.tick(Input::Neutral);
    assert!(!rt.scene_active());
    Some(rt)
}

#[test]
fn chest_item_normal_input_grants_once_parks_field_and_survives_reload() {
    let Some(mut rt) = fixture(0x47, 17, 18) else {
        return;
    };
    let npc_count = rt.map_record().unwrap().npcs.len();
    assert_eq!(rt.map.chest_slot_base(), npc_count);
    assert!(!rt.map.is_walkable(Cell::new(17, 17)));
    rt.tick(Input::Action);
    assert!(matches!(
        rt.loot_state().unwrap().outcome,
        ChestOutcome::Took { item: 125, .. }
    ));
    assert_eq!(rt.loot_state().unwrap().item_name, "MONOMATE");
    let snapshot = rt.game.snapshot();
    for _ in 0..60 {
        rt.tick(Input::Direction(Direction::Down));
    }
    assert_eq!(rt.state().cell(), Cell::new(17, 18));
    assert_eq!(rt.game.snapshot(), snapshot);
    assert!(rt.acknowledge_loot());
    rt.tick(Input::Neutral);
    rt.tick(Input::Action);
    assert_eq!(rt.loot_state().unwrap().outcome, ChestOutcome::AlreadyOpen);
    assert_eq!(rt.game.snapshot(), snapshot);
    rt.acknowledge_loot();
    let dir = std::env::temp_dir().join(format!("psiv-chest-save-{}", std::process::id()));
    rt.save_slot(&dir, 0).unwrap();
    let restored = Runtime::load_slot(rt.data.clone(), &dir, 0, StepFrames::default()).unwrap();
    assert!(restored.game.is_set(Flag::chest(32)));
    assert_eq!(
        restored.map.chest_is_open_at_slot(npc_count + 1),
        Some(true)
    );
    assert_eq!(restored.game.inventory(), rt.game.inventory());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn chest_meseta_needs_no_free_slot_and_the_live_lid_survives_field_refresh() {
    let Some(mut rt) = fixture(0x47, 40, 17) else {
        return;
    };
    *rt.game.inventory_mut() = Inventory::from_slots([125; 40]);
    let before = rt.game.money();
    rt.tick(Input::Action);
    assert_eq!(
        rt.loot_state().unwrap().outcome,
        ChestOutcome::Meseta { amount: 100 }
    );
    assert_eq!(rt.game.money(), before + 100);
    rt.acknowledge_loot();
    rt.refresh_field_after_battle().unwrap();
    assert_eq!(rt.map.chests().len(), 2);
    assert_eq!(
        rt.map.chest_is_open_at_slot(rt.map.chest_slot_base()),
        Some(true)
    );
    assert_eq!(rt.game.inventory().occupied(), 40);
}

#[test]
fn full_chest_return_closes_without_grant_and_protects_necessary_items() {
    let Some(mut rt) = fixture(0x4A, 40, 33) else {
        return;
    };
    *rt.game.inventory_mut() = Inventory::from_slots([141; 40]);
    let before = rt.game.snapshot();
    rt.tick(Input::Action);
    assert_eq!(
        rt.loot_state().unwrap().outcome,
        ChestOutcome::Full { item: 141 }
    );
    let slot = rt.loot_state().unwrap().object_slot;
    assert_eq!(rt.map.chest_is_open_at_slot(slot), Some(true));
    assert!(!rt.acknowledge_loot());
    assert_eq!(rt.discard_for_loot(0), LootResult::Necessary);
    assert_eq!(rt.game.snapshot(), before);
    assert!(rt.return_loot());
    assert_eq!(rt.map.chest_is_open_at_slot(slot), Some(false));
    assert_eq!(rt.game.snapshot(), before);
    assert!(!rt.return_loot());
}

#[test]
fn full_chest_discards_first_duplicate_appends_alshline_then_releases_its_scene() {
    let Some(mut rt) = fixture(0x4A, 40, 33) else {
        return;
    };
    let mut items = [125; 40];
    items[0] = 57;
    items[2] = 126;
    *rt.game.inventory_mut() = Inventory::from_slots(items);
    rt.tick(Input::Action);
    assert!(rt.game.is_clear(Flag::chest(8)));
    assert_eq!(
        rt.discard_for_loot(0),
        LootResult::Necessary,
        "zero-price Psyco Wand"
    );
    assert_eq!(rt.discard_for_loot(99), LootResult::Invalid);
    assert_eq!(
        rt.discard_for_loot(7),
        LootResult::Took {
            item: 141,
            discarded: 125
        }
    );
    assert_eq!(&rt.game.inventory().slots()[..3], &[57, 126, 125]);
    assert_eq!(rt.game.inventory().get(39), Some(141));
    assert!(rt.game.is_set(Flag::chest(8)));
    assert_eq!(rt.discard_for_loot(7), LootResult::Invalid);
    for _ in 0..20 {
        rt.tick(Input::Neutral);
    }
    assert!(
        !rt.scene_active(),
        "message must be acknowledged before Alshline scene"
    );
    rt.acknowledge_loot();
    rt.tick(Input::Neutral);
    assert!(rt.scene_active(), "real chest flag starts FindingAlshline");
}
