//! The Esper Mansion's door and guard events (issue #83): short bodies, each a
//! few facings, a dialogue entry and a flag or two.
//!
//! Records are `docs/scenes/99_*` onward. Every body was read from the US image
//! through its `EventPtrs` slot; the clone's labels are navigation only.

use crate::scene::{ActorRef, DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

/// `DialogueTree20` and `DialogueTree21` (`ps4.asm` `DialogueTreesToRAM`
/// operands at `$06FE1C` and `$06FE44`).
const TREE_20: u32 = 0x001E_DCA0;
/// See [`TREE_20`].
const TREE_21: u32 = 0x001E_E9B0;

/// `moveq #entry, d0 / jsr Event_GetAndRunDialogue` (`$5AC66`): entry `entry`
/// of the current tree in the standard window, which every body here uses.
const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn set_flag(flag: Flag) -> SceneOp {
    SceneOp::SetFlag { flag, value: true }
}

const fn branch(flag: Flag, if_set: usize, if_clear: usize) -> SceneOp {
    SceneOp::BranchFlag {
        flag,
        if_set,
        if_clear,
    }
}

/// `lea (Field_Obj_Secondary).w, a4 / lea (Character_1).w, a3 / move.w $6(a3),
/// d0 / bchg #2, d0 / jsr Event_UpdateObjFacing`: object `npc` turns to face
/// the party leader.
const fn face_leader(npc: usize) -> SceneOp {
    SceneOp::FaceOppositeOf {
        actor: ActorRef::Npc(npc),
        of: ActorRef::PartyMember(0),
    }
}

/// `$004F`, `Event_EsperGuardPermission`, retail `$0709A2..$070A29`: the two
/// guards at the mansion door (objects 0 and 1) turn to the leader and speak
/// by what the party has done. With Dark Force 2 down (`$9E`) entry 2 lets
/// the party in, with the carnivorous trees fought (`$95`) entry 1 does, and
/// otherwise entry 0 turns them away. Either welcome rewrites both guards'
/// dialogue ids and sets `TempEveFlag_EspMansionGuards` (`$1A`), which
/// `FieldObj_EsperGuard` tests to step aside (`ps4.asm:96545`).
pub static ESPER_GUARD_PERMISSION: Scene = Scene {
    name: "Event_EsperGuardPermission",
    event: EventIndex(0x004F),
    ops: &[
        face_leader(0),
        face_leader(1),
        // Dark Force 2 defeated: the "please enter" line, entry 2.
        branch(Flag::event(0x9E), 6, 3),
        // Carnivorous trees fought: entry 1.
        branch(Flag::event(0x95), 10, 4),
        // Neither: entry 0 and the event ends without a flag.
        standard(0),
        SceneOp::End,
        // loc_709EE.
        standard(2),
        SceneOp::SetNpcDialogue {
            npc_index: 0,
            dialogue_id: 2,
        },
        SceneOp::SetNpcDialogue {
            npc_index: 1,
            dialogue_id: 2,
        },
        SceneOp::Jump { to: 13 },
        // loc_70A08.
        standard(1),
        SceneOp::SetNpcDialogue {
            npc_index: 0,
            dialogue_id: 1,
        },
        SceneOp::SetNpcDialogue {
            npc_index: 1,
            dialogue_id: 1,
        },
        // loc_70A20.
        set_flag(Flag::temp(0x1A)),
    ],
};

/// `$0045`, `Event_PersistentEsperGuards`, retail `$06FE1C..$06FE55`: the
/// inner guards (objects 3 and 4 of `EspMansionNorth`) bar Kyra, Rune speaks
/// them down, and Inner Sanctuary `$96` makes `FieldObj_InnerEsperGuards`
/// step aside (`ps4.asm:96583`). The entry is tree 20's; the map's own tree is
/// 21, so the body loads 20 and puts 21 back (both `if revision>0`).
pub static PERSISTENT_ESPER_GUARDS: Scene = Scene {
    name: "Event_PersistentEsperGuards",
    event: EventIndex(0x0045),
    ops: &[
        SceneOp::SetDialogueTree { rom_addr: TREE_20 },
        standard(0x2E),
        SceneOp::SetNpcDialogue {
            npc_index: 3,
            dialogue_id: 0x23,
        },
        SceneOp::SetNpcDialogue {
            npc_index: 4,
            dialogue_id: 0x23,
        },
        SceneOp::SetDialogueTree { rom_addr: TREE_21 },
        set_flag(Flag::event(0x96)),
    ],
};
