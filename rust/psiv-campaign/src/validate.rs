//! Route validation: every id in a route exists in the pack, and every walk it
//! asks for is statically possible.
//!
//! The validator replays a route against the pack without a game. It tracks
//! two things the way the route claims them:
//!
//! * **position**, from the route's `start`, each `go_to`/`go_to_map` (the cell
//!   the plan ends on) and each `expect` that names a map and cell;
//! * **flags**, from the start state and each `expect`'s `flags_set` and
//!   `flags_clear`.
//!
//! Each `go_to` and `go_to_map` is then planned by [`MapGraph`] with the flags
//! the route has claimed so far, so a bridge that opens on a flag is open only
//! after the route says the flag is set. Objectives that need to know where the
//! party is (`talk`, `open_chest`) need a position the route has pinned.
//!
//! The validator proves reachability only. Whether a scene or a lost battle
//! leaves the party where a chapter says is the runner's job (R1); that is
//! what `expect` is for.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use psiv_core::{Cell, Flag};
use psiv_data::{BattleFiles, GameData};

use crate::cell_plan::{Flood, Mover};
use crate::map_plan::{MapGraph, Plan, PlanError, Position, Target};
use crate::route::{Chapter, Expectation, NameOrId, Objective, Route};

/// One problem, located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Chapter id.
    pub chapter: String,
    /// Objective index within the chapter; `None` for a chapter-level problem
    /// or a closing assertion (the reason then says which).
    pub objective: Option<usize>,
    /// What is wrong.
    pub reason: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.objective {
            Some(i) => write!(
                f,
                "chapter {:?} objective {i}: {}",
                self.chapter, self.reason
            ),
            None => write!(f, "chapter {:?}: {}", self.chapter, self.reason),
        }
    }
}

/// What a validation run found and planned.
#[derive(Debug, Default)]
pub struct Report {
    /// Problems; empty means the route is valid.
    pub errors: Vec<ValidationError>,
    /// Notes that are not errors (an unpinned position skipped a check).
    pub warnings: Vec<String>,
    /// Pad steps across every planned walk.
    pub planned_steps: usize,
    /// Warps across every planned walk.
    pub planned_warps: usize,
    /// Objectives checked.
    pub objectives: usize,
}

impl Report {
    /// Whether the route is valid.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validates `route` against the pack.
#[must_use]
pub fn validate(route: &Route, data: &GameData, battle: &BattleFiles) -> Report {
    let mut run = Run::new(route, data, battle);
    run.seed();
    let mut seen = BTreeSet::new();
    for chapter in &route.chapters {
        if !seen.insert(chapter.id.as_str()) {
            run.chapter_error(chapter, "duplicate chapter id");
        }
        run.chapter(chapter);
    }
    run.report
}

struct Run<'a> {
    route: &'a Route,
    data: &'a GameData,
    battle: &'a BattleFiles,
    report: Report,
    pos: Option<Position>,
    flags: BTreeSet<Flag>,
    /// Cells an `interact` opened, by map; dropped when the party leaves.
    opened: BTreeMap<u16, Vec<Cell>>,
    /// How the party moves, as the route's `vehicle` assertions claim it.
    mover: Mover,
}

impl<'a> Run<'a> {
    fn new(route: &'a Route, data: &'a GameData, battle: &'a BattleFiles) -> Run<'a> {
        Run {
            route,
            data,
            battle,
            report: Report::default(),
            pos: None,
            flags: BTreeSet::new(),
            opened: BTreeMap::new(),
            mover: Mover::Foot,
        }
    }

    fn seed(&mut self) {
        if let Some(start) = &self.route.start {
            self.pos = Some(Position {
                map: start.map,
                cell: start.cell.cell(),
            });
            self.flags.extend(start.flags_set.iter().map(|f| f.0));
            return;
        }
        if let Some(gs) = self.data.manifest().game_start.as_ref() {
            self.pos = Some(Position {
                map: gs.map.id,
                cell: Cell::new(
                    u16::try_from(gs.x_cell).unwrap_or(0),
                    u16::try_from(gs.y_cell).unwrap_or(0),
                ),
            });
            self.flags.extend(
                gs.event_flags_set
                    .iter()
                    .chain(&gs.extended_event_flags_set)
                    .map(|id| Flag::event(*id)),
            );
            self.flags
                .extend(gs.chest_flags_set.iter().map(|id| Flag::chest(*id)));
            self.flags
                .extend(gs.town_flags_set.iter().map(|id| Flag::town(*id)));
        } else {
            self.report
                .warnings
                .push("the pack has no game_start and the route no start: position unknown".into());
        }
    }

    fn chapter_error(&mut self, chapter: &Chapter, reason: &str) {
        self.report.errors.push(ValidationError {
            chapter: chapter.id.clone(),
            objective: None,
            reason: reason.to_owned(),
        });
    }

    fn error(&mut self, chapter: &Chapter, objective: usize, reason: String) {
        self.report.errors.push(ValidationError {
            chapter: chapter.id.clone(),
            objective: Some(objective),
            reason,
        });
    }

    fn chapter(&mut self, chapter: &Chapter) {
        if chapter.random_battle_policy.trim().is_empty() {
            self.chapter_error(chapter, "random_battle_policy is empty");
        }
        if chapter.objectives.is_empty() {
            self.chapter_error(chapter, "no objectives");
        }
        for (index, step) in chapter.objectives.iter().enumerate() {
            self.report.objectives += 1;
            if let Err(reason) = self.objective(&step.objective) {
                self.error(chapter, index, reason);
            }
        }
        for (index, expect) in chapter.closing.iter().enumerate() {
            if let Err(reason) = self.expect(expect) {
                self.report.errors.push(ValidationError {
                    chapter: chapter.id.clone(),
                    objective: None,
                    reason: format!("closing assertion {index}: {reason}"),
                });
            }
        }
    }

    fn graph(&self) -> Result<MapGraph<'a>, String> {
        let flags: Vec<Flag> = self.flags.iter().copied().collect();
        let mut graph = MapGraph::new(self.data, &flags)
            .map_err(|e| e.to_string())?
            .with_mover(self.mover);
        for (map, cells) in &self.opened {
            graph.open_cells(*map, cells.clone());
        }
        Ok(graph)
    }

    fn require_map(&self, map: u16) -> Result<(), String> {
        if self.data.contains(psiv_data::MapId(map)) {
            Ok(())
        } else {
            Err(format!("map {map:#05x} is not in the pack"))
        }
    }

    fn here(&self, what: &str) -> Result<Position, String> {
        self.pos.ok_or_else(|| {
            format!("{what} needs a known map: pin one with an expect that names map and cell")
        })
    }

    /// Moves the tracked position; leaving a map closes what was opened on it.
    fn move_to(&mut self, to: Position) {
        if let Some(from) = self.pos
            && from.map != to.map
        {
            self.opened.remove(&from.map);
        }
        self.pos = Some(to);
    }

    fn note_plan(&mut self, plan: &Plan) {
        self.report.planned_steps += plan.total_steps();
        self.report.planned_warps += plan.legs.len();
    }

    fn objective(&mut self, objective: &Objective) -> Result<(), String> {
        match objective {
            Objective::GoTo { map, cell } => {
                self.require_map(*map)?;
                let target = Position {
                    map: *map,
                    cell: cell.cell(),
                };
                self.go(Target::Cell {
                    map: *map,
                    cell: target.cell,
                })
            }
            Objective::StepOnto { map, cell } => self.step_onto(*map, cell.cell()),
            Objective::GoToMap { map, via_warp } => {
                self.require_map(*map)?;
                self.go_to_map(*map, *via_warp)
            }
            Objective::Talk { npc } => {
                let here = self.here("talk")?;
                let count = self.record_of(here.map)?.npcs.len();
                if (*npc as usize) < count {
                    Ok(())
                } else {
                    Err(format!(
                        "map {:#05x} has {count} objects; object {npc} does not exist",
                        here.map
                    ))
                }
            }
            Objective::Answer { .. } | Objective::RestInn { .. } | Objective::FightScripted => {
                Ok(())
            }
            Objective::Board { to, .. } => {
                self.here("board")?;
                let world = crate::ship::world_of(to, self.data)?;
                // The flight ends on the landing table's row for the world
                // (`loc_64B5A`, `ps4.asm:134593`), whichever menu row picked it.
                let landing = psiv_core::flight_target(psiv_core::FlightLeg::Landing, 0, world)
                    .ok_or_else(|| format!("World_Index {world} has no landing"))?;
                self.require_map(landing.map)?;
                self.move_to(Position {
                    map: landing.map,
                    cell: Cell::new(landing.start_x / 2, landing.start_y / 2 + 1),
                });
                Ok(())
            }
            Objective::Dismount => {
                if self.mover == Mover::Foot {
                    return Err("dismount, and the route has the party on foot".into());
                }
                self.mover = Mover::Foot;
                Ok(())
            }
            Objective::Interact { cell, opens, .. } => {
                let here = self.here("interact")?;
                self.go(Target::Cell {
                    map: here.map,
                    cell: cell.cell(),
                })?;
                if !opens.is_empty() {
                    self.opened
                        .insert(here.map, opens.iter().map(|c| c.cell()).collect());
                }
                Ok(())
            }
            Objective::Patrol {
                map,
                a,
                b,
                until,
                refuge,
            } => {
                self.require_map(*map)?;
                if until.party_level_at_least.is_none() && until.money_at_least.is_none() {
                    return Err("patrol has no stop condition".into());
                }
                self.go(Target::Cell {
                    map: *map,
                    cell: a.cell(),
                })?;
                // A refuge is an out-and-back trip: it must leave from and
                // return to the patrol's map.
                if !refuge.is_empty() {
                    let before = self.here("patrol refuge")?;
                    for step in refuge {
                        self.objective(&step.objective)?;
                    }
                    if self.here("patrol refuge")?.map != *map {
                        return Err(format!("the refuge does not end on map {map:#x}"));
                    }
                    self.move_to(before);
                }
                // Battles and rests decide where a patrol really ends; the
                // runner plans from the actual cell. The validator ends it on
                // `b`.
                self.go(Target::Cell {
                    map: *map,
                    cell: b.cell(),
                })
            }
            Objective::OpenChest { chest } => {
                let here = self.here("open_chest")?;
                let count = self.record_of(here.map)?.treasure_chests.len();
                if (*chest as usize) < count {
                    Ok(())
                } else {
                    Err(format!(
                        "map {:#05x} has {count} chests; chest {chest} does not exist",
                        here.map
                    ))
                }
            }
            Objective::Buy { item, count, .. } => {
                if *count == 0 {
                    return Err("buy count is 0".into());
                }
                self.item(item).map(|_| ())
            }
            Objective::Sell { item, .. } => self.item(item).map(|_| ()),
            Objective::Equip { member, item } => {
                self.member(member)?;
                self.item(item).map(|_| ())
            }
            Objective::UseTechnique {
                caster,
                technique,
                target,
            } => {
                self.member(caster)?;
                self.technique(technique)?;
                self.member(target).map(|_| ())
            }
            Objective::UseItem { item, target } => {
                self.item(item)?;
                self.member(target).map(|_| ())
            }
            Objective::Reorder { order } => {
                let mut seen = BTreeSet::new();
                for who in order {
                    if !seen.insert(self.member(who)?) {
                        return Err(format!("reorder lists {who} twice"));
                    }
                }
                if order.is_empty() {
                    return Err("reorder is empty".into());
                }
                Ok(())
            }
            Objective::Save { slot } => {
                if usize::from(*slot) < psiv_core::RETAIL_SLOT_COUNT {
                    Ok(())
                } else {
                    Err(format!(
                        "save slot {slot} is not one of the {} retail slots",
                        psiv_core::RETAIL_SLOT_COUNT
                    ))
                }
            }
            Objective::Expect(expect) => self.expect(expect),
        }
    }

    fn record_of(&self, map: u16) -> Result<&'a psiv_data::MapRecord, String> {
        self.data
            .map(psiv_data::MapId(map))
            .ok_or_else(|| format!("map {map:#05x} is not in the pack"))
    }

    fn go(&mut self, target: Target) -> Result<(), String> {
        let Some(from) = self.pos else {
            // Nothing to plan from. Pin the destination so later steps check.
            if let Target::Cell { map, cell } = target {
                self.report
                    .warnings
                    .push("go_to with no known start was not planned".into());
                self.pos = Some(Position { map, cell });
            }
            return Ok(());
        };
        let plan = self
            .graph()?
            .plan(from, target)
            .map_err(|e| plan_reason(&e))?;
        self.note_plan(&plan);
        self.move_to(plan.to);
        Ok(())
    }

    /// A `step_onto`: the cell must be a footprint a step can fire from where
    /// the party stands. The scene decides where the party ends, so the
    /// position is unknown until a later `expect` pins it.
    fn step_onto(&mut self, map: u16, cell: Cell) -> Result<(), String> {
        self.require_map(map)?;
        let from = self.here("step_onto")?;
        if from.map != map {
            return Err(format!(
                "step_onto on map {map:#05x}, and the route has the party on map {:#05x}",
                from.map
            ));
        }
        let mut graph = self.graph()?;
        let field = graph.field_map(map).map_err(|e| plan_reason(&e))?;
        let flood = Flood::for_mover(&field, from.cell, self.mover)
            .map_err(|e| plan_reason(&PlanError::Cells(map, e)))?;
        let plan = flood.onto_plan(&field, self.mover, cell).ok_or_else(|| {
            format!(
                "no walk from ({},{}) ends in a step that fires a warp onto ({},{}); \
                     a cell that is not a warp footprint is a go_to",
                from.cell.x, from.cell.y, cell.x, cell.y
            )
        })?;
        self.report.planned_steps += plan.steps.len();
        self.pos = None;
        Ok(())
    }

    fn go_to_map(&mut self, map: u16, via_warp: Option<u32>) -> Result<(), String> {
        let from = self.here("go_to_map")?;
        let mut graph = self.graph()?;
        let (mut steps, mut warps, mut at) = (0, 0, from);
        if let Some(index) = via_warp {
            let leg = graph
                .leg_via_warp(from, index)
                .map_err(|e| format!("via_warp {index}: {}", plan_reason(&e)))?;
            steps += leg.steps.len();
            warps += 1;
            at = Position {
                map: leg.hop.target_map,
                cell: leg.hop.arrival,
            };
        }
        if at.map != map || via_warp.is_none() {
            let plan = graph
                .plan(at, Target::Map(map))
                .map_err(|e| plan_reason(&e))?;
            steps += plan.total_steps();
            warps += plan.legs.len();
            at = plan.to;
        }
        self.report.planned_steps += steps;
        self.report.planned_warps += warps;
        self.move_to(at);
        Ok(())
    }

    fn expect(&mut self, expect: &Expectation) -> Result<(), String> {
        let scratch = psiv_core::GameState::new();
        let mut checked = scratch;
        for flag in expect.flags_set.iter().chain(&expect.flags_clear) {
            checked
                .set(flag.0)
                .map_err(|e| format!("flag {:?}: {e}", flag.0))?;
        }
        for flag in &expect.flags_set {
            if expect.flags_clear.iter().any(|c| c.0 == flag.0) {
                return Err(format!("flag {:?} is both set and clear", flag.0));
            }
        }
        if expect.cell.is_some() && expect.map.is_none() {
            return Err("cell without map".into());
        }
        if let Some(map) = expect.map {
            self.require_map(map)?;
            if let Some(cell) = expect.cell {
                let built = self.graph()?.field_map(map).map_err(|e| plan_reason(&e))?;
                if built.normalize(cell.cell()).is_none() {
                    return Err(format!(
                        "cell ({},{}) is off map {map:#05x}",
                        cell.0, cell.1
                    ));
                }
                self.move_to(Position {
                    map,
                    cell: cell.cell(),
                });
            } else if self.pos.is_some_and(|p| p.map != map) {
                // The route claims a different map and no cell: position is
                // honestly unknown until a later expect pins it.
                self.pos = None;
            }
        }
        if let Some(party) = &expect.party {
            let mut seen = BTreeSet::new();
            for who in party {
                if !seen.insert(self.member(who)?) {
                    return Err(format!("party lists {who} twice"));
                }
            }
        }
        if let Some(vehicle) = expect.vehicle {
            if vehicle > psiv_core::VEHICLE_INDEX_MAX {
                return Err(format!("vehicle {vehicle} is not 0 (on foot) or 1..=3"));
            }
            self.mover = if vehicle == 0 {
                Mover::Foot
            } else {
                Mover::Vehicle(vehicle)
            };
        }
        for flag in &expect.flags_set {
            self.flags.insert(flag.0);
        }
        for flag in &expect.flags_clear {
            self.flags.remove(&flag.0);
        }
        Ok(())
    }

    fn item(&self, item: &NameOrId) -> Result<u8, String> {
        let file = &self.battle.equipment;
        match item {
            NameOrId::Id(id) => u8::try_from(*id)
                .ok()
                .and_then(|id| file.item(id))
                .map(|i| i.id)
                .ok_or_else(|| format!("item #{id} does not exist")),
            NameOrId::Name(name) => file
                .items
                .iter()
                .find(|i| names_match(i.display_name.as_deref(), name))
                .map(|i| i.id)
                .ok_or_else(|| format!("no item named {name:?}")),
        }
    }

    fn technique(&self, technique: &NameOrId) -> Result<u16, String> {
        let list = &self.battle.abilities.techniques;
        match technique {
            NameOrId::Id(id) => list
                .iter()
                .find(|t| t.id == *id)
                .map(|t| t.id)
                .ok_or_else(|| format!("technique #{id} does not exist")),
            NameOrId::Name(name) => list
                .iter()
                .find(|t| names_match(t.display_name.as_deref(), name))
                .map(|t| t.id)
                .ok_or_else(|| format!("no technique named {name:?}")),
        }
    }

    fn member(&self, member: &NameOrId) -> Result<u8, String> {
        let list = &self.battle.characters.characters;
        match member {
            NameOrId::Id(id) => list
                .iter()
                .find(|c| u16::from(c.character_id) == *id)
                .map(|c| c.character_id)
                .ok_or_else(|| format!("character #{id} does not exist")),
            NameOrId::Name(name) => list
                .iter()
                .find(|c| names_match(c.display_name.as_deref(), name))
                .map(|c| c.character_id)
                .ok_or_else(|| format!("no character named {name:?}")),
        }
    }
}

fn names_match(have: Option<&str>, want: &str) -> bool {
    have.is_some_and(|h| h.eq_ignore_ascii_case(want))
}

fn plan_reason(error: &PlanError) -> String {
    error.to_string()
}
