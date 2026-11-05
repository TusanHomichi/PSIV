//! Six direct interactions of the late arc (issue #83), each a short branch on
//! the state at the moment of the talk: the musk cats of the Mystery Valley,
//! Zosa's penguin keeper, Raja in the Meese clinic, Rune at the Inner Sanctuary
//! and Kuran's Hijammer chest.
//!
//! Record: `docs/scenes/180_LateArcInteractions.md`. Every body was read from
//! the US image through its `EventPtrs` slot.

use crate::geom::Direction;
use crate::scene::{ActorRef, MoneyTest, SavedByte, SceneOp};
use crate::scene_runner::Scene;
use crate::scenes::WREN;
use crate::scenes::recovery::RECOVERY_OPS as R;
use crate::scenes::vahal_common::{
    SFX_ANDROID_SKILL_IMPLANT, SFX_CHEST_OPENED, set_event, sound, standard, wait,
};
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// `EventFlag_PenguinNoMoney`, read by entry `$12`'s `$FA` to pick the keeper's
/// answer to a YES.
const FLAG_PENGUIN_NO_MONEY: u16 = 0x8C;
/// `EventFlag_DarkForce2`.
const FLAG_DARK_FORCE_2: u16 = 0x9E;
/// `cmpi.l #500, (Current_Money).w` and the `subi.l` that follows it.
const PENGUIN_FEE: u32 = 500;
/// The keeper's object, `$FFFFC700`: the seventeenth of Zosa's.
const PENGUIN_KEEPER: usize = 16;
/// Raja in bed, `$FFFFC400`.
const RAJA_IN_BED: usize = 4;

/// `lea (object).w, a4 / lea (Character_1).w, a3 / move.w $6(a3), d0 / bchg #2,
/// d0 / jsr Event_UpdateObjFacing`. Follow it with [`FACING_VINT`].
const fn face_leader(npc: usize) -> SceneOp {
    SceneOp::FaceOppositeOf {
        actor: ActorRef::Npc(npc),
        of: LEADER,
    }
}

/// The `jsr VInt_Prepare` that ends US retail's `Event_UpdateObjFacing`
/// (`$05A972`, in the `if grand_cross=0` block): one frame with no map update,
/// whose VInt still runs `UpdateRNGSeed` and counts `Main_Frame_Count`
/// (`ps4.asm:617,623`). The port's facing ops take no frame (#164), so these
/// scenes spend it here.
const FACING_VINT: SceneOp = SceneOp::WaitFrames { frames: 1 };

const fn set_npc_dialogue(npc_index: usize, dialogue_id: u16) -> SceneOp {
    SceneOp::SetNpcDialogue {
        npc_index,
        dialogue_id,
    }
}

/// `$0049`, `Event_MuskCatsGuarding`, retail `$07069E..$0706BF`: the two guards
/// at `MystVale_Part2`'s entrance speak, take entry `$1D` from now on, and
/// `EventFlag_MuskCats` (`$90`) sends them aside (`FieldObj_MuskCatGuard`).
pub static MUSK_CATS_GUARDING: Scene = Scene {
    name: "Event_MuskCatsGuarding",
    event: EventIndex(0x0049),
    ops: &[
        standard(0x1C),
        set_npc_dialogue(0, 0x1D),
        set_npc_dialogue(1, 0x1D),
        set_event(0x90),
    ],
};

/// `$004A`, `Event_MuskCatElder`, retail `$0706C0..$070701`: the chief gives the
/// Silver Tusk (`$53`) and `EventFlag_SilverTusk` (`$91`) records it. With all
/// forty slots taken he says entry `$2E`, which ends before the gift, and
/// nothing is written, so he offers it again on the next talk.
pub static MUSK_CAT_ELDER: Scene = Scene {
    name: "Event_MuskCatElder",
    event: EventIndex(0x004A),
    ops: &[
        // 0: the `dbf` count of non-zero slots / `cmpi.w #40, d0 / bcs.w`.
        SceneOp::BranchIfInventoryFull {
            if_full: 1,
            if_not: 3,
        },
        // 1: `moveq #$2E, d0 / jmp Event_GetAndRunDialogue`.
        standard(0x2E),
        SceneOp::End,
        // 3: `loc_706E2`.
        standard(0x2D),
        // 4: the first zero byte of `Inventory` takes `ItemID_SilvrTusk`.
        SceneOp::AddItem { item: 0x53 },
        set_event(0x91),
    ],
};

/// `$004B`, `Event_PenguinOwner`, retail `$070702..$070773`: the purse sets or
/// clears `EventFlag_PenguinNoMoney` (`$8C`) before entry `$12`, whose `$FA` and
/// `$F5` pick among three endings. Only the YES with the money ends on `!`
/// (`$3A`, `revision>0`); it pays, the keeper takes entry `$15` from now on, and
/// `EventFlag_Penguin` (`$8A`) sets `FieldObj_FellowPenguin` following.
pub static PENGUIN_OWNER: Scene = Scene {
    name: "Event_PenguinOwner",
    event: EventIndex(0x004B),
    ops: &[
        face_leader(PENGUIN_KEEPER),
        FACING_VINT,
        // 2: `cmpi.l #500, (Current_Money).w / bcc.s`.
        SceneOp::BranchIfMoneyBelow {
            amount: PENGUIN_FEE,
            test: MoneyTest::Unsigned,
            if_below: 3,
            if_not: 5,
        },
        // 3: `EventFlags_Set`, `bra.s loc_70738`.
        set_event(FLAG_PENGUIN_NO_MONEY),
        SceneOp::SkipOps { count: 1 },
        // 5: `EventFlags_Clear`.
        SceneOp::SetFlag {
            flag: Flag::event(FLAG_PENGUIN_NO_MONEY),
            value: false,
        },
        // 6: `loc_70738`.
        standard(0x12),
        // 7: `tst.b (Yes_No_Option).w / bne.s loc_70772`: 0 is YES.
        SceneOp::BranchChoice {
            if_yes: 8,
            if_no: 12,
        },
        // 8: `popdlg / cmpi.b #$3A, -$2(a0) / bne.s loc_70772`.
        SceneOp::BranchIfSavedDialogueByte {
            at: SavedByte::Last,
            value: 0x3A,
            if_equal: 9,
            if_not: 12,
        },
        // 9: `subi.l #500, (Current_Money).w`.
        SceneOp::SubtractMoney {
            amount: PENGUIN_FEE,
        },
        set_npc_dialogue(PENGUIN_KEEPER, 0x15),
        set_event(0x8A),
        // 12: `loc_70772: rts`.
    ],
};

/// `$005B`, `Event_RajaSick`, retail `$070C30..$070C81`: Raja in bed turns to
/// the party. Before Dark Force 2 falls he says one of three lines, chosen by
/// `UpdateRNGSeed` and the seed's low two bits (`$5E` on 0, `$5F` on 1, `$5D` on
/// 2 and 3); after, entry `$60`.
pub static RAJA_SICK: Scene = Scene {
    name: "Event_RajaSick",
    event: EventIndex(0x005B),
    ops: &[
        face_leader(RAJA_IN_BED),
        // The facing's frame comes before the draw: its VInt stirs the seed.
        FACING_VINT,
        SceneOp::BranchFlag {
            flag: Flag::event(FLAG_DARK_FORCE_2),
            if_set: 10,
            if_clear: 3,
        },
        // 3: `jsr UpdateRNGSeed / move.w (RNG_Seed).w, d0 / andi.w #3, d0`, and
        // the `beq.s` / `cmpi.w #1, d0 / beq.s` under it.
        SceneOp::BranchOnRngLow2 {
            targets: [4, 6, 8, 8],
        },
        // 4: `loc_70C6C`.
        standard(0x5E),
        SceneOp::End,
        // 6: `loc_70C70`.
        standard(0x5F),
        SceneOp::End,
        // 8: `moveq #$5D, d0 / bra.s loc_70C72`.
        standard(0x5D),
        SceneOp::End,
        // 10: `loc_70C78`.
        standard(0x60),
    ],
};

/// `$0067`, `Event_RuneHealingChaz`, retail `$071E56..$071E65`: Rune's line,
/// then `jsr Event_Recovery` (the census's chain of `$21`, whose ops these are).
pub static RUNE_HEALING_CHAZ: Scene = Scene {
    name: "Event_RuneHealingChaz",
    event: EventIndex(0x0067),
    ops: &[
        standard(0x2B),
        R[0],
        R[1],
        R[2],
        R[3],
        R[4],
        R[5],
        R[6],
        R[7],
        R[8],
        R[9],
        R[10],
        R[11],
        R[12],
        R[13],
        R[14],
        R[15],
    ],
};

/// `$006D`, `Event_Hijammer`, retail `$072428..$0724A3`: Wren installs Hijammer
/// from the chest at `$FFFFC340` (object 1, the chest after `Kuran_F1_Part2`'s
/// one map object), the shape of `Event_Burstroc`. `EventFlag_Hijammer` (`$5E`)
/// records it.
pub static HIJAMMER: Scene = Scene {
    name: "Event_Hijammer",
    event: EventIndex(0x006D),
    ops: &[
        sound(SFX_CHEST_OPENED),
        SceneOp::Face {
            actor: ActorRef::Npc(1),
            facing: Direction::Up,
        },
        wait(40),
        standard(0x0A),
        wait(40),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(40),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(20),
        sound(SFX_ANDROID_SKILL_IMPLANT),
        wait(60),
        SceneOp::RunDialogueResume,
        SceneOp::SetCharacterSkill {
            who: WREN,
            slot: 4,
            skill: 0x20,
        },
        set_event(0x5E),
    ],
};
