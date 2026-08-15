//! Specification of talking to field objects.
//!
//! The rule is transcribed from `Interaction_ChkObjects` (`ps4.asm:118729`):
//! build a point one cell ahead of the party in its facing direction, then take
//! the first object within 8 pixels of it on both axes. For cell-aligned
//! objects that is plain "the cell you face"; for the 85 retail objects sitting
//! on half-cells it is deliberately wider, and these tests pin both.

mod common;

use common::{
    face_then_act, map, map_with, npc, npc_offset, party, party_at_rate, party_facing,
    press_action, walk_one_step,
};
use psiv_core::{Cell, Direction, Effect, Input, InteractReach, NpcId, TALK_RANGE_PX};

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
            vec![Effect::Interact {
                npc_index,
                cell,
                reach: InteractReach::Adjacent
            }],
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
            cell: Cell::new(2, 2),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(1, 1),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(1, 0),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(1, 1),
            reach: InteractReach::Adjacent,
        }]
    );

    // From (3, 1) facing left: talk point is cell (2, 1), object is -8. Also a
    // hit, even though the object's cell is (1, 1).
    let mut from_right = party_facing(&map, 3, 1, Direction::Left);
    assert_eq!(
        press_action(&mut from_right, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(2, 1),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(2, 1),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(1, 1),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(1, 0),
            reach: InteractReach::Adjacent,
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
            cell: Cell::new(3, 1),
            reach: InteractReach::Adjacent,
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

// ---------------------------------------------------------------------------
// Reaching across a counter
//
// `Interaction_ChkObjsSpecial` (`ps4.asm:119267`) runs before the ordinary
// object check. When the faced tile is collision $C it displaces the probe a
// further 16 pixels in the facing direction (`loc_5915C`) and re-runs the scan,
// which lands it two cells ahead — the Piata academy principal behind his desk,
// or a shop clerk behind the counter.
// ---------------------------------------------------------------------------

#[test]
fn confirm_reaches_across_a_counter_cell_to_the_npc_beyond() {
    // Party at (1, 3), counter at (1, 2), principal at (1, 1).
    let map = map_with(
        &["...", "...", ".$.", "..."],
        vec![],
        vec![npc(0x108, 1, 1)],
    );
    let mut state = party_facing(&map, 1, 3, Direction::Up);

    let effects = press_action(&mut state, &map);

    assert_eq!(
        effects,
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 1),
            reach: InteractReach::AcrossCounter,
        }],
        "the desk should be reached across, not blocked by"
    );
}

#[test]
fn the_counter_reach_works_from_every_facing() {
    // Party in the middle of a 5x5 room, a $C counter on each side of it and
    // an NPC directly beyond each counter.
    #[rustfmt::skip]
    let rows = [
        ".....",
        "..$..",
        ".$.$.",
        "..$..",
        ".....",
    ];
    let npcs = vec![
        npc(0x200, 2, 0), // beyond the counter above
        npc(0x201, 2, 4), // below
        npc(0x202, 0, 2), // left
        npc(0x203, 4, 2), // right
    ];
    let map = map_with(&rows, vec![], npcs);

    let expected = [
        (Direction::Up, 0, Cell::new(2, 0)),
        (Direction::Down, 1, Cell::new(2, 4)),
        (Direction::Left, 2, Cell::new(0, 2)),
        (Direction::Right, 3, Cell::new(4, 2)),
    ];

    for (dir, npc_index, cell) in expected {
        let mut state = party_facing(&map, 2, 2, dir);
        assert_eq!(
            press_action(&mut state, &map),
            vec![Effect::Interact {
                npc_index,
                cell,
                reach: InteractReach::AcrossCounter,
            }],
            "counter reach facing {dir:?}"
        );
    }
}

#[test]
fn the_extended_reach_only_applies_across_a_shop_cell() {
    // The same geometry with a solid wall instead of a counter: two cells is
    // out of reach, because `ChkObjsSpecial` only fires on collision $C.
    for blocker in ['#', '~', 's', 'i', '.'] {
        let middle = format!(".{blocker}.");
        let map = map_with(
            &["...", "...", &middle, "..."],
            vec![],
            vec![npc(0x108, 1, 1)],
        );
        let mut state = party_facing(&map, 1, 3, Direction::Up);

        assert_eq!(
            press_action(&mut state, &map),
            vec![Effect::InteractNothing {
                facing: Direction::Up
            }],
            "collision {blocker:?} must not extend the talk reach"
        );
    }
}

#[test]
fn an_npc_standing_on_the_counter_itself_is_still_reached() {
    // The shopkeeper stands *on* the $C cell rather than behind it — 62 retail
    // objects do. The special probe misses (nothing two cells ahead) and the
    // ordinary probe picks them up.
    let map = map_with(&["...", ".$.", "..."], vec![], vec![npc(0x108, 1, 1)]);
    let mut state = party_facing(&map, 1, 2, Direction::Up);

    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 1),
            reach: InteractReach::Adjacent,
        }]
    );
}

#[test]
fn the_counter_probe_wins_when_both_probes_would_hit() {
    // `Interaction_DoChecks` calls the special probe first, so the object
    // beyond the counter answers before the one standing on it.
    let map = map_with(
        &["...", ".$.", "..."],
        vec![],
        vec![npc(0x300, 1, 1), npc(0x301, 1, 0)],
    );
    let mut state = party_facing(&map, 1, 2, Direction::Up);

    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 1,
            cell: Cell::new(1, 0),
            reach: InteractReach::AcrossCounter,
        }],
        "the object across the counter is checked first"
    );
}

#[test]
fn a_counter_with_nothing_behind_it_reports_nothing() {
    let map = map_with(&["...", ".$.", "..."], vec![], vec![]);
    let mut state = party_facing(&map, 1, 2, Direction::Up);

    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::InteractNothing {
            facing: Direction::Up
        }]
    );
}

#[test]
fn the_counter_reach_stops_at_the_edge_of_a_bounded_map() {
    // Counter on the top row: there is no cell beyond it to probe.
    let map = map_with(&["$..", "...", "..."], vec![], vec![]);
    let mut state = party_facing(&map, 0, 1, Direction::Up);

    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::InteractNothing {
            facing: Direction::Up
        }],
        "probing past the edge must not panic or wrap"
    );
}

#[test]
fn the_ordinary_one_cell_reach_is_unchanged() {
    // The regression guard: no counter anywhere, adjacent NPC, Adjacent reach.
    let map = map_with(&["...", "...", "..."], vec![], vec![npc(0x400, 1, 0)]);
    let mut state = party_facing(&map, 1, 1, Direction::Up);

    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 0),
            reach: InteractReach::Adjacent,
        }]
    );
}

// ---------------------------------------------------------------------------
// Despawned NPCs keep their index
//
// The cartridge despawns by clearing the object's RAM slot and leaving the slot
// in place (`clr.w` + `trap #0`; `Field_RunObjects` then skips any slot whose
// id word is zero). Indices stay stable, which is what dialogue bindings and
// `Effect::Interact` depend on.
// ---------------------------------------------------------------------------

#[test]
fn an_inactive_npc_can_be_walked_through() {
    let mut map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 1)]);
    let mut state = party(&map, 0, 1);

    // Active: it blocks.
    assert!(walk_one_step(&mut state, &map, Direction::Right).is_empty());
    assert_eq!(state.cell(), Cell::new(0, 1));

    map.set_npc_active(0, false).expect("index 0 exists");
    assert!(map.is_walkable(Cell::new(1, 1)));
    assert!(map.npc_at(Cell::new(1, 1)).is_none());

    let mut state = party(&map, 0, 1);
    walk_one_step(&mut state, &map, Direction::Right);
    assert_eq!(state.cell(), Cell::new(1, 1), "the cell is now free");
}

#[test]
fn an_inactive_npc_cannot_be_reached_by_either_probe() {
    // Adjacent probe.
    let mut adjacent = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 0)]);
    adjacent.set_npc_active(0, false).unwrap();
    let mut state = party_facing(&adjacent, 1, 1, Direction::Up);
    assert_eq!(
        press_action(&mut state, &adjacent),
        vec![Effect::InteractNothing {
            facing: Direction::Up
        }]
    );

    // Counter-reaching probe: same object, two cells across a $C cell.
    let mut across = map_with(&["...", ".$.", "..."], vec![], vec![npc(0x108, 1, 0)]);
    across.set_npc_active(0, false).unwrap();
    let mut state = party_facing(&across, 1, 2, Direction::Up);
    assert_eq!(
        press_action(&mut state, &across),
        vec![Effect::InteractNothing {
            facing: Direction::Up
        }],
        "the counter probe must not reach a despawned object either"
    );
}

#[test]
fn reactivating_restores_blocking_and_talking() {
    let mut map = map_with(&["...", "...", "..."], vec![], vec![npc(10, 1, 0)]);
    map.set_npc_active(0, false).unwrap();
    map.set_npc_active(0, true).unwrap();

    assert!(!map.is_walkable(Cell::new(1, 0)), "blocks again");
    let mut state = party_facing(&map, 1, 1, Direction::Up);
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 0,
            cell: Cell::new(1, 0),
            reach: InteractReach::Adjacent,
        }],
        "and talks again"
    );
}

#[test]
fn despawning_does_not_renumber_the_objects_after_it() {
    // The whole point: index 2 is still index 2 once index 0 is gone.
    let mut map = map_with(
        &["....", "....", "...."],
        vec![],
        vec![npc(10, 0, 0), npc(11, 1, 0), npc(12, 2, 0)],
    );
    map.set_npc_active(0, false).unwrap();

    assert_eq!(map.npcs().len(), 3, "the slot stays in place");
    assert_eq!(map.npcs()[0].id, NpcId(10), "and keeps its identity");
    assert!(!map.npcs()[0].active);
    assert_eq!(map.npcs()[2].id, NpcId(12));

    // Talking to the survivor still reports index 2.
    let mut state = party_facing(&map, 2, 1, Direction::Up);
    assert_eq!(
        press_action(&mut state, &map),
        vec![Effect::Interact {
            npc_index: 2,
            cell: Cell::new(2, 0),
            reach: InteractReach::Adjacent,
        }]
    );
}

#[test]
fn set_npc_active_rejects_an_index_that_names_nothing() {
    let mut map = map_with(&["..", ".."], vec![], vec![npc(10, 0, 0)]);
    assert!(matches!(
        map.set_npc_active(1, false),
        Err(psiv_core::MapError::NpcIndexOutOfRange { index: 1, count: 1 })
    ));
    assert!(map.set_npc_active(0, false).is_ok());
}

#[test]
fn npcs_are_active_by_default() {
    let plain = npc(10, 0, 0);
    assert!(plain.active);
    assert!(!plain.with_active(false).active);
    assert!(plain.with_active(false).with_active(true).active);
}
