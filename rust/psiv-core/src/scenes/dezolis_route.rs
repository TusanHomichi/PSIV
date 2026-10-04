//! Retail events on the Dezolis route between the Raja Temple exit and the
//! Kuran handoff that `dezo_campaign.rs` and `post_zio_cutscenes.rs` start from.
//!
//! The records are `docs/scenes/91_*` onward. They were found one halt at a
//! time (runner log H28) until `docs/scenes/EVENT_COVERAGE.md` made the set
//! structural: every event the cartridge can fire is either registered here or
//! listed, with an issue, in the census allowlist.

use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::scene_runner::drift::{CoordCmp, NpcDrift};
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `DialogueTree14`, the tree `Event_OutsideRajaTemple` loads (`move.l
/// #$1E99F0, d0` at `$06FC76`).
const TREE_14: u32 = 0x001E_99F0;

/// `EventFlag_Snowstorm`, `$80` (`move.b #$80, d0` at `$06FC8A`).
const FLAG_SNOWSTORM: u16 = 0x80;

/// `$0043`, `Event_OutsideRajaTemple`, retail `$06FC76..$06FC93`.
///
/// `RunEvent_OutsideRajaTemple` (`$0043` is written at `$056D50`) has no
/// position test, so this plays on the first field frame after every arrival
/// outside Raja Temple until `$80` is set. The routine ends in a tail
/// `jmp EventFlags_Set`, so the flag write is its last act.
pub static OUTSIDE_RAJA_TEMPLE: Scene = Scene {
    name: "Event_OutsideRajaTemple",
    event: EventIndex(0x0043),
    ops: &[
        SceneOp::SetDialogueTree { rom_addr: TREE_14 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(6)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetFlag {
            flag: Flag::event(FLAG_SNOWSTORM),
            value: true,
        },
    ],
};

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// `EventFlag_TylerGrave`, `$84` (`move.b #$84, d0` at `$06FCA2`, `$06FDEE`
/// and `$06FE08`).
const FLAG_TYLER_GRAVE: u16 = 0x84;

/// `SFXID_GraveOpening`, `$DD` (`$06FCB6`, `$06FDB8`).
const SFX_GRAVE_OPENING: u8 = 0xDD;

/// Tyler's left gravestone half, the object `$C380` the scene tests and drifts
/// (map object 2: RAM `$C300 + 2 * $40`).
const STONE_LEFT: usize = 2;
/// The right half, `$C3C0`.
const STONE_RIGHT: usize = 3;

/// `x_step_constant` the scene writes to the six objects it sends left
/// (`move.l d0, $20(a4)` with `d0 = $FFFFC000`, `$06FD04`): -0.25 px a frame.
const STEP_LEFT: i32 = -0x4000;
/// ...and to the six it sends right (`d0 = $8000`, `$06FD46`): +0.5 px a frame.
const STEP_RIGHT: i32 = 0x8000;

/// The twelve objects `$C380..$C640`, in the order the scene writes them
/// (`$06FD0A..$06FD84`). Objects 4..13 are `FieldObj_InvisibleBlock`
/// (`$0488D8`, which calls `FieldObj_UpdatePosition` while on screen) and 2, 3
/// are `FieldObj_GravestoneHalf` (`$04DDAE`, which always does). Sixty-four
/// frames later (`moveq #$3F, d0 / jsr DoMainUpdatesLoop`, `$06FD88`) the left
/// six have moved 16 px and the right six 32 px.
pub(crate) static GRAVE_DRIFT: &[NpcDrift] = &[
    NpcDrift {
        npc: 2,
        step_x: STEP_LEFT,
        step_y: 0,
    },
    NpcDrift {
        npc: 4,
        step_x: STEP_LEFT,
        step_y: 0,
    },
    NpcDrift {
        npc: 5,
        step_x: STEP_LEFT,
        step_y: 0,
    },
    NpcDrift {
        npc: 8,
        step_x: STEP_LEFT,
        step_y: 0,
    },
    NpcDrift {
        npc: 9,
        step_x: STEP_LEFT,
        step_y: 0,
    },
    NpcDrift {
        npc: 10,
        step_x: STEP_LEFT,
        step_y: 0,
    },
    NpcDrift {
        npc: 3,
        step_x: STEP_RIGHT,
        step_y: 0,
    },
    NpcDrift {
        npc: 6,
        step_x: STEP_RIGHT,
        step_y: 0,
    },
    NpcDrift {
        npc: 7,
        step_x: STEP_RIGHT,
        step_y: 0,
    },
    NpcDrift {
        npc: 11,
        step_x: STEP_RIGHT,
        step_y: 0,
    },
    NpcDrift {
        npc: 12,
        step_x: STEP_RIGHT,
        step_y: 0,
    },
    NpcDrift {
        npc: 13,
        step_x: STEP_RIGHT,
        step_y: 0,
    },
];

/// `$0044`, `Event_TylerGraveOpening`, retail `$06FC94..$06FE1B`.
///
/// Reached by dialogue `$F6` (`DialogueTree14` entry 35, the grave's
/// inscription) on Tyler. The stone's x is the "not yet moved" test: after the
/// slide the object is no longer at `$120`, so a second interaction only reads
/// entry `$22`. The first dialogue (entry `$1E`) yields at its `$F7`; after the
/// grave opens, `popdlg / Event_RunDialogue` resumes its second half.
pub static TYLER_GRAVE_OPENING: Scene = Scene {
    name: "Event_TylerGraveOpening",
    event: EventIndex(0x0044),
    ops: &[
        // 0: `cmpi.w #$120, $30($C380)` / `bne.w loc_6FE12` ($06FC98).
        SceneOp::BranchIfActorCoord {
            actor: ActorRef::Npc(STONE_LEFT),
            axis: Axis::X,
            cmp: CoordCmp::Equal,
            value: 0x120,
            if_true: 1,
            if_false: 18,
        },
        // 1: `EventFlags_Test $84` / `bne.b loc_6FCB6` ($06FCA2).
        SceneOp::BranchFlag {
            flag: Flag::event(FLAG_TYLER_GRAVE),
            if_set: 3,
            if_clear: 2,
        },
        // 2: `moveq #$1E, d0 / jsr Event_GetAndRunDialogue` ($06FCAE).
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x1E)),
            window: DialogueWindow::Standard,
        },
        // 3: `loc_6FCB6`.
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        // 4-5: `Event_MoveCamera` to the leader's (x + 4, y), then (x, y), speed
        // 4 (`$06FCBE..$06FCE0`). The 4 px intermediate is not modelled: the
        // camera op reads the leader's cell, and the glide it replaces ends at
        // the leader either way.
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: LEADER,
                speed: 4,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: LEADER,
                speed: 4,
            },
        },
        // 6: `GetMapLayoutOffset(d1 = $A, d2 = $C, d3 = 1)` (BG), `move.b #$47,
        // (a1)`, `RefreshPlaneBG` ($06FCE4..$06FCFE): the stair chunk.
        SceneOp::WriteMapChunks {
            chunks: &[(10, 12, 0x47)],
        },
        // 7: the twelve step constants, then `DoMainUpdatesLoop($3F)` (64 frames).
        SceneOp::DriftNpcs {
            drifts: GRAVE_DRIFT,
            frames: 64,
        },
        // 8-9: after zeroing the steps, `$6(a4)` = 8 on `$C380`, `$C` on `$C3C0`
        // ($06FDA4..$06FDB2): the halves face each other across the opening.
        SceneOp::Face {
            actor: ActorRef::Npc(STONE_LEFT),
            facing: Direction::Right,
        },
        SceneOp::Face {
            actor: ActorRef::Npc(STONE_RIGHT),
            facing: Direction::Left,
        },
        // 10: `$06FDB8`.
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        // 11-12: the camera again ($06FDC0..$06FDE0).
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: LEADER,
                speed: 4,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: LEADER,
                speed: 4,
            },
        },
        // 13: `moveq #$3B, d0 / jsr DoMapUpdateLoop` ($06FDE6).
        SceneOp::Wait { ticks: 60 },
        // 14: `EventFlags_Test $84` / `bne.b loc_6FE08` ($06FDEE).
        SceneOp::BranchFlag {
            flag: Flag::event(FLAG_TYLER_GRAVE),
            if_set: 16,
            if_clear: 15,
        },
        // 15: `popdlg / Event_RunDialogue`: the rest of entry `$1E` ($06FDFA).
        SceneOp::RunDialogueResume,
        // 16: `loc_6FE08`: tail `jmp EventFlags_Set` with `$84`.
        SceneOp::SetFlag {
            flag: Flag::event(FLAG_TYLER_GRAVE),
            value: true,
        },
        // 17.
        SceneOp::End,
        // 18: `loc_6FE12`: `moveq #$22, d0 / jmp Event_GetAndRunDialogue`.
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x22)),
            window: DialogueWindow::Standard,
        },
    ],
};
