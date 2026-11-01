//! The strip club in the Hunter's Guild building (issue #81): the dancers'
//! entrance when the party steps up to the stage, and the temporary flag the
//! guild's doorway clears on the way out.
//!
//! Records are `docs/scenes/123_StripClub.md`. Both bodies were read from the
//! US image through their `EventPtrs` slots; the clone's labels are
//! navigation only.

use crate::scene::{ActorRef, SceneOp};
use crate::scene_presentation::{CameraSubject, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `TempEveFlag_StrippersDance` (`$1B`): set when the dancers come out,
/// cleared when the party leaves. Map trigger `$79` fires `$94` only while it
/// is clear, `$7A` fires `$95` only while it is set.
const STRIPPERS_DANCE: Flag = Flag::temp(0x1B);
/// `Sound_StopMusic`.
const STOP_MUSIC: u8 = 0xFB;
/// `MusicID_PaoPao`.
const MUSIC_PAO_PAO: u8 = 0xB0;
/// `FieldObj_Stripper`, walking out.
const STRIPPER: u16 = 0x344;
/// `FieldObj_StripperCoat`.
const COAT: u16 = 0x348;
/// `FieldObj_StripClubCustomer`.
const CUSTOMER: u16 = 0x34C;
/// `FieldObj_DancingStripper1..3`.
const DANCERS: [u16; 3] = [0x35C, 0x360, 0x364];
/// The tile every stripper, coat and dancer routine writes to `$16(a4)`: the
/// map's own upload.
const STRIPPER_TILE: u16 = 0x3BA;
/// `NemDecomp` of `$14A1CC` to tile `$372`, the customer's new art.
const CUSTOMER_ART: u32 = 0x0014_A1CC;
const CUSTOMER_TILE: u16 = 0x372;
/// `$FFFE` and `$FFFF` written to the high words of the coats' step
/// constants (`$20`, `$24`): two pixels left and one up a frame.
const COAT_STEP_X: i32 = -0x2_0000;
const COAT_STEP_Y: i32 = -0x1_0000;

const fn stripper(slot: usize, object_id: u16, x: i32, y: i32) -> SceneOp {
    SceneOp::CreateFieldObject {
        slot,
        object_id,
        art_tile: STRIPPER_TILE,
        x,
        y,
    }
}

const fn walk(slot: usize, x: i32, y: i32) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::WalkObject { slot, x, y },
    }
}

const fn drift(slot: usize) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::DriftObject {
            slot,
            step_x: COAT_STEP_X,
            step_y: COAT_STEP_Y,
        },
    }
}

/// `Field_UpdateObjects`, `Field_LoadSprites`, `Field_BuildSprites`,
/// `RunMapUpdates`, then one `VInt_Prepare`.
const REBUILD: SceneOp = SceneOp::Presentation {
    op: PresentationOp::RebuildSprites,
};
const VINT: SceneOp = SceneOp::WaitFrames { frames: 1 };

/// Frames of `Event_MoveObjectStatic` on stripper 1, from `($160,$190)` to
/// `($220,$210)`: twelve cells of X then eight of Y at eight frames a cell.
pub const STRIPPER_WALK_FRAMES: u16 = (12 + 8) * 8;

/// `$0094`, `Event_StrippersAppearing`, retail `$073440..$0735ED`
/// (`ps4.asm:153164-153251`): the music turns to PaoPao, the camera pans to
/// the stage, three strippers walk out (objects 1 to 3) and drop their coats
/// (objects 5 to 7), the camera returns to the leader, the strippers start to
/// dance and the customer (object 0) takes `FieldObj_StripClubCustomer`, with
/// new art. `TempEveFlag_StrippersDance` is the last write.
pub static STRIPPERS_APPEARING: Scene = Scene {
    name: "Event_StrippersAppearing",
    event: EventIndex(0x0094),
    ops: &[
        // 0-2: `Sound_StopMusic`, `DoMapUpdateLoop(9)`, PaoPao unless saved.
        SceneOp::PlaySound { id: STOP_MUSIC },
        SceneOp::Wait { ticks: 10 },
        SceneOp::PlayMusicIfSavedDifferent { id: MUSIC_PAO_PAO },
        // 3: `Event_MoveCamera` to `($220,$1C0)`, speed 1: the routine returns
        // on arrival, so nothing walks until the stage is framed.
        SceneOp::PanCamera {
            subject: CameraSubject::At { x: 0x220, y: 0x1C0 },
            speed: 1,
        },
        // 4-6: `$C340`, `$C380`, `$C3C0`.
        stripper(1, STRIPPER, 0x160, 0x190),
        stripper(2, STRIPPER, 0x150, 0x190),
        stripper(3, STRIPPER, 0x170, 0x190),
        REBUILD,
        VINT,
        // 9-10: `dest` words of objects 2 and 3.
        walk(2, 0x200, 0x1F0),
        walk(3, 0x240, 0x1F0),
        // 11-12: `Event_MoveObjectStatic` on object 1: every object runs until
        // it stands on `($220,$210)`.
        walk(1, 0x220, 0x210),
        SceneOp::Wait {
            ticks: STRIPPER_WALK_FRAMES,
        },
        // 13: `DoMapUpdateLoop($3B)`.
        SceneOp::Wait { ticks: 60 },
        // 14-19: `facing_dir = $10` on objects 1 to 3 (renderer state), then
        // the coats at `$C440`, `$C480`, `$C4C0`.
        stripper(5, COAT, 0x210, 0x210),
        drift(5),
        stripper(6, COAT, 0x1F0, 0x1F0),
        drift(6),
        stripper(7, COAT, 0x230, 0x1F0),
        drift(7),
        REBUILD,
        VINT,
        // 22: `DoMainUpdatesLoop($59)`.
        SceneOp::Wait { ticks: 90 },
        // 23: `$C400`, `$C440`, `$C480` cleared: objects 4 to 6. The third
        // coat (object 7) is left flying.
        SceneOp::DespawnNpc {
            npc_index: 4,
            count: 3,
        },
        // 24: the customer's art.
        SceneOp::LoadArt {
            rom_addr: CUSTOMER_ART,
            tile: CUSTOMER_TILE,
        },
        // 25: `Event_MoveCamera` to `Character_1`, speed 1: the rest of the
        // scene, and the player's control, wait for the view to come home.
        SceneOp::PanCamera {
            subject: CameraSubject::Actor(ActorRef::PartyMember(0)),
            speed: 1,
        },
        // 26-29: `Field_Obj_Secondary` and objects 1 to 3 take new routines.
        SceneOp::SetObjectRoutine {
            npc_index: 0,
            routine: CUSTOMER,
        },
        stripper(1, DANCERS[0], 0x220, 0x210),
        stripper(2, DANCERS[1], 0x200, 0x1F0),
        stripper(3, DANCERS[2], 0x240, 0x1F0),
        REBUILD,
        VINT,
        // 32: `moveq #TempEveFlag_StrippersDance, d0 / jmp TempEveFlags_Set`.
        SceneOp::SetFlag {
            flag: STRIPPERS_DANCE,
            value: true,
        },
    ],
};

/// `$0095`, `Event_ExitingStripClub`, retail `$0735EE..$0735F5`
/// (`ps4.asm:153254-153256`): `TempEveFlag_StrippersDance` is cleared, so the
/// dancers come out again on the next visit.
pub static EXITING_STRIP_CLUB: Scene = Scene {
    name: "Event_ExitingStripClub",
    event: EventIndex(0x0095),
    ops: &[SceneOp::SetFlag {
        flag: STRIPPERS_DANCE,
        value: false,
    }],
};
