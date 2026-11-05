//! The two souvenirs the ITEM menu hangs up in Chaz's house (issue #81).
//!
//! `ItemAction_Pennant` and `ItemAction_WoodCarvin` (`ps4.asm:123470-123495`,
//! `rust/psiv-runtime/src/item_action.rs`) accept only on `ChazHouse`
//! (`$5E`), write `Event_Index` `$97` or `$98` and set `Routine_Exit_Flags`
//! bit 1, so these run with the menu gone. Each speaks one line, takes every
//! copy of the item, sets its temporary flag and reloads the house, whose map
//! data shows the souvenir while the flag is set. Record:
//! `docs/scenes/203_HomeDecor.md`.

use crate::geom::Direction;
use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `SFXID_Souvenir`.
const SFX_SOUVENIR: u8 = 0xF1;
/// `MapID_ChazHouse`.
const CHAZ_HOUSE: u16 = 0x05E;
/// `MapID_Aiedo`, written to `Field_Map_Index_2`.
const AIEDO: u16 = 0x054;

/// `$0097`, `Event_Pennant`, retail `$0736EA..$073753`
/// (`ps4.asm:153322-153345`).
pub static PENNANT: Scene = Scene {
    name: "Event_Pennant",
    event: EventIndex(0x0097),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x64)),
            window: DialogueWindow::Standard,
        },
        // `ItemID_Pennant`.
        SceneOp::RemoveItemsInField(0x94),
        SceneOp::FadeOut,
        // `TempEveFlag_Pennant`.
        SceneOp::SetFlag {
            flag: Flag::temp(0x1C),
            value: true,
        },
        SceneOp::PlaySound { id: SFX_SOUVENIR },
        SceneOp::LoadMap {
            map: CHAZ_HOUSE,
            prev_map: AIEDO,
            start_x: 0x50,
            start_y: 0x46,
            facing: Direction::Left,
            align: 0x10,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
    ],
};

/// `$0098`, `Event_WoodCarving`, retail `$073754..$0737BD`
/// (`ps4.asm:153346-153369`).
pub static WOOD_CARVING: Scene = Scene {
    name: "Event_WoodCarving",
    event: EventIndex(0x0098),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x66)),
            window: DialogueWindow::Standard,
        },
        // `ItemID_WoodCarvin`.
        SceneOp::RemoveItemsInField(0x95),
        SceneOp::FadeOut,
        // `TempEveFlag_WoodCarving`.
        SceneOp::SetFlag {
            flag: Flag::temp(0x1D),
            value: true,
        },
        SceneOp::PlaySound { id: SFX_SOUVENIR },
        SceneOp::LoadMap {
            map: CHAZ_HOUSE,
            prev_map: AIEDO,
            start_x: 0x38,
            start_y: 0x3E,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
    ],
};
