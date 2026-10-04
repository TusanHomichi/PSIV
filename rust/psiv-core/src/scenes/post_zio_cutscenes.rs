//! Retail scenes after Zio's fall.
//!
//! The source bodies are the `grand_cross=0` branch selected by the retail
//! pointer tables.  The spaceship routine is a shared, player-facing menu:
//! `scenes/flight.rs` carries its tables and the flight as scene ops, and the
//! runtime's destination session mode owns the window
//! (`docs/scenes/41_InsideSpaceship.md`).
//! Renderer-only panel, palette and temporary-object writes stay typed
//! presentation records.  Persistent edges — maps, flags, party, inventory
//! and battles — remain ordinary scene operations.

use super::{CHAZ, INSIDE_SPACESHIP_ROUTE, RAJA, RIKA, RUNE, WREN, retained};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::scenes::{DestinationMask, FlightLeg};
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
/// The body jumps to the shared spaceship menu, `loc_63BC4`; the ops are
/// [`INSIDE_SPACESHIP_ROUTE`], whose skips are relative so the two scenes that
/// inline the same `jmp` share them.
pub static INSIDE_SPACESHIP: Scene = Scene {
    name: "Cutscene_InsideSpaceship",
    event: EventIndex(0x800D),
    ops: &INSIDE_SPACESHIP_ROUTE,
};

/// Ops between the sabotage menu and its cancel leg: everything after the menu
/// up to and including the final `Return { value: 1 }`.
const SABOTAGE_CANCEL_SKIP: u16 = 28;

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
        // The copy of the menu with the one-row mask `$08`, Kuran
        // (`loc_76586`, `ps4.asm:155741`). Cancel (`:155546`, `loc_764F8`)
        // lands on the cancel leg at the end of the scene.
        SceneOp::DestinationMenu {
            mask: DestinationMask::Fixed(0x08),
            cancel_skip: SABOTAGE_CANCEL_SKIP,
        },
        SceneOp::PanelDestroyAll,
        SceneOp::FadeOut,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::PlaySound { id: MUSIC_STOP },
        SceneOp::LoadMap {
            map: 0x18C,
            prev_map: 0xFFFF,
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
        SceneOp::LoadFlightMap {
            leg: FlightLeg::Return,
        },
        SceneOp::Return { value: 0 },
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
        // `move.b #1, (World_Index).w` right after the RefreshMap
        // (`ps4.asm:155847`): the party is on Dezolis from here on.
        SceneOp::SetWorldIndex { world: 1 },
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
        // $076820..$07684A: the first Raja Temple load clears bit 3;
        // RajaTemple $85 is still clear, so its roof has not opened yet.
        SceneOp::LoadMap {
            map: 0x14C,
            prev_map: 0x001,
            start_x: 6,
            start_y: 0x2E,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
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
        // $076AE6 -> $076E00: Raja Temple BG (47,9), (48,9),
        // (47,10), (48,10). $40(a1) advances one 64-chunk row, not X.
        // $076E58..$076E6B: five frames; $076E16/$076E50 runs six
        // RunMapUpdates + DMAPlane_B_VInt passes after every frame.
        SceneOp::WriteMapChunks {
            chunks: &[(47, 9, 0x50), (48, 9, 0x51), (47, 10, 0x58), (48, 10, 0x59)],
        },
        SceneOp::Wait { ticks: 6 },
        SceneOp::WriteMapChunks {
            chunks: &[(47, 9, 0x52), (48, 9, 0x53), (47, 10, 0x58), (48, 10, 0x59)],
        },
        SceneOp::Wait { ticks: 6 },
        SceneOp::WriteMapChunks {
            chunks: &[(47, 9, 0x54), (48, 9, 0x55), (47, 10, 0x5A), (48, 10, 0x5B)],
        },
        SceneOp::Wait { ticks: 6 },
        SceneOp::WriteMapChunks {
            chunks: &[(47, 9, 0x56), (48, 9, 0x57), (47, 10, 0x5C), (48, 10, 0x5D)],
        },
        SceneOp::Wait { ticks: 6 },
        SceneOp::WriteMapChunks {
            chunks: &[(47, 9, 0x56), (48, 9, 0x57), (47, 10, 0x5E), (48, 10, 0x5F)],
        },
        SceneOp::Wait { ticks: 6 },
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
        // $076BEA..$076C06: only after the live write, set $85 and
        // RefreshMap with bit 3 set. MapDataMan_RajaTemple ($052AAE)
        // reapplies the last frame on this and later loads.
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
        retained(6),
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
        // `move.b #3, (World_Index).w` (`ps4.asm:156396`)
        SceneOp::SetWorldIndex { world: 3 },
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
        retained(0x48),
        SceneOp::SetFlag {
            flag: Flag::event(0x41),
            value: true,
        },
        SceneOp::StartBattle { index: 3 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0041`, `Event_JuzaDefeated`, retail `$06FB5E..$06FBED`.
///
/// The stairs at chunks `(12,9)`, `(13,9)`, `(12,10)`, `(13,10)` — cells
/// `(24..27,18..21)` — are *rewritten live*: `GetMapLayoutOffset(12,9,1)`
/// resolves the BG layout address (`$06FB74`) and the table at `$06FBC4`
/// alternates the closed chunk `$16` with the open set `$23,$1A,$19,$1B`
/// eight times, each group followed by `RefreshPlaneBG` and a seven-frame
/// `DMAPlane_B_VInt` loop. Collision reads the layout the plane holds, so the
/// stairway's map-change tiles are live the moment the event ends; the map's
/// own effect entry `$38` writes the closed chunk back only while Juza
/// Defeated `$48` is clear.
pub static JUZA_DEFEATED: Scene = Scene {
    name: "Event_JuzaDefeated",
    event: EventIndex(0x0041),
    ops: &[
        SceneOp::PlaySound {
            id: SFX_DOOR_OPENED,
        },
        // `$06FBC4`: eight five-byte rows — delay, then four chunk ids —
        // written to the same four chunks, alternating the closed chunk `$16`
        // with `$23,$1A,$19,$1B`, each row followed by a plane refresh and a
        // seven-frame `DMAPlane_B_VInt`. The row the table ends on is the map's
        // *own* layout at those chunks, so the live state the cartridge leaves
        // is exactly "these chunks are their baked selves again" — which is
        // what `RestoreMapChunks` asserts and applies. The alternating frames
        // change nothing the party can reach mid-scene.
        SceneOp::RestoreMapChunks {
            chunks: &[(12, 9, 0x23), (13, 9, 0x1A), (12, 10, 0x19), (13, 10, 0x1B)],
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x48),
            value: true,
        },
    ],
};

#[cfg(test)]
mod world_index_tests {
    use super::*;

    /// `Cutscene_CrashLaanding` writes `World_Index = 1` immediately after the
    /// RefreshMap that loads Dezolis (`ps4.asm:155847`), and nowhere else.
    #[test]
    fn the_crash_landing_sets_dezolis_right_after_loading_it() {
        let ops = CRASH_LANDING.ops;
        let writes: Vec<usize> = ops
            .iter()
            .enumerate()
            .filter(|(_, op)| matches!(op, SceneOp::SetWorldIndex { .. }))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(writes.len(), 1, "exactly one World_Index write");
        let at = writes[0];
        assert!(matches!(ops[at], SceneOp::SetWorldIndex { world: 1 }));
        assert!(
            matches!(ops[at - 1], SceneOp::LoadMap { map: 0x001, .. }),
            "the write follows the Dezolis load"
        );
    }
}
