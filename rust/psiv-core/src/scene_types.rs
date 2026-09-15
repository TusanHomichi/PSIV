//! Inputs, effects, and faults shared by the scene interpreter and runtime.
//!
//! Kept apart from the scene instruction vocabulary so adding a retail
//! presentation record does not turn the core's public contract into a
//! thousand-line monolith.

use crate::geom::{Cell, Direction};
use crate::scene::{ActorRef, DialogueId, SceneOp};
use crate::state::{CharId, Flag, PARTY_SLOTS};

/// What the runtime tells the runner between ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneInput {
    /// Nothing happened.
    #[default]
    None,
    /// The dialogue window the runner asked for has closed.
    DialogueClosed,
    /// The text reached its final FF terminator, rather than yielding at F7.
    DialogueEnded,
    /// The player answered the pending choice.
    Choice(bool),
    /// The battle requested by the scene has ended.
    BattleFinished {
        /// The result reported by the battle engine.
        outcome: crate::battle::Outcome,
    },
    /// The runtime completed a scene-requested map load and recast the runner.
    MapLoaded,
    /// The ending's retail `Joypad_Pressed` loop received Start.
    EndingContinue,
}

/// Something the runtime must act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneEffect {
    /// Open this dialogue. The runner waits for [`SceneInput::DialogueClosed`].
    DialogueOpen(DialogueId),
    /// Ask the pending yes/no question.
    ChoiceRequested,
    /// Open the dialogue whose entry index lives on a field object.
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
    /// A flag changed.
    FlagChanged {
        /// Which flag.
        flag: Flag,
        /// Its new value.
        value: bool,
    },
    /// The party composition changed.
    PartyChanged,
    /// The scene copied its party slots to retail's transient save area.
    PartySlotsSaved {
        /// The five captured slots.
        slots: [Option<CharId>; PARTY_SLOTS],
    },
    /// Restore retail's transient party-slot area.
    PartySlotsRestored,
    /// The inventory changed.
    InventoryChanged,
    /// A scene explicitly restores these chunks and refreshes the map plane.
    MapChunksRestored {
        /// (Chunk x, chunk y, expected base chunk id).
        chunks: &'static [(u32, u32, u16)],
    },
    /// A scene writes the named chunks into the live map layout.
    MapChunksWritten {
        /// (Chunk x, chunk y, replacement chunk id).
        chunks: Vec<(u32, u32, u16)>,
    },
    /// The selected vehicle changed.
    VehicleChanged {
        /// The new vehicle id.
        index: u16,
    },
    /// A roster record changed outside a battle.
    RosterChanged {
        /// The character whose record was written.
        who: CharId,
    },
    /// Drop objects from the map.
    NpcDespawned {
        /// The first object cleared.
        npc_index: usize,
        /// How many consecutive objects.
        count: usize,
    },
    /// An actor was placed outright.
    ActorPlaced {
        /// Who.
        actor: ActorRef,
        /// Where.
        at: Cell,
    },
    /// The scene returned a value.
    Returned {
        /// The `d0` value.
        value: u16,
    },
    /// The purse changed.
    MoneyChanged {
        /// The new total.
        total: u32,
    },
    /// Hand control to a battle; the scene remains blocked until resumed.
    BattleRequested {
        /// The event battle index.
        index: u16,
    },
    /// Load a map and rebuild the field.
    MapRequested {
        /// The transcribed load op.
        op: SceneOp,
    },
    /// An op consumed by the presentation/runtime seam.
    Presentation {
        /// The op that ran.
        op: SceneOp,
    },
    /// The volatile retail `Game_Cleared_Flag` was set.
    GameCleared,
    /// The scene ended.
    Finished,
    /// The scene could not continue.
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
    /// The op budget ran out.
    Runaway,
}
