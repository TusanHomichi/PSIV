//! The board: what a player sees when a member's command window opens.
//!
//! [`Board::read`] builds it from the open command window
//! ([`CommandMenuView`]) and the runtime's read-only battle surface
//! (`battle_roster`, `battle_data`, the pack). The decision rules in
//! [`crate::policy_plan`] read only a [`Board`], so tests construct one directly.
//!
//! What it holds:
//!
//! * every fighter's battle stats, element factors and status, as the engine
//!   reads them (the live roster's `Stats`, an enemy's built from its pack
//!   record, `Stats::from_enemy`);
//! * every action the actor can take now, each with the record bytes its
//!   formula reads and the target list the command menu offers (the core's
//!   `technique_targets`, `skill_targets` and `item_targets`): techniques the
//!   window enables (supported, TP enough, not sealed), skills with uses left
//!   (and a weapon when the record wants one), and the battle items the pack
//!   and the actor's hands hold;
//! * the actor's weapon: whether it reaches every enemy and the elements its
//!   hands carry (shields skipped, `Character_DamageEnemy`'s larger-of-two-hands
//!   rule, `ps4.asm:3910`).
//!
//! Nothing here writes to the runtime.

use psiv_core::battle::{
    BattleData, EnemyRecord, Fighter, FighterId, ItemKind, Reach, Roster, Side, Stats,
    item_targets, skill_targets, status, technique_targets, weapon_reach,
};
use psiv_runtime::{CommandMenuView, Runtime};

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

/// The actor's weapon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Weapon {
    /// The element ids the hands carry (shields skipped).
    pub elements: Vec<u8>,
    /// A swing reaches every enemy (weapon types 2 and 4).
    pub all: bool,
}

/// What the actor sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// The acting fighter id.
    pub actor: u8,
    /// Every party fighter, the fallen included.
    pub party: Vec<Combatant>,
    /// The living enemies, in fighter-id order.
    pub enemies: Vec<Combatant>,
    /// Every action besides ATTACK and DEFEND the actor can take now.
    pub abilities: Vec<Ability>,
    /// The actor's weapon, or `None` when ATTACK is not offered.
    pub weapon: Option<Weapon>,
    /// Copies the pack holds of each consumable the actor can use.
    pub stock: Vec<(u8, u16)>,
}

impl Board {
    /// The actor.
    #[must_use]
    pub fn me(&self) -> Option<&Combatant> {
        self.party.iter().find(|m| m.id == self.actor)
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

    /// The board of the open window, or `None` outside a battle with a roster.
    #[must_use]
    pub fn read(menu: &CommandMenuView, runtime: &Runtime) -> Option<Board> {
        let actor = FighterId::new(menu.actor?)?;
        let roster = runtime.battle_roster()?;
        let data = runtime.battle_data()?;
        let me = roster.get(actor)?;
        let party = roster
            .side(Side::Party)
            .map(|f| Combatant::from_fighter(f, None))
            .collect();
        let enemies = roster
            .living(Side::Enemy)
            .map(|f| Combatant::from_fighter(f, data.enemy(f.stats.enemy_id).ok()))
            .collect();
        let mine = Combatant::from_fighter(me, None);
        let mut abilities = Vec::new();
        techniques(menu, roster, data, actor, &mine, &mut abilities);
        skills(menu, roster, data, actor, &mine, &mut abilities);
        items(runtime, roster, data, actor, &me.stats, &mut abilities);
        let held = runtime.game().inventory();
        let stock = abilities
            .iter()
            .filter_map(|a| match a.source {
                Source::Item { id, consumable: true, .. } => Some((
                    id,
                    u16::try_from((0..40).filter(|slot| held.get(*slot) == Some(id)).count())
                        .unwrap_or(u16::MAX),
                )),
                _ => None,
            })
            .collect();
        Some(Board {
            actor: actor.get(),
            party,
            enemies,
            abilities,
            weapon: weapon(&me.stats, data),
            stock,
        })
    }
}

fn ids(targets: &[FighterId]) -> Vec<u8> {
    targets.iter().map(FighterId::get).collect()
}

fn techniques(
    menu: &CommandMenuView,
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Combatant,
    out: &mut Vec<Ability>,
) {
    for entry in menu.techniques.iter().filter(|e| e.available) {
        let Some(tech) = data.technique(entry.id) else {
            continue;
        };
        out.push(Ability {
            source: Source::Technique(tech.id),
            effect: tech.effect,
            range: tech.targeting & 15,
            // `resolve_technique`: the caster's mental, always.
            power_stat: me.mental,
            power: tech.power,
            resistance: tech.resistance,
            element: tech.element,
            tp_cost: tech.cost,
            targets: ids(&technique_targets(roster, actor, tech)),
        });
    }
}

fn skills(
    menu: &CommandMenuView,
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Combatant,
    out: &mut Vec<Ability>,
) {
    for entry in menu
        .skills
        .iter()
        .filter(|e| e.available && e.remaining > 0)
    {
        let Some(skill) = data.skill(entry.id) else {
            continue;
        };
        out.push(Ability {
            source: Source::Skill(skill.id),
            effect: skill.effect,
            range: skill.targeting & 15,
            // `resolve_skill`: the record's selected stat (VISION's fixed 8
            // has no damage or healing use here).
            power_stat: me.stat(skill.power_stat),
            power: skill.power,
            resistance: skill.resistance,
            element: skill.element,
            tp_cost: 0,
            targets: ids(&skill_targets(roster, actor, skill)),
        });
    }
}

fn items(
    runtime: &Runtime,
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Stats,
    out: &mut Vec<Ability>,
) {
    let held = runtime.game().inventory();
    let mut seen = Vec::new();
    let hands = me.equipment.iter().copied();
    let pack = (0..40).filter_map(|slot| held.get(slot));
    for id in hands.chain(pack).filter(|id| *id != 0) {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let Some(item) = data.battle_item(id) else {
            continue;
        };
        // A consumable is used from the pack only (`resolve_item` refuses an
        // equipped copy); anything else from a hand or the pack.
        if !item.supported() || (item.consumable && !held.contains(id)) {
            continue;
        }
        out.push(Ability {
            source: Source::Item {
                id,
                name: item.name.clone(),
                consumable: item.consumable,
            },
            effect: item.effect,
            range: item.targeting,
            power_stat: item.actor_power.into(),
            power: item.power,
            resistance: item.resistance,
            element: item.element,
            tp_cost: 0,
            targets: ids(&item_targets(roster, actor, item)),
        });
    }
}

fn weapon(me: &Stats, data: &BattleData) -> Option<Weapon> {
    let reach = weapon_reach(me, data).ok().flatten()?;
    let elements = me.equipment[..2]
        .iter()
        .filter(|id| **id != 0)
        .filter_map(|id| data.item(*id).ok())
        .filter(|item| item.kind != ItemKind::Shield)
        .map(|item| item.element)
        .collect();
    Some(Weapon {
        elements,
        all: reach == Reach::All,
    })
}
