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

/// A proven enemy attack presentation beat attached to one ordered event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleAnimationEvent {
    /// Index into [`BattleTimeline::events`].
    pub event_index: usize,
    /// Enemy fighter whose attack object was dispatched.
    pub actor: FighterId,
    /// Pack enemy record selected by the fighter's live stats.
    pub enemy_id: u16,
    /// The exact retail SFX selected by the same object graph.
    pub sfx_id: u8,
    /// Mapping duration, when a retail helper or oracle receipt proved it.
    pub frame_duration: Option<u8>,
    /// Mapping count, when available.
    pub frame_count: Option<u8>,
    /// Sum of the per-frame durations, when available.
    pub total_frames: Option<u16>,
    /// Variable timer bytes, one per mapping frame.  Fixed records carry a
    /// repeated vector so the Godot clock has one contract for both forms.
    pub frame_durations: Option<Vec<u8>>,
    /// Retail movement was not inferred into the body sprite.
    pub movement_proven: bool,
    /// The selected six-byte mapping records compose against the enemy art
    /// bank and may replace the body flash with a real attack layer.
    pub sprite_sheet_proven: bool,
    /// The fixed timing is safe to use as a presentation beat.
    pub flash_timing_proven: bool,
}

/// One ordered battle presentation batch plus its retail sidecars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleTimeline {
    /// Core events in deterministic resolution order.
    pub events: Vec<BattleEvent>,
    /// Sound requests keyed to the event that reaches the retail moment.
    pub sounds: Vec<BattleSoundEvent>,
    /// Proven enemy attack presentation records keyed to the same events.
    pub animations: Vec<BattleAnimationEvent>,
}

impl BattleTimeline {
    /// A non-mutating sound probe used by `PSIV_DEBUG_BATTLE=0x88`.
    ///
    /// The oracle battle selector intentionally does not start a runtime
    /// round. This fixture still exercises the exact Godot queue and live
    /// `Field::play_sound` path with a static exact attack, a moving exact
    /// attack, and a miss.
    #[must_use]
    pub fn debug_audio_probe() -> BattleTimeline {
        let party = FighterId::new(1).expect("fighter id 1");
        let zoran = FighterId::new(6).expect("fighter id 6");
        let twin_arms = FighterId::new(7).expect("fighter id 7");
        BattleTimeline {
            events: vec![
                BattleEvent::Attacked {
                    actor: zoran,
                    targets: vec![party],
                },
                BattleEvent::Resolved {
                    actor: zoran,
                    target: party,
                    verdict: Verdict::Normal,
                    damage: Some(7),
                    remaining_hp: 18,
                },
                BattleEvent::Attacked {
                    actor: twin_arms,
                    targets: vec![party],
                },
                BattleEvent::Resolved {
                    actor: twin_arms,
                    target: party,
                    verdict: Verdict::Miss,
                    damage: None,
                    remaining_hp: 18,
                },
            ],
            sounds: vec![
                BattleSoundEvent {
                    event_index: 0,
                    id: 0xD8,
                },
                BattleSoundEvent {
                    event_index: 2,
                    id: 0xD5,
                },
                BattleSoundEvent {
                    event_index: 3,
                    id: 0xB8,
                },
            ],
            animations: vec![
                BattleAnimationEvent {
                    event_index: 0,
                    actor: zoran,
                    enemy_id: 10,
                    sfx_id: 0xD8,
                    frame_duration: Some(2),
                    frame_count: Some(8),
                    total_frames: Some(16),
                    frame_durations: Some(vec![2; 8]),
                    movement_proven: true,
                    sprite_sheet_proven: true,
                    flash_timing_proven: true,
                },
                BattleAnimationEvent {
                    event_index: 2,
                    actor: twin_arms,
                    enemy_id: 87,
                    sfx_id: 0xD5,
                    frame_duration: Some(2),
                    frame_count: Some(4),
                    total_frames: Some(8),
                    frame_durations: Some(vec![2; 4]),
                    movement_proven: true,
                    sprite_sheet_proven: true,
                    flash_timing_proven: true,
                },
            ],
        }
    }

    /// Builds an attack-only probe for a real formation whose newly decoded
    /// members are supplied by the Godot setup.  The records are copied from
    /// the retail pack census so the live selector exercises the variable and
    /// selector clocks without requiring a player to drive a command menu.
    #[must_use]
    pub fn debug_newly_exact_probe(enemies: &[(u8, u16)]) -> BattleTimeline {
        let party = FighterId::new(1).expect("fighter id 1");
        let mut events = Vec::new();
        let mut sounds = Vec::new();
        let mut animations = Vec::new();
        for (fighter_id, enemy_id) in enemies {
            let Some((sfx_id, durations)) = (match *enemy_id {
                2 => Some((0xD6, vec![6, 4, 4, 5])),
                17 => Some((0xB6, vec![4, 3, 4])),
                24 => Some((0xD6, vec![4, 6, 6])),
                39 => Some((
                    0xCD,
                    vec![1, 1, 1, 1, 1, 2, 1, 1, 3, 1, 1, 4, 1, 1, 5, 1, 1],
                )),
                149 => Some((0xD5, vec![6, 8, 8, 8, 8, 8, 8, 8, 8])),
                _ => None,
            }) else {
                continue;
            };
            let actor = FighterId::new(*fighter_id).expect("debug enemy fighter id");
            let event_index = events.len();
            let total_frames = durations.iter().map(|duration| u16::from(*duration)).sum();
            events.push(BattleEvent::Attacked {
                actor,
                targets: vec![party],
            });
            sounds.push(BattleSoundEvent {
                event_index,
                id: sfx_id,
            });
            animations.push(BattleAnimationEvent {
                event_index,
                actor,
                enemy_id: *enemy_id,
                sfx_id,
                frame_duration: durations.first().copied(),
                frame_count: u8::try_from(durations.len()).ok(),
                total_frames: Some(total_frames),
                frame_durations: Some(durations),
                movement_proven: true,
                sprite_sheet_proven: true,
                flash_timing_proven: true,
            });
            events.push(BattleEvent::Resolved {
                actor,
                target: party,
                verdict: Verdict::Normal,
                damage: Some(1),
                remaining_hp: 24,
            });
        }
        BattleTimeline {
            events,
            sounds,
            animations,
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
        /// Proven enemy attack presentation records keyed to `events`.
        animations: Vec<BattleAnimationEvent>,
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
    /// A mounted Action press hit a non-empty dismount cell.
    VehicleDismountBlocked,
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
