//! The earthquake at Monsen (issue #81): a villager's talk about the quakes is
//! cut off by one, Gryz panics, villagers come to ask the party to stop them
//! and the party takes on the Plate System (`EventFlag_PlateSystem`).
//!
//! Record: `docs/scenes/143_MonsenEarthquake.md`. The body was read from the US
//! image through its `EventPtrs` slot; the clone's labels are navigation only.

use super::{CHAZ, DEMI, GRYZ, RIKA, RUNE};
use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::{CameraSubject, PresentationOp};
use crate::scene_runner::Scene;
use crate::state::{CharId, Flag};
use crate::trigger::EventIndex;

/// `$FFFFC480`, the object whose line `$0A` fires this.
const INFORMANT: usize = 6;
const LEADER: ActorRef = ActorRef::PartyMember(0);
/// `FieldObj_NPCScriptMove`, walking to its `dest` words.
const SCRIPT_MOVE: u16 = 0x194;

const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn face_informant() -> SceneOp {
    SceneOp::FaceOppositeOf {
        actor: ActorRef::Npc(INFORMANT),
        of: LEADER,
    }
}

const fn face(who: CharId, facing: Direction) -> SceneOp {
    SceneOp::Face {
        actor: ActorRef::Character(who),
        facing,
    }
}

const fn dest(who: CharId, x: i32, y: i32) -> SceneOp {
    SceneOp::SetActorDest {
        actor: ActorRef::Character(who),
        x,
        y,
    }
}

const fn quake(frames: u16, opening: bool) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::Quake { frames, opening },
    }
}

const fn villager(slot: usize, art_tile: u16, x: i32, y: i32) -> SceneOp {
    SceneOp::CreateFieldObject {
        slot,
        object_id: SCRIPT_MOVE,
        art_tile,
        x,
        y,
    }
}

const fn walk(slot: usize, x: i32, y: i32) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::WalkObject { slot, x, y },
    }
}

/// `popdlg` / `jsr Event_RunDialogue`: the next stretch of entry `$40`.
const RESUME: SceneOp = SceneOp::RunDialogueResume;
const REBUILD: SceneOp = SceneOp::Presentation {
    op: PresentationOp::RebuildSprites,
};
const VINT: SceneOp = SceneOp::WaitFrames { frames: 1 };
/// Two `VInt_Prepare` after the turn, and the one inside
/// `Event_UpdateObjFacing` (`$05A972`, US).
const SPIN_FRAMES: SceneOp = SceneOp::WaitFrames { frames: 3 };

/// `$002C`, `Event_MonsenEarthquake`, retail `$06E0EA..$06E633` with its
/// helpers and tables up to `$06E759` (`ps4.asm:147499-147854`). Dialogue `$F6`, tree 12
/// entry `$0A`, past the chain on `$65`, `$61` and `$60`. Entry `$40` holds the
/// whole talk, with nine `$F7` stops.
pub static MONSEN_EARTHQUAKE: Scene = Scene {
    name: "Event_MonsenEarthquake",
    event: EventIndex(0x002C),
    ops: &[
        face_informant(),
        standard(0x40),
        SceneOp::SetFollowMode { bits: 0b100 },
        quake(120, false),
        SceneOp::Wait { ticks: 120 },
        SceneOp::Face {
            actor: ActorRef::Npc(INFORMANT),
            facing: Direction::Down,
        },
        dest(CHAZ, 0x2E0, 0x220),
        dest(GRYZ, 0x2F0, 0x220),
        dest(RIKA, 0x300, 0x220),
        dest(DEMI, 0x2F0, 0x230),
        SceneOp::BranchIfPartyMember {
            who: RUNE,
            if_present: 11,
            if_absent: 12,
        },
        dest(RUNE, 0x2A0, 0x210),
        SceneOp::SetFollowMode { bits: 0b111 },
        quake(60, true),
        SceneOp::Wait { ticks: 60 },
        SceneOp::Wait { ticks: 1 },
        face(CHAZ, Direction::Right),
        face(GRYZ, Direction::Down),
        face(RIKA, Direction::Left),
        face(DEMI, Direction::Up),
        SceneOp::BranchIfPartyMember {
            who: RUNE,
            if_present: 21,
            if_absent: 22,
        },
        face(RUNE, Direction::Left),
        SceneOp::SetFollowMode { bits: 0b101 },
        RESUME,
        // Gryz spins, `loc_6E2E2`: twenty turns, two frames after each.
        face(GRYZ, Direction::Down),
        SPIN_FRAMES,
        face(GRYZ, Direction::Up),
        SPIN_FRAMES,
        face(GRYZ, Direction::Right),
        SPIN_FRAMES,
        face(GRYZ, Direction::Left),
        SPIN_FRAMES,
        face(GRYZ, Direction::Down),
        SPIN_FRAMES,
        face(GRYZ, Direction::Up),
        SPIN_FRAMES,
        face(GRYZ, Direction::Right),
        SPIN_FRAMES,
        face(GRYZ, Direction::Left),
        SPIN_FRAMES,
        face(GRYZ, Direction::Down),
        SPIN_FRAMES,
        face(GRYZ, Direction::Up),
        SPIN_FRAMES,
        face(GRYZ, Direction::Right),
        SPIN_FRAMES,
        face(GRYZ, Direction::Left),
        SPIN_FRAMES,
        face(GRYZ, Direction::Down),
        SPIN_FRAMES,
        face(GRYZ, Direction::Up),
        SPIN_FRAMES,
        face(GRYZ, Direction::Right),
        SPIN_FRAMES,
        face(GRYZ, Direction::Left),
        SPIN_FRAMES,
        face(GRYZ, Direction::Down),
        SPIN_FRAMES,
        face(GRYZ, Direction::Up),
        SPIN_FRAMES,
        face(GRYZ, Direction::Right),
        SPIN_FRAMES,
        face(GRYZ, Direction::Left),
        SPIN_FRAMES,
        RESUME,
        face(GRYZ, Direction::Down),
        SceneOp::Wait { ticks: 60 },
        RESUME,
        face(CHAZ, Direction::Down),
        face(RIKA, Direction::Down),
        RESUME,
        face(GRYZ, Direction::Left),
        RESUME,
        // The first villager, `$C500`.
        villager(8, 0x42B, 0x230, 0x220),
        REBUILD,
        VINT,
        walk(8, 0x2D0, 0x220),
        SceneOp::Wait { ticks: 80 },
        SceneOp::FaceParty {
            facing: Direction::Left,
        },
        RESUME,
        // The second, `$C540`.
        villager(9, 0x3E3, 0x240, 0x220),
        REBUILD,
        VINT,
        walk(9, 0x2E0, 0x230),
        SceneOp::Wait { ticks: 88 },
        SceneOp::FaceParty {
            facing: Direction::Down,
        },
        RESUME,
        // Chaz looks about: `DoMapUpdateLoop($1D)` three times.
        face(CHAZ, Direction::Left),
        SceneOp::Wait { ticks: 30 },
        face(CHAZ, Direction::Down),
        SceneOp::Wait { ticks: 30 },
        face(CHAZ, Direction::Left),
        SceneOp::Wait { ticks: 30 },
        face(CHAZ, Direction::Down),
        RESUME,
        // `$C580` and `$C5C0` come in from the right; `loc_6E698` runs all objects.
        villager(10, 0x42B, 0x3A0, 0x210),
        REBUILD,
        VINT,
        walk(10, 0x310, 0x210),
        villager(11, 0x39B, 0x3A0, 0x220),
        REBUILD,
        VINT,
        walk(11, 0x310, 0x220),
        SceneOp::Wait { ticks: 120 },
        RESUME,
        // All four leave, Y first.
        walk(8, 0x230, 0x220),
        walk(9, 0x230, 0x220),
        walk(10, 0x3D0, 0x210),
        walk(11, 0x3D0, 0x220),
        SceneOp::SetFollowMode { bits: 0b111 },
        SceneOp::Wait { ticks: 120 },
        SceneOp::SetFollowMode { bits: 0b101 },
        SceneOp::DespawnNpc {
            npc_index: 8,
            count: 4,
        },
        // `bclr #2`, `bclr #0`, the leader steps down, and the camera comes back.
        SceneOp::SetFollowMode { bits: 0 },
        SceneOp::MoveActorTo {
            actor: LEADER,
            x: 0x310,
            y: 0x220,
            wait: true,
        },
        SceneOp::PanCamera {
            subject: CameraSubject::Actor(LEADER),
            speed: 1,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x60),
            value: true,
        },
    ],
};
