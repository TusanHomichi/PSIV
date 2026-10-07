//! The party policy's rules: what a player does with everything a member can do.
//!
//! One [`Planner`] serves every route policy name; the names differ only in
//! when they RUN (`policy.rs`). It reads a [`Board`] and returns an [`Intent`].
//! It never reads the RNG: every number is the engine's own formula on a local
//! roll sequence ([`crate::policy_estimate`]), on the stats the engine reads.
//!
//! # The battle's kind sets the budget
//!
//! A scripted battle (a boss, an event fight) is fought with everything: TP,
//! skill uses and items. A random encounter is fought cheaply, as a player who
//! must walk on afterwards fights it: skill uses and consumable items are kept
//! for the scripted battles (only an inn or a rest restores them), and TP goes
//! to damage only when one cast does at least [`ENCOUNTER_SPEND_FACTOR`] times
//! what the plain attack would. Cures are never rationed by the kind.
//!
//! # The order of the rules, for each member's window
//!
//! 1. **Group cure.** A learned all-ally cure, once a round, when two or more
//!    of the members it reaches are at risk. At risk means below the larger of
//!    [`GROUP_CURE_PERCENT`] of maximum HP and the threat below.
//! 2. **Single cure.** The member at the lowest share of HP who is at risk
//!    (below the larger of [`HURT_PERCENT`] and the threat) gets the cheapest
//!    cure that lifts them out of it, or the strongest there is. In a random
//!    encounter one cure a round is enough.
//! 3. **Revival** (scripted battles): a fallen member is raised while the fight
//!    has more than a round left; a full revival before a partial one.
//! 4. **Status cure** (scripted battles): a paralysed or sleeping member.
//! 5. **The best action by value**, in HP: the plain attack, every damaging
//!    technique, skill or item, an instant-death effect (its chance to land
//!    times the HP it removes), a sleep (its chance times what the enemy does
//!    in a turn), and in scripted battles an attack-down on the enemies or an
//!    attack- or defence-up on the party (the damage it adds or saves over the
//!    rounds the fight has left, at most [`BUFF_ROUNDS`]). A single-target
//!    command takes its best target.
//! 6. ATTACK, or DEFEND for a member with no weapon.
//!
//! **The threat** is what a member could lose before their next turn: the
//! larger of the most HP any member lost over one round of this battle (a
//! player watches the numbers) and the strongest plain attack a living enemy
//! could land on them (the damage formula at its highest roll). A threat at or
//! above a member's maximum HP is one no cure prevents, so it does not move the
//! line.
//!
//! **The round's book.** A player orders the round knowing what the earlier
//! members were told: the damage they will do comes off each enemy's HP before
//! the next member is valued (overkill counts nothing), cures count toward the
//! patient, a revival or a sleep is not ordered twice, and an item copy is not
//! spent twice.
//!
//! **TP for cures.** A member who knows a cure keeps the TP for its cheapest
//! one when choosing anything else.

use std::collections::BTreeMap;

use psiv_core::battle::status;

use crate::policy::Intent;
use crate::policy_board::{Ability, Board, Combatant, Source};
use crate::policy_estimate::{
    self as estimate, EffectClass, HP, Stat, Value, attack_damage, damage, healing, max_damage,
};

/// A member below this share of their maximum HP is at risk whatever the threat.
pub const HURT_PERCENT: u32 = 50;

/// A group cure is worth a command once two members it reaches are below this.
pub const GROUP_CURE_PERCENT: u32 = 70;

/// In a random encounter, TP buys damage only at this multiple of the attack.
pub const ENCOUNTER_SPEND_FACTOR: u64 = 2;

/// A buff or debuff is valued over at most this many of the rounds left.
pub const BUFF_ROUNDS: u64 = 4;

/// What one battle has taught the player so far.
#[derive(Debug, Clone, Default)]
struct Memory {
    /// Each member's HP at the start of the last round's orders.
    party_hp: Vec<(u8, u16)>,
    /// The enemies' total HP at the start of the last round's orders.
    enemy_hp: Option<u32>,
    /// The most HP one living member lost over one round.
    worst_loss: u16,
    /// The HP the whole party lost over the last round.
    party_loss: u32,
    /// The HP the enemies lost over the last round.
    enemy_loss: u32,
}

/// What the members already ordered this round.
#[derive(Debug, Clone, Default)]
struct Book {
    damage: BTreeMap<u8, Value>,
    healing: BTreeMap<u8, Value>,
    raised: Vec<u8>,
    slept: Vec<u8>,
    cured: Vec<u8>,
    buffed: Vec<(u8, u8)>,
    items: BTreeMap<u8, u16>,
    single_cures: u32,
    group_cured: bool,
}

#[cfg(test)]
#[path = "policy_plan_tests.rs"]
mod tests;

/// The rules, with their battle memory and round book.
#[derive(Debug, Clone, Default)]
pub struct Planner {
    scripted: bool,
    memory: Memory,
    book: Book,
    round_open: bool,
}

/// A scored candidate.
struct Pick {
    value: Value,
    intent: Intent,
    each: Vec<(u8, Value)>,
}

impl Planner {
    /// A battle begins; `scripted` for an event battle.
    pub fn begin_battle(&mut self, scripted: bool) {
        *self = Planner {
            scripted,
            ..Planner::default()
        };
    }

    /// Whether the battle in progress is scripted.
    #[must_use]
    pub const fn scripted(&self) -> bool {
        self.scripted
    }

    /// The round's orders are in: the next window opens a new round.
    pub fn end_round(&mut self) {
        self.book = Book::default();
        self.round_open = false;
    }

    /// The choice for the actor of `board`.
    pub fn decide(&mut self, board: &Board) -> Intent {
        if !self.round_open {
            self.observe(board);
            self.round_open = true;
        }
        let Some(me) = board.me() else {
            return Intent::Defend;
        };
        let intent = self
            .group_cure(board, me)
            .or_else(|| self.single_cure(board, me))
            .or_else(|| self.revive(board, me))
            .or_else(|| self.status_cure(board, me))
            .or_else(|| self.best_action(board, me));
        intent.unwrap_or(if board.weapon.is_some() {
            Intent::Attack { target: None }
        } else {
            Intent::Defend
        })
    }

    /// The round's first window: what the last round did to both sides.
    fn observe(&mut self, board: &Board) {
        let memory = &mut self.memory;
        let mut lost = 0_u32;
        for member in board.party.iter().filter(|m| m.alive()) {
            if let Some((_, before)) = memory.party_hp.iter().find(|(id, _)| *id == member.id) {
                let loss = before.saturating_sub(member.hp);
                memory.worst_loss = memory.worst_loss.max(loss);
                lost += u32::from(loss);
            }
        }
        memory.party_loss = lost;
        let enemy_hp: u32 = board.enemies.iter().map(|e| u32::from(e.hp)).sum();
        if let Some(before) = memory.enemy_hp {
            memory.enemy_loss = before.saturating_sub(enemy_hp);
        }
        memory.enemy_hp = Some(enemy_hp);
        memory.party_hp = board.party.iter().map(|m| (m.id, m.hp)).collect();
    }

    /// The most `member` could lose before their next turn.
    fn threat(&self, board: &Board, member: &Combatant) -> u16 {
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

    /// The HP below which `member` is at risk.
    fn line(&self, board: &Board, member: &Combatant, percent: u32) -> u16 {
        let floor = u16::try_from(u32::from(member.max_hp) * percent / 100).unwrap_or(u16::MAX);
        let threat = self.threat(board, member);
        if threat >= member.max_hp {
            floor
        } else {
            floor.max(threat.saturating_add(1))
        }
    }

    /// HP counting the cures already ordered.
    fn hp_after_cures(&self, member: &Combatant) -> u16 {
        let promised = self.book.healing.get(&member.id).copied().unwrap_or(0) / HP;
        member
            .hp
            .saturating_add(u16::try_from(promised).unwrap_or(u16::MAX))
            .min(member.max_hp)
    }

    fn at_risk(&self, board: &Board, member: &Combatant, percent: u32) -> Option<u16> {
        if !member.alive() {
            return None;
        }
        let hp = self.hp_after_cures(member);
        let line = self.line(board, member, percent);
        (hp < line).then(|| line - hp)
    }

    /// Whether the actor can pay for `ability` now: the TP for a technique
    /// (and no seal on the caster: retail takes the TP and wastes the turn,
    /// `CharTech_Cast`, `ps4.asm:14256`), a copy not yet ordered this round
    /// for an item. A skill on the board has a use left.
    fn in_stock(&self, ability: &Ability, board: &Board) -> bool {
        match &ability.source {
            Source::Item { id, .. } => {
                let ordered = self.book.items.get(id).copied().unwrap_or(0);
                board.stock(*id).is_none_or(|copies| ordered < copies)
            }
            Source::Technique(_) => board.me().is_some_and(|me| {
                me.tp >= u16::from(ability.tp_cost) && me.status & status::TECH_SEALED == 0
            }),
            Source::Skill(_) => true,
        }
    }

    /// The TP the actor keeps for their cheapest cure.
    fn reserve(board: &Board) -> u16 {
        board
            .abilities
            .iter()
            .filter(|a| {
                matches!(a.source, Source::Technique(_))
                    && matches!(a.class(), EffectClass::Heal | EffectClass::HealRevive)
            })
            .map(|a| u16::from(a.tp_cost))
            .min()
            .unwrap_or(0)
    }

    /// Ranks cures: a renewable one before an item, then by TP.
    fn cure_cost(ability: &Ability) -> (bool, u8) {
        (
            matches!(ability.source, Source::Item { .. }),
            ability.tp_cost,
        )
    }

    fn heals(ability: &Ability, me: &Combatant) -> bool {
        matches!(ability.class(), EffectClass::Heal | EffectClass::HealRevive)
            && (ability.range != 3 || ability.targets.contains(&me.id))
    }

    fn group_cure(&mut self, board: &Board, me: &Combatant) -> Option<Intent> {
        if self.book.group_cured {
            return None;
        }
        let mut options: Vec<(&Ability, Value, u16, Vec<u8>)> = Vec::new();
        for ability in board.abilities.iter().filter(|a| {
            !a.single() && a.range != 3 && Self::heals(a, me) && self.in_stock(a, board)
        }) {
            let patients: Vec<(u8, u16)> = board
                .party
                .iter()
                .filter(|m| ability.targets.contains(&m.id))
                .filter_map(|m| Some((m.id, self.at_risk(board, m, GROUP_CURE_PERCENT)?)))
                .collect();
            if patients.len() < 2 {
                continue;
            }
            let need = patients.iter().map(|(_, n)| *n).max().unwrap_or(0);
            let amount = healing(ability.power_stat, ability.power.into(), u16::MAX);
            options.push((ability, amount, need, patients.iter().map(|p| p.0).collect()));
        }
        let chosen = options
            .iter()
            .filter(|(_, amount, need, _)| *amount >= Value::from(*need) * HP)
            .min_by_key(|(a, ..)| Self::cure_cost(a))
            .or_else(|| options.iter().max_by_key(|(_, amount, ..)| *amount))?;
        let (ability, amount, _, patients) = chosen;
        self.book.group_cured = true;
        for patient in patients {
            *self.book.healing.entry(*patient).or_default() += amount;
        }
        Some(self.order(ability, None))
    }

    fn single_cure(&mut self, board: &Board, me: &Combatant) -> Option<Intent> {
        if !self.scripted && self.book.single_cures > 0 {
            return None;
        }
        let cures: Vec<&Ability> = board
            .abilities
            .iter()
            .filter(|a| (a.single() || a.range == 3) && Self::heals(a, me) && self.in_stock(a, board))
            .collect();
        let patient = board
            .party
            .iter()
            .filter(|m| cures.iter().any(|a| a.targets.contains(&m.id)))
            .filter_map(|m| Some((m, self.at_risk(board, m, HURT_PERCENT)?)))
            .min_by(|(a, _), (b, _)| {
                (u32::from(self.hp_after_cures(a)) * u32::from(b.max_hp))
                    .cmp(&(u32::from(self.hp_after_cures(b)) * u32::from(a.max_hp)))
            });
        let (patient, need) = patient?;
        let mut options: Vec<(&Ability, Value)> = cures
            .into_iter()
            .filter(|a| a.targets.contains(&patient.id))
            .map(|a| (a, healing(a.power_stat, a.power.into(), u16::MAX)))
            .collect();
        options.sort_by_key(|(a, _)| Self::cure_cost(a));
        let chosen = options
            .iter()
            .find(|(_, amount)| *amount >= Value::from(need) * HP)
            .or_else(|| options.iter().max_by_key(|(_, amount)| *amount))?;
        let (ability, amount) = *chosen;
        self.book.single_cures += 1;
        *self.book.healing.entry(patient.id).or_default() += amount;
        Some(self.order(ability, ability.single().then_some(patient.id)))
    }

    fn revive(&mut self, board: &Board, me: &Combatant) -> Option<Intent> {
        if !self.scripted || self.rounds_left(board, me) < 2 {
            return None;
        }
        let fallen: Vec<&Combatant> = board
            .party
            .iter()
            .filter(|m| !m.alive() && !self.book.raised.contains(&m.id))
            .collect();
        let rank = |a: &Ability| match a.class() {
            EffectClass::Revive { full: true } => Some(0),
            EffectClass::HealRevive => Some(1),
            EffectClass::Revive { full: false } => Some(2),
            _ => None,
        };
        let (ability, patients) = board
            .abilities
            .iter()
            .filter(|a| self.in_stock(a, board))
            .filter_map(|a| {
                let rank = rank(a)?;
                let patients: Vec<u8> = fallen
                    .iter()
                    .filter(|m| a.targets.contains(&m.id))
                    .map(|m| m.id)
                    .collect();
                (!patients.is_empty()).then_some((rank, Self::cure_cost(a), a, patients))
            })
            .min_by_key(|(rank, cost, ..)| (*rank, *cost))
            .map(|(_, _, a, patients)| (a, patients))?;
        let target = ability.single().then(|| patients[0]);
        match target {
            Some(id) => self.book.raised.push(id),
            None => self.book.raised.extend(patients),
        }
        Some(self.order(ability, target))
    }

    fn status_cure(&mut self, board: &Board, me: &Combatant) -> Option<Intent> {
        if !self.scripted || self.rounds_left(board, me) < 2 {
            return None;
        }
        for ability in board.abilities.iter().filter(|a| self.in_stock(a, board)) {
            let EffectClass::Cure(bits) = ability.class() else {
                continue;
            };
            let bits = bits & (status::PARALYZED | status::ASLEEP);
            let patient = board.party.iter().find(|m| {
                m.alive()
                    && m.status & bits != 0
                    && ability.targets.contains(&m.id)
                    && !self.book.cured.contains(&m.id)
            });
            if let Some(patient) = patient {
                self.book.cured.push(patient.id);
                return Some(self.order(ability, ability.single().then_some(patient.id)));
            }
        }
        None
    }

    /// An enemy's HP after the damage already ordered.
    fn left(&self, enemy: &Combatant) -> u16 {
        let ordered = self.book.damage.get(&enemy.id).copied().unwrap_or(0) / HP;
        enemy
            .hp
            .saturating_sub(u16::try_from(ordered).unwrap_or(u16::MAX))
    }

    /// Rounds the fight has left at the rate the party removes HP.
    fn rounds_left(&self, board: &Board, me: &Combatant) -> u64 {
        let remaining: u64 = board.enemies.iter().map(|e| u64::from(self.left(e))).sum();
        let living = board.party.iter().filter(|m| m.alive()).count() as u64;
        let rate = if self.memory.enemy_loss > 0 {
            u64::from(self.memory.enemy_loss)
        } else {
            let swing = self.attack_pick(board, me).map_or(HP, |p| p.value);
            (swing * living.max(1) / HP).max(1)
        };
        remaining.div_ceil(rate.max(1)).max(1)
    }

    /// What an enemy does to the party in one of its turns.
    fn enemy_turn(&self, board: &Board, enemy: &Combatant) -> Value {
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
        let seen =
            Value::from(self.memory.party_loss) * HP / (board.enemies.len().max(1) as u64);
        plain.max(seen)
    }

    fn attack_pick(&self, board: &Board, me: &Combatant) -> Option<Pick> {
        let weapon = board.weapon.as_ref()?;
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
                intent: Intent::Attack { target: None },
                each,
            });
        }
        board
            .enemies
            .iter()
            .map(|e| (e.id, hit(e)))
            .fold(None::<(u8, Value)>, |best, (id, v)| match best {
                Some((_, b)) if b >= v => best,
                _ => Some((id, v)),
            })
            .map(|(id, value)| Pick {
                value,
                intent: Intent::Attack { target: Some(id) },
                each: vec![(id, value)],
            })
    }

    /// What `ability` does to enemy `e`, in HP.
    fn on_enemy(&self, board: &Board, me: &Combatant, ability: &Ability, e: &Combatant) -> Value {
        let factor = if ability.element >= 0x10 {
            board.weapon.as_ref().map_or(0, |w| {
                w.elements.iter().map(|el| e.factor(*el)).max().unwrap_or(0)
            })
        } else {
            e.factor(ability.element)
        };
        let resist = e.stat(ability.resistance);
        let lands = || {
            estimate::landing_chance(
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
                lands() * saved * self.rounds_left(board, me).saturating_sub(1).min(BUFF_ROUNDS)
                    / 64
            }
            _ => 0,
        }
    }

    /// What `ability` does for party member `m`, in HP over the rounds left.
    fn on_member(&self, board: &Board, me: &Combatant, ability: &Ability, m: &Combatant) -> Value {
        if !self.scripted || !m.alive() || self.book.buffed.contains(&(ability.effect, m.id)) {
            return 0;
        }
        let rounds = self.rounds_left(board, me).saturating_sub(1).min(BUFF_ROUNDS);
        match ability.class() {
            EffectClass::Buff(Stat::Attack) if m.attack.0 <= m.attack.1 => {
                let Some(base) = self.attack_pick(board, m) else {
                    return 0;
                };
                let strong = Combatant {
                    attack: (m.attack.0.saturating_add(ability.power_stat), m.attack.1),
                    ..m.clone()
                };
                let raised = self.attack_pick(board, &strong).map_or(0, |p| p.value);
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

    fn ability_pick(&self, board: &Board, me: &Combatant, ability: &Ability) -> Option<Pick> {
        let onto_enemies = matches!(
            ability.class(),
            EffectClass::Damage | EffectClass::Death | EffectClass::Sleep | EffectClass::Debuff(_)
        );
        let candidates: Vec<(u8, Value)> = ability
            .targets
            .iter()
            .filter_map(|id| {
                let fighter = board.fighter(*id)?;
                Some((
                    *id,
                    if onto_enemies {
                        board.enemies.iter().any(|e| e.id == *id).then(|| {
                            self.on_enemy(board, me, ability, fighter)
                        })?
                    } else {
                        self.on_member(board, me, ability, fighter)
                    },
                ))
            })
            .collect();
        if ability.single() {
            let (id, value) = candidates.iter().copied().fold(None, |best, (id, v)| match best {
                Some((_, b)) if b >= v => best,
                _ => Some((id, v)),
            })?;
            Some(Pick {
                value,
                intent: self.intent(ability, Some(id)),
                each: vec![(id, value)],
            })
        } else {
            Some(Pick {
                value: candidates.iter().map(|(_, v)| v).sum(),
                intent: self.intent(ability, None),
                each: candidates,
            })
        }
    }

    fn best_action(&mut self, board: &Board, me: &Combatant) -> Option<Intent> {
        let attack = self.attack_pick(board, me);
        let floor = attack.as_ref().map_or(0, |p| p.value);
        let reserve = Self::reserve(board);
        let mut best = attack;
        for ability in board.abilities.iter().filter(|a| self.in_stock(a, board)) {
            if matches!(
                ability.class(),
                EffectClass::Heal | EffectClass::HealRevive
            ) {
                continue;
            }
            if let Source::Technique(_) = ability.source
                && me.tp < u16::from(ability.tp_cost) + reserve
            {
                continue;
            }
            if !self.scripted
                && (matches!(ability.source, Source::Skill(_))
                    || matches!(ability.source, Source::Item { consumable: true, .. }))
            {
                continue;
            }
            let Some(pick) = self.ability_pick(board, me, ability) else {
                continue;
            };
            if !self.scripted && ability.spends() && pick.value < floor * ENCOUNTER_SPEND_FACTOR {
                continue;
            }
            if pick.value > best.as_ref().map_or(0, |b| b.value) {
                best = Some(pick);
            }
        }
        let best = best.filter(|p| p.value > 0)?;
        self.commit(board, &best);
        if std::env::var_os("PSIV_CAMPAIGN_TRACE").is_some() {
            eprintln!(
                "  value {} -> {:?} ({} HP)",
                me.name,
                best.intent,
                best.value / HP
            );
        }
        Some(best.intent)
    }

    /// Books a chosen action's effect for the members still to order.
    fn commit(&mut self, board: &Board, pick: &Pick) {
        let ability = match &pick.intent {
            Intent::Technique { id, .. } => board
                .abilities
                .iter()
                .find(|a| a.source == Source::Technique(*id)),
            Intent::Skill { id, .. } => board
                .abilities
                .iter()
                .find(|a| a.source == Source::Skill(*id)),
            Intent::Item { name, .. } => board
                .abilities
                .iter()
                .find(|a| matches!(&a.source, Source::Item { name: n, .. } if n == name)),
            _ => None,
        };
        let class = ability.map_or(EffectClass::Damage, Ability::class);
        for (id, value) in &pick.each {
            match class {
                EffectClass::Damage | EffectClass::Death => {
                    *self.book.damage.entry(*id).or_default() += value;
                }
                EffectClass::Sleep => self.book.slept.push(*id),
                _ => {
                    if let Some(a) = ability {
                        self.book.buffed.push((a.effect, *id));
                    }
                }
            }
        }
        if let Some(Source::Item { id, .. }) = ability.map(|a| &a.source) {
            *self.book.items.entry(*id).or_default() += 1;
        }
    }

    fn intent(&self, ability: &Ability, target: Option<u8>) -> Intent {
        let target = if ability.single() { target } else { None };
        match &ability.source {
            Source::Technique(id) => Intent::Technique { id: *id, target },
            Source::Skill(id) => Intent::Skill { id: *id, target },
            Source::Item { name, .. } => Intent::Item {
                name: name.clone(),
                target,
            },
        }
    }

    /// Orders `ability` and books an item copy.
    fn order(&mut self, ability: &Ability, target: Option<u8>) -> Intent {
        if let Source::Item { id, .. } = ability.source {
            *self.book.items.entry(id).or_default() += 1;
        }
        self.intent(ability, target)
    }
}
