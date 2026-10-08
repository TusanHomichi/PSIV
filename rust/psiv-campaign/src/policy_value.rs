//! What each action is worth to the party, in 1/256 HP: the valuations
//! [`Planner`] ranks. Every damage, healing and chance number is the engine's
//! formula through [`crate::policy_estimate`].

use psiv_core::battle::status;

use super::{BUFF_ROUNDS, ENCOUNTER_SPEND_FACTOR, Pick, Planner};
use crate::policy::Intent;
use crate::policy_board::{Ability, Board, Combatant, Kit, Source};
use crate::policy_estimate::{
    EffectClass, HP, Stat, Value, attack_damage, damage, landing_chance, max_damage,
};

impl Planner {
    /// The most `member` could lose before their next turn.
    pub(super) fn threat(&self, board: &Board, member: &Combatant) -> u16 {
        let plain = board
            .enemies
            .iter()
            .filter(|e| e.plain_attacks > 0)
            .map(|e| {
                max_damage(
                    e.attack.0,
                    member.defence.0,
                    member.factor(e.attack_element),
                    0,
                )
            })
            .max()
            .unwrap_or(0);
        plain.max(self.memory.worst_loss)
    }

    /// The HP below which `member` is at risk: `percent` of their maximum, or
    /// one more than the threat when that is higher and still survivable.
    pub(super) fn line(&self, board: &Board, member: &Combatant, percent: u32) -> u16 {
        let floor = u16::try_from(u32::from(member.max_hp) * percent / 100).unwrap_or(u16::MAX);
        let threat = self.threat(board, member);
        if threat >= member.max_hp {
            floor
        } else {
            floor.max(threat.saturating_add(1))
        }
    }

    /// An enemy's HP after the damage already ordered.
    pub(super) fn left(&self, enemy: &Combatant) -> u16 {
        let ordered = self.book.damage.get(&enemy.id).copied().unwrap_or(0) / HP;
        enemy
            .hp
            .saturating_sub(u16::try_from(ordered).unwrap_or(u16::MAX))
    }

    /// Rounds the fight has left at the rate the party removes HP: what the
    /// last round took, or before any round has, every member's plain attack.
    pub(super) fn rounds_left(&self, board: &Board) -> u64 {
        let remaining: u64 = board.enemies.iter().map(|e| u64::from(self.left(e))).sum();
        let rate = if self.memory.enemy_loss > 0 {
            u64::from(self.memory.enemy_loss)
        } else {
            let swings: Value = board
                .kits
                .iter()
                .filter_map(|kit| {
                    let me = board.member(kit.id)?;
                    Some(self.attack_pick(board, kit, me)?.value)
                })
                .sum();
            swings / HP
        };
        remaining.div_ceil(rate.max(1)).max(1)
    }

    /// What an enemy does to the party in one of its turns: its plain attack
    /// on an average living member, or the last round's party loss shared out
    /// over the enemies, whichever is more.
    pub(super) fn enemy_turn(&self, board: &Board, enemy: &Combatant) -> Value {
        let living: Vec<&Combatant> = board.party.iter().filter(|m| m.alive()).collect();
        let n = living.len().max(1) as u64;
        let plain: Value = living
            .iter()
            .map(|m| {
                damage(
                    enemy.attack.0,
                    m.defence.0,
                    m.factor(enemy.attack_element),
                    0,
                    m.hp,
                )
            })
            .sum::<Value>()
            / n;
        let seen = Value::from(self.memory.party_loss) * HP / (board.enemies.len().max(1) as u64);
        plain.max(seen)
    }

    /// The member's best plain attack: every enemy for a group weapon, else the
    /// target it does most to.
    pub(super) fn attack_pick(&self, board: &Board, kit: &Kit, me: &Combatant) -> Option<Pick> {
        let weapon = kit.weapon.as_ref()?;
        let hit = |e: &Combatant| -> Value {
            let factor = weapon
                .elements
                .iter()
                .map(|el| e.factor(*el))
                .max()
                .unwrap_or(0);
            attack_damage(
                me.attack.0,
                me.dexterity,
                e.agility,
                e.defence.0,
                factor,
                weapon.all,
                self.left(e),
            )
        };
        if weapon.all {
            let each: Vec<(u8, Value)> = board.enemies.iter().map(|e| (e.id, hit(e))).collect();
            return Some(Pick {
                value: each.iter().map(|(_, v)| v).sum(),
                intent: Intent::ATTACK,
                each,
                ability: None,
            });
        }
        best_of(board.enemies.iter().map(|e| (e.id, hit(e)))).map(|(id, value)| Pick {
            value,
            intent: Intent::Attack { target: Some(id) },
            each: vec![(id, value)],
            ability: None,
        })
    }

    /// What `ability` does to enemy `e`, in HP.
    fn on_enemy(&self, board: &Board, kit: &Kit, ability: &Ability, e: &Combatant) -> Value {
        // Byte 5 at `$10` and up is the weapon's element (`loc_26F8`,
        // `ps4.asm:3814-3817`): the larger factor of the two hands.
        let factor = if ability.element >= 0x10 {
            kit.weapon.as_ref().map_or(0, |w| {
                w.elements.iter().map(|el| e.factor(*el)).max().unwrap_or(0)
            })
        } else {
            e.factor(ability.element)
        };
        let resist = e.stat(ability.resistance);
        let lands = || {
            landing_chance(
                ability.power_stat,
                resist,
                factor,
                ability.power,
                ability.effect,
                ability.resistance,
            )
        };
        match ability.class() {
            EffectClass::Damage => damage(
                ability.power_stat,
                resist,
                factor,
                ability.power.into(),
                self.left(e),
            ),
            EffectClass::Death => lands() * Value::from(self.left(e)) * HP / 64,
            EffectClass::Sleep
                if e.status & (status::ASLEEP | status::PARALYZED) == 0
                    && !self.book.slept.contains(&e.id) =>
            {
                lands() * self.enemy_turn(board, e) / 64
            }
            EffectClass::Debuff(Stat::Attack) if self.scripted && e.attack.0 >= e.attack.1 => {
                let lowered = Combatant {
                    attack: (e.attack.0.saturating_sub(ability.power_stat), e.attack.1),
                    ..e.clone()
                };
                let saved = self
                    .enemy_turn(board, e)
                    .saturating_sub(self.enemy_turn(board, &lowered))
                    * u64::from(e.plain_attacks)
                    / 8;
                lands() * saved * self.rounds_left(board).saturating_sub(1).min(BUFF_ROUNDS) / 64
            }
            _ => 0,
        }
    }

    /// What `ability` does for party member `m`, in HP over the rounds left.
    fn on_member(&self, board: &Board, ability: &Ability, m: &Combatant) -> Value {
        if !self.scripted || !m.alive() || self.book.buffed.contains(&(ability.effect, m.id)) {
            return 0;
        }
        let rounds = self.rounds_left(board).saturating_sub(1).min(BUFF_ROUNDS);
        match ability.class() {
            EffectClass::Buff(Stat::Attack) if m.attack.0 <= m.attack.1 => {
                let Some(kit) = board.kit(m.id) else {
                    return 0;
                };
                let Some(base) = self.attack_pick(board, kit, m) else {
                    return 0;
                };
                let strong = Combatant {
                    attack: (m.attack.0.saturating_add(ability.power_stat), m.attack.1),
                    ..m.clone()
                };
                let raised = self.attack_pick(board, kit, &strong).map_or(0, |p| p.value);
                raised.saturating_sub(base.value) * rounds
            }
            EffectClass::Buff(Stat::Defence) if m.defence.0 <= m.defence.1 => {
                let strong = Combatant {
                    defence: (m.defence.0.saturating_add(ability.power_stat), m.defence.1),
                    ..m.clone()
                };
                let saved: Value = board
                    .enemies
                    .iter()
                    .map(|e| {
                        let hit = |t: &Combatant| {
                            damage(e.attack.0, t.defence.0, t.factor(e.attack_element), 0, t.hp)
                        };
                        hit(m).saturating_sub(hit(&strong)) * u64::from(e.plain_attacks) / 8
                    })
                    .sum();
                saved * rounds
            }
            _ => 0,
        }
    }

    pub(super) fn ability_pick(&self, board: &Board, kit: &Kit, index: usize) -> Option<Pick> {
        let ability = kit.abilities.get(index)?;
        let onto_enemies = matches!(
            ability.class(),
            EffectClass::Damage | EffectClass::Death | EffectClass::Sleep | EffectClass::Debuff(_)
        );
        let candidates: Vec<(u8, Value)> = ability
            .targets
            .iter()
            .filter_map(|id| {
                let value = if onto_enemies {
                    let enemy = board.enemies.iter().find(|e| e.id == *id)?;
                    self.on_enemy(board, kit, ability, enemy)
                } else {
                    self.on_member(board, ability, board.member(*id)?)
                };
                Some((*id, value))
            })
            .collect();
        if ability.single() {
            let (id, value) = best_of(candidates.iter().copied())?;
            Some(Pick {
                value,
                intent: Self::intent(ability, Some(id)),
                each: vec![(id, value)],
                ability: Some(index),
            })
        } else {
            Some(Pick {
                value: candidates.iter().map(|(_, v)| v).sum(),
                intent: Self::intent(ability, None),
                each: candidates,
                ability: Some(index),
            })
        }
    }

    /// The TP member `kit` keeps for their cheapest cure.
    fn reserve(kit: &Kit) -> u16 {
        kit.abilities
            .iter()
            .filter(|a| matches!(a.source, Source::Technique(_)) && Self::heals(a))
            .map(|a| u16::from(a.tp_cost))
            .min()
            .unwrap_or(0)
    }

    /// Member `kit`'s most valuable action that is not a cure, unbooked.
    pub(super) fn best_pick(&self, board: &Board, kit: &Kit) -> Option<Pick> {
        let me = board.member(kit.id)?;
        let attack = self.attack_pick(board, kit, me);
        let floor = attack.as_ref().map_or(0, |p| p.value);
        let reserve = Self::reserve(kit);
        let mut best = attack;
        for (index, ability) in kit.abilities.iter().enumerate() {
            if Self::heals(ability) || !self.in_stock(board, kit, ability) {
                continue;
            }
            if matches!(ability.source, Source::Technique(_))
                && me.tp < u16::from(ability.tp_cost) + reserve
            {
                continue;
            }
            if !self.scripted
                && matches!(
                    ability.source,
                    Source::Skill(_)
                        | Source::Item {
                            consumable: true,
                            ..
                        }
                )
            {
                continue;
            }
            let Some(pick) = self.ability_pick(board, kit, index) else {
                continue;
            };
            if !self.scripted && ability.spends() && pick.value < floor * ENCOUNTER_SPEND_FACTOR {
                continue;
            }
            if pick.value > best.as_ref().map_or(0, |b| b.value) {
                best = Some(pick);
            }
        }
        best.filter(|p| p.value > 0)
    }
}

/// The candidate worth most; the first of equals.
fn best_of(candidates: impl Iterator<Item = (u8, Value)>) -> Option<(u8, Value)> {
    candidates.fold(None, |best, (id, v)| match best {
        Some((_, b)) if b >= v => best,
        _ => Some((id, v)),
    })
}
