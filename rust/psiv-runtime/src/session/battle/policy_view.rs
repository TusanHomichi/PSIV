//! The battle data a player policy reads, read-only.
//!
//! A campaign policy values a command with the engine's own formulas on the
//! records the engine reads: technique, skill and item records, the weapons'
//! kinds and elements, and an enemy's pack record (its element factors and its
//! regular ability slots). This is that one borrow; nothing here changes a rule
//! or the battle's state.

use crate::Runtime;

impl Runtime {
    /// The battle records the engine reads, when battles are armed.
    #[must_use]
    pub fn battle_data(&self) -> Option<&psiv_core::battle::BattleData> {
        self.battles.as_ref().map(|set| &set.data)
    }
}
