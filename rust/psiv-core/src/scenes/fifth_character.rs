//! `Event_PickingFifthCharacter` (`$68`, issue #83): the five friends waiting at
//! the Mota Spaceport after the reunion, one of whom may join as the fifth.
//!
//! Record: `docs/scenes/181_PickingFifthCharacter.md`. The scene is the
//! routine's legs as ops, indexing the row in `Window_Option_Index_2` the way
//! the routine does ([`crate::scene::FifthOp`], `scene_runner/fifth.rs`). Its
//! tables come from the pack ([`FifthTable`]).

use crate::scene::{ActorRef, FifthOp, SceneOp};
use crate::scene_presentation::CameraSubject;
use crate::scene_runner::Scene;
use crate::state::{CharId, Flag};
use crate::trigger::EventIndex;

/// `Character_1`.
const LEADER: ActorRef = ActorRef::PartyMember(0);

/// The first waiting friend's object, `$FFFFC440`, Gryz. Row `n` is object
/// `FIRST_OBJECT + n`.
pub(crate) const FIRST_OBJECT: usize = 5;

/// One row of `loc_72182`, eight bytes, as the pack carries it
/// (`psiv_tools/fifth_pack.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FifthRow {
    /// `$0`, where the friend stands: X in pixels.
    pub x: i32,
    /// `$2`: Y in pixels.
    pub y: i32,
    /// `$4`.
    pub who: CharId,
    /// `$5`, the first of the friend's lines: the pitch; `+2` asks and `+3`
    /// is the goodbye of the one who steps out.
    pub first_line: u16,
    /// `$6`, the `EventFlag_*Picked` set while this friend is the fifth.
    pub picked: u16,
}

/// The routine's two tables. The eighth byte of each row, the art offset of
/// the object's routine word, has no counterpart: the map's own record already
/// draws the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FifthTable {
    /// `loc_72182`, in the order of the objects at `$FFFFC440`.
    pub rows: [FifthRow; 5],
    /// `loc_721B8`: the records refilled whichever friend is spoken to.
    pub records: [CharId; 5],
}

/// `EventFlag_GryzPicked`, `HahnPicked`, `DemiPicked`, `RajaPicked`,
/// `KyraPicked`: the five immediates the routine clears (`$0720E8`).
const PICKED: [u16; 5] = [0xDC, 0xDB, 0xDD, 0xDE, 0xDF];

const fn clear_picked(index: usize) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(PICKED[index]),
        value: false,
    }
}

/// `$0068`, `Event_PickingFifthCharacter`, retail `$071E66..$0721CB` (code to
/// `$072181`, then `loc_72182`, `loc_721AA`, `loc_721B0` and `loc_721B8`).
pub static PICKING_FIFTH_CHARACTER: Scene = Scene {
    name: "Event_PickingFifthCharacter",
    event: EventIndex(0x0068),
    ops: &[
        // 0: `loc_71E74`, until every party object has stopped.
        SceneOp::FifthCharacter(FifthOp::WaitParty),
        // 1: `loc_71EAE`.
        SceneOp::FifthCharacter(FifthOp::RefillRecords),
        // 2: `$071EC6..$071F2C`, the row under the leader's facing, and that
        // friend turns to the leader.
        SceneOp::FifthCharacter(FifthOp::PickRow),
        // 3: `$071F30..$071F3B`, `$5(a0)`.
        SceneOp::FifthCharacter(FifthOp::RunLine { offset: 0 }),
        // 4: `$071F3C..$071F4B`, `$5(a0)` plus 2.
        SceneOp::FifthCharacter(FifthOp::RunLine { offset: 2 }),
        // 5: `tst.b (Yes_No_Option).w / bne.w loc_72128`: 0 is YES.
        SceneOp::BranchChoice {
            if_yes: 6,
            if_no: 18,
        },
        // 6: `tst.b (Current_Party_Slot_5).w / bmi.w loc_72080`.
        SceneOp::FifthCharacter(FifthOp::BranchFifthSlotFree {
            if_free: 11,
            if_taken: 7,
        }),
        // 7: `bsr loc_72156`, the party slot of the friend already along.
        SceneOp::FifthCharacter(FifthOp::PickLeaver),
        // 8: `$071F80`: that friend's line, `$5(a0)` plus 3.
        SceneOp::FifthCharacter(FifthOp::RunLeaverLine),
        // 9: `$071F8C..$07207C`.
        SceneOp::FifthCharacter(FifthOp::Exchange),
        SceneOp::SkipOps { count: 1 },
        // 11: `loc_72080`.
        SceneOp::FifthCharacter(FifthOp::TakeFifthSlot),
        // 12: `loc_720E8`.
        clear_picked(0),
        clear_picked(1),
        clear_picked(2),
        clear_picked(3),
        clear_picked(4),
        // 17: `$6(a0)`.
        SceneOp::FifthCharacter(FifthOp::SetPicked),
        // 18: `loc_72128`: `Event_MoveCamera` to the leader, speed 2, on every
        // leg.
        SceneOp::PanCamera {
            subject: CameraSubject::Actor(LEADER),
            speed: 2,
        },
    ],
};
