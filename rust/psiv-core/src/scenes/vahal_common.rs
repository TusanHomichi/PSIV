//! Shared literals and op shorthands of the Vahal Fort and Weapon Plant
//! scenes (issue #82). Records are `docs/scenes/110_*` onward and
//! `docs/field/PLATFORMS_AND_BELTS.md`.

use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
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
pub(super) const PLATFORM_SETTLE_FRAMES: u16 = 60;

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
pub(super) const fn platform_object(slot: usize, x: i32, y: i32) -> SceneOp {
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
pub(super) const fn ride(slot: usize, step: i16, frames: u16) -> SceneOp {
    SceneOp::RidePlatform {
        slot,
        step_y: (step as i32) << 8,
        frames,
    }
}
