//! Runtime events emitted for the presentation layer to consume.

use psiv_core::battle::BattleEvent;
use psiv_core::{Cell, Direction, InteractReach, MapId, WarpTrigger};

/// What a [`crate::Runtime`] tick produced, for the presentation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// The party finished a step.
    StepCompleted {
        /// The cell landed on.
        cell: Cell,
    },
    /// A transition fired and the runtime switched maps. The renderer should
    /// rebuild the scene for [`crate::Runtime::map_id`]; `trigger` says whether
    /// this was a doorway (fade) or ground transition (scroll), per the design doc.
    MapChanged {
        /// The map now loaded.
        map: MapId,
        /// Which transition class fired.
        trigger: WarpTrigger,
    },
    /// A transition targeted a map the pack does not contain (skipped or
    /// unpacked). The party stays put; the presentation layer should surface
    /// this as a data warning.
    UnpackedTarget {
        /// The map the transition wanted.
        map: MapId,
    },
    /// The engine reported a type-1 cell with no matching doorway record — a
    /// pack defect. Surface it, never swallow it.
    WarpUnmapped {
        /// The offending cell.
        cell: Cell,
    },
    /// The party talked to an NPC. The renderer looks up the NPC's
    /// dialogue binding in the map record and opens a window; while it is
    /// open, it stops sending direction inputs (the engine is not modal).
    Interact {
        /// Index into the current map's NPC list.
        npc_index: usize,
        /// The probe cell that was hit.
        cell: Cell,
        /// Whether the hit came from the ordinary one-cell probe or across a
        /// `$C` counter. Counter hits check the shop-location table (shop vs
        /// dialogue); adjacent hits are always dialogue.
        reach: InteractReach,
    },
    /// Confirm pressed with nothing in talk range — the cartridge answers
    /// with the leader's "Nothing here" line.
    InteractNothing {
        /// Which way the party was facing.
        facing: Direction,
    },
    /// A trigger fired and a transcribed scene began. Cinema mode on.
    SceneStarted {
        /// The RunEventsJmpTbl index that fired.
        trigger: u8,
    },
    /// The running scene finished (or faulted; faults are logged). Cinema off.
    SceneEnded,
    /// A trigger fired an event with no transcribed scene yet.
    SceneMissing {
        /// The event index that has no scene.
        event: u16,
    },
    /// A trigger hit one of the four honestly-unsupported custom checks.
    TriggerUnsupported {
        /// The trigger index.
        trigger: u8,
    },
    /// The scene asks for a dialogue entry (within the current map's bound
    /// tree). The renderer opens the window and calls
    /// [`crate::Runtime::dialogue_closed`] when it shuts.
    SceneDialogue {
        /// Entry index in the map's dialogue tree.
        entry: u16,
    },
    /// A scene requested an event battle and the runtime started it. The
    /// initial setup events are delivered with the same timeline as random
    /// encounters, so the presentation layer can use one battle screen.
    SceneBattleStarted {
        /// The event battle index.
        index: u16,
        /// Battle-start timeline events.
        events: Vec<BattleEvent>,
    },
    /// A scene battle could not be started. The runtime resumes the scene
    /// with an escaped outcome so a malformed pack cannot deadlock it.
    SceneBattleFailed {
        /// The event battle index.
        index: u16,
        /// The conversion or setup error.
        error: String,
    },
    /// The party composition changed (join, swap, leader change). The
    /// renderer refreshes party sprites.
    PartyChanged,
    /// A random encounter fired on this landing. The shell seats the party
    /// and calls [`crate::Runtime::start_battle`] with this formation.
    EncounterRolled {
        /// The formation id the encounter tables picked.
        formation: u16,
    },
    /// Map objects were despawned in place (indices stable). The renderer
    /// hides their nodes.
    NpcsDespawned {
        /// First object index.
        first: usize,
        /// How many consecutive objects.
        count: usize,
    },
}
