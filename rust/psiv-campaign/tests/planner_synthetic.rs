//! The cell planner on hand-built maps: no pack needed, so these always run.

use psiv_campaign::{CellPlanError, Flood, Goal, plan_cells};
use psiv_core::{
    Cell, CellRect, CollisionGrid, Direction, FieldMap, MapId, Npc, NpcId, Warp, WarpTrigger,
};

fn grid(width: u16, rows: &[&str]) -> CollisionGrid {
    let cells: Vec<u8> = rows
        .iter()
        .flat_map(|r| r.chars().map(|c| c.to_digit(16).unwrap() as u8))
        .collect();
    CollisionGrid::new(width, u16::try_from(rows.len()).unwrap(), cells).unwrap()
}

fn ground_warp(rect: CellRect, to: u16) -> Warp {
    Warp {
        source: rect,
        trigger: WarpTrigger::NormalGround,
        target_map: MapId(to),
        target_cell: Cell::new(0, 0),
        facing: Direction::Down,
    }
}

fn door_warp(rect: CellRect, to: u16) -> Warp {
    Warp {
        trigger: WarpTrigger::MapChange,
        ..ground_warp(rect, to)
    }
}

fn map(rows: &[&str], warps: Vec<Warp>, npcs: Vec<Npc>) -> FieldMap {
    let width = u16::try_from(rows[0].len()).unwrap();
    FieldMap::new(MapId(7), grid(width, rows), warps, npcs).unwrap()
}

#[test]
fn ties_break_up_down_left_right_and_are_repeatable() {
    let open = map(&["000", "000", "000"], vec![], vec![]);
    let plan = plan_cells(&open, Cell::new(0, 0), Goal::Cell(Cell::new(2, 2))).unwrap();
    // Neighbours expand Up, Down, Left, Right; the first parent to reach a
    // cell keeps it, so the equally short paths resolve to Down, Down, Right,
    // Right.
    assert_eq!(
        plan.steps,
        [
            Direction::Down,
            Direction::Down,
            Direction::Right,
            Direction::Right
        ]
    );
    for _ in 0..3 {
        assert_eq!(
            plan_cells(&open, Cell::new(0, 0), Goal::Cell(Cell::new(2, 2))).unwrap(),
            plan
        );
    }
    let empty = plan_cells(&open, Cell::new(1, 1), Goal::Cell(Cell::new(1, 1))).unwrap();
    assert!(empty.steps.is_empty());
}

#[test]
fn walls_and_npcs_block_and_the_goal_must_be_walkable() {
    let rows = ["0800", "0800", "0000"];
    let m = map(&rows, vec![], vec![]);
    let plan = plan_cells(&m, Cell::new(0, 0), Goal::Cell(Cell::new(2, 0))).unwrap();
    assert_eq!(plan.steps.len(), 6, "down, down, right x2, up, up");
    assert_eq!(
        plan_cells(&m, Cell::new(0, 0), Goal::Cell(Cell::new(1, 0))),
        Err(CellPlanError::GoalNotWalkable(Cell::new(1, 0)))
    );
    // An NPC standing in the corridor closes it.
    let blocker = Npc::new(NpcId(1), Cell::new(1, 2), Direction::Down);
    let shut = map(&rows, vec![], vec![blocker]);
    assert_eq!(
        plan_cells(&shut, Cell::new(0, 0), Goal::Cell(Cell::new(2, 0))),
        Err(CellPlanError::Unreachable)
    );
}

#[test]
fn off_map_starts_and_missing_warps_are_errors() {
    let m = map(&["00", "00"], vec![], vec![]);
    assert_eq!(
        plan_cells(&m, Cell::new(5, 5), Goal::Cell(Cell::new(0, 0))),
        Err(CellPlanError::StartOffMap(Cell::new(5, 5)))
    );
    assert_eq!(
        plan_cells(&m, Cell::new(0, 0), Goal::Warp(3)),
        Err(CellPlanError::NoSuchWarp(3))
    );
}

/// A warp that must not be crossed mid-leg is avoided: with room to go round,
/// the leg detours; with no room, the target is unreachable.
#[test]
fn another_warp_is_never_crossed_during_a_leg() {
    let warp = ground_warp(CellRect::single(Cell::new(2, 0)), 9);
    // Three rows: the straight line along row 0 crosses the warp at (2,0).
    let wide = map(&["00000", "00000", "00000"], vec![warp], vec![]);
    let plan = plan_cells(&wide, Cell::new(0, 0), Goal::Cell(Cell::new(4, 0))).unwrap();
    assert_eq!(
        plan.steps.len(),
        6,
        "a detour round the warp, not the 4-step line"
    );
    let mut at = Cell::new(0, 0);
    for step in &plan.steps {
        at = wide.neighbor(at, *step).unwrap();
        assert_ne!(at, Cell::new(2, 0), "stepped on the other warp");
    }
    // One row: no way round.
    let corridor = map(&["00000"], vec![warp], vec![]);
    assert_eq!(
        plan_cells(&corridor, Cell::new(0, 0), Goal::Cell(Cell::new(4, 0))),
        Err(CellPlanError::Unreachable)
    );
    // Asking for that warp takes the straight line and fires it last.
    let take = plan_cells(&corridor, Cell::new(0, 0), Goal::Warp(0)).unwrap();
    assert_eq!(take.steps, [Direction::Right, Direction::Right]);
    assert_eq!(take.fires, Some(0));
    assert_eq!(take.end, Cell::new(2, 0));
}

/// The intended warp is the only one a warp leg may take.
#[test]
fn a_warp_leg_fires_only_its_own_warp() {
    let near = ground_warp(CellRect::single(Cell::new(1, 0)), 8);
    let far = ground_warp(CellRect::single(Cell::new(3, 0)), 9);
    let m = map(&["00000"], vec![near, far], vec![]);
    let flood = Flood::new(&m, Cell::new(0, 0)).unwrap();
    assert_eq!(flood.warps().collect::<Vec<_>>(), [0], "far is behind near");
    assert!(flood.warp_plan(1).is_none());
    assert_eq!(
        plan_cells(&m, Cell::new(0, 0), Goal::Warp(1)),
        Err(CellPlanError::Unreachable)
    );
}

/// A doorway fires once on entering type 1 from open ground, never while
/// moving from one type-1 cell to the next, and fires again when re-entered.
#[test]
fn a_doorway_fires_on_entry_from_open_ground_only() {
    // Two type-1 cells at (1,0) and (1,1) inside a 3x2 map.
    let rows = ["010", "010"];
    let door = door_warp(CellRect::new(1, 0, 1, 2), 5);
    let m = map(&rows, vec![door], vec![]);
    // From open ground it fires on the first step in.
    let from_ground = plan_cells(&m, Cell::new(0, 0), Goal::Warp(0)).unwrap();
    assert_eq!(from_ground.steps, [Direction::Right]);
    // Landing inside the doorway (as a party does after arriving by one):
    // moving along the doorway does not fire; stepping off and back does.
    let from_inside = plan_cells(&m, Cell::new(1, 0), Goal::Warp(0)).unwrap();
    assert_eq!(from_inside.steps.len(), 2, "off the doorway and back on");
    assert_eq!(
        from_inside.steps[0],
        Direction::Left,
        "Up, Down, Left, Right order: Left comes before Right"
    );
    assert_eq!(from_inside.steps[1], Direction::Right);
    // Walking along the doorway is a plain walk to the other type-1 cell.
    let along = plan_cells(&m, Cell::new(1, 0), Goal::Cell(Cell::new(1, 1))).unwrap();
    assert_eq!(along.steps, [Direction::Down]);
}

#[test]
fn a_rect_goal_stops_on_its_nearest_cell() {
    let m = map(&["00000", "00000"], vec![], vec![]);
    let plan = plan_cells(&m, Cell::new(0, 0), Goal::Rect(CellRect::new(3, 0, 2, 2))).unwrap();
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.end, Cell::new(3, 0));
}
