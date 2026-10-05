//! Walking: the `go_to`, `go_to_map` and `patrol` controllers.
//!
//! The planner is R0's: [`plan_cells`] over the *live* map for steps inside a
//! map (so a door the party opened, a bridge a flag laid and an NPC standing in
//! the way are all what the engine says they are now), and a [`MapGraph`] built
//! from the flags the game actually holds for the warp chain between maps. The
//! controller holds a direction for a step, and re-plans from the real cell
//! whenever the party comes to rest somewhere the plan did not expect: a scene
//! or a battle moved it, a map loaded, something stood in the way.

use psiv_core::{Cell, Direction, Flag};
use psiv_runtime::{Button, Runtime};

use crate::cell_plan::{Flood, Goal, Mover, plan_cells_for};
use crate::driver::{Driver, dir_pad};
use crate::exec::{Memory, execute};
use crate::field::Settled;
use crate::halt::{Halt, HaltKind, Res};
use crate::map_plan::{MapGraph, Position, Target};
use crate::route::{Step, Until};

/// Times in a row a walk may end where it began before the run halts.
const NO_PROGRESS_LIMIT: u32 = 8;

/// Re-plans one objective may spend: a long Motavia crossing meets many
/// battles, and each ends a walk.
const REPLAN_LIMIT: u32 = 2_000;

/// Neutral frames spent waiting for whatever blocked a step to move on.
const BLOCKED_WAIT: u32 = 6;

impl Driver {
    /// The standing cell.
    #[must_use]
    pub fn cell(&self) -> Cell {
        crate::driver::standing_cell(self.runtime())
    }

    /// How the party moves: on foot, or in the vehicle the game has mounted.
    #[must_use]
    pub fn mover(&self) -> Mover {
        self.runtime()
            .vehicle_index()
            .map_or(Mover::Foot, Mover::Vehicle)
    }

    /// The map the party is on.
    #[must_use]
    pub fn map(&self) -> u16 {
        self.runtime().map_id().0
    }

    /// Walks to `target` on `map`, crossing warps when `map` is elsewhere. A
    /// scene that fires on the way and moves the party off `map` ends the
    /// objective: the story owns the party from there.
    ///
    /// # Errors
    ///
    /// [`HaltKind::Unreachable`] when no walk exists, [`HaltKind::Stuck`] when
    /// the party cannot move, or a halt from a frame.
    pub fn go_to(&mut self, map: u16, target: Cell) -> Res {
        let mut stalled = 0;
        let began_on_target_map = self.map() == map;
        let scenes_before = self.scenes_ended();
        for _ in 0..REPLAN_LIMIT {
            let prompt = self.settle(true)? == Settled::Choice;
            let here = (self.map(), self.cell());
            if here == (map, target) {
                return Ok(());
            }
            if prompt {
                return Err(unanswered_prompt());
            }
            if began_on_target_map && here.0 != map && self.scenes_ended() > scenes_before {
                // A scene fired on the way and carried the party off the map:
                // the story took the party, and what it left is the next
                // objective's to assert.
                self.note(format!(
                    "go_to ({},{}): a scene took the party to map {:#x}",
                    target.x, target.y, here.0
                ));
                return Ok(());
            }
            let steps = if here.0 == map {
                let plan = plan_cells_for(
                    self.runtime().map(),
                    self.mover(),
                    here.1,
                    Goal::Cell(target),
                )
                .map_err(|e| unreachable_halt(map, target, &e.to_string()))?;
                plan.steps
            } else {
                self.first_leg(Target::Cell { map, cell: target })?
            };
            self.walk(&steps)?;
            stalled = progress(&mut stalled, here, (self.map(), self.cell()))?;
        }
        Err(Halt::new(
            HaltKind::Stuck,
            format!("{REPLAN_LIMIT} re-plans and the party is not at the target"),
        ))
    }

    /// Walks to the cell beside `target` on `map` and takes the one step onto
    /// it, `target` being a cell where a map trigger starts a scene: a warp
    /// footprint, where the scene wins over the warp (`RunEvents` runs before
    /// `RunMapTransitions` on foot, `ps4.asm:116768-116773`), or any other
    /// trigger cell. The objective is met when a scene has run; what it leaves
    /// is the next objective's to assert. A `go_to` on such a cell would plan
    /// again from wherever the scene left the party and fire the trigger a
    /// second time.
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when the party is not on `map`, the step
    /// fired the warp with no scene, or the party stands on the cell and no
    /// scene ran; [`HaltKind::Unreachable`] when no walk reaches the step,
    /// [`HaltKind::Stuck`] when the party cannot move, or a halt from a frame.
    pub fn step_onto(&mut self, map: u16, target: Cell) -> Res {
        let mut stalled = 0;
        let scenes_before = self.scenes_ended();
        for _ in 0..REPLAN_LIMIT {
            let prompt = self.settle(true)? == Settled::Choice;
            if self.scenes_ended() > scenes_before {
                return Ok(());
            }
            let here = (self.map(), self.cell());
            if here.0 != map {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    format!(
                        "step_onto ({},{}) of map {map:#x}: the party is on map {:#x} and no \
                         scene ran, so the step fired the warp",
                        target.x, target.y, here.0
                    ),
                ));
            }
            if prompt {
                return Err(unanswered_prompt());
            }
            if self.runtime().map().normalize(target) == Some(here.1) {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    format!(
                        "step_onto ({},{}) of map {map:#x}: the party stands on the cell and no \
                         scene ran: no trigger fires there with the flags the game holds",
                        target.x, target.y
                    ),
                ));
            }
            let flood = Flood::for_mover(self.runtime().map(), here.1, self.mover())
                .map_err(|e| unreachable_halt(map, target, &e.to_string()))?;
            let plan = flood
                .onto_plan(self.runtime().map(), self.mover(), target)
                .ok_or_else(|| {
                    unreachable_halt(map, target, "no walk ends in a step onto the cell")
                })?;
            self.walk(&plan.steps)?;
            stalled = progress(&mut stalled, here, (self.map(), self.cell()))?;
        }
        Err(Halt::new(
            HaltKind::Stuck,
            format!(
                "{REPLAN_LIMIT} re-plans and no scene ran on ({},{})",
                target.x, target.y
            ),
        ))
    }

    /// Arrives on `map`, taking the warp whose record index is `via_warp`
    /// first when the route names one.
    ///
    /// # Errors
    ///
    /// As [`Driver::go_to`].
    pub fn go_to_map(&mut self, map: u16, via_warp: Option<u32>) -> Res {
        let mut stalled = 0;
        let mut first = via_warp;
        let start_map = self.map();
        for _ in 0..REPLAN_LIMIT {
            let settled = self.settle_at(true, true)?;
            let prompt = settled == Settled::Choice;
            let here = (self.map(), self.cell());
            if settled == Settled::Menu {
                // The same for the ship's menu a scene opens on arrival.
                if here.0 == map {
                    return Ok(());
                }
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    "the ship's destination menu is open and no objective asked",
                ));
            }
            if prompt {
                // A scene that fires on arrival and asks a question (a house
                // that offers a rest) has delivered the party: the prompt is
                // the next objective's to answer.
                if here.0 == map {
                    return Ok(());
                }
                return Err(unanswered_prompt());
            }
            if first.is_some() && here.0 != start_map {
                first = None;
            }
            let steps = match first {
                Some(index) => self.via_warp_steps(index)?,
                None if here.0 == map => return Ok(()),
                None => self.first_leg(Target::Map(map))?,
            };
            self.walk(&steps)?;
            stalled = progress(&mut stalled, here, (self.map(), self.cell()))?;
        }
        Err(Halt::new(
            HaltKind::Stuck,
            format!("{REPLAN_LIMIT} re-plans and the party has not reached map {map:#x}"),
        ))
    }

    /// Presses Action in the vehicle the party rides, which dismounts when the
    /// standing cell is open ground (`psiv_core::dismount_allowed`).
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when the party is not mounted;
    /// [`HaltKind::Stuck`] when the press did not dismount (the cell is not
    /// open ground).
    pub fn dismount(&mut self) -> Res {
        self.settle(false)?;
        if self.runtime().vehicle_index().is_none() {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                "the party is not riding a vehicle",
            ));
        }
        self.tap(Button::Speak)?;
        self.neutral(2)?;
        if self.runtime().vehicle_index().is_some() {
            return Err(Halt::new(
                HaltKind::Stuck,
                format!(
                    "the vehicle would not stop at ({},{}) on map {:#x}: the cell is not open ground",
                    self.cell().x,
                    self.cell().y,
                    self.map()
                ),
            ));
        }
        Ok(())
    }

    /// Walks between `a` and `b` on `map` until `until` holds, fighting what
    /// comes, and ends standing on `b`. When the party cannot train on (see
    /// [`Driver::needs_refuge`]) and the route gave a `refuge`, the refuge runs
    /// first and the patrol resumes from where it ends.
    ///
    /// # Errors
    ///
    /// As [`Driver::go_to`], and a refuge objective's own halt.
    pub fn patrol(&mut self, map: u16, a: Cell, b: Cell, until: &Until, refuge: &[Step]) -> Res {
        loop {
            if !refuge.is_empty() && self.needs_refuge() {
                self.note("patrol: the party cannot train on, taking the refuge");
                let mut memory = Memory::default();
                for step in refuge {
                    execute(self, &mut memory, &step.objective)?;
                }
            }
            self.go_to(map, a)?;
            if self.until_holds(until) {
                break;
            }
            self.go_to(map, b)?;
            if self.until_holds(until) {
                return Ok(());
            }
        }
        self.go_to(map, b)
    }

    /// Whether the party has run out of ways to train: a member is down, or a
    /// living member is below half HP after the camp cure had its turn.
    #[must_use]
    pub fn needs_refuge(&self) -> bool {
        self.runtime().camp_state().party.iter().any(|m| {
            m.current_hp == 0
                || m.status & psiv_core::battle::status::DEAD != 0
                || u32::from(m.current_hp) * 100 < u32::from(m.max_hp) * 50
        })
    }

    /// Whether a patrol's condition holds: every given clause.
    #[must_use]
    pub fn until_holds(&self, until: &Until) -> bool {
        let camp = self.runtime().camp_state();
        until
            .party_level_at_least
            .is_none_or(|level| camp.party.iter().all(|m| m.level >= u16::from(level)))
            && until.money_at_least.is_none_or(|money| camp.money >= money)
    }

    /// The steps that start the warp whose pack record index is `index`, from
    /// the live map.
    fn via_warp_steps(&self, index: u32) -> Res<Vec<Direction>> {
        let runtime = self.runtime();
        let record = runtime
            .map_record()
            .ok_or_else(|| Halt::new(HaltKind::Unreachable, "the current map has no record"))?;
        let position = record
            .warps
            .iter()
            .filter(|w| w.rect.is_some())
            .position(|w| w.index == index)
            .ok_or_else(|| {
                Halt::new(
                    HaltKind::WrongObject,
                    format!(
                        "map {:#x} has no warp with record index {index} that has a trigger area",
                        self.map()
                    ),
                )
            })?;
        let flood = Flood::for_mover(runtime.map(), self.cell(), self.mover())
            .map_err(|e| Halt::new(HaltKind::Unreachable, e.to_string()))?;
        // A warp the ordinary stepping rule fires is walked to and stepped
        // through. One whose trigger area the party can stand in without the
        // rule firing it (an opened elevator door: its cells are map-change
        // ground, the warp record is a ground trigger) is walked into, and the
        // game's own event decides what the step does.
        let plan = flood.warp_plan(position).or_else(|| {
            let source = runtime.map().warps().get(position)?.source;
            flood.rect_plan(runtime.map(), source)
        });
        plan.map(|p| p.steps).ok_or_else(|| {
            Halt::new(
                HaltKind::Unreachable,
                format!(
                    "no walk from ({},{}) reaches warp {index} of map {:#x}",
                    self.cell().x,
                    self.cell().y,
                    self.map()
                ),
            )
        })
    }

    /// The walkable cells where the current map's record puts an object that
    /// is no longer there: gone from the map or standing elsewhere now.
    fn vacated_spawn_cells(&self) -> Vec<Cell> {
        let runtime = self.runtime();
        let Some(record) = runtime.map_record() else {
            return Vec::new();
        };
        let live = runtime.map();
        record
            .npcs
            .iter()
            .enumerate()
            .filter_map(|(index, spawn)| {
                let cell = Cell::new(spawn.x_cell as u16, spawn.y_cell as u16);
                let here = live.npcs().get(index)?;
                let moved = !here.active || here.cell != cell;
                let open = live
                    .collision_at(cell)
                    .is_some_and(|collision| !collision.is_blocking());
                (moved && open).then_some(cell)
            })
            .collect()
    }

    /// The steps of the first warp on the cheapest chain to `target`, planned
    /// from the flags the game holds now and re-walked on the live map.
    fn first_leg(&self, target: Target) -> Res<Vec<Direction>> {
        let runtime = self.runtime();
        let flags = set_flags(runtime);
        let mut graph = MapGraph::new(runtime.data(), &flags)
            .map_err(|e| Halt::new(HaltKind::Unreachable, e.to_string()))?
            .with_mover(self.mover());
        let from = Position {
            map: self.map(),
            cell: self.cell(),
        };
        // The graph is built from the pack's records, where every object
        // stands at its spawn cell. An object the game has since moved off
        // (the Esper Mansion's door guards, a scene's actor) no longer blocks
        // the cell it was recorded on.
        let vacated = self.vacated_spawn_cells();
        if !vacated.is_empty() {
            graph.open_cells(from.map, vacated);
        }
        let plan = graph
            .plan(from, target)
            .map_err(|e| Halt::new(HaltKind::Unreachable, e.to_string()))?;
        let Some(leg) = plan.legs.first() else {
            return Err(Halt::new(
                HaltKind::Unreachable,
                "the planner found no warp to take",
            ));
        };
        // The live map is the truth: a door opened by an interaction, an NPC in
        // the way. The graph's own steps are the fallback when the live flood
        // cannot see the same warp.
        let live = Flood::for_mover(runtime.map(), from.cell, self.mover())
            .ok()
            .and_then(|flood| flood.warp_plan(leg.hop.warp_index))
            .map(|p| p.steps);
        Ok(live.unwrap_or_else(|| leg.steps.clone()))
    }

    /// Holds each direction for a step, in order, until the steps run out, the
    /// map changes, something other than the field owns the frame, or the way
    /// is blocked. The caller re-plans from wherever the party stands.
    fn walk(&mut self, steps: &[Direction]) -> Res {
        let start_map = self.map();
        for &direction in steps {
            loop {
                if self.map() != start_map || self.mode_name() != "field" {
                    return Ok(());
                }
                if self.runtime().loot_state().is_some() || self.runtime().field_notice().is_some()
                {
                    return Ok(());
                }
                if crate::driver::is_stepping(self.runtime()) {
                    self.neutral(1)?;
                } else {
                    break;
                }
            }
            let before = self.cell();
            let expected = self
                .mover()
                .landing(self.runtime().map(), before, direction);
            self.tick(dir_pad(direction))?;
            while self.mode_name() == "field" && crate::driver::is_stepping(self.runtime()) {
                self.neutral(1)?;
            }
            if self.map() != start_map {
                return Ok(());
            }
            let after = self.cell();
            if Some(after) == expected {
                continue;
            }
            if after == before {
                // Blocked: a wanderer, an NPC that stepped in. Wait a moment
                // and let the caller plan around it.
                self.neutral(BLOCKED_WAIT)?;
            }
            return Ok(());
        }
        Ok(())
    }
}

/// Counts walks that went nowhere; halts after [`NO_PROGRESS_LIMIT`] in a row.
fn progress(stalled: &mut u32, before: (u16, Cell), after: (u16, Cell)) -> Res<u32> {
    if before == after {
        *stalled += 1;
        if *stalled >= NO_PROGRESS_LIMIT {
            return Err(Halt::new(
                HaltKind::Stuck,
                format!(
                    "the party has not moved from map {:#x} ({},{}) in {NO_PROGRESS_LIMIT} walks",
                    before.0, before.1.x, before.1.y
                ),
            ));
        }
    } else {
        *stalled = 0;
    }
    Ok(*stalled)
}

fn unanswered_prompt() -> Halt {
    Halt::new(
        HaltKind::UnexpectedState,
        "a yes/no prompt opened and the route does not answer it here",
    )
}

fn unreachable_halt(map: u16, target: Cell, why: &str) -> Halt {
    Halt::new(
        HaltKind::Unreachable,
        format!("map {map:#x} ({},{}): {why}", target.x, target.y),
    )
}

/// Every flag the game holds set, for a planner that must see the same maps
/// the runtime built.
fn set_flags(runtime: &Runtime) -> Vec<Flag> {
    let game = runtime.game();
    let events = (0..512).map(Flag::event);
    let temps = (0..256).map(Flag::temp);
    let towns = (0..128).map(Flag::town);
    events
        .chain(temps)
        .chain(towns)
        .filter(|flag| game.is_set(*flag))
        .collect()
}
