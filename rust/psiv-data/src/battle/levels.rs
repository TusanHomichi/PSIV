//! `battle/levels.json`: the per-character level tables.

use super::NamedId;
use serde::{Deserialize, Serialize};

/// `battle/levels.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelsFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many characters have tables.
    pub character_count: u32,
    /// How many level records there are in total.
    pub total_records: u32,
    /// One table per character, in `CharLevelTablePtrs` order.
    pub characters: Vec<LevelTable>,
}

/// One character's level table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTable {
    /// Index into `Character_Stats`.
    pub character_id: u8,
    /// The character's name.
    pub character: String,
    /// The `[starting level .w]` half of the pointer-table entry, subtracted
    /// from the current level before indexing [`LevelTable::levels`].
    pub starting_level: u16,
    /// The records, in level order.
    pub levels: Vec<Level>,
}

/// One 22-byte level record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    /// The level this record grants.
    pub level: u16,
    /// Total experience needed to reach it.
    pub experience_required: u32,
    /// New maximum HP.
    pub hp: u16,
    /// New maximum TP.
    pub tp: u16,
    /// New base stats.
    pub stats: LevelStats,
    /// A technique appended at this level, if any.
    #[serde(default)]
    pub new_technique: Option<NamedId>,
    /// A skill appended at this level, if any.
    #[serde(default)]
    pub new_skill: Option<NamedId>,
    /// Maximum uses for each of the eight learned-skill positions.
    pub skill_uses: [u8; 8],
}

/// The four base stats a level record sets.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelStats {
    pub strength: u8,
    pub mental: u8,
    pub agility: u8,
    pub dexterity: u8,
}
