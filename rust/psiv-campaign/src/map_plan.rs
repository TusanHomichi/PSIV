//! Map-level planner: the chain of warps from a (map, cell) to a target.
//!
//! The graph's nodes are (map, standing cell) pairs: where the party stands
//! after arriving by a warp. A node's edges are the warps its cell flood
//! ([`Flood`]) can actually fire, weighted by the walk to them, so the plan
//! never takes a warp it cannot walk to. Overworld torus topology is
//! `FieldMap`'s own (`psiv-runtime`'s bridge builds maps 0 and 1 as tori).
//!
//! Search is Dijkstra on step counts. Ties are broken by insertion order of the
//! frontier (first relaxed, first popped), and within a map by
//! [`crate::cell_plan`]'s documented search order, so a plan is a pure function
//! of the pack and the flags.
//!
//! # Flags
//!
//! Flag-gated map patches (`MapDataManager` writes, overworld page hooks) change
//! collision. A [`MapGraph`] takes the flag set as input and builds every map
//! through `psiv_runtime::evaluate_map_effects` and
//! `psiv_runtime::field_map_entered`, so a bridge that opens when
//! `EventFlag_RikaJoined` is set is open exactly when the caller says the flag
//! is. Flags are static for one plan; a route that changes flags mid-way plans
//! each objective with the flags it claims at that point.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;
use std::rc::Rc;

use psiv_core::{Cell, Direction, FieldMap, Flag, GameState, MapError};
use psiv_data::{GameData, MapRecord};
use psiv_runtime::{BridgeError, evaluate_map_effects, field_map_entered};

use crate::cell_plan::{CellPlanError, Flood, Mover};

/// A map and the standing cell on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// Map id.
    pub map: u16,
    /// Standing cell.
    pub cell: Cell,
}

/// What a plan is trying to reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Arrive on this map, anywhere (the cell the last warp lands on).
    Map(u16),
    /// Stand on this cell of this map.
    Cell {
        /// Map id.
        map: u16,
        /// Standing cell.
        cell: Cell,
    },
}

/// One warp the plan takes, with its provenance in the pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    /// Index into `FieldMap::warps` of the source map.
    pub warp_index: usize,
    /// The warp's own `index` in the pack record's `warps` list
    /// (`transition index` in the ledgers; differs from `warp_index` only when
    /// the record holds dead warps with no trigger area).
    pub pack_index: u32,
    /// The ROM offset of the transition record (`0x100706` ...).
    pub record_offset: String,
    /// Map the warp leads to.
    pub target_map: u16,
    /// Standing cell the party arrives on.
    pub arrival: Cell,
    /// Facing on arrival.
    pub facing: Direction,
}

/// A walk across one map ending in a warp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leg {
    /// The map walked.
    pub map: u16,
    /// Where the leg starts.
    pub from: Cell,
    /// Pad directions; the last one fires the warp.
    pub steps: Vec<Direction>,
    /// The warp fired.
    pub hop: Hop,
}

/// A full plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Where the plan starts.
    pub from: Position,
    /// The warps, in order.
    pub legs: Vec<Leg>,
    /// The final walk on the arrival map (empty for [`Target::Map`]).
    pub tail: Vec<Direction>,
    /// Where the plan ends.
    pub to: Position,
}

impl Plan {
    /// Total pad steps across every leg and the tail.
    #[must_use]
    pub fn total_steps(&self) -> usize {
        self.legs.iter().map(|l| l.steps.len()).sum::<usize>() + self.tail.len()
    }
}

/// Why a plan could not be made.
#[derive(Debug)]
pub enum PlanError {
    /// A map id is not in the pack.
    UnknownMap(u16),
    /// A map record could not be turned into an engine map.
    Bridge(u16, BridgeError),
    /// A flag id is out of range for its bank.
    Flag(MapError),
    /// The cell planner rejected a leg.
    Cells(u16, CellPlanError),
    /// No chain of warps reaches the target.
    Unreachable {
        /// Where the search started.
        from: Position,
        /// What it was after.
        target: Target,
        /// How many (map, cell) nodes it explored.
        explored: usize,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::UnknownMap(m) => write!(f, "map {m:#05x} is not in the pack"),
            PlanError::Bridge(m, e) => write!(f, "map {m:#05x}: {e}"),
            PlanError::Flag(e) => write!(f, "{e}"),
            PlanError::Cells(m, e) => write!(f, "map {m:#05x}: {e}"),
            PlanError::Unreachable {
                from,
                target,
                explored,
            } => {
                let to = match target {
                    Target::Map(m) => format!("map {m:#05x}"),
                    Target::Cell { map, cell } => {
                        format!("map {map:#05x} cell ({},{})", cell.x, cell.y)
                    }
                };
                write!(
                    f,
                    "no walkable chain of warps from map {:#05x} ({},{}) reaches {to} \
                     ({explored} positions explored)",
                    from.map, from.cell.x, from.cell.y
                )
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// The pack's maps as an engine graph, for one flag set.
pub struct MapGraph<'a> {
    data: &'a GameData,
    base: GameState,
    maps: HashMap<u16, Rc<FieldMap>>,
    floods: HashMap<Position, Rc<Flood>>,
    opened: HashMap<u16, Vec<Cell>>,
    mover: Mover,
}

impl<'a> MapGraph<'a> {
    /// A graph over `data` with `flags` set.
    ///
    /// # Errors
    ///
    /// [`PlanError::Flag`] when a flag id is outside its bank.
    pub fn new(data: &'a GameData, flags: &[Flag]) -> Result<MapGraph<'a>, PlanError> {
        let mut base = GameState::new();
        for flag in flags {
            base.set(*flag).map_err(PlanError::Flag)?;
        }
        Ok(MapGraph {
            data,
            base,
            maps: HashMap::new(),
            floods: HashMap::new(),
            opened: HashMap::new(),
            mover: Mover::Foot,
        })
    }

    /// Plans every walk of this graph for a party that moves as `mover`: a
    /// mounted party stays mounted across the warps of a chain, as the engine
    /// keeps `Vehicle_Index` through a map change.
    #[must_use]
    pub fn with_mover(mut self, mover: Mover) -> MapGraph<'a> {
        self.mover = mover;
        self.floods.clear();
        self
    }

    /// Declares `cells` of `map` walkable (collision 0, and free of any object
    /// standing on them), as a door or elevator or a scene that moves objects
    /// aside, which a route opens by interaction, would leave them. Replaces any earlier
    /// declaration for `map` and drops the plans cached for it.
    pub fn open_cells(&mut self, map: u16, cells: Vec<Cell>) {
        self.maps.remove(&map);
        self.floods.retain(|at, _| at.map != map);
        if cells.is_empty() {
            self.opened.remove(&map);
        } else {
            self.opened.insert(map, cells);
        }
    }

    /// The engine map for `map`, built with this graph's flags.
    ///
    /// # Errors
    ///
    /// [`PlanError::UnknownMap`] or [`PlanError::Bridge`].
    pub fn field_map(&mut self, map: u16) -> Result<Rc<FieldMap>, PlanError> {
        if let Some(found) = self.maps.get(&map) {
            return Ok(Rc::clone(found));
        }
        let record = self
            .data
            .map(psiv_data::MapId(map))
            .ok_or(PlanError::UnknownMap(map))?;
        let built = Rc::new(self.build(record).map_err(|e| PlanError::Bridge(map, e))?);
        self.maps.insert(map, Rc::clone(&built));
        Ok(built)
    }

    /// The pack record for `map`.
    #[must_use]
    pub fn record(&self, map: u16) -> Option<&'a MapRecord> {
        self.data.map(psiv_data::MapId(map))
    }

    fn build(&self, record: &MapRecord) -> Result<FieldMap, BridgeError> {
        // Each map sees the flags as they stand at load; `flag_clear` writes
        // mutate the state the walk sees, so evaluate on a private copy.
        let mut game = self.base.clone();
        let mut outcome = evaluate_map_effects(record, &mut game);
        let opened = self.opened.get(&record.id.0).map_or(&[][..], Vec::as_slice);
        for cell in opened {
            outcome
                .cell_patches
                .push((u32::from(cell.x), u32::from(cell.y), 0));
        }
        let mut map = field_map_entered(record, &outcome, &game)?;
        // An opened cell is one the party can stand on, so an object the scene
        // moves off it (Tyler's grave blocks) no longer occupies it either.
        for cell in opened {
            while let Some(index) = map
                .npcs()
                .iter()
                .position(|npc| npc.active && npc.cell == *cell)
            {
                let _ = map.set_npc_active(index, false);
            }
        }
        Ok(map)
    }

    fn flood(&mut self, at: Position) -> Result<Rc<Flood>, PlanError> {
        if let Some(found) = self.floods.get(&at) {
            return Ok(Rc::clone(found));
        }
        let map = self.field_map(at.map)?;
        let flood = Rc::new(
            Flood::for_mover(&map, at.cell, self.mover).map_err(|e| PlanError::Cells(at.map, e))?,
        );
        self.floods.insert(at, Rc::clone(&flood));
        Ok(flood)
    }

    /// The leg that walks from `from` and fires the warp whose record index
    /// in the pack is `pack_index` (the ledgers' "transition index").
    ///
    /// # Errors
    ///
    /// [`PlanError::Cells`] when the map has no such warp with a trigger area
    /// or the walk cannot reach it without firing another; [`PlanError`]
    /// for an unknown map.
    pub fn leg_via_warp(&mut self, from: Position, pack_index: u32) -> Result<Leg, PlanError> {
        let record = self
            .record(from.map)
            .ok_or(PlanError::UnknownMap(from.map))?;
        let warp = record
            .warps
            .iter()
            .filter(|p| p.rect.is_some())
            .position(|p| p.index == pack_index)
            .ok_or(PlanError::Cells(
                from.map,
                CellPlanError::NoSuchWarp(pack_index as usize),
            ))?;
        let map = self.field_map(from.map)?;
        let plan = self
            .flood(from)?
            .warp_plan(warp)
            .ok_or(PlanError::Cells(from.map, CellPlanError::Unreachable))?;
        let w = map.warps()[warp];
        Ok(Leg {
            map: from.map,
            from: from.cell,
            steps: plan.steps,
            hop: hop_of(record, warp, &w),
        })
    }

    /// Plans the cheapest chain of warps (then final walk) from `from` to
    /// `target`.
    ///
    /// # Errors
    ///
    /// [`PlanError`]; [`PlanError::Unreachable`] when no chain exists.
    pub fn plan(&mut self, from: Position, target: Target) -> Result<Plan, PlanError> {
        self.search(from, target)
    }
}

/// The provenance of `FieldMap` warp `index` of `record`'s map.
///
/// `FieldMap` drops warps with no trigger area, so its index is the position
/// among the record's warps that have one.
fn hop_of(record: &MapRecord, index: usize, warp: &psiv_core::Warp) -> Hop {
    let pack = record.warps.iter().filter(|p| p.rect.is_some()).nth(index);
    Hop {
        warp_index: index,
        pack_index: pack.map_or(u32::MAX, |p| p.index),
        record_offset: pack.map_or_else(String::new, |p| p.record_offset.clone()),
        target_map: warp.target_map.0,
        arrival: warp.target_cell,
        facing: warp.facing,
    }
}

/// One reached position: the cheapest known cost and the leg that got there.
struct Node {
    at: Position,
    cost: usize,
    via: Option<(usize, Leg)>,
}

/// Heap entry `(cost, sequence, node, finish)`. `finish` marks the virtual
/// terminal of a [`Target::Cell`] search, popped when the final walk is the
/// cheapest completion. The sequence number makes equal costs pop in the order
/// they were pushed.
type Entry = Reverse<(usize, u64, usize, bool)>;

impl MapGraph<'_> {
    /// Dijkstra over (map, cell) nodes.
    fn search(&mut self, from: Position, target: Target) -> Result<Plan, PlanError> {
        // Fail early and by name on an unknown source or target map.
        self.field_map(from.map)?;
        let target_map = match target {
            Target::Map(m) | Target::Cell { map: m, .. } => m,
        };
        self.field_map(target_map)?;

        let mut nodes = vec![Node {
            at: from,
            cost: 0,
            via: None,
        }];
        let mut index: HashMap<Position, usize> = HashMap::from([(from, 0)]);
        let mut heap: BinaryHeap<Entry> = BinaryHeap::new();
        let mut seq = 0_u64;
        heap.push(Reverse((0, seq, 0, false)));

        while let Some(Reverse((cost, _, n, finish))) = heap.pop() {
            if finish {
                return self.finish(&nodes, n, target);
            }
            if cost > nodes[n].cost {
                continue; // stale entry
            }
            let here = nodes[n].at;
            if here.map == target_map {
                match target {
                    Target::Map(_) => return self.finish(&nodes, n, target),
                    Target::Cell { cell, .. } => {
                        if let Some(plan) = self.flood(here)?.cell_plan(cell) {
                            seq += 1;
                            heap.push(Reverse((cost + plan.steps.len(), seq, n, true)));
                        }
                    }
                }
            }
            let flood = self.flood(here)?;
            let map = self.field_map(here.map)?;
            let record = self
                .record(here.map)
                .ok_or(PlanError::UnknownMap(here.map))?;
            for warp in flood.warps() {
                let Some(plan) = flood.warp_plan(warp) else {
                    continue;
                };
                let w = map.warps()[warp];
                let target_id = w.target_map.0;
                if self.record(target_id).is_none() {
                    continue; // a warp out of the packed set cannot be followed
                }
                let arrival = Position {
                    map: target_id,
                    cell: w.target_cell,
                };
                let next_cost = cost + plan.steps.len();
                let slot = *index.entry(arrival).or_insert_with(|| {
                    nodes.push(Node {
                        at: arrival,
                        cost: usize::MAX,
                        via: None,
                    });
                    nodes.len() - 1
                });
                if next_cost >= nodes[slot].cost {
                    continue;
                }
                nodes[slot].cost = next_cost;
                nodes[slot].via = Some((
                    n,
                    Leg {
                        map: here.map,
                        from: here.cell,
                        steps: plan.steps,
                        hop: hop_of(record, warp, &w),
                    },
                ));
                seq += 1;
                heap.push(Reverse((next_cost, seq, slot, false)));
            }
        }
        Err(PlanError::Unreachable {
            from,
            target,
            explored: nodes.len(),
        })
    }

    fn finish(&mut self, nodes: &[Node], last: usize, target: Target) -> Result<Plan, PlanError> {
        let mut legs = Vec::new();
        let mut at = last;
        while let Some((prev, leg)) = &nodes[at].via {
            legs.push(leg.clone());
            at = *prev;
        }
        legs.reverse();
        let from = nodes[at].at;
        let end = nodes[last].at;
        let (tail, to) = match target {
            Target::Map(_) => (Vec::new(), end),
            Target::Cell { map, cell } => {
                let plan = self
                    .flood(end)?
                    .cell_plan(cell)
                    .ok_or(PlanError::Cells(map, CellPlanError::Unreachable))?;
                (plan.steps, Position { map, cell })
            }
        };
        Ok(Plan {
            from,
            legs,
            tail,
            to,
        })
    }
}
