//! Specification of talking to field objects.
//!
//! The rule is transcribed from `Interaction_ChkObjects` (`ps4.asm:118729`):
//! build a point one cell ahead of the party in its facing direction, then take
//! the first object within 8 pixels of it on both axes. For cell-aligned
//! objects that is plain "the cell you face"; for the 85 retail objects sitting
//! on half-cells it is deliberately wider, and these tests pin both.

mod common;

use common::{
    face_then_act, map, map_with, npc, npc_offset, party, party_at_rate, party_facing, press_action,
};
use psiv_core::{Cell, Direction, Effect, Input, TALK_RANGE_PX};

// ---------------------------------------------------------------------------
// The faced cell
// ---------------------------------------------------------------------------

#[test]
fn confirm_talks_to_the_npc_in_the_faced_cell_from_every_direction() {
    // One NPC on each side of the party, which stands in the middle.
    let npcs = vec![
        npc(10, 1, 0), // above
        npc(11, 1, 2), // below
        npc(12, 0, 1), // left
        npc(13, 2, 1), // right
    ];
    let map = map_with(&["...", "...", "..."], vec![], npcs);

    let expected = [
        (Direction::Up, 0, Cell::new(1, 0)),
        (Direction::Down, 1, Cell::new(1, 2)),
        (Direction::Left, 2, Cell::new(0, 1)),
        (Direction::Right, 3, Cell::new(2, 1)),
    ];

    for (dir, npc_index, cell) in expected {
        let mut state = party(&map, 1, 1);
        let effects = face_then_act(&mut state, &map, dir);
        assert_eq!(
            effects,
            vec![Effect::Interact { npc_index, cell }],
            "facing {dir:?} should reach the NPC in that cell"
        );
        assert_eq!(
            state.cell(),
            Cell::new(1, 1),
            "talking must not move anyone"
        );
    }
}

#[test]
fn confirm_facing_an_empty_cell_reports_nothing_there() {
    let map = map(&["...", "...", "..."]);
    let mut state = party(&map, 1, 1);

    let effects = press_action(&mut state, &map);

    assert_eq!(
        effects,
        vec![Effect::InteractNothing {
            facing: Direction::Down
        }]
    );
}

#[test]
fn confirm_never_reaches_past_the_faced_cell() {
    // The NPC is two cells ahead. The talk point is one cell ahead, so it is a
    // full cell out of range.
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 2)]);
    let mut state = party(&map, 1, 0);

    let effects = face_then_act(&mut state, &map, Direction::Right);
    assert_eq!(
        effects,
        vec![Effect::InteractNothing {
            facing: Direction::Right
        }]
    );
}

#[test]
fn confirm_does_not_reach_diagonally() {
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 2, 2)]);
    let mut state = party(&map, 1, 1);

    for dir in [Direction::Right, Direction::Down] {
        let mut state = state.clone();
        let effects = face_then_act(&mut state, &map, dir);
        assert_eq!(
            effects,
            vec![Effect::InteractNothing { facing: dir }],
            "an NPC diagonally adjacent is out of range facing {dir:?}"
        );
    }

    // Facing it head-on from an orthogonally adjacent cell does work.
    state
        .enter_map(&map, Cell::new(2, 1), Direction::Down)
        .expect("valid placement");
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(2, 2)
        }]
    );
}

#[test]
fn confirm_reaches_an_npc_through_a_wall_it_stands_on() {
    // 62 retail shopkeepers stand on $C counter cells. Blocking is irrelevant
    // to the talk check — `Interaction_ChkObjects` never consults collision.
    let map = map_with(&["...", ".$.", "..."], vec![], vec![npc(0x108, 1, 1)]);
    let mut state = party(&map, 1, 2);

    let effects = face_then_act(&mut state, &map, Direction::Up);

    assert_eq!(
        effects,
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 1)
        }]
    );
}

#[test]
fn the_first_npc_in_map_order_wins() {
    // Two objects sharing a cell, as the cartridge permits.
    let map = map_with(
        &["...", "...", "..."],
        vec![],
        vec![npc(20, 1, 0), npc(21, 1, 0)],
    );
    let mut state = party(&map, 1, 1);

    let effects = face_then_act(&mut state, &map, Direction::Up);

    assert_eq!(
        effects,
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 0)
        }]
    );
}

#[test]
fn facing_off_the_grid_reports_nothing_and_does_not_panic() {
    // The party stands in the corner with an NPC on its own cell; facing off
    // the grid must find nothing rather than wrap around to it.
    let map = map_with(&["..", ".."], vec![], vec![npc(10, 0, 0)]);

    for dir in [Direction::Up, Direction::Left] {
        let mut state = party_facing(&map, 0, 0, dir);
        let effects = press_action(&mut state, &map);
        assert_eq!(effects, vec![Effect::InteractNothing { facing: dir }]);
    }
}

// ---------------------------------------------------------------------------
// The half-cell rule
// ---------------------------------------------------------------------------

#[test]
fn the_talk_range_is_half_a_cell_on_each_axis() {
    assert_eq!(TALK_RANGE_PX, 8);
}

#[test]
fn an_object_on_a_half_cell_is_reachable_from_both_cells_it_straddles() {
    // Object at cell (1, 1) but 8 pixels right, so it visually straddles the
    // boundary with (2, 1). `cmpi.l #$80000 / bhi` is inclusive at exactly 8,
    // so both faced positions reach it.
    let map = map_with(
        &["....", "....", "...."],
        vec![],
        vec![npc_offset(30, 1, 1, 8, 0)],
    );

    // From (0, 1) facing right: talk point is cell (1, 1), object is +8. Hit.
    let mut from_left = party_facing(&map, 0, 1, Direction::Right);
    assert_eq!(
        press_action(&mut from_left, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 1)
        }]
    );

    // From (3, 1) facing left: talk point is cell (2, 1), object is -8. Also a
    // hit, even though the object's cell is (1, 1).
    let mut from_right = party_facing(&map, 3, 1, Direction::Left);
    assert_eq!(
        press_action(&mut from_right, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(2, 1)
        }]
    );
}

#[test]
fn a_half_cell_object_is_still_out_of_range_a_full_cell_away() {
    let map = map_with(
        &[".....", ".....", "....."],
        vec![],
        vec![npc_offset(30, 2, 1, 8, 0)],
    );
    // Talk point at cell (1, 1); object sits at 2*16+8 = 40, distance 24.
    let mut state = party_facing(&map, 0, 1, Direction::Right);
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::InteractNothing {
            facing: Direction::Right
        }]
    );
}

#[test]
fn a_nine_pixel_offset_falls_out_of_range_on_the_near_side() {
    // 9 > 8, so the object no longer answers from its own cell — the boundary
    // is exact, not approximate.
    let map = map_with(
        &["....", "....", "...."],
        vec![],
        vec![npc_offset(30, 1, 1, 9, 0)],
    );
    let mut state = party_facing(&map, 0, 1, Direction::Right);
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::InteractNothing {
            facing: Direction::Right
        }]
    );

    // But it does answer from the far side, 16 - 9 = 7 pixels away.
    let mut far = party_facing(&map, 3, 1, Direction::Left);
    assert_eq!(
        press_action(&mut far, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(2, 1)
        }]
    );
}

#[test]
fn the_range_applies_to_both_axes_independently() {
    // Offset on the perpendicular axis also has to be within 8 or the object
    // is missed, which is what stops a vertical half-cell object answering a
    // horizontal talk from the wrong row.
    let map = map_with(
        &["....", "....", "...."],
        vec![],
        vec![npc_offset(30, 1, 1, 0, 9)],
    );
    let mut state = party_facing(&map, 0, 1, Direction::Right);
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::InteractNothing {
            facing: Direction::Right
        }],
        "a 9-pixel vertical offset puts the object out of the box"
    );

    let map = map_with(
        &["....", "....", "...."],
        vec![],
        vec![npc_offset(31, 1, 1, 0, 8)],
    );
    let mut state = party_facing(&map, 0, 1, Direction::Right);
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 1)
        }],
        "8 pixels is still inside it"
    );
}

// ---------------------------------------------------------------------------
// Timing: edge triggering, buffering, and movement
// ---------------------------------------------------------------------------

#[test]
fn holding_confirm_talks_once_not_once_per_frame() {
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 0)]);
    let mut state = party(&map, 1, 1);
    state.tick(&map, Input::Direction(Direction::Up));
    while state.is_stepping() {
        state.tick(&map, Input::Neutral);
    }
    // The party stepped onto (1, 0)? No — an NPC blocks it, so it only turned.
    assert_eq!(state.cell(), Cell::new(1, 1));

    let mut talks = 0;
    for _ in 0..60 {
        for effect in state.tick(&map, Input::Action) {
            if matches!(effect, Effect::Interact { .. }) {
                talks += 1;
            }
        }
    }

    assert_eq!(
        talks, 1,
        "a held button is edge-triggered, like Joypad_Pressed"
    );
}

#[test]
fn releasing_and_pressing_confirm_again_talks_again() {
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 0)]);
    let mut state = party(&map, 1, 1);
    face_then_act(&mut state, &map, Direction::Up);

    state.tick(&map, Input::Neutral);
    let again = state.tick(&map, Input::Action);

    assert_eq!(
        again,
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 0)
        }]
    );
}

#[test]
fn confirm_mid_step_is_buffered_and_fires_when_the_step_lands() {
    // `FieldControls_GetInput` latches the press into `Field_Input_Buffer` and
    // only spends it once both step durations are zero, so the press is not
    // dropped — it talks the instant the party comes to rest.
    let map = map_with(&["....", "....", "...."], vec![], vec![npc(10, 3, 1)]);
    let mut state = party_at_rate(&map, 1, 1, 8);

    // Start a step to the right, toward the cell before the NPC.
    let start = state.tick(&map, Input::Direction(Direction::Right));
    assert!(start.is_empty());
    assert!(state.is_stepping());

    // Press confirm mid-step: nothing happens yet.
    let mid = state.tick(&map, Input::Action);
    assert!(
        mid.is_empty(),
        "the talk must not interrupt a committed step"
    );
    assert!(state.is_stepping());

    // Ride out the rest of the step with the button released.
    let mut effects = Vec::new();
    while state.is_stepping() {
        effects.extend(state.tick(&map, Input::Neutral));
    }
    assert_eq!(
        effects,
        vec![Effect::StepCompleted {
            cell: Cell::new(2, 1)
        }]
    );

    // The very next tick at rest spends the buffered press.
    let landed = state.tick(&map, Input::Neutral);
    assert_eq!(
        landed,
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(3, 1)
        }],
        "the buffered press fires on the first tick at rest"
    );
}

#[test]
fn a_buffered_press_is_spent_only_once() {
    let map = map_with(&["....", "....", "...."], vec![], vec![npc(10, 3, 1)]);
    let mut state = party_at_rate(&map, 1, 1, 8);

    state.tick(&map, Input::Direction(Direction::Right));
    state.tick(&map, Input::Action);
    while state.is_stepping() {
        state.tick(&map, Input::Neutral);
    }

    let mut talks = 0;
    for _ in 0..30 {
        for effect in state.tick(&map, Input::Neutral) {
            if matches!(effect, Effect::Interact { .. }) {
                talks += 1;
            }
        }
    }

    assert_eq!(talks, 1);
}

#[test]
fn entering_a_map_discards_a_pending_press() {
    // A buffered talk must not survive a map transition and fire at the
    // destination, where it would target a completely unrelated cell.
    let map = map_with(&["....", "....", "...."], vec![], vec![npc(10, 3, 1)]);
    let mut state = party_at_rate(&map, 1, 1, 8);

    state.tick(&map, Input::Direction(Direction::Right));
    state.tick(&map, Input::Action);
    while state.is_stepping() {
        state.tick(&map, Input::Neutral);
    }

    state
        .enter_map(&map, Cell::new(2, 1), Direction::Right)
        .expect("valid placement");

    let after = state.tick(&map, Input::Neutral);
    assert!(
        after.is_empty(),
        "the pending press should not survive a warp"
    );
}

#[test]
fn talking_consumes_the_tick_so_no_step_starts() {
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 0)]);
    let mut state = party(&map, 1, 1);

    // Face down toward open ground, then press confirm while the engine would
    // otherwise be free to walk.
    let effects = state.tick(&map, Input::Action);

    assert_eq!(
        effects,
        vec![Effect::InteractNothing {
            facing: Direction::Down
        }]
    );
    assert!(
        !state.is_stepping(),
        "the interaction frame never reaches the movement code"
    );
}

#[test]
fn confirm_does_not_change_facing() {
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 0)]);
    let mut state = party(&map, 1, 1);
    let before = state.facing();

    state.tick(&map, Input::Action);

    assert_eq!(state.facing(), before);
}
