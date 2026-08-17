//! Retail event bodies after `Cutscene_MeetingRika`.
//!
//! The pointer table and the trigger table do not describe one straight
//! hallway.  `$2B` and `$06` are the Land Rover/Machine Center pair, `$2E`
//! and `$2F` are the Ladea Tower pair, and `$34` is the Nurvus event.  The
//! cutscene hand-offs that sit between them live in `post_rika_cutscenes`.
//! These records keep the cartridge's state writes in the core and carry the
//! RAM/VDP choreography as typed presentation operations.

use super::RUNE;
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

const MUSIC_STOP: u8 = 0xFB;
const MUSIC_LAND_MASTER: u8 = 0x8D;
const MUSIC_ENEMY_APPEARANCE: u8 = 0xA3;
const MUSIC_THE_BLACK_BLOOD: u8 = 0xA8;
const SFX_GRAVE_OPENING: u8 = 0xDD;
const SFX_BARRIER_BROKEN: u8 = 0xE6;
const SFX_CONVEYOR_BELT: u8 = 0xE7;
const SFX_SPACESHIP_RADAR: u8 = 0xF8;
const SOUND_STOP_SPC: u8 = 0xFD;
const SOUND_STOP_ALL: u8 = 0xFE;

const ITEM_LAND_ROVER: u8 = 0x96;
const ITEM_CONTROL_KEY: u8 = 0x99;

/// `Event_MachineCenterAppearing`, EventPtrs[$06], retail
/// `$06B4B2..$06B6F3` (578 bytes).
pub static MACHINE_CENTER_APPEARING: Scene = Scene {
    name: "Event_MachineCenterAppearing",
    event: EventIndex(0x0006),
    ops: &[
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D3710,
                destination_ram: 0xFFFF0000,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D2ABC,
                destination_ram: 0xFFFF0800,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D2F4E,
                destination_ram: 0xFFFF1000,
            },
        },
        SceneOp::MoveCamera {
            x: 0x730,
            y: 0xB40,
            speed: 1,
        },
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        // `moveq #$170` + `dbf`: 369 map-update iterations.
        SceneOp::Wait { ticks: 369 },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x78,
            art_tile: 0x04BE,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x84,
            art_tile: 0x0000,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x7C,
            art_tile: 0x0000,
            frames: 24,
        },
        SceneOp::MoveCamera {
            x: 0x730,
            y: 0xB40,
            speed: 2,
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0B)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x43),
            value: true,
        },
    ],
};

/// `Event_GettingLandRover`, EventPtrs[$2B], retail
/// `$06DEBE..$06E0E9` (556 bytes).
pub static GETTING_LAND_ROVER: Scene = Scene {
    name: "Event_GettingLandRover",
    event: EventIndex(0x002B),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(9)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::SetFollowMode { bits: 1 },
        // `moveq #$63` + `DoMainUpdatesLoop`: 100 iterations.
        SceneOp::Wait { ticks: 100 },
        SceneOp::RunDialogueResume,
        SceneOp::PlaySound {
            id: SFX_SPACESHIP_RADAR,
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::MoveCamera {
            x: 0x1E0,
            y: 0x200,
            speed: 1,
        },
        SceneOp::PlaySound {
            id: SFX_CONVEYOR_BELT,
        },
        // `move.w #$167,d7` + `dbf`: 360 iterations.
        SceneOp::Wait { ticks: 360 },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 0,
                x: 0x1E0,
                y: 0x190,
            },
        },
        SceneOp::Wait { ticks: 30 },
        SceneOp::RunDialogueResume,
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x190,
            wait: true,
        },
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::FadeOut,
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::Wait { ticks: 15 },
        SceneOp::SetSavedMusic {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::SetVehicleIndex { index: 1 },
        SceneOp::LoadMap {
            map: 0x00,
            prev_map: 0xB7,
            start_x: 0xE4,
            start_y: 0x160,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::FadeIn,
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0C)),
            window: DialogueWindow::Standard,
        },
        SceneOp::RemoveItem {
            item: ITEM_CONTROL_KEY,
        },
        SceneOp::AddItem {
            item: ITEM_LAND_ROVER,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x44),
            value: true,
        },
    ],
};

/// `Event_RuneLadaeTower`, EventPtrs[$2E], retail
/// `$06E930..$06EA15` (230 bytes).
pub static RUNE_LADEA_TOWER: Scene = Scene {
    name: "Event_RuneLadaeTower",
    event: EventIndex(0x002E),
    ops: &[
        SceneOp::JoinParty { slot: 4, who: RUNE },
        SceneOp::ConfigureCharacter {
            who: RUNE,
            equipment: [0x37, 0x00, 0x36, 0x38],
            restore_hp_tp: true,
        },
        SceneOp::ObjectAnimation {
            slot: 4,
            object_id: 0x10,
            art_tile: 0x0554,
            frames: 1,
        },
        SceneOp::SetStepOffset { value: 0 },
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 4,
                x: 0x3D0,
                y: 0x3B0,
            },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(6)),
            window: DialogueWindow::Standard,
        },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 3 },
        },
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::SetFlag {
            flag: Flag::event(0x62),
            value: true,
        },
    ],
};

/// `Event_PsycoWandChest`, EventPtrs[$2F], retail
/// `$06EA16..$06EC61` (588 bytes).
pub static PSYCO_WAND_CHEST: Scene = Scene {
    name: "Event_PsycoWandChest",
    event: EventIndex(0x002F),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(7)),
            window: DialogueWindow::Standard,
        },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::SetFollowMode { bits: 1 },
        // `moveq #$3B` + `dbf`: 60 update/vblank iterations.
        SceneOp::Wait { ticks: 60 },
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 4,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::Wait { ticks: 30 },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0x1FC,
            art_tile: 0x02E6,
            frames: 18,
        },
        SceneOp::Wait { ticks: 30 },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(8)),
            window: DialogueWindow::Standard,
        },
        SceneOp::MoveCamera {
            x: 0x1E0,
            y: 0x160,
            speed: 1,
        },
        SceneOp::LoadArt {
            rom_addr: 0x001D5DF0,
            tile: 0x02E6,
        },
        SceneOp::PlaySound {
            id: MUSIC_ENEMY_APPEARANCE,
        },
        SceneOp::Wait { ticks: 30 },
        SceneOp::PlaySound {
            id: MUSIC_THE_BLACK_BLOOD,
        },
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0x69),
            value: true,
        },
        SceneOp::StartBattle { index: 5 },
        SceneOp::Return { value: 1 },
    ],
};

/// `Event_ZioNurvus`, EventPtrs[$34], retail
/// `$06F2EA..$06F439` (336 bytes).
pub static ZIO_NURVUS: Scene = Scene {
    name: "Event_ZioNurvus",
    event: EventIndex(0x0034),
    ops: &[
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::Wait { ticks: 40 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x18,
                first: 0x8C89,
                second: 0x8C81,
            },
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x1E8,
            art_tile: 0x0347,
            frames: 126,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0x1EC,
            art_tile: 0x0580,
            frames: 126,
        },
        SceneOp::PlaySound {
            id: MUSIC_ENEMY_APPEARANCE,
        },
        SceneOp::Wait { ticks: 126 },
        SceneOp::Wait { ticks: 20 },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x1E8,
            art_tile: 0x0347,
            frames: 1,
        },
        SceneOp::PlaySound {
            id: MUSIC_THE_BLACK_BLOOD,
        },
        SceneOp::Wait { ticks: 1 },
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0x65),
            value: true,
        },
        SceneOp::SetSavedMusic { id: SOUND_STOP_ALL },
        SceneOp::SetMapLoadFlags {
            set: 0x88,
            clear: 0,
        },
        SceneOp::StartBattle { index: 6 },
        SceneOp::Return { value: 1 },
    ],
};
