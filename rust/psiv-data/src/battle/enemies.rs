//! `battle/enemies.json`: the 48-byte `EnemyData` records.

use super::{NamedId, Property};
use serde::{Deserialize, Serialize};

/// `battle/enemies.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemiesFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many records the file declares.
    pub count: u32,
    /// The element-property slot names, in record order.
    #[serde(default)]
    pub properties: Vec<String>,
    /// The records, in id order.
    pub enemies: Vec<Enemy>,
}

/// One 48-byte `EnemyData` record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enemy {
    /// Index into `EnemyData`.
    pub id: u16,
    /// The disassembly's identifier, which disambiguates duplicate display
    /// names.
    pub symbol: String,
    /// The cartridge's own name, when it has one.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Starting and maximum HP.
    pub hp: u16,
    /// The seven combat stats.
    pub stats: EnemyStats,
    /// What a plain attack from this enemy carries and inflicts.
    pub attack: EnemyAttack,
    /// Resistance to each element, keyed by slot name. Values are 0 (immune)
    /// through 4 (very weak); the damage pipeline multiplies by this and
    /// divides by four.
    pub properties: std::collections::BTreeMap<String, Property>,
    /// The AI's ability lists.
    pub ai: EnemyAi,
    /// What it leaves behind.
    pub rewards: Rewards,
}

/// An enemy's combat stats. `defense` and `magic_defense` keep the pack's
/// spelling.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyStats {
    pub strength: u8,
    pub mental: u8,
    pub agility: u8,
    pub dexterity: u8,
    pub attack: u16,
    pub defense: u16,
    pub magic_defense: u16,
}

/// The two bytes `Battle_SetupEnemyData` reuses the TP slots for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAttack {
    /// What element a plain attack carries.
    pub element: NamedId,
    /// What status it inflicts, `0` for none.
    pub status_effect: NamedId,
}

/// `$50`..`$5F` of the in-battle stats struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAi {
    /// Four condition ids, `0` terminating.
    pub condition_ids: Vec<u8>,
    /// The ability each condition substitutes when it fires.
    pub conditional_ability_ids: Vec<u8>,
    /// The eight the per-turn roll picks between.
    pub regular_ability_ids: Vec<u8>,
}

/// What an enemy contributes to the victory pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewards {
    /// Added to `$FFFF41CE`.
    pub experience: u16,
    /// Added to `$FFFF41D0`.
    pub meseta: u16,
}
