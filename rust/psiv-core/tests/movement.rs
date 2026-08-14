//! Specification of field movement: what blocks, what does not, when a step
//! commits, and where the walker may not go.

mod common;

use common::{map, map_with, npc, party, party_at_rate, run, walk_one_step};
use psiv_core::{Cell, Direction, Effect, FieldState, Input, StepFrames};

const BLOCKING: [(char, u8, &str); 5] = [
    ('#', 0x8, "solid"),
    ('~', 0x9, "water"),
    ('s', 0xA, "sand"),
    ('i', 0xB, "ice"),
    ('$', 0xC, "shop"),
];

const WALKABLE: [(char, u8, &str); 4] = [
    ('.', 0x0, "normal"),
    ('D', 0x1, "map change"),
    ('+', 0x2, "recovery"),
    ('5', 0x5, "unnamed"),
];

// ---------------------------------------------------------------------------
// Blocking
// ---------------------------------------------------------------------------

#[test]
fn each_blocking_type_stops_the_walker() {
    for (symbol, value, name) in BLOCKING {
        // The party starts in the middle; the blocker sits directly right.
        let row: String = format!(".{symbol}.");
        let map = map(&["...", &row, "..."]);
        let mut state = party(&map, 0, 1);

        let effects = walk_one_step(&mut state, &map, Direction::Right);

        assert!(
            effects.is_empty(),
            "{name} ({value:#X}) let the walker step onto it"
        );
        assert_eq!(state.cell(), Cell::new(0, 1), "{name} ({value:#X})");
        assert!(!state.is_stepping(), "{name} ({value:#X})");
        assert_eq!(
            state.facing(),
            Direction::Right,
            "{name} ({value:#X}) should still turn the walker"
        );
    }
}

#[test]
fn each_walkable_type_accepts_the_walker() {
    for (symbol, value, name) in WALKABLE {
        let row: String = format!(".{symbol}.");
        let map = map(&["...", &row, "..."]);
        let mut state = party(&map, 0, 1);

        walk_one_step(&mut state, &map, Direction::Right);

        assert_eq!(
            state.cell(),
            Cell::new(1, 1),
            "{name} ({value:#X}) should be walkable"
        );
    }
}

#[test]
fn npcs_block_movement() {
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(4, 1, 1)]);
    let mut state = party(&map, 0, 1);

    let effects = walk_one_step(&mut state, &map, Direction::Right);

    assert!(effects.is_empty());
    assert_eq!(state.cell(), Cell::new(0, 1));
    assert_eq!(state.facing(), Direction::Right);
}

#[test]
fn a_cleared_npc_cell_becomes_walkable() {
    // The same geometry without the NPC: proves the previous test failed on the
    // NPC and not on the terrain.
    let map = map_with(&["...", "...", "..."], vec![], vec![]);
    let mut state = party(&map, 0, 1);

    walk_one_step(&mut state, &map, Direction::Right);

    assert_eq!(state.cell(), Cell::new(1, 1));
}

// ---------------------------------------------------------------------------
// Facing
// ---------------------------------------------------------------------------

#[test]
fn facing_a_wall_is_free() {
    let map = map(&["###", "#.#", "###"]);
    let mut state = party(&map, 1, 1);

    for dir in Direction::ALL {
        let effects = state.tick(&map, Input::Direction(dir));
        assert!(effects.is_empty());
        assert_eq!(state.facing(), dir, "facing should change without moving");
        assert_eq!(state.cell(), Cell::new(1, 1));
        assert!(!state.is_stepping());
    }
}

#[test]
fn neutral_input_changes_nothing() {
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 1, 1);
    let before = state.clone();

    let effects = run(&mut state, &map, Input::Neutral, 30);

    assert!(effects.is_empty());
    assert_eq!(state, before);
}

#[test]
fn facing_updates_on_the_tick_a_step_begins() {
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 0, 0);

    state.tick(&map, Input::Direction(Direction::Right));

    assert_eq!(state.facing(), Direction::Right);
    assert!(state.is_stepping());
    assert_eq!(
        state.cell(),
        Cell::new(0, 0),
        "the cell only moves on landing"
    );
}

// ---------------------------------------------------------------------------
// Step commitment
// ---------------------------------------------------------------------------

#[test]
fn a_started_step_completes_after_the_input_is_released() {
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 0, 0);

    let first = state.tick(&map, Input::Direction(Direction::Right));
    assert!(first.is_empty());

    // Input released for the rest of the step; the cartridge commits to the
    // whole cell once the step starts.
    let rest = run(&mut state, &map, Input::Neutral, 7);

    assert_eq!(
        rest,
        vec![Effect::StepCompleted {
            cell: Cell::new(1, 0)
        }]
    );
    assert_eq!(state.cell(), Cell::new(1, 0));
    assert!(!state.is_stepping());
}

#[test]
fn a_direction_change_mid_step_is_ignored_until_the_step_lands() {
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 0, 0);

    state.tick(&map, Input::Direction(Direction::Right));
    let mid = run(&mut state, &map, Input::Direction(Direction::Down), 6);

    assert!(mid.is_empty());
    assert_eq!(
        state.facing(),
        Direction::Right,
        "facing is not re-read mid-step"
    );
    assert_eq!(state.step_direction(), Some(Direction::Right));

    // The eighth tick lands, and only then does the new direction take effect.
    let land = state.tick(&map, Input::Direction(Direction::Down));
    assert_eq!(
        land,
        vec![Effect::StepCompleted {
            cell: Cell::new(1, 0)
        }]
    );
    assert_eq!(state.cell(), Cell::new(1, 0));

    let next = state.tick(&map, Input::Direction(Direction::Down));
    assert!(next.is_empty());
    assert_eq!(state.facing(), Direction::Down);
    assert_eq!(state.step_destination(), Some(Cell::new(1, 1)));
}

#[test]
fn a_step_lands_on_exactly_the_configured_frame() {
    for frames in [1_u8, 2, 3, 8, 16, 30] {
        let map = map(&["....", "....", "....", "...."]);
        let mut state = party_at_rate(&map, 0, 0, frames);

        for tick in 1..frames {
            let effects = state.tick(&map, Input::Direction(Direction::Right));
            assert!(
                effects.is_empty(),
                "landed early on tick {tick} of {frames}"
            );
            assert_eq!(state.cell(), Cell::new(0, 0));
        }

        let effects = state.tick(&map, Input::Direction(Direction::Right));
        assert_eq!(
            effects,
            vec![Effect::StepCompleted {
                cell: Cell::new(1, 0)
            }],
            "step of {frames} frames did not land on its last frame"
        );
    }
}

#[test]
fn holding_a_direction_walks_cell_after_cell() {
    let map = map(&["....", "....", "....", "...."]);
    let mut state = party(&map, 0, 0);

    let effects = run(&mut state, &map, Input::Direction(Direction::Right), 24);

    assert_eq!(
        effects,
        vec![
            Effect::StepCompleted {
                cell: Cell::new(1, 0)
            },
            Effect::StepCompleted {
                cell: Cell::new(2, 0)
            },
            Effect::StepCompleted {
                cell: Cell::new(3, 0)
            },
        ]
    );
    assert_eq!(state.cell(), Cell::new(3, 0));
}

#[test]
fn walking_into_the_edge_after_a_step_stops_cleanly() {
    let map = map(&["..", ".."]);
    let mut state = party(&map, 0, 0);

    let effects = run(&mut state, &map, Input::Direction(Direction::Right), 40);

    assert_eq!(
        effects,
        vec![Effect::StepCompleted {
            cell: Cell::new(1, 0)
        }],
        "the walker should stop at the edge, not keep emitting"
    );
    assert_eq!(state.cell(), Cell::new(1, 0));
}

// ---------------------------------------------------------------------------
// Boundaries
// ---------------------------------------------------------------------------

#[test]
fn the_grid_edge_blocks_in_every_direction_without_wrapping() {
    let map = map(&["..", ".."]);

    // Top-left corner: up and left leave the grid.
    let mut top_left = party(&map, 0, 0);
    for dir in [Direction::Up, Direction::Left] {
        let effects = walk_one_step(&mut top_left, &map, dir);
        assert!(effects.is_empty(), "{dir:?} escaped the top-left corner");
        assert_eq!(top_left.cell(), Cell::new(0, 0));
        assert_eq!(top_left.facing(), dir);
    }

    // Bottom-right corner: down and right leave the grid.
    let mut bottom_right = party(&map, 1, 1);
    for dir in [Direction::Down, Direction::Right] {
        let effects = walk_one_step(&mut bottom_right, &map, dir);
        assert!(
            effects.is_empty(),
            "{dir:?} escaped the bottom-right corner"
        );
        assert_eq!(bottom_right.cell(), Cell::new(1, 1));
    }
}

#[test]
fn a_one_cell_map_traps_the_walker() {
    let map = map(&["."]);
    let mut state = party(&map, 0, 0);

    for dir in Direction::ALL {
        let effects = walk_one_step(&mut state, &map, dir);
        assert!(effects.is_empty());
        assert_eq!(state.cell(), Cell::new(0, 0));
    }
}

// ---------------------------------------------------------------------------
// Render interpolation
// ---------------------------------------------------------------------------

#[test]
fn render_offset_is_zero_at_rest() {
    let map = map(&["...", "...", "..."]);
    let state = party(&map, 1, 1);
    assert_eq!(state.render_offset_16ths(), (0, 0));
    assert_eq!(state.step_progress(), 0);
}

#[test]
fn render_offset_advances_monotonically_across_a_step() {
    for frames in [2_u8, 3, 5, 8, 16] {
        let map = map(&["...", "...", "..."]);
        let mut state = party_at_rate(&map, 0, 1, frames);

        let mut previous = 0;
        for tick in 1..frames {
            state.tick(&map, Input::Direction(Direction::Right));
            let (dx, dy) = state.render_offset_16ths();
            assert_eq!(dy, 0, "a horizontal step must not drift vertically");
            assert!(
                dx >= previous,
                "offset went backwards on tick {tick} of {frames}: {dx} after {previous}"
            );
            assert!(
                dx < 16,
                "offset reached the next cell before landing: {dx} on tick {tick} of {frames}"
            );
            previous = dx;
        }

        state.tick(&map, Input::Direction(Direction::Right));
        assert_eq!(
            state.render_offset_16ths(),
            (0, 0),
            "landing resets the offset"
        );
        assert_eq!(state.cell(), Cell::new(1, 1));
    }
}

#[test]
fn render_offset_follows_the_step_axis_and_sign() {
    let map = map(&["...", "...", "..."]);
    let expected = [
        (Direction::Up, (0, -2)),
        (Direction::Down, (0, 2)),
        (Direction::Left, (-2, 0)),
        (Direction::Right, (2, 0)),
    ];

    for (dir, offset) in expected {
        let mut state = party(&map, 1, 1);
        state.tick(&map, Input::Direction(dir));
        assert_eq!(
            state.render_offset_16ths(),
            offset,
            "first tick of a {dir:?} step at 8 frames"
        );
    }
}

#[test]
fn the_default_eight_frame_step_walks_two_pixels_a_tick() {
    // A cell is 16 pixels and the offset is in sixteenths of a cell, so these
    // numbers are pixels at 1x.
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 0, 0);

    let mut offsets = Vec::new();
    for _ in 0..8 {
        state.tick(&map, Input::Direction(Direction::Right));
        offsets.push(state.render_offset_16ths().0);
    }

    assert_eq!(offsets, vec![2, 4, 6, 8, 10, 12, 14, 0]);
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

#[test]
fn entering_a_map_cancels_any_step_in_progress() {
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 0, 0);

    state.tick(&map, Input::Direction(Direction::Right));
    assert!(state.is_stepping());

    state
        .enter_map(&map, Cell::new(2, 2), Direction::Up)
        .expect("valid placement");

    assert!(!state.is_stepping());
    assert_eq!(state.cell(), Cell::new(2, 2));
    assert_eq!(state.facing(), Direction::Up);
    assert_eq!(state.render_offset_16ths(), (0, 0));
}

#[test]
fn a_step_duration_of_zero_is_rejected() {
    assert!(StepFrames::new(0).is_err());
}

#[test]
fn the_state_reports_the_map_it_is_on() {
    let map = map(&["...", "...", "..."]);
    let state = FieldState::new(&map, Cell::new(0, 0), Direction::Up, StepFrames::default())
        .expect("valid placement");
    assert_eq!(state.map(), map.id());
}
