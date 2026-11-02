//! The guild's missing student (issue #81): the dorm owner in Piata, the
//! student found in Kadary's church, the old man and the Perolymate at
//! Kadary's inn, and the walk home to the dorm.
//!
//! The record is `docs/scenes/160_MissingStudent.md`. Every body was read from
//! the US image through its `EventPtrs` slot; the clone's labels are
//! navigation only.

use super::aiedo_events::{face_leader, set_flag, standard};
use crate::geom::Direction;
use crate::scene::{ActorRef, ItemOp, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `EventFlag_DormOwner`: the owner has told the party about her.
const DORM_OWNER: u16 = 0x27;
/// `EventFlag_StudentSick`: she collapsed in the church.
const STUDENT_SICK: u16 = 0x28;
/// `EventFlag_Perolymate`: the old man took the medicine.
const PEROLYMATE: u16 = 0x29;
/// `EventFlag_StudentRecovered`: she is home. The guild's board reads it.
const STUDENT_RECOVERED: u16 = 0x2A;
/// `ItemID_Perolymate`.
const ITEM_PEROLYMATE: u8 = 0x93;
/// `Sound_StopMusic`.
const STOP_MUSIC: u8 = 0xFB;
/// `MusicID_Inn`.
const MUSIC_INN: u8 = 0x82;
/// `MusicID_AHappySettlement`.
const MUSIC_A_HAPPY_SETTLEMENT: u8 = 0x9E;
/// `MapID_PiataDorm`, `MapID_Piata`, `MapID_KadaryInn`, `MapID_Kadary`.
const PIATA_DORM: u16 = 0x18;
const PIATA: u16 = 0x10;
const KADARY_INN: u16 = 0x71;
const KADARY: u16 = 0x68;
/// `KadaryChurch` object 4 (`$FFFFC400`).
const STUDENT_IN_CHURCH: usize = 4;
/// `KadaryInn` object 1 (`$FFFFC340`).
const OLD_MAN: usize = 1;
/// `KadaryInn` object 2 (`$FFFFC380`).
const STUDENT_IN_BED: usize = 2;

/// `moveq #facing, d0 / jsr Event_UpdateObjFacing` on the student, then
/// `moveq #n, d0 / jsr DoMapUpdateLoop`, which runs `n + 1` times.
const fn look(facing: Direction) -> SceneOp {
    SceneOp::Face {
        actor: ActorRef::Npc(STUDENT_IN_CHURCH),
        facing,
    }
}
const LOOK_WAIT: SceneOp = SceneOp::Wait { ticks: 10 };

/// `$0076`, `Event_PiataDormOwner`, retail `$072898..$0728BD`
/// (`ps4.asm:152399-152408`): the owner (`PiataDorm` object 0,
/// `Field_Obj_Secondary`) turns to the leader, speaks entry `$6C` and sets
/// `EventFlag_DormOwner`.
pub static PIATA_DORM_OWNER: Scene = Scene {
    name: "Event_PiataDormOwner",
    event: EventIndex(0x0076),
    ops: &[face_leader(0), standard(0x6C), set_flag(DORM_OWNER)],
};

/// `$0077`, `Event_MissingStudentFound`, retail `$0728BE..$0729E3`
/// (`ps4.asm:152409-152478`): before the dorm owner has spoken she only says
/// entry `$52`. After it she speaks, looks round, speaks again and collapses:
/// the screen goes black, the inn's music starts, `EventFlag_StudentSick` is
/// set and the party wakes at Kadary's inn.
pub static MISSING_STUDENT_FOUND: Scene = Scene {
    name: "Event_MissingStudentFound",
    event: EventIndex(0x0077),
    ops: &[
        face_leader(STUDENT_IN_CHURCH),
        // 1: `EventFlags_Test` / `bne.s loc_728E6`.
        SceneOp::BranchFlag {
            flag: Flag::event(DORM_OWNER),
            if_set: 4,
            if_clear: 2,
        },
        standard(0x52),
        SceneOp::End,
        // 4: `loc_728E6`.
        standard(0x4C),
        look(Direction::Down),
        LOOK_WAIT,
        look(Direction::Right),
        LOOK_WAIT,
        look(Direction::Up),
        LOOK_WAIT,
        look(Direction::Left),
        LOOK_WAIT,
        look(Direction::Down),
        LOOK_WAIT,
        look(Direction::Right),
        LOOK_WAIT,
        look(Direction::Down),
        // `moveq #$1D, d0 / jsr DoMapUpdateLoop`.
        SceneOp::Wait { ticks: 30 },
        standard(0x4D),
        SceneOp::FadeOut,
        SceneOp::PlaySound { id: STOP_MUSIC },
        SceneOp::Wait { ticks: 15 },
        SceneOp::PlayMusicIfSavedDifferent { id: MUSIC_INN },
        SceneOp::WaitFrames { frames: 1 },
        set_flag(STUDENT_SICK),
        SceneOp::LoadMap {
            map: KADARY_INN,
            prev_map: KADARY,
            start_x: 0x48,
            start_y: 0x38,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0b0000_1000,
        },
        SceneOp::FadeIn,
        standard(0x53),
    ],
};

/// `$0078`, `Event_OldManNearMissingStudent`, retail `$0729E4..$072A25`
/// (`ps4.asm:152479-152496`): the old man (`KadaryInn` object 1) turns to the
/// leader. With a Perolymate in the bag he takes it, speaks entry `$4F` and
/// sets `EventFlag_Perolymate`; without one he speaks entry `$53`.
pub static OLD_MAN_NEAR_MISSING_STUDENT: Scene = Scene {
    name: "Event_OldManNearMissingStudent",
    event: EventIndex(0x0078),
    ops: &[
        face_leader(OLD_MAN),
        SceneOp::Item(ItemOp::Take {
            item: ITEM_PEROLYMATE,
            if_missing: 5,
        }),
        standard(0x4F),
        set_flag(PEROLYMATE),
        SceneOp::End,
        // 5: `loc_72A1E`.
        standard(0x53),
    ],
};

/// `$0079`, `Event_MissingStudentInBed`, retail `$072A26..$072ACF`
/// (`ps4.asm:152497-152535`): the student (`KadaryInn` object 2) turns to the
/// leader. Before the medicine she only says entry `$50`. After it she speaks
/// entry `$54`, the screen fades, the happy music starts and fades back in
/// over the inn, `EventFlag_StudentRecovered` is set, and the party stands in
/// Piata's dorm for entry `$5B`. Unlike `$77` the `Pal_FadeIn` comes before
/// `RefreshMap`.
pub static MISSING_STUDENT_IN_BED: Scene = Scene {
    name: "Event_MissingStudentInBed",
    event: EventIndex(0x0079),
    ops: &[
        face_leader(STUDENT_IN_BED),
        SceneOp::BranchFlag {
            flag: Flag::event(PEROLYMATE),
            if_set: 4,
            if_clear: 2,
        },
        standard(0x50),
        SceneOp::End,
        // 4: `loc_72A4E`.
        standard(0x54),
        SceneOp::FadeOut,
        SceneOp::PlaySound { id: STOP_MUSIC },
        SceneOp::Wait { ticks: 15 },
        SceneOp::PlayMusicIfSavedDifferent {
            id: MUSIC_A_HAPPY_SETTLEMENT,
        },
        SceneOp::FadeIn,
        SceneOp::WaitFrames { frames: 1 },
        set_flag(STUDENT_RECOVERED),
        SceneOp::LoadMap {
            map: PIATA_DORM,
            prev_map: PIATA,
            start_x: 0x5A,
            start_y: 0x36,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0b0000_1000,
        },
        standard(0x5B),
    ],
};
