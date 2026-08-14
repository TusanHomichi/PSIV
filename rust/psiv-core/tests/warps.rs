//! Specification of map transitions: when a warp fires, what it carries, and
//! what happens when the data does not line up.

mod common;

use common::{map_with, party, run, walk_one_step};
use psiv_core::{Cell, CellRect, Direction, Effect, FieldMap, Input, MapId, Warp, WarpTrigger};

fn door(source: Cell, target_map: u16, target: Cell, facing: Direction) -> Warp {
    Warp::door(source, MapId(target_map), target, facing)
}

/// A three-wide room with a doorway at the top middle.
fn doorway_map(warps: Vec<Warp>) -> FieldMap {
    map_with(&[".D.", "...", "..."], warps, vec![])
}

#[test]
fn stepping_onto_a_mapped_doorway_warps_on_the_landing_tick() {
    let map = doorway_map(vec![door(
        Cell::new(1, 0),
        0x13,
        Cell::new(2, 6),
        Direction::Down,
    )]);
    let mut state = party(&map, 1, 1);

    // Seven ticks of the eight-frame step: still walking, nothing fired.
    let during = run(&mut state, &map, Input::Direction(Direction::Up), 7);
    assert!(during.is_empty(), "the warp fired before the step landed");

    let landing = state.tick(&map, Input::Direction(Direction::Up));
    assert_eq!(
        landing,
        vec![
            Effect::StepCompleted {
                cell: Cell::new(1, 0)
            },
            Effect::Warp {
                from: Cell::new(1, 0),
                trigger: WarpTrigger::MapChange,
                target_map: MapId(0x13),
                target_cell: Cell::new(2, 6),
                facing: Direction::Down,
            },
        ],
        "StepCompleted must precede the Warp it caused"
    );
    assert_eq!(state.cell(), Cell::new(1, 0));
}

#[test]
fn a_warp_fires_exactly_once_while_the_party_stands_on_the_doorway() {
    let map = doorway_map(vec![door(
        Cell::new(1, 0),
        0x13,
        Cell::new(2, 6),
        Direction::Down,
    )]);
    let mut state = party(&map, 1, 1);

    let mut warps = 0;
    // Walk in, then keep holding up against the map edge for a long time.
    for effect in run(&mut state, &map, Input::Direction(Direction::Up), 200) {
        if matches!(effect, Effect::Warp { .. }) {
            warps += 1;
        }
    }

    assert_eq!(warps, 1, "the transition retriggered while standing on it");
}

#[test]
fn stepping_off_and_back_onto_a_doorway_fires_it_again() {
    let map = doorway_map(vec![door(
        Cell::new(1, 0),
        0x13,
        Cell::new(2, 6),
        Direction::Down,
    )]);
    let mut state = party(&map, 1, 1);

    walk_one_step(&mut state, &map, Direction::Up);
    walk_one_step(&mut state, &map, Direction::Down);
    let again = walk_one_step(&mut state, &map, Direction::Up);

    assert_eq!(
        again
            .iter()
            .filter(|e| matches!(e, Effect::Warp { .. }))
            .count(),
        1,
        "arriving a second time should fire the transition again"
    );
}

#[test]
fn placing_the_party_onto_a_doorway_does_not_fire_it() {
    // Otherwise every warp whose destination is itself a doorway cell would
    // bounce the party forever.
    let map = doorway_map(vec![door(
        Cell::new(1, 0),
        0x13,
        Cell::new(2, 6),
        Direction::Down,
    )]);
    let mut state = party(&map, 1, 1);

    state
        .enter_map(&map, Cell::new(1, 0), Direction::Up)
        .expect("valid placement");
    let effects = run(&mut state, &map, Input::Neutral, 20);

    assert!(effects.is_empty());
}

#[test]
fn landing_on_an_unmapped_map_change_cell_reports_it() {
    let map = doorway_map(vec![]);
    let mut state = party(&map, 1, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Up);

    assert_eq!(
        effects,
        vec![
            Effect::StepCompleted {
                cell: Cell::new(1, 0)
            },
            Effect::WarpUnmapped {
                cell: Cell::new(1, 0)
            },
        ]
    );
    assert_eq!(
        state.cell(),
        Cell::new(1, 0),
        "an unmapped transition still leaves the party standing on the cell"
    );
}

#[test]
fn a_warp_rect_over_a_non_map_change_cell_does_not_fire() {
    // The warp's rectangle covers the whole top row, but only the middle cell
    // is collision type 1. Type is the trigger; the rectangle only selects
    // which transition applies.
    let warp = Warp {
        source: CellRect::new(0, 0, 3, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x13),
        target_cell: Cell::new(2, 6),
        facing: Direction::Down,
    };
    let map = doorway_map(vec![warp]);

    let mut state = party(&map, 0, 1);
    let effects = walk_one_step(&mut state, &map, Direction::Up);
    assert_eq!(
        effects,
        vec![Effect::StepCompleted {
            cell: Cell::new(0, 0)
        }],
        "a plain cell inside a warp rect must not warp"
    );

    let mut state = party(&map, 1, 1);
    let effects = walk_one_step(&mut state, &map, Direction::Up);
    assert_eq!(
        effects.len(),
        2,
        "the type-1 cell inside the rect must warp"
    );
}

#[test]
fn a_map_change_cell_outside_every_warp_rect_is_unmapped() {
    // A warp exists, but its rectangle is somewhere else entirely.
    let warp = Warp::door(Cell::new(0, 2), MapId(0x13), Cell::new(0, 0), Direction::Up);
    let map = doorway_map(vec![warp]);
    let mut state = party(&map, 1, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Up);

    assert!(effects.contains(&Effect::WarpUnmapped {
        cell: Cell::new(1, 0)
    }));
}

#[test]
fn a_multi_cell_warp_rect_covers_every_doorway_cell_it_spans() {
    let warp = Warp {
        source: CellRect::new(0, 0, 3, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x20),
        target_cell: Cell::new(1, 1),
        facing: Direction::Left,
    };
    let map = map_with(&["DDD", "...", "..."], vec![warp], vec![]);

    for x in 0..3 {
        let mut state = party(&map, x, 1);
        let effects = walk_one_step(&mut state, &map, Direction::Up);
        assert_eq!(
            effects.last(),
            Some(&Effect::Warp {
                from: Cell::new(x, 0),
                trigger: WarpTrigger::MapChange,
                target_map: MapId(0x20),
                target_cell: Cell::new(1, 1),
                facing: Direction::Left,
            }),
            "doorway cell ({x}, 0)"
        );
    }
}

#[test]
fn overlapping_warps_resolve_to_the_first_in_data_order() {
    // Mirrors the cartridge's linear scan of the transition table.
    let first = Warp {
        source: CellRect::new(0, 0, 3, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x01),
        target_cell: Cell::new(0, 0),
        facing: Direction::Up,
    };
    let second = Warp {
        source: CellRect::new(1, 0, 1, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x02),
        target_cell: Cell::new(2, 2),
        facing: Direction::Down,
    };
    let map = doorway_map(vec![first, second]);
    let mut state = party(&map, 1, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Up);

    assert_eq!(
        effects.last(),
        Some(&Effect::Warp {
            from: Cell::new(1, 0),
            trigger: WarpTrigger::MapChange,
            target_map: MapId(0x01),
            target_cell: Cell::new(0, 0),
            facing: Direction::Up,
        })
    );
}

#[test]
fn the_full_round_trip_through_a_warp_lands_where_the_data_says() {
    let town = map_with(
        &[".D.", "...", "..."],
        vec![door(
            Cell::new(1, 0),
            0x13,
            Cell::new(1, 2),
            Direction::Down,
        )],
        vec![],
    );
    let shop = FieldMap::new(
        MapId(0x13),
        common::grid(&["...", "...", ".D."]),
        vec![door(
            Cell::new(1, 2),
            0x00,
            Cell::new(1, 1),
            Direction::Down,
        )],
        vec![],
    )
    .expect("valid map");

    let mut state = party(&town, 1, 1);

    let into_shop = walk_one_step(&mut state, &town, Direction::Up);
    let Some(&Effect::Warp {
        target_map,
        target_cell,
        facing,
        ..
    }) = into_shop.last()
    else {
        panic!("expected a warp into the shop, got {into_shop:?}");
    };
    assert_eq!(target_map, shop.id());

    state
        .enter_map(&shop, target_cell, facing)
        .expect("the transition table's destination must be standable");
    assert_eq!(state.map(), MapId(0x13));
    assert_eq!(state.cell(), Cell::new(1, 2));
    assert_eq!(state.facing(), Direction::Down);

    // Walk off the doorway and back onto it to return.
    walk_one_step(&mut state, &shop, Direction::Up);
    let back = walk_one_step(&mut state, &shop, Direction::Down);
    assert_eq!(
        back.last(),
        Some(&Effect::Warp {
            from: Cell::new(1, 2),
            trigger: WarpTrigger::MapChange,
            target_map: MapId(0x00),
            target_cell: Cell::new(1, 1),
            facing: Direction::Down,
        })
    );
}

// ---------------------------------------------------------------------------
// The two transition tables
//
// `RunMapTransitions` picks its walker from the collision type of the cell the
// party stands on: type 1 reads the map-change table (`MapTransTile_MapChange`)
// and everything else standable reads the normal table (`MapTransTile_Normal`).
// ---------------------------------------------------------------------------

#[test]
fn walking_along_a_wide_doorway_fires_once_not_once_per_cell() {
    // The whole top row is type 1 and covered by one map-change warp. The
    // cartridge only fires when the previously occupied cell was not also
    // type 1, so walking in and then sideways is a single transition.
    let warp = Warp {
        source: CellRect::new(0, 0, 3, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x20),
        target_cell: Cell::new(1, 1),
        facing: Direction::Left,
    };
    let map = map_with(&["DDD", "...", "..."], vec![warp], vec![]);
    let mut state = party(&map, 0, 1);

    let entering = walk_one_step(&mut state, &map, Direction::Up);
    assert_eq!(
        entering.len(),
        2,
        "stepping onto the doorway should fire once"
    );

    let along = walk_one_step(&mut state, &map, Direction::Right);
    assert_eq!(
        along,
        vec![Effect::StepCompleted {
            cell: Cell::new(1, 0)
        }],
        "walking from one doorway cell to the next must not re-fire"
    );

    let further = walk_one_step(&mut state, &map, Direction::Right);
    assert_eq!(
        further,
        vec![Effect::StepCompleted {
            cell: Cell::new(2, 0)
        }]
    );

    // Step off onto ordinary ground and back on: that is a fresh arrival.
    walk_one_step(&mut state, &map, Direction::Down);
    let re_entering = walk_one_step(&mut state, &map, Direction::Up);
    assert_eq!(re_entering.len(), 2, "re-entering the doorway fires again");
}

#[test]
fn an_unmapped_doorway_does_not_re_report_as_you_walk_along_it() {
    let map = map_with(&["DDD", "...", "..."], vec![], vec![]);
    let mut state = party(&map, 0, 1);

    let entering = walk_one_step(&mut state, &map, Direction::Up);
    assert!(entering.contains(&Effect::WarpUnmapped {
        cell: Cell::new(0, 0)
    }));

    let along = walk_one_step(&mut state, &map, Direction::Right);
    assert_eq!(
        along,
        vec![Effect::StepCompleted {
            cell: Cell::new(1, 0)
        }],
        "the data problem is reported on arrival, not on every cell"
    );
}

#[test]
fn arriving_on_a_doorway_cell_suppresses_it_until_you_step_off() {
    // Warp destinations are often the doormat itself. If arrival counted as a
    // fresh entry the party would ping-pong between two maps forever.
    let warp = Warp::door(
        Cell::new(1, 0),
        MapId(0x20),
        Cell::new(1, 0),
        Direction::Down,
    );
    let map = map_with(&[".D.", "...", "..."], vec![warp], vec![]);
    let mut state = party(&map, 1, 1);

    state
        .enter_map(&map, Cell::new(1, 0), Direction::Down)
        .expect("valid placement");

    // Walk sideways along the row: (0, 0) is ordinary ground, so this is a
    // step off the doorway, not a second arrival on it.
    let sideways = walk_one_step(&mut state, &map, Direction::Left);
    assert_eq!(
        sideways,
        vec![Effect::StepCompleted {
            cell: Cell::new(0, 0)
        }]
    );

    let back = walk_one_step(&mut state, &map, Direction::Right);
    assert_eq!(back.len(), 2, "coming back onto the doorway fires it");
}

#[test]
fn a_normal_ground_transition_fires_from_ordinary_cells() {
    // A map edge: the whole bottom row transitions, and none of it is type 1.
    let edge = Warp::ground(
        CellRect::new(0, 2, 3, 1),
        MapId(0x31),
        Cell::new(1, 0),
        Direction::Down,
    );
    let map = map_with(&["...", "...", "..+"], vec![edge], vec![]);
    let mut state = party(&map, 2, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Down);

    assert_eq!(
        effects,
        vec![
            Effect::StepCompleted {
                cell: Cell::new(2, 2)
            },
            Effect::Warp {
                from: Cell::new(2, 2),
                trigger: WarpTrigger::NormalGround,
                target_map: MapId(0x31),
                target_cell: Cell::new(1, 0),
                facing: Direction::Down,
            },
        ],
        "a recovery cell inside a normal-ground rect still transitions"
    );
}

#[test]
fn a_normal_ground_transition_does_not_fire_from_a_doorway_cell() {
    // The rect covers the doorway too, but standing on type 1 sends
    // `RunMapTransitions` to the other table, which has nothing here.
    let edge = Warp::ground(
        CellRect::new(0, 0, 3, 1),
        MapId(0x31),
        Cell::new(1, 1),
        Direction::Down,
    );
    let map = map_with(&[".D.", "...", "..."], vec![edge], vec![]);
    let mut state = party(&map, 1, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Up);

    assert_eq!(
        effects,
        vec![
            Effect::StepCompleted {
                cell: Cell::new(1, 0)
            },
            Effect::WarpUnmapped {
                cell: Cell::new(1, 0)
            },
        ]
    );
}

#[test]
fn a_map_change_warp_never_fires_from_ordinary_ground() {
    let doorway = Warp {
        source: CellRect::new(0, 0, 3, 1),
        trigger: WarpTrigger::MapChange,
        target_map: MapId(0x20),
        target_cell: Cell::new(1, 1),
        facing: Direction::Left,
    };
    let map = map_with(&["...", "...", "..."], vec![doorway], vec![]);
    let mut state = party(&map, 1, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Up);

    assert_eq!(
        effects,
        vec![Effect::StepCompleted {
            cell: Cell::new(1, 0)
        }],
        "no cell in the rect is type 1, so the doorway table is never read"
    );
}

#[test]
fn the_two_tables_coexist_on_one_map() {
    let doorway = Warp::door(Cell::new(1, 0), MapId(0x20), Cell::new(0, 0), Direction::Up);
    let edge = Warp::ground(
        CellRect::new(0, 2, 3, 1),
        MapId(0x31),
        Cell::new(2, 2),
        Direction::Down,
    );
    let map = map_with(&[".D.", "...", "..."], vec![doorway, edge], vec![]);

    let mut state = party(&map, 1, 1);
    let up = walk_one_step(&mut state, &map, Direction::Up);
    assert_eq!(
        up.last(),
        Some(&Effect::Warp {
            from: Cell::new(1, 0),
            trigger: WarpTrigger::MapChange,
            target_map: MapId(0x20),
            target_cell: Cell::new(0, 0),
            facing: Direction::Up,
        })
    );

    let mut state = party(&map, 1, 1);
    let down = walk_one_step(&mut state, &map, Direction::Down);
    assert_eq!(
        down.last(),
        Some(&Effect::Warp {
            from: Cell::new(1, 2),
            trigger: WarpTrigger::NormalGround,
            target_map: MapId(0x31),
            target_cell: Cell::new(2, 2),
            facing: Direction::Down,
        })
    );
}
