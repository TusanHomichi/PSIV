//! Rocky, the Hunter's Guild's dog job (issue #81): found at Monsen, then run
//! down at Monsen, Krup and Termi until the party holds a shortcake.
//!
//! Record: `docs/scenes/141_Rocky.md`. Both bodies were read from the US image
//! through their `EventPtrs` slots; the clone's labels are navigation only.

use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, RockyOp, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `MapID_Krup`.
const KRUP: &[u16] = &[0x39];
/// `MapID_Monsen`.
const MONSEN: &[u16] = &[0x73];
/// `MapID_RockyHouse`.
const ROCKY_HOUSE: u16 = 0x65;
/// `MapID_Aiedo`.
const AIEDO: u16 = 0x54;
/// `$FFFFC5C0`: Rocky's slot on Krup.
const AT_KRUP: usize = 11;
/// `$FFFFC4C0`: on Monsen.
const AT_MONSEN: usize = 7;
/// `$FFFFC540`: on Termi.
const AT_TERMI: usize = 9;
/// `ItemID_ShortCake`.
const SHORTCAKE: u8 = 0x91;
/// `loc_4BC80`, the dog making off to his `dest` words.
const RUNNING_OFF: u16 = 0x2F8;
/// `subi.w #$B0, d0`: the run ends eleven cells left of where he stood.
const RUN: i32 = -0xB0;
/// `MusicID_AHappySettlement`.
const MUSIC_A_HAPPY_SETTLEMENT: u8 = 0x9E;

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue` (`$5AC66`).
const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn set_flag(id: u16, value: bool) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value,
    }
}

/// The object turns to face the leader.
const fn face_leader(npc: usize) -> SceneOp {
    SceneOp::FaceOppositeOf {
        actor: ActorRef::Npc(npc),
        of: ActorRef::PartyMember(0),
    }
}

/// `move.w #$2F8, (a4)`.
const fn run_routine(npc: usize) -> SceneOp {
    SceneOp::SetObjectRoutine {
        npc_index: npc,
        routine: RUNNING_OFF,
    }
}

const fn run_fast(npc: usize) -> SceneOp {
    SceneOp::Rocky(RockyOp::RunFast {
        actor: ActorRef::Npc(npc),
    })
}

/// `Event_MoveSingleObject` to his own X less `$B0`, his own Y.
const fn run_off(npc: usize) -> SceneOp {
    SceneOp::MoveActorOffset {
        actor: ActorRef::Npc(npc),
        dx: RUN,
        dy: 0,
        wait: true,
    }
}

/// `clr.w (a4)`.
const fn gone(npc: usize) -> SceneOp {
    SceneOp::DespawnNpc {
        npc_index: npc,
        count: 1,
    }
}

const REBUILD: SceneOp = SceneOp::Presentation {
    op: PresentationOp::RebuildSprites,
};
const VINT: SceneOp = SceneOp::WaitFrames { frames: 1 };

/// `$0073`, `Event_RockyFound`, retail `$07269E..$0726C3`
/// (`ps4.asm:152242-152251`): the dog at Monsen (object 7) turns, speaks entry
/// `$2C` and sets `EventFlag_RockyFound` (`$20`), which sends him to Termi.
pub static ROCKY_FOUND: Scene = Scene {
    name: "Event_RockyFound",
    event: EventIndex(0x0073),
    ops: &[face_leader(AT_MONSEN), standard(0x2C), set_flag(0x20, true)],
};

/// Op index of the caught arm, `loc_727CA`.
const CAUGHT: u16 = 43;

/// `$0074`, `Event_CatchingRocky`, retail `$0726C4..$072871`
/// (`ps4.asm:152252-152386`). The routine reads `loc_72866`, one row per map:
/// the first line, the flag for the map he leaves and the caught line. Each
/// map's reads are one arm here. Without a shortcake he speaks, runs off,
/// the line resumes at its `$F7` and that map's escape flag goes on; with one
/// he speaks, every shortcake goes, `EventFlag_RockyHome` (`$24`) is set and
/// the party is at his house.
pub static CATCHING_ROCKY: Scene = Scene {
    name: "Event_CatchingRocky",
    event: EventIndex(0x0074),
    ops: &[
        // 0-6: `lea ($FFFFC5C0).w`, `$C4C0` or `$C540`, and the turn.
        SceneOp::SkipUnlessMap {
            maps: KRUP,
            skip: 2,
        },
        face_leader(AT_KRUP),
        SceneOp::SkipOps { count: 4 },
        SceneOp::SkipUnlessMap {
            maps: MONSEN,
            skip: 2,
        },
        face_leader(AT_MONSEN),
        SceneOp::SkipOps { count: 1 },
        face_leader(AT_TERMI),
        // 7: `GetItem`, `beq.w loc_727CA`.
        SceneOp::Rocky(RockyOp::BranchIfHolding {
            item: SHORTCAKE,
            if_held: CAUGHT,
            if_not: 8,
        }),
        // 8-10: all three escape flags clear.
        set_flag(0x21, false),
        set_flag(0x22, false),
        set_flag(0x23, false),
        // 11-21: Krup's row, `$35`, `$22`.
        SceneOp::SkipUnlessMap {
            maps: KRUP,
            skip: 10,
        },
        standard(0x35),
        run_routine(AT_KRUP),
        REBUILD,
        VINT,
        run_fast(AT_KRUP),
        run_off(AT_KRUP),
        gone(AT_KRUP),
        SceneOp::RunDialogueResume,
        set_flag(0x22, true),
        SceneOp::End,
        // 22-32: Monsen's row, `$2A`, `$23`.
        SceneOp::SkipUnlessMap {
            maps: MONSEN,
            skip: 10,
        },
        standard(0x2A),
        run_routine(AT_MONSEN),
        REBUILD,
        VINT,
        run_fast(AT_MONSEN),
        run_off(AT_MONSEN),
        gone(AT_MONSEN),
        SceneOp::RunDialogueResume,
        set_flag(0x23, true),
        SceneOp::End,
        // 33-42: Termi's row, `$4C`, `$21`.
        standard(0x4C),
        run_routine(AT_TERMI),
        REBUILD,
        VINT,
        run_fast(AT_TERMI),
        run_off(AT_TERMI),
        gone(AT_TERMI),
        SceneOp::RunDialogueResume,
        set_flag(0x21, true),
        SceneOp::End,
        // 43-49: `loc_727CA`, the row's last byte.
        SceneOp::SkipUnlessMap {
            maps: KRUP,
            skip: 2,
        },
        standard(0x36),
        SceneOp::SkipOps { count: 4 },
        SceneOp::SkipUnlessMap {
            maps: MONSEN,
            skip: 2,
        },
        standard(0x2B),
        SceneOp::SkipOps { count: 1 },
        standard(0x4D),
        // 50: `loc_727EE`.
        SceneOp::Rocky(RockyOp::RemoveAll { item: SHORTCAKE }),
        // 51-53: `PalFadeOut_ClrSpriteTbl`, `$24`, `RefreshMap`.
        SceneOp::FadeOut,
        set_flag(0x24, true),
        SceneOp::LoadMap {
            map: ROCKY_HOUSE,
            prev_map: AIEDO,
            start_x: 0x3C,
            start_y: 0x42,
            facing: Direction::Left,
            align: 0x10,
            clear_load_flags: 0x08,
        },
        // 54-55.
        SceneOp::PlaySound {
            id: MUSIC_A_HAPPY_SETTLEMENT,
        },
        SceneOp::FadeIn,
        // 56-57: the owner at `$FFFFC340`, in the new map's tree 11.
        SceneOp::Face {
            actor: ActorRef::Npc(1),
            facing: Direction::Right,
        },
        standard(0x46),
    ],
};
