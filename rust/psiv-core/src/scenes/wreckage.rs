//! The Wreckage's system (issue #81): Rika reads the Parma ship's computer on
//! Wreckage F2, the floor lights up and `EventFlag_WreckageSystem` is set.
//!
//! Record: `docs/scenes/206_WreckageEngine.md`. The body was read from the US
//! image through its `EventPtrs` slot; the clone's labels are navigation only.

use super::RIKA;
use super::vahal_common::{sound, standard, wait};
use crate::geom::Direction;
use crate::scene::{ActorRef, Axis, SceneOp, WreckageOp};
use crate::scene_presentation::{CameraSubject, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const LEADER: ActorRef = ActorRef::PartyMember(0);
const RIKA_OBJ: ActorRef = ActorRef::Character(RIKA);
/// `EventFlag_WreckageSystem`.
const WRECKAGE_SYSTEM: u16 = 0x40;
/// `SpcSFXID_SpaceshipRadar`.
const SFX_SPACESHIP_RADAR: u8 = 0xF8;
/// `Sound_StopSpcSFX`.
const STOP_SPC_SFX: u8 = 0xFD;

/// `bset`/`bclr #0` on `Char_Move_Flags` with bit 2 already set.
const FOLLOW_FREE: SceneOp = SceneOp::SetFollowMode { bits: 0b101 };
const FOLLOW_CHAIN: SceneOp = SceneOp::SetFollowMode { bits: 0b100 };

const fn offset(actor: ActorRef, dx: i32) -> SceneOp {
    SceneOp::MoveActorOffset {
        actor,
        dx,
        dy: 0,
        wait: false,
    }
}

const fn row_of(actor: ActorRef, target: ActorRef) -> SceneOp {
    SceneOp::MoveActorToActorAxis {
        actor,
        target,
        axis: Axis::Y,
        wait: false,
    }
}

const WAIT_LEADER: SceneOp = SceneOp::WaitForActor { actor: LEADER };

/// `$0029`, `Event_WreckageEngine`, retail `$06DBE8..$06DE7B`
/// (`ps4.asm:147172-147370`): read before, one line; otherwise Rika takes the
/// leader's place at the console, the text resumes with the next member's
/// portrait, the floor lights outward and the screens brighten,
/// `EventFlag_WreckageSystem` (`$40`) is set, and Rika and the leader trade
/// back.
pub static WRECKAGE_ENGINE: Scene = Scene {
    name: "Event_WreckageEngine",
    event: EventIndex(0x0029),
    ops: &[
        // 0-2: read before, the line `$48` only.
        SceneOp::BranchFlag {
            flag: Flag::event(WRECKAGE_SYSTEM),
            if_set: 1,
            if_clear: 3,
        },
        standard(0x48),
        SceneOp::End,
        // 3-7: entry `$47` up to its first `$F7`; `FieldObj_Step_Offset := 0`, `bset #2, Char_Move_Flags`; Rika in slot 0 skips the swap to op 19; `bset #0`.
        standard(0x47),
        SceneOp::SetStepOffset { value: 0 },
        FOLLOW_CHAIN,
        SceneOp::Wreckage(WreckageOp::BranchIfLeads {
            who: RIKA,
            if_leads: 19,
            if_follows: 7,
        }),
        FOLLOW_FREE,
        // 8-16: `loc_6DE7C`'s three legs: the leader and Rika trade places.
        offset(LEADER, -0x10),
        offset(RIKA_OBJ, 0x10),
        WAIT_LEADER,
        row_of(LEADER, RIKA_OBJ),
        row_of(RIKA_OBJ, LEADER),
        WAIT_LEADER,
        offset(LEADER, 0x10),
        offset(RIKA_OBJ, -0x10),
        WAIT_LEADER,
        // 17-18: `bclr #0`; Rika faces the console.
        FOLLOW_CHAIN,
        SceneOp::Face {
            actor: RIKA_OBJ,
            facing: Direction::Up,
        },
        // 19-22: `popdlg`, `loc_6DCBE`'s portrait, `Event_RunDialogue`, the radar, `Event_MoveCamera` to (leader's X, `$1F0`) at speed 1.
        SceneOp::Wreckage(WreckageOp::ResumePortraitFromParty),
        SceneOp::RunDialogueResume,
        sound(SFX_SPACESHIP_RADAR),
        SceneOp::PanCamera {
            subject: CameraSubject::ActorX {
                actor: LEADER,
                y: 0x1F0,
            },
            speed: 1,
        },
        // 23-38: `loc_6DD08`'s eight passes: chunk `$A6` at `(a1)`, `(a1,d7)`, `$20(a1)`, `$20(a1,d7)`, with `a1` chunk column `15 + k` on row 12 (`$1E0` and `$180`, both `>> 5`; the map is 32 chunks wide, so `$20` is row 13) and `d7` from -1 down by two; then `RefreshPlaneFG`, `DMAPlane_A_VInt` and three `VInt_Prepare`s.
        SceneOp::WriteMapChunks {
            chunks: &[
                (15, 12, 0xA6),
                (14, 12, 0xA6),
                (15, 13, 0xA6),
                (14, 13, 0xA6),
            ],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[
                (16, 12, 0xA6),
                (13, 12, 0xA6),
                (16, 13, 0xA6),
                (13, 13, 0xA6),
            ],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[
                (17, 12, 0xA6),
                (12, 12, 0xA6),
                (17, 13, 0xA6),
                (12, 13, 0xA6),
            ],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[
                (18, 12, 0xA6),
                (11, 12, 0xA6),
                (18, 13, 0xA6),
                (11, 13, 0xA6),
            ],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[
                (19, 12, 0xA6),
                (10, 12, 0xA6),
                (19, 13, 0xA6),
                (10, 13, 0xA6),
            ],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[(20, 12, 0xA6), (9, 12, 0xA6), (20, 13, 0xA6), (9, 13, 0xA6)],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[(21, 12, 0xA6), (8, 12, 0xA6), (21, 13, 0xA6), (8, 13, 0xA6)],
        },
        SceneOp::WaitFrames { frames: 4 },
        SceneOp::WriteMapChunks {
            chunks: &[(22, 12, 0xA6), (7, 12, 0xA6), (22, 13, 0xA6), (7, 13, 0xA6)],
        },
        SceneOp::WaitFrames { frames: 4 },
        // 39-46: `addi.w #$220, Palette_Table_Buffer+$1A` (word `$D`), eight `VInt_Prepare`s, four times.
        SceneOp::Presentation {
            op: PresentationOp::AddPaletteWord {
                offset: 0x0D,
                add: 0x0220,
            },
        },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::Presentation {
            op: PresentationOp::AddPaletteWord {
                offset: 0x0D,
                add: 0x0220,
            },
        },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::Presentation {
            op: PresentationOp::AddPaletteWord {
                offset: 0x0D,
                add: 0x0220,
            },
        },
        SceneOp::WaitFrames { frames: 8 },
        SceneOp::Presentation {
            op: PresentationOp::AddPaletteWord {
                offset: 0x0D,
                add: 0x0220,
            },
        },
        SceneOp::WaitFrames { frames: 8 },
        // 47-49: the flag, `$77 + 1` map-update frames, `popdlg` / `Event_RunDialogue`.
        SceneOp::SetFlag {
            flag: Flag::event(WRECKAGE_SYSTEM),
            value: true,
        },
        wait(120),
        SceneOp::RunDialogueResume,
        // 50-63: the swap back, the leader faces up, the camera returns; Rika in slot 0 skips all of it to `loc_6DE58`.
        SceneOp::Wreckage(WreckageOp::BranchIfLeads {
            who: RIKA,
            if_leads: 64,
            if_follows: 51,
        }),
        FOLLOW_FREE,
        offset(LEADER, -0x10),
        offset(RIKA_OBJ, 0x10),
        WAIT_LEADER,
        row_of(LEADER, RIKA_OBJ),
        row_of(RIKA_OBJ, LEADER),
        WAIT_LEADER,
        offset(LEADER, 0x10),
        offset(RIKA_OBJ, -0x10),
        WAIT_LEADER,
        FOLLOW_CHAIN,
        SceneOp::Face {
            actor: LEADER,
            facing: Direction::Up,
        },
        SceneOp::PanCamera {
            subject: CameraSubject::Actor(LEADER),
            speed: 1,
        },
        // 64-66: `loc_6DE58`: `FieldObj_Step_Offset := 1`, `Palette_Table_Buffer` words `$3E`/`$3F` := `$620`/`$2CE`, the radar stopped.
        SceneOp::SetStepOffset { value: 1 },
        SceneOp::Presentation {
            op: PresentationOp::SetPaletteWords {
                offset: 0x3E,
                first: 0x0620,
                second: 0x02CE,
            },
        },
        sound(STOP_SPC_SFX),
    ],
};
