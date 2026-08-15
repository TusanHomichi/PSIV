//! The scene registry: transcribed retail scenes as data.
//!
//! Each scene here is transcribed from `docs/scenes/`, which disassembled the
//! retail cartridge directly. That indirection matters: `ps4.asm` `include`s
//! `script/scenes/<Name>/event.asm` for eighteen scenes **ungated**, and that
//! directory does not exist in the clone at all — for those scenes the
//! reference contains no behaviour, just a label, an absent include and an
//! `rts`. Six of them are in the opening act. Nothing here was read off the
//! clone.
//!
//! # Coverage
//!
//! Two of the eleven opening-act scenes are transcribed. The remaining nine
//! are documented but not yet entered here; see the report accompanying this
//! module. They are mechanical to add now that the op vocabulary is complete —
//! and deliberately *not* guessed at, because a scene that runs but does the
//! wrong thing looks finished.
//!
//! | Scene | Event | Doc | Here |
//! |---|---|---|---|
//! | `Event_GameStart` | `$9F` | `01_GameStart.md` | no |
//! | `Event_PiataChazAlone` | `$A0` | `02_PiataChazAlone.md` | **yes** |
//! | `Event_AlysFound` | `$03` | `03_AlysFound.md` | **yes** |
//! | `Cutscene_PiataPrincipal` | `$8001` | `04_PiataPrincipal.md` | no |
//! | `Event_SuspicionOnPrincipal` | `$0F` | `05_SuspicionOnPrincipal.md` | no |
//! | `Event_MeetingHahn` | `$04` | `06_MeetingHahn.md` | no |
//! | `Event_BasementContainers` | `$0C` | `07_BasementContainers.md` | no |
//! | `Event_IgglanovaBattle` | `$6B` | `08_IgglanovaBattle.md` | no |
//! | `Event_AfterIgglanova` | `$25` | `09_AfterIgglanova.md` | no |
//! | `Event_PrincipalConfession` | `$26` | `10_PrincipalConfession.md` | no |
//! | `Event_PiataGuardsReprimand` | `$9E` | `11_PiataGuardsReprimand.md` | no |

use crate::scene::{ActorRef, Axis, DialogueId, DialogueSource, SceneOp};
use crate::scene_runner::Scene;
use crate::state::{CharId, Flag};
use crate::trigger::EventIndex;

/// Chaz's character id (`CharID_Chaz`).
pub const CHAZ: CharId = CharId(0);
/// Alys's character id (`CharID_Alys`).
pub const ALYS: CharId = CharId(1);

/// NPC-Alys on PiataAcademy_F1: field object `$FFFFC4C0`, which is
/// `$C300 + 7*$40` — map NPC index 7.
const NPC_ALYS: ActorRef = ActorRef::Npc(7);

/// `Event_PiataChazAlone` — retail `$073ECE..$073EDF`, 18 bytes.
///
/// Transcribed from `docs/scenes/02_PiataChazAlone.md`. Chaz notices he has
/// lost Alys. Two ops: one line of dialogue, one flag.
static PIATA_CHAZ_ALONE: &[SceneOp] = &[
    // tree 1, entry $6D — "Oops! I wandered around and now I've gotten
    // separated from Alys."
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(0x6D)),
    },
    // EventFlag_PiataChazControl: set when you gain control of Chaz in Piata.
    SceneOp::SetFlag {
        flag: Flag::event(0x15),
        value: true,
    },
    SceneOp::End,
];

/// `Event_AlysFound` — retail `$06B2D8..$06B3B9`, 226 bytes.
///
/// Transcribed from `docs/scenes/03_AlysFound.md`, which disassembled it from
/// cartridge bytes. The clone's copy is the act's most dangerous file: it kept
/// the retail prologue and epilogue and cut a hole in the middle, so it *looks*
/// like retail while omitting both the dialogue call and — critically — the
/// `EventFlags_Set` that stops `RunEvent_FindingAlys` re-firing forever.
///
/// Alys aligns with Chaz, turns to face him, they talk, and **Alys takes the
/// lead**: the party word write is `(Alys << 8) | Chaz`, so she is slot 1.
static ALYS_FOUND: &[SceneOp] = &[
    // 0: skip the alignment step when she is already level with Chaz.
    //    Retail: `cmp.w $34(a3),d0` on curr_y_pos.
    SceneOp::BranchIfAligned {
        a: ActorRef::Character(CHAZ),
        b: NPC_ALYS,
        axis: Axis::Y,
        if_aligned: 2,
        if_not: 1,
    },
    // 1: movement command 2 = one 16-pixel step down, spun until idle.
    SceneOp::MoveActorCommand {
        actor: NPC_ALYS,
        command: 2,
        wait: true,
    },
    // 2: FacingDir_Right.
    SceneOp::Face {
        actor: NPC_ALYS,
        facing: crate::geom::Direction::Right,
    },
    // 3: the entry index is read off her object's dialogue_id byte ($29), not
    //    written into the scene.
    SceneOp::RunDialogue {
        source: DialogueSource::NpcDialogueId(NPC_ALYS),
    },
    // 4: `move.w #$0100, $F40A` — Alys slot 1, Chaz slot 2.
    SceneOp::SetParty {
        slots: [Some(ALYS), Some(CHAZ), None, None, None],
    },
    // 5: `trap #1`, 32 words — Chaz's field-object struct into Character_2.
    //    The party slots were already written at op 4; this moves the object.
    SceneOp::CopyCharSlot { from: 0, to: 1 },
    // 6: Chaz's art tile in his new slot.
    SceneOp::SetArtTile {
        actor: ActorRef::Character(CHAZ),
        tile: 0x53C,
    },
    // 7: build field-Alys in slot 1 at NPC-Alys's position: obj id 8, facing
    //    left, art tile $534.
    SceneOp::PromoteNpcToChar {
        npc: 7,
        char_id: ALYS,
        // Event_AddMacro with d0 = CharID_Alys; slot 1 in the cartridge's
        // 1-based numbering, index 0 here, matching the word write at op 4.
        slot: 0,
        art_tile: 0x534,
        facing: crate::geom::Direction::Left,
    },
    // 8: `clr.w` + `trap #0` — NPC-Alys's object slot is cleared.
    SceneOp::DespawnNpc { npc_index: 7 },
    // 9: camera to Chaz's position at speed 2.
    SceneOp::MoveCamera {
        x: 0x260,
        y: 0xF0,
        speed: 2,
    },
    // 10: EventFlag_AlysFound. The clone omits this; retail ends on it.
    SceneOp::SetFlag {
        flag: Flag::event(0x08),
        value: true,
    },
    SceneOp::End,
];

/// Every transcribed scene, looked up by [`crate::EventIndex`].
pub static SCENES: &[Scene] = &[
    Scene {
        name: "Event_PiataChazAlone",
        event: EventIndex(0x00A0),
        ops: PIATA_CHAZ_ALONE,
    },
    Scene {
        name: "Event_AlysFound",
        event: EventIndex(0x0003),
        ops: ALYS_FOUND,
    },
];

/// The scene an event index selects, if it has been transcribed.
#[must_use]
pub fn scene_for(event: EventIndex) -> Option<&'static Scene> {
    SCENES.iter().find(|scene| scene.event == event)
}
