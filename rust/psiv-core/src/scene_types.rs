//! Inputs, effects, and faults shared by the scene interpreter and runtime.
//!
//! Kept apart from the scene instruction vocabulary so adding a retail
//! presentation record does not turn the core's public contract into a
//! thousand-line monolith.

use crate::geom::{Cell, Direction};
use crate::scene::SceneOp;
use crate::scenes::FlightLeg;
use crate::state::{CharId, Flag, PARTY_SLOTS};

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

/// Which dialogue-window setup a scene asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DialogueWindow {
    /// `Event_GetAndRunDialogue` (`$5AC66`): the window closes, the panels
    /// are destroyed and the map chunks reload (`Panel_DestroyAll`,
    /// `Window_Destroy`, `Map_LoadChunks`).
    #[default]
    Standard,
    /// `Event_GetAndRunDialogue2` (`$5ACDC`, `ps4.asm:121634`): the same text
    /// loop, but the routine ends by zeroing `Panel_Num`, `Windows_Opened_Num`
    /// and `$FFFFECA4` instead of destroying anything, so the text window and
    /// every panel stay on screen and no chunk is reloaded. Fourteen
    /// cartridge callers (`docs/scenes/DIALOGUE2_CALLERS.md`), every one a
    /// scene that ends in a battle or a map change.
    Retained,
    /// `Event_GetAndRunDialogue5` (`$5ADF8`) — `Cutscene_PiataPrincipal`.
    Cutscene,
    /// `Event_GetAndRunDialogue3`, used by the Rykros surface.
    Cutscene3,
    /// The ending's first `RunText3` window before the panel sequence.
    EndingIntro,
    /// `Event_GetAndRunDialogue4` / `Event_RunDialogue4` in `$8021`.
    Ending,
    /// `Event_RunDialogue5` used for the Rykros hand-off line.
    Cutscene5,
}

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
    /// The runtime's `Event_MoveCamera` glide reached its target.
    CameraArrived,
    /// The ending's retail `Joypad_Pressed` loop received Start.
    EndingContinue,
    /// The destination menu's player confirmed a row; the session already
    /// wrote `World_Index` (`ps4.asm:133677`).
    DestinationChosen,
    /// The destination menu's player cancelled (`ps4.asm:133692`).
    DestinationCancelled,
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
    /// All party objects were parked at the boarding lattice cell. A pan is
    /// required only for an original coordinate with bit `$10` set.
    VehicleBoardingAligned {
        /// The vehicle body object staged before the selector write.
        index: u16,
        /// The snapped standing cell.
        cell: Cell,
        /// Whether to run and await the speed-2 camera pan.
        pan_camera: bool,
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
    /// A field object's `dialogue_id` was written.
    NpcDialogueSet {
        /// The object's index in the map's object list.
        npc_index: usize,
        /// The dialogue entry it speaks from now on.
        dialogue_id: u16,
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
    /// The scene wrote `World_Index`.
    WorldIndexSet {
        /// The world.
        world: u8,
    },
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
    /// A flight leg's table has no row for the current map or world
    /// (`loc_64B02`, `loc_64B34`, `loc_64B5A`, `ps4.asm:134540-134610`).
    NoFlightTarget {
        /// The table.
        leg: FlightLeg,
        /// The map the party was on.
        map: u16,
        /// `World_Index` at the request.
        world: u8,
    },
    /// A [`SceneOp::ConveyorRide`] reached the end of the map's chunk plane
    /// (or ran with no layout to read) before the belt's chunks ended.
    NoLayout,
}
