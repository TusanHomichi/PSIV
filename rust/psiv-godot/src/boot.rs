//! Field boot data and the flag view consumed by presentation widgets.

use psiv_runtime::Runtime;

/// Fallback spawn when the pack predates game-start extraction.
pub(crate) const FALLBACK_SPAWN_MAP: u16 = 0x010;
pub(crate) const FALLBACK_SPAWN_CELL: (u16, u16) = (31, 8);

pub(crate) fn collect_event_flags(rt: &Runtime) -> Vec<bool> {
    (0..512u16)
        .map(|id| rt.game().is_set(psiv_core::Flag::event(id)))
        .collect()
}
