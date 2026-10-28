//! Aiedo's side events (issue #81): Rocky's owner and the Hunter's Guild bail
//! quest, short bodies that face the leader, speak an entry of
//! `DialogueTree11` and set the quest's flags.
//!
//! Records are `docs/scenes/120_*` onward. Every body was read from the US
//! image through its `EventPtrs` slot; the clone's labels are navigation only.

use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SavedByte, SceneOp};
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

/// `lea (object).w, a4 / lea (Character_1).w, a3 / move.w $6(a3), d0 / bchg
/// #2, d0 / jsr Event_UpdateObjFacing`: map object `npc` (RAM `$FFFFC300 +
/// $40 * npc`) turns to face the party leader.
const fn face_leader(npc: usize) -> SceneOp {
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
