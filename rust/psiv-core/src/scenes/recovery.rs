//! `Event_Recovery` (`$21`), the recovery tile: `RunEvent_Recovery`
//! (`RunEventsJmpTbl[$12]`, `ps4.asm:115574-115581`) fires it on the frame the
//! leader steps onto a collision-2 cell from a cell that is not one. The Air
//! Castle has two (`AirCastle_F1_Part9` `$176` (31..32,28..29) and the
//! Xe-A-Thoul room `$184` (31..32,18..19)); the spaceports carry the rest.
//!
//! Record `docs/scenes/102_Recovery.md`. The body was read from the US image
//! through `EventPtrs[$21]` (`$05A2B4 + $84` holds `$0006D37C`); every
//! instruction word matches `ps4.asm:146852-146934`.

use crate::scene::SceneOp;
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::trigger::EventIndex;

/// `SFXID_Recovery` (`ps4.constants.asm:1001`).
const SFX_RECOVERY: u8 = 0xCD;
/// `SFXID_Res` (`ps4.constants.asm:1000`).
const SFX_RES: u8 = 0xCC;
/// `moveq #$F, d7` in both flash loops: sixteen `VInt_Prepare` frames.
const FLASH_FRAMES: u16 = 16;
/// `move.w #$1F, d0` in both flash loops: the first 32 palette words.
const FLASH_WORDS: u16 = 32;

/// One `bsr loc_6D3BA` (`brighten`) or `bsr loc_6D40A` and its frames.
const fn flash(brighten: bool) -> [SceneOp; 2] {
    [
        SceneOp::Presentation {
            op: PresentationOp::PaletteToneFlash {
                brighten,
                words: FLASH_WORDS,
                frames: FLASH_FRAMES,
            },
        },
        SceneOp::WaitFrames {
            frames: FLASH_FRAMES,
        },
    ]
}

const UP: [SceneOp; 2] = flash(true);
const DOWN: [SceneOp; 2] = flash(false);

/// `$0021`, `Event_Recovery`, retail `$06D37C..$06D467` (the body to the `rts`
/// at `$06D3B8` and its two flash subroutines).
pub static RECOVERY: Scene = Scene {
    name: "Event_Recovery",
    event: EventIndex(0x0021),
    ops: &[
        // $06D37C..$06D389: `lea Palette_Table_Buffer, a0` / `lea
        // Palette_Table_Buffer_2, a1` / `move.w #$1F, d7` / `trap #1`, a copy
        // of 32 longwords (all 64 palette words) to the second buffer.
        SceneOp::Presentation {
            op: PresentationOp::CopyRamWords {
                source_ram: 0xFFFF_FB00,
                destination_ram: 0xFFFF_FB80,
                words: 64,
            },
        },
        // $06D38A..$06D391: `move.b #SFXID_Recovery, (Sound_Index).l`.
        SceneOp::PlaySound { id: SFX_RECOVERY },
        // $06D392..$06D3A9: `bsr loc_6D3BA` / `bsr loc_6D40A`, three times.
        UP[0],
        UP[1],
        DOWN[0],
        DOWN[1],
        UP[0],
        UP[1],
        DOWN[0],
        DOWN[1],
        UP[0],
        UP[1],
        DOWN[0],
        DOWN[1],
        // $06D3AA..$06D3B1: `move.b #SFXID_Res, (Sound_Index).l`.
        SceneOp::PlaySound { id: SFX_RES },
        // $06D3B2..$06D3B7: `jsr RecoverStats` (`ps4.asm:136503`): every party
        // member's HP, TP and skill uses to their maximum and the status byte
        // to zero (an android shut down by bit 6 included), then the vehicles'
        // skill uses. `rts` at $06D3B8; a plain event's `d0` is ignored
        // (`loc_5A27A`, `ps4.asm:120553-120568`).
        SceneOp::RecoverStats,
    ],
};
