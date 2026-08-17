//! NPC wander — the cartridge's shared random-walk families.
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
pub const WANDER_STEP_FRAMES: u8 = WanderSpeed::Selector0.frames();

/// The three records in the cartridge's `FieldObj_UpdateStepDuration` table.
///
/// The table starts at `ps4.asm:97404` (`$04A20E`) and its records are 88
/// bytes apart: selectors 0, 1 and 2 are `$80/32`, `$100/16` and `$200/8`.
/// The first two were already used by the engine; keeping all three here makes
/// the extracted third record executable rather than merely documented data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WanderSpeed {
    /// `$04A20E`, velocity `$80`, 32 frames per cell.
    Selector0,
    /// `$04A266`, velocity `$100`, 16 frames per cell.
    Selector1,
    /// `$04A2BE`, velocity `$200`, 8 frames per cell.
    Selector2,
}

impl WanderSpeed {
    /// The selector passed in `d7` to `FieldObj_UpdateStepDuration`.
    #[must_use]
    pub const fn selector(self) -> u8 {
        match self {
            WanderSpeed::Selector0 => 0,
            WanderSpeed::Selector1 => 1,
            WanderSpeed::Selector2 => 2,
        }
    }

    /// The cartridge's 8.8 velocity word.
    #[must_use]
    pub const fn velocity_8_8(self) -> u16 {
        match self {
            WanderSpeed::Selector0 => 0x0080,
            WanderSpeed::Selector1 => 0x0100,
            WanderSpeed::Selector2 => 0x0200,
        }
    }

    /// Frames for one `$1000` fixed-point cell.
    #[must_use]
    pub const fn frames(self) -> u8 {
        match self {
            WanderSpeed::Selector0 => 32,
            WanderSpeed::Selector1 => 16,
            WanderSpeed::Selector2 => 8,
        }
    }

    /// Converts a disassembly selector to a table record.
    #[must_use]
    pub const fn from_selector(selector: u8) -> Option<WanderSpeed> {
        match selector {
            0 => Some(WanderSpeed::Selector0),
            1 => Some(WanderSpeed::Selector1),
            2 => Some(WanderSpeed::Selector2),
            _ => None,
        }
    }
}

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
    /// `NPCType4` — `FieldObj_GetRandomMove3`, a fresh roll every idle frame.
    Type4,
    /// `NPCType28` — the same no-timer random routine, with an 8-cell leash.
    Type28,
    /// The routine at `loc_490B8`, `FieldObj_GetRandomMove`, 8-cell leash.
    StoreWoman,
    /// The routine at `loc_49746`, `FieldObj_GetRandomMove`, 4-cell leash.
    ClinicWoman,
    /// `FieldObj_Penguin`, `FieldObj_GetRandomMove`, 8-cell leash.
    Penguin,
    /// `FieldObj_Butterfly`, `FieldObj_GetRandomMove3`, 16-cell leash.
    Butterfly,
    /// `FieldObj_MuskCat`, `FieldObj_GetRandomMove`, 16-cell leash.
    MuskCat,
    /// `FieldObj_Xanafalgue`, random before the leader crosses `$100`, then
    /// its scripted leftward escape.
    Xanafalgue,
}

impl WanderKind {
    /// The mask the routine applies to the roll to get its pause.
    #[must_use]
    pub const fn pause_mask(self) -> u16 {
        match self {
            WanderKind::Type2
            | WanderKind::Type4
            | WanderKind::Type28
            | WanderKind::StoreWoman
            | WanderKind::ClinicWoman
            | WanderKind::Penguin
            | WanderKind::Butterfly
            | WanderKind::MuskCat
            | WanderKind::Xanafalgue => 0x3F,
            WanderKind::Type3 => 0x7F,
        }
    }

    /// Whether the routine calls `UpdateRNGSeed` on every idle frame instead
    /// of maintaining the usual `$28` pause timer.
    #[must_use]
    pub const fn rolls_every_idle(self) -> bool {
        matches!(
            self,
            WanderKind::Type4 | WanderKind::Type28 | WanderKind::Butterfly
        )
    }

    /// The initialization values written to `$3C..$3F` by the routine.
    #[must_use]
    pub const fn leash(self) -> Leash {
        match self {
            WanderKind::Type28 | WanderKind::StoreWoman | WanderKind::Penguin => Leash {
                x_max: 8,
                y_max: 8,
                x: 4,
                y: 4,
            },
            WanderKind::Butterfly | WanderKind::MuskCat => Leash {
                x_max: 16,
                y_max: 16,
                x: 8,
                y: 8,
            },
            WanderKind::Xanafalgue => Leash {
                x_max: 2,
                y_max: 2,
                x: 1,
                y: 1,
            },
            WanderKind::Type2 | WanderKind::Type3 | WanderKind::Type4 | WanderKind::ClinicWoman => {
                Leash {
                    x_max: 4,
                    y_max: 4,
                    x: 2,
                    y: 2,
                }
            }
        }
    }

    /// The speed selector used by this family. The packed random families all
    /// pass `d7 = 0`; selectors 1 and 2 remain available through
    /// [`WanderSpeed`] for the other field-object routines and replay data.
    #[must_use]
    pub const fn speed(self) -> WanderSpeed {
        WanderSpeed::Selector0
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
    /// A zeroed boundary record, used by object routines that never write a
    /// movement leash of their own.
    pub const ZERO: Leash = Leash {
        x_max: 0,
        y_max: 0,
        x: 0,
        y: 0,
    };

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

    /// Applies a cell delta using the cartridge's boundary gate.
    ///
    /// This is public because the bespoke field-object routines share the
    /// exact same `loc_4A3B6` gate as the ordinary random walkers.
    pub fn accepts_delta(&mut self, dx: i8, dy: i8) -> bool {
        self.accepts(dx, dy)
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
        Self::new_with_speed(npc_index, kind, kind.speed())
    }

    /// A wanderer with an explicit `FieldObj_UpdateStepDuration` selector.
    ///
    /// The packed random families all use selector 0, but the disassembly's
    /// speed table is shared by bespoke field-object routines. Keeping this
    /// constructor explicit lets those callers use selector 1 or 2 without
    /// duplicating the movement implementation.
    #[must_use]
    pub fn new_with_speed(npc_index: usize, kind: WanderKind, speed: WanderSpeed) -> Wanderer {
        Wanderer {
            npc_index,
            kind,
            leash: kind.leash(),
            timer: 0,
            step: None,
            frames: StepFrames::new(speed.frames()).unwrap_or(StepFrames::DEFAULT),
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

    /// The extracted speed-table record this object uses.
    #[must_use]
    pub const fn speed(&self) -> WanderSpeed {
        self.kind.speed()
    }

    /// Frames in one committed step.
    #[must_use]
    pub const fn step_frames(&self) -> u8 {
        self.frames.get()
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
        let objects: Vec<(usize, WanderKind, WanderSpeed)> = objects
            .iter()
            .map(|&(npc_index, kind)| (npc_index, kind, kind.speed()))
            .collect();
        Self::build_with_speeds(map, &objects)
    }

    /// Builds a set with an explicit speed-table selector per object.
    pub fn build_with_speeds(
        map: &FieldMap,
        objects: &[(usize, WanderKind, WanderSpeed)],
    ) -> Result<WanderSet, MapError> {
        let count = map.npcs().len();
        let mut wanderers = Vec::with_capacity(objects.len());
        for &(npc_index, kind, speed) in objects {
            if npc_index >= count {
                return Err(MapError::NpcIndexOutOfRange {
                    index: npc_index,
                    count,
                });
            }
            wanderers.push(Wanderer::new_with_speed(npc_index, kind, speed));
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
        self.tick_with_driver_pixels_opt(map, rolls, party, visible, None);
    }

    /// Advances every wanderer with the leader's cartridge pixel position.
    ///
    /// Xanafalgue's routine branches on `Character_1.curr_y_pos` and, after
    /// `$100`, leaves the random walker entirely: it walks left at `$FFFC`
    /// until `$1A0`, then sets its temporary event flag and clears its object
    /// slot. Ordinary families ignore the extra position and use the same
    /// shared RNG path as [`WanderSet::tick`].
    pub fn tick_with_driver_pixels(
        &mut self,
        map: &mut FieldMap,
        rolls: &mut impl Rolls,
        party: &[Cell],
        driver_pixels: (i32, i32),
        visible: impl Fn(usize) -> bool,
    ) {
        self.tick_with_driver_pixels_opt(map, rolls, party, visible, Some(driver_pixels));
    }

    fn tick_with_driver_pixels_opt(
        &mut self,
        map: &mut FieldMap,
        rolls: &mut impl Rolls,
        party: &[Cell],
        visible: impl Fn(usize) -> bool,
        driver_pixels: Option<(i32, i32)>,
    ) {
        for index in 0..self.wanderers.len() {
            if !visible(self.wanderers[index].npc_index) {
                continue;
            }
            if self.wanderers[index].kind == WanderKind::Xanafalgue
                && driver_pixels.is_some_and(|(_, y)| y > 0x100)
            {
                self.tick_xanafalgue_escape(index, map);
                continue;
            }
            self.tick_one(index, map, rolls, party);
        }
    }

    fn tick_xanafalgue_escape(&mut self, index: usize, map: &mut FieldMap) {
        let npc_index = self.wanderers[index].npc_index;
        let Some(npc) = map.npcs().get(npc_index).copied() else {
            return;
        };
        let base = crate::trigger::PixelPos::from_cell(npc.cell);
        let x = base.x + i32::from(npc.offset.x);
        let y = base.y + i32::from(npc.offset.y);
        if x < 0x1A0 {
            let _ = map.set_npc_active(npc_index, false);
            self.wanderers[index].step = None;
        } else {
            let _ = map.set_npc_pixel_position(npc_index, x - 4, y);
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
        self.wanderers[index].timer = if kind.rolls_every_idle() {
            0
        } else {
            (roll & kind.pause_mask()) as i16
        };
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

    /// Ticks the walker for one object slot, preserving map-object order when
    /// the runtime interleaves it with bespoke routines.
    pub fn tick_one_for_npc(
        &mut self,
        npc_index: usize,
        map: &mut FieldMap,
        rolls: &mut impl Rolls,
        party: &[Cell],
        driver_pixels: (i32, i32),
    ) {
        let Some(index) = self
            .wanderers
            .iter()
            .position(|wanderer| wanderer.npc_index == npc_index)
        else {
            return;
        };
        if self.wanderers[index].kind == WanderKind::Xanafalgue && driver_pixels.1 > 0x100 {
            self.tick_xanafalgue_escape(index, map);
        } else {
            self.tick_one(index, map, rolls, party);
        }
    }
}

#[cfg(test)]
#[path = "wander_tests.rs"]
mod tests;
