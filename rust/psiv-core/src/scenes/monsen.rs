//! Tallas's rescue under Monsen (issue #81): his mother's two talks, the fall
//! into the crevice and the walk home after the Fract Ooze.
//!
//! Record: `docs/scenes/140_TallasRescue.md`. Every body was read from the US
//! image through its `EventPtrs` slot; the clone's labels are navigation only.

use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `$C300 + 2 * $40`: Tallas's object on `MonsenCave`, `$C380`.
const TALLAS: usize = 2;
/// `FieldObj_Tallas`, walking to his `dest` words.
const TALLAS_ROUTINE: u16 = 0x2FC;
/// `MusicID_InTheCave`.
const MUSIC_IN_THE_CAVE: u8 = 0x8A;

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue` (`$5AC66`).
const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

/// `moveq #flag, d0 / jmp EventFlags_Set`.
const fn set_flag(id: u16) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value: true,
    }
}

/// `Field_Obj_Secondary`, the mother on `MonsenHouse4`, turns to the leader.
const FACE_MOTHER: SceneOp = SceneOp::FaceOppositeOf {
    actor: ActorRef::Npc(0),
    of: ActorRef::PartyMember(0),
};

/// `lea ($FFFFC380).w, a4 / move.w #x, d0 / move.w #y, d1 / jsr
/// Event_MoveSingleObject`.
const fn tallas_walks(x: i32, y: i32) -> SceneOp {
    SceneOp::MoveActorTo {
        actor: ActorRef::Npc(TALLAS),
        x,
        y,
        wait: true,
    }
}

/// `$007A`, `Event_TallasMom`, retail `$072AD0..$072AF5`
/// (`ps4.asm:152536-152545`): she asks the party to find him and sets
/// `EventFlag_TallasMother` (`$2D`), which `MapDataMan_MonsenSign` reads to
/// take the danger sign off the crevice.
pub static TALLAS_MOM: Scene = Scene {
    name: "Event_TallasMom",
    event: EventIndex(0x007A),
    ops: &[FACE_MOTHER, standard(0x2D), set_flag(0x2D)],
};

/// `$007B`, `Event_TallasMomAfterRescue`, retail `$072AF6..$072B1B`
/// (`ps4.asm:152546-152555`): her thanks, `EventFlag_AfterCrevice` (`$3C`).
pub static TALLAS_MOM_AFTER_RESCUE: Scene = Scene {
    name: "Event_TallasMomAfterRescue",
    event: EventIndex(0x007B),
    ops: &[FACE_MOTHER, standard(0x2E), set_flag(0x3C)],
};

/// `$007C`, `Event_InsideMonsenHole`, retail `$072B1C..$072B2B`
/// (`ps4.asm:152556-152560`): the party's line on landing, then
/// `EventFlag_InsideCave` (`$2E`), which ends map trigger `$71`.
pub static INSIDE_MONSEN_HOLE: Scene = Scene {
    name: "Event_InsideMonsenHole",
    event: EventIndex(0x007C),
    ops: &[standard(0x24), set_flag(0x2E)],
};

/// `$007E`, `Event_TallasRescued`, retail `$072B52..$072C07`
/// (`ps4.asm:152571-152611`): after the Fract Ooze Tallas speaks, takes
/// `FieldObj_Tallas` and walks out by the way the record gives, and his slot
/// is cleared. The saved music goes back to the cave's and
/// `EventFlag_TallasSaved` (`$3E`) ends map trigger `$72`.
pub static TALLAS_RESCUED: Scene = Scene {
    name: "Event_TallasRescued",
    event: EventIndex(0x007E),
    ops: &[
        standard(0x27),
        // `move.w #$2FC, (a4)`, then the one-object frame.
        SceneOp::SetObjectRoutine {
            npc_index: TALLAS,
            routine: TALLAS_ROUTINE,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        // The first two calls name the same spot; the second returns after
        // one frame of the routine, which the runner's arrival takes no frame
        // for. The record says so.
        tallas_walks(0x610, 0x2D0),
        tallas_walks(0x610, 0x2D0),
        tallas_walks(0x610, 0x320),
        tallas_walks(0x670, 0x320),
        tallas_walks(0x670, 0x300),
        tallas_walks(0x5F0, 0x300),
        tallas_walks(0x5F0, 0x390),
        // `clr.w (a4)`.
        SceneOp::DespawnNpc {
            npc_index: TALLAS,
            count: 1,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_IN_THE_CAVE,
        },
        set_flag(0x3E),
    ],
};
