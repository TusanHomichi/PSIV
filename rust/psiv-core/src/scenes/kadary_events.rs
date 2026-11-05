//! Kadary's two events outside the guild's job (issue #81): the priest who
//! faints in the church, and the Zio believer who writhes in the street once
//! Zio is gone.
//!
//! The record is `docs/scenes/164_KadaryPriestAndBeliever.md`. Both bodies
//! were read from the US image through their `EventPtrs` slots; the clone's
//! labels are navigation only.

use super::aiedo_events::{face_leader, standard};
use crate::geom::Direction;
use crate::scene::{ActorRef, ObjectOp, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::trigger::EventIndex;

/// `KadaryChurch` object 9 (`$FFFFC540`), the priest.
const PRIEST: usize = 9;
/// `$FFFFC580`: the slot the invisible block is built in, past the church's
/// ten objects.
const BLOCK: usize = 10;
/// `FieldObjectsJmpTbl` `$180`, `FieldObj_FaintedPriest`.
const FAINTED_PRIEST: u16 = 0x180;
/// The line the priest and the block both answer with afterwards.
const AFTER_FAINTING: u16 = 0x38;
/// Frames of the `.animateloop` (`$06D640..$06D66B`): `FieldObj_FaintedPriest`
/// starts its six-mapping script (`loc_4A79E`, seven frames a mapping) at the
/// mapping `Event_UpdateObjFacing` left, mapping 0, and the loop stops on the
/// call that leaves mapping 5 with a duration of 1: call 46.
pub const FAINT_FRAMES: u16 = 46;

/// `Kadary` object 4 (`$FFFFC400`), the believer standing in the street.
const BELIEVER: usize = 4;
/// `Kadary` object 9 (`$FFFFC540`), the writhing believer.
const WRITHING: usize = 9;
/// `FieldObjectsJmpTbl` `$148`, `loc_49212`.
const WRITHING_ROUTINE: u16 = 0x148;

/// `$0024`, `Event_FaintingPriest`, retail `$06D616..$06D6C7`
/// (`ps4.asm:147047-147092`): the priest turns to the leader and speaks
/// entry `$45` up to its first stop, faints (`FieldObj_FaintedPriest` until
/// its script settles), an invisible block is built in slot 10 beside him so
/// he still fills two cells, the text resumes, and both the priest and the
/// block answer with `$38` from then on. Nothing is written that a map load
/// keeps: the next visit rebuilds him standing.
pub static FAINTING_PRIEST: Scene = Scene {
    name: "Event_FaintingPriest",
    event: EventIndex(0x0024),
    ops: &[
        // 0-1: `$06D616..$06D633`.
        face_leader(PRIEST),
        standard(0x45),
        // 2-4: `$06D634..$06D66B`: routine `$180`, `clr.w $6(a4)`, then the
        // routine, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare`
        // until mapping 5 holds a duration of 1. Only the priest runs.
        SceneOp::SetObjectRoutine {
            npc_index: PRIEST,
            routine: FAINTED_PRIEST,
        },
        SceneOp::Face {
            actor: ActorRef::Npc(PRIEST),
            facing: Direction::Down,
        },
        SceneOp::WaitFrames {
            frames: FAINT_FRAMES,
        },
        // 5-8: `$06D66C..$06D69D`: slot 10's id word from `($74).w` (zero in
        // the US image, so `FieldObj_None` once `FieldObj_InvisibleBlock`'s
        // init sets bit 15), at `($200,$140)` with dialogue `$38`; the init
        // sets render bit 3, so it blocks and answers. One sprite pass and a
        // `VInt_Prepare`.
        SceneOp::Object(ObjectOp::Spawn {
            slot: BLOCK,
            routine: 0,
            art_tile: 0,
            x: 0x200,
            y: 0x140,
            solid: true,
        }),
        SceneOp::SetNpcDialogue {
            npc_index: BLOCK,
            dialogue_id: AFTER_FAINTING,
        },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 1 },
        // 9-11: `$06D69E..$06D6C7`: `popdlg` / `Event_RunDialogue`, the
        // priest's dialogue `$38`, `Game_Mode_Routine` 0 and
        // `Map_LoadChunks`.
        SceneOp::RunDialogueResume,
        SceneOp::SetNpcDialogue {
            npc_index: PRIEST,
            dialogue_id: AFTER_FAINTING,
        },
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks,
        },
    ],
};

/// `$0031`, `Event_ZioFanatic`, retail `$06EE40..$06EEBD`
/// (`ps4.asm:148262-148298`, the US side of `if grand_cross=0`): after Zio,
/// the believer standing in Kadary's street (object 4) turns to the leader
/// and speaks entry `$49`, then object 9, the writhing believer, takes his
/// place, he is cleared, object 9 writhes for 30 frames and says `$1F`.
pub static ZIO_FANATIC: Scene = Scene {
    name: "Event_ZioFanatic",
    event: EventIndex(0x0031),
    ops: &[
        // 0-1: `$06EE40..$06EE5D`.
        face_leader(BELIEVER),
        standard(0x49),
        // 2: `$06EE5E..$06EE87`: slot 9 := `$148` at object 4's position,
        // rounded to 8 pixels, and one pass of `loc_49212`, whose init sets
        // render bit 3.
        SceneOp::Object(ObjectOp::SpawnAt {
            slot: WRITHING,
            routine: WRITHING_ROUTINE,
            at: ActorRef::Npc(BELIEVER),
            solid: true,
        }),
        // 3: `$06EE88..$06EE8F`: `loc_5B0C0` clears object 4's 64 bytes.
        SceneOp::DespawnNpc {
            npc_index: BELIEVER,
            count: 1,
        },
        // 4-5: `$06EE90..$06EEB5`: `moveq #$1D, d7` and a `dbf` loop of
        // `loc_49212`, `Field_LoadSprites`, `Field_BuildSprites`,
        // `VInt_Prepare`: 30 frames with only object 9 running. The first
        // pass's sprite calls draw object 9 where object 4 stood and drop
        // object 4, before the loop's first frame is shown.
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::WaitFrames { frames: 30 },
        // 6: `$06EEB6..$06EEBD`: entry `$1F`, tail `jmp`.
        standard(0x1F),
    ],
};
