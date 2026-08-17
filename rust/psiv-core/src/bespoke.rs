//! Bespoke field-object routines from `FieldObjectsJmpTbl`.
//!
//! The ordinary random families live in [`crate::wander`]. This module keeps
//! the remaining field routines explicit: deterministic patterns, fixed
//! direction attempts whose leash makes them stationary, flag-gated walkers,
//! and actors whose only remaining work is scene or presentation state. A
//! routine is never silently treated as a generic random walker.

use crate::battle::Rolls;
use crate::error::MapError;
use crate::field::StepFrames;
use crate::geom::{Cell, Direction};
use crate::map::FieldMap;
use crate::{Leash, WanderSpeed};

const IDLE: u8 = 0xFF;

/// The event/temp bits read by field-object routines in this module.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BespokeFlags {
    /// `EventFlag_PrincipalConfession` (`$0C`).
    pub principal_confession: bool,
    /// `EventFlag_IgglanovaZema` (`$33`).
    pub igglanova_zema: bool,
    /// `EventFlag_Penguin` (`$8A`).
    pub penguin: bool,
    /// `EventFlag_MuskCats` (`$90`).
    pub musk_cats: bool,
    /// `EventFlag_InnerSanctuary` (`$96`).
    pub inner_sanctuary: bool,
    /// `TempEveFlag_EspMansionGuards` (`$1A`).
    pub esp_mansion_guards: bool,
}

/// A flag a bespoke routine branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BespokeFlag {
    /// Base event bank.
    PrincipalConfession,
    /// Base event bank.
    IgglanovaZema,
    /// Base event bank.
    Penguin,
    /// Base event bank.
    MuskCats,
    /// Base event bank.
    InnerSanctuary,
    /// Temporary event bank.
    EspMansionGuards,
}

impl BespokeFlag {
    const fn enabled(self, flags: BespokeFlags) -> bool {
        match self {
            BespokeFlag::PrincipalConfession => flags.principal_confession,
            BespokeFlag::IgglanovaZema => flags.igglanova_zema,
            BespokeFlag::Penguin => flags.penguin,
            BespokeFlag::MuskCats => flags.musk_cats,
            BespokeFlag::InnerSanctuary => flags.inner_sanctuary,
            BespokeFlag::EspMansionGuards => flags.esp_mansion_guards,
        }
    }
}

/// How a follow routine chooses its cardinal command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowTarget {
    /// `NPCType6`'s guard distance test.
    LeaderGuard,
    /// `NPCType7`'s ranch-owner distance test.
    LeaderRanch,
    /// The last occupied party slot, as `loc_4B366` does for FellowPenguin.
    PartyTail,
    /// The preceding secondary-object slot, as `loc_4A05E` does for the
    /// Aiedo store child.
    PreviousObject,
    /// A fixed secondary-object slot, as the Dezolis penguin follower uses.
    ObjectAt(usize),
}

/// The random helper used by a bespoke routine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BespokeRandom {
    /// `loc_49DCA`, used by Mouse and NPCType12.
    Mouse,
    /// `loc_49CEA`, gated by `EventFlag_IgglanovaZema`.
    Igglanova,
    /// `loc_49E20`, repeats until raw `& 7` is 3 or 4.
    FilteredCardinal,
    /// `loc_49EA4`, the Prisoner's down/left/right routine.
    Prisoner,
    /// `loc_49D34`, chooses one of eight deterministic command sequences.
    SequenceChoice,
}

/// The executable classification of a non-random field-object routine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BespokeKind {
    /// Position and animation only; no movement or RNG state is consumed.
    StaticAnimation,
    /// The routine takes input from the scene/field-object controller. The
    /// scene lane owns it, so the field tick deliberately does not synthesize
    /// input here.
    SceneDriven,
    /// The routine has deterministic presentation state not represented by
    /// the cell movement core (barrier beams, flicker, one-shot art, etc.).
    PresentationOnly,
    /// A fixed raw command, including routines whose boundary rejects it.
    Fixed {
        /// Raw command before `loc_4A150` (`1=up, 2=down, 3=left, 4=right`).
        command: u8,
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
    /// A deterministic command table terminated by `0xFF`.
    Pattern {
        /// Raw command sequence from the ROM.
        commands: &'static [u8],
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
    /// A bespoke random helper with a shared LCG stream.
    Random {
        /// Helper body.
        kind: BespokeRandom,
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
    /// A leader/object-relative deterministic routine.
    Follow {
        /// Which party position is the target.
        target: FollowTarget,
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
    /// A fixed command before a flag, then a deterministic pattern.
    FlaggedPattern {
        /// Branch flag.
        flag: BespokeFlag,
        /// Command while the flag is clear.
        before: u8,
        /// Command sequence after the flag is set.
        after: &'static [u8],
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
    /// A fixed command before a flag, then a follow routine.
    FlaggedFollow {
        /// Branch flag.
        flag: BespokeFlag,
        /// Command while the flag is clear.
        before: u8,
        /// Target after the flag is set.
        target: FollowTarget,
        /// Whether the follow branch is the flag-set branch.
        follow_when_set: bool,
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
    /// No movement while clear, then a random helper after the flag.
    FlaggedRandom {
        /// Branch flag.
        flag: BespokeFlag,
        /// Helper after the flag is set.
        kind: BespokeRandom,
        /// `FieldObj_UpdateStepDuration` selector.
        speed: WanderSpeed,
        /// Initial `$3C..$3F` boundary bytes.
        leash: Leash,
    },
}

impl BespokeKind {
    /// The initial boundary state written by this routine.
    #[must_use]
    pub const fn leash(self) -> Leash {
        match self {
            BespokeKind::StaticAnimation
            | BespokeKind::SceneDriven
            | BespokeKind::PresentationOnly => Leash::ZERO,
            BespokeKind::Fixed { leash, .. }
            | BespokeKind::Pattern { leash, .. }
            | BespokeKind::Random { leash, .. }
            | BespokeKind::Follow { leash, .. }
            | BespokeKind::FlaggedPattern { leash, .. }
            | BespokeKind::FlaggedFollow { leash, .. }
            | BespokeKind::FlaggedRandom { leash, .. } => leash,
        }
    }

    /// The speed-table selector used by the routine.
    #[must_use]
    pub const fn speed(self) -> WanderSpeed {
        match self {
            BespokeKind::StaticAnimation
            | BespokeKind::SceneDriven
            | BespokeKind::PresentationOnly => WanderSpeed::Selector0,
            BespokeKind::Fixed { speed, .. }
            | BespokeKind::Pattern { speed, .. }
            | BespokeKind::Random { speed, .. }
            | BespokeKind::Follow { speed, .. }
            | BespokeKind::FlaggedPattern { speed, .. }
            | BespokeKind::FlaggedFollow { speed, .. }
            | BespokeKind::FlaggedRandom { speed, .. } => speed,
        }
    }

    /// Whether the actor contributes object-state columns beyond its position.
    #[must_use]
    pub const fn reports_state(self) -> bool {
        matches!(
            self,
            BespokeKind::Fixed { .. }
                | BespokeKind::Pattern { .. }
                | BespokeKind::Random { .. }
                | BespokeKind::Follow { .. }
                | BespokeKind::FlaggedPattern { .. }
                | BespokeKind::FlaggedFollow { .. }
                | BespokeKind::FlaggedRandom { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BespokeStep {
    dir: Direction,
    from: Cell,
    progress: u8,
}

/// One placed bespoke actor and its routine-local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BespokeActor {
    npc_index: usize,
    kind: BespokeKind,
    leash: Leash,
    timer: i16,
    cursor: usize,
    choice: u8,
    step: Option<BespokeStep>,
    frames: u8,
}

impl BespokeActor {
    /// Creates an actor at a stable map-object index.
    #[must_use]
    pub fn new(npc_index: usize, kind: BespokeKind) -> BespokeActor {
        BespokeActor {
            npc_index,
            kind,
            leash: kind.leash(),
            timer: 0,
            cursor: 0,
            choice: 0,
            step: None,
            frames: StepFrames::new(kind.speed().frames())
                .unwrap_or(StepFrames::DEFAULT)
                .get(),
        }
    }

    /// The object slot this actor drives.
    #[must_use]
    pub const fn npc_index(&self) -> usize {
        self.npc_index
    }

    /// The transcribed routine classification.
    #[must_use]
    pub const fn kind(&self) -> BespokeKind {
        self.kind
    }

    /// Current boundary bytes.
    #[must_use]
    pub const fn leash(&self) -> Leash {
        self.leash
    }

    /// Countdown state used by a random helper.
    #[must_use]
    pub const fn timer(&self) -> i16 {
        self.timer
    }

    /// Whether the routine has a committed step in progress.
    #[must_use]
    pub const fn is_stepping(&self) -> bool {
        self.step.is_some()
    }

    /// The step progress, with `0` meaning idle.
    #[must_use]
    pub fn progress(&self) -> u8 {
        self.step.map_or(0, |step| step.progress)
    }

    /// The cell the current step started from.
    #[must_use]
    pub fn step_origin(&self) -> Option<Cell> {
        self.step.map(|step| step.from)
    }

    /// Signed whole-pixel travel from the origin cell.
    #[must_use]
    pub fn travelled_px(&self) -> (i32, i32) {
        let Some(step) = self.step else { return (0, 0) };
        let n = i32::from(step.progress) * crate::field::SUBCELL_UNITS;
        let frames = i32::from(self.frames);
        let (dx, dy) = step.dir.delta();
        ((dx * n).div_euclid(frames), (dy * n).div_euclid(frames))
    }

    /// The cartridge's remaining step-duration words.
    #[must_use]
    pub fn step_durations(&self) -> (u16, u16) {
        let Some(step) = self.step else { return (0, 0) };
        let per_frame = 0x1000 / u16::from(self.frames);
        let left = 0x1000u16.saturating_sub(u16::from(step.progress) * per_frame);
        match step.dir {
            Direction::Left | Direction::Right => (left, 0),
            Direction::Up | Direction::Down => (0, left),
        }
    }
}

/// Context visible to a field-object routine for one tick.
#[derive(Debug, Clone, Copy)]
pub struct BespokeContext<'a> {
    /// `Main_Frame_Count` after the vblank increment.
    pub frame: u16,
    /// Leader position in field pixels, matching `Character_1`.
    pub driver_pixels: (i32, i32),
    /// Party cells in retail slot order.
    pub party: &'a [Cell],
    /// Event/temp bits used by the bespoke routines.
    pub flags: BespokeFlags,
}

/// Every remaining bespoke actor on the current map, in object-slot order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BespokeSet {
    actors: Vec<BespokeActor>,
}

impl BespokeSet {
    /// Builds actors from `(npc_index, routine)` pairs.
    pub fn build(map: &FieldMap, objects: &[(usize, BespokeKind)]) -> Result<BespokeSet, MapError> {
        let count = map.npcs().len();
        let mut actors = Vec::with_capacity(objects.len());
        for &(npc_index, kind) in objects {
            if npc_index >= count {
                return Err(MapError::NpcIndexOutOfRange {
                    index: npc_index,
                    count,
                });
            }
            actors.push(BespokeActor::new(npc_index, kind));
        }
        Ok(BespokeSet { actors })
    }

    /// Actors in cartridge object order.
    #[must_use]
    pub fn actors(&self) -> &[BespokeActor] {
        &self.actors
    }

    /// Finds the actor for one map-object slot.
    #[must_use]
    pub fn get(&self, npc_index: usize) -> Option<&BespokeActor> {
        self.actors
            .iter()
            .find(|actor| actor.npc_index == npc_index)
    }

    /// Whether this map has no bespoke actors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// Applies interaction-bit changes that happen before the visibility test.
    pub fn sync_flags(&self, map: &mut FieldMap, flags: BespokeFlags) {
        for actor in &self.actors {
            if let BespokeKind::FlaggedFollow {
                flag: BespokeFlag::Penguin,
                follow_when_set: true,
                ..
            } = actor.kind
            {
                let _ = map.set_npc_interactable(actor.npc_index, !flags.penguin);
            }
        }
    }

    /// Ticks one actor. The caller supplies visibility and invokes this in
    /// ascending map-object index order so all RNG consumers share the one
    /// cartridge stream.
    pub fn tick_one_for_npc(
        &mut self,
        npc_index: usize,
        map: &mut FieldMap,
        rolls: &mut impl Rolls,
        context: BespokeContext<'_>,
    ) {
        let Some(index) = self
            .actors
            .iter()
            .position(|actor| actor.npc_index == npc_index)
        else {
            return;
        };
        if !map.npcs().get(npc_index).is_some_and(|npc| npc.active) {
            return;
        }

        if self.actors[index].step.is_some() {
            let actor = &mut self.actors[index];
            if let Some(step) = actor.step.as_mut() {
                step.progress = step.progress.saturating_add(1);
                if step.progress >= actor.frames {
                    actor.step = None;
                }
            }
            return;
        }

        let raw = self.next_command(index, map, rolls, context);
        let Some(dir) = direction_for_raw(raw) else {
            return;
        };
        let actor = &mut self.actors[index];
        let _ = map.set_npc_facing(npc_index, dir);
        let (dx, dy) = dir.delta();
        let uses_leash = match actor.kind {
            BespokeKind::FlaggedFollow {
                flag,
                target: FollowTarget::PartyTail,
                follow_when_set,
                ..
            } => flag.enabled(context.flags) != follow_when_set,
            _ => true,
        };
        if uses_leash && !actor.leash.accepts_delta(dx as i8, dy as i8) {
            return;
        }
        let Some(from) = map.npcs().get(npc_index).map(|npc| npc.cell) else {
            return;
        };
        let Some(to) = map.neighbor(from, dir) else {
            return;
        };
        if map.grid().is_blocking(to)
            || context.party.contains(&to)
            || map.npc_at(to).is_some_and(|npc| npc.cell != from)
        {
            return;
        }
        if map.set_npc_cell(npc_index, to).is_ok() {
            actor.step = (1 < actor.frames).then_some(BespokeStep {
                dir,
                from,
                progress: 1,
            });
        }
    }

    fn next_command(
        &mut self,
        index: usize,
        map: &FieldMap,
        rolls: &mut impl Rolls,
        context: BespokeContext<'_>,
    ) -> u8 {
        let kind = self.actors[index].kind;
        match kind {
            BespokeKind::StaticAnimation
            | BespokeKind::SceneDriven
            | BespokeKind::PresentationOnly => 0,
            BespokeKind::Fixed { command, .. } => command,
            BespokeKind::Pattern { commands, .. } => self.next_pattern(index, commands),
            BespokeKind::Random { kind, .. } => self.next_random(index, kind, rolls, context),
            BespokeKind::Follow { target, .. } => {
                follow_command(map, self.actors[index].npc_index, target, context)
            }
            BespokeKind::FlaggedPattern {
                flag,
                before,
                after,
                ..
            } => {
                if flag.enabled(context.flags) {
                    self.next_pattern(index, after)
                } else {
                    before
                }
            }
            BespokeKind::FlaggedFollow {
                flag,
                before,
                target,
                follow_when_set,
                ..
            } => {
                if flag.enabled(context.flags) == follow_when_set {
                    follow_command(map, self.actors[index].npc_index, target, context)
                } else {
                    before
                }
            }
            BespokeKind::FlaggedRandom { flag, kind, .. } => {
                if flag.enabled(context.flags) {
                    self.next_random(index, kind, rolls, context)
                } else {
                    0
                }
            }
        }
    }

    fn next_pattern(&mut self, index: usize, commands: &'static [u8]) -> u8 {
        if commands.is_empty() {
            return 0;
        }
        let actor = &mut self.actors[index];
        let command = commands[actor.cursor % commands.len()];
        actor.cursor += 1;
        if command == IDLE {
            actor.cursor = 0;
            0
        } else {
            command
        }
    }

    fn next_random(
        &mut self,
        index: usize,
        kind: BespokeRandom,
        rolls: &mut impl Rolls,
        context: BespokeContext<'_>,
    ) -> u8 {
        match kind {
            BespokeRandom::Mouse => {
                let actor = &mut self.actors[index];
                actor.timer -= 1;
                if actor.timer >= 0 {
                    return 0;
                }
                if context.frame & 3 == 0 {
                    let roll = rolls.next_roll();
                    actor.timer = 0x20;
                    if roll == 0 { 1 } else { 2 }
                } else if rolls.next_roll() & 1 == 0 {
                    4
                } else {
                    3
                }
            }
            BespokeRandom::Igglanova => {
                let actor = &mut self.actors[index];
                actor.timer -= 1;
                if actor.timer >= 0 {
                    return 0;
                }
                let roll = rolls.next_roll();
                actor.timer = (roll & 0x3F) as i16;
                (roll & 7) as u8
            }
            BespokeRandom::FilteredCardinal => loop {
                let roll = rolls.next_roll();
                self.actors[index].timer = (roll & 0x3F) as i16;
                let command = (roll & 7) as u8;
                if command == 3 || command == 4 {
                    break command;
                }
            },
            BespokeRandom::Prisoner => {
                if rolls.next_roll() & 0x3F != 0 {
                    2
                } else if self.actors[index].choice != 0 {
                    self.actors[index].choice = 0;
                    4
                } else {
                    self.actors[index].choice = 1;
                    3
                }
            }
            BespokeRandom::SequenceChoice => {
                if self.actors[index].cursor == 0 {
                    self.actors[index].choice = (rolls.next_roll() & 7) as u8;
                }
                let sequence = TYPE11_CHOICES[usize::from(self.actors[index].choice)];
                self.next_pattern(index, sequence)
            }
        }
    }
}

fn direction_for_raw(raw: u8) -> Option<Direction> {
    match raw {
        1 => Some(Direction::Up),
        2 => Some(Direction::Down),
        3 => Some(Direction::Left),
        4 => Some(Direction::Right),
        _ => None,
    }
}

fn follow_command(
    map: &FieldMap,
    npc_index: usize,
    target: FollowTarget,
    context: BespokeContext<'_>,
) -> u8 {
    let Some(npc) = map.npcs().get(npc_index) else {
        return 0;
    };
    let target_pixels = match target {
        FollowTarget::LeaderGuard | FollowTarget::LeaderRanch => context.driver_pixels,
        FollowTarget::PartyTail => context.party.last().map_or(context.driver_pixels, |cell| {
            (i32::from(cell.x) * 16, (i32::from(cell.y) - 1) * 16)
        }),
        FollowTarget::PreviousObject | FollowTarget::ObjectAt(_) => {
            let target_index = match target {
                FollowTarget::PreviousObject => npc_index.checked_sub(1),
                FollowTarget::ObjectAt(index) => Some(index),
                _ => None,
            };
            let Some(target_index) = target_index else {
                return 0;
            };
            let Some(target_npc) = map.npcs().get(target_index) else {
                return 0;
            };
            let (dx, dy) = target_npc.facing.opposite().delta();
            (
                (i32::from(target_npc.cell.x) + dx) * 16,
                (i32::from(target_npc.cell.y) - 1 + dy) * 16,
            )
        }
    };
    let object_pixels = (
        i32::from(npc.cell.x) * 16 + i32::from(npc.offset.x),
        (i32::from(npc.cell.y) - 1) * 16 + i32::from(npc.offset.y),
    );
    let dy = object_pixels.1 - target_pixels.1;
    let dx = object_pixels.0 - target_pixels.0;
    let distance = |value: i32| value.unsigned_abs() <= 16;
    match target {
        FollowTarget::LeaderGuard => {
            if !(0..=16).contains(&dy) || dx == 0 || !distance(dx) {
                1
            } else if dx > 0 {
                3
            } else {
                4
            }
        }
        FollowTarget::LeaderRanch => {
            if dy.unsigned_abs() > 16 || dx == 0 || !distance(dx) {
                2
            } else if dx > 0 {
                3
            } else {
                4
            }
        }
        FollowTarget::PartyTail | FollowTarget::PreviousObject | FollowTarget::ObjectAt(_) => {
            if dx != 0 {
                if horizontal_step(map, object_pixels.0, target_pixels.0) < 0 {
                    3
                } else {
                    4
                }
            } else if vertical_step(map, object_pixels.1, target_pixels.1) > 0 {
                2
            } else if vertical_step(map, object_pixels.1, target_pixels.1) < 0 {
                1
            } else {
                0
            }
        }
    }
}

fn horizontal_step(map: &FieldMap, current: i32, target: i32) -> i32 {
    axis_step(map, current, target, i32::from(map.width()) * 16)
}

fn vertical_step(map: &FieldMap, current: i32, target: i32) -> i32 {
    axis_step(map, current, target, i32::from(map.height()) * 16)
}

fn axis_step(map: &FieldMap, current: i32, target: i32, period: i32) -> i32 {
    let direct = target - current;
    if direct == 0 {
        return 0;
    }
    if !map.wraps() {
        return direct.signum();
    }
    let wrapped = period - direct.abs();
    if wrapped <= direct.abs() {
        -direct.signum()
    } else {
        direct.signum()
    }
}

const TYPE11_CHOICES: [&[u8]; 8] = [
    &[3, 3, 3, 3, 3, IDLE],
    &[4, 4, 4, 4, 4, 4, 4, IDLE],
    &[1, 3, 3, 3, 3, IDLE],
    &[1, 1, 4, 4, 4, IDLE],
    &[2, 2, 3, 3, 3, 3, 3, IDLE],
    &[2, 4, 4, 4, 4, IDLE],
    &[1, 1, 1, 1, 1, IDLE],
    &[2, 2, 2, 2, 2, IDLE],
];

/// `loc_49BDA`, Rocky and NPCType5.
pub const PATTERN_TYPE5: &[u8] = &[1, 1, 4, 4, 2, 2, 3, 3, IDLE];
/// `loc_49E5C`, NPCType17.
pub const PATTERN_TYPE17: &[u8] = &[2, 2, 2, 4, 4, 4, 1, 1, 1, 3, 3, 3, IDLE];
/// `loc_49EE0`, NPCType35.
pub const PATTERN_TYPE35: &[u8] = &[4, 4, 4, 4, 4, 4, 2, 2, 2, 3, 3, 3, 3, 3, 3, 1, 1, 1, IDLE];
/// `loc_49F2A`, NPCType36.
pub const PATTERN_TYPE36: &[u8] = &[4, 4, 4, 4, 2, 2, 2, 3, 3, 3, 3, 3, 3, 1, 1, 1, 4, 4, IDLE];
/// `loc_49F74`, the small loop at `loc_48F36`.
pub const PATTERN_48F36: &[u8] = &[
    4, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 4, 3, 3, 3, IDLE,
];
/// `loc_49FC0`, the long fixed route at `loc_49128`.
pub const PATTERN_49128: &[u8] = &[
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 1, 3, 3, 3, 2, 2, 4, 4, 4,
    4, 4, 4, 1, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1, 1, 4, IDLE,
];
/// `loc_4A022`, used by MuskCatGuard after its event flag.
pub const PATTERN_MUSK_GUARD: &[u8] = &[3, 3, 2, IDLE];
/// `loc_4A0A4`/`loc_4A0F4`, the Esper guard route after permission.
pub const PATTERN_ESPER_GUARD: &[u8] = &[3, 2, IDLE];

#[cfg(test)]
#[path = "bespoke_tests.rs"]
mod tests;
