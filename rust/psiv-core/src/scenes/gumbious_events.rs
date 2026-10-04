//! The Gumbious Temple's events (issue #83): the Eclipse Torch is stolen.
//!
//! Records are `docs/scenes/101_*` onward. The body was read from the US image
//! through its `EventPtrs` slot; the clone's labels are navigation only.
//! Party, map, flag, dialogue and timing writes stay explicit here; the
//! thieves' sprites are typed presentation records (`CreateFieldObject`,
//! `SetObjectDestination`), as in the other Dezolis scenes.

use crate::geom::Direction;
use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `DialogueTree37` and `DialogueTree38`: the `DialogueTreesToRAM` operands at
/// `$06FE56` and `$070174`.
const TREE_37: u32 = 0x001F_9580;
/// See [`TREE_37`].
const TREE_38: u32 = 0x001F_A480;

/// `MusicID_TheBlackBlood`, `SFXID_Teleport`, `Sound_StopMusic`,
/// `MusicID_Suspicion`, `MusicID_TempleNgangbius` (`ps4.constants.asm`).
const MUSIC_THE_BLACK_BLOOD: u8 = 0xA8;
const SFX_TELEPORT: u8 = 0xDE;
const STOP_MUSIC: u8 = 0xFB;
const MUSIC_SUSPICION: u8 = 0x9F;
const MUSIC_TEMPLE_NGANGBIUS: u8 = 0x93;

/// The art tile every thief object carries (`move.w #$36E, d0`).
const THIEF_TILE: u16 = 0x36E;

const LEADER: ActorRef = ActorRef::PartyMember(0);
/// The monk at (39,22): object 6, `$FFFFC480`.
const MONK: ActorRef = ActorRef::Npc(6);

const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

/// `jsr Event_RunDialogue` after `popdlg`.
const RESUME: SceneOp = SceneOp::RunDialogueResume;

const fn wait(ticks: u16) -> SceneOp {
    SceneOp::Wait { ticks }
}

const fn face(actor: ActorRef, facing: Direction) -> SceneOp {
    SceneOp::Face { actor, facing }
}

/// A thief object written at `$FFFFC300 + slot * $40`.
const fn thief(slot: usize, object_id: u16, x: i32, y: i32) -> SceneOp {
    SceneOp::CreateFieldObject {
        slot,
        object_id,
        art_tile: THIEF_TILE,
        x,
        y,
    }
}

const fn destination(slot: usize, x: i32, y: i32) -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::SetObjectDestination { slot, x, y },
    }
}

/// `$0046`, `Event_EclipseTorchStolen`, retail `$06FE56..$07018A`: the monk's
/// line, tree 37's entry `$32`, runs the scene. Six robed figures teleport in
/// around the party (`$FFFFC500..$FFFFC640`, objects 8 to 13), speak, four
/// more follow (objects 14 to 17) while the torch's two objects are cleared
/// (`$FFFFC300` and `$FFFFC340`, objects 0 and 1), the figures change shape
/// and the camera returns to the leader. `$98` is the last write.
pub static ECLIPSE_TORCH_STOLEN: Scene = Scene {
    name: "Event_EclipseTorchStolen",
    event: EventIndex(0x0046),
    ops: &[
        // `if revision>0`: tree 37.
        SceneOp::SetDialogueTree { rom_addr: TREE_37 },
        SceneOp::FaceOppositeOf {
            actor: MONK,
            of: LEADER,
        },
        standard(0x32),
        SceneOp::LoadArt {
            rom_addr: 0x001B_1D06,
            tile: THIEF_TILE,
        },
        face(MONK, Direction::Down),
        wait(30),
        face(LEADER, Direction::Down),
        face(ActorRef::PartyMember(1), Direction::Down),
        face(ActorRef::PartyMember(2), Direction::Down),
        face(ActorRef::PartyMember(3), Direction::Down),
        face(ActorRef::PartyMember(4), Direction::Down),
        wait(30),
        SceneOp::PlaySound {
            id: MUSIC_THE_BLACK_BLOOD,
        },
        SceneOp::MoveCamera {
            x: 0x270,
            y: 0xF0,
            speed: 1,
        },
        SceneOp::PlaySound { id: SFX_TELEPORT },
        thief(8, 0x280, 0x250, 0xF0),
        thief(9, 0x280, 0x270, 0xE0),
        thief(10, 0x280, 0x290, 0xF0),
        thief(11, 0x28C, 0x250, 0xF0),
        thief(12, 0x28C, 0x270, 0xE0),
        thief(13, 0x28C, 0x290, 0xF0),
        // `DoMainUpdatesLoop($61)`.
        wait(98),
        RESUME,
        SceneOp::PlaySound { id: SFX_TELEPORT },
        destination(8, 0x290, 0xF0),
        destination(10, 0x250, 0xF0),
        thief(14, 0x294, 0x250, 0xF0),
        destination(14, 0x290, 0xF0),
        thief(15, 0x294, 0x250, 0xF0),
        destination(15, 0x290, 0xF0),
        thief(16, 0x294, 0x290, 0xF0),
        destination(16, 0x24F, 0xF0),
        thief(17, 0x294, 0x290, 0xF0),
        destination(17, 0x250, 0xF0),
        // `DoMainUpdatesLoop($0F)`.
        wait(16),
        // The torch and its pedestal block: `move.w d0, (Field_Obj_Secondary).w`
        // and `($FFFFC340).w`.
        SceneOp::DespawnNpc {
            npc_index: 0,
            count: 2,
        },
        // `DoMainUpdatesLoop($59)`.
        wait(90),
        RESUME,
        SceneOp::PlaySound { id: SFX_TELEPORT },
        // The figures' ids change to `$288` / `$290` (`bset #1, $2(a4)` on the
        // first three).
        thief(8, 0x288, 0x290, 0xF0),
        thief(9, 0x288, 0x270, 0xE0),
        thief(10, 0x288, 0x250, 0xF0),
        thief(11, 0x290, 0x250, 0xF0),
        thief(12, 0x290, 0x270, 0xE0),
        thief(13, 0x290, 0x290, 0xF0),
        // `DoMainUpdatesLoop($55)`, then `DoMapUpdateLoop($3B)`.
        wait(86),
        wait(60),
        SceneOp::Presentation {
            op: PresentationOp::CameraToActor {
                actor: LEADER,
                speed: 1,
            },
        },
        SceneOp::PlaySound { id: STOP_MUSIC },
        wait(10),
        SceneOp::PlaySound {
            id: MUSIC_SUSPICION,
        },
        // `DoMapUpdateLoop(0)`: one iteration.
        wait(1),
        RESUME,
        SceneOp::PlaySound { id: STOP_MUSIC },
        wait(10),
        SceneOp::PlaySound {
            id: MUSIC_TEMPLE_NGANGBIUS,
        },
        // `if revision>0`: tree 38.
        SceneOp::SetDialogueTree { rom_addr: TREE_38 },
        SceneOp::SetFlag {
            flag: Flag::event(0x98),
            value: true,
        },
    ],
};
