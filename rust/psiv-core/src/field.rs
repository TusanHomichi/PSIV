//! The field-mode state machine: `State + Input -> State + Effects`.
//!
//! Integer math only. No clocks, no randomness, no interior mutability, no
//! hashed iteration — the same state, map and input sequence always produces
//! the same state and the same effect log. See `docs/RUNTIME_DESIGN.md`,
//! "Fidelity spine (field mode)".

use crate::collision::CollisionType;
use crate::error::MapError;
use crate::geom::{Cell, Direction};
use crate::map::{FieldMap, MapId, Warp, WarpTrigger};

/// How many sixteenths make up one cell. A collision cell is 16 pixels, so a
/// sixteenth of a cell is exactly one pixel at 1x — see
/// [`FieldState::render_offset_16ths`].
pub const SUBCELL_UNITS: i32 = 16;

/// One tick of player input: a d-pad direction, or nothing.
///
/// There is no diagonal. The original resolves movement on the 16-pixel
/// collision grid one axis at a time, so a bridge that reads two d-pad axes
/// must pick one direction per tick before calling [`FieldState::tick`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Input {
    /// No direction held this tick.
    #[default]
    Neutral,
    /// A direction held this tick.
    Direction(Direction),
}

impl Input {
    /// The held direction, if any.
    #[must_use]
    pub const fn direction(self) -> Option<Direction> {
        match self {
            Input::Neutral => None,
            Input::Direction(dir) => Some(dir),
        }
    }
}

impl From<Direction> for Input {
    fn from(dir: Direction) -> Input {
        Input::Direction(dir)
    }
}

/// How many ticks one cell-step takes.
///
/// **The default of 8 is extracted fidelity, not a guess** (it began life as
/// a placeholder and the cartridge later agreed): `FieldObj_MovementsTbl` at
/// ROM `0x047AA8` defines the normal walking step constant `$0200` — two
/// pixels per frame in 16.16 fixed point — which is exactly 8 frames per
/// 16-pixel collision cell. The table's other blocks are slow (`$0100`, 16
/// frames) and fast (`$0400`, 4 frames), selected by `FieldObj_Step_Offset`;
/// this stays a construction parameter so those modes cost nothing to add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepFrames(u8);

impl StepFrames {
    /// The cartridge's normal walking speed: 8 ticks per cell.
    pub const DEFAULT: StepFrames = StepFrames(8);

    /// Builds a step duration.
    ///
    /// # Errors
    ///
    /// [`MapError::ZeroStepFrames`] when `frames` is zero — a step has to take
    /// at least one tick or "mid-step" is unrepresentable.
    pub const fn new(frames: u8) -> Result<StepFrames, MapError> {
        if frames == 0 {
            return Err(MapError::ZeroStepFrames);
        }
        Ok(StepFrames(frames))
    }

    /// The duration in ticks.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for StepFrames {
    fn default() -> StepFrames {
        StepFrames::DEFAULT
    }
}

/// Something the field engine did this tick, for the layer above to act on.
///
/// Effects are returned in a fixed order (see [`FieldState::tick`]), so effect
/// logs compare cleanly between runs.
///
/// Deliberately *not* `#[non_exhaustive]`: when battle and dialogue effects
/// land, every consumer's `match` should stop compiling until someone decides
/// what the new effect means there. Silently dropping an effect is exactly the
/// failure mode this runtime cannot afford.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// The party finished a step and now stands on `cell`.
    StepCompleted {
        /// The cell just landed on.
        cell: Cell,
    },
    /// The party landed on a cell that fired a transition. The layer above
    /// loads `target_map` and calls [`FieldState::enter_map`].
    Warp {
        /// The cell that triggered the transition.
        from: Cell,
        /// Which table fired. Carried because presentation differs — a doorway
        /// fades, a map edge scrolls — and presentation is the bridge's call.
        trigger: WarpTrigger,
        /// The map to load.
        target_map: MapId,
        /// Where the party lands on that map.
        target_cell: Cell,
        /// Which way the party faces on arrival.
        facing: Direction,
    },
    /// The party landed on a map-change cell that no map-change warp covers.
    ///
    /// A data problem, surfaced instead of panicked on: either the pack lost a
    /// transition record, or the `XYRange` selector resolved to the wrong
    /// rectangle. The party simply stays put.
    ///
    /// There is no equivalent for ordinary ground: most of a map is walkable
    /// cells with no transition over them, so a miss there is the normal case,
    /// not a defect.
    WarpUnmapped {
        /// The unmapped map-change cell.
        cell: Cell,
    },
}

/// Whether `cell` on `map` is collision type 1. Out of bounds is not, which is
/// the graceful answer when a caller hands `tick` the wrong map.
fn is_map_change(map: &FieldMap, cell: Cell) -> bool {
    map.collision_at(cell)
        .is_some_and(CollisionType::is_map_change)
}

fn warp_effect(from: Cell, warp: &Warp) -> Effect {
    Effect::Warp {
        from,
        trigger: warp.trigger,
        target_map: warp.target_map,
        target_cell: warp.target_cell,
        facing: warp.facing,
    }
}

/// A step in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    dir: Direction,
    /// Destination, resolved and validated when the step began.
    to: Cell,
    /// Ticks elapsed, in `1..=step_frames`.
    progress: u8,
}

/// The field-mode party state.
///
/// The party's logical position is always a whole cell: during a step, `cell()`
/// stays on the cell it left until the step completes, and
/// [`FieldState::render_offset_16ths`] carries the sub-cell displacement for
/// the renderer. That keeps collision, warp and NPC queries answerable at any
/// tick without ever asking "which cell is it half-way into".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldState {
    map: MapId,
    cell: Cell,
    facing: Direction,
    step: Option<Step>,
    step_frames: StepFrames,
    /// Whether the cell the party currently occupies is collision type 1.
    /// `MapTransTile_MapChange` only fires when the *previous* cell was not,
    /// which is what keeps a wide doorway from re-firing as you walk along it.
    on_map_change: bool,
}

impl FieldState {
    /// Places the party on `map`.
    ///
    /// # Errors
    ///
    /// [`MapError::PartyOutOfBounds`] when the start cell is outside the grid.
    pub fn new(
        map: &FieldMap,
        cell: Cell,
        facing: Direction,
        step_frames: StepFrames,
    ) -> Result<FieldState, MapError> {
        let mut state = FieldState {
            map: map.id(),
            cell,
            facing,
            step: None,
            step_frames,
            on_map_change: false,
        };
        state.enter_map(map, cell, facing)?;
        Ok(state)
    }

    /// Moves the party onto `map` at `cell`, facing `facing`, cancelling any
    /// step in progress.
    ///
    /// This is how a [`Effect::Warp`] is completed once the caller has loaded
    /// the target map. Placement deliberately does **not** fire warps, even
    /// onto a map-change cell: transitions fire on arrival by walking, which is
    /// what keeps a doorway from bouncing the party back and forth forever.
    /// Arriving on a map-change cell counts as already occupying one, so the
    /// doorway you land in stays quiet until you step off it and back on —
    /// the same "previous cell was not type 1" rule the walker uses.
    ///
    /// Placement checks bounds and nothing else. It deliberately accepts
    /// blocking terrain and NPC-occupied cells, because the cartridge's own
    /// transition destinations land there: of the 815 retail transitions whose
    /// target layout decodes, 119 put the party on a solid cell and 4 put it on
    /// an NPC. That is consistent rather than broken — the walker only ever
    /// tests the cell it is stepping *into*, never the one it is standing on,
    /// so a party dropped inside a wall walks straight out of it. Refusing
    /// these would strand the bridge at, among others, Piata's academy doorway.
    ///
    /// Callers wanting to know can ask [`FieldMap::is_walkable`].
    ///
    /// # Errors
    ///
    /// [`MapError::PartyOutOfBounds`] when `cell` is outside the grid.
    pub fn enter_map(
        &mut self,
        map: &FieldMap,
        cell: Cell,
        facing: Direction,
    ) -> Result<(), MapError> {
        if !map.grid().contains(cell) {
            return Err(MapError::PartyOutOfBounds {
                map: map.id(),
                cell,
                width: map.width(),
                height: map.height(),
            });
        }
        self.map = map.id();
        self.cell = cell;
        self.facing = facing;
        self.step = None;
        self.on_map_change = is_map_change(map, cell);
        Ok(())
    }

    /// Advances one tick.
    ///
    /// The rules, in the order they apply:
    ///
    /// 1. **At rest with a direction held**: face that way (facing always
    ///    changes, even into a wall — you can look at a sign you cannot walk
    ///    into). If the target cell is in bounds, non-blocking and unoccupied
    ///    by an NPC, a step begins and advances on this same tick, so a step
    ///    started on tick `T` completes on tick `T + frames - 1`.
    /// 2. **Mid-step**: progress advances regardless of input, and input is not
    ///    read at all. The original commits to a whole cell-step once it
    ///    starts; releasing the d-pad mid-step still finishes the cell.
    /// 3. **On completion**: the party lands, and effects are pushed in this
    ///    order — [`Effect::StepCompleted`] first, then at most one of
    ///    [`Effect::Warp`] / [`Effect::WarpUnmapped`]. A landing therefore
    ///    yields one or two effects, never more, and a warp fires exactly once,
    ///    on the completion tick. Which transition table is consulted follows
    ///    `RunMapTransitions`: a landing on collision type 1 reads the
    ///    map-change table (and only if the cell just left was not also type 1);
    ///    any other landing reads the normal-ground table. See [`WarpTrigger`].
    ///
    /// A tick that starts and does not complete a step returns no effects.
    ///
    /// `map` must be the map the state is on ([`FieldState::map`]); the caller
    /// changes maps through [`FieldState::enter_map`], never by handing `tick`
    /// a different map mid-step. Doing so is a caller bug, caught by a
    /// `debug_assert`, and handled gracefully in release: the landing cell is
    /// re-checked against whatever map was passed and simply triggers no warp
    /// if it is out of that map's bounds.
    pub fn tick(&mut self, map: &FieldMap, input: Input) -> Vec<Effect> {
        debug_assert_eq!(
            map.id(),
            self.map,
            "tick called with a different map than the party is on; use enter_map"
        );

        let mut effects = Vec::new();

        if self.step.is_none()
            && let Some(dir) = input.direction()
        {
            self.facing = dir;
            if let Some(to) = self.cell.neighbor(dir)
                && map.is_walkable(to)
            {
                self.step = Some(Step {
                    dir,
                    to,
                    progress: 0,
                });
            }
        }

        let Some(step) = self.step.as_mut() else {
            return effects;
        };

        step.progress = step.progress.saturating_add(1);
        if step.progress < self.step_frames.get() {
            return effects;
        }

        let landed = step.to;
        let came_from_map_change = self.on_map_change;
        self.step = None;
        self.cell = landed;
        self.on_map_change = is_map_change(map, landed);
        effects.push(Effect::StepCompleted { cell: landed });

        // `RunMapTransitions` picks its walker from the standing collision
        // type: type 1 reads the map-change table, anything else the walker can
        // stand on reads the normal table.
        if self.on_map_change {
            if !came_from_map_change {
                match map.warp_at(landed, WarpTrigger::MapChange) {
                    Some(warp) => effects.push(warp_effect(landed, warp)),
                    None => effects.push(Effect::WarpUnmapped { cell: landed }),
                }
            }
        } else if map.collision_at(landed).is_some()
            && let Some(warp) = map.warp_at(landed, WarpTrigger::NormalGround)
        {
            effects.push(warp_effect(landed, warp));
        }

        effects
    }

    /// The map the party is on.
    #[must_use]
    pub const fn map(&self) -> MapId {
        self.map
    }

    /// The party's logical cell: the cell it stands on, or the cell it is
    /// stepping *from* while a step is in progress.
    #[must_use]
    pub const fn cell(&self) -> Cell {
        self.cell
    }

    /// Which way the party faces.
    #[must_use]
    pub const fn facing(&self) -> Direction {
        self.facing
    }

    /// Whether a step is in progress.
    #[must_use]
    pub const fn is_stepping(&self) -> bool {
        self.step.is_some()
    }

    /// The direction of the step in progress, if any.
    #[must_use]
    pub fn step_direction(&self) -> Option<Direction> {
        self.step.map(|step| step.dir)
    }

    /// The cell the step in progress will land on, if any.
    #[must_use]
    pub fn step_destination(&self) -> Option<Cell> {
        self.step.map(|step| step.to)
    }

    /// Ticks elapsed in the step in progress, in `1..step_frames`. Zero at
    /// rest, and never equal to `step_frames` — a step that reaches its last
    /// tick lands within that same tick.
    #[must_use]
    pub fn step_progress(&self) -> u8 {
        self.step.map_or(0, |step| step.progress)
    }

    /// The configured step duration.
    #[must_use]
    pub const fn step_frames(&self) -> StepFrames {
        self.step_frames
    }

    /// The sub-cell displacement from [`FieldState::cell`], in sixteenths of a
    /// cell, as `(dx, dy)`.
    ///
    /// `(0, 0)` at rest. During a step it moves monotonically away from zero
    /// along the step axis and returns to zero on the landing tick, when
    /// `cell()` advances instead. A cell is 16 pixels, so these sixteenths are
    /// exactly pixels at 1x scale; a renderer draws the party at
    /// `cell * 16 + offset` (times whatever it scales by).
    ///
    /// Integer division, truncating: with the default 8 frames the sequence is
    /// 2, 4, 6, 8, 10, 12, 14 and then a landing. A step duration that does not
    /// divide 16 simply advances unevenly rather than introducing a float.
    /// Renderers wanting smoother interpolation can use
    /// [`FieldState::step_progress`] and [`FieldState::step_frames`] directly.
    #[must_use]
    pub fn render_offset_16ths(&self) -> (i32, i32) {
        let Some(step) = self.step else {
            return (0, 0);
        };
        let travelled =
            i32::from(step.progress) * SUBCELL_UNITS / i32::from(self.step_frames.get());
        let (dx, dy) = step.dir.delta();
        (dx * travelled, dy * travelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::CollisionGrid;

    fn open_map() -> FieldMap {
        FieldMap::new(
            MapId(0),
            CollisionGrid::filled(4, 4, 0).unwrap(),
            vec![],
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn step_frames_rejects_zero() {
        assert!(matches!(StepFrames::new(0), Err(MapError::ZeroStepFrames)));
        assert_eq!(StepFrames::new(3).unwrap().get(), 3);
        assert_eq!(StepFrames::default(), StepFrames::DEFAULT);
        assert_eq!(StepFrames::DEFAULT.get(), 8);
    }

    #[test]
    fn a_one_frame_step_lands_on_the_tick_it_starts() {
        let map = open_map();
        let mut state = FieldState::new(
            &map,
            Cell::new(0, 0),
            Direction::Down,
            StepFrames::new(1).unwrap(),
        )
        .unwrap();
        let effects = state.tick(&map, Direction::Right.into());
        assert_eq!(
            effects,
            vec![Effect::StepCompleted {
                cell: Cell::new(1, 0)
            }]
        );
        assert_eq!(state.render_offset_16ths(), (0, 0));
    }

    #[test]
    fn placement_rejects_only_out_of_bounds_cells() {
        let map = open_map();
        assert!(matches!(
            FieldState::new(
                &map,
                Cell::new(9, 0),
                Direction::Down,
                StepFrames::default()
            ),
            Err(MapError::PartyOutOfBounds { .. })
        ));
    }

    #[test]
    fn a_party_placed_inside_a_wall_can_walk_out_of_it() {
        // 119 retail transition destinations do exactly this. The walker only
        // tests the cell it steps into, so the way out is open.
        let mut grid = CollisionGrid::filled(4, 4, 0).unwrap();
        grid.set(Cell::new(1, 1), 0x8).unwrap();
        let map = FieldMap::new(MapId(4), grid, vec![], vec![]).unwrap();

        let mut state = FieldState::new(
            &map,
            Cell::new(1, 1),
            Direction::Down,
            StepFrames::new(1).unwrap(),
        )
        .expect("a blocked start cell is legal");

        let effects = state.tick(&map, Input::Direction(Direction::Right));
        assert_eq!(
            effects,
            vec![Effect::StepCompleted {
                cell: Cell::new(2, 1)
            }]
        );
    }
}
