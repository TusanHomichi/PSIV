//! Retail-only surfaces that sit outside the fixed `$33..$54` event wave.
//!
//! The three routines here are still deterministic scene-shaped programs:
//! Raja Sick (`$8012`), the Rykros arrival/tower hand-off (`$801B`), and the
//! final presentation (`$8021`). The guild and fifth-character routines stay
//! in `88_RetailBoundaries.md`; their joypad-driven state machines are not
//! honest `SceneOp` slices.

use crate::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{CreditsPlane, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

use super::{CHAZ, RAJA, RIKA, RUNE, WREN};

const TREE39: u32 = 0x001F_AAC0;
const TREE42: u32 = 0x001F_C920;

const MUSIC_EXPLOSION: u8 = 0xAD;
const MUSIC_STAFF_ROLL: u8 = 0xAE;
const MUSIC_PROMISING_1: u8 = 0xAF;
const MUSIC_PROMISING_2: u8 = 0xB2;
const SOUND_STOP_ALL: u8 = 0xFE;
const SFX_BARRIER_BROKEN: u8 = 0xE6;
const SFX_STAIRS: u8 = 0xDF;
const SFX_RED_ALERT: u8 = 0xA9;

const RYKROS_PALETTE_DELAYS: &[u8] = &[7, 5, 7, 7, 7, 7];
const RAJA_SICK_COMPANIONS: &[crate::CharId] = &[CHAZ, RUNE, RIKA, WREN];

/// `$8012`, `Cutscene_RajaSick`, `$07734C..$077787`.
pub static RAJA_SICK: Scene = Scene {
    name: "Cutscene_RajaSick",
    event: EventIndex(0x8012),
    ops: &[
        SceneOp::FaceOppositeOf {
            actor: ActorRef::PartyMember(0),
            of: ActorRef::PartyMember(0),
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x59)),
            window: DialogueWindow::Standard,
        },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Down,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Left,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Down,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Right,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Up,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::Face {
            actor: ActorRef::Character(RAJA),
            facing: Direction::Left,
        },
        SceneOp::Wait { ticks: 10 },
        SceneOp::LoadArt {
            rom_addr: 0x001D_78CA,
            tile: 0x37F,
        },
        SceneOp::Presentation {
            op: PresentationOp::RajaSickTemporaryObject {
                ram_addr: 0xFFFF_C4C0,
                object_id: 0x374,
                art_tile: 0x37F,
                frames: 20,
            },
        },
        SceneOp::PlaySound { id: 0xE1 },
        SceneOp::Wait { ticks: 20 },
        SceneOp::PlaySound { id: 0x9F },
        SceneOp::Wait { ticks: 20 },
        SceneOp::Presentation {
            op: PresentationOp::RajaSickArrangeParty {
                reference: RAJA,
                companions: RAJA_SICK_COMPANIONS,
            },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x62)),
            window: DialogueWindow::Standard,
        },
        SceneOp::FadeOut,
        SceneOp::SetFlag {
            flag: Flag::event(0x94),
            value: true,
        },
        SceneOp::RemovePartyMember { who: RAJA },
        SceneOp::Presentation {
            op: PresentationOp::RajaSickResetRaja {
                stats_ram: 0xFFFF_F900,
                character: RAJA,
                art_offset: 0x16,
            },
        },
        SceneOp::LoadMap {
            map: 0x134,
            prev_map: 0x133,
            start_x: 0x34,
            start_y: 0x38,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaceActor {
            actor: ActorRef::PartyMember(0),
            x: 0x1A0,
            y: 0x1C0,
        },
        SceneOp::PlaceActor {
            actor: ActorRef::PartyMember(1),
            x: 0x1B0,
            y: 0x1C0,
        },
        SceneOp::PlaceActor {
            actor: ActorRef::PartyMember(2),
            x: 0x1B0,
            y: 0x1D0,
        },
        SceneOp::PlaceActor {
            actor: ActorRef::PartyMember(3),
            x: 0x1A0,
            y: 0x1E0,
        },
        SceneOp::Wait { ticks: 1 },
        SceneOp::FadeIn,
        SceneOp::RunDialogueResume,
        SceneOp::MoveCamera {
            x: 0x1A0,
            y: 0x240,
            speed: 4,
        },
        SceneOp::PlaySound { id: SFX_STAIRS },
        SceneOp::Wait { ticks: 20 },
        SceneOp::ObjectAnimation {
            slot: 20,
            object_id: 0x8194,
            art_tile: 0,
            frames: 48,
        },
        SceneOp::Wait { ticks: 48 },
        SceneOp::PlaySound { id: SFX_RED_ALERT },
        SceneOp::Wait { ticks: 10 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x63)),
            window: DialogueWindow::Standard,
        },
        SceneOp::Face {
            actor: ActorRef::Character(CHAZ),
            facing: Direction::Down,
        },
        SceneOp::Face {
            actor: ActorRef::Character(RUNE),
            facing: Direction::Down,
        },
        SceneOp::Face {
            actor: ActorRef::Character(RIKA),
            facing: Direction::Down,
        },
        SceneOp::Face {
            actor: ActorRef::Character(WREN),
            facing: Direction::Down,
        },
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: ActorRef::PartyMember(0),
                speed: 4,
            },
        },
        SceneOp::RunDialogueResume,
        SceneOp::SetSavedMusic { id: SFX_RED_ALERT },
        SceneOp::Return { value: 1 },
    ],
};

/// `$801B`, `Cutscene_Rykros`, `$07818E..$078345`.
pub static RYKROS: Scene = Scene {
    name: "Cutscene_Rykros",
    event: EventIndex(0x801B),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x18B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x18F },
        SceneOp::DmaPlanes,
        SceneOp::SetDialogueTree { rom_addr: TREE39 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x16)),
            window: DialogueWindow::Cutscene3,
        },
        SceneOp::PanelCreate { id: 0x190 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB00,
                destination_ram: 0xFFFF_FB80,
                words: 64,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone { frames: 30 },
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::RunDialogueResume,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResume,
        SceneOp::Presentation {
            op: PresentationOp::RykrosPaletteCycle {
                source_rom_addr: 0x0007_82DA,
                destination_ram: 0xFFFF_FB2E,
                words_per_frame: 9,
                delays: RYKROS_PALETTE_DELAYS,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Cutscene5,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0xD0),
            value: true,
        },
        SceneOp::Return { value: 0 },
    ],
};

/// `$8021`, `Cutscene_Ending`, `$078F3E..$07A811`.
pub static ENDING: Scene = Scene {
    name: "Cutscene_Ending",
    event: EventIndex(0x8021),
    ops: &[
        SceneOp::SetDialogueTree { rom_addr: TREE42 },
        SceneOp::PlaySound {
            id: MUSIC_EXPLOSION,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(9)),
            window: DialogueWindow::EndingIntro,
        },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::WindowDestroy { render_mode: 2 },
        },
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound {
            id: MUSIC_PROMISING_1,
        },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x147 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 240 },
        SceneOp::PanelCreate { id: 0x148 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 130 },
        SceneOp::PanelCreate { id: 0x149 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 50 },
        SceneOp::PanelCreate { id: 0x14A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 50 },
        SceneOp::PanelCreate { id: 0x14B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0A)),
            window: DialogueWindow::Ending,
        },
        SceneOp::InitVramAndCram,
        SceneOp::PanelDestroyAll,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x14C },
        SceneOp::DmaPlanes,
        SceneOp::FadeIn,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x14D },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x14E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x14F },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0B)),
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x150 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::PanelCreate { id: 0x187 },
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PanelCreate { id: 0x151 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 240 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::PanelCreate { id: 0x182 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PanelCreate { id: 0x152 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x153 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x154 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 200 },
        SceneOp::PanelCreate { id: 0x155 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelCreate { id: 0x156 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 240 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PlaySound {
            id: MUSIC_PROMISING_2,
        },
        SceneOp::PanelCreate { id: 0x157 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x158 },
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x159 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x15A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x188 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x15D },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x15F },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x160 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x161 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x162 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x163 },
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 160 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x164 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x189 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x165 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x166 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x167 },
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x168 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x169 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB00,
                destination_ram: 0xFFFF_FB80,
                words: 64,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0x002E_EA04,
                destination_ram: 0xFFFF_FB80,
                words: 16,
            },
        },
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x16A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 160 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x16B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x16C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x16D },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x16E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::PanelCreate { id: 0x16F },
        SceneOp::DmaPlanes,
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::PanelCreate { id: 0x170 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 300 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 7,
            },
        },
        SceneOp::WaitFrames { frames: 100 },
        SceneOp::Presentation {
            op: PresentationOp::DmaPlanesLoop { frames: 60 },
        },
        SceneOp::InitVramAndCram,
        SceneOp::Presentation {
            op: PresentationOp::ClearPaletteLine { line: 3, words: 15 },
        },
        SceneOp::PanelCreate { id: 0x171 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0C)),
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1,
            },
        },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB80,
                destination_ram: 0xFFFF_FB00,
                words: 64,
            },
        },
        SceneOp::PanelCreate { id: 0x173 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1,
            },
        },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB80,
                destination_ram: 0xFFFF_FB00,
                words: 64,
            },
        },
        SceneOp::PanelCreate { id: 0x175 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1,
            },
        },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB80,
                destination_ram: 0xFFFF_FB00,
                words: 64,
            },
        },
        SceneOp::PanelCreate { id: 0x177 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1,
            },
        },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB80,
                destination_ram: 0xFFFF_FB00,
                words: 64,
            },
        },
        SceneOp::PanelCreate { id: 0x179 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1,
            },
        },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB80,
                destination_ram: 0xFFFF_FB00,
                words: 64,
            },
        },
        SceneOp::PanelCreate { id: 0x17C },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 5,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 1,
            },
        },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB80,
                destination_ram: 0xFFFF_FB00,
                words: 64,
            },
        },
        SceneOp::PanelCreate { id: 0x17E },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 5,
            },
        },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 0x0F,
            },
        },
        SceneOp::PlaySound { id: SOUND_STOP_ALL },
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone { frames: 60 },
        },
        SceneOp::Presentation {
            op: PresentationOp::ClearRamWords {
                destination_ram: 0xFFFF_FB80,
                words: 64,
                value: 0,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::ClearPlanes,
        },
        SceneOp::DmaPlanes,
        SceneOp::DrawTextToPlane {
            entry: 0x12,
            plane: 0xFFFF_8406,
            vram: 0xC100,
        },
        SceneOp::DmaPlanes,
        SceneOp::DrawTextToPlane {
            entry: 0x13,
            plane: 0xFFFF_8586,
            vram: 0xC180,
        },
        SceneOp::DmaPlanes,
        SceneOp::DrawTextToPlane {
            entry: 0x14,
            plane: 0xFFFF_8706,
            vram: 0xC200,
        },
        SceneOp::DmaPlanes,
        SceneOp::DrawTextToPlane {
            entry: 0x15,
            plane: 0xFFFF_8886,
            vram: 0xC280,
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingStaffRollTransition {
                palette_offset: 0x5E,
                fade_out_step: 0x222,
                fade_in_step: 0x222,
                frame_divisor: 8,
                first_wait: 180,
                music: MUSIC_STAFF_ROLL,
                second_wait: 180,
                target_bit: 0x0C,
                final_hold: 60,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingCreditsAssets {
                small_planet_rom_addr: 0x001D_E278,
                small_planet_vram: 0x2000,
                large_planet_rom_addr: 0x001D_EA48,
                large_planet_vram: 0x4000,
                credit_font_rom_addr: 0x002A_3350,
                credit_font_tile: 0x07C0,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingCreditsStage {
                stage: 1,
                scroll_delay: 0x380,
                foreground_step: 0,
                background_step: 0x2000,
                source_ram: 0xFFFF_1000,
                plane: CreditsPlane::B,
                commands_rom_addr: 0x0007_A522,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingCreditsStage {
                stage: 2,
                scroll_delay: 0x400,
                foreground_step: 0x2000,
                background_step: 0x1000,
                source_ram: 0xFFFF_2000,
                plane: CreditsPlane::A,
                commands_rom_addr: 0x0007_A522,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingCreditsStage {
                stage: 3,
                scroll_delay: 0x480,
                foreground_step: 0x2000,
                background_step: 0x1000,
                source_ram: 0xFFFF_2000,
                plane: CreditsPlane::A,
                commands_rom_addr: 0x0007_A548,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingCreditsPaletteRamp {
                primary_rom_addr: 0x0007_A274,
                secondary_rom_addr: 0x0007_A2A0,
                primary_words: 8,
                steps: 14,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone { frames: 60 },
        },
        SceneOp::LoadMap {
            map: 0x07B,
            prev_map: 0xFFFF,
            start_x: 0x36,
            start_y: 0x28,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingFinaleFieldPrep {
                character_ram: 0xFFFF_C000,
                character_longs: 0x50,
                secondary_ram: 0xFFFF_C300,
                secondary_longs: 0x70,
                scratch_ram: 0xFFFF_C540,
                scratch_longs: 0x10,
                plane_width: 0x18,
                plane_height: 0x1C,
                plane_flags: 0x8002,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB00,
                destination_ram: 0xFFFF_FB80,
                words: 64,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::FillRamWords {
                destination_ram: 0xFFFF_FB00,
                words: 64,
                value: 0x0EEE,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::EndingFinale {
                art_rom_addr: 0x001D_EEB8,
                art_tile: 0x580,
                mapping_rom_addr: 0x001D_F52C,
                mapping_vram: 0xC580,
                palette_rom_addr: 0x001D_F59A,
                palette_words: 16,
                lightning_frames: 40,
            },
        },
        SceneOp::WaitForStart,
        SceneOp::MarkGameCleared,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: false,
                mode: 3,
            },
        },
        SceneOp::End,
    ],
};
