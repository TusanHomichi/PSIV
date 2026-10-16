//! The battle data a player policy reads, read-only.
//!
//! A campaign policy values a command with the engine's own formulas on the
//! records the engine reads: technique, skill and item records, the weapons'
//! kinds and elements, and an enemy's pack record (its element factors and its
//! regular ability slots). It also needs what each member's command window
//! would offer, and asks the window's own rule (`menu/commands.rs`) rather than
//! restating it. Nothing here changes a rule or the battle's state.

use psiv_core::battle::{FighterId, ItemSource};

use super::menu::CommandsMenu;
use crate::Runtime;

impl Runtime {
    /// The battle records the engine reads, when battles are armed.
    #[must_use]
    pub fn battle_data(&self) -> Option<&psiv_core::battle::BattleData> {
        self.battles.as_ref().map(|set| &set.data)
    }

    /// The technique ids member `fighter`'s TECH window would enable if it
    /// opened now, in the window's order; empty outside a battle.
    #[must_use]
    pub fn window_techniques(&self, fighter: FighterId) -> Vec<u8> {
        let (Some(data), Some(member)) = (
            self.battle_data(),
            self.battle_roster().and_then(|r| r.get(fighter)),
        ) else {
            return Vec::new();
        };
        CommandsMenu::listed_techniques(&member.stats, |id| data.technique(id))
            .into_iter()
            .filter(|tech| CommandsMenu::technique_enabled(&member.stats, tech))
            .map(|tech| tech.id)
            .collect()
    }

    /// The skill ids member `fighter`'s SKILL window would enable if it opened
    /// now, in the window's order; empty outside a battle.
    #[must_use]
    pub fn window_skills(&self, fighter: FighterId) -> Vec<u8> {
        let (Some(data), Some(member)) = (
            self.battle_data(),
            self.battle_roster().and_then(|r| r.get(fighter)),
        ) else {
            return Vec::new();
        };
        let armed = self.battle_armed_fighters().contains(&fighter);
        CommandsMenu::listed_skills(&member.stats, |id| data.skill(id))
            .into_iter()
            .filter(|(slot, skill)| CommandsMenu::skill_enabled(&member.stats, *slot, skill, armed))
            .map(|(_, skill)| skill.id)
            .collect()
    }

    /// The items member `fighter`'s ITEM window would enable if it opened now,
    /// before any earlier actor reserved a pack copy: each with where it is
    /// taken from, in the window's order; empty outside a battle.
    #[must_use]
    pub fn window_items(&self, fighter: FighterId) -> Vec<(ItemSource, u8)> {
        let (Some(data), Some(member)) = (
            self.battle_data(),
            self.battle_roster().and_then(|r| r.get(fighter)),
        ) else {
            return Vec::new();
        };
        let inventory = self.game().inventory();
        CommandsMenu::listed_items(&member.stats.equipment, inventory.slots(), |id| {
            data.battle_item(id)
        })
        .into_iter()
        .filter(|(_, item)| CommandsMenu::item_enabled(item))
        .map(|(source, item)| (source, item.id))
        .collect()
    }
}
