//! Runtime events emitted for the presentation layer to consume.

use psiv_core::battle::{BattleEvent, FighterId, Verdict};
use psiv_core::{Cell, Direction, InteractReach, MapId, SceneFault, SceneOp, WarpTrigger};

/// A retail sound request attached to one ordered battle event.
///
/// The core resolves a round atomically, while the cartridge raises
/// `Sound_Index` from the action/animation state machine. `event_index` keeps
/// that boundary explicit: the Godot presentation must dispatch this id when
/// it starts the corresponding event, not when the whole round is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleSoundEvent {
    /// Index into [`BattleTimeline::events`].
    pub event_index: usize,
    /// Retail `Sound_Index` byte.
    pub id: u8,
}

/// One ordered battle presentation batch plus its retail SFX sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleTimeline {
    /// Core events in deterministic resolution order.
    pub events: Vec<BattleEvent>,
    /// Sound requests keyed to the event that reaches the retail moment.
    pub sounds: Vec<BattleSoundEvent>,
}

impl BattleTimeline {
    /// A non-mutating sound probe used by `PSIV_DEBUG_BATTLE=0x88`.
    ///
    /// The oracle battle selector intentionally does not start a runtime
    /// round. This fixture still exercises the exact Godot queue and live
    /// `Field::play_sound` path with an attack, a second attack, and a miss.
    #[must_use]
    pub fn debug_audio_probe() -> BattleTimeline {
        let actor = FighterId::new(1).expect("fighter id 1");
        let target = FighterId::new(6).expect("fighter id 6");
        BattleTimeline {
            events: vec![
                BattleEvent::Attacked {
                    actor,
                    targets: vec![target],
                },
                BattleEvent::Resolved {
                    actor,
                    target,
                    verdict: Verdict::Normal,
                    damage: Some(7),
                    remaining_hp: 18,
                },
                BattleEvent::Attacked {
                    actor,
                    targets: vec![target],
                },
                BattleEvent::Resolved {
                    actor,
                    target,
                    verdict: Verdict::Miss,
                    damage: None,
                    remaining_hp: 18,
                },
            ],
            sounds: vec![
                BattleSoundEvent {
                    event_index: 0,
                    id: 0xF5,
                },
                BattleSoundEvent {
                    event_index: 2,
                    id: 0xF5,
                },
                BattleSoundEvent {
                    event_index: 3,
                    id: 0xB8,
                },
            ],
        }
    }
}

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
    /// An interaction area resolved an event and installed its scene.
    ///
    /// This is separate from [`RuntimeEvent::SceneStarted`] because the
    /// source is a map-area record, not the map's RunEvents list. The Godot
    /// shell uses both notifications to enter cinema mode.
    SceneStartedFromInteraction {
        /// Index in the current map's interaction-area list.
        area: u32,
        /// The resolved Event_Index word, including the cutscene bit.
        event: u16,
    },
    /// The running scene finished successfully. Cinema off.
    SceneEnded,
    /// One typed presentation operation emitted by the scene interpreter.
    ///
    /// The runtime keeps these in the exact order returned by one scene tick;
    /// the shell consumes them on that same tick. Blocking operations such as
    /// `WaitFrames` therefore remain interpreter semantics while the Godot
    /// layer receives every visual/audio side effect instead of losing it at
    /// the translation boundary.
    ScenePresentation {
        /// The scene operation to apply in order.
        op: SceneOp,
    },
    /// The scene interpreter rejected an op. A fault is never a successful
    /// scene completion; the shell can surface the exact actor/jump/write
    /// defect to the developer instead of silently dropping it.
    SceneFaulted {
        /// Interpreter fault.
        fault: SceneFault,
    },
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
        /// Retail SFX requests keyed to `events`.
        sounds: Vec<BattleSoundEvent>,
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
    /// The scene changed the party inventory.
    InventoryChanged,
    /// The scene selected a different vehicle.
    VehicleChanged {
        /// The retail vehicle id.
        index: u16,
    },
    /// The scene wrote a persistent character record.
    RosterChanged {
        /// The character id whose record changed.
        who: psiv_core::CharId,
    },
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
