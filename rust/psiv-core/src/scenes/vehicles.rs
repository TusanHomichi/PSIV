//! The field menu's vehicle events: boarding a machine from the ITEM menu.
//!
//! `ItemAction_LandRover`, `ItemAction_IceDigger` and `ItemAction_HydroFoil`
//! (`ps4.asm:123419-123465`, `rust/psiv-runtime/src/item_action.rs`) write
//! `Event_Index` `9`, `$A` or `$B` and set `Routine_Exit_Flags` bit 1, which
//! `Field_MenuExit` turns into the field's own event routine
//! (`ps4.asm:117150-117169`): the menu is gone by the time these run, and the
//! party is standing on the tile the action accepted.
//!
//! The three bodies are the same routine with one object id and one art
//! address changed (`ps4.asm:144950-145008`, `:145009-145067`,
//! `:145068-145128`). Each:
//!
//! 1. collapses the followers onto the leader (`Event_OverlapCharacters`);
//! 2. rebuilds the sprite table (`Field_BuildSprites` + `VInt_Prepare`);
//! 3. writes `MusicID_LandMaster` into `Sound_Index` and `Saved_Sound_Index`
//!    only when `Saved_Sound_Index` differs;
//! 4. decompresses the machine's Nemesis art into `RAM_Start` and its sixteen
//!    palette words into `Palette_Line_4` (`$FFFFFB60`, CRAM line 4);
//! 5. writes the machine's object id into `Character_1` and clears the four
//!    follower object ids, so the field draws one wide object instead of a
//!    walking party;
//! 6. snaps X and standing Y to the 32-pixel vehicle grid; if either original
//!    coordinate had bit `$10`, updates objects, reloads/builds sprites, then
//!    blocks on `Event_MoveCamera` at speed 2;
//! 7. writes `Vehicle_Index` — `1`, `2` or `3`.
//!
//! A scene-only vehicle body replaces the party draw during the pan; the
//! persistent `Vehicle_Index` keeps that machine mounted afterwards. The pack
//! supplies its sheet and map palette. `AlignVehicleBoarding` owns the shared
//! coordinate transform and conditional refresh/pan gate, with the runtime's
//! existing camera glide completing before `Vehicle_Index` is set.

use crate::scene::SceneOp;
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::trigger::EventIndex;

/// `MusicID_LandMaster` (`ps4.constants.asm:930`).
const MUSIC_LAND_MASTER: u8 = 0x8D;

/// `RAM_Start` (`ps4.constants.asm:1990`): where the machine's Nemesis art is
/// decompressed.
const RAM_START: u32 = 0xFFFF_0000;
/// `Palette_Line_4` (`ps4.constants.asm:2424`): the machine's palette words.
const PALETTE_LINE_4: u32 = 0xFFFF_FB60;

/// The Land Rover's art and palette (`loc_296320`, `loc_29754E`).
const LAND_ROVER_ART: u32 = 0x0029_6320;
/// See [`LAND_ROVER_ART`].
const LAND_ROVER_PALETTE: u32 = 0x0029_754E;

/// The Ice Digger's art and palette (`loc_296A54`, `loc_29756E`).
const ICE_DIGGER_ART: u32 = 0x0029_6A54;
/// See [`ICE_DIGGER_ART`].
const ICE_DIGGER_PALETTE: u32 = 0x0029_756E;

/// The Hydrofoil's art and palette (`loc_2971A4`, `loc_29758E`).
const HYDROFOIL_ART: u32 = 0x0029_71A4;
/// See [`HYDROFOIL_ART`].
const HYDROFOIL_PALETTE: u32 = 0x0029_758E;

/// `Event_BoardingLandRover`, EventPtrs[`$09`] (`ps4.asm:144950-145008`).
pub static BOARDING_LAND_ROVER: Scene = Scene {
    name: "Event_BoardingLandRover",
    event: EventIndex(0x0009),
    ops: &[
        SceneOp::OverlapCharacters,
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::PlayMusicIfSavedDifferent {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: LAND_ROVER_ART,
                destination_ram: RAM_START,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: LAND_ROVER_PALETTE,
                destination_ram: PALETTE_LINE_4,
            },
        },
        SceneOp::AlignVehicleBoarding { index: 1 },
        SceneOp::SetVehicleIndex { index: 1 },
    ],
};

/// `Event_BoardingIceDigger`, EventPtrs[`$0A`] (`ps4.asm:145009-145067`).
pub static BOARDING_ICE_DIGGER: Scene = Scene {
    name: "Event_BoardingIceDigger",
    event: EventIndex(0x000A),
    ops: &[
        SceneOp::OverlapCharacters,
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::PlayMusicIfSavedDifferent {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: ICE_DIGGER_ART,
                destination_ram: RAM_START,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: ICE_DIGGER_PALETTE,
                destination_ram: PALETTE_LINE_4,
            },
        },
        SceneOp::AlignVehicleBoarding { index: 2 },
        SceneOp::SetVehicleIndex { index: 2 },
    ],
};

/// `Event_BoardingHydrofoil`, EventPtrs[`$0B`] (`ps4.asm:145068-145127`).
pub static BOARDING_HYDROFOIL: Scene = Scene {
    name: "Event_BoardingHydrofoil",
    event: EventIndex(0x000B),
    ops: &[
        SceneOp::OverlapCharacters,
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::PlayMusicIfSavedDifferent {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: HYDROFOIL_ART,
                destination_ram: RAM_START,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: HYDROFOIL_PALETTE,
                destination_ram: PALETTE_LINE_4,
            },
        },
        SceneOp::AlignVehicleBoarding { index: 3 },
        SceneOp::SetVehicleIndex { index: 3 },
    ],
};
