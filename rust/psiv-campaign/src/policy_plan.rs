//! The party policy's rules: what a player does with everything the party can do.
//!
//! One [`Planner`] serves every route policy name; the names differ only in
//! when they RUN (`policy.rs`). It reads a [`Board`] and returns an [`Intent`].
//! It never reads the RNG: every number is the engine's own formula on a local
//! roll sequence ([`crate::policy_estimate`]), on the stats the engine reads
//! (`policy_value.rs` holds the valuations).
//!
//! # A round is ordered as a whole
//!
//! The first command window of a round orders every member who can act, the
//! way a player thinks the round through before pressing anything; each window
//! then takes its member's order. A window the plan did not expect (the board
//! changed under it) is ordered on its own by the same rules.
//!
//! # The battle's kind sets the budget
//!
//! A scripted battle (a boss, an event fight) is fought with everything: TP,
//! skill uses and items. A random encounter is fought cheaply, as a player who
//! must walk on afterwards fights it: skill uses and consumable items are kept
//! for the scripted battles (only an inn or a rest restores them), and TP goes
//! to damage only when one cast does at least [`ENCOUNTER_SPEND_FACTOR`] times
//! what the member's plain attack would. Cures are never rationed by the kind,
//! but a random encounter needs at most one single cure and one group cure a
//! round.
//!
//! # The order of the rules
//!
//! 1. **Finish it.** When the members' best actions are expected to remove
//!    every enemy's HP this round, nobody cures: the round is all offence.
//! 2. **Cures**, most urgent first, each given to the member whose own best
//!    action is worth least (a cure that lifts the patient clear of the threat
//!    before one that does not, then the cheaper cure):
//!    * a group cure when two or more members it reaches are at risk, below the
//!      larger of [`GROUP_CURE_PERCENT`] of maximum HP and the threat;
//!    * a single cure for each member at risk, below the larger of
//!      [`HURT_PERCENT`] and the threat, lowest share of HP first, one a
//!      member;
//!    * in a scripted battle with more than a round left, a revival for each
//!      fallen member (full before partial) and a status cure for a paralysed
//!      or sleeping one.
//! 3. **The best action by value**, in HP, for everyone else in slot order: the
//!    plain attack, every damaging technique, skill or item, an instant-death
//!    effect (its chance to land times the HP it removes), a sleep (its chance
//!    times what the enemy does in a turn), and in scripted battles an
//!    attack-down on the enemies or an attack- or defence-up on the party (the
//!    damage it adds or saves over the rounds the fight has left, at most
//!    [`BUFF_ROUNDS`]). A single-target command takes its best target.
//! 4. ATTACK, or DEFEND for a member with no weapon.
//!
//! **The threat** is what a member could lose before their next turn: the
//! larger of the most HP any member lost over one round of this battle (a
//! player watches the numbers) and the strongest plain attack a living enemy
//! could land on them (the damage formula at its highest roll). A threat at or
//! above a member's maximum HP is one no cure prevents, so it does not move the
//! line.
//!
//! **The round's book.** The damage an order will do comes off each enemy's HP
//! before the next member is valued (overkill counts nothing), cures count
//! toward the patient, a revival or a sleep is not ordered twice, and an item
//! copy is not spent twice.
//!
//! **TP for cures.** A member who knows a cure keeps the TP for its cheapest
//! one when choosing anything else.

use std::collections::BTreeMap;

use psiv_core::battle::status;

use crate::policy::Intent;
use crate::policy_board::{Ability, Board, Combatant, Kit, Source};
use crate::policy_estimate::{EffectClass, HP, Value, healing};

#[cfg(test)]
#[path = "policy_plan_tests.rs"]
mod tests;

#[path = "policy_value.rs"]
mod value;

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
    group_cures: u32,
    /// Members who already have a single cure ordered.
    patients: Vec<u8>,
}

/// The rules, with their battle memory and the round's orders.
#[derive(Debug, Clone, Default)]
pub struct Planner {
    scripted: bool,
    memory: Memory,
    book: Book,
    orders: BTreeMap<u8, Intent>,
    round_open: bool,
}

/// A scored candidate: its value, the order, what it does to each fighter,
/// and which of the kit's abilities it uses (`None` for ATTACK).
struct Pick {
    value: Value,
    intent: Intent,
    each: Vec<(u8, Value)>,
    ability: Option<usize>,
}

/// A cure on offer: who casts what, on whom, and how it ranks.
struct Offer<'a> {
    member: u8,
    ability: &'a Ability,
    target: Option<u8>,
    patients: Vec<u8>,
    amount: Value,
    rank: (bool, u8, Value, (bool, u8), u8),
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
        self.orders.clear();
        self.round_open = false;
    }

    /// The choice for the actor of `board`.
    pub fn decide(&mut self, board: &Board) -> Intent {
        if !self.round_open {
            self.observe(board);
            self.plan_round(board);
            self.round_open = true;
        }
        if let Some(intent) = self.orders.get(&board.actor) {
            return intent.clone();
        }
        let intent = self.offence(board, board.actor);
        self.orders.insert(board.actor, intent.clone());
        intent
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

    /// The members who will be asked this round, in slot order.
    fn ready(board: &Board) -> Vec<u8> {
        board
            .kits
            .iter()
            .filter(|k| {
                board
                    .member(k.id)
                    .is_some_and(|m| m.alive() && m.status & status::NO_TURN == 0)
            })
            .map(|k| k.id)
            .collect()
    }

    fn plan_round(&mut self, board: &Board) {
        let mut free = Self::ready(board);
        if !free.contains(&board.actor) {
            free.push(board.actor);
            free.sort_unstable();
        }
        // Finish it: if the whole round's offence empties the enemy side, no
        // member spends a turn on a cure.
        let mut trial = self.clone();
        for id in &free {
            let intent = trial.offence(board, *id);
            trial.orders.insert(*id, intent);
        }
        if board.enemies.iter().all(|e| trial.left(e) == 0) {
            *self = trial;
            return;
        }
        while let Some((member, intent)) = self.next_cure(board, &free) {
            self.orders.insert(member, intent);
            free.retain(|id| *id != member);
        }
        for id in free {
            let intent = self.offence(board, id);
            self.orders.insert(id, intent);
        }
    }

    /// The best action of member `id`, booked; ATTACK or DEFEND when nothing
    /// is worth more.
    fn offence(&mut self, board: &Board, id: u8) -> Intent {
        let kit = board.kit(id);
        let pick = kit.and_then(|kit| self.best_pick(board, kit));
        match pick {
            Some(pick) => {
                self.commit(kit.expect("a pick has a kit"), &pick);
                if std::env::var_os("PSIV_CAMPAIGN_TRACE").is_some() {
                    eprintln!("  value {id} -> {:?} ({} HP)", pick.intent, pick.value / HP);
                }
                pick.intent
            }
            None if kit.is_some_and(|k| k.weapon.is_some()) => Intent::ATTACK,
            None => Intent::Defend,
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

    /// How far below the risk line `member` stands, or `None` when clear.
    fn at_risk(&self, board: &Board, member: &Combatant, percent: u32) -> Option<u16> {
        if !member.alive() {
            return None;
        }
        let hp = self.hp_after_cures(member);
        let line = self.line(board, member, percent);
        (hp < line).then(|| line - hp)
    }

    /// Whether member `kit` can pay for `ability` now: the TP for a technique
    /// (and no seal: retail takes the TP and wastes the turn, `CharTech_Cast`,
    /// `ps4.asm:14256`), a copy not yet ordered this round for an item. A skill
    /// on the board has a use left.
    fn in_stock(&self, board: &Board, kit: &Kit, ability: &Ability) -> bool {
        match &ability.source {
            Source::Item { id, .. } => {
                let ordered = self.book.items.get(id).copied().unwrap_or(0);
                board.stock(*id).is_none_or(|copies| ordered < copies)
            }
            Source::Technique(_) => board.member(kit.id).is_some_and(|me| {
                me.tp >= u16::from(ability.tp_cost) && me.status & status::TECH_SEALED == 0
            }),
            Source::Skill(_) => true,
        }
    }

    /// Ranks cures by what they spend: a renewable one before an item, then by TP.
    fn cure_cost(ability: &Ability) -> (bool, u8) {
        (
            matches!(ability.source, Source::Item { .. }),
            ability.tp_cost,
        )
    }

    fn heals(ability: &Ability) -> bool {
        matches!(ability.class(), EffectClass::Heal | EffectClass::HealRevive)
    }

    /// The most urgent cure some free member can give, booked.
    fn next_cure(&mut self, board: &Board, free: &[u8]) -> Option<(u8, Intent)> {
        let offers = self
            .group_offers(board, free)
            .or_else(|| self.single_offers(board, free))
            .or_else(|| self.revive_offers(board, free))
            .or_else(|| self.status_offers(board, free))?;
        let best = offers.into_iter().min_by_key(|o| o.rank)?;
        let class = best.ability.class();
        match class {
            EffectClass::Heal | EffectClass::HealRevive if best.target.is_none() && best.patients.len() > 1 => {
                self.book.group_cures += 1;
            }
            EffectClass::Heal | EffectClass::HealRevive => {
                self.book.single_cures += 1;
                self.book.patients.extend(&best.patients);
            }
            EffectClass::Revive { .. } => {}
            _ => {}
        }
        for patient in &best.patients {
            match class {
                EffectClass::Heal | EffectClass::HealRevive => {
                    *self.book.healing.entry(*patient).or_default() += best.amount;
                    if class == EffectClass::HealRevive {
                        self.book.raised.push(*patient);
                    }
                }
                EffectClass::Revive { .. } => self.book.raised.push(*patient),
                _ => self.book.cured.push(*patient),
            }
        }
        if let Source::Item { id, .. } = best.ability.source {
            *self.book.items.entry(id).or_default() += 1;
        }
        Some((best.member, Self::intent(best.ability, best.target)))
    }

    /// What member `id`'s best action is worth, unbooked: a cure's price.
    fn worth(&self, board: &Board, id: u8) -> Value {
        board
            .kit(id)
            .and_then(|kit| self.best_pick(board, kit))
            .map_or(0, |p| p.value)
    }

    fn group_offers<'a>(&self, board: &'a Board, free: &[u8]) -> Option<Vec<Offer<'a>>> {
        if !self.scripted && self.book.group_cures > 0 {
            return None;
        }
        let mut offers = Vec::new();
        for kit in board.kits.iter().filter(|k| free.contains(&k.id)) {
            for ability in kit.abilities.iter().filter(|a| {
                !a.single() && a.range != 3 && Self::heals(a) && self.in_stock(board, kit, a)
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
                offers.push(Offer {
                    member: kit.id,
                    ability,
                    target: None,
                    patients: patients.iter().map(|p| p.0).collect(),
                    amount,
                    rank: (
                        amount < Value::from(need) * HP,
                        0,
                        self.worth(board, kit.id),
                        Self::cure_cost(ability),
                        kit.id,
                    ),
                });
            }
        }
        (!offers.is_empty()).then_some(offers)
    }

    fn single_offers<'a>(&self, board: &'a Board, free: &[u8]) -> Option<Vec<Offer<'a>>> {
        if !self.scripted && self.book.single_cures > 0 {
            return None;
        }
        let reaches = |kit: &Kit, a: &Ability, patient: u8| {
            (a.single() && a.targets.contains(&patient)) || (a.range == 3 && kit.id == patient)
        };
        let cures: Vec<(&Kit, &Ability)> = board
            .kits
            .iter()
            .filter(|k| free.contains(&k.id))
            .flat_map(|k| k.abilities.iter().map(move |a| (k, a)))
            .filter(|(k, a)| (a.single() || a.range == 3) && Self::heals(a) && self.in_stock(board, k, a))
            .collect();
        let (patient, need) = board
            .party
            .iter()
            .filter(|m| !self.book.patients.contains(&m.id))
            .filter(|m| cures.iter().any(|(k, a)| reaches(k, a, m.id)))
            .filter_map(|m| Some((m, self.at_risk(board, m, HURT_PERCENT)?)))
            .min_by(|(a, _), (b, _)| {
                (u32::from(self.hp_after_cures(a)) * u32::from(b.max_hp))
                    .cmp(&(u32::from(self.hp_after_cures(b)) * u32::from(a.max_hp)))
            })?;
        let offers: Vec<Offer<'a>> = cures
            .into_iter()
            .filter(|(k, a)| reaches(k, a, patient.id))
            .map(|(kit, ability)| {
                let amount = healing(ability.power_stat, ability.power.into(), u16::MAX);
                Offer {
                    member: kit.id,
                    ability,
                    target: ability.single().then_some(patient.id),
                    patients: vec![patient.id],
                    amount,
                    rank: (
                        amount < Value::from(need) * HP,
                        0,
                        self.worth(board, kit.id),
                        Self::cure_cost(ability),
                        kit.id,
                    ),
                }
            })
            .collect();
        (!offers.is_empty()).then_some(offers)
    }

    fn revive_offers<'a>(&self, board: &'a Board, free: &[u8]) -> Option<Vec<Offer<'a>>> {
        if !self.scripted || self.rounds_left(board) < 2 {
            return None;
        }
        let fallen: Vec<u8> = board
            .party
            .iter()
            .filter(|m| !m.alive() && !self.book.raised.contains(&m.id))
            .map(|m| m.id)
            .collect();
        let rank = |a: &Ability| match a.class() {
            EffectClass::Revive { full: true } => Some(0),
            EffectClass::HealRevive => Some(1),
            EffectClass::Revive { full: false } => Some(2),
            _ => None,
        };
        let mut offers = Vec::new();
        for kit in board.kits.iter().filter(|k| free.contains(&k.id)) {
            for ability in kit.abilities.iter().filter(|a| self.in_stock(board, kit, a)) {
                let Some(class_rank) = rank(ability) else {
                    continue;
                };
                let patients: Vec<u8> = fallen
                    .iter()
                    .copied()
                    .filter(|id| ability.targets.contains(id))
                    .collect();
                let Some(first) = patients.first().copied() else {
                    continue;
                };
                let single = ability.single();
                offers.push(Offer {
                    member: kit.id,
                    ability,
                    target: single.then_some(first),
                    patients: if single { vec![first] } else { patients },
                    amount: 0,
                    rank: (
                        false,
                        class_rank,
                        self.worth(board, kit.id),
                        Self::cure_cost(ability),
                        kit.id,
                    ),
                });
            }
        }
        (!offers.is_empty()).then_some(offers)
    }

    fn status_offers<'a>(&self, board: &'a Board, free: &[u8]) -> Option<Vec<Offer<'a>>> {
        if !self.scripted || self.rounds_left(board) < 2 {
            return None;
        }
        let mut offers = Vec::new();
        for kit in board.kits.iter().filter(|k| free.contains(&k.id)) {
            for ability in kit.abilities.iter().filter(|a| self.in_stock(board, kit, a)) {
                let EffectClass::Cure(bits) = ability.class() else {
                    continue;
                };
                let bits = bits & (status::PARALYZED | status::ASLEEP);
                let Some(patient) = board.party.iter().find(|m| {
                    m.alive()
                        && m.status & bits != 0
                        && ability.targets.contains(&m.id)
                        && !self.book.cured.contains(&m.id)
                }) else {
                    continue;
                };
                offers.push(Offer {
                    member: kit.id,
                    ability,
                    target: ability.single().then_some(patient.id),
                    patients: vec![patient.id],
                    amount: 0,
                    rank: (
                        false,
                        0,
                        self.worth(board, kit.id),
                        Self::cure_cost(ability),
                        kit.id,
                    ),
                });
            }
        }
        (!offers.is_empty()).then_some(offers)
    }

    /// Books a chosen action's effect for the members still to order.
    fn commit(&mut self, kit: &Kit, pick: &Pick) {
        let ability = pick.ability.and_then(|index| kit.abilities.get(index));
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

    fn intent(ability: &Ability, target: Option<u8>) -> Intent {
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
}
