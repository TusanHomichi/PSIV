//! The scene interpreter: scripted cutscenes as data.
//!
//! The cartridge's ~162 `Event_*` routines are hand-written 68k, but composed
//! almost entirely from a small vocabulary — `Event_UpdateObjFacing`,
//! `Event_GetAndRunDialogue`, `Event_MoveCamera`, `Event_AddMacro`,
//! move-object-and-wait loops, `Current_Party_Slots` writes, flag sets, NPC
//! despawns. This module implements that vocabulary once as [`SceneOp`] and
//! runs sequences of it deterministically.
//!
//! # Actor motion is the same movement model as everything else
//!
//! A scripted walk is not a special animation: the cartridge's
//! `Event_MoveSingleObject` / `Event_MoveCharacters` set an object's
//! `dest_x_pos`/`dest_y_pos` and then call the ordinary object update until
//! `curr == dest`. The object's direction comes from `FieldObj_GetAutoInput`
//! (`ps4.asm:93232`), which compares current against destination one axis at a
//! time — X first unless `Char_Move_Flags` bit 1 is set. So a scene actor
//! walks in whole cell-steps at the same rate as the party, and turns to face
//! the way it walks. [`ScriptedActor`] is exactly that.
//!
//! # Blocking
//!
//! The runner executes ops until it reaches one that blocks — a wait, a
//! dialogue, a choice, or the end. Everything non-blocking in between runs in
//! the same tick, which is what makes a "set three flags and move the camera"
//! run of ops take one tick rather than three.
//!
//! Dialogue is the interesting one: the runner emits
//! [`SceneEffect::DialogueOpen`] and then stops advancing until the runtime
//! reports the window closed with [`SceneInput::DialogueClosed`]. The engine
//! neither draws nor times the window; it only knows the script is waiting.

use crate::field::StepFrames;
use crate::geom::{Cell, Direction};
use crate::map::FieldMap;
use crate::state::{CharId, Flag, PARTY_SLOTS};

/// How many ops one tick may execute before the runner assumes the script is
/// looping and faults. Generous for real scenes, finite for broken ones.
pub const OP_BUDGET_PER_TICK: usize = 1024;

/// Which coordinate a comparison reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis {
    /// `curr_x_pos`.
    X,
    /// `curr_y_pos`.
    Y,
}

/// Who a scene op is talking about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActorRef {
    /// A party member by slot; 0 is the leader.
    PartyMember(usize),
    /// A field object by its index in [`FieldMap::npcs`].
    ///
    /// Scene-lane proved these live at field-object RAM `$FFFFC300 + N*$40`,
    /// so "NPC index N" is an unambiguous handle into the current map.
    Npc(usize),
    /// A character, resolved to whichever field object currently holds them
    /// via the party slots — the `Event_GetCharacter` (`$5A6D6`) primitive.
    /// Lets a scene name Chaz or Alys instead of guessing a slot.
    Character(CharId),
}

/// A dialogue tree entry to open. The engine carries no text — the id is
/// resolved against the pack by the runtime.
///
/// `GetDialogueByID` (`$59164`) skips `d0` `$FF` terminators through whichever
/// tree was decompressed to `$FFFF3000`, so an entry index is only meaningful
/// against the *current* tree. [`SceneOp::SetDialogueTree`] changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DialogueId(pub u16);

/// Where a dialogue entry index comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DialogueSource {
    /// A literal entry index written into the scene.
    Entry(DialogueId),
    /// Read `dialogue_id` (`$14`) off a field object. `Event_AlysFound` does
    /// exactly this — `move.b $14(a4),d0` — so the line Alys speaks is a
    /// property of her map object, not of the scene.
    NpcDialogueId(ActorRef),
}

/// One instruction of a scene.
///
/// Jump targets are op indices into the same scene slice, so a scene is
/// self-contained data with no labels to resolve at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneOp {
    /// Start `actor` walking to `to`. Does **not** block; pair it with
    /// [`SceneOp::WaitForActor`] when the script should wait.
    MoveActor {
        /// Who walks.
        actor: ActorRef,
        /// Where to.
        to: Cell,
    },
    /// Turn `actor` in place.
    Face {
        /// Who turns.
        actor: ActorRef,
        /// Which way.
        facing: Direction,
    },
    /// Open a dialogue and block until the runtime closes it.
    RunDialogue {
        /// Which entry, and where its index comes from.
        source: DialogueSource,
    },
    /// Resume the dialogue where `RunText` left off.
    ///
    /// `RunText` always records its stopping point in `Saved_Dialogue_Addr`
    /// (`$ECF0`); this is the clone's `popdlg` idiom. Blocks like
    /// [`SceneOp::RunDialogue`].
    RunDialogueResume,
    /// Point the dialogue system at a different tree (`DialogueTreesToRAM`,
    /// `$53F00`). Entry indices after this are relative to the new tree.
    SetDialogueTree {
        /// The tree's ROM address, as the transcription records it.
        rom_addr: u32,
    },
    /// Set or clear an event flag.
    SetFlag {
        /// Which flag.
        flag: Flag,
        /// The value to write.
        value: bool,
    },
    /// Put a character in a party slot.
    JoinParty {
        /// Which slot, 0-based.
        slot: usize,
        /// Who joins.
        who: CharId,
    },
    /// Replace the whole party at once — the shape `Event_AlysFound` uses.
    SetParty {
        /// The five slots.
        slots: [Option<CharId>; PARTY_SLOTS],
    },
    /// Remove a field object from the map. The runtime rebuilds the
    /// [`FieldMap`] without it.
    DespawnNpc {
        /// Which object.
        npc_index: usize,
    },
    /// Move the camera to a pixel position at `speed`.
    ///
    /// `Event_MoveCamera` (`$5AAEE`) subtracts `$98`/`$58` for the half-screen
    /// offset itself, so these are the values the scene passes, untouched.
    MoveCamera {
        /// Target X in pixels.
        x: i32,
        /// Target Y in pixels.
        y: i32,
        /// Scroll speed.
        speed: u16,
    },
    /// Walk an actor to a pixel position — `Event_MoveCharacters` (`$5AA84`,
    /// the party, followers in tow) or `Event_MoveSingleObject` (`$5A9FC`).
    MoveActorTo {
        /// Who walks.
        actor: ActorRef,
        /// Target X in pixels.
        x: i32,
        /// Target Y in pixels.
        y: i32,
    },
    /// Drive an actor with one of the NPC movement-command bytes and
    /// optionally spin until the step completes — the `AlysFound` idiom of
    /// calling the NPC's own field routine with `d0 = command`.
    ///
    /// The command byte indexes the cartridge's NPC movement-command table,
    /// which this crate does not carry, so the runner reports it rather than
    /// executing it. Prefer [`SceneOp::MoveActorTo`] where a transcription can
    /// express the move as a destination.
    MoveActorCommand {
        /// Who moves.
        actor: ActorRef,
        /// The movement-command byte.
        command: u8,
        /// Whether the scene spins until the step finishes.
        wait: bool,
    },
    /// Collapse the followers onto the leader (`Event_OverlapCharacters`,
    /// `$5A87A`).
    OverlapCharacters,
    /// Turn an NPC into a party character: the `trap #1` struct copy plus
    /// field-object construction that makes NPC-Alys become field-Alys.
    PromoteNpcToChar {
        /// The map object being promoted.
        npc: usize,
        /// Who they become.
        char_id: CharId,
        /// Which party slot they take.
        slot: usize,
        /// The object's art tile.
        art_tile: u16,
        /// Their facing on arrival.
        facing: Direction,
    },
    /// Add to the purse (`Current_Money`).
    AddMoney {
        /// How much.
        amount: u32,
    },
    /// Hand control to a battle (`Event_Battle_Index` + the routine-exit bit).
    StartBattle {
        /// The event battle index.
        index: u16,
    },
    /// Write `Char_Move_Flags` (`$ECFE`): bit 0 disables the follow chain,
    /// bit 1 picks Y-first auto-pathing, bit 2 locks the camera.
    SetFollowMode {
        /// The raw flag bits.
        bits: u8,
    },
    /// Write `FieldObj_Step_Offset` (`$ECE0`): 0 slower, 1 normal, 2 faster.
    SetStepOffset {
        /// The raw value.
        value: u8,
    },
    /// Play a sound, music track, or the stop-music byte — one `Sound_Index`
    /// write covers all three.
    PlaySound {
        /// The sound id.
        id: u8,
    },
    /// Set `Saved_Sound_Index`, the track restored after an interruption.
    SetSavedMusic {
        /// The sound id.
        id: u8,
    },
    /// Fade the palette in (`Pal_FadeIn`, `$421D4`).
    FadeIn,
    /// Fade the palette out (`PalFadeOut_ClrSpriteTbl`, `$4223E`).
    FadeOut,
    /// Load a map and place the party — the scene-driven counterpart to a warp.
    LoadMap {
        /// The map to load.
        map: u16,
        /// What `Field_Map_Index` should report as the previous map.
        prev_map: u16,
        /// `Map_Start_X_Pos`, in 8-pixel units as the loader stores it.
        start_x: u16,
        /// `Map_Start_Y_Pos`, in 8-pixel units.
        start_y: u16,
        /// `Map_Start_Facing_Dir`.
        facing: Direction,
        /// `Map_Start_Char_Align`.
        align: u8,
    },
    /// Whether sprites render during a cutscene
    /// (`Render_Sprites_In_Cutscenes`, `$ECFD`).
    SetRenderSpritesInCutscene {
        /// The new value.
        enabled: bool,
    },
    /// Wait without running a map update — `VInt_PrepareLoop` (`$5A7AC`),
    /// which is a *different* wait from [`SceneOp::Wait`].
    WaitFrames {
        /// How many frames.
        frames: u16,
    },
    /// Block for a fixed number of ticks, running map updates
    /// (`DoMapUpdateLoop`, `$5A71E`).
    ///
    /// The retail loop is a `dbra`, so a scene's literal `d0` runs `d0 + 1`
    /// times; transcriptions record the **already-corrected** tick count.
    Wait {
        /// How many.
        ticks: u16,
    },
    /// Block until `actor` finishes walking.
    WaitForActor {
        /// Who to wait for.
        actor: ActorRef,
    },
    /// Jump depending on a flag.
    BranchFlag {
        /// Which flag.
        flag: Flag,
        /// Op index taken when set.
        if_set: usize,
        /// Op index taken when clear.
        if_clear: usize,
    },
    /// Ask the player a yes/no question and jump on the answer. Blocks until
    /// [`SceneInput::Choice`] arrives.
    BranchChoice {
        /// Op index taken on yes.
        if_yes: usize,
        /// Op index taken on no.
        if_no: usize,
    },
    /// Jump on whether two actors share a coordinate.
    ///
    /// `Event_AlysFound` opens with `cmp.w $34(a3),d0` — Chaz's `curr_y_pos`
    /// against Alys's — to skip the alignment step when she is already level
    /// with him. Scene-lane flagged this as the act's only *position* branch
    /// and asked that [`SceneOp::BranchFlag`] not be silently widened to cover
    /// it, so it gets its own op.
    BranchIfAligned {
        /// One actor.
        a: ActorRef,
        /// The other.
        b: ActorRef,
        /// Which coordinate to compare.
        axis: Axis,
        /// Op index taken when the coordinates match.
        if_aligned: usize,
        /// Op index taken otherwise.
        if_not: usize,
    },
    /// Copy one character *field object's* whole struct over another — the
    /// `trap #1` 32-word copy that relocates Chaz's on-map object from
    /// `Character_1` to `Character_2`.
    ///
    /// This moves the **object**, not the party slot. The two are separate in
    /// the cartridge: `Current_Party_Slots` says who is in the party and the
    /// `$C000`-series objects are their on-map bodies. `Event_AlysFound`
    /// writes the slots first and copies the struct second, and conflating
    /// them silently overwrites whoever the slot write just installed.
    CopyCharSlot {
        /// Source character-object index (0 = `Character_1`).
        from: usize,
        /// Destination character-object index.
        to: usize,
    },
    /// Set a field object's art tile (`$16`).
    SetArtTile {
        /// Whose.
        actor: ActorRef,
        /// The VRAM tile number.
        tile: u16,
    },
    /// Unconditional jump.
    Jump {
        /// Op index.
        to: usize,
    },
    /// Finish the scene.
    End,
}

/// What the runtime tells the runner between ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneInput {
    /// Nothing happened.
    #[default]
    None,
    /// The dialogue window the runner asked for has closed.
    DialogueClosed,
    /// The player answered the pending choice.
    Choice(bool),
}

/// Something the runtime must act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneEffect {
    /// Open this dialogue. The runner is blocked until
    /// [`SceneInput::DialogueClosed`].
    DialogueOpen(DialogueId),
    /// Ask the pending yes/no question. Blocked until [`SceneInput::Choice`].
    ChoiceRequested,
    /// Open the dialogue whose entry index lives on a field object's
    /// `dialogue_id` byte. The runtime reads it from the map record.
    DialogueOpenFromNpc {
        /// Whose `dialogue_id` to read.
        actor: ActorRef,
    },
    /// Resume the dialogue from `Saved_Dialogue_Addr`.
    DialogueResume,
    /// One character slot's struct was copied over another.
    CharSlotCopied {
        /// Source slot.
        from: usize,
        /// Destination slot.
        to: usize,
    },
    /// An NPC became a party character.
    NpcPromoted {
        /// The map object that was promoted.
        npc: usize,
        /// Who they became.
        char_id: CharId,
        /// The slot they took.
        slot: usize,
        /// Their art tile.
        art_tile: u16,
        /// Their facing.
        facing: Direction,
    },
    /// An actor began walking to `to`.
    ActorMoveStarted {
        /// Who.
        actor: ActorRef,
        /// Their destination.
        to: Cell,
    },
    /// An actor arrived.
    ActorArrived {
        /// Who.
        actor: ActorRef,
        /// Where.
        at: Cell,
    },
    /// An actor turned in place.
    ActorFaced {
        /// Who.
        actor: ActorRef,
        /// Which way.
        facing: Direction,
    },
    /// A flag changed. The runtime rebuilds anything flag-gated — layout
    /// patches, NPC despawns, `MapDataManager` effects.
    FlagChanged {
        /// Which flag.
        flag: Flag,
        /// Its new value.
        value: bool,
    },
    /// The party composition changed.
    PartyChanged,
    /// Drop this object from the map.
    NpcDespawned {
        /// Which object.
        npc_index: usize,
    },
    /// The purse changed.
    MoneyChanged {
        /// The new total.
        total: u32,
    },
    /// Hand control to a battle; the scene stays blocked until the runtime
    /// resumes it.
    BattleRequested {
        /// The event battle index.
        index: u16,
    },
    /// Load a map. The runtime rebuilds the [`FieldMap`] and places the party.
    MapRequested {
        /// The op as transcribed, with every field the loader needs.
        op: SceneOp,
    },
    /// An op the engine has no state for: camera, palette, sound, fades, the
    /// intro's bespoke text presentation. Carried as the op itself so the
    /// runtime matches on it directly rather than through a parallel enum that
    /// would have to be kept in step.
    Presentation {
        /// The op that ran.
        op: SceneOp,
    },
    /// The scene ended.
    Finished,
    /// The scene could not continue. Emitted instead of panicking or looping
    /// forever; the runner stops.
    Faulted(SceneFault),
}

/// Why a scene stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneFault {
    /// A jump or branch target was past the end of the scene.
    BadJump {
        /// The offending target.
        target: usize,
    },
    /// An op named an actor the scene never declared.
    UnknownActor {
        /// The offending reference.
        actor: ActorRef,
    },
    /// A flag or party write was rejected.
    BadWrite,
    /// The op budget ran out — almost certainly a jump loop with no wait in it.
    Runaway,
}

/// A scene actor: a cell position, a facing, and a walk in progress.
///
/// Movement is the auto-input model: close the X gap first, then the Y gap,
/// one whole cell-step at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptedActor {
    /// Who this is.
    pub actor: ActorRef,
    /// Current cell.
    pub cell: Cell,
    /// Current facing.
    pub facing: Direction,
    /// Where it is walking, if anywhere.
    pub target: Option<Cell>,
    step: Option<(Direction, Cell, u8)>,
}

impl ScriptedActor {
    /// A stationary actor.
    #[must_use]
    pub const fn new(actor: ActorRef, cell: Cell, facing: Direction) -> ScriptedActor {
        ScriptedActor {
            actor,
            cell,
            facing,
            target: None,
            step: None,
        }
    }

    /// Whether it is mid-step.
    #[must_use]
    pub const fn is_stepping(&self) -> bool {
        self.step.is_some()
    }

    /// Whether it still has walking to do.
    #[must_use]
    pub const fn is_walking(&self) -> bool {
        self.target.is_some() || self.step.is_some()
    }

    /// Sub-cell displacement in sixteenths, as [`crate::FieldState`] reports it.
    #[must_use]
    pub fn render_offset_16ths(&self, frames: StepFrames) -> (i32, i32) {
        let Some((dir, _, progress)) = self.step else {
            return (0, 0);
        };
        let travelled = i32::from(progress) * crate::field::SUBCELL_UNITS / i32::from(frames.get());
        let (dx, dy) = dir.delta();
        (dx * travelled, dy * travelled)
    }

    /// The direction that closes the gap to `target`: X first, then Y.
    fn direction_toward(&self, target: Cell) -> Option<Direction> {
        if target.x != self.cell.x {
            return Some(if target.x > self.cell.x {
                Direction::Right
            } else {
                Direction::Left
            });
        }
        if target.y != self.cell.y {
            return Some(if target.y > self.cell.y {
                Direction::Down
            } else {
                Direction::Up
            });
        }
        None
    }

    /// Advances one tick. Returns the cell arrived at when a walk finishes.
    ///
    /// Crate-internal: the runner drives this, callers read the actor instead.
    pub(crate) fn tick(&mut self, map: &FieldMap, frames: StepFrames) -> Option<Cell> {
        if self.step.is_none() {
            let target = self.target?;
            match self.direction_toward(target) {
                Some(dir) => {
                    self.facing = dir;
                    // Scene actors walk where the script says. Collision is not
                    // consulted: the cartridge's scripted moves write dest and
                    // drive the object there, and several scenes deliberately
                    // walk actors across cells the player could not.
                    let to = map.neighbor(self.cell, dir).unwrap_or(self.cell);
                    self.step = Some((dir, to, 0));
                }
                None => {
                    self.target = None;
                    return Some(self.cell);
                }
            }
        }

        let (dir, to, progress) = self.step?;
        let progress = progress.saturating_add(1);
        if progress < frames.get() {
            self.step = Some((dir, to, progress));
            return None;
        }
        self.step = None;
        self.cell = to;
        if self.target == Some(self.cell) {
            self.target = None;
            return Some(self.cell);
        }
        None
    }
}
