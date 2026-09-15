//! Retail Dezo campaign scenes from Dark Force 2's defeat through Gumbious.

use super::super::{KYRA, SETH};
use super::INSIDE_SPACESHIP_ROUTE;
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

const TREE_22: u32 = 0x001E_F130;
const TREE_38: u32 = 0x001F_A480;
const TREE_39: u32 = 0x001F_AAC0;
const TREE_42: u32 = 0x001F_C920;

const MUSIC_DEZOLIS_FIELD: u8 = 0x99;
const MUSIC_DEZOLIS_FIELD_2: u8 = 0x9D;
const MUSIC_ENEMY_APPEARANCE: u8 = 0xA3;
const MUSIC_EXPLOSION: u8 = 0xAD;
const MUSIC_MACHINE_CENTER: u8 = 0x89;
const MUSIC_RED_ALERT: u8 = 0xA9;
const MUSIC_TAKE_OFF_LANDALE: u8 = 0x9B;
const MUSIC_TEMPLE_NGANGBIUS: u8 = 0x93;
const SOUND_STOP_SPC: u8 = 0xFD;
const SFX_BARRIER_BROKEN: u8 = 0xE6;
const SFX_GRAVE_OPENING: u8 = 0xDD;

const ITEM_ECLIPSE_TORCH: u8 = 0x8E;
const ITEM_HYDROFOIL: u8 = 0x98;

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
/// `$8017`, `Cutscene_DarkForce2Defeated`, `$077BDA..$077DC5`.
pub static DARK_FORCE_2_DEFEATED: Scene = Scene {
    name: "Cutscene_DarkForce2Defeated",
    event: EventIndex(0x8017),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x10E },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound {
            id: MUSIC_EXPLOSION,
        },
        SceneOp::WaitFrames { frames: 1 },
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::PanelCreate { id: 0x10F },
        SceneOp::DmaPlanes,
        cutscene(0x3B),
        SceneOp::PanelCreate { id: 0x110 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound {
            id: MUSIC_DEZOLIS_FIELD_2,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResume,
        SceneOp::PanelDestroyAll,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0xA1),
            value: true,
        },
        SceneOp::RemovePartyMember { who: KYRA },
        SceneOp::ClearCharacterStatus { who: KYRA },
        SceneOp::PlaySound { id: 0xFB },
        SceneOp::Wait { ticks: 20 },
        SceneOp::PlaySound {
            id: MUSIC_DEZOLIS_FIELD_2,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD_2,
        },
        SceneOp::LoadMap {
            map: 0x001,
            prev_map: 0xFFFF,
            start_x: 0x174,
            start_y: 0x0E,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$8019`, `Cutscene_MeetingSeth`, `$077EAC..$077F2D`.
pub static MEETING_SETH: Scene = Scene {
    name: "Cutscene_MeetingSeth",
    event: EventIndex(0x8019),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x118 },
        SceneOp::DmaPlanes,
        SceneOp::SetDialogueTree { rom_addr: TREE_42 },
        cutscene(0),
        SceneOp::JoinParty { slot: 4, who: SETH },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 0x0A },
        },
        SceneOp::LoadMap {
            map: 0x000,
            prev_map: 0xFFFF,
            start_x: 0xEE,
            start_y: 0x136,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xC1),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$801A`, `Cutscene_AeroPrism`, `$077F2E..$07818D`.
pub static AERO_PRISM: Scene = Scene {
    name: "Cutscene_AeroPrism",
    event: EventIndex(0x801A),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x0A0 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        standard(1),
        SceneOp::Wait { ticks: 30 },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Wait { ticks: 1 },
        SceneOp::RunDialogueResume,
        SceneOp::PanelCreate { id: 0x0A4 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResume,
        SceneOp::PanelCreate { id: 0x0A5 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: 0xEC },
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResume,
        SceneOp::RemovePartyMember { who: SETH },
        SceneOp::ClearCharacterStatus { who: SETH },
        SceneOp::SetFlag {
            flag: Flag::event(0xC5),
            value: true,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x12 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0050`, `Event_DarkForce3Defeated`, `$070A2A..$070A4D`.
pub static DARK_FORCE_3_DEFEATED: Scene = Scene {
    name: "Event_DarkForce3Defeated",
    event: EventIndex(0x0050),
    ops: &[
        standard(2),
        SceneOp::SetFlag {
            flag: Flag::event(0xC6),
            value: true,
        },
    ],
};

/// `$0053`, `Event_ReshelBattle`, `$070A9E..$070ABB`.
pub static RESHEL_BATTLE: Scene = Scene {
    name: "Event_ReshelBattle",
    event: EventIndex(0x0053),
    ops: &[
        SceneOp::SetFlag {
            flag: Flag::event(0x8B),
            value: true,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x0D },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0054`, `Event_ClmCenterForcedBattle`, `$070ABC..$070AD9`.
pub static CLM_CENTER_FORCED_BATTLE: Scene = Scene {
    name: "Event_ClmCenterForcedBattle",
    event: EventIndex(0x0054),
    ops: &[
        SceneOp::SetFlag {
            flag: Flag::event(0x92),
            value: true,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x0B },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0055`, `Event_ClmCenterAfterBattle`, `$070ADA..$070AEB`.
pub static CLM_CENTER_AFTER_BATTLE: Scene = Scene {
    name: "Event_ClmCenterAfterBattle",
    event: EventIndex(0x0055),
    ops: &[
        standard(0x31),
        SceneOp::SetFlag {
            flag: Flag::event(0xA4),
            value: true,
        },
    ],
};

/// `$0056`, `Event_DElmLars`, `$070AEC..$070B0B`.
pub static D_ELM_LARS: Scene = Scene {
    name: "Event_DElmLars",
    event: EventIndex(0x0056),
    ops: &[
        standard(0x33),
        SceneOp::SetFlag {
            flag: Flag::event(0x93),
            value: true,
        },
        SceneOp::StartBattle { index: 0x0C },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0057`, `Event_AfterDElmLarsBattle`, `$070B0C..$070B1D`.
pub static AFTER_D_ELM_LARS_BATTLE: Scene = Scene {
    name: "Event_AfterDElmLarsBattle",
    event: EventIndex(0x0057),
    ops: &[
        standard(0x34),
        SceneOp::SetFlag {
            flag: Flag::event(0xA5),
            value: true,
        },
    ],
};

/// `$8015`, `Cutscene_FindingAirCastle`, `$077A2E..$077A67`.
pub static FINDING_AIR_CASTLE: Scene = Scene {
    name: "Cutscene_FindingAirCastle",
    event: EventIndex(0x8015),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x18A },
        SceneOp::DmaPlanes,
        cutscene(0x18),
        SceneOp::SetFlag {
            flag: Flag::event(0x99),
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

/// `$0058`, `Event_AirCastleArrival`, `$070B1E..$070B2F`.
pub static AIR_CASTLE_ARRIVAL: Scene = Scene {
    name: "Event_AirCastleArrival",
    event: EventIndex(0x0058),
    ops: &[
        standard(0x35),
        SceneOp::SetFlag {
            flag: Flag::event(0x9F),
            value: true,
        },
    ],
};

/// `$0059`, `Event_XeAThoulBeforeBattle`, `$070B30..$070B55`.
pub static XE_ATHOUL_BEFORE_BATTLE: Scene = Scene {
    name: "Event_XeAThoulBeforeBattle",
    event: EventIndex(0x0059),
    ops: &[
        standard(0x36),
        SceneOp::SetFlag {
            flag: Flag::event(0x9A),
            value: true,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x0E },
        SceneOp::Return { value: 1 },
    ],
};

/// `$005A`, `Event_AirCastleFakeChest`, `$070B56..$070C2F`.
pub static AIR_CASTLE_FAKE_CHEST: Scene = Scene {
    name: "Event_AirCastleFakeChest",
    event: EventIndex(0x005A),
    ops: &[
        standard(0x3C),
        SceneOp::SetFlag {
            flag: Flag::event(0xA6),
            value: true,
        },
        SceneOp::RemoveItem {
            item: ITEM_ECLIPSE_TORCH,
        },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 3,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 4,
            object_id: 0,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x0F },
        SceneOp::Return { value: 1 },
    ],
};

/// `$005D`, `Event_LashiecAppearance`, `$070CB4..$070E4D`.
pub static LASHIEC_APPEARANCE: Scene = Scene {
    name: "Event_LashiecAppearance",
    event: EventIndex(0x005D),
    ops: &[
        SceneOp::LoadArt {
            rom_addr: 0x001D_7710,
            tile: 0x371,
        },
        SceneOp::MoveCamera {
            x: 0x1E0,
            y: 0x110,
            speed: 1,
        },
        SceneOp::PlaySound {
            id: MUSIC_ENEMY_APPEARANCE,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x33C,
            art_tile: 0x371,
            frames: 20,
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::PlaySound {
            id: MUSIC_RED_ALERT,
        },
        standard(0x37),
        SceneOp::SetFlag {
            flag: Flag::event(0x9B),
            value: true,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x88,
            clear: 0,
        },
        SceneOp::StartBattle { index: 0x10 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8016`, `Cutscene_LashiecDefeated`, `$077A68..$077BD9`.
pub static LASHIEC_DEFEATED: Scene = Scene {
    name: "Cutscene_LashiecDefeated",
    event: EventIndex(0x8016),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x107 },
        SceneOp::DmaPlanes,
        cutscene(0x38),
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelDestroyAll,
        SceneOp::DmaPlanes,
        SceneOp::FadeOut,
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound {
            id: MUSIC_EXPLOSION,
        },
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelCreate { id: 0x10A },
        SceneOp::DmaPlanes,
        SceneOp::FadeIn,
        SceneOp::WaitFrames { frames: 240 },
        SceneOp::PlaySound {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::RunDialogueResume,
        SceneOp::AddItem {
            item: ITEM_ECLIPSE_TORCH,
        },
        SceneOp::InitVramAndCram,
        SceneOp::LoadMap {
            map: 0x162,
            prev_map: 0xFFFF,
            start_x: 0x4E,
            start_y: 0x2C,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_TEMPLE_NGANGBIUS,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_TEMPLE_NGANGBIUS,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_22 },
        standard(0x39),
        SceneOp::SetDialogueTree { rom_addr: TREE_38 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8018`, `Cutscene_GumbiousBishop`, `$077DC6..$077EAB`.
pub static GUMBIOUS_BISHOP: Scene = Scene {
    name: "Cutscene_GumbiousBishop",
    event: EventIndex(0x8018),
    ops: &[
        standard(0x33),
        SceneOp::FadeOut,
        SceneOp::Wait { ticks: 60 },
        SceneOp::PlaySound {
            id: MUSIC_TAKE_OFF_LANDALE,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::InitVramAndCram,
        SceneOp::LoadMap {
            map: 0x0BF,
            prev_map: 0xFFFF,
            start_x: 0x3C,
            start_y: 0x26,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_MACHINE_CENTER,
        },
        SceneOp::FadeIn,
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x1E0,
            y: 0x170,
            wait: true,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_39 },
        standard(0x17),
        SceneOp::RemoveItem {
            item: ITEM_ECLIPSE_TORCH,
        },
        SceneOp::AddItem {
            item: ITEM_HYDROFOIL,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x9D),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xC0),
            value: true,
        },
        SceneOp::Return { value: 1 },
    ],
};
