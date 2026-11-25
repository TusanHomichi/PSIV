//! Shared literals and op shorthands of the Vahal Fort and Weapon Plant
//! scenes (issue #82). Records are `docs/scenes/110_*` onward and
//! `docs/field/PLATFORMS_AND_BELTS.md`.

use crate::scene::{DialogueId, DialogueSource, DialogueWindow, RideOp, SceneOp};
use crate::state::Flag;

/// `SFXID_ConveyorBelt` (`ps4.constants.asm:1027`), the platforms' and the
/// belts' loop sound.
pub(super) const SFX_CONVEYOR_BELT: u8 = 0xE7;
/// `Sound_StopSFX` (`ps4.constants.asm:1056`).
pub(super) const SOUND_STOP_SFX: u8 = 0xFC;
/// `SFXID_ChestOpened`, `SFXID_AndroidSkillImplant`, `SFXID_Alarm`,
/// `SFXID_PowerDown`.
pub(super) const SFX_CHEST_OPENED: u8 = 0xE1;
pub(super) const SFX_ANDROID_SKILL_IMPLANT: u8 = 0xCF;
pub(super) const SFX_ALARM: u8 = 0xDB;
pub(super) const SFX_POWER_DOWN: u8 = 0xE4;

/// The object id `move.w #$124, (a4)` stores into a platform's temporary
/// object (`$06C494`).
pub(super) const PLATFORM_OBJECT: u16 = 0x124;

/// `moveq #$3B, d7` / `VInt_Prepare` / `dbf` (`$06C4E6..$06C4EE`): the
/// platform sits 60 frames before it moves.
const PLATFORM_SETTLE_FRAMES: u16 = 60;

/// `Event_GetAndRunDialogue` (`$5AC66`) on entry `entry` of the map's tree.
pub(super) const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

/// `moveq #id, d0 / jsr EventFlags_Set` (`$57666`).
pub(super) const fn set_event(id: u16) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value: true,
    }
}

/// `move.b #id, (Sound_Index).l`.
pub(super) const fn sound(id: u8) -> SceneOp {
    SceneOp::PlaySound { id }
}

/// `DoMapUpdateLoop` with the retail `d0`; the scene records `d0 + 1`.
pub(super) const fn wait(ticks: u16) -> SceneOp {
    SceneOp::Wait { ticks }
}

/// `TempEveFlags_Toggle` (`$576EA`).
pub(super) const fn toggle_temp(id: u16) -> SceneOp {
    SceneOp::ToggleFlag {
        flag: Flag::temp(id),
    }
}

/// `TempEveFlags_Test` (`$57638`) then `beq`: a set flag jumps to `if_set`.
pub(super) const fn test_temp(id: u16, if_set: usize, if_clear: usize) -> SceneOp {
    SceneOp::BranchFlag {
        flag: Flag::temp(id),
        if_set,
        if_clear,
    }
}

/// The platform object a ride starts from: `Field_LoadObject` takes the first
/// free object slot, `move.w #$124, (a4)` names it, and the start row's
/// `x`/`y - $20` become its `curr_x_pos`/`curr_y_pos` (`$06C494..$06C4A2`).
const fn platform_object(slot: usize, x: i32, y: i32) -> SceneOp {
    SceneOp::CreateFieldObject {
        slot,
        object_id: PLATFORM_OBJECT,
        art_tile: 0,
        x,
        y: y - 0x20,
    }
}

/// The ride loop with the table's step word `step` (`$0200` or `$FE00`):
/// `ext.l d0 / lsl.l #8, d0` makes it signed 16.16 pixels.
const fn ride(slot: usize, step: i16, frames: u16) -> SceneOp {
    SceneOp::Ride(RideOp::Platform {
        slot,
        step_y: (step as i32) << 8,
        frames,
    })
}

/// One flag state of a moving platform: its start-table row (`loc_6C5EC` and
/// siblings: `x`, `y`, the pair written where it rests) and its ride-table row
/// (`loc_6C5F8`: the step word, the frames `|dest - y| / 2`, the `$CC,$CD`
/// pair written where it stops).
pub(super) struct PlatformRow {
    /// The start row's `x` and `y`.
    pub(super) object: (i32, i32),
    /// The start pair, at `(x >> 5, y >> 5)` and the chunk to its right.
    pub(super) start: &'static [(u32, u32, u16)],
    /// The ride row's step word and the ride loop's frame count.
    pub(super) ride: (i16, u16),
    /// The end pair, at `(x >> 5, dest >> 5)` and the chunk to its right.
    pub(super) end: &'static [(u32, u32, u16)],
}

/// Ops in every [`platform`] scene.
pub(super) const PLATFORM_OPS: usize = 22;

/// A moving platform's routine (`Event_VahFortMovingPlatform1`, `$06C478`, and
/// its five siblings). Retail tests the temp flag twice, once to pick the start
/// row and once the ride row, and runs one body between and after them; the
/// ops do the same, so each of the body's two `DMAPlane_A_VInt` calls is one
/// [`SceneOp::DmaPlanes`] whichever way the platform goes.
pub(super) const fn platform(
    slot: usize,
    flag: u16,
    clear: PlatformRow,
    set: PlatformRow,
) -> [SceneOp; PLATFORM_OPS] {
    [
        // `lea start_table(pc), a3`, `TempEveFlags_Test`: set takes row 2.
        test_temp(flag, 5, 1),
        platform_object(slot, clear.object.0, clear.object.1),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: std::borrow::Cow::Borrowed(clear.start),
        },
        SceneOp::Jump { to: 8 },
        platform_object(slot, set.object.0, set.object.1),
        SceneOp::OverlapCharacters,
        SceneOp::WriteMapChunks {
            chunks: std::borrow::Cow::Borrowed(set.start),
        },
        // `RefreshPlaneFG`, `RunMapUpdates`, `DMAPlane_A_VInt` (`$06C4E0`):
        // the start pair's frame.
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames {
            frames: PLATFORM_SETTLE_FRAMES,
        },
        sound(SFX_CONVEYOR_BELT),
        // `lea ride_table(pc), a3` (`$06C4FA`), `TempEveFlags_Test` again.
        test_temp(flag, 16, 12),
        ride(slot, clear.ride.0, clear.ride.1),
        toggle_temp(flag),
        SceneOp::WriteMapChunks {
            chunks: std::borrow::Cow::Borrowed(clear.end),
        },
        SceneOp::Jump { to: 19 },
        ride(slot, set.ride.0, set.ride.1),
        toggle_temp(flag),
        SceneOp::WriteMapChunks {
            chunks: std::borrow::Cow::Borrowed(set.end),
        },
        // `RefreshPlaneFG`, `RunMapUpdates`, `DMAPlane_A_VInt` (`$06C5D8`):
        // the end pair's frame.
        SceneOp::DmaPlanes,
        SceneOp::DespawnNpc {
            npc_index: slot,
            count: 1,
        },
        sound(SOUND_STOP_SFX),
    ]
}
