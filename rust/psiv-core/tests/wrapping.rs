//! Specification of toroidal maps — the two overworlds.
//!
//! Motavia and Dezolis are globes: position wraps at `(size + 1) << 5` pixels
//! on both axes, which for their 256-cell grids is the 4,096-pixel period.
//! Every other map keeps hard edges, so each rule below is pinned against a
//! bounded map of the same shape.

mod common;

use common::{map_with, party, party_at_rate, party_facing, run, torus, torus_with, walk_one_step};
use psiv_core::{
    Cell, CellRect, CollisionGrid, Direction, Effect, FieldMap, Input, MapError, MapId, Npc, NpcId,
    StepFrames, SubCellOffset, Topology, Warp, WarpTrigger,
};

fn open_torus() -> FieldMap {
    torus(&["....", "....", "....", "...."])
}

// ---------------------------------------------------------------------------
// Stepping across the seams
// ---------------------------------------------------------------------------

#[test]
fn the_walker_crosses_all_four_seams() {
    let map = open_torus();
    let crossings = [
        // start cell, direction, cell arrived at
        (Cell::new(3, 1), Direction::Right, Cell::new(0, 1)),
        (Cell::new(0, 1), Direction::Left, Cell::new(3, 1)),
        (Cell::new(1, 3), Direction::Down, Cell::new(1, 0)),
        (Cell::new(1, 0), Direction::Up, Cell::new(1, 3)),
    ];

    for (start, dir, arrive) in crossings {
        let mut state = party(&map, start.x, start.y);
        let effects = walk_one_step(&mut state, &map, dir);

        assert_eq!(
            effects,
            vec![Effect::StepCompleted { cell: arrive }],
            "walking {dir:?} off ({}, {}) should arrive at ({}, {})",
            start.x,
            start.y,
            arrive.x,
            arrive.y
        );
        assert_eq!(state.cell(), arrive);
    }
}

#[test]
fn the_same_edges_still_block_on_a_bounded_map() {
    // The regression half: identical geometry, bounded, nothing moves.
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);

    for (start, dir) in [
        (Cell::new(3, 1), Direction::Right),
        (Cell::new(0, 1), Direction::Left),
        (Cell::new(1, 3), Direction::Down),
        (Cell::new(1, 0), Direction::Up),
    ] {
        let mut state = party(&map, start.x, start.y);
        let effects = walk_one_step(&mut state, &map, dir);
        assert!(effects.is_empty(), "{dir:?} escaped a bounded map");
        assert_eq!(state.cell(), start);
        assert_eq!(state.facing(), dir, "the walker still turns");
    }
}

#[test]
fn walking_a_full_lap_returns_to_the_start() {
    let map = open_torus();
    let mut state = party(&map, 0, 0);

    let effects = run(&mut state, &map, Input::Direction(Direction::Right), 8 * 4);

    assert_eq!(effects.len(), 4, "four cells is one lap of a 4-wide world");
    assert_eq!(state.cell(), Cell::new(0, 0));
    assert!(!state.is_stepping());
}

#[test]
fn a_step_across_the_seam_is_committed_like_any_other() {
    let map = open_torus();
    let mut state = party_at_rate(&map, 3, 1, 8);

    let first = state.tick(&map, Input::Direction(Direction::Right));
    assert!(first.is_empty());
    assert_eq!(state.step_destination(), Some(Cell::new(0, 1)));

    // Release the input mid-crossing; the cell still completes.
    let rest = run(&mut state, &map, Input::Neutral, 7);

    assert_eq!(
        rest,
        vec![Effect::StepCompleted {
            cell: Cell::new(0, 1)
        }]
    );
    assert_eq!(state.cell(), Cell::new(0, 1));
}

#[test]
fn blocking_terrain_across_the_seam_stops_the_walker() {
    // Column 0 is solid, so stepping right off column 3 hits it.
    let map = torus(&["#...", "#...", "#...", "#..."]);
    let mut state = party(&map, 3, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Right);

    assert!(effects.is_empty());
    assert_eq!(state.cell(), Cell::new(3, 1));
    assert_eq!(state.facing(), Direction::Right);
}

#[test]
fn an_npc_across_the_seam_blocks_the_walker() {
    let map = torus_with(
        &["....", "....", "....", "...."],
        vec![],
        vec![Npc::new(NpcId(7), Cell::new(0, 1), Direction::Down)],
    );
    let mut state = party(&map, 3, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Right);

    assert!(
        effects.is_empty(),
        "the NPC one cell across the seam blocks"
    );
    assert_eq!(state.cell(), Cell::new(3, 1));

    // And the cell it stands on is reported unwalkable through the seam.
    assert!(!map.is_walkable(Cell::new(0, 1)));
}

// ---------------------------------------------------------------------------
// Render interpolation through a seam
// ---------------------------------------------------------------------------

#[test]
fn render_offset_stays_continuous_across_a_seam_crossing() {
    // The offset is a function of the step's direction and progress, never of
    // the coordinates, so a seam crossing looks exactly like any other step.
    // The renderer, not the engine, decides how to draw the wrap.
    let map = open_torus();
    let mut seam = party(&map, 3, 1);
    let mut inland = party(&map, 1, 1);

    let mut seam_offsets = Vec::new();
    let mut inland_offsets = Vec::new();
    for _ in 0..8 {
        seam.tick(&map, Input::Direction(Direction::Right));
        inland.tick(&map, Input::Direction(Direction::Right));
        seam_offsets.push(seam.render_offset_16ths());
        inland_offsets.push(inland.render_offset_16ths());
    }

    assert_eq!(seam_offsets, inland_offsets);
    assert_eq!(
        seam_offsets,
        vec![
            (2, 0),
            (4, 0),
            (6, 0),
            (8, 0),
            (10, 0),
            (12, 0),
            (14, 0),
            (0, 0)
        ]
    );
    assert_eq!(seam.cell(), Cell::new(0, 1));
}

// ---------------------------------------------------------------------------
// Warps at the seam
// ---------------------------------------------------------------------------

#[test]
fn a_warp_rect_may_straddle_the_seam() {
    // Columns 3 and 0: legal on a torus, and it fires from both.
    let warp = Warp {
        source: CellRect::new(3, 1, 2, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x13),
        target_cell: Cell::new(2, 2),
        facing: Direction::Down,
    };
    let map = torus_with(&["....", "D..D", "....", "...."], vec![warp], vec![]);

    for (start, dir, landed) in [
        (Cell::new(3, 2), Direction::Up, Cell::new(3, 1)),
        (Cell::new(0, 2), Direction::Up, Cell::new(0, 1)),
    ] {
        let mut state = party(&map, start.x, start.y);
        let effects = walk_one_step(&mut state, &map, dir);
        assert_eq!(
            effects.last(),
            Some(&Effect::Warp {
                from: landed,
                trigger: WarpTrigger::MapChange,
                target_map: MapId(0x13),
                target_cell: Cell::new(2, 2),
                facing: Direction::Down,
            }),
            "the wrapped rect should cover ({}, {})",
            landed.x,
            landed.y
        );
    }
}

#[test]
fn a_seam_straddling_rect_does_not_cover_the_cells_in_between() {
    let warp = Warp {
        source: CellRect::new(3, 1, 2, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x13),
        target_cell: Cell::new(2, 2),
        facing: Direction::Down,
    };
    let map = torus_with(&["....", "DDDD", "....", "...."], vec![warp], vec![]);

    // Cells (1, 1) and (2, 1) are type 1 but outside the wrapped rect.
    for x in 1..=2 {
        let mut state = party(&map, x, 2);
        let effects = walk_one_step(&mut state, &map, Direction::Up);
        assert_eq!(
            effects.last(),
            Some(&Effect::WarpUnmapped {
                cell: Cell::new(x, 1)
            }),
            "({x}, 1) is not inside a rect that runs 3 -> 0"
        );
    }
}

#[test]
fn a_rect_running_past_the_edge_is_rejected_on_a_bounded_map() {
    let warp = Warp::ground(
        CellRect::new(3, 1, 2, 1),
        MapId(1),
        Cell::new(0, 0),
        Direction::Down,
    );
    let grid = CollisionGrid::filled(4, 4, 0).unwrap();

    assert!(matches!(
        FieldMap::new(MapId(0), grid.clone(), vec![warp], vec![]),
        Err(MapError::WarpRectOutOfBounds { warp_index: 0, .. })
    ));
    assert!(
        FieldMap::with_topology(MapId(0), grid, vec![warp], vec![], Topology::Torus).is_ok(),
        "the same rect is meaningful once the world wraps"
    );
}

#[test]
fn a_rect_bigger_than_the_world_is_rejected() {
    let warp = Warp::ground(
        CellRect::new(0, 0, 5, 1),
        MapId(1),
        Cell::new(0, 0),
        Direction::Down,
    );
    let grid = CollisionGrid::filled(4, 4, 0).unwrap();

    assert!(matches!(
        FieldMap::with_topology(MapId(0), grid, vec![warp], vec![], Topology::Torus),
        Err(MapError::WarpRectLargerThanMap { warp_index: 0, .. })
    ));
}

#[test]
fn unreachable_warp_diagnostics_follow_the_seam() {
    // The rect wraps onto a type-1 cell, so it is reachable even though its
    // own origin column is ordinary ground.
    let warp = Warp {
        source: CellRect::new(3, 1, 2, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x13),
        target_cell: Cell::new(0, 0),
        facing: Direction::Down,
    };
    let map = torus_with(&["....", "D...", "....", "...."], vec![warp], vec![]);

    assert!(map.unreachable_map_change_warps().is_empty());
}

// ---------------------------------------------------------------------------
// Talking across the seam
// ---------------------------------------------------------------------------

#[test]
fn an_npc_across_the_seam_is_in_talk_range() {
    let map = torus_with(
        &["....", "....", "....", "...."],
        vec![],
        vec![Npc::new(NpcId(7), Cell::new(0, 1), Direction::Left)],
    );
    let mut state = party_facing(&map, 3, 1, Direction::Right);

    let effects = state.tick(&map, Input::Action);

    assert_eq!(
        effects,
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(0, 1)
        }],
        "the talk point wraps with everything else"
    );
}

#[test]
fn a_half_cell_npc_still_straddles_correctly_at_the_seam() {
    // The object sits 8 pixels into cell 0, i.e. straddling cells 0 and 1 —
    // and the seam sits behind it, so the wrapped delta has to be the short
    // way round, not three cells the long way.
    let map = torus_with(
        &["....", "....", "....", "...."],
        vec![],
        vec![Npc::with_offset(
            NpcId(7),
            Cell::new(0, 1),
            SubCellOffset::new(8, 0),
            Direction::Left,
        )],
    );

    // From cell 3 facing right, the talk point is cell 0 and the object is +8.
    let mut across_seam = party_facing(&map, 3, 1, Direction::Right);
    assert_eq!(
        across_seam.tick(&map, Input::Action),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(0, 1)
        }]
    );

    // From cell 2 facing right, the talk point is cell 3: the object is a full
    // cell plus 8 away, still out of range even measured the short way.
    let mut too_far = party_facing(&map, 2, 1, Direction::Right);
    assert_eq!(
        too_far.tick(&map, Input::Action),
        vec![Effect::InteractNothing {
            facing: Direction::Right
        }]
    );
}

#[test]
fn talking_off_the_edge_of_a_bounded_map_is_unaffected() {
    let map = map_with(
        &["....", "....", "....", "...."],
        vec![],
        vec![Npc::new(NpcId(7), Cell::new(0, 1), Direction::Left)],
    );
    let mut state = party_facing(&map, 3, 1, Direction::Right);

    assert_eq!(
        state.tick(&map, Input::Action),
        vec![Effect::InteractNothing {
            facing: Direction::Right
        }],
        "a bounded map has no seam to reach through"
    );
}

// ---------------------------------------------------------------------------
// Coordinate normalisation
// ---------------------------------------------------------------------------

#[test]
fn normalize_wraps_on_a_torus_and_rejects_off_a_bounded_map() {
    let wrapped = open_torus();
    let bounded = map_with(&["....", "....", "....", "...."], vec![], vec![]);

    assert_eq!(
        wrapped.normalize_signed(-1, -1),
        Some(Cell::new(3, 3)),
        "negative coordinates come round the other side"
    );
    assert_eq!(wrapped.normalize_signed(4, 4), Some(Cell::new(0, 0)));
    assert_eq!(wrapped.normalize_signed(9, 6), Some(Cell::new(1, 2)));
    assert_eq!(wrapped.normalize(Cell::new(2, 2)), Some(Cell::new(2, 2)));

    assert_eq!(bounded.normalize_signed(-1, 0), None);
    assert_eq!(bounded.normalize_signed(4, 0), None);
    assert_eq!(bounded.normalize(Cell::new(2, 2)), Some(Cell::new(2, 2)));
}

#[test]
fn topology_is_reported_and_defaults_to_bounded() {
    let bounded = map_with(&["..", ".."], vec![], vec![]);
    assert_eq!(bounded.topology(), Topology::Bounded);
    assert!(!bounded.wraps());
    assert_eq!(Topology::default(), Topology::Bounded);

    let wrapped = torus(&["..", ".."]);
    assert_eq!(wrapped.topology(), Topology::Torus);
    assert!(wrapped.wraps());
}

#[test]
fn lookups_accept_unwrapped_coordinates_on_a_torus() {
    let map = torus(&["#...", "....", "....", "...."]);

    // Cell (4, 4) is (0, 0), which is solid.
    assert!(
        map.collision_at(Cell::new(4, 4))
            .is_some_and(|t| t.is_blocking())
    );
    assert!(!map.is_walkable(Cell::new(4, 4)));
    assert!(map.is_walkable(Cell::new(5, 4)));
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn a_wrapping_map_replays_identically() {
    fn script(seed: u32, len: usize) -> Vec<Input> {
        let mut s = seed;
        (0..len)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                match (s >> 16) % 6 {
                    0 => Input::Neutral,
                    1 => Input::Direction(Direction::Up),
                    2 => Input::Direction(Direction::Down),
                    3 => Input::Direction(Direction::Left),
                    4 => Input::Direction(Direction::Right),
                    _ => Input::Action,
                }
            })
            .collect()
    }

    #[rustfmt::skip]
    let rows = [
        "..D.....",
        ".#..~...",
        "........",
        "...##...",
        "D.......",
        "..#.....",
        "........",
        ".....$..",
    ];
    let warp = Warp {
        // Straddles the seam on both axes' worth of interest: x 7 -> 0.
        source: CellRect::new(7, 4, 2, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x13),
        target_cell: Cell::new(1, 1),
        facing: Direction::Down,
    };
    let npcs = vec![
        Npc::new(NpcId(1), Cell::new(0, 2), Direction::Down),
        Npc::new(NpcId(2), Cell::new(6, 6), Direction::Up),
    ];

    let replay = |inputs: &[Input]| {
        let map = torus_with(&rows, vec![warp], npcs.clone());
        let mut state = psiv_core::FieldState::new(
            &map,
            Cell::new(2, 2),
            Direction::Down,
            StepFrames::default(),
        )
        .expect("valid placement");
        let mut log = Vec::new();
        for (tick, &input) in inputs.iter().enumerate() {
            for effect in state.tick(&map, input) {
                log.push((tick, effect));
            }
        }
        (state, log)
    };

    for seed in 0..16_u32 {
        let inputs = script(seed.wrapping_mul(2_654_435_761), 200);
        let (a_state, a_log) = replay(&inputs);
        let (b_state, b_log) = replay(&inputs);
        assert_eq!(a_state, b_state, "seed {seed}");
        assert_eq!(a_log, b_log, "seed {seed}");
    }

    // And a long run never leaves the (entirely legal) world.
    let map = torus_with(&rows, vec![warp], npcs.clone());
    let mut state = psiv_core::FieldState::new(
        &map,
        Cell::new(2, 2),
        Direction::Down,
        StepFrames::default(),
    )
    .expect("valid placement");
    for (tick, &input) in script(0x5EED, 2_000).iter().enumerate() {
        state.tick(&map, input);
        let cell = state.cell();
        assert!(
            map.grid().contains(cell),
            "tick {tick}: ({}, {}) is outside the wrapped grid",
            cell.x,
            cell.y
        );
        assert!(
            map.is_walkable(cell),
            "tick {tick}: stood on a blocked cell ({}, {})",
            cell.x,
            cell.y
        );
    }
}

#[test]
fn a_degenerate_one_cell_world_does_not_panic() {
    // Not real data — the overworlds are 256 cells — but a 1x1 torus makes
    // every cell its own neighbour, which is the sharpest edge the wrapping
    // arithmetic has. It must stay boring rather than overflow or spin.
    let map = torus(&["."]);
    let mut state = party(&map, 0, 0);

    assert_eq!(
        map.neighbor(Cell::new(0, 0), Direction::Right),
        Some(Cell::new(0, 0))
    );

    let effects = run(&mut state, &map, Input::Direction(Direction::Right), 24);

    assert_eq!(state.cell(), Cell::new(0, 0));
    assert!(
        effects
            .iter()
            .all(|e| matches!(e, Effect::StepCompleted { .. })),
        "stepping onto itself should be an ordinary, if pointless, step"
    );
}
