use super::*;

fn open_grid() -> CollisionGrid {
    CollisionGrid::filled(4, 4, 0).unwrap()
}

#[test]
fn rejects_a_warp_rect_that_leaves_the_grid() {
    let warp = Warp::ground(
        CellRect::new(3, 0, 2, 1),
        MapId(1),
        Cell::new(0, 0),
        Direction::Down,
    );
    assert!(matches!(
        FieldMap::new(MapId(0), open_grid(), vec![warp], vec![]),
        Err(MapError::WarpRectOutOfBounds { warp_index: 0, .. })
    ));
}

#[test]
fn rejects_an_empty_warp_rect() {
    let warp = Warp::ground(
        CellRect::new(1, 1, 0, 1),
        MapId(1),
        Cell::new(0, 0),
        Direction::Down,
    );
    assert!(matches!(
        FieldMap::new(MapId(0), open_grid(), vec![warp], vec![]),
        Err(MapError::EmptyWarpRect { warp_index: 0 })
    ));
}

#[test]
fn rejects_npcs_placed_outside_the_grid() {
    let out = Npc::new(NpcId(1), Cell::new(9, 9), Direction::Down);
    assert!(matches!(
        FieldMap::new(MapId(0), open_grid(), vec![], vec![out]),
        Err(MapError::NpcOutOfBounds { .. })
    ));
}

#[test]
fn accepts_the_retail_placements_that_look_wrong_and_are_not() {
    // A shopkeeper behind a $C counter, a scenery object in a wall, and two
    // clerks sharing a type id: all three occur in the cartridge, so all
    // three have to load.
    let mut grid = open_grid();
    grid.set(Cell::new(2, 2), 0xC).unwrap();
    grid.set(Cell::new(3, 3), 0x8).unwrap();

    let clerk = Npc::new(NpcId(0x108), Cell::new(2, 2), Direction::Down);
    let twin = Npc::new(NpcId(0x108), Cell::new(3, 3), Direction::Down);
    let map = FieldMap::new(MapId(0), grid, vec![], vec![clerk, twin]).unwrap();

    assert_eq!(map.npcs_on_blocking_cells(), vec![0, 1]);
    assert!(!map.is_walkable(Cell::new(2, 2)));
}

#[test]
fn npcs_may_share_a_cell() {
    let a = Npc::new(NpcId(1), Cell::new(1, 1), Direction::Down);
    let b = Npc::new(NpcId(2), Cell::new(1, 1), Direction::Up);
    let map = FieldMap::new(MapId(0), open_grid(), vec![], vec![a, b]).unwrap();
    assert_eq!(
        map.npc_at(Cell::new(1, 1)).map(|npc| npc.id),
        Some(NpcId(1))
    );
    assert!(!map.is_walkable(Cell::new(1, 1)));
}

#[test]
fn warp_lookup_is_per_table() {
    let mut grid = open_grid();
    grid.set(Cell::new(1, 0), 0x1).unwrap();
    let doorway = Warp::door(Cell::new(1, 0), MapId(9), Cell::new(3, 3), Direction::Up);
    let edge = Warp::ground(
        CellRect::new(0, 3, 4, 1),
        MapId(2),
        Cell::new(0, 0),
        Direction::Down,
    );
    let map = FieldMap::new(MapId(0), grid, vec![doorway, edge], vec![]).unwrap();

    assert!(
        map.warp_at(Cell::new(1, 0), WarpTrigger::MapChange)
            .is_some()
    );
    assert!(
        map.warp_at(Cell::new(1, 0), WarpTrigger::NormalGround)
            .is_none()
    );
    assert!(
        map.warp_at(Cell::new(2, 3), WarpTrigger::NormalGround)
            .is_some()
    );
    assert!(
        map.warp_at(Cell::new(2, 3), WarpTrigger::MapChange)
            .is_none()
    );
}

#[test]
fn a_map_change_warp_over_no_type_one_cell_is_reported_not_rejected() {
    let mut grid = open_grid();
    grid.set(Cell::new(3, 3), 0x1).unwrap();
    let reachable = Warp::door(Cell::new(3, 3), MapId(9), Cell::new(0, 0), Direction::Up);
    // Sits entirely on ordinary ground, so `MapTransTile_MapChange` can
    // never reach it.
    let stranded = Warp::door(Cell::new(0, 0), MapId(9), Cell::new(0, 0), Direction::Up);

    let map = FieldMap::new(MapId(0), grid, vec![reachable, stranded], vec![]).unwrap();

    assert_eq!(map.unreachable_map_change_warps(), vec![1]);
}
