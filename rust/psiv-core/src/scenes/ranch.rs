//! The ranch owner at Mile (issue #81): he steps out from behind his fence for
//! the sandworm job, then thanks the party once it is dead.
//!
//! Record: `docs/scenes/142_RanchOwner.md`. Both bodies were read from the US
//! image through their `EventPtrs` slots; the clone's labels are navigation
//! only.

use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `Field_Obj_Secondary`, the owner.
const OWNER: ActorRef = ActorRef::Npc(0);

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue` (`$5AC66`).
const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn set_flag(id: u16) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value: true,
    }
}

/// `$0070`, `Event_RanchOwner`, retail `$07259E..$072653`
/// (`ps4.asm:152178-152220`): the owner walks level with the leader, faces
/// down and speaks entry `$20`; the two blocks that closed his gate go, and he
/// walks to (`$110,$2D0`), Y first, and faces down.
/// `EventFlag_MileRanchOwner` (`$1A`) is the last write.
pub static RANCH_OWNER: Scene = Scene {
    name: "Event_RanchOwner",
    event: EventIndex(0x0070),
    ops: &[
        // `.loop`: `FieldObj_NPCType7` a frame at a time until the X words match.
        SceneOp::MoveActorToActorAxis {
            actor: OWNER,
            target: ActorRef::PartyMember(0),
            axis: Axis::X,
            wait: true,
        },
        SceneOp::Face {
            actor: OWNER,
            facing: Direction::Down,
        },
        standard(0x20),
        // `lea ($FFFFC500).w, a4 / clr.w (a4) / clr.w $40(a4)`.
        SceneOp::DespawnNpc {
            npc_index: 8,
            count: 2,
        },
        // `move.w #$194, (a4)`, `$13`, `$16` (the record's own tile) and
        // `dest` := `curr`, then the one-object frame.
        SceneOp::SetObjectRoutine {
            npc_index: 0,
            routine: 0x194,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        // `bset #1, Char_Move_Flags`.
        SceneOp::SetFollowMode { bits: 2 },
        SceneOp::MoveActorTo {
            actor: OWNER,
            x: 0x110,
            y: 0x2D0,
            wait: true,
        },
        SceneOp::Face {
            actor: OWNER,
            facing: Direction::Down,
        },
        SceneOp::SetFollowMode { bits: 0 },
        set_flag(0x1A),
    ],
};

/// `$0072`, `Event_RanchOwnerAfterBattle`, retail `$072678..$07269D`
/// (`ps4.asm:152232-152241`): he turns to the leader, speaks entry `$21` and
/// sets `EventFlag_RanchOwnerAfterBattle` (`$1C`).
pub static RANCH_OWNER_AFTER_BATTLE: Scene = Scene {
    name: "Event_RanchOwnerAfterBattle",
    event: EventIndex(0x0072),
    ops: &[
        SceneOp::FaceOppositeOf {
            actor: OWNER,
            of: ActorRef::PartyMember(0),
        },
        standard(0x21),
        set_flag(0x1C),
    ],
};
