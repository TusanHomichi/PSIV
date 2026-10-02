//! The final-state digest: one line that a replay must reproduce.
//!
//! It reads only the runtime's own views — the map, the standing cell and
//! facing, the whole persistent [`psiv_core::StateSnapshot`] (flags, inventory,
//! the eleven character records, party, purse) — so two runs that agree on it
//! agree on everything the game would save and show at rest.

use psiv_runtime::Runtime;

use crate::tape::fnv1a64;

/// A state digest and the text it hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    /// The canonical text of the state.
    pub text: String,
    /// FNV-1a 64 of [`Digest::text`].
    pub hash: u64,
}

impl Digest {
    /// The digest of `runtime` after `frames` frames.
    #[must_use]
    pub fn of(runtime: &Runtime, frames: u64) -> Digest {
        let state = runtime.state();
        let text = format!(
            "frames={frames} map={:#x} cell=({},{}) facing={:?} money={} party={:?} snapshot={:?}",
            runtime.map_id().0,
            state.cell().x,
            state.cell().y,
            state.facing(),
            runtime.game().money(),
            runtime.game().party_members(),
            runtime.game().snapshot(),
        );
        let hash = fnv1a64(text.as_bytes());
        Digest { text, hash }
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.hash)
    }
}
