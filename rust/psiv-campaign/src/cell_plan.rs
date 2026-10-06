//! Cell-level planner: the pad directions that walk the party across one map.
//!
//! Everything here asks `psiv_core::FieldMap` and re-implements nothing:
//! walkability is [`FieldMap::is_walkable`], adjacency is
//! [`FieldMap::neighbor`] (so an overworld torus wraps), and whether a step
//! fires a warp is the field engine's own rule, read through
//! [`FieldMap::warp_at`] and [`FieldMap::collision_at`].
//!
//! # The firing rule
//!
//! A step from `at` into `next` fires a warp exactly when the field tick would
//! (`psiv-core/src/field.rs`, the landing branch of `FieldState::tick`):
//!
//! * `next` is collision type 1 (map change) and `at` is not: the first
//!   [`WarpTrigger::MapChange`] warp covering `next` fires. Walking along a
//!   doorway several cells wide fires once, not per cell.
//! * `next` is any other walkable cell: the first [`WarpTrigger::NormalGround`]
//!   warp covering `next` fires (map edges, cave mouths, doormats).
//!
//! A step that fires a warp leaves the map, so it is never traversed *through*.
//! It is a terminal. During a leg only the intended terminal may be taken;
//! every other firing step is simply not an edge (the generalised rule of the
//! single-map `first_step` in `psiv-runtime/examples/support/mod.rs`, which
//! avoided every cell of another warp's rectangle).
//!
//! # Mounted
//!
//! A party in a vehicle (the Land Rover from the Machine Center on) moves on a
//! 32-pixel grid: one press is a step of two cells, allowed when the whole
//! four-cell footprint ahead is a terrain the vehicle crosses
//! (`psiv_core::can_enter`), and the warp rule reads the footprint's
//! collision (`psiv_core::standing_collision`), exactly as
//! `VehicleState::tick` does. [`Mover::Vehicle`] plans with those rules, so the
//! sand a Land Rover crosses and a person cannot is part of the graph.
//!
//! # Determinism
//!
//! Breadth-first search, a FIFO frontier, and neighbours expanded in
//! [`Direction::ALL`] order (Up, Down, Left, Right). The first time a cell or
//! a warp is reached fixes its parent, so ties between equally short paths
//! always resolve the same way: the path found first by that search order.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use psiv_core::{
    Cell, CellRect, CollisionType, Direction, FieldMap, WarpTrigger, can_enter, standing_collision,
};

/// How the party moves across a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mover {
    /// On foot: one cell a step, over walkable collision.
    Foot,
    /// Mounted: the vehicle with this `Vehicle_Index`, two cells a step.
    Vehicle(u16),
}

impl Mover {
    /// The cell a press of `direction` lands on from `at`, when the engine
    /// would move that way (the landing is not yet checked for a warp).
    #[must_use]
    pub fn landing(self, map: &FieldMap, at: Cell, direction: Direction) -> Option<Cell> {
        match self {
            Mover::Foot => map
                .neighbor(at, direction)
                .filter(|next| map.is_walkable(*next)),
            Mover::Vehicle(index) => {
                let anchor = map.neighbor(at, direction)?;
                let next = map.neighbor(anchor, direction)?;
                can_enter(index, map, at, direction).then_some(next)
            }
        }
    }

    /// The warp, if any, that landing on `next` from `at` fires.
    #[must_use]
    pub fn firing_warp(self, map: &FieldMap, at: Cell, next: Cell) -> Option<usize> {
        match self {
            Mover::Foot => firing_warp(map, at, next),
            Mover::Vehicle(_) => vehicle_firing_warp(map, at, next),
        }
    }
}

/// Where a leg ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// Stand on this cell without firing any warp.
    Cell(Cell),
    /// Stand on the nearest cell of this rectangle without firing any warp.
    Rect(CellRect),
    /// Take the warp at this index in [`FieldMap::warps`]: the leg's last step
    /// is the one that fires it.
    Warp(usize),
}

/// A planned walk across one map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellPlan {
    /// One pad direction per step, in order.
    pub steps: Vec<Direction>,
    /// Where the party stands after the last step. For a [`Goal::Warp`] leg
    /// this is the cell the last step moves onto, which fires the warp.
    pub end: Cell,
    /// The warp the last step fires, for a [`Goal::Warp`] leg.
    pub fires: Option<usize>,
}

/// Why a cell plan could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellPlanError {
    /// The start cell is not on the map.
    StartOffMap(Cell),
    /// The goal cell is not walkable or not on the map.
    GoalNotWalkable(Cell),
    /// A warp goal named an index the map does not have.
    NoSuchWarp(usize),
    /// No walk reaches the goal without firing a different warp.
    Unreachable,
}

impl fmt::Display for CellPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellPlanError::StartOffMap(c) => {
                write!(f, "start cell ({},{}) is off the map", c.x, c.y)
            }
            CellPlanError::GoalNotWalkable(c) => write!(
                f,
                "goal cell ({},{}) is off the map or not walkable",
                c.x, c.y
            ),
            CellPlanError::NoSuchWarp(i) => write!(f, "the map has no warp index {i}"),
            CellPlanError::Unreachable => {
                write!(f, "no walk reaches the goal without firing another warp")
            }
        }
    }
}

impl std::error::Error for CellPlanError {}

/// The warp, if any, that stepping `at` -> `next` fires. `next` must already be
/// walkable. Returns the index into [`FieldMap::warps`].
#[must_use]
pub fn firing_warp(map: &FieldMap, at: Cell, next: Cell) -> Option<usize> {
    let trigger = if map
        .collision_at(next)
        .is_some_and(CollisionType::is_map_change)
    {
        // A doorway fires once on entering type 1, never when already on it.
        if map
            .collision_at(at)
            .is_some_and(CollisionType::is_map_change)
        {
            return None;
        }
        WarpTrigger::MapChange
    } else {
        WarpTrigger::NormalGround
    };
    let found = map.warp_at(next, trigger)?;
    map.warps().iter().position(|w| std::ptr::eq(w, found))
}

/// `VehicleState::transition_effect`: a landing whose footprint reads collision
/// 1 fires the map-change warp unless the footprint already read 1 at the last
/// stop; any other landing that does not block fires the ground warp.
#[must_use]
pub fn vehicle_firing_warp(map: &FieldMap, at: Cell, next: Cell) -> Option<usize> {
    let raw = standing_collision(map, next);
    let trigger = if raw == 1 {
        if standing_collision(map, at) == 1 {
            return None;
        }
        WarpTrigger::MapChange
    } else if CollisionType::from_raw(raw).is_blocking() {
        return None;
    } else {
        WarpTrigger::NormalGround
    };
    let found = map.warp_at(next, trigger)?;
    map.warps().iter().position(|w| std::ptr::eq(w, found))
}

/// One breadth-first flood from a start cell: the shortest walk to every
/// reachable cell and to every warp it can fire, never passing through a
/// firing step.
#[derive(Debug, Clone)]
pub struct Flood {
    start: Cell,
    /// Cell -> (parent, step that reached it). The start has no entry.
    parent: HashMap<Cell, (Cell, Direction)>,
    /// Cells in discovery order (start first).
    order: Vec<Cell>,
    /// Warp index -> (cell stepped from, direction, cell stepped onto); the
    /// first (shortest) firing step found.
    terminals: BTreeMap<usize, (Cell, Direction, Cell)>,
}

impl Flood {
    /// Floods `map` from `start`.
    ///
    /// # Errors
    ///
    /// [`CellPlanError::StartOffMap`] when `start` is not a cell of `map`.
    pub fn new(map: &FieldMap, start: Cell) -> Result<Flood, CellPlanError> {
        Flood::for_mover(map, start, Mover::Foot)
    }

    /// Floods `map` from `start` for a party that moves as `mover`.
    ///
    /// # Errors
    ///
    /// [`CellPlanError::StartOffMap`] when `start` is not a cell of `map`.
    pub fn for_mover(map: &FieldMap, start: Cell, mover: Mover) -> Result<Flood, CellPlanError> {
        let start = map
            .normalize(start)
            .ok_or(CellPlanError::StartOffMap(start))?;
        let mut flood = Flood {
            start,
            parent: HashMap::new(),
            order: vec![start],
            terminals: BTreeMap::new(),
        };
        let mut queue = VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            for direction in Direction::ALL {
                let Some(next) = mover.landing(map, at, direction) else {
                    continue;
                };
                // Firing depends on where the step comes from, not only on the
                // cell, so a cell already reached (the start included) is
                // still checked: a doorway entered from a doorway does not
                // fire, but the same doorway entered from open ground does.
                if let Some(warp) = mover.firing_warp(map, at, next) {
                    flood.terminals.entry(warp).or_insert((at, direction, next));
                    continue;
                }
                if next == start || flood.parent.contains_key(&next) {
                    continue;
                }
                flood.parent.insert(next, (at, direction));
                flood.order.push(next);
                queue.push_back(next);
            }
        }
        Ok(flood)
    }

    /// The flood's start cell.
    #[must_use]
    pub const fn start(&self) -> Cell {
        self.start
    }

    /// Whether `cell` is reachable without firing a warp (the start counts).
    #[must_use]
    pub fn reaches(&self, cell: Cell) -> bool {
        cell == self.start || self.parent.contains_key(&cell)
    }

    /// Cells reachable without firing a warp, in discovery order.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.order
    }

    /// The warps the flood can fire, ascending by index.
    pub fn warps(&self) -> impl Iterator<Item = usize> + '_ {
        self.terminals.keys().copied()
    }

    fn path_to(&self, cell: Cell) -> Option<Vec<Direction>> {
        if !self.reaches(cell) {
            return None;
        }
        let mut steps = Vec::new();
        let mut at = cell;
        while at != self.start {
            let (from, direction) = self.parent[&at];
            steps.push(direction);
            at = from;
        }
        steps.reverse();
        Some(steps)
    }

    /// The plan that ends standing on `cell`.
    #[must_use]
    pub fn cell_plan(&self, cell: Cell) -> Option<CellPlan> {
        Some(CellPlan {
            steps: self.path_to(cell)?,
            end: cell,
            fires: None,
        })
    }

    /// The plan whose last step fires `warp`.
    #[must_use]
    pub fn warp_plan(&self, warp: usize) -> Option<CellPlan> {
        let &(from, direction, onto) = self.terminals.get(&warp)?;
        let mut steps = self.path_to(from)?;
        steps.push(direction);
        Some(CellPlan {
            steps,
            end: onto,
            fires: Some(warp),
        })
    }

    /// The plan whose last step lands on `cell` and starts the scene there.
    ///
    /// A warp footprint is the first kind: the step fires a warp, and a map
    /// trigger on it wins because `RunEvents` runs before `RunMapTransitions`
    /// (`ps4.asm:116768-116773`). Any other cell a trigger list names (the
    /// carnivorous trees' corridor, `RunEvent_CarnivorousTrees`,
    /// `ps4.asm:115977`) is the second: the ordinary walk that ends on it. A
    /// cell the party already stands on has no step onto it.
    #[must_use]
    pub fn onto_plan(&self, map: &FieldMap, mover: Mover, cell: Cell) -> Option<CellPlan> {
        let cell = map.normalize(cell)?;
        for &at in &self.order {
            for direction in Direction::ALL {
                if mover.landing(map, at, direction) != Some(cell) {
                    continue;
                }
                let Some(warp) = mover.firing_warp(map, at, cell) else {
                    continue;
                };
                let mut steps = self.path_to(at)?;
                steps.push(direction);
                return Some(CellPlan {
                    steps,
                    end: cell,
                    fires: Some(warp),
                });
            }
        }
        if cell == self.start {
            return None;
        }
        self.cell_plan(cell)
    }

    /// The plan to the first-discovered cell of `rect`.
    #[must_use]
    pub fn rect_plan(&self, map: &FieldMap, rect: CellRect) -> Option<CellPlan> {
        let cell = self
            .order
            .iter()
            .copied()
            .find(|c| map.rect_contains(rect, *c))?;
        self.cell_plan(cell)
    }
}

/// Plans a walk on `map` from `start` to `goal`.
///
/// # Errors
///
/// [`CellPlanError`]: the start is off the map, the goal is not a walkable
/// cell (or names a warp the map lacks), or no walk reaches it without firing
/// another warp.
pub fn plan_cells(map: &FieldMap, start: Cell, goal: Goal) -> Result<CellPlan, CellPlanError> {
    plan_cells_for(map, Mover::Foot, start, goal)
}

/// [`plan_cells`] for a party that moves as `mover`.
///
/// # Errors
///
/// As [`plan_cells`]; a mounted party's goal cell must be one its flood
/// reaches, which a cell off the vehicle's two-cell grid is not.
pub fn plan_cells_for(
    map: &FieldMap,
    mover: Mover,
    start: Cell,
    goal: Goal,
) -> Result<CellPlan, CellPlanError> {
    let flood = Flood::for_mover(map, start, mover)?;
    match goal {
        Goal::Cell(cell) => {
            let cell = map
                .normalize(cell)
                .filter(|c| mover != Mover::Foot || map.is_walkable(*c))
                .ok_or(CellPlanError::GoalNotWalkable(cell))?;
            flood.cell_plan(cell).ok_or(CellPlanError::Unreachable)
        }
        Goal::Rect(rect) => flood.rect_plan(map, rect).ok_or(CellPlanError::Unreachable),
        Goal::Warp(index) => {
            if index >= map.warps().len() {
                return Err(CellPlanError::NoSuchWarp(index));
            }
            flood.warp_plan(index).ok_or(CellPlanError::Unreachable)
        }
    }
}
