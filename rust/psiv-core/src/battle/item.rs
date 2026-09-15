//! Inventory/equipment activation: Character_DoItem and CharItem_ItemObjOffs.
//! Gameplay uses the item's power bytes, never the user's mental stat.

use super::technique::{in_range, stat};
use super::{
    BattleData, BattleEvent, FighterId, Rolls, Roster, Side, Stats, TechniqueStat, Verdict,
};
use super::{calc_healing, calculate_chances, calculate_damage, clamp_damage, status};
use crate::Inventory;

#[cfg(test)]
#[path = "item_tests.rs"]
mod tests;

/// Which concrete copy was selected. Inventory slots are shared by the party.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemSource {
    /// Raw zero-based slot in the forty-byte inventory, including holes.
    Inventory(u8),
    /// Raw equipment slot on the acting fighter (right, left, head, body).
    Equipment(u8),
}

/// An item's activation data, separate from its equipment bonuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleItem {
    /// Cartridge item id.
    pub id: u8,
    /// Display name.
    pub name: String,
    /// AbilityEffectsOffs id, byte 0.
    pub effect: u8,
    /// Literal power stat, byte 1. This is not a character stat selector.
    pub actor_power: u8,
    /// Target range, byte 2's low nibble.
    pub targeting: u8,
    /// Damage/healing power or miss threshold, byte 3.
    pub power: u8,
    /// Target stat selector, byte 4.
    pub resistance: u8,
    /// Target resistance element, byte 5.
    pub element: u8,
    /// CharItem_ItemObjOffs dispatcher, byte 7.
    pub object: u8,
    /// Inventory type byte 10 equals 8 (disposable).
    pub consumable: bool,
}

impl BattleItem {
    /// Only combinations whose object-side gameplay is transcribed are enabled.
    #[must_use]
    pub const fn supported(&self) -> bool {
        self.resistance <= 7
            && self.element <= 14
            && matches!(
                (self.object, self.effect, self.targeting),
                (1 | 13 | 14, 18, 4)
                    | (7 | 18, 18, 5)
                    | (2, 39, 2)
                    | (3 | 4 | 5 | 12 | 21, 1, 1)
                    | (8 | 22, 1, 2)
                    | (6 | 10, 7, 2)
                    | (9, 12, 3)
                    | (11, 9, 3)
                    | (20, 10, 3)
                    | (15, 19, 4)
                    | (16, 20, 4)
                    | (17, 21, 4)
                    | (19, 22, 4)
                    | (23, 22, 6)
            )
    }

    /// Whether selecting this item requires a target cursor.
    #[must_use]
    pub const fn single_target(&self) -> bool {
        matches!(self.targeting, 1 | 4 | 6 | 8)
    }
}

/// Rejection before any consumption or rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemRejection {
    /// Missing definition, unimplemented dispatcher, or vehicle battle.
    Unavailable,
    /// The selected copy is no longer present in the named source slot.
    Missing,
    /// The target does not belong to this item's range.
    InvalidTarget,
}

/// Menu recipients. Party targets include downed fighters for revival items.
#[must_use]
pub fn item_targets(roster: &Roster, actor: FighterId, item: &BattleItem) -> Vec<FighterId> {
    roster
        .iter()
        .filter(|f| {
            in_range(&f.stats, f.id, actor, item.targeting)
                && (f.id.side() == actor.side() || f.is_alive())
        })
        .map(|f| f.id)
        .collect()
}

pub(super) fn resolve_item(
    roster: &mut Roster,
    inventory: &mut Inventory,
    actor: FighterId,
    command: (u8, ItemSource, Option<FighterId>),
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Vec<FighterId> {
    let (id, source, intended) = command;
    let Some(caster) = roster.get(actor) else {
        return Vec::new();
    };
    let held = match source {
        ItemSource::Inventory(slot) => inventory.get(usize::from(slot)),
        ItemSource::Equipment(slot) => caster
            .stats
            .equipment
            .get(usize::from(slot))
            .copied()
            .filter(|id| *id != 0),
    };
    let definition = data.battle_item(id);
    let opens_zio_barrier = definition.is_some_and(|item| item.object == 2 && item.effect == 39)
        && roster
            .get(FighterId::new(6).expect("first enemy slot"))
            .is_some_and(|f| f.stats.enemy_id == 139);
    let rejection = match definition {
        None => Some(ItemRejection::Unavailable),
        Some(item) if actor.side() != Side::Party || !item.supported() => {
            Some(ItemRejection::Unavailable)
        }
        Some(_)
            if opens_zio_barrier
                && roster.side(Side::Enemy).any(|f| {
                    data.enemy(if f.id.get() == 6 {
                        140
                    } else {
                        f.stats.enemy_id
                    })
                    .is_err()
                }) =>
        {
            Some(ItemRejection::Unavailable)
        }
        Some(_) if held != Some(id) => Some(ItemRejection::Missing),
        Some(item) if item.consumable && matches!(source, ItemSource::Equipment(_)) => {
            Some(ItemRejection::Unavailable)
        }
        Some(item)
            if item.single_target()
                && !intended
                    .and_then(|t| roster.get(t))
                    .is_some_and(|f| in_range(&f.stats, f.id, actor, item.targeting)) =>
        {
            Some(ItemRejection::InvalidTarget)
        }
        _ => None,
    };
    if let Some(reason) = rejection {
        events.push(BattleEvent::ItemRejected {
            actor,
            item: id,
            reason,
        });
        return Vec::new();
    }
    let item = definition.expect("validated");
    if item.consumable
        && let ItemSource::Inventory(slot) = source
    {
        inventory.remove(usize::from(slot));
    }
    events.push(BattleEvent::ItemUsed {
        actor,
        item: id,
        name: item.name.clone(),
        consumed: item.consumable,
    });
    let mut targets = item_targets(roster, actor, item);
    if item.single_target() {
        let target = intended.filter(|t| targets.contains(t)).or_else(|| {
            (item.targeting == 1)
                .then(|| targets.first().copied())
                .flatten()
        });
        targets = target.into_iter().collect();
    }
    let mut died = Vec::new();
    for target in targets {
        let stats = &mut roster.get_mut(target).expect("listed").stats;
        let before = stats.clone();
        // Ability_ProcessRange: MoonDew requires death, SolDew/RepairKit allow
        // either state, and other effects exclude downed targets.
        let eligible = match item.effect {
            21 => stats.is_out(),
            22 => true,
            _ => !stats.is_out(),
        };
        // ItemObj_StatUp refuses androids for SwiftHelm/PowShield. The separate
        // MahlayShld dispatcher permits them.
        if !eligible || (matches!(item.object, 9 | 11) && stats.is_android()) {
            events.push(BattleEvent::ItemIneffective { actor, target });
            continue;
        }
        match item.effect {
            1 => {
                let amount = clamp_damage(calculate_damage(
                    u16::from(item.actor_power),
                    stat(stats, item.resistance),
                    u16::from(stats.element_factor(item.element).unwrap_or(0)),
                    u16::from(item.power),
                    rolls,
                ));
                stats.curr_hp = stats.curr_hp.saturating_sub(amount);
                events.push(BattleEvent::Resolved {
                    actor,
                    target,
                    verdict: Verdict::Normal,
                    damage: Some(amount),
                    remaining_hp: stats.curr_hp,
                });
                if stats.curr_hp == 0 {
                    stats.status |= status::DEAD;
                    died.push(target);
                    events.push(BattleEvent::Died { fighter: target });
                }
            }
            7 => {
                if stats.status & (status::ASLEEP | status::PARALYZED) == 0
                    && effect_lands(stats, item, rolls)
                {
                    // Both DreamRod and MoonSlashr use loc_3D212: set sleep,
                    // leave agility untouched, as with Earth.
                    stats.status |= status::ASLEEP;
                    events.push(BattleEvent::FellAsleep { actor, target });
                }
            }
            9 | 10 | 12 => {
                if effect_lands(stats, item, rolls) {
                    let (stat, value) = match item.effect {
                        9 => {
                            stats.attack.battle = stats
                                .attack
                                .derived
                                .wrapping_add(u16::from(item.actor_power));
                            (TechniqueStat::Attack, stats.attack.battle)
                        }
                        10 => {
                            stats.defence.battle = stats
                                .defence
                                .derived
                                .wrapping_add(u16::from(item.actor_power));
                            (TechniqueStat::Defence, stats.defence.battle)
                        }
                        _ => {
                            stats.agility.battle =
                                stats.agility.modified.wrapping_add(item.actor_power);
                            (TechniqueStat::Agility, u16::from(stats.agility.battle))
                        }
                    };
                    events.push(BattleEvent::StatChanged {
                        actor,
                        target,
                        stat,
                        value,
                    });
                }
            }
            18 => {
                let amount =
                    calc_healing(u16::from(item.actor_power), u16::from(item.power), rolls);
                stats.curr_hp = stats.curr_hp.saturating_add(amount).min(stats.max_hp);
            }
            19..=22 => {
                // Retail cures also reset AGI/DEX. Preserve this documented
                // quirk; do not erase other buffs or TP/skill resources.
                stats.agility.battle = stats.agility.modified;
                stats.dexterity.battle = stats.dexterity.modified;
                match item.object {
                    15 => stats.status &= !status::POISONED,
                    16 => stats.status &= !status::PARALYZED,
                    17 | 19 => {
                        if stats.is_out() {
                            // $80 is a transient sprite flag, represented by
                            // Revived in the port rather than persisted.
                            stats.status &= status::TECH_SEALED;
                        }
                        stats.curr_hp = if item.object == 17 {
                            (stats.max_hp / 4).max(1)
                        } else {
                            stats.max_hp
                        };
                    }
                    23 => {
                        stats.status &= !status::ANDROID_DEAD;
                        stats.curr_hp = stats.max_hp;
                    }
                    _ => unreachable!("validated restorative object"),
                }
            }
            39 => {
                // PsycoWand removes the enemy's special stat/resistance boosts.
                stats.element_props = stats.element_shadow;
                stats.physical_prop_save = stats.element_props[0];
                stats.refresh_battle_stats();
                events.push(BattleEvent::StatsRestored { actor, target });
            }
            _ => unreachable!("validated dispatcher"),
        }
        if before.is_out() && !stats.is_out() {
            events.push(BattleEvent::Revived {
                actor,
                target,
                remaining_hp: stats.curr_hp,
            });
        } else if stats.curr_hp > before.curr_hp {
            events.push(BattleEvent::Healed {
                actor,
                target,
                amount: stats.curr_hp - before.curr_hp,
                remaining_hp: stats.curr_hp,
            });
        }
        let removed = before.status & !stats.status;
        if removed != 0 {
            events.push(BattleEvent::StatusRestored {
                actor,
                target,
                removed,
            });
        }
        if matches!(item.effect, 19..=22)
            && (before.agility.battle != stats.agility.battle
                || before.dexterity.battle != stats.dexterity.battle)
        {
            events.push(BattleEvent::StatsRestored { actor, target });
        }
        if *stats == before && !matches!(item.effect, 1 | 9 | 10 | 12 | 39) {
            events.push(BattleEvent::ItemIneffective { actor, target });
        }
    }
    if opens_zio_barrier {
        // BattleObj_PsycoWand loc_3CF60 changes the FIRST formation entry
        // only if it is enemy $8B, then loc_7F22 reloads every enemy's stats.
        // Keep object occupancy and the queued turn: this is not Fission or
        // resurrection, and the new Zio can still act later this round.
        for fighter in roster.iter_mut().filter(|f| f.id.side() == Side::Enemy) {
            let enemy_id = if fighter.id.get() == 6 {
                140
            } else {
                fighter.stats.enemy_id
            };
            let record = data.enemy(enemy_id).expect("validated reload records");
            let active = fighter.is_alive();
            fighter.stats = Stats::from_enemy(record);
            fighter.name = record.name.clone();
            fighter.active = active;
            events.push(BattleEvent::EnemyStatsReloaded {
                actor,
                fighter: fighter.id,
                enemy_id,
                name: record.name.clone(),
                hp: record.hp,
            });
        }
    }
    died
}

fn effect_lands(stats: &Stats, item: &BattleItem, rolls: &mut impl Rolls) -> bool {
    item.resistance == 0
        || calculate_chances(
            i16::from(item.actor_power),
            stat(stats, item.resistance) as i16,
            i16::from(stats.element_factor(item.element).unwrap_or(0)),
            i16::from(item.power),
            i16::from(item.effect),
            rolls,
        ) != Verdict::Miss
}
