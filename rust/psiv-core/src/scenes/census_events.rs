//! Events the census found fired and untranscribed (issues #56 and #71): short
//! bodies, each one dialogue, a flag and, for four of them, an event battle.
//!
//! Records are `docs/scenes/93_*` onward. Every body was read from the US
//! image through its `EventPtrs` slot; the clone's labels are navigation only.

use super::retained;
use crate::scene::{DialogueId, DialogueSource, DialogueWindow, SceneOp};
use crate::scene_runner::Scene;
use crate::state::Flag;
use crate::trigger::EventIndex;

const fn standard(entry: u16) -> SceneOp {
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(entry)),
        window: DialogueWindow::Standard,
    }
}

const fn set_flag(id: u16) -> SceneOp {
    SceneOp::SetFlag {
        flag: Flag::event(id),
        value: true,
    }
}

/// `bset #3, Map_Load_Flags` (`$FFFFEC4E`), the bit every event battle sets
/// before the exit flag.
const LOAD_FLAGS_EVENT_BATTLE: SceneOp = SceneOp::SetMapLoadFlags {
    set: 0x08,
    clear: 0,
};

/// `$002A`, `Event_CancellerReminder`, retail `$06DEAE..$06DEBD`: Zelan F1's
/// reminder when the party reaches the elevator before opening the Canceller
/// chest (runner log H26).
pub static CANCELLER_REMINDER: Scene = Scene {
    name: "Event_CancellerReminder",
    event: EventIndex(0x002A),
    ops: &[standard(0x0C), set_flag(0x73)],
};

/// `$0071`, `Event_MileSandWormBattle`, retail `$072654..$072677`: the Mile
/// sandworm, one in thirty-two per eligible step (`RunEvent_MileSandWorm`).
pub static MILE_SAND_WORM_BATTLE: Scene = Scene {
    name: "Event_MileSandWormBattle",
    event: EventIndex(0x0071),
    ops: &[
        set_flag(0x1B),
        SceneOp::SetSavedMusic { id: 0x9E },
        LOAD_FLAGS_EVENT_BATTLE,
        SceneOp::StartBattle { index: 0x02 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$007D`, `Event_FractOozeFound`, retail `$072B2C..$072B51`.
pub static FRACT_OOZE_FOUND: Scene = Scene {
    name: "Event_FractOozeFound",
    event: EventIndex(0x007D),
    ops: &[
        retained(0x2F),
        set_flag(0x2F),
        LOAD_FLAGS_EVENT_BATTLE,
        SceneOp::StartBattle { index: 0x07 },
        SceneOp::Return { value: 1 },
    ],
};

/// `$0088`, `Event_KingRappy`, retail `$072FDC..$073017`: the first visit
/// (`$AE` clear) only talks; once `$AE` is set the fight starts.
pub static KING_RAPPY: Scene = Scene {
    name: "Event_KingRappy",
    event: EventIndex(0x0088),
    ops: &[
        SceneOp::BranchFlag {
            flag: Flag::event(0xAE),
            if_set: 1,
            if_clear: 6,
        },
        retained(0x36),
        set_flag(0xAF),
        LOAD_FLAGS_EVENT_BATTLE,
        SceneOp::StartBattle { index: 0x13 },
        SceneOp::Return { value: 1 },
        standard(0x35),
    ],
};

/// `$008F`, `Event_DaughterTerminal`, retail `$0731B2..$0731D9`.
pub static DAUGHTER_TERMINAL: Scene = Scene {
    name: "Event_DaughterTerminal",
    event: EventIndex(0x008F),
    ops: &[
        retained(0x04),
        set_flag(0xB6),
        LOAD_FLAGS_EVENT_BATTLE,
        SceneOp::StartBattle { index: 0x15 },
        SceneOp::Return { value: 1 },
    ],
};
