//! The Weapon Plant's events (issue #82): the arrival conversation, the
//! Burst Roc unit, the belt-direction terminal and the four moving platforms.
//!
//! Records are `docs/scenes/110_*` onward and `docs/field/PLATFORMS_AND_BELTS.md`.
//! Every body was read from the US image through its `EventPtrs` slot
//! (`$05A2B4`); the clone's labels are navigation only.

use super::WREN;
use super::vahal_common::{
    PLATFORM_SETTLE_FRAMES, SFX_ALARM, SFX_ANDROID_SKILL_IMPLANT, SFX_CHEST_OPENED,
    SFX_CONVEYOR_BELT, SOUND_STOP_SFX, platform_object, ride, set_event, sound, standard,
    test_temp, toggle_temp, wait,
};
use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::scene_runner::drift::CoordCmp;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

/// Weapon Plant F1 holds five map objects, so `Field_LoadObject` hands a
/// platform the sixth object slot (`$FFFFC440`).
const PLATFORM_SLOT: usize = 5;

/// `$0092`, `Event_WeaponPlantArrival`, retail `$0733BA..$0733CB`: what the
/// plant is, then `EventFlag_WeaponPlant` (`$C7`).
pub static WEAPON_PLANT_ARRIVAL: Scene = Scene {
    name: "Event_WeaponPlantArrival",
    event: EventIndex(0x0092),
    ops: &[standard(7), set_event(0xC7)],
};

/// `$006E`, `Event_Burstroc`, retail `$0724A4..$07251F`: Wren installs the
/// Burst Roc unit from the chest at `$FFFFC4C0` (object 7, the chest after
/// Weapon Plant F2's seven map objects) and `EventFlag_Burstroc` (`$74`)
/// records it.
pub static BURSTROC: Scene = Scene {
    name: "Event_Burstroc",
    event: EventIndex(0x006E),
    ops: &[
        sound(SFX_CHEST_OPENED),
        SceneOp::Face {
            actor: ActorRef::Npc(7),
            facing: Direction::Up,
        },
        wait(40),
        standard(8),
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
            slot: 5,
            skill: 0x0E,
        },
        set_event(0x74),
    ],
};

/// `$001C`, `Event_WeaponPlantTerminal`, retail `$06CE18..$06CE5B`: the same
/// terminal as Vahal Fort's, splitting the room at `x = $300` (`$12` from
/// there on, `$11` below it).
pub static WEAPON_PLANT_TERMINAL: Scene = Scene {
    name: "Event_WeaponPlantTerminal",
    event: EventIndex(0x001C),
    ops: &[
        sound(SFX_ALARM),
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 3 },
        },
        sound(SFX_ALARM),
        SceneOp::Presentation {
            op: PresentationOp::FadeFromRed { lines: 3 },
        },
        SceneOp::BranchIfActorCoord {
            actor: LEADER,
            axis: Axis::X,
            cmp: CoordCmp::Below,
            value: 0x300,
            if_true: 7,
            if_false: 5,
        },
        toggle_temp(0x12),
        SceneOp::End,
        toggle_temp(0x11),
    ],
};

/// `$0017`, `Event_WpnPlntMovingPlatform1`, retail `$06C798..$06C927`: the
/// platform at x `$1C0` (chunks 14-15), at row 21 with temp flag `$0D` clear or
/// row 28 with it set, 112 frames between them.
pub static WPN_PLNT_MOVING_PLATFORM_1: Scene = Scene {
    name: "Event_WpnPlntMovingPlatform1",
    event: EventIndex(0x0017),
    ops: &[
        test_temp(0x0D, 12, 1),
        platform_object(PLATFORM_SLOT, 0x1C0, 0x2A0),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(14, 21, 0xC2), (15, 21, 0xC3)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, 0x0200, 112),
        toggle_temp(0x0D),
        SceneOp::WriteMapChunks {
            chunks: &[(14, 28, 0xCC), (15, 28, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
        SceneOp::End,
        platform_object(PLATFORM_SLOT, 0x1C0, 0x380),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(14, 28, 0xC0), (15, 28, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, -0x0200, 112),
        toggle_temp(0x0D),
        SceneOp::WriteMapChunks {
            chunks: &[(14, 21, 0xCC), (15, 21, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ],
};

/// `$0018`, `Event_WpnPlntMovingPlatform2`, retail `$06C928..$06CAB7`: x `$220`
/// (chunks 17-18), row 11 with temp flag `$0E` clear or row 18 with it set,
/// 112 frames between them.
pub static WPN_PLNT_MOVING_PLATFORM_2: Scene = Scene {
    name: "Event_WpnPlntMovingPlatform2",
    event: EventIndex(0x0018),
    ops: &[
        test_temp(0x0E, 12, 1),
        platform_object(PLATFORM_SLOT, 0x220, 0x160),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(17, 11, 0xC2), (18, 11, 0xC3)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, 0x0200, 112),
        toggle_temp(0x0E),
        SceneOp::WriteMapChunks {
            chunks: &[(17, 18, 0xCC), (18, 18, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
        SceneOp::End,
        platform_object(PLATFORM_SLOT, 0x220, 0x240),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(17, 18, 0xC0), (18, 18, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, -0x0200, 112),
        toggle_temp(0x0E),
        SceneOp::WriteMapChunks {
            chunks: &[(17, 11, 0xCC), (18, 11, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ],
};

/// `$0019`, `Event_WpnPlntMovingPlatform3`, retail `$06CAB8..$06CC47`: x `$380`
/// (chunks 28-29), row 11 with temp flag `$0F` clear or row 18 with it set,
/// 112 frames between them.
pub static WPN_PLNT_MOVING_PLATFORM_3: Scene = Scene {
    name: "Event_WpnPlntMovingPlatform3",
    event: EventIndex(0x0019),
    ops: &[
        test_temp(0x0F, 12, 1),
        platform_object(PLATFORM_SLOT, 0x380, 0x160),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(28, 11, 0xC2), (29, 11, 0xC3)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, 0x0200, 112),
        toggle_temp(0x0F),
        SceneOp::WriteMapChunks {
            chunks: &[(28, 18, 0xCC), (29, 18, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
        SceneOp::End,
        platform_object(PLATFORM_SLOT, 0x380, 0x240),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(28, 18, 0xC0), (29, 18, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, -0x0200, 112),
        toggle_temp(0x0F),
        SceneOp::WriteMapChunks {
            chunks: &[(28, 11, 0xCC), (29, 11, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ],
};

/// `$001A`, `Event_WpnPlntMovingPlatform4`, retail `$06CC48..$06CDD3`: x `$3E0`
/// (chunks 31-32), row 21 with temp flag `$10` clear or row 28 with it set,
/// 112 frames between them.
pub static WPN_PLNT_MOVING_PLATFORM_4: Scene = Scene {
    name: "Event_WpnPlntMovingPlatform4",
    event: EventIndex(0x001A),
    ops: &[
        test_temp(0x10, 12, 1),
        platform_object(PLATFORM_SLOT, 0x3E0, 0x2A0),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(31, 21, 0xC2), (32, 21, 0xC3)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, 0x0200, 112),
        toggle_temp(0x10),
        SceneOp::WriteMapChunks {
            chunks: &[(31, 28, 0xCC), (32, 28, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
        SceneOp::End,
        platform_object(PLATFORM_SLOT, 0x3E0, 0x380),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: &[(31, 28, 0xC0), (32, 28, 0xC1)],
        },
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        ride(PLATFORM_SLOT, -0x0200, 112),
        toggle_temp(0x10),
        SceneOp::WriteMapChunks {
            chunks: &[(31, 21, 0xCC), (32, 21, 0xCD)],
        },
        SceneOp::DespawnNpc {
            npc_index: PLATFORM_SLOT,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ],
};
