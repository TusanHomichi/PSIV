//! `Cutscene_Elsydeon` (`$801E`), the Elsydeon cave's panel sequence:
//! transcribed from the US bytes with every palette loop, white fill and
//! inline page (`docs/scenes/83_Elsydeon.md`).

use super::{CHAZ, cutscene3};
use crate::geom::Direction;
use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{PaletteBuffer, PaletteSource, PresentationOp, SAVE_PALETTE};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const MUSIC_THE_AGE_OF_FABLES: u8 = 0xA1;
/// `SFXID_Deban` (`ps4.constants.asm:1016`), played under `if revision>0`
/// (`ps4.asm:157483`) between the `$31` dialogue's wait and the `$12B` panel.
const SFX_DEBAN: u8 = 0xDC;
const ITEM_ELSYDEON: u8 = 0x77;

const fn cutscene(id: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(id)),
        window: DialogueWindow::Cutscene,
    }
}

/// The white `Cutscene_Elsydeon` fills its buffer with (`move.l #$EEE0EEE`).
const WHITE: u16 = 0x0EEE;

/// Forty frames of `Pal_IncreaseTone` (`moveq #$27, d7`), nothing held.
const fn tone() -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::PaletteIncreaseTone {
            frames: 40,
            hold: None,
        },
    }
}

/// Forty frames of `loc_58666` back toward `Palette_Table_Buffer_2`.
const fn restore() -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::PaletteRestoreTone { frames: 40 },
    }
}

const fn palette_copy(
    source: PaletteSource,
    destination: PaletteBuffer,
    start: u8,
    words: u8,
) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::PaletteCopy {
            source,
            destination,
            start,
            words,
        },
    }
}

/// `$801E`, `Cutscene_Elsydeon`, `$078584..$078A7B`
/// (`docs/scenes/83_Elsydeon.md`). Every `VInt_PrepareLoop` of `n` is `n + 1`
/// frames; the palette loops are forty frames of `Pal_IncreaseTone` or
/// `loc_58666` each, the white fills `move.l #$EEE0EEE` runs over the live
/// buffer after a `trap #1` copy to the saved one.
pub static ELSYDEON: Scene = Scene {
    name: "Cutscene_Elsydeon",
    event: EventIndex(0x801E),
    ops: &[
        SceneOp::InitVramAndCram,
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x129 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        // `$0785B2`: `jsr Event_GetAndRunDialogue3`.
        SceneOp::SetRenderSpritesInCutscene { enabled: true },
        cutscene3(0x31),
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PlaySound { id: SFX_DEBAN },
        SceneOp::PanelCreate { id: 0x12B },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x12C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PlaySound {
            id: MUSIC_THE_AGE_OF_FABLES,
        },
        // `loc_78608`: the screen brightens to white.
        tone(),
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelCreate { id: 0x12D },
        // `Palette_Line_4` to `Palette_2_Line_4` (`moveq #$F, d7 / trap #1`),
        // then eight longwords of white over it (`loc_78652`).
        palette_copy(
            PaletteSource::Buffer(PaletteBuffer::Live),
            PaletteBuffer::Saved,
            48,
            16,
        ),
        palette_copy(PaletteSource::Fill(WHITE), PaletteBuffer::Live, 48, 16),
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 60 },
        // `loc_7866A`: back from white.
        restore(),
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x12E },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        // `$0786AC`: `popdlg` / `jsr Event_RunDialogue3`.
        SceneOp::RunDialogueResume,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x12F },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 240 },
        // The whole buffer saved and filled white (`loc_786F2`).
        SAVE_PALETTE,
        palette_copy(PaletteSource::Fill(WHITE), PaletteBuffer::Live, 0, 64),
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        // `loc_7870C`.
        restore(),
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x130 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 20 },
        // `$078740`: `popdlg`, then the scene's own window code (`Window_Create`,
        // `RunText`, a button wait). The cartridge leaves that window up
        // until `$078810`'s `Window_Destroy`; the port closes it at the
        // button (`docs/source-notes/README.md`).
        SceneOp::RunDialogueResume,
        SceneOp::WaitFrames { frames: 30 },
        SceneOp::PanelCreate { id: 0x131 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 50 },
        SceneOp::PanelCreate { id: 0x132 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 240 },
        // `loc_787E4`.
        SAVE_PALETTE,
        palette_copy(PaletteSource::Fill(WHITE), PaletteBuffer::Live, 0, 64),
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::DmaPlanes,
        // `loc_7882E`.
        restore(),
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::WaitFrames { frames: 60 },
        SceneOp::PanelCreate { id: 0x18C },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        SceneOp::PanelCreate { id: 0x133 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        SceneOp::PanelCreate { id: 0x134 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        SceneOp::PanelCreate { id: 0x135 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        // `loc_788B2`.
        tone(),
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelDestroyLast,
        SceneOp::PanelCreate { id: 0x136 },
        // Lines 0 and 1 saved (`moveq #$1F, d7 / trap #1`) and filled white
        // (`loc_78902`, sixteen longwords).
        palette_copy(
            PaletteSource::Buffer(PaletteBuffer::Live),
            PaletteBuffer::Saved,
            0,
            32,
        ),
        palette_copy(PaletteSource::Fill(WHITE), PaletteBuffer::Live, 0, 32),
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        // `loc_7891A`.
        restore(),
        SceneOp::WaitFrames { frames: 40 },
        // `$07892A`: the second inline page, its window as at `$078740`.
        SceneOp::RunDialogueResume,
        SceneOp::Wait { ticks: 60 },
        // `loc_7897E`, then `Panel_DestroyAll` and the windows' teardown.
        tone(),
        SceneOp::WaitFrames { frames: 40 },
        SceneOp::PanelDestroyAll,
        SceneOp::InitVramAndCram,
        SceneOp::Wait { ticks: 60 },
        SceneOp::FadeIn,
        SceneOp::PanelCreate { id: 0x139 },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 120 },
        SceneOp::PanelCreate { id: 0x13A },
        SceneOp::DmaPlanes,
        SceneOp::WaitFrames { frames: 160 },
        SceneOp::PanelCreate { id: 0x13B },
        SceneOp::DmaPlanes,
        // `$078A1E`: `jsr Event_GetAndRunDialogue5`.
        cutscene(0x32),
        SceneOp::SetFlag {
            flag: Flag::event(0xD9),
            value: true,
        },
        SceneOp::SetCharacterEquipment {
            who: CHAZ,
            slots: [Some(ITEM_ELSYDEON), Some(0), None, None],
        },
        SceneOp::RestorePartySlots,
        SceneOp::LoadMap {
            map: 0x16F,
            prev_map: 0xFFFF,
            start_x: 0x3C,
            start_y: 0x32,
            facing: Direction::Down,
            align: 0,
            clear_load_flags: 0x08,
        },
        SceneOp::Return { value: 0 },
    ],
};
