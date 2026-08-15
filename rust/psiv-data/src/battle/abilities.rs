//! `battle/abilities.json`: the eight-byte ability records.

use super::NamedId;
use serde::{Deserialize, Serialize};

/// `battle/abilities.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilitiesFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// What the dispatch table looks like.
    pub effects: EffectTable,
    /// Player techniques.
    pub techniques: Vec<Ability>,
    /// Player skills.
    pub skills: Vec<Ability>,
    /// Enemy skills.
    pub enemy_skills: Vec<Ability>,
    /// The eight-byte records embedded in inventory entries.
    pub item_effects: Vec<Ability>,
}

impl AbilitiesFile {
    /// Every record across all four kinds.
    pub fn all(&self) -> impl Iterator<Item = &Ability> {
        self.techniques
            .iter()
            .chain(&self.skills)
            .chain(&self.enemy_skills)
            .chain(&self.item_effects)
    }

    /// Records safe to hand to the engine.
    ///
    /// Excludes anything whose effect id runs past `AbilityEffectsOffs`, which
    /// the cartridge would dispatch through an unbounded `TRAP #2` into
    /// whatever follows the table. `docs/RUNTIME_DESIGN.md` "Battle bug policy"
    /// ratifies rejecting those rather than reproducing the jump, and this is
    /// where the rejection happens — a consumer that iterates this cannot pass
    /// one on by accident.
    pub fn usable(&self) -> impl Iterator<Item = &Ability> {
        self.all().filter(|record| !record.effect_out_of_range)
    }

    /// Records the engine must not be given, with [`Ability::identity`] ready
    /// for a log line.
    ///
    /// Retail holds exactly one — `BLACK WAVE`, effect `$2C`, carried only by
    /// the enemy `Zio3`, which appears in none of the 531 formations.
    pub fn rejected(&self) -> impl Iterator<Item = &Ability> {
        self.all().filter(|record| record.effect_out_of_range)
    }
}

/// `AbilityEffectsOffs` and the warning that goes with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectTable {
    /// The table's retail address, as a hex string.
    pub table: String,
    /// How many entries it really has: 44.
    pub count: u32,
    /// Always false — `TRAP #2` dispatches with no bound.
    pub bounds_checked: bool,
}

/// One eight-byte ability record, whichever table it came from.
///
/// The four kinds share a layout but not a vocabulary: a technique's byte 1 is
/// a TP cost while a skill's is a stat selector, so both spellings appear and
/// only one is populated per record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ability {
    /// Index into its own table.
    pub id: u16,
    /// Which table: `techniques`, `skills`, `enemy_skills` or `item_effects`.
    pub kind: String,
    /// The disassembly's identifier, for the tables that carry one.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The extractor's name, for the tables that carry one.
    #[serde(default)]
    pub name: Option<String>,
    /// The cartridge's own name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Byte 0, the index into `AbilityEffectsOffs`.
    pub effect_id: u8,
    /// Byte 1, for a technique.
    #[serde(default)]
    pub tp_cost: Option<u8>,
    /// Byte 1's low seven bits, for a skill or enemy skill: which of the
    /// actor's stats supplies the attack power.
    #[serde(default)]
    pub relevant_stat: Option<NamedId>,
    /// Byte 1's high bit, for a skill.
    #[serde(default)]
    pub requires_weapon: Option<bool>,
    /// Byte 3.
    #[serde(default)]
    pub power_or_hit_chance: Option<u16>,
    /// Byte 4: which of the target's stats resists.
    #[serde(default)]
    pub resistance_stat: Option<NamedId>,
    /// Byte 5: which element the target resists it with. `$10` and above mean
    /// "use the attacker's weapon element instead".
    #[serde(default)]
    pub element: Option<NamedId>,
    /// Whether [`Ability::effect_id`] runs past the dispatch table. Retail has
    /// exactly one: `BLACK WAVE`, used only by the unreferenced enemy `Zio3`.
    pub effect_out_of_range: bool,
}

impl Ability {
    /// The best name this record has, for an error message.
    #[must_use]
    pub fn identity(&self) -> String {
        let name = self
            .display_name
            .as_deref()
            .or(self.symbol.as_deref())
            .or(self.name.as_deref())
            .unwrap_or("?");
        format!("{} {} ({name})", self.kind, self.id)
    }
}
