//! Retail Dezo campaign scenes from the Le Roof gate through Gumbious.
//!
//! The pointer ranges for these records are in `docs/scenes/51_*` onward.
//! Renderer-only palette/panel/object choreography stays in typed presentation
//! ops; party, map, inventory, flag and battle writes stay explicit here.

use super::KYRA;
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const KYRA_ACTOR: ActorRef = ActorRef::Character(KYRA);

const TREE_37: u32 = 0x001F_9580;

const MUSIC_DEZOLIS_FIELD: u8 = 0x99;
const MUSIC_TOWER: u8 = 0x9A;
const MUSIC_LAND_MASTER: u8 = 0x8D;
const MUSIC_TAKE_OFF_LANDALE: u8 = 0x9B;
const MUSIC_FAL: u8 = 0x92;
const MUSIC_RED_ALERT: u8 = 0xA9;
const SOUND_STOP_SPC: u8 = 0xFD;
const SOUND_STOP_ALL: u8 = 0xFE;
const SFX_DEBAN: u8 = 0xDC;
const SFX_STAIRS: u8 = 0xDF;
const SFX_SPACESHIP_PROPELLED: u8 = 0xE3;

const fn entry(id: u16) -> DialogueSource {
    DialogueSource::Entry(DialogueId(id))
}

const fn standard(id: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: entry(id),
        window: DialogueWindow::Standard,
    }
}

const fn cutscene(id: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: entry(id),
        window: DialogueWindow::Cutscene,
    }
}

/// The shared retail `$800D` route, inlined by the two source bodies that end
/// in `jmp (Cutscene_InsideSpaceship).l`.
pub(super) const INSIDE_SPACESHIP_ROUTE: [SceneOp; 11] = [
    SceneOp::InitVramAndCram,
    SceneOp::FadeIn,
    SceneOp::PlaySound {
        id: MUSIC_TAKE_OFF_LANDALE,
    },
    SceneOp::SetSavedMusic {
        id: MUSIC_TAKE_OFF_LANDALE,
    },
    SceneOp::LoadMap {
        map: 0x00,
        prev_map: 0x0BF,
        start_x: 0x68,
        start_y: 0xB4,
        facing: Direction::Down,
        align: 0,
        clear_load_flags: 0x08,
    },
    SceneOp::Wait { ticks: 224 },
    SceneOp::PlaySound {
        id: SFX_SPACESHIP_PROPELLED,
    },
    SceneOp::Wait { ticks: 195 },
    SceneOp::LoadMap {
        map: 0x18C,
        prev_map: 0x000,
        start_x: 0x43,
        start_y: 0x1C,
        facing: Direction::Down,
        align: 0,
        clear_load_flags: 0x08,
    },
    SceneOp::Wait { ticks: 257 },
    SceneOp::LoadMap {
        map: 0x18D,
        prev_map: 0x18C,
        start_x: 0x3E,
        start_y: 0x5A,
        facing: Direction::Down,
        align: 0,
        clear_load_flags: 0x08,
    },
];

/// `$0048`, `Event_MeetingLeRoof`, `$070482..$07069D`.
pub static MEETING_LE_ROOF: Scene = Scene {
    name: "Event_MeetingLeRoof",
    event: EventIndex(0x0048),
    ops: &[
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1F0,
            y: 0x1E0,
            wait: true,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 2 },
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::LoadArt {
            rom_addr: 0x001D_B2C8,
            tile: 0x260,
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D_DE0A,
                destination_ram: 0xFFFF_0000,
            },
        },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: 0xA1 },
        SceneOp::Presentation {
            op: PresentationOp::FadeFromRed { lines: 2 },
        },
        SceneOp::Wait { ticks: 180 },
        SceneOp::Wait { ticks: 120 },
        standard(0),
        SceneOp::Wait { ticks: 120 },
        SceneOp::PlaySound { id: 0xFB },
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::PlaySound { id: MUSIC_TOWER },
        SceneOp::SetFlag {
            flag: Flag::event(0xD1),
            value: true,
        },
    ],
};

/// `$801C`, `Cutscene_LeRoofAgain`, `$078346..$0784B5`.
pub static LE_ROOF_AGAIN: Scene = Scene {
    name: "Cutscene_LeRoofAgain",
    event: EventIndex(0x801C),
    ops: &[
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1F0,
            y: 0x1F0,
            wait: true,
        },
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 2 },
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::LoadArt {
            rom_addr: 0x001D_B2C8,
            tile: 0x260,
        },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: 0xA1 },
        SceneOp::Presentation {
            op: PresentationOp::FadeFromRed { lines: 2 },
        },
        SceneOp::Wait { ticks: 180 },
        SceneOp::Wait { ticks: 120 },
        standard(0x0B),
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x11B },
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResume,
        SceneOp::FadeOut,
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x125 },
        SceneOp::DmaPlanes,
        cutscene(0x0C),
        SceneOp::SetFlag {
            flag: Flag::event(0xD6),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xD7),
            value: true,
        },
        INSIDE_SPACESHIP_ROUTE[0],
        INSIDE_SPACESHIP_ROUTE[1],
        INSIDE_SPACESHIP_ROUTE[2],
        INSIDE_SPACESHIP_ROUTE[3],
        INSIDE_SPACESHIP_ROUTE[4],
        INSIDE_SPACESHIP_ROUTE[5],
        INSIDE_SPACESHIP_ROUTE[6],
        INSIDE_SPACESHIP_ROUTE[7],
        INSIDE_SPACESHIP_ROUTE[8],
        INSIDE_SPACESHIP_ROUTE[9],
        INSIDE_SPACESHIP_ROUTE[10],
        SceneOp::Return { value: 0 },
    ],
};

/// `$004C`, `Event_CarnivorousTrees`, `$070774..$070855`.
pub static CARNIVOROUS_TREES: Scene = Scene {
    name: "Event_CarnivorousTrees",
    event: EventIndex(0x004C),
    ops: &[
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::BranchIfVehicle {
            if_mounted: 2,
            if_on_foot: 7,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Wait { ticks: 1 },
        SceneOp::PlaySound {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::Jump { to: 8 },
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x34),
        SceneOp::MoveActorOffset {
            actor: LEADER,
            dx: 0,
            dy: 0x20,
            wait: true,
        },
        SceneOp::StartBattle { index: 0x0A },
        SceneOp::Return { value: 1 },
    ],
};

/// `$004D`, `Event_SavingKyra`, `$070856..$070975`.
pub static SAVING_KYRA: Scene = Scene {
    name: "Event_SavingKyra",
    event: EventIndex(0x004D),
    ops: &[
        SceneOp::BranchIfVehicle {
            if_mounted: 2,
            if_on_foot: 8,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Wait { ticks: 1 },
        SceneOp::PlaySound { id: 0xFB },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x34),
        SceneOp::PlaySound {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::Wait { ticks: 1 },
        SceneOp::PanelCreate { id: 0x98 },
        SceneOp::DmaPlanes,
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x26),
        SceneOp::MoveActorOffset {
            actor: LEADER,
            dx: 0,
            dy: 0x20,
            wait: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x95),
            value: true,
        },
        SceneOp::StartBattle { index: 0x0A },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8013`, `Cutscene_MeetingKyra`, `$077788..$077895`.
pub static MEETING_KYRA: Scene = Scene {
    name: "Cutscene_MeetingKyra",
    event: EventIndex(0x8013),
    ops: &[
        SceneOp::PanelCreate { id: 0x140 },
        SceneOp::DmaPlanes,
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        cutscene(0x27),
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound { id: MUSIC_FAL },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x99 },
        SceneOp::DmaPlanes,
        cutscene(0x28),
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x9B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::RunDialogueResume,
        SceneOp::JoinParty { slot: 4, who: KYRA },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 9 },
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xA0),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x001,
            prev_map: 0xFFFF,
            start_x: 0x174,
            start_y: 0x1C,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_LAND_MASTER,
        },
        SceneOp::SetVehicleIndex { index: 2 },
        SceneOp::SetMapLoadFlags { set: 1, clear: 0 },
        SceneOp::Return { value: 0 },
    ],
};

/// `$0047`, `Event_EclipseTorchUsed`, `$07018A..$070481`.
pub static ECLIPSE_TORCH_USED: Scene = Scene {
    name: "Event_EclipseTorchUsed",
    event: EventIndex(0x0047),
    ops: &[
        SceneOp::BranchIfVehicle {
            if_mounted: 1,
            if_on_foot: 7,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::Wait { ticks: 1 },
        SceneOp::Jump { to: 7 },
        SceneOp::PanelCreate { id: 0x10D },
        SceneOp::DmaPlanes,
        SceneOp::Wait { ticks: 120 },
        SceneOp::PlaySound { id: SFX_DEBAN },
        SceneOp::Wait { ticks: 40 },
        SceneOp::PanelDestroy { id: 0x10D },
        SceneOp::DmaPlanes,
        SceneOp::LoadArt {
            rom_addr: 0x001D_712C,
            tile: 0x3AF,
        },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 3,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 4,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 5,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 6,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 7,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 8,
            object_id: 0x338,
            art_tile: 0x3AF,
            frames: 1,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        standard(0x2A),
        SceneOp::SetFlag {
            flag: Flag::event(0x9C),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$004E`, `Event_DarkForce2`, `$070976..$0709A1`.
pub static DARK_FORCE_2: Scene = Scene {
    name: "Event_DarkForce2",
    event: EventIndex(0x004E),
    ops: &[
        standard(0x3A),
        SceneOp::SetFlag {
            flag: Flag::event(0x9E),
            value: true,
        },
        SceneOp::SetSavedMusic { id: SOUND_STOP_ALL },
        SceneOp::SetMapLoadFlags {
            set: 0x80,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x11 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8014`, `Cutscene_LutzRevelation`, `$077896..$077A2D`.
pub static LUTZ_REVELATION: Scene = Scene {
    name: "Cutscene_LutzRevelation",
    event: EventIndex(0x8014),
    ops: &[
        standard(0x2A),
        SceneOp::Wait { ticks: 30 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x200,
            y: 0x1E0,
            wait: true,
        },
        SceneOp::PlaySound { id: SFX_STAIRS },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0x16E,
            start_x: 0x3C,
            start_y: 0x4C,
            facing: Direction::Left,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x1F0,
            wait: true,
        },
        SceneOp::MoveActorOffset {
            actor: KYRA_ACTOR,
            dx: 0x10,
            dy: 0,
            wait: true,
        },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: KYRA_ACTOR,
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveActorTo {
            actor: KYRA_ACTOR,
            x: 0x1F0,
            y: 0x1F0,
            wait: true,
        },
        standard(0x2F),
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x9E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 100 },
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0x97),
            value: true,
        },
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0x16E,
            start_x: 0x3C,
            start_y: 0x48,
            facing: Direction::Down,
            align: 8,
            clear_load_flags: 0x08,
        },
        SceneOp::Return { value: 0 },
    ],
};

#[path = "dezo_campaign_late.rs"]
mod dezo_campaign_late;
pub use dezo_campaign_late::*;
