//! Aiedo's side events (issue #81): Rocky's owner, the Hunter's Guild bail
//! quest and the townsfolk's own scenes, short bodies that face the leader and
//! speak entries of `DialogueTree11`.
//!
//! Records are `docs/scenes/120_*` onward. Every body was read from the US
//! image through its `EventPtrs` slot; the clone's labels are navigation only.

use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SavedByte, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `EventFlag_BailPaid` (`$7C`): the guard has the money.
const FLAG_BAIL_PAID: u16 = 0x7C;
/// `EventFlag_Bail` (`$7F`): the purse covered the bail when the guard asked.
/// Entry `$52` opens with `$FA $7F $01`, so the flag picks which answer the
/// guard gives.
const FLAG_BAIL: u16 = 0x7F;
/// The bail, `subi.l #50000` both times.
const BAIL: u32 = 50_000;
/// `MusicID_AHappySettlement`.
const MUSIC_A_HAPPY_SETTLEMENT: u8 = 0x9E;

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue` (`$5AC66`).
pub(super) const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

pub(super) const fn set_flag(id: u16) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value: true,
    }
}

/// `lea (object).w, a4 / lea (Character_1).w, a3 / move.w $6(a3), d0 / bchg
/// #2, d0 / jsr Event_UpdateObjFacing`: map object `npc` (RAM `$FFFFC300 +
/// $40 * npc`) turns to face the party leader.
pub(super) const fn face_leader(npc: usize) -> SceneOp {
    SceneOp::FaceOppositeOf {
        actor: ActorRef::Npc(npc),
        of: ActorRef::PartyMember(0),
    }
}

/// `$0075`, `Event_RockyOwner`, retail `$072872..$072897`
/// (`ps4.asm:152389-152398`): Rocky's owner (`RockyHouse` object 1, at
/// `$FFFFC340`) turns to the leader, asks for the dog in entry `$5E` and sets
/// `EventFlag_RockyOwner` (`$1F`). Tree 11 entry `$44` fires it, reached from
/// the owner's entry `$42` once the guild job is taken (`$1E`).
pub static ROCKY_OWNER: Scene = Scene {
    name: "Event_RockyOwner",
    event: EventIndex(0x0075),
    ops: &[face_leader(1), standard(0x5E), set_flag(0x1F)],
};

/// `$0080`, `Event_GirlPrison`, retail `$072C2E..$072C53`
/// (`ps4.asm:152622-152631`): the sister in the cell (`AiedoPrison` object 1)
/// turns, speaks entry `$5F` and sets `EventFlag_GirlPrison` (`$7A`).
pub static GIRL_PRISON: Scene = Scene {
    name: "Event_GirlPrison",
    event: EventIndex(0x0080),
    ops: &[face_leader(1), standard(0x5F), set_flag(0x7A)],
};

/// `$0081`, `Event_GirlsBail`, retail `$072C54..$072C79`
/// (`ps4.asm:152632-152641`): the guard (`AiedoPrison` object 0,
/// `Field_Obj_Secondary`) names the bail in entry `$50` and sets
/// `EventFlag_PrisonGuard` (`$7B`).
pub static GIRLS_BAIL: Scene = Scene {
    name: "Event_GirlsBail",
    event: EventIndex(0x0081),
    ops: &[face_leader(0), standard(0x50), set_flag(0x7B)],
};

/// `$0082`, `Event_PayingGirlsBail`, retail `$072C7A..$072CDB`
/// (`ps4.asm:152642-152673`): `$7F` is cleared, then set again if the purse
/// holds 50,000; the guard asks in entry `$51`. YES jumps to entry `$52`,
/// whose `$FA $7F $01` sends a covered purse to `$53`, the only one of the
/// two that ends on `!` (`$3A`). The body pays when the answer was YES and
/// the byte before the stopping terminator is `$3A` (the US `if revision>0`
/// compare, `cmpi.b #$3A, -$2(a0)` at `$072CC2`).
pub static PAYING_GIRLS_BAIL: Scene = Scene {
    name: "Event_PayingGirlsBail",
    event: EventIndex(0x0082),
    ops: &[
        // 0: `moveq #$7F, d0 / jsr EventFlags_Clear`.
        SceneOp::SetFlag {
            flag: Flag::event(FLAG_BAIL),
            value: false,
        },
        // 1: `move.l (Current_Money).w, d0 / subi.l #50000, d0 / bmi.s`.
        SceneOp::BranchIfMoneyBelow {
            amount: BAIL,
            if_below: 3,
            if_not: 2,
        },
        // 2.
        set_flag(FLAG_BAIL),
        // 3: `loc_72C96`.
        face_leader(0),
        standard(0x51),
        // 5: `tst.b (Yes_No_Option).w / bne.s loc_72CDA`: 0 is YES.
        SceneOp::BranchChoice {
            if_yes: 6,
            if_no: 9,
        },
        // 6: `popdlg / cmpi.b #$3A, -$2(a0) / bne.s loc_72CDA`.
        SceneOp::BranchIfSavedDialogueByte {
            at: SavedByte::Last,
            value: 0x3A,
            if_equal: 7,
            if_not: 9,
        },
        // 7: `subi.l #50000, (Current_Money).w`.
        SceneOp::SubtractMoney { amount: BAIL },
        set_flag(FLAG_BAIL_PAID),
        // 9: `loc_72CDA: rts`.
    ],
};

/// `$0083`, `Event_GirlsBailedOut`, retail `$072CDC..$072D43`
/// (`ps4.asm:152674-152705`): the freed sister speaks entry `$61`,
/// `EventFlag_GirlsBailedOut` (`$7D`) is set, and the party is taken home to
/// Uzo: `UzoHouse3` (`$0E4`, previous map Uzo `$0E0`) at `($38, $46)` facing
/// up, alignment 4. The US body turns object 0 of the new map down after the
/// load (`if revision>0`, `$072D2A`), and the parents speak entry `$0A` of the
/// house's tree 25.
pub static GIRLS_BAILED_OUT: Scene = Scene {
    name: "Event_GirlsBailedOut",
    event: EventIndex(0x0083),
    ops: &[
        standard(0x61),
        set_flag(0x7D),
        SceneOp::PlaySound {
            id: MUSIC_A_HAPPY_SETTLEMENT,
        },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0x0E4,
            prev_map: 0x0E0,
            start_x: 0x38,
            start_y: 0x46,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        },
        // `lea (Field_Obj_Secondary).w, a4 / moveq #0, d0 / jsr
        // Event_UpdateObjFacing`.
        SceneOp::Face {
            actor: ActorRef::Npc(0),
            facing: Direction::Down,
        },
        SceneOp::FadeIn,
        standard(0x0A),
    ],
};

/// `$0022`, `Event_AiedoManSavings`, retail `$06D468..$06D4A5`
/// (`ps4.asm:146935-146948`): the man of `AiedoHouse3` (object 0) searches
/// for his savings in entry `$3D` until its `$F7`, turns to the leader, and
/// the text resumes where it closed (`popdlg` / `jsr Event_RunDialogue`,
/// `$5AC6C`). The body then clears `Game_Mode_Routine` and reloads the map's
/// chunks itself. Tree 11 entry `$18` (his dialogue id) is only
/// `$F6 $00 $22`, so it runs on every talk and sets no flag.
pub static AIEDO_MAN_SAVINGS: Scene = Scene {
    name: "Event_AiedoManSavings",
    event: EventIndex(0x0022),
    ops: &[
        standard(0x3D),
        face_leader(0),
        SceneOp::RunDialogueResume,
        // `move.w #0, (Game_Mode_Routine).w / movea.l (Map_Chunk_Addr).w, a0 /
        // jsr Map_LoadChunks`.
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
    ],
};

/// `$000E`, `Event_AiedoManCrushOnAlys`, retail `$06BE78..$06BEE7`
/// (`ps4.asm:145309-145337`, the US `grand_cross=0` body): the man on Aiedo's
/// street (object 5, `$FFFFC440`) turns to the leader and flatters Alys in
/// entry `$3C` until its `$F7`, then runs off: his routine becomes `$BC`
/// (`loc_489D6`, a walk up that clears its own slot once off screen), and only
/// he moves until he is gone. The `trap #0` clears his slot, and the party's
/// reaction resumes the same text. Tree 11 entry 7 (his dialogue id) fires it
/// while `EventFlag_Zio` (`$42`) is clear: `$FA $42 $01` sends a later party
/// to entry 8.
pub static AIEDO_MAN_CRUSH_ON_ALYS: Scene = Scene {
    name: "Event_AiedoManCrushOnAlys",
    event: EventIndex(0x000E),
    ops: &[
        face_leader(5),
        standard(0x3C),
        // `lea ($FFFFC440).w, a4 / move.w #$BC, (a4)` and `loc_6BE9E`.
        SceneOp::RunObjectRoutine {
            npc_index: 5,
            routine: 0xBC,
        },
        // `lea ($FFFFC440).w, a0 / move.w #$F, d7 / trap #0`.
        SceneOp::DespawnNpc {
            npc_index: 5,
            count: 1,
        },
        SceneOp::RunDialogueResume,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
    ],
};
