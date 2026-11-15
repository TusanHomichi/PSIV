//! Retail-only surfaces that sit outside the fixed `$33..$54` event wave.
//!
//! The two routines here are still deterministic scene-shaped programs:
//! Raja Sick (`$8012`) and the Rykros arrival/tower hand-off (`$801B`). The
//! final presentation (`$8021`) is in [`super::ending`].

use crate::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{CameraSubject, PresentationOp, SAVE_PALETTE};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

use super::{CHAZ, RAJA, RIKA, RUNE, WREN};

const TREE39: u32 = 0x001F_AAC0;

const SFX_BARRIER_BROKEN: u8 = 0xE6;
const SFX_STAIRS: u8 = 0xDF;
const SFX_RED_ALERT: u8 = 0xA9;

/// Frame durations derived from `loc_782D2` at US `$0782D2`:
/// each `dbf` count spends one frame more than its byte.
/// The ROM-backed presentation test checks these against local retail input.
const RYKROS_PALETTE_DELAYS: &[u8] = &[8, 6, 8, 8, 8, 8];
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
        // `ps4.asm:156643-156646`: literal `($1A0,$240)`, speed 4.
        SceneOp::PanCamera {
            subject: CameraSubject::At { x: 0x1A0, y: 0x240 },
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
        // `ps4.asm:156688-156692`: `Character_1`'s live position, speed 4.
        SceneOp::PanCamera {
            subject: CameraSubject::Actor(ActorRef::PartyMember(0)),
            speed: 4,
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
        SAVE_PALETTE,
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone {
                frames: 30,
                hold: None,
            },
        },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PlaySound {
            id: SFX_BARRIER_BROKEN,
        },
        // $078220..$078231: `moveq #$1D, d7` / `jsr loc_58666` / `jsr
        // VInt_Prepare` / `dbf`: thirty frames back toward the copy.
        SceneOp::Presentation {
            op: PresentationOp::PaletteRestoreTone { frames: 30 },
        },
        SceneOp::WaitFrames { frames: 30 },
        // $07823A, $078264, $07827C: `move.b #1, (Render_Sprites_In_Cutscenes).w`
        // after each `popdlg`, before both `Event_RunDialogue3`s and the
        // `Event_RunDialogue5`. The last text's return cleared it, and the
        // panel portrait layout reads it.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::Presentation {
            op: PresentationOp::RykrosPaletteCycle {
                source_rom_addr: 0x0007_82DA,
                destination_ram: 0xFFFF_FB2E,
                words_per_frame: 9,
                delays: RYKROS_PALETTE_DELAYS,
            },
        },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
