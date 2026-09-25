//! The map's own scalars: what plays on arrival and its four per-map flags.
//!
//! These are the bytes map load stores rather than content it places; the accessors
//! spell the flag tests out so a caller does not compare zeroes.

use serde::{Deserialize, Serialize};

/// The map's music. Id 0 means "keep playing whatever is playing", which is
/// what `changes_music` records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Music {
    /// The music id as the map record stores it.
    pub id: u8,
    /// The track's symbol, such as `MotabiaTown`.
    #[serde(default)]
    pub symbol: Option<String>,
    /// False when the id is 0 and the current track keeps playing.
    pub changes_music: bool,
}

/// The four per-map flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    /// Non-zero on maps that drain HP as the party walks.
    pub poison: u8,
    /// Non-zero where random encounters roll.
    pub random_battles: u8,
    /// Non-zero blocks town teleport. Zero still requires no dungeon exit.
    pub town_teleport: u8,
    /// Which dungeon-teleport destination this map belongs to.
    ///
    /// `None` when the stored byte has bit 7 set, which the extractor reads as
    /// an inherited index: map load skips the store. Nine retail maps do this: the
    /// eight `ValleyMaze*` parts and `Passageway`.
    #[serde(default)]
    pub dungeon_teleport_index: Option<u8>,
}

impl Flags {
    /// Does walking here cost HP?
    pub fn poisons(&self) -> bool {
        self.poison != 0
    }

    /// Do random encounters roll here?
    pub fn rolls_random_battles(&self) -> bool {
        self.random_battles != 0
    }

    /// May the town-teleport technique be used here?
    pub fn allows_town_teleport(&self) -> bool {
        self.town_teleport == 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_helpers_read_the_bytes() {
        let flags = Flags {
            poison: 0,
            random_battles: 1,
            town_teleport: 0,
            dungeon_teleport_index: Some(3),
        };
        assert!(!flags.poisons());
        assert!(flags.rolls_random_battles());
        assert!(flags.allows_town_teleport());
    }
}
