//! Retail scenes after Zio's fall.
//!
//! The source bodies are the `grand_cross=0` branch selected by the retail
//! pointer tables.  The spaceship routine is a shared, player-facing menu;
//! its scene record below follows the Zelan selection used by the headless arc
//! and keeps the full destination table in `docs/scenes/41_InsideSpaceship.md`.
//! Renderer-only panel, palette and temporary-object writes stay typed
//! presentation records.  Persistent edges — maps, flags, party, inventory
//! and battles — remain ordinary scene operations.

use super::{CHAZ, RAJA, RIKA, RUNE, WREN};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const CHAZ_ACTOR: ActorRef = ActorRef::Character(CHAZ);
const RIKA_ACTOR: ActorRef = ActorRef::Character(RIKA);
const RUNE_ACTOR: ActorRef = ActorRef::Character(RUNE);
const WREN_ACTOR: ActorRef = ActorRef::Character(WREN);

const TREE_14: u32 = 0x001E99F0;

const MUSIC_STOP: u8 = 0xFB;
const MUSIC_TAKE_OFF_LANDALE: u8 = 0x9B;
const MUSIC_DEZOLIS_FIELD: u8 = 0x99;
const MUSIC_JIJY_NO_RAG: u8 = 0xA6;
const MUSIC_MACHINE_CENTER: u8 = 0x89;
const MUSIC_EXPLOSION: u8 = 0xAD;
const SFX_FOI: u8 = 0xBE;
const SFX_LEGEON: u8 = 0xBF;
const SFX_MEGID: u8 = 0xC0;
const SFX_TANDLE: u8 = 0xCE;
const SFX_SPARK: u8 = 0xD3;
const SFX_POWER_DOWN: u8 = 0xE4;
const SFX_ELEVATOR_OPEN: u8 = 0xE5;
const SFX_GRAVE_OPENING: u8 = 0xDD;
const SFX_DOOR_OPENED: u8 = 0xE2;
const SFX_SPACESHIP_PROPELLED: u8 = 0xE3;
const SFX_SELECTION: u8 = 0xF3;
const SFX_SPACESHIP_RADAR: u8 = 0xF8;
const SOUND_STOP_SPC: u8 = 0xFD;

/// `$800C`, `Cutscene_MeetingWren`, retail `$075FC8..$07606B`.
pub static MEETING_WREN: Scene = Scene {
    name: "Cutscene_MeetingWren",
    event: EventIndex(0x800C),
    ops: &[
        SceneOp::FaceOppositeOf {
            actor: ActorRef::Npc(0),
            of: LEADER,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x76 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(1)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::ObjectAnimation {
            slot: 4,
            object_id: 0x20,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 4,
                x: 0x1F0,
                y: 0xC0,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::JoinParty { slot: 3, who: WREN },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 7 },
        },
        SceneOp::DespawnNpc {
            npc_index: 0,
            count: 1,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x70),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$800D`, `Cutscene_InsideSpaceship`, retail `$07606C..$07607D`.
///
/// The body jumps to the shared spaceship menu.  This is the route realization
/// used by the headless arc: Mota Spaceport -> Motavia -> Zelan Space -> Zelan.
pub static INSIDE_SPACESHIP: Scene = Scene {
    name: "Cutscene_InsideSpaceship",
    event: EventIndex(0x800D),
    ops: &[
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
        // `loc_64568` uses DoMapUpdateLoop through the initial flight and
        // emits SpaceshipPropelled at loop counter `$E0`.
        SceneOp::Wait { ticks: 224 },
        SceneOp::PlaySound {
            id: SFX_SPACESHIP_PROPELLED,
        },
        SceneOp::Wait { ticks: 195 },
        SceneOp::LoadMap {
            map: 0x18C,
            prev_map: 0x00,
            start_x: 0x43,
            start_y: 0x1C,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        // `loc_64800`: `$20000` down by `$200`, including the terminal pass.
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
        SceneOp::Return { value: 0 },
    ],
};

/// `$800E`, `Cutscene_SpaceshipSabotage`, retail `$07607E..$076589`.
pub static SPACESHIP_SABOTAGE: Scene = Scene {
    name: "Cutscene_SpaceshipSabotage",
    event: EventIndex(0x800E),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D7FAE,
                destination_ram: 0xFFFF0000,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D8A0E,
                destination_ram: 0xFFFF0000,
            },
        },
        SceneOp::PlaySound {
            id: SFX_SPACESHIP_RADAR,
        },
        SceneOp::PanelCreate { id: 5 },
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 6 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: SFX_SELECTION },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::Wait { ticks: 60 },
        SceneOp::PanelDestroyAll,
        SceneOp::FadeOut,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::LoadMap {
            map: 0x18C,
            prev_map: 0x18D,
            start_x: 0x43,
            start_y: 0x3C,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::PanelCreate { id: 0x7A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x7C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(2)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::PanelDestroyAll,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PanelCreate { id: 0x7D },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PlaySound { id: 0xA9 },
        SceneOp::PanelCreate { id: 0x7E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0x71),
            value: true,
        },
        SceneOp::SetSavedMusic { id: 0xA9 },
        SceneOp::SetMapLoadFlags {
            set: 0x88,
            clear: 0,
        },
        SceneOp::StartBattle { index: 8 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$800F`, `Cutscene_CrashLaanding`, retail `$07658A..$07714F`.
pub static CRASH_LANDING: Scene = Scene {
    name: "Cutscene_CrashLaanding",
    event: EventIndex(0x800F),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x80 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: SFX_FOI },
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PlaySound { id: SFX_TANDLE },
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PlaySound { id: SFX_SPARK },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PlaySound { id: SFX_SPARK },
        SceneOp::PlaySound { id: SFX_SPARK },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::PlaySound { id: SFX_TANDLE },
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PlaySound { id: SFX_LEGEON },
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PlaySound { id: SFX_FOI },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(3)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::PanelDestroyAll,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::PanelCreate { id: 0x86 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: SFX_MEGID },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::PlaySound { id: SFX_LEGEON },
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PlaySound { id: SFX_LEGEON },
        SceneOp::WaitFrames { frames: 6 },
        SceneOp::PlaySound {
            id: MUSIC_EXPLOSION,
        },
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PlaySound { id: SFX_MEGID },
        SceneOp::WaitFrames { frames: 6 },
        SceneOp::PlaySound { id: SFX_MEGID },
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 9 },
        },
        SceneOp::PanelDestroyAll,
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::InitVramAndCram,
        SceneOp::Wait { ticks: 120 },
        SceneOp::PanelCreate { id: 0x87 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_14 },
        SceneOp::FadeIn,
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(7)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::FadeOut,
        SceneOp::LoadMap {
            map: 0x001,
            prev_map: 0x18C,
            start_x: 0x20,
            start_y: 0xBA,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::Wait { ticks: 120 },
        SceneOp::FadeIn,
        SceneOp::MoveCamera {
            x: 0x240,
            y: 0x5D0,
            speed: 2,
        },
        SceneOp::FadeOut,
        SceneOp::SetFlag {
            flag: Flag::event(0x85),
            value: true,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x08,
            clear: 0,
        },
        SceneOp::LoadMap {
            map: 0x14C,
            prev_map: 0x001,
            start_x: 6,
            start_y: 0x2E,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0,
        },
        SceneOp::Presentation {
            op: PresentationOp::SavePartySpriteX { parked_x: 0x5F0 },
        },
        SceneOp::PlaceActor {
            actor: CHAZ_ACTOR,
            x: 0x5F0,
            y: 0x120,
        },
        SceneOp::PlaceActor {
            actor: RIKA_ACTOR,
            x: 0x5F0,
            y: 0x120,
        },
        SceneOp::PlaceActor {
            actor: RUNE_ACTOR,
            x: 0x5F0,
            y: 0x120,
        },
        SceneOp::PlaceActor {
            actor: WREN_ACTOR,
            x: 0x5F0,
            y: 0x120,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::PlaySound {
            id: MUSIC_EXPLOSION,
        },
        SceneOp::Wait { ticks: 40 },
        SceneOp::PlaySound { id: SFX_POWER_DOWN },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveCamera {
            x: 0x5F0,
            y: 0x120,
            speed: 1,
        },
        SceneOp::Presentation {
            op: PresentationOp::SetObjectDestination {
                slot: 0,
                x: 0x5D0,
                y: 0x190,
            },
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::PlaySound {
            id: SFX_ELEVATOR_OPEN,
        },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0x1F4,
            art_tile: 0x2C3,
            frames: 1,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::MoveActorTo {
            actor: CHAZ_ACTOR,
            x: 0x5F0,
            y: 0x140,
            wait: true,
        },
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound {
            id: MUSIC_JIJY_NO_RAG,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_JIJY_NO_RAG,
        },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x88 },
        SceneOp::DmaPlanes,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(8)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::PlaceActor {
            actor: CHAZ_ACTOR,
            x: 0x5F0,
            y: 0x190,
        },
        SceneOp::PlaceActor {
            actor: RIKA_ACTOR,
            x: 0x610,
            y: 0x190,
        },
        SceneOp::PlaceActor {
            actor: RUNE_ACTOR,
            x: 0x600,
            y: 0x1A0,
        },
        SceneOp::PlaceActor {
            actor: WREN_ACTOR,
            x: 0x600,
            y: 0x150,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::FadeIn,
        SceneOp::Wait { ticks: 60 },
        SceneOp::SetRenderSpritesInCutscene { enabled: false },
        SceneOp::RunDialogueResume,
        SceneOp::SetFollowMode { bits: 1 },
        SceneOp::MoveActorTo {
            actor: WREN_ACTOR,
            x: 0x600,
            y: 0x180,
            wait: true,
        },
        SceneOp::Face {
            actor: CHAZ_ACTOR,
            facing: Direction::Up,
        },
        SceneOp::RunDialogueResume,
        SceneOp::Face {
            actor: CHAZ_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Face {
            actor: WREN_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::RunDialogueResume,
        SceneOp::MoveActorTo {
            actor: RIKA_ACTOR,
            x: 0x620,
            y: 0x190,
            wait: false,
        },
        SceneOp::MoveActorTo {
            actor: WREN_ACTOR,
            x: 0x610,
            y: 0x180,
            wait: false,
        },
        SceneOp::MoveActorTo {
            actor: RUNE_ACTOR,
            x: 0x610,
            y: 0x1A0,
            wait: true,
        },
        SceneOp::Face {
            actor: RIKA_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Face {
            actor: WREN_ACTOR,
            facing: Direction::Down,
        },
        SceneOp::Face {
            actor: RUNE_ACTOR,
            facing: Direction::Up,
        },
        SceneOp::MoveActorTo {
            actor: CHAZ_ACTOR,
            x: 0x5E0,
            y: 0x190,
            wait: true,
        },
        SceneOp::Face {
            actor: CHAZ_ACTOR,
            facing: Direction::Right,
        },
        SceneOp::RunDialogueResume,
        SceneOp::Wait { ticks: 60 },
        SceneOp::Face {
            actor: WREN_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Face {
            actor: RUNE_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::Face {
            actor: CHAZ_ACTOR,
            facing: Direction::Left,
        },
        SceneOp::RunDialogueResume,
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::JoinParty { slot: 4, who: RAJA },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 8 },
        },
        SceneOp::MoveActorOffset {
            actor: CHAZ_ACTOR,
            dx: 0,
            dy: 16,
            wait: true,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_14 },
        SceneOp::SetFlag {
            flag: Flag::event(0x88),
            value: true,
        },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8010`, `Cutscene_Landale`, retail `$077150..$0771D1`.
pub static LANDALE: Scene = Scene {
    name: "Cutscene_Landale",
    event: EventIndex(0x8010),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x8B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x24)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::LoadMap {
            map: 0x001,
            prev_map: 0x15F,
            start_x: 0x12,
            start_y: 0x92,
            facing: Direction::Right,
            align: 0x0C,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_DEZOLIS_FIELD,
        },
        SceneOp::FadeIn,
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D3C1E,
                destination_ram: 0xFFFF0000,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D4234,
                destination_ram: 0xFFFF0800,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::LoadSceneAsset {
                source_rom_addr: 0x001D46C6,
                destination_ram: 0xFFFF1000,
            },
        },
        SceneOp::MoveCamera {
            x: 0xC0,
            y: 0x480,
            speed: 1,
        },
        SceneOp::PlaySound {
            id: SFX_GRAVE_OPENING,
        },
        SceneOp::Wait { ticks: 369 },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0x78,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0x84,
            art_tile: 0,
            frames: 1,
        },
        SceneOp::MoveCamera {
            x: 0xC0,
            y: 0x480,
            speed: 2,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x82),
            value: true,
        },
        SceneOp::Return { value: 1 },
    ],
};

/// `$003D`, `Event_KuranArrival`, retail `$06FAD4..$06FAE5`.
pub static KURAN_ARRIVAL: Scene = Scene {
    name: "Event_KuranArrival",
    event: EventIndex(0x003D),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(4)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x86),
            value: true,
        },
    ],
};

/// `$003E`, `Event_NearDarkForce1`, retail `$06FAE6..$06FAF7`.
pub static NEAR_DARK_FORCE_1: Scene = Scene {
    name: "Event_NearDarkForce1",
    event: EventIndex(0x003E),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(5)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x87),
            value: true,
        },
    ],
};

/// `$003F`, `Event_DarkForce1`, retail `$06FAF8..$06FB1D`.
pub static DARK_FORCE_1: Scene = Scene {
    name: "Event_DarkForce1",
    event: EventIndex(0x003F),
    ops: &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(6)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x83),
            value: true,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x80,
            clear: 0,
        },
        SceneOp::StartBattle { index: 9 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$8011`, `Cutscene_DarkForce1Defeated`, retail `$0771D2..$07734B`.
pub static DARK_FORCE_1_DEFEATED: Scene = Scene {
    name: "Cutscene_DarkForce1Defeated",
    event: EventIndex(0x8011),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x8E },
        SceneOp::DmaPlanes,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(7)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::PlaySound {
            id: MUSIC_TAKE_OFF_LANDALE,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::InitVramAndCram,
        SceneOp::LoadMap {
            map: 0x18E,
            prev_map: 0xFFFF,
            start_x: 0x3E,
            start_y: 0x20,
            facing: Direction::Down,
            align: 8,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_MACHINE_CENTER,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_MACHINE_CENTER,
        },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x92 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x93 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PlaySound {
            id: SFX_SPACESHIP_RADAR,
        },
        SceneOp::PlaySound { id: SOUND_STOP_SPC },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(8)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::RemoveItem { item: 0x9A },
        SceneOp::AddItem { item: 0x97 },
        SceneOp::SetFlag {
            flag: Flag::event(0x89),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$0040`, `Event_Juza`, retail `$06FB1E..$06FB5D`.
pub static JUZA: Scene = Scene {
    name: "Event_Juza",
    event: EventIndex(0x0040),
    ops: &[
        SceneOp::Presentation {
            op: PresentationOp::FadeToRed { lines: 2 },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x48)),
            window: DialogueWindow::Standard,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x41),
            value: true,
        },
        SceneOp::StartBattle { index: 3 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0041`, `Event_JuzaDefeated`, retail `$06FB5E..$06FBED`.
pub static JUZA_DEFEATED: Scene = Scene {
    name: "Event_JuzaDefeated",
    event: EventIndex(0x0041),
    ops: &[
        SceneOp::PlaySound {
            id: SFX_DOOR_OPENED,
        },
        // Four tile groups, each with six map-update iterations in the source.
        SceneOp::ObjectAnimation {
            slot: 0,
            object_id: 0x16,
            art_tile: 0x16,
            frames: 6,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x23,
            art_tile: 0x1A,
            frames: 6,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0x16,
            art_tile: 0x16,
            frames: 6,
        },
        SceneOp::ObjectAnimation {
            slot: 3,
            object_id: 0x23,
            art_tile: 0x1A,
            frames: 6,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x48),
            value: true,
        },
    ],
};
