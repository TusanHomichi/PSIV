//! Three short Motavia events (issue #81): Uzo's mother asks after her
//! daughters, Sekreas explains the bird in Rappy Cave, and the patients of
//! Meese's clinic answer from their beds.
//!
//! Records are `docs/scenes/200_LostGirlsMother.md`,
//! `201_KingRappyDefeated.md` and `202_MeeseClinicSickWoman.md`. Every body
//! was read from the US image through its `EventPtrs` slot; the clone's labels
//! are navigation only.

use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::scene_runner::drift::CoordCmp;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue` (`$5AC66`).
const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

/// `lea (object).w, a4 / lea (Character_1).w, a3 / move.w $6(a3), d0 / bchg
/// #2, d0 / jsr Event_UpdateObjFacing`.
const fn face_leader(npc: usize) -> SceneOp {
    SceneOp::FaceOppositeOf {
        actor: ActorRef::Npc(npc),
        of: ActorRef::PartyMember(0),
    }
}

/// `$007F`, `Event_LostGirlsMother`, retail `$072C08..$072C2D`: `UzoHouse3`'s
/// object 1 turns to the leader, speaks entry `$13` and sets
/// `EventFlag_MissingGirlsMom` (`$79`), which the prison's sister reads.
pub static LOST_GIRLS_MOTHER: Scene = Scene {
    name: "Event_LostGirlsMother",
    event: EventIndex(0x007F),
    ops: &[
        face_leader(1),
        standard(0x13),
        SceneOp::SetFlag {
            flag: Flag::event(0x79),
            value: true,
        },
    ],
};

/// `Field_Obj_Secondary + 2 * $40`: the invisible `KingRappyFlyingAway`.
const FLYING_AWAY: ActorRef = ActorRef::Npc(2);
/// `$FFFFC3C0`: the slot the body builds Sekreas in.
const SEKREAS_SLOT: usize = 3;
/// `FieldObj_NPCScriptMove`.
const NPC_SCRIPT_MOVE: u16 = 0x194;
/// `move.w #$202, $16(a4)`.
const SEKREAS_TILE: u16 = 0x202;
/// `loc_730FC`, the sixteen words copied to `Palette_Line_4`.
const SEKREAS_PALETTE: u32 = 0x0007_30FC;
/// `Event_MoveSingleObject` on Sekreas, `$A0` pixels of Y: ten cells at the
/// normal step's eight frames a cell.
pub const SEKREAS_WALK_FRAMES: u16 = 10 * 8;

/// `$0089`, `Event_KingRappyDefeated`, retail `$073018..$0730FB`: after the
/// fight the invisible object walks off and goes, Sekreas's voice stops the
/// party (entry `$37`), the leader walks up the cave, Sekreas comes up, talks
/// and walks back, entry `$38` closes it, and `EventFlag_SekreasReason`
/// (`$BA`) is the last write.
pub static KING_RAPPY_DEFEATED: Scene = Scene {
    name: "Event_KingRappyDefeated",
    event: EventIndex(0x0089),
    ops: &[
        SceneOp::MoveActorTo {
            actor: FLYING_AWAY,
            x: 0x3C0,
            y: 0x1C0,
            wait: true,
        },
        SceneOp::DespawnNpc {
            npc_index: 2,
            count: 1,
        },
        standard(0x37),
        SceneOp::MoveActorTo {
            actor: ActorRef::PartyMember(0),
            x: 0x3E0,
            y: 0x110,
            wait: true,
        },
        SceneOp::RunDialogueResume,
        SceneOp::LoadPalette {
            rom_addr: SEKREAS_PALETTE,
            words: 16,
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::CreateFieldObject {
            slot: SEKREAS_SLOT,
            object_id: NPC_SCRIPT_MOVE,
            art_tile: SEKREAS_TILE,
            x: 0x3E0,
            y: 0x1C0,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::Presentation {
            op: PresentationOp::WalkObject {
                slot: SEKREAS_SLOT,
                x: 0x3E0,
                y: 0x120,
            },
        },
        SceneOp::Wait {
            ticks: SEKREAS_WALK_FRAMES,
        },
        SceneOp::RunDialogueResume,
        SceneOp::Presentation {
            op: PresentationOp::WalkObject {
                slot: SEKREAS_SLOT,
                x: 0x3E0,
                y: 0x1C0,
            },
        },
        SceneOp::Wait {
            ticks: SEKREAS_WALK_FRAMES,
        },
        SceneOp::DespawnNpc {
            npc_index: SEKREAS_SLOT,
            count: 1,
        },
        standard(0x38),
        SceneOp::SetFlag {
            flag: Flag::event(0xBA),
            value: true,
        },
    ],
};

/// Both halves of one `loc_73420` row's compare: `(a1)` against `$30`, then
/// `$2(a1)` against `$34`.
const fn clinic_x(x: u16, on_match: usize, on_miss: usize) -> SceneOp {
    SceneOp::BranchIfActorCoord {
        actor: ActorRef::PartyMember(0),
        axis: Axis::X,
        cmp: CoordCmp::Equal,
        value: x,
        if_true: on_match,
        if_false: on_miss,
    }
}

/// See [`clinic_x`].
const fn clinic_y(y: u16, on_match: usize, on_miss: usize) -> SceneOp {
    SceneOp::BranchIfActorCoord {
        actor: ActorRef::PartyMember(0),
        axis: Axis::Y,
        cmp: CoordCmp::Equal,
        value: y,
        if_true: on_match,
        if_false: on_miss,
    }
}

/// `$0093`, `Event_MeeseClinicSickWoman`, retail `$0733CC..$07341F`: the row
/// of `loc_73420` the leader stands on names the patient (`$C7C0`, `$C6C0`,
/// `$C640`, `$C740`, objects `$13`, `$F`, `$D`, `$11`), who turns; she speaks
/// entry `$3C`, or `$3D` once `EventFlag_DarkForce2` (`$9E`) is set. From any
/// other cell the routine returns at once.
pub static MEESE_CLINIC_SICK_WOMAN: Scene = Scene {
    name: "Event_MeeseClinicSickWoman",
    event: EventIndex(0x0093),
    ops: &[
        // 0-7: the table, in its order.
        clinic_x(0x110, 1, 2),
        clinic_y(0x260, 8, 2),
        clinic_x(0x110, 3, 4),
        clinic_y(0x1A0, 10, 4),
        clinic_x(0x200, 5, 6),
        clinic_y(0x140, 12, 6),
        clinic_x(0x230, 7, 20),
        clinic_y(0x140, 14, 20),
        // 8-15: `loc_733F2`, on the row's object.
        face_leader(0x13),
        SceneOp::Jump { to: 16 },
        face_leader(0xF),
        SceneOp::Jump { to: 16 },
        face_leader(0xD),
        SceneOp::Jump { to: 16 },
        face_leader(0x11),
        SceneOp::Jump { to: 16 },
        // 16-19: `EventFlag_DarkForce2` picks the line.
        SceneOp::BranchFlag {
            flag: Flag::event(0x9E),
            if_set: 19,
            if_clear: 17,
        },
        standard(0x3C),
        SceneOp::SkipOps { count: 1 },
        standard(0x3D),
    ],
};
