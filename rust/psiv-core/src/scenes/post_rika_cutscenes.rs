//! Retail cutscenes dispatched after MeetingRika.
//!
//! The source routines are long 68k presentation scripts. Their state edges
//! are short and important: Demi rescue heals the party and starts battle 4;
//! Alys wounded removes Hahn/Alys and seats Demi; the Psycho Wand scene sets
//! the two Alys-death flags; Zio defeated removes Gryz/Demi and lands the
//! party back in Motavia. Panel and temporary-object choreography remains
//! typed data so the presentation lane can consume it without growing a
//! second field interpreter.

use super::{ALYS, CHAZ, DEMI, GRYZ, HAHN};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);

const TREE_5: u32 = 0x001E2680;

const MUSIC_HER_LAST_BREATH: u8 = 0xA4;
const MUSIC_PAIN: u8 = 0xA5;
const MUSIC_FIELD_MOTABIA: u8 = 0x8C;
const MUSIC_RED_ALERT: u8 = 0xA9;
const SFX_TELEPORT: u8 = 0xDE;
const SFX_SWORD: u8 = 0xF5;
const SFX_TANDLE: u8 = 0xCE;
const SOUND_STOP_MUSIC: u8 = 0xFB;

/// `Cutscene_DemiRescue`, CutscenePtrs[$08], retail
/// `$074A7E..$074B71` (244 bytes).
pub static DEMI_RESCUE: Scene = Scene {
    name: "Cutscene_DemiRescue",
    event: EventIndex(0x8008),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x42)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x40 },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: SFX_SWORD },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelDestroy { id: 0x40 },
        SceneOp::DmaPlanes,
        SceneOp::PanelDestroy { id: 0x40 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::PanelCreate { id: 0x42 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::RestorePartyHp { amount: 0x1C },
        SceneOp::SetFlag {
            flag: Flag::event(0x42),
            value: true,
        },
        SceneOp::SetSavedMusic {
            id: MUSIC_RED_ALERT,
        },
        SceneOp::SetMapLoadFlags {
            set: 0x88,
            clear: 0,
        },
        SceneOp::StartBattle { index: 4 },
        SceneOp::Return { value: 1 },
    ],
};

/// `Cutscene_AlysWounded`, CutscenePtrs[$09], retail
/// `$074B72..$0751FF` (1678 bytes).
pub static ALYS_WOUNDED: Scene = Scene {
    name: "Cutscene_AlysWounded",
    event: EventIndex(0x8009),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x43)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::PlaySound { id: SFX_TELEPORT },
        SceneOp::FadeOut,
        SceneOp::PanelDestroyAll,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x4A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 70 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::PlaySound { id: SFX_TELEPORT },
        SceneOp::Wait { ticks: 70 },
        SceneOp::BranchFlag {
            flag: Flag::event(0x12),
            if_set: 26,
            if_clear: 19,
        },
        // First-visit Saya branch: the ROM builds Krup Inn F1 from Motavia,
        // heals Alys through the field object sequence, then resumes twice.
        SceneOp::LoadMap {
            map: 0x3F,
            prev_map: 0x00,
            start_x: 0x4C,
            start_y: 0x6C,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound { id: 0x83 },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x8194,
            art_tile: 0x02B8,
            frames: 1,
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x2C)),
            window: DialogueWindow::Standard,
        },
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0x12),
            value: true,
        },
        // Both branches join here at `loc_74ECA`.
        SceneOp::PlaySound { id: MUSIC_PAIN },
        SceneOp::RemovePartyMember { who: HAHN },
        SceneOp::ClearCharacterStatus { who: HAHN },
        SceneOp::RemovePartyMember { who: ALYS },
        SceneOp::ClearCharacterStatus { who: ALYS },
        SceneOp::JoinParty { slot: 3, who: DEMI },
        SceneOp::Presentation {
            op: PresentationOp::AddMacro { slot: 6 },
        },
        SceneOp::LoadMap {
            map: 0x3F,
            prev_map: 0x3E,
            start_x: 0x46,
            start_y: 0x44,
            facing: Direction::Right,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
        SceneOp::MoveCamera {
            x: 0x1E0,
            y: 0x1E0,
            speed: 2,
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x2D)),
            window: DialogueWindow::Standard,
        },
        // Ten `popdlg`/`Event_RunDialogue` continuations, with the facing
        // writes between them retained by the presentation lane's timing.
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResume,
        SceneOp::SetFollowMode { bits: 1 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x210,
            y: 0x210,
            wait: true,
        },
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x210,
            y: 0x220,
            wait: true,
        },
        SceneOp::MoveCamera {
            x: 0x210,
            y: 0x220,
            speed: 2,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x47),
            value: true,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_5 },
        SceneOp::Return { value: 1 },
    ],
};

/// `Cutscene_PsycoWand`, CutscenePtrs[$0A], retail
/// `$075200..$075A11` (2066 bytes).
pub static PSYCO_WAND: Scene = Scene {
    name: "Cutscene_PsycoWand",
    event: EventIndex(0x800A),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(5)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::LoadMap {
            map: 0x3F,
            prev_map: 0x3E,
            start_x: 0x46,
            start_y: 0x44,
            facing: Direction::Left,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::PlaySound {
            id: MUSIC_HER_LAST_BREATH,
        },
        SceneOp::FadeIn,
        SceneOp::SetFollowMode { bits: 1 },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x2E)),
            window: DialogueWindow::Standard,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x56 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x57 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::PanelDestroyAll,
        SceneOp::FadeOut,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x5A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelCreate { id: 0x5B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 10 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::PanelCreate { id: 0x5D },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelCreate { id: 0x5E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::FadeOut,
        SceneOp::PanelDestroyAll,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::Wait { ticks: 90 },
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x60 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x61 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelCreate { id: 0x62 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 80 },
        SceneOp::PanelCreate { id: 0x63 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelCreate { id: 0x64 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 300 },
        SceneOp::FadeOut,
        SceneOp::PanelDestroyAll,
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x65 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x66 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x67 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::PanelDestroyAll,
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 90 },
        SceneOp::PanelCreate { id: 0x69 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x6A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::PanelCreate { id: 0x6C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::FadeOut,
        SceneOp::PlaySound {
            id: SOUND_STOP_MUSIC,
        },
        SceneOp::PanelDestroyAll,
        SceneOp::RecoverStats,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::LoadMap {
            map: 0x39,
            prev_map: 0x00,
            start_x: 0x28,
            start_y: 0x1C,
            facing: Direction::Left,
            align: 0x10,
            clear_load_flags: 0x08,
        },
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::ObjectAnimation {
            slot: 1,
            object_id: 0x8194,
            art_tile: 0x02B8,
            frames: 1,
        },
        SceneOp::ObjectAnimation {
            slot: 2,
            object_id: 0x000C,
            art_tile: 0x0480,
            frames: 1,
        },
        SceneOp::PlaySound { id: 0x83 },
        SceneOp::FadeIn,
        SceneOp::RunDialogueResume,
        SceneOp::SetFlag {
            flag: Flag::event(0x63),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x67),
            value: true,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_5 },
        SceneOp::Return { value: 1 },
    ],
};

/// `Cutscene_ZioDefeated`, CutscenePtrs[$0B], retail
/// `$075A12..$075FC7` (1462 bytes).
pub static ZIO_DEFEATED: Scene = Scene {
    name: "Cutscene_ZioDefeated",
    event: EventIndex(0x800B),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x6D },
        SceneOp::DmaPlanes,
        SceneOp::PlaySound { id: SFX_TANDLE },
        SceneOp::WaitFrames { frames: 6 },
        SceneOp::PlaySound { id: SFX_TANDLE },
        SceneOp::WaitFrames { frames: 6 },
        SceneOp::PlaySound { id: SFX_TANDLE },
        SceneOp::WaitFrames { frames: 3 },
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::PanelDestroyAll,
        SceneOp::FadeOut,
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
        SceneOp::Wait { ticks: 60 },
        SceneOp::LoadMap {
            map: 0x00,
            prev_map: 0xAC,
            start_x: 0x70,
            start_y: 0xBC,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::PlaySound {
            id: MUSIC_FIELD_MOTABIA,
        },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x73 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x74 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x75 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResume,
        SceneOp::RemovePartyMember { who: GRYZ },
        SceneOp::ClearCharacterStatus { who: GRYZ },
        SceneOp::RemovePartyMember { who: DEMI },
        SceneOp::ClearCharacterStatus { who: DEMI },
        SceneOp::ReviveIfDead { who: CHAZ },
        SceneOp::LoadMap {
            map: 0x00,
            prev_map: 0xAC,
            start_x: 0x6C,
            start_y: 0xB8,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::FadeIn,
        SceneOp::SetFlag {
            flag: Flag::event(0x68),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x66),
            value: true,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x61),
            value: true,
        },
        SceneOp::Return { value: 1 },
    ],
};
