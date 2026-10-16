//! The board: what a player sees when a member's command window opens.
//!
//! `Board::read` (`policy_board_read.rs`) builds it from the open command
//! window and the runtime's read-only battle surface (`battle_roster`,
//! `battle_data`, the pack). The decision rules in
//! [`crate::policy_plan`] read only a [`Board`], so tests construct one directly.
//!
//! What it holds:
//!
//! * every fighter as the engine holds it (the live roster's `Fighter`), read
//!   through the core's own rules: `Fighter::is_alive`, the stat selector
//!   `stat`, and the element factors `ability_element_factor`,
//!   `character_element_factor` and `enemy_element_factor` over the battle
//!   records the board borrows;
//! * for every living member, a [`Kit`]: every action they can take this round,
//!   each with the record bytes its formula reads and the target list the
//!   command menu offers, and whether their swing reaches every enemy.
//!
//! Nothing here writes to the runtime.

use psiv_core::battle::{
    BattleData, EnemyRecord, Fighter, Stats, ability_element_factor, character_element_factor,
    enemy_element_factor, stat, status,
};

use crate::policy_estimate::EffectClass;

/// One fighter as the policy sees it: the engine's own [`Fighter`] (id, name,
/// live stats, whether its slot is occupied), read through the core's rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combatant {
    /// The fighter as the roster holds it.
    pub fighter: Fighter,
    /// How many of an enemy's eight regular ability slots are a plain attack.
    pub plain_attacks: u8,
}

impl Combatant {
    /// From a live roster fighter; `record` is an enemy's pack record.
    #[must_use]
    pub fn from_fighter(fighter: &Fighter, record: Option<&EnemyRecord>) -> Combatant {
        Combatant {
            fighter: fighter.clone(),
            plain_attacks: record.map_or(0, |r| {
                u8::try_from(r.regular_abilities.iter().filter(|a| **a == 0).count()).unwrap_or(0)
            }),
        }
    }

    /// Retail fighter id: 1-5 the party, 6-9 the enemies.
    #[must_use]
    pub const fn id(&self) -> u8 {
        self.fighter.id.get()
    }

    /// The live stats.
    #[must_use]
    pub const fn stats(&self) -> &Stats {
        &self.fighter.stats
    }

    /// Current HP.
    #[must_use]
    pub const fn hp(&self) -> u16 {
        self.fighter.stats.curr_hp
    }

    /// Maximum HP.
    #[must_use]
    pub const fn max_hp(&self) -> u16 {
        self.fighter.stats.max_hp
    }

    /// Current TP (a party member's; an enemy's slot holds its attack element).
    #[must_use]
    pub const fn tp(&self) -> u16 {
        self.fighter.stats.curr_tp
    }

    /// Status bits (`psiv_core::battle::status`).
    #[must_use]
    pub const fn status(&self) -> u8 {
        self.fighter.stats.status
    }

    /// Live attack power.
    #[must_use]
    pub const fn attack(&self) -> u16 {
        self.fighter.stats.attack.battle
    }

    /// Unbuffed attack power.
    #[must_use]
    pub const fn attack_base(&self) -> u16 {
        self.fighter.stats.attack.derived
    }

    /// Live defence.
    #[must_use]
    pub const fn defence(&self) -> u16 {
        self.fighter.stats.defence.battle
    }

    /// Unbuffed defence.
    #[must_use]
    pub const fn defence_base(&self) -> u16 {
        self.fighter.stats.defence.derived
    }

    /// Standing: the engine's own `Fighter::is_alive`.
    #[must_use]
    pub const fn alive(&self) -> bool {
        self.fighter.is_alive()
    }

    /// The stat a record's selector byte names (`psiv_core::battle::stat`).
    #[must_use]
    pub fn stat(&self, selector: u8) -> u16 {
        stat(&self.fighter.stats, selector)
    }

    /// The factor this fighter's plain attack presents against `target`
    /// (`enemy_element_factor`): an enemy's attack element.
    #[must_use]
    pub fn plain_factor(&self, target: &Combatant) -> u16 {
        enemy_element_factor(self.stats(), target.stats())
    }

    /// A copy with live attack power `attack`, for valuing a buff or debuff.
    #[must_use]
    pub fn with_attack(&self, attack: u16) -> Combatant {
        let mut c = self.clone();
        c.fighter.stats.attack.battle = attack;
        c
    }

    /// A copy with live defence `defence`, for valuing a buff.
    #[must_use]
    pub fn with_defence(&self, defence: u16) -> Combatant {
        let mut c = self.clone();
        c.fighter.stats.defence.battle = defence;
        c
    }
}

/// Whether a party member seen through a view (the field's party, the battle's
/// status strip) stands, which is [`Fighter::is_alive`] for a seated member:
/// the slot is occupied, so only `Stats::is_out`'s mask (`status::OUT`, `$44`)
/// decides.
#[must_use]
pub const fn standing(status_byte: u8) -> bool {
    status_byte & status::OUT == 0
}

/// Where an action comes from, which is how `battle.rs` steers to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// TECH, one-based technique id.
    Technique(u8),
    /// SKILL, one-based skill id.
    Skill(u8),
    /// ITEM: the item id, the row label, and whether using it spends it.
    Item {
        /// Cartridge item id.
        id: u8,
        /// The display name the ITEM page shows.
        name: String,
        /// Whether the copy is used up.
        consumable: bool,
    },
}

/// One action the actor can take, with what its formula reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ability {
    /// Where it is chosen.
    pub source: Source,
    /// The record's effect id (`AbilityEffectsOffs`).
    pub effect: u8,
    /// The target range, the targeting byte's low nibble.
    pub range: u8,
    /// The actor-side value the formula reads: mental for a technique, the
    /// record's selected stat for a skill, the item's literal byte 1.
    pub power_stat: u16,
    /// Byte 3: damage or healing power, or an effect's threshold.
    pub power: u8,
    /// Byte 4: the target stat selector.
    pub resistance: u8,
    /// Byte 5: the element; `$10` and up is the actor's weapon.
    pub element: u8,
    /// TP the command costs (techniques).
    pub tp_cost: u8,
    /// The fighters the command menu offers, in fighter-id order.
    pub targets: Vec<u8>,
}

impl Ability {
    /// The effect's class.
    #[must_use]
    pub const fn class(&self) -> EffectClass {
        EffectClass::of(self.effect)
    }

    /// Whether the command takes a target cursor.
    #[must_use]
    pub const fn single(&self) -> bool {
        matches!(self.range, 1 | 4 | 6 | 8)
    }

    /// Whether choosing it spends something a rest restores (TP, a skill use)
    /// or an item that does not come back.
    #[must_use]
    pub const fn spends(&self) -> bool {
        match self.source {
            Source::Technique(_) | Source::Skill(_) => true,
            Source::Item { consumable, .. } => consumable,
        }
    }
}

/// A member's weapon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Weapon {
    /// A swing reaches every enemy (weapon types 2 and 4).
    pub all: bool,
}

/// What one member can do this round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kit {
    /// The member's fighter id.
    pub id: u8,
    /// Every action besides ATTACK and DEFEND the member can take now.
    pub abilities: Vec<Ability>,
    /// The member's weapon, or `None` when ATTACK is not offered.
    pub weapon: Option<Weapon>,
}

/// What the player sees when a member's window opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board<'d> {
    /// The battle records the engine reads (the weapons' element factors).
    pub data: &'d BattleData,
    /// The acting fighter id.
    pub actor: u8,
    /// Every party fighter, the fallen included.
    pub party: Vec<Combatant>,
    /// The living enemies, in fighter-id order.
    pub enemies: Vec<Combatant>,
    /// What each living party member can do (`policy_board_read.rs`).
    pub kits: Vec<Kit>,
    /// Copies the pack holds of each consumable a member can use.
    pub stock: Vec<(u8, u16)>,
}

impl Board<'_> {
    /// The factor `me`'s plain attack presents against `target`: the larger of
    /// the two hands (`character_element_factor`); 0 for an unknown item.
    #[must_use]
    pub fn weapon_factor(&self, me: &Combatant, target: &Combatant) -> u16 {
        character_element_factor(me.stats(), target.stats(), self.data).unwrap_or(0)
    }

    /// The factor an ability of element `element` cast by `me` presents
    /// against `target`, the weapon fallback included (`ability_element_factor`);
    /// 0 for an unknown item.
    #[must_use]
    pub fn ability_factor(&self, me: &Combatant, target: &Combatant, element: u8) -> u16 {
        ability_element_factor(me.stats(), target.stats(), element, self.data).unwrap_or(0)
    }

    /// The actor.
    #[must_use]
    pub fn me(&self) -> Option<&Combatant> {
        self.member(self.actor)
    }

    /// A party member by fighter id.
    #[must_use]
    pub fn member(&self, id: u8) -> Option<&Combatant> {
        self.party.iter().find(|m| m.id() == id)
    }

    /// What member `id` can do.
    #[must_use]
    pub fn kit(&self, id: u8) -> Option<&Kit> {
        self.kits.iter().find(|k| k.id == id)
    }

    /// A fighter by id, on either side.
    #[must_use]
    pub fn fighter(&self, id: u8) -> Option<&Combatant> {
        self.party
            .iter()
            .chain(self.enemies.iter())
            .find(|f| f.id() == id)
    }

    /// Copies of consumable item `id` in the pack; `None` for an item that is
    /// not used up.
    #[must_use]
    pub fn stock(&self, id: u8) -> Option<u16> {
        self.stock
            .iter()
            .find(|(item, _)| *item == id)
            .map(|(_, n)| *n)
    }
}
