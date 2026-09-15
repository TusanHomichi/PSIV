//! Real table coverage for entry state, costs, destinations and save words.
use super::*;
use psiv_core::battle::status;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn fixture() -> Option<Runtime> {
    let pack = Path::new(PACK);
    if !pack.join("travel.json").is_file() {
        return None;
    }
    let mut rt = Runtime::new(
        GameData::load(pack).unwrap(),
        0,
        Cell::new(46, 144),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&psiv_data::BattleFiles::load(pack).unwrap())
        .unwrap();
    rt.game.set_party([Some(CharId(0)), None, None, None, None]);
    rt.resize_party();
    rt.game.set(Flag::event(0x0C)).unwrap();
    rt.game.set(Flag::town(0)).unwrap();
    let stats = rt.game.roster_mut().get_mut(CharId(0)).unwrap();
    stats.techniques = [RYUKA, HINAS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    stats.curr_tp = 50;
    stats.status = 0;
    Some(rt)
}

#[test]
fn travel_ordinary_town_entry_registers_visit_and_updates_both_saved_map_words() {
    let Some(mut rt) = fixture() else { return };
    assert!(rt.game.is_clear(Flag::town(1)));
    // The native warp path calls change_map, without directly touching flags.
    rt.change_map(MapId(0x1D), Cell::new(27, 41), Direction::Up)
        .unwrap();
    assert!(rt.game.is_set(Flag::town(1)));
    assert_eq!(rt.previous_map_id(), 0);
    rt.change_map(MapId(0), Cell::new(61, 102), Direction::Down)
        .unwrap();
    assert_eq!(rt.previous_map_id(), 0x1D);
    let directory = std::env::temp_dir().join(format!("psiv-travel-map-{}", std::process::id()));
    let path = rt.save_slot(&directory, 0).unwrap();
    let save = psiv_core::RetailSlot::from_bytes(&std::fs::read(path).unwrap(), 0)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(save.location.map_index, 0);
    assert_eq!(save.location.map_index_2, 0x1D);
    let loaded = Runtime::load_slot(rt.data.clone(), &directory, 0, StepFrames::default()).unwrap();
    assert!(loaded.game.is_set(Flag::town(1)));
    assert_eq!(loaded.previous_map_id(), 0x1D);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn travel_ryuka_browse_is_free_and_confirmation_charges_once_at_the_selected_town() {
    let Some(mut rt) = fixture() else { return };
    rt.game.set(Flag::town(1)).unwrap();
    let before = rt.game.snapshot();
    let rng = rt.rng.clone();
    assert!(matches!(
        rt.begin_camp_travel(0, RYUKA),
        Ok(CampTravelMenu::Towns(_))
    ));
    assert!(
        matches!(rt.begin_camp_travel(0, RYUKA), Ok(CampTravelMenu::Towns(_))),
        "cancel and reopen"
    );
    assert!(rt.select_camp_town(0, 5).is_err(), "unvisited town");
    assert_eq!(rt.game.snapshot(), before);
    assert_eq!(rt.rng, rng);
    assert_eq!(
        rt.select_camp_town(0, 1),
        Ok(CampTravelMenu::Ready {
            name: "RYUKA".into()
        })
    );
    assert_eq!(rt.game.roster().get(CharId(0)).unwrap().curr_tp, 42);
    assert!(
        rt.select_camp_town(0, 1).is_err(),
        "pending message cannot charge twice"
    );
    assert_eq!(
        rt.state().cell(),
        Cell::new(46, 144),
        "message precedes transition"
    );
    assert!(matches!(
        &rt.complete_camp_travel().unwrap()[..],
        [RuntimeEvent::MapChanged { map: MapId(0), .. }]
    ));
    assert_eq!(rt.state().cell(), Cell::new(61, 102));
    assert_eq!(rt.previous_map_id(), 0x1D);
    assert_eq!(rt.rng, rng);
    assert!(rt.complete_camp_travel().unwrap().is_empty());
    assert_eq!(rt.game.roster().get(CharId(0)).unwrap().curr_tp, 42);
}

#[test]
fn travel_hinas_keeps_each_maze_entrance_across_interior_maps() {
    let Some(mut rt) = fixture() else { return };
    for (map, previous, index, destination) in [
        (0x9B, 0xD8, 3, Cell::new(128, 122)),
        (0xA1, 0xD9, 4, Cell::new(148, 94)),
    ] {
        rt.change_map_from(MapId(map), Cell::new(31, 32), Direction::Down, previous)
            .unwrap();
        assert_eq!(rt.dungeon_exit_index(), index);
        rt.change_map(MapId(0x9D), Cell::new(16, 12), Direction::Down)
            .unwrap();
        assert_eq!(rt.dungeon_exit_index(), index);
        let before = rt.game.snapshot();
        let rng = rt.rng.clone();
        assert!(rt.begin_camp_travel(0, RYUKA).is_err());
        assert_eq!(rt.game.snapshot(), before);
        assert_eq!(
            rt.begin_camp_travel(0, HINAS),
            Ok(CampTravelMenu::Ready {
                name: "HINAS".into()
            })
        );
        rt.complete_camp_travel().unwrap();
        assert_eq!(rt.map_id().0, 0);
        assert_eq!(rt.state().cell(), destination);
        assert_eq!(rt.previous_map_id(), 0x9D);
        assert_eq!(rt.dungeon_exit_index(), 0);
        assert_eq!(rt.rng, rng);
    }
    assert_eq!(rt.game.roster().get(CharId(0)).unwrap().curr_tp, 42);
}

#[test]
fn travel_rejects_blocked_status_tp_map_and_world_without_payment() {
    let Some(mut rt) = fixture() else { return };
    for (status, tp, error) in [
        (4, 50, "CASTER IS DOWN"),
        (2, 50, "CASTER IS PARALYZED"),
        (0, 7, "NOT ENOUGH TP"),
    ] {
        let stats = rt.game.roster_mut().get_mut(CharId(0)).unwrap();
        stats.status = status;
        stats.curr_tp = tp;
        let before = rt.game.snapshot();
        assert_eq!(rt.begin_camp_travel(0, RYUKA), Err(error.into()));
        assert_eq!(rt.game.snapshot(), before);
    }
    let stats = rt.game.roster_mut().get_mut(CharId(0)).unwrap();
    stats.status = 0x10;
    stats.curr_tp = 50;
    assert!(
        rt.begin_camp_travel(0, RYUKA).is_ok(),
        "field TECH ignores seal"
    );
    assert!(rt.begin_camp_travel(0, HINAS).is_err(), "no dungeon exit");
    rt.saved_world_index = 0x0300;
    assert!(
        rt.begin_camp_travel(0, RYUKA).is_err(),
        "Zelan blocks RYUKA"
    );
    rt.saved_world_index = 0x0100;
    rt.game.set(Flag::town(16)).unwrap();
    let towns = rt.town_destinations();
    assert_eq!(towns.len(), 1);
    assert_eq!(
        towns[0].world, 1,
        "World_Index is the high byte of the saved word"
    );
    rt.select_camp_town(0, 0).unwrap();
    rt.complete_camp_travel().unwrap();
    assert_eq!(rt.map_id().0, 1);
    assert_eq!(rt.state().cell(), Cell::new(36, 96));
    assert_eq!(rt.previous_map_id(), 0x14C);
}

#[test]
fn travel_passageway_selects_opposite_exit_by_entry_x() {
    let Some(mut rt) = fixture() else { return };
    rt.change_map_from(
        MapId(0x81),
        Cell::new(0x520 / 16, 20),
        Direction::Down,
        0xDA,
    )
    .unwrap();
    assert_eq!(rt.dungeon_exit_index(), 8);
    rt.change_map_from(MapId(0x81), Cell::new(20, 20), Direction::Down, 0xDA)
        .unwrap();
    assert_eq!(rt.dungeon_exit_index(), 7);
}

#[test]
fn travel_native_restart_preserves_inherited_exits_and_legacy_slots_still_load() {
    let Some(mut rt) = fixture() else { return };
    let directory =
        std::env::temp_dir().join(format!("psiv-travel-inherit-{}", std::process::id()));
    for (map, previous, index) in [(0x9B, 0xD8, 3), (0xA1, 0xD9, 4)] {
        rt.change_map_from(MapId(map), Cell::new(31, 32), Direction::Down, previous)
            .unwrap();
        rt.change_map(MapId(0x9D), Cell::new(16, 12), Direction::Down)
            .unwrap();
        let path = rt.save_slot(&directory, 0).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let encoded = psiv_core::RetailSlot::from_bytes(&bytes, 0).unwrap();
        assert_eq!(encoded.dungeon_exit(), Some(index));
        let mut loaded =
            Runtime::load_slot(rt.data.clone(), &directory, 0, StepFrames::default()).unwrap();
        assert_eq!(loaded.dungeon_exit_index(), index);
        loaded
            .enable_battles(&psiv_data::BattleFiles::load(Path::new(PACK)).unwrap())
            .unwrap();
        loaded.begin_camp_travel(0, HINAS).unwrap();
        loaded.complete_camp_travel().unwrap();
        assert_eq!(
            loaded.state().cell(),
            if index == 3 {
                Cell::new(128, 122)
            } else {
                Cell::new(148, 94)
            }
        );
        // A legacy slot has no extra header metadata and no remembered exit
        // in this interior map. Loading it remains supported.
        let legacy = psiv_core::RetailSlot::encode(&encoded.decode().unwrap(), 0).unwrap();
        assert_eq!(legacy.dungeon_exit(), None);
        assert_eq!(&legacy.as_bytes()[0x200..], &bytes[0x200..]);
        std::fs::write(&path, legacy.as_bytes()).unwrap();
        let loaded =
            Runtime::load_slot(rt.data.clone(), &directory, 0, StepFrames::default()).unwrap();
        assert_eq!(loaded.dungeon_exit_index(), 0);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn travel_telepipe_cancel_and_rejections_keep_inventory_then_consume_selected_duplicate_once() {
    let Some(mut rt) = fixture() else { return };
    rt.game.set(Flag::town(1)).unwrap();
    for item in [TELEPIPE, 125, TELEPIPE, 126] {
        rt.game.inventory_mut().add(item).unwrap();
    }
    let stats = rt.game.roster_mut().get_mut(CharId(0)).unwrap();
    stats.curr_tp = 0;
    stats.status = status::PARALYZED | status::TECH_SEALED;
    stats.techniques = [0; 16];
    let before = rt.game.snapshot();
    let rng = rt.rng.clone();
    for _ in 0..2 {
        assert!(matches!(
            rt.begin_camp_item_travel(2),
            Ok(CampTravelMenu::Towns(_))
        ));
    }
    assert!(rt.begin_camp_item_travel(1).is_err());
    assert!(rt.begin_camp_item_travel(40).is_err());
    assert!(rt.select_camp_item_town(2, 5).is_err());
    assert_eq!(
        rt.game.snapshot(),
        before,
        "cancel and invalid targets are free"
    );
    assert_eq!(
        rt.select_camp_item_town(2, 1),
        Ok(CampTravelMenu::Ready {
            name: "TELEPIPE".into()
        })
    );
    assert_eq!(&rt.game.inventory().slots()[..4], &[TELEPIPE, 125, 126, 0]);
    assert_eq!(
        rt.game.roster(),
        &psiv_core::GameState::from_snapshot(&before)
            .roster()
            .clone()
    );
    assert!(
        rt.select_camp_item_town(0, 1).is_err(),
        "pending message cannot consume another pipe"
    );
    rt.complete_camp_travel().unwrap();
    assert_eq!(rt.state().cell(), Cell::new(61, 102));
    assert_eq!(rt.previous_map_id(), 0x1D);
    assert!(rt.complete_camp_travel().unwrap().is_empty());
    assert_eq!(&rt.game.inventory().slots()[..4], &[TELEPIPE, 125, 126, 0]);
    assert_eq!(rt.rng, rng);
}

#[test]
fn travel_escapipe_requires_an_exit_and_uses_the_inherited_entrance_without_tp() {
    let Some(mut rt) = fixture() else { return };
    rt.game.inventory_mut().add(ESCAPIPE).unwrap();
    rt.game.inventory_mut().add(TELEPIPE).unwrap();
    rt.game.roster_mut().get_mut(CharId(0)).unwrap().curr_tp = 0;
    let before = rt.game.snapshot();
    assert_eq!(
        rt.begin_camp_item_travel(0),
        Err("CANNOT TELEPORT HERE".into())
    );
    assert_eq!(rt.game.snapshot(), before);
    rt.change_map_from(MapId(0xA1), Cell::new(31, 32), Direction::Down, 0xD9)
        .unwrap();
    rt.change_map(MapId(0x9D), Cell::new(16, 12), Direction::Down)
        .unwrap();
    assert_eq!(rt.dungeon_exit_index(), 4);
    assert!(
        rt.begin_camp_item_travel(1).is_err(),
        "town travel inside a dungeon"
    );
    let rng = rt.rng.clone();
    assert_eq!(
        rt.begin_camp_item_travel(0),
        Ok(CampTravelMenu::Ready {
            name: "ESCAPIPE".into()
        })
    );
    assert_eq!(&rt.game.inventory().slots()[..2], &[TELEPIPE, 0]);
    assert_eq!(rt.map_id().0, 0x9D, "payment precedes acknowledgement");
    rt.complete_camp_travel().unwrap();
    assert_eq!(rt.map_id().0, 0);
    assert_eq!(rt.state().cell(), Cell::new(148, 94));
    assert_eq!(rt.previous_map_id(), 0x9D);
    assert_eq!(rt.game.roster().get(CharId(0)).unwrap().curr_tp, 0);
    assert_eq!(rt.rng, rng);
}

#[test]
fn travel_telepipe_rechecks_the_selected_slot_after_browsing() {
    let Some(mut rt) = fixture() else { return };
    rt.game.inventory_mut().add(TELEPIPE).unwrap();
    rt.game.inventory_mut().add(ESCAPIPE).unwrap();
    rt.begin_camp_item_travel(0).unwrap();
    rt.game.inventory_mut().remove_in_field(0).unwrap();
    let before = rt.game.snapshot();
    assert_eq!(rt.select_camp_item_town(0, 0), Err("NOT A TELEPIPE".into()));
    assert_eq!(rt.game.snapshot(), before);
    assert!(rt.complete_camp_travel().unwrap().is_empty());
}
