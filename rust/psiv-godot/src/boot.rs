//! Field boot data and the flag view consumed by presentation widgets.

use std::path::Path;

use psiv_core::StepFrames;
use psiv_data::GameData;
use psiv_runtime::Runtime;

use crate::input::requested_save_slot;

/// Fallback spawn when the pack predates game-start extraction.
pub(crate) const FALLBACK_SPAWN_MAP: u16 = 0x010;
pub(crate) const FALLBACK_SPAWN_CELL: (u16, u16) = (31, 8);

pub(crate) fn collect_event_flags(rt: &Runtime) -> Vec<bool> {
    (0..512u16)
        .map(|id| rt.game().is_set(psiv_core::Flag::event(id)))
        .collect()
}

/// Title-screen bypasses used by save/debug fix loops. These are deliberately
/// checked before the title node is built: a screenshot or a battle/camp/shop
/// selector must still reach the surface it was written to inspect.
pub(crate) fn title_bypassed() -> bool {
    let title_shot = std::env::var("PSIV_DEBUG_TITLE_SHOT").is_ok_and(|value| value == "1");
    requested_save_slot().is_some()
        || std::env::var_os("PSIV_DEBUG_BATTLE").is_some()
        || std::env::args().any(|argument| argument.starts_with("--psiv-debug-battle="))
        || std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1")
        || std::env::var_os("PSIV_DEBUG_SHOP").is_some()
        || (std::env::var_os("PSIV_DEBUG_SHOT").is_some() && !title_shot)
}

/// Valid retail-shaped slots visible to the title menu. Loading is the same
/// checksum/slot validation the continue path uses; no save bytes are
/// written, repaired, or reserialized here.
pub(crate) fn available_save_slots(data: &GameData, directory: &Path) -> [bool; 3] {
    std::array::from_fn(|slot| {
        Runtime::load_slot(data.clone(), directory, slot, StepFrames::default()).is_ok()
    })
}
