//! The final presentation, `Cutscene_Ending` (`$8021`): the post-battle story
//! panels, the staff roll, the credits renderer and the final Start gate
//! (`docs/scenes/89_Ending.md`).

use crate::Direction;
use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{
    CreditsPlane, PaletteBuffer, PaletteSource, PresentationOp, RESTORE_PALETTE, SAVE_PALETTE,
};
use crate::scene_runner::Scene;
use crate::trigger::EventIndex;

const TREE42: u32 = 0x001F_C920;

const MUSIC_EXPLOSION: u8 = 0xAD;
const MUSIC_STAFF_ROLL: u8 = 0xAE;
const MUSIC_PROMISING_1: u8 = 0xAF;
const MUSIC_PROMISING_2: u8 = 0xB2;
const SOUND_STOP_ALL: u8 = 0xFE;

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
        // $078F9C..$078FAD: `moveq #$3B, d7` / `jsr Pal_IncreaseTone` / `jsr
        // VInt_Prepare` / `dbra`: sixty frames, the tone and its frames.
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone {
                frames: 60,
                hold: None,
            },
        },
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
        // `$07907E`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$07910E`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0x0B)),
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x150 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        // `$079146`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::PanelCreate { id: 0x187 },
        SceneOp::DmaPlanes,
        // `$07916A`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$079220`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 120 },
        // `$07923E`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$0792EC`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$07937E`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$0793D0`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        // `$0793EE`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$079454`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$0794B8`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x15F },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        // `$0794F0`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$079584`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$07965A`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        SAVE_PALETTE,
        SceneOp::Presentation {
            op: PresentationOp::PaletteCopy {
                source: PaletteSource::Rom(0x002E_EA04),
                destination: PaletteBuffer::Saved,
                start: 0,
                words: 16,
            },
        },
        // $0796E4..$0796F5: `moveq #$27, d7` / `jsr loc_58666` / `jsr
        // VInt_Prepare` / `dbf`: forty frames toward the copy just loaded.
        SceneOp::Presentation {
            op: PresentationOp::PaletteRestoreTone { frames: 40 },
        },
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x16A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        // `$079722`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$0797A0`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // `$07982A`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
            // $07987A..$079887: `lea Palette_Line_3, a0` / `moveq #0, d0` /
            // `moveq #$E, d7`: fifteen zero words from `$FFFFFB40`.
            op: PresentationOp::PaletteCopy {
                source: PaletteSource::Fill(0),
                destination: PaletteBuffer::Live,
                start: 32,
                words: 15,
            },
        },
        SceneOp::PanelCreate { id: 0x171 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        // `$0798A6`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        RESTORE_PALETTE,
        SceneOp::PanelCreate { id: 0x173 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        // `$079926`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        RESTORE_PALETTE,
        SceneOp::PanelCreate { id: 0x175 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        // `$0799A6`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        RESTORE_PALETTE,
        SceneOp::PanelCreate { id: 0x177 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        // `$079A26`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        RESTORE_PALETTE,
        SceneOp::PanelCreate { id: 0x179 },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        // `$079AA6`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        RESTORE_PALETTE,
        SceneOp::PanelCreate { id: 0x17C },
        SceneOp::DmaPlanes,
        // `$079B1E`: `move.b #1, ($FFFFED52).w` / `jsr Pal_VariableFadeIn`;
        // mode 5 is the last panel's (`$079B9E`).
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 1,
            },
        },
        // `$079B2C`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        RESTORE_PALETTE,
        SceneOp::PanelCreate { id: 0x17E },
        SceneOp::DmaPlanes,
        SceneOp::Presentation {
            op: PresentationOp::VariablePaletteFade {
                fade_in: true,
                mode: 5,
            },
        },
        // `$079BAC`: `move.b #1, (Render_Sprites_In_Cutscenes).w`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
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
        // $079BD2..$079BE3: the same sixty-frame loop.
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone {
                frames: 60,
                hold: None,
            },
        },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::Presentation {
            op: PresentationOp::PaletteCopy {
                source: PaletteSource::Fill(0),
                destination: PaletteBuffer::Saved,
                start: 0,
                words: 64,
            },
        },
        SceneOp::Presentation {
            op: PresentationOp::ClearPlanes,
        },
        // `$079C0A`: `jsr VDP_EnableDisplay`; the display has been off since
        // the `Pal_VariableFadeOut` before the loop.
        SceneOp::Presentation {
            op: PresentationOp::EnableDisplay,
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
        SceneOp::DmaPlanes,
        // $079C8E..$079CAD: `Palette_Table_Buffer+$5E` down by `$222` on
        // every eighth frame, until the step that would store zero.
        SceneOp::Presentation {
            op: PresentationOp::EndingStaffRollFadeOut {
                palette_offset: 0x5E,
                step: 0x222,
            },
        },
        // $079CAE..$079CC5: `move.w #$B3, d0` / `VInt_PrepareLoop`, the
        // staff-roll music, the same wait again.
        SceneOp::WaitFrames { frames: 180 },
        SceneOp::PlaySound {
            id: MUSIC_STAFF_ROLL,
        },
        SceneOp::WaitFrames { frames: 180 },
        // $079CCA..$079CED: back up by `$222` until the sum sets bit `$C`.
        SceneOp::Presentation {
            op: PresentationOp::EndingStaffRollFadeIn {
                palette_offset: 0x5E,
                step: 0x222,
                target_bit: 0x0C,
            },
        },
        // $079CEE..$079CF4: `ClearPlanes_A_B_Buf`, `DMAPlanes_VInt`.
        SceneOp::Presentation {
            op: PresentationOp::ClearPlanes,
        },
        SceneOp::DmaPlanes,
        // $079CFA..$079D0B: `moveq #$3B, d7` / `jsr loc_58666` / `jsr
        // VInt_Prepare` / `dbf`: sixty frames toward the cleared copy.
        SceneOp::Presentation {
            op: PresentationOp::PaletteRestoreTone { frames: 60 },
        },
        SceneOp::WaitFrames { frames: 60 },
        // `$079D0C`: normal fall-through and the held-Menu shortcut meet
        // at this Init, after the staff palette restore.
        SceneOp::InitVramAndCram,
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
        // `$079E70`: after the credits assets/initial plane upload, before
        // the stage-1 counters and scrolling loop.
        SceneOp::FadeIn,
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
                primary_ram: 0xFFFF_FB24,
                primary_words: 8,
                secondary_rom_addr: 0x0007_A2A0,
                secondary_ram: 0xFFFF_FB3C,
                secondary_words: 2,
                secondary_from: 9,
                steps: 14,
            },
        },
        // $07A0FA..$07A10B: the same sixty-frame loop.
        SceneOp::Presentation {
            op: PresentationOp::PaletteIncreaseTone {
                frames: 60,
                hold: None,
            },
        },
        SceneOp::WaitFrames { frames: 60 },
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
        SAVE_PALETTE,
        SceneOp::Presentation {
            op: PresentationOp::PaletteCopy {
                source: PaletteSource::Fill(0x0EEE),
                destination: PaletteBuffer::Live,
                start: 0,
                words: 64,
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
