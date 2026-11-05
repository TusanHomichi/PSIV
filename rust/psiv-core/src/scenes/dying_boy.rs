//! Torinco's dying boy (issue #81): the Alis Sword cures Culvers's son.
//!
//! The record is `docs/scenes/163_DyingBoy.md`. The body was read from the
//! US image through its `EventPtrs` slot; the clone's labels are navigation
//! only.

use super::aiedo_events::{face_leader, set_flag, standard};
use super::vahal_fort::go;
use super::{CHAZ, DEMI, GRYZ, HAHN, KYRA, RAJA, RIKA, RUNE, SETH, WREN};
use crate::geom::Direction;
use crate::scene::{ActorRef, ItemOp, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `EventFlag_Culvers`: the father has asked for help.
const CULVERS: u16 = 0xA9;
/// `EventFlag_AlisSword`: the boy is cured.
const ALIS_SWORD: u16 = 0xAA;
/// `ItemID_AlisSword`.
const ITEM_ALIS_SWORD: u8 = 0x8A;
/// `CulversHouse` object 2 (`$FFFFC380`), the boy in bed.
const BOY: usize = 2;
/// `$FFFFC480`: the slot `FieldObj_ChazAlisSword` runs in, past the map's six
/// objects.
const SWORD_SLOT: usize = 6;
/// `FieldObjectsJmpTbl` `$334`, `FieldObj_ChazAlisSword`.
const SWORD_ROUTINE: u16 = 0x334;
/// The routine's `move.w #$43C, $16(a4)`.
const SWORD_TILE: u16 = 0x43C;
/// Frames of the `loc_72EF4` loop, the routine's first pass included: the
/// script at `loc_4C6F6` runs ten mappings of 4, 4, 44, 4, 44, 4, 4, 44, 4 and
/// 4 frames, and the routine writes `$FFFF` to `$1C` on the call that leaves
/// mapping 9 with a duration of 1. From a cleared slot that is call 164.
pub const SWORD_FRAMES: u16 = 164;

/// `Char_Move_Flags` bit 2 (camera lock) and bit 0 (follow chain off).
const CAMERA_LOCK: u8 = 0b100;
const FOLLOW_OFF: u8 = 0b001;

/// Op indices of the two short arms, and of the first op past the
/// fifth-member chain.
const NOT_YET: usize = 50;
const ALREADY: usize = 52;
const AFTER_FIFTH: usize = 28;

/// One test of the fifth-member chain (`$072E3A..$072E7F`): `Event_GetCharacter`
/// then `bmi` to the next test or `bra loc_72E80`.
const fn fifth(who: crate::state::CharId, at: usize) -> SceneOp {
    SceneOp::BranchIfPartyMember {
        who,
        if_present: 16 + 2 * (at - 10),
        if_absent: at + 1,
    }
}

/// `$0086`, `Event_DyingBoy`, retail `$072D94..$072FB3`
/// (`ps4.asm:152726-152855`): the boy (`CulversHouse` object 2) turns to the
/// leader. Once the sword has been used he says entry `$3E`; before Culvers
/// has asked, or without an Alis Sword `GetItem` can find, entry `$3B`.
/// Otherwise the sword is taken, the party gathers at the bed, Chaz raises the
/// sword, walks to the foot of the bed and the party steps back, and
/// `EventFlag_AlisSword` is the last write.
pub static DYING_BOY: Scene = Scene {
    name: "Event_DyingBoy",
    event: EventIndex(0x0086),
    ops: &[
        // 0: `$072D94..$072DA9`.
        face_leader(BOY),
        // 1-3: `$072DAA..$072DDB`: the three tests, then `clr.b (a0)` and
        // `ReorderInventory`.
        SceneOp::BranchFlag {
            flag: Flag::event(ALIS_SWORD),
            if_set: ALREADY,
            if_clear: 2,
        },
        SceneOp::BranchFlag {
            flag: Flag::event(CULVERS),
            if_set: 3,
            if_clear: NOT_YET,
        },
        SceneOp::Item(ItemOp::Take {
            item: ITEM_ALIS_SWORD,
            if_missing: NOT_YET,
        }),
        // 4: `$072DDC..$072DE3`: entry `$33` up to its first stop.
        standard(0x33),
        // 5: `$072DE4`: `bset #2, (Char_Move_Flags).w`.
        SceneOp::SetFollowMode { bits: CAMERA_LOCK },
        // 6-9: `$072DEA..$072E39`: Chaz, Rune, Rika and Wren's `dest` words,
        // with no membership test.
        go(CHAZ, 0x1F0, 0x1C0),
        go(RUNE, 0x1E0, 0x1A0),
        go(RIKA, 0x1F0, 0x1A0),
        go(WREN, 0x200, 0x1A0),
        // 10-15: `$072E3A..$072E7F`: the fifth member, first hit wins.
        fifth(HAHN, 10),
        fifth(GRYZ, 11),
        fifth(DEMI, 12),
        fifth(RAJA, 13),
        fifth(KYRA, 14),
        SceneOp::BranchIfPartyMember {
            who: SETH,
            if_present: 26,
            if_absent: AFTER_FIFTH,
        },
        // 16-27: `loc_72E80`, `$072E80..$072E8B`: `($210,$1A0)` for the member
        // found.
        go(HAHN, 0x210, 0x1A0),
        SceneOp::Jump { to: AFTER_FIFTH },
        go(GRYZ, 0x210, 0x1A0),
        SceneOp::Jump { to: AFTER_FIFTH },
        go(DEMI, 0x210, 0x1A0),
        SceneOp::Jump { to: AFTER_FIFTH },
        go(RAJA, 0x210, 0x1A0),
        SceneOp::Jump { to: AFTER_FIFTH },
        go(KYRA, 0x210, 0x1A0),
        SceneOp::Jump { to: AFTER_FIFTH },
        go(SETH, 0x210, 0x1A0),
        SceneOp::Jump { to: AFTER_FIFTH },
        // 28-30: `loc_72E8C`, `$072E8C..$072E9F`: `FieldObj_Step_Offset` 0,
        // the follow chain off, `DoMainUpdatesLoop($95)`.
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::SetFollowMode {
            bits: CAMERA_LOCK | FOLLOW_OFF,
        },
        SceneOp::Wait { ticks: 150 },
        // 31-33: `$072EA0..$072EC5`: `loc_5A97C` with d0 = 0, Chaz faces left,
        // `popdlg` / `Event_RunDialogue`.
        SceneOp::FaceParty {
            facing: Direction::Down,
        },
        SceneOp::Face {
            actor: ActorRef::Character(CHAZ),
            facing: Direction::Left,
        },
        SceneOp::RunDialogueResume,
        // 34-36: `$072EC6..$072F17`: Chaz's sprite off (`bset #1, $2(a4)`),
        // `FieldObj_ChazAlisSword` in slot 6, run until it writes `$FFFF` to
        // `$1C`. Only that object runs, with no map update.
        SceneOp::Presentation {
            op: PresentationOp::SetCharacterVisible {
                who: CHAZ,
                visible: false,
            },
        },
        SceneOp::ObjectAnimation {
            slot: SWORD_SLOT,
            object_id: SWORD_ROUTINE,
            art_tile: SWORD_TILE,
            frames: SWORD_FRAMES,
        },
        SceneOp::WaitFrames {
            frames: SWORD_FRAMES,
        },
        // 37-40: `$072F18..$072F43`: Chaz back, slot 6 cleared, one pass and
        // a `VInt_Prepare`.
        SceneOp::Presentation {
            op: PresentationOp::SetCharacterVisible {
                who: CHAZ,
                visible: true,
            },
        },
        SceneOp::DespawnNpc {
            npc_index: SWORD_SLOT,
            count: 1,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        // 41-42: `$072F44..$072F67`: `popdlg` / `Event_RunDialogue`, then
        // `Event_MoveSingleObject` on Chaz to `($1A0,$1C0)`: only he moves.
        SceneOp::RunDialogueResume,
        SceneOp::MoveActorTo {
            actor: ActorRef::Character(CHAZ),
            x: 0x1A0,
            y: 0x1C0,
            wait: true,
        },
        // 43-45: `$072F68..$072F8D`: the follow chain back on, `popdlg` /
        // `Event_RunDialogue`, `Event_MoveObjectStatic` on `Character_1` to
        // `($1A0,$1D0)` with every object running.
        SceneOp::SetFollowMode { bits: CAMERA_LOCK },
        SceneOp::RunDialogueResume,
        SceneOp::MoveActorTo {
            actor: ActorRef::PartyMember(0),
            x: 0x1A0,
            y: 0x1D0,
            wait: true,
        },
        // 46-49: `$072F8E..$072FA3`: step offset 1, `Char_Move_Flags` clear,
        // `EventFlag_AlisSword`.
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::SetFollowMode { bits: 0 },
        set_flag(ALIS_SWORD),
        SceneOp::End,
        // 50-51: `loc_72FA4`.
        standard(0x3B),
        SceneOp::End,
        // 52: `loc_72FAC`.
        standard(0x3E),
    ],
};
