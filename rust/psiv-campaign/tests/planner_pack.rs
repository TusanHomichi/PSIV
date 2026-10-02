//! The planners against the real pack: the independent Aiedo oracle, a town
//! to an interior, a torus wrap, flag-gated and unreachable targets.

mod common;

use common::pack;
use psiv_campaign::{Goal, MapGraph, PlanError, Position, Target, plan_cells};
use psiv_core::{Cell, Direction, Flag};

fn at(map: u16, x: u16, y: u16) -> Position {
    Position {
        map,
        cell: Cell::new(x, y),
    }
}

/// `docs/campaign/AIEDO.md` section 1 derived this route with `oracle/route.py`,
/// a different tool: one warp (record `0x100706`, transition index 7), arrival
/// at standing (47,83) facing up, 67 steps (13 up, 20 left, then the
/// staircase down the cliff band, 8 left to the doorway).
#[test]
fn the_aiedo_route_matches_the_independent_oracle() {
    let Some(pack) = pack() else { return };
    let mut graph = MapGraph::new(&pack.data, &[]).unwrap();
    let plan = graph.plan(at(0, 84, 64), Target::Map(0x54)).unwrap();
    assert_eq!(plan.legs.len(), 1, "exactly one warp");
    let leg = &plan.legs[0];
    assert_eq!(leg.hop.record_offset, "0x100706");
    assert_eq!(leg.hop.pack_index, 7, "transition index 7");
    assert_eq!(leg.hop.target_map, 0x54);
    assert_eq!(leg.hop.arrival, Cell::new(47, 83));
    assert_eq!(leg.hop.facing, Direction::Up);
    assert_eq!(leg.steps.len(), 67);
    assert_eq!(plan.total_steps(), 67);
    assert_eq!(plan.to, at(0x54, 47, 83));
    let up = leg.steps.iter().filter(|d| **d == Direction::Up).count();
    let left = leg.steps.iter().filter(|d| **d == Direction::Left).count();
    let down = leg.steps.iter().filter(|d| **d == Direction::Down).count();
    assert_eq!(
        (up, down, left),
        (13, 7, 47),
        "the ledger's twelve-leg table"
    );
    assert_eq!(leg.steps[..13], [Direction::Up; 13]);
}

#[test]
fn a_plan_is_a_pure_function_of_the_pack_and_flags() {
    let Some(pack) = pack() else { return };
    let first = MapGraph::new(&pack.data, &[])
        .unwrap()
        .plan(at(0, 84, 64), Target::Map(0x54))
        .unwrap();
    let second = MapGraph::new(&pack.data, &[])
        .unwrap()
        .plan(at(0, 84, 64), Target::Map(0x54))
        .unwrap();
    assert_eq!(first, second);
}

/// Piata town (arrival from the academy gate) to the Academy ground floor,
/// through the academy grounds.
#[test]
fn piata_town_reaches_the_academy_interior() {
    let Some(pack) = pack() else { return };
    let mut graph = MapGraph::new(&pack.data, &[]).unwrap();
    let plan = graph.plan(at(0x10, 31, 7), Target::Map(0x13)).unwrap();
    let maps: Vec<u16> = plan.legs.iter().map(|l| l.hop.target_map).collect();
    assert_eq!(maps, [0x11, 0x13], "town -> grounds -> PiataAcademy_F1");
    assert_eq!(plan.legs[0].hop.pack_index, 1);
    assert_eq!(plan.legs[1].hop.pack_index, 2);
    assert_eq!(plan.to, at(0x13, 17, 19));
    // The ledger-checked native legs: (31,19) -> (16,19) over the grounds.
    assert_eq!(plan.legs[1].steps.len(), 17);
}

/// A target cell on a later map: the plan ends with a walk, not a warp.
#[test]
fn a_cell_target_ends_with_a_final_walk() {
    let Some(pack) = pack() else { return };
    let mut graph = MapGraph::new(&pack.data, &[]).unwrap();
    let plan = graph
        .plan(
            at(0, 84, 64),
            Target::Cell {
                map: 0x54,
                cell: Cell::new(47, 80),
            },
        )
        .unwrap();
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.tail, [Direction::Up; 3]);
    assert_eq!(plan.to, at(0x54, 47, 80));
    assert_eq!(plan.total_steps(), 70);
}

/// Dezolis (map 1) is a torus too; rows 26..=29 are open across its seam, so
/// three steps left from column 1 reach column 254, not 253 steps right.
#[test]
fn a_walk_wraps_across_an_overworld_seam() {
    let Some(pack) = pack() else { return };
    let mut graph = MapGraph::new(&pack.data, &[]).unwrap();
    let map = graph.field_map(1).unwrap();
    assert!(map.wraps());
    let plan = plan_cells(&map, Cell::new(1, 26), Goal::Cell(Cell::new(254, 26))).unwrap();
    assert_eq!(plan.steps, [Direction::Left; 3]);
    assert_eq!(plan.end, Cell::new(254, 26));
    let down = plan_cells(&map, Cell::new(254, 27), Goal::Cell(Cell::new(1, 27))).unwrap();
    assert_eq!(down.steps, [Direction::Right; 3]);
}

/// The Rika bridge opens on `EventFlag_RikaJoined` ($35): the north bank is cut
/// off from Zema's side of the river until the route claims the flag.
#[test]
fn the_bridge_patch_is_flag_gated() {
    let Some(pack) = pack() else { return };
    let target = Target::Cell {
        map: 0,
        cell: Cell::new(84, 70),
    };
    let mut closed = MapGraph::new(&pack.data, &[]).unwrap();
    let error = closed.plan(at(0, 84, 64), target).unwrap_err();
    assert!(matches!(error, PlanError::Unreachable { .. }), "{error}");
    let mut open = MapGraph::new(&pack.data, &[Flag::event(0x35)]).unwrap();
    let plan = open.plan(at(0, 84, 64), target).unwrap();
    assert_eq!(plan.tail, [Direction::Down; 6]);
}

/// Negative controls: a map no walk reaches, a wall as a goal, an unknown id.
#[test]
fn unreachable_targets_are_errors() {
    let Some(pack) = pack() else { return };
    let mut graph = MapGraph::new(&pack.data, &[]).unwrap();
    // Dezolis has no walkable chain from the north bank.
    let error = graph.plan(at(0, 84, 64), Target::Map(1)).unwrap_err();
    assert!(matches!(error, PlanError::Unreachable { .. }), "{error}");
    assert!(error.to_string().contains("no walkable chain"), "{error}");
    // A wall cell of a reachable map.
    let error = graph
        .plan(
            at(0, 84, 64),
            Target::Cell {
                map: 0,
                cell: Cell::new(0, 0),
            },
        )
        .unwrap_err();
    assert!(matches!(error, PlanError::Unreachable { .. }), "{error}");
    // A map the pack does not hold.
    let error = graph.plan(at(0, 84, 64), Target::Map(0x7FF)).unwrap_err();
    assert!(matches!(error, PlanError::UnknownMap(0x7FF)), "{error}");
}

/// Zema's overworld exit lands on a map-change cell of its own entrance
/// footprint (99,83); leaving and re-entering must fire the warp. The ledgers
/// recorded this as a driver workaround ("leave the footprint first").
#[test]
fn a_party_landing_inside_a_doorway_can_take_it_again() {
    let Some(pack) = pack() else { return };
    let mut graph = MapGraph::new(&pack.data, &[]).unwrap();
    let leave = graph.plan(at(0x24, 31, 50), Target::Map(0)).unwrap();
    assert_eq!(
        leave.to,
        at(0, 99, 83),
        "ledger: the party exits to (99,83)"
    );
    let plan = graph.plan(leave.to, Target::Map(0x24)).unwrap();
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.legs[0].steps, [Direction::Down, Direction::Up]);
    assert_eq!(plan.legs[0].hop.pack_index, 2);
}
