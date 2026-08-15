//! NPC wander — `FieldObj_NPCType2` / `NPCType3`.
//!
//! Transcribed from the cartridge; the evidence and the retail addresses are in
//! `docs/NPC_WANDER.md`. Two types share one walker and differ in a single
//! constant, and between them they are 297 of the 949 objects in the packed
//! maps.
//!
//! # The generator is the portable one
//!
//! `FieldObj_GetRandomMove` calls **`UpdateRNGSeed`** (`$04236C`), the
//! multiply-by-41 LCG — not the VDP-counter `UpdateRNGSeed2` that makes battle
//! rolls unreproducible. So cartridge NPC positions are bit-exactly
//! reproducible given the same seed and the same frame cadence, and this module
//! takes a [`Rolls`] like everything else rather than owning a generator.
//!
//! # Why this exists
//!
//! The comparator found it: from frame 7819 of tape 02 the cartridge's
//! townsfolk have wandered off their spawn cells while ours stood still, and
//! our walker then blocked on a cell the cartridge's NPC had already left. A
//! wanderer therefore has to move the object *in the map*, not merely animate
//! beside it — occupancy, blocking, the follower trail and the talk probes all
//! read [`FieldMap::npc_at`], so they only agree if the map is what moves.

use crate::battle::Rolls;
use crate::error::MapError;
use crate::field::StepFrames;
use crate::geom::{Cell, Direction};
use crate::map::FieldMap;

/// Frames one wander step takes: speed selector 0, `$1000` of travel at
/// `$80` per frame. Four times slower than the party's eight.
pub const WANDER_STEP_FRAMES: u8 = 32;

/// The command byte each of the eight roll indices maps to, from `loc_4A150`:
/// `$00 $01 $02 $04 $08 $00 $00 $00`.
///
/// Reading the roll as a command byte directly is the trap this table exists to
/// prevent — the raw command table has three ids for left and three for right,
/// so an unmapped `& 7` makes right unreachable and left three times as likely.
const COMMAND_FOR_INDEX: [u8; 8] = [0x00, 0x01, 0x02, 0x04, 0x08, 0x00, 0x00, 0x00];

/// Which way a command byte moves, or `None` for the standing commands.
const fn direction_for_command(command: u8) -> Option<Direction> {
    match command {
        0x01 => Some(Direction::Up),
        0x02 => Some(Direction::Down),
        0x04 => Some(Direction::Left),
        0x08 => Some(Direction::Right),
        _ => None,
    }
}

/// Which wandering routine an object runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WanderKind {
    /// `NPCType2` — `FieldObj_GetRandomMove`, pause masked `$3F`.
    Type2,
    /// `NPCType3` — `FieldObj_GetRandomMove2`, pause masked `$7F`. The same
    /// walker idling about twice as long.
    Type3,
}

impl WanderKind {
    /// The mask the routine applies to the roll to get its pause.
    #[must_use]
    pub const fn pause_mask(self) -> u16 {
        match self {
            WanderKind::Type2 => 0x3F,
            WanderKind::Type3 => 0x7F,
        }
    }
}

/// The leash: how far an object may stray from where it started.
///
/// `FieldObj_NPCType2`'s init writes maxima of 4 on both axes and current
/// offsets of 2, so a wanderer roams a 5×5 cell box centred on its spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leash {
    /// `x_max_move_boundary` (`$3C`).
    pub x_max: u8,
    /// `y_max_move_boundary` (`$3D`).
    pub y_max: u8,
    /// `x_move_boundary` (`$3E`), the current offset.
    pub x: u8,
    /// `y_move_boundary` (`$3F`).
    pub y: u8,
}

impl Default for Leash {
    fn default() -> Leash {
        Leash {
            x_max: 4,
            y_max: 4,
            x: 2,
            y: 2,
        }
    }
}

impl Leash {
    /// Applies a cell delta, or refuses.
    ///
    /// **Commits on success**, which is what the cartridge does — and it does
    /// so *before* the terrain and object checks run, so a move refused later
    /// has still spent its leash budget and the box drifts. Reproduced
    /// deliberately; see `docs/NPC_WANDER.md`.
    fn accepts(&mut self, dx: i8, dy: i8) -> bool {
        let Some(x) = self.x.checked_add_signed(dx) else {
            return false;
        };
        let Some(y) = self.y.checked_add_signed(dy) else {
            return false;
        };
        if x > self.x_max || y > self.y_max {
            return false;
        }
        self.x = x;
        self.y = y;
        true
    }
}

/// A step a wanderer is part-way through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WanderStep {
    dir: Direction,
    from: Cell,
    progress: u8,
}

/// One wandering object.
///
/// Holds the object's index into [`FieldMap::npcs`] and everything the walker
/// needs. The object's *cell* lives in the map, not here, because that is what
/// every occupancy query reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanderer {
    npc_index: usize,
    kind: WanderKind,
    leash: Leash,
    timer: i16,
    step: Option<WanderStep>,
    frames: StepFrames,
}

impl Wanderer {
    /// A wanderer for the object at `npc_index`.
    ///
    /// The timer starts at zero, so the object rolls on its first on-screen
    /// frame — matching a fresh map load, where the object slot was
    /// block-cleared.
    #[must_use]
    pub fn new(npc_index: usize, kind: WanderKind) -> Wanderer {
        Wanderer {
            npc_index,
            kind,
            leash: Leash::default(),
            timer: 0,
            step: None,
            frames: StepFrames::new(WANDER_STEP_FRAMES).unwrap_or(StepFrames::DEFAULT),
        }
    }

    /// Which object this drives.
    #[must_use]
    pub const fn npc_index(&self) -> usize {
        self.npc_index
    }

    /// Which routine it runs.
    #[must_use]
    pub const fn kind(&self) -> WanderKind {
        self.kind
    }

    /// Its leash state.
    #[must_use]
    pub const fn leash(&self) -> Leash {
        self.leash
    }

    /// Frames left before the next roll.
    #[must_use]
    pub const fn timer(&self) -> i16 {
        self.timer
    }

    /// Whether it is mid-step.
    #[must_use]
    pub const fn is_stepping(&self) -> bool {
        self.step.is_some()
    }

    /// How far into its step it is, in frames. `0` at rest, `1..=frames` while
    /// walking — the step's first frame is the frame the command was issued.
    #[must_use]
    pub fn progress(&self) -> u8 {
        self.step.map_or(0, |step| step.progress)
    }

    /// The cell it stepped away from, while stepping.
    ///
    /// The object's *current* cell is in the map — committed at step start —
    /// so this is what the pixel position has to be measured from.
    #[must_use]
    pub fn step_origin(&self) -> Option<Cell> {
        self.step.map(|step| step.from)
    }

    /// The signed pixel offset from the origin cell, floored as the hardware
    /// floors it.
    ///
    /// Position is a 16.16 fixed point and the logged word is its integer half,
    /// so the fraction rounds **down**, not toward zero — and that is not
    /// symmetric. Half a pixel into a step, an object moving down reads `+0`
    /// while one moving left reads `−1`. Frame 6939 of the oracle log shows
    /// both at once: slot 3 walking down sits at its origin `y`, slot 2 walking
    /// left is already a pixel past its origin `x`.
    #[must_use]
    pub fn travelled_px(&self) -> (i32, i32) {
        let Some(step) = self.step else {
            return (0, 0);
        };
        let numerator = i32::from(step.progress) * crate::field::SUBCELL_UNITS;
        let frames = i32::from(self.frames.get());
        let (dx, dy) = step.dir.delta();
        (
            (dx * numerator).div_euclid(frames),
            (dy * numerator).div_euclid(frames),
        )
    }

    /// `(x_step_duration, y_step_duration)` in the cartridge's units.
    ///
    /// An object's duration counts `$1000` down to 0 in `$80` steps — a
    /// thirty-second of a cell per frame. Only the walked axis is nonzero, and
    /// both are zero at rest.
    #[must_use]
    pub fn step_durations(&self) -> (u16, u16) {
        let Some(step) = self.step else {
            return (0, 0);
        };
        let per_frame = 0x1000 / u16::from(self.frames.get());
        let left = 0x1000u16.saturating_sub(u16::from(step.progress) * per_frame);
        match step.dir {
            Direction::Left | Direction::Right => (left, 0),
            Direction::Up | Direction::Down => (0, left),
        }
    }

    /// Sub-cell displacement in sixteenths, for the renderer.
    ///
    /// Measured from the cell it is walking *to*, because the object commits to
    /// its destination cell the moment the step starts — so the offset runs
    /// from −16 back to 0 rather than 0 out to 16.
    ///
    /// The truncation matches the hardware's: on a step's first frame the
    /// object has moved *nothing* (offset a full −16), because half a pixel
    /// truncates to zero. Measuring the remaining distance instead would put it
    /// a pixel ahead of the cartridge for the whole step.
    #[must_use]
    pub fn render_offset_16ths(&self) -> (i32, i32) {
        if self.step.is_none() {
            return (0, 0);
        }
        // The offset is measured back from the committed destination cell.
        let (tx, ty) = self.travelled_px();
        let (dx, dy) = self.step.map_or((0, 0), |step| step.dir.delta());
        (
            tx - dx * crate::field::SUBCELL_UNITS,
            ty - dy * crate::field::SUBCELL_UNITS,
        )
    }
}

/// A wanderer's state at a moment in time, for picking a replay up mid-run.
///
/// The engine cannot execute the opening scene, so at a replay's alignment
/// frame the cartridge's objects have already been wandering for thousands of
/// frames — they are not at their spawn cells and their leashes are not
/// centred. Restoring is the object-side twin of seeding the RNG and the
/// party's `game_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WanderState {
    /// The countdown to the next roll.
    pub timer: i16,
    /// The leash offsets and maxima.
    pub leash: Leash,
    /// A step in progress: the direction, how many frames in it is (`1..=32`),
    /// and the cell it stepped **out of**.
    ///
    /// The object's current cell is its *destination* and lives in the map; the
    /// origin is what the pixel position interpolates from, so a restore has to
    /// carry it rather than leave it to be guessed.
    pub step: Option<(Direction, u8, Cell)>,
}

/// Every wanderer on a map.
///
/// Ticked once per frame, in index order, so the sequence of rolls is a
/// function of the set and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WanderSet {
    wanderers: Vec<Wanderer>,
}

impl WanderSet {
    /// An empty set.
    #[must_use]
    pub const fn new() -> WanderSet {
        WanderSet {
            wanderers: Vec::new(),
        }
    }

    /// Builds a set from `(npc_index, kind)` pairs.
    ///
    /// # Errors
    ///
    /// [`MapError::NpcIndexOutOfRange`] when a pair names no object on `map`.
    pub fn build(map: &FieldMap, objects: &[(usize, WanderKind)]) -> Result<WanderSet, MapError> {
        let count = map.npcs().len();
        let mut wanderers = Vec::with_capacity(objects.len());
        for &(npc_index, kind) in objects {
            if npc_index >= count {
                return Err(MapError::NpcIndexOutOfRange {
                    index: npc_index,
                    count,
                });
            }
            wanderers.push(Wanderer::new(npc_index, kind));
        }
        Ok(WanderSet { wanderers })
    }

    /// The wanderers, in tick order.
    #[must_use]
    pub fn wanderers(&self) -> &[Wanderer] {
        &self.wanderers
    }

    /// One wanderer by its object index.
    #[must_use]
    pub fn get(&self, npc_index: usize) -> Option<&Wanderer> {
        self.wanderers.iter().find(|w| w.npc_index == npc_index)
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.wanderers.is_empty()
    }

    /// Restores one wanderer's state, for a replay picking up mid-run.
    ///
    /// The object's cell and facing live in the map; set those with
    /// [`FieldMap::set_npc_cell`] and [`FieldMap::set_npc_facing`], remembering
    /// that a mid-step object's cell is its **destination**.
    ///
    /// # Errors
    ///
    /// [`MapError::NpcIndexOutOfRange`] when no wanderer drives that object.
    pub fn restore(&mut self, npc_index: usize, state: WanderState) -> Result<(), MapError> {
        let count = self.wanderers.len();
        let Some(wanderer) = self.wanderers.iter_mut().find(|w| w.npc_index == npc_index) else {
            return Err(MapError::NpcIndexOutOfRange {
                index: npc_index,
                count,
            });
        };
        wanderer.timer = state.timer;
        wanderer.leash = state.leash;
        wanderer.step = state.step.map(|(dir, progress, from)| WanderStep {
            dir,
            from,
            progress,
        });
        Ok(())
    }

    /// Advances every wanderer one frame.
    ///
    /// `visible` decides which objects are on camera. **Off-screen objects are
    /// frozen** — `FieldObj_OnScreenTest` makes a rejected object skip the
    /// move, the duration update *and* the position update, so an off-screen
    /// wanderer neither steps nor counts down nor draws a roll. The engine has
    /// no camera, so the caller supplies the predicate; passing `|_| true`
    /// models a camera that sees everything.
    ///
    /// Objects move **in the map**: a started step commits the destination cell
    /// immediately, which is what keeps occupancy, blocking, the follower trail
    /// and the talk probes agreeing with each other.
    pub fn tick(
        &mut self,
        map: &mut FieldMap,
        rolls: &mut impl Rolls,
        party: &[Cell],
        visible: impl Fn(usize) -> bool,
    ) {
        for index in 0..self.wanderers.len() {
            if !visible(self.wanderers[index].npc_index) {
                continue;
            }
            self.tick_one(index, map, rolls, party);
        }
    }

    fn tick_one(
        &mut self,
        index: usize,
        map: &mut FieldMap,
        rolls: &mut impl Rolls,
        party: &[Cell],
    ) {
        // Mid-step: advance and land. `FieldObj_GetRandomMove` returns command
        // 0 while either duration is nonzero, so nothing else happens.
        if let Some(step) = self.wanderers[index].step.as_mut() {
            step.progress = step.progress.saturating_add(1);
            if step.progress >= self.wanderers[index].frames.get() {
                self.wanderers[index].step = None;
            }
            return;
        }

        // Idle: count down, and only roll when the countdown has run out.
        // `subq.w #1` then `bpl` means a timer of n waits n+1 frames.
        self.wanderers[index].timer -= 1;
        if self.wanderers[index].timer >= 0 {
            return;
        }

        let roll = rolls.next_roll();
        let kind = self.wanderers[index].kind;
        self.wanderers[index].timer = (roll & kind.pause_mask()) as i16;
        let command = COMMAND_FOR_INDEX[usize::from((roll & 7) as u8)];

        let Some(dir) = direction_for_command(command) else {
            return;
        };

        let npc_index = self.wanderers[index].npc_index;
        let Some(from) = map.npcs().get(npc_index).map(|npc| npc.cell) else {
            return;
        };

        // A refused move still turns the object on the spot, so the facing is
        // written before any gate can reject.
        let _ = map.set_npc_facing(npc_index, dir);

        // Gate 1: the leash. Commits on success, before the other two run.
        let (dx, dy) = dir.delta();
        let accepted = self.wanderers[index].leash.accepts(dx as i8, dy as i8);
        if !accepted {
            return;
        }

        // Gate 2: terrain, the same blocking set the party uses.
        let Some(to) = map.neighbor(from, dir) else {
            return;
        };
        if map.grid().is_blocking(to) {
            return;
        }

        // Gate 3: other objects, and the party. `loc_4A316` scans the five
        // character slots, so a wanderer will not walk into the party; and
        // occupancy is the same `active && interactable` predicate everything
        // else uses.
        if party.contains(&to) {
            return;
        }
        if map.npc_at(to).is_some_and(|npc| npc.cell != from) {
            return;
        }

        // Accepted. The object moves in the map now, not on landing.
        if map.set_npc_cell(npc_index, to).is_ok() {
            // `FieldObj_UpdateStepDuration` and `FieldObj_UpdatePosition` run
            // in the same frame as `FieldObj_NPCMove`, so the step's first
            // frame is the frame the command was issued — exactly as the
            // party's steps work. A step therefore spans `frames` ticks, not
            // `frames + 1`.
            let step = WanderStep {
                dir,
                from,
                progress: 1,
            };
            self.wanderers[index].step =
                (step.progress < self.wanderers[index].frames.get()).then_some(step);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::SliceRolls;
    use crate::collision::CollisionGrid;
    use crate::map::{MapId, Npc, NpcId};

    fn map_with_npc(rows: &[&str], cell: Cell) -> FieldMap {
        let height = u16::try_from(rows.len()).unwrap();
        let width = u16::try_from(rows[0].len()).unwrap();
        let cells: Vec<u8> = rows
            .iter()
            .flat_map(|row| row.chars())
            .map(|ch| if ch == '#' { 8 } else { 0 })
            .collect();
        let grid = CollisionGrid::new(width, height, cells).unwrap();
        FieldMap::new(
            MapId(0),
            grid,
            vec![],
            vec![Npc::new(NpcId(2), cell, Direction::Down)],
        )
        .unwrap()
    }

    /// A roll whose low three bits pick `index` and whose masked value is the
    /// pause. Since one roll supplies both, they cannot be chosen apart.
    const fn roll_for(index: u16) -> u16 {
        index
    }

    #[test]
    fn the_remap_table_is_symmetric() {
        let mut counts = [0usize; 5]; // stand, up, down, left, right
        for index in 0..8u8 {
            match direction_for_command(COMMAND_FOR_INDEX[usize::from(index)]) {
                None => counts[0] += 1,
                Some(Direction::Up) => counts[1] += 1,
                Some(Direction::Down) => counts[2] += 1,
                Some(Direction::Left) => counts[3] += 1,
                Some(Direction::Right) => counts[4] += 1,
            }
        }
        assert_eq!(counts, [4, 1, 1, 1, 1], "50% stand, 12.5% each direction");
    }

    #[test]
    fn a_wanderer_steps_and_commits_its_destination_immediately() {
        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // index 2 -> command $02 -> down.
        let draws = [roll_for(2)];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);

        assert_eq!(
            map.npcs()[0].cell,
            Cell::new(1, 2),
            "the map moves on the frame the step starts"
        );
        assert_eq!(map.npcs()[0].facing, Direction::Down);
        assert!(set.get(0).unwrap().is_stepping());
        assert!(
            map.is_walkable(Cell::new(1, 1)),
            "and the cell it left is free at once"
        );
    }

    #[test]
    fn a_step_takes_thirty_two_frames() {
        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [roll_for(2)];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);
        for frame in 1..WANDER_STEP_FRAMES {
            assert!(set.get(0).unwrap().is_stepping(), "frame {frame}");
            set.tick(&mut map, &mut rolls, &[], |_| true);
        }
        assert!(!set.get(0).unwrap().is_stepping(), "lands on frame 32");
    }

    #[test]
    fn one_roll_supplies_both_the_pause_and_the_direction() {
        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // $32: low three bits = 2 (down), masked with $3F = 50 (the pause).
        let draws = [0x32u16];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(
            map.npcs()[0].cell,
            Cell::new(1, 2),
            "direction from bits 0-2"
        );
        assert_eq!(set.get(0).unwrap().timer(), 0x32, "pause from bits 0-5");
        assert_eq!(rolls.drawn(), 1, "one draw, not two");
    }

    #[test]
    fn type_three_pauses_about_twice_as_long() {
        assert_eq!(WanderKind::Type2.pause_mask(), 0x3F);
        assert_eq!(WanderKind::Type3.pause_mask(), 0x7F);

        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut two = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let mut three = WanderSet::build(&map, &[(0, WanderKind::Type3)]).unwrap();
        let draws = [0x7Au16];
        let mut a = SliceRolls::new(&draws);
        let mut b = SliceRolls::new(&draws);

        let mut copy = map.clone();
        two.tick(&mut copy, &mut a, &[], |_| true);
        three.tick(&mut map, &mut b, &[], |_| true);

        assert_eq!(two.get(0).unwrap().timer(), 0x3A, "$7A & $3F");
        assert_eq!(three.get(0).unwrap().timer(), 0x7A, "$7A & $7F");
    }

    #[test]
    fn an_off_screen_wanderer_is_frozen_and_draws_no_rolls() {
        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [roll_for(2)];
        let mut rolls = SliceRolls::new(&draws);

        for _ in 0..100 {
            set.tick(&mut map, &mut rolls, &[], |_| false);
        }

        assert_eq!(map.npcs()[0].cell, Cell::new(1, 1), "never moved");
        assert_eq!(set.get(0).unwrap().timer(), 0, "never counted down");
        assert_eq!(rolls.drawn(), 0, "and never consumed the shared generator");
    }

    #[test]
    fn the_leash_keeps_a_wanderer_within_two_cells() {
        let rows = ["..........", "..........", "..........", ".........."];
        let mut map = map_with_npc(&rows, Cell::new(5, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // Always roll "left", with a zero pause so it rolls every other frame.
        let draws = [0x08u16 | 3]; // low bits 3 -> command $04 -> left
        let mut rolls = SliceRolls::new(&draws);

        for _ in 0..400 {
            set.tick(&mut map, &mut rolls, &[], |_| true);
        }

        assert_eq!(
            map.npcs()[0].cell,
            Cell::new(3, 1),
            "two cells left of spawn and no further"
        );
        assert_eq!(set.get(0).unwrap().leash().x, 0, "the leash is spent");
    }

    #[test]
    fn terrain_refuses_a_move_but_still_turns_the_object() {
        let mut map = map_with_npc(&["....", ".#..", "...."], Cell::new(1, 2));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // index 1 -> command $01 -> up, into the wall at (1, 1).
        let draws = [roll_for(1)];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);

        assert_eq!(map.npcs()[0].cell, Cell::new(1, 2), "did not move");
        assert_eq!(
            map.npcs()[0].facing,
            Direction::Up,
            "but turned on the spot"
        );
        assert!(!set.get(0).unwrap().is_stepping());
    }

    #[test]
    fn a_terrain_refusal_still_spends_the_leash() {
        // The cartridge commits the boundary before checking terrain, so the
        // box drifts. Reproduced deliberately.
        let mut map = map_with_npc(&["....", ".#..", "...."], Cell::new(1, 2));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [roll_for(1)];
        let mut rolls = SliceRolls::new(&draws);

        assert_eq!(set.get(0).unwrap().leash().y, 2);
        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(
            set.get(0).unwrap().leash().y,
            1,
            "the refused move consumed leash budget anyway"
        );
    }

    #[test]
    fn a_wanderer_will_not_walk_into_the_party() {
        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [roll_for(2)];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[Cell::new(1, 2)], |_| true);

        assert_eq!(map.npcs()[0].cell, Cell::new(1, 1), "the party blocks it");
        assert_eq!(map.npcs()[0].facing, Direction::Down, "it still turns");
    }

    #[test]
    fn two_wanderers_do_not_share_a_cell() {
        let grid = CollisionGrid::filled(6, 4, 0).unwrap();
        let mut map = FieldMap::new(
            MapId(0),
            grid,
            vec![],
            vec![
                Npc::new(NpcId(2), Cell::new(1, 1), Direction::Down),
                Npc::new(NpcId(2), Cell::new(1, 2), Direction::Down),
            ],
        )
        .unwrap();
        let mut set =
            WanderSet::build(&map, &[(0, WanderKind::Type2), (1, WanderKind::Type2)]).unwrap();
        // Both roll "down"; the first moves into (1,2) only if it is free, and
        // it is not until the second moves out of it.
        let draws = [roll_for(2)];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);

        let cells: Vec<Cell> = map.npcs().iter().map(|n| n.cell).collect();
        assert_eq!(cells[0], Cell::new(1, 1), "blocked by the one below it");
        assert_eq!(cells[1], Cell::new(1, 3), "which moved on its own tick");
    }

    #[test]
    fn a_bit_clear_object_is_not_occupancy_for_a_wanderer_either() {
        let grid = CollisionGrid::filled(6, 4, 0).unwrap();
        let mut map = FieldMap::new(
            MapId(0),
            grid,
            vec![],
            vec![
                Npc::new(NpcId(2), Cell::new(1, 1), Direction::Down),
                Npc::new(NpcId(0x50), Cell::new(1, 2), Direction::Down).with_interactable(false),
            ],
        )
        .unwrap();
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [roll_for(2)];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);

        assert_eq!(
            map.npcs()[0].cell,
            Cell::new(1, 2),
            "it walks onto the bit-clear object, as the party would"
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_positions() {
        let rows = ["........", "........", "........", "........"];
        let replay = || {
            let mut map = map_with_npc(&rows, Cell::new(4, 2));
            let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
            let mut rolls = crate::battle::Lcg41::new(0x1234_5678);
            let mut trail = Vec::new();
            for _ in 0..2_000 {
                set.tick(&mut map, &mut rolls, &[], |_| true);
                trail.push(map.npcs()[0].cell);
            }
            (trail, set)
        };

        let (first, set_a) = replay();
        let (second, set_b) = replay();
        assert_eq!(first, second, "same seed, same walk");
        assert_eq!(set_a, set_b);
        assert!(
            first.windows(2).any(|pair| pair[0] != pair[1]),
            "and it actually wandered"
        );

        // Never outside the 5x5 box around the spawn.
        for cell in &first {
            assert!(
                cell.x.abs_diff(4) <= 2 && cell.y.abs_diff(2) <= 2,
                "strayed to ({}, {})",
                cell.x,
                cell.y
            );
        }
    }

    #[test]
    fn the_step_trace_matches_the_oracles_object_columns() {
        // Pinned against `oracle/logs/02_walk_timing.csv` slot o00, frames
        // 6962-6994: timer reads 0, the next frame rolls and reloads to 10
        // (and 10 & 7 = 2 = down, which is how the remap table gets confirmed
        // from hardware), ydur runs $0F80 down to 0 in $80 steps over exactly
        // 32 frames, and y advances 240 -> 256 half a pixel at a time.
        let mut map = map_with_npc(&["....", "....", "....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [10u16];
        let mut rolls = SliceRolls::new(&draws);

        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(
            set.get(0).unwrap().timer(),
            10,
            "the reload the oracle logs"
        );
        assert_eq!(map.npcs()[0].facing, Direction::Down, "10 & 7 = 2 = down");

        let origin = crate::trigger::PixelPos::from_cell(Cell::new(1, 1));
        let mut durations = Vec::new();
        let mut pixels = Vec::new();
        // Oracle frames 6963..6993: the 31 frames with distance still to run.
        for _ in 0..(WANDER_STEP_FRAMES - 1) {
            let w = set.get(0).unwrap();
            durations.push(w.step_durations().1);
            pixels.push(origin.y + w.travelled_px().1);
            set.tick(&mut map, &mut rolls, &[], |_| true);
        }

        assert_eq!(
            &durations[..4],
            &[0x0F80, 0x0F00, 0x0E80, 0x0E00],
            "$80 a frame, the first sample already decremented"
        );
        assert_eq!(
            durations.last(),
            Some(&0x0080),
            "the last frame with distance left"
        );
        // Oracle frames 6963-6967 read y = 240, 241, 241, 242, 242.
        assert_eq!(
            &pixels[..5],
            &[
                origin.y,
                origin.y + 1,
                origin.y + 1,
                origin.y + 2,
                origin.y + 2
            ],
            "half a pixel a frame, truncated"
        );

        // The last of those ticks is frame 6994, arrival: the duration reads
        // zero and the timer is still the reload, because the arriving frame
        // does no timer work.
        assert_eq!(set.get(0).unwrap().step_durations(), (0, 0), "arrived");
        assert_eq!(map.npcs()[0].cell, Cell::new(1, 2), "one cell down");
        assert_eq!(
            set.get(0).unwrap().timer(),
            10,
            "untouched for the whole step"
        );

        // Frame 6995: the first frame to decrement it again.
        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(set.get(0).unwrap().timer(), 9);
    }

    #[test]
    fn the_pixel_offset_floors_rather_than_truncating_toward_zero() {
        // Oracle frame 6939: slot 3 walking *down* with ydur $0F80 reads y at
        // its origin (112), while slot 2 walking *left* with xdur $0F80 already
        // reads x one pixel past its origin (639 from 640). Half a pixel floors
        // to 0 going down and to -1 going left.
        let mut map = map_with_npc(&["......", "......", "......"], Cell::new(2, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // index 2 -> down.
        let draws = [2u16];
        let mut rolls = SliceRolls::new(&draws);
        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(
            set.get(0).unwrap().travelled_px(),
            (0, 0),
            "down: floors to 0"
        );

        let mut map = map_with_npc(&["......", "......", "......"], Cell::new(2, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // index 3 -> command $04 -> left.
        let draws = [3u16];
        let mut rolls = SliceRolls::new(&draws);
        set.tick(&mut map, &mut rolls, &[], |_| true);
        assert_eq!(
            set.get(0).unwrap().travelled_px(),
            (-1, 0),
            "left: floors to -1"
        );
    }

    #[test]
    fn a_restored_wanderer_carries_its_timer_leash_and_step() {
        let map = map_with_npc(&["......", "......", "......"], Cell::new(2, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();

        set.restore(
            0,
            WanderState {
                timer: 23,
                leash: Leash {
                    x_max: 4,
                    y_max: 4,
                    x: 4,
                    y: 2,
                },
                step: Some((Direction::Left, 1, Cell::new(2, 1))),
            },
        )
        .unwrap();

        let w = set.get(0).unwrap();
        assert_eq!(w.timer(), 23);
        assert_eq!(w.leash().x, 4, "already at the leash limit, as slot 0 is");
        assert!(w.is_stepping());
        assert_eq!(w.step_durations(), (0x0F80, 0), "one frame into the step");
        assert_eq!(w.travelled_px(), (-1, 0));

        assert!(matches!(
            set.restore(
                9,
                WanderState {
                    timer: 0,
                    leash: Leash::default(),
                    step: None
                }
            ),
            Err(MapError::NpcIndexOutOfRange { .. })
        ));
    }

    #[test]
    fn a_roll_fires_on_the_frame_the_timer_reads_zero() {
        // Not on an expiry edge some frames later: the countdown reaches 0,
        // and the *next* frame rolls. A reload of 0 therefore rolls again
        // immediately, which is the consecutive-roll case.
        let mut map = map_with_npc(&["....", "....", "...."], Cell::new(1, 1));
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        // Roll 0: index 0 -> stand still, and a reload of 0.
        let draws = [0u16];
        let mut rolls = SliceRolls::new(&draws);

        for frame in 0..5 {
            set.tick(&mut map, &mut rolls, &[], |_| true);
            assert_eq!(
                set.get(0).unwrap().timer(),
                0,
                "frame {frame}: a zero reload keeps the timer at zero"
            );
        }
        assert_eq!(rolls.drawn(), 5, "so it rolls every single frame");
    }

    #[test]
    fn only_registered_wanderers_consume_rolls() {
        // Slot 7 on PiataAcademy_F1 is NPCAlysPiata: the oracle sees her timer
        // hold 0 for all 1563 field frames while consuming nothing, because
        // her routine is not GetRandomMove. A set that does not register an
        // object must never draw for it.
        let grid = CollisionGrid::filled(6, 4, 0).unwrap();
        let mut map = FieldMap::new(
            MapId(0),
            grid,
            vec![],
            vec![
                Npc::new(NpcId(0x3C), Cell::new(1, 1), Direction::Down),
                Npc::new(NpcId(0x68), Cell::new(3, 1), Direction::Down),
            ],
        )
        .unwrap();
        // Only slot 0 wanders.
        let mut set = WanderSet::build(&map, &[(0, WanderKind::Type2)]).unwrap();
        let draws = [0x20u16];
        let mut rolls = SliceRolls::new(&draws);

        for _ in 0..200 {
            set.tick(&mut map, &mut rolls, &[], |_| true);
        }

        assert_eq!(
            map.npcs()[1].cell,
            Cell::new(3, 1),
            "the unregistered object never moved"
        );
        assert!(set.get(1).is_none(), "and has no wander state at all");
    }

    #[test]
    fn build_rejects_an_index_that_names_no_object() {
        let map = map_with_npc(&["..", ".."], Cell::new(0, 0));
        assert!(matches!(
            WanderSet::build(&map, &[(1, WanderKind::Type2)]),
            Err(MapError::NpcIndexOutOfRange { index: 1, count: 1 })
        ));
        assert!(WanderSet::build(&map, &[(0, WanderKind::Type2)]).is_ok());
    }
}
