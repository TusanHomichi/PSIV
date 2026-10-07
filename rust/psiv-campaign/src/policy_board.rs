//! The board: what a player sees when a member's command window opens.
//!
//! `Board::read` (`policy_board_read.rs`) builds it from the open command
//! window and the runtime's read-only battle surface (`battle_roster`,
//! `battle_data`, the pack). The decision rules in
//! [`crate::policy_plan`] read only a [`Board`], so tests construct one directly.
//!
//! What it holds:
//!
//! * every fighter's battle stats, element factors and status, as the engine
//!   reads them (the live roster's `Stats`, an enemy's built from its pack
//!   record, `Stats::from_enemy`);
//! * for every living member, a [`Kit`]: every action they can take this round,
//!   each with the record bytes its formula reads and the target list the
//!   command menu offers, and their weapon (whether a swing reaches every
//!   enemy, and the elements the hands carry).
//!
//! Nothing here writes to the runtime.

use psiv_core::battle::{EnemyRecord, Fighter, Side, status};

use crate::policy_estimate::EffectClass;

/// One fighter as the policy sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combatant {
    /// Retail fighter id: 1-5 the party, 6-9 the enemies.
    pub id: u8,
    /// Display name, for the trace.
    pub name: String,
    /// Current HP.
    pub hp: u16,
    /// Maximum HP.
    pub max_hp: u16,
    /// Current TP (party only).
    pub tp: u16,
    /// Status bits (`psiv_core::battle::status`).
    pub status: u8,
    /// Whether this is an android (healing techniques pass it by).
    pub android: bool,
    /// Live strength, mental, agility and dexterity.
    pub strength: u16,
    /// Live mental.
    pub mental: u16,
    /// Live agility.
    pub agility: u16,
    /// Live dexterity.
    pub dexterity: u16,
    /// Live attack power, and its unbuffed value.
    pub attack: (u16, u16),
    /// Live defence, and its unbuffed value.
    pub defence: (u16, u16),
    /// Live mental defence, and its unbuffed value.
    pub mental_defence: (u16, u16),
    /// The factor against each element id 1..=14: 0 immune, 2 normal, 4 very
    /// weak (`physical_prop` `$30` to `destroy_prop` `$4A`,
    /// `ps4.constants.asm:36-49`).
    pub elements: [u8; 14],
    /// An enemy's plain-attack element (its record's `curr_tp` slot).
    pub attack_element: u8,
    /// How many of an enemy's eight regular ability slots are a plain attack.
    pub plain_attacks: u8,
}

impl Combatant {
    /// A blank fighter: every stat zero, every element factor normal.
    #[must_use]
    pub fn new(id: u8, hp: u16, max_hp: u16) -> Combatant {
        Combatant {
            id,
            name: format!("F{id}"),
            hp,
            max_hp,
            tp: 0,
            status: 0,
            android: false,
            strength: 0,
            mental: 0,
            agility: 0,
            dexterity: 0,
            attack: (0, 0),
            defence: (0, 0),
            mental_defence: (0, 0),
            elements: [2; 14],
            attack_element: 1,
            plain_attacks: 8,
        }
    }

    /// From a live roster fighter; `record` is an enemy's pack record.
    #[must_use]
    pub fn from_fighter(fighter: &Fighter, record: Option<&EnemyRecord>) -> Combatant {
        let s = &fighter.stats;
        Combatant {
            id: fighter.id.get(),
            name: fighter.name.clone(),
            hp: s.curr_hp,
            max_hp: s.max_hp,
            tp: if fighter.id.side() == Side::Party {
                s.curr_tp
            } else {
                0
            },
            status: s.status,
            android: s.is_android(),
            strength: s.strength.battle.into(),
            mental: s.mental.battle.into(),
            agility: s.agility.battle.into(),
            dexterity: s.dexterity.battle.into(),
            attack: (s.attack.battle, s.attack.derived),
            defence: (s.defence.battle, s.defence.derived),
            mental_defence: (s.mental_defence.battle, s.mental_defence.derived),
            elements: s.element_props,
            attack_element: record.map_or(0, |r| r.attack_element),
            plain_attacks: record.map_or(0, |r| {
                u8::try_from(r.regular_abilities.iter().filter(|a| **a == 0).count()).unwrap_or(0)
            }),
        }
    }

    /// Standing: HP above zero and neither dead bit set.
    #[must_use]
    pub const fn alive(&self) -> bool {
        self.hp > 0 && self.status & status::OUT == 0
    }

    /// The cartridge's stat selector (`technique::stat`, the `loc_275A` table,
    /// `ps4.asm:3797-3812`): 1 strength .. 7 mental defence.
    #[must_use]
    pub const fn stat(&self, selector: u8) -> u16 {
        match selector {
            1 => self.strength,
            2 => self.mental,
            3 => self.agility,
            4 => self.dexterity,
            5 => self.attack.0,
            6 => self.defence.0,
            7 => self.mental_defence.0,
            _ => 0,
        }
    }

    /// The factor against element `element` (`$2E + 2 * id`), or 0 for an id
    /// that selects nothing, as `Stats::element_factor`.
    #[must_use]
    pub fn factor(&self, element: u8) -> u16 {
        usize::from(element)
            .checked_sub(1)
            .and_then(|index| self.elements.get(index))
            .map_or(0, |f| u16::from(*f))
    }
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
    /// The element ids the hands carry (shields skipped).
    pub elements: Vec<u8>,
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
pub struct Board {
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

impl Board {
    /// The actor.
    #[must_use]
    pub fn me(&self) -> Option<&Combatant> {
        self.member(self.actor)
    }

    /// A party member by fighter id.
    #[must_use]
    pub fn member(&self, id: u8) -> Option<&Combatant> {
        self.party.iter().find(|m| m.id == id)
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
            .find(|f| f.id == id)
    }

    /// Copies of consumable item `id` in the pack; `None` for an item that is
    /// not used up.
    #[must_use]
    pub fn stock(&self, id: u8) -> Option<u16> {
        self.stock.iter().find(|(item, _)| *item == id).map(|(_, n)| *n)
    }
}
