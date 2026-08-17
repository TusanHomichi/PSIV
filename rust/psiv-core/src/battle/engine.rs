//! The turn engine: a battle, resolved a round at a time.
//!
//! # The shape of a round
//!
//! One call to [`Battle::round`] runs a whole round and returns the timeline.
//! Nothing is resolved lazily and nothing is left half-done, because the
//! oracle's comparison point is end-of-round RAM — on hardware the HP
//! subtraction lives inside the animation state machine, so "mid-round" is not
//! a state a headless core can meaningfully match anyway.
//!
//! The sequence inside a round follows `BattleRoutines` (`ps4.asm:7524`):
//!
//! 1. `Battle_ProcessRUN`, if the party chose RUN. Success ends the battle;
//!    failure sets `Battle_Priority` to `$FF`, which hands the round to the
//!    enemies.
//! 2. `Battle_OrderTurns` — nine jitter rolls, the sort, then four
//!    `Enemy_TargetCharacter` rolls that commit every enemy to a target
//!    *before* anyone swings.
//! 3. Each queue entry acts, in order.
//! 4. `Battle_RestoreStatsAtTurnEnd` puts the physical property back, which is
//!    how Defend wears off.
//!
//! # Roll accounting
//!
//! Every roll the cartridge draws is drawn here, in the same order, including
//! the ones whose result is discarded — the nine jitter draws for five
//! fighters, the four target draws for two enemies, the reroll an enemy burns
//! to pick the same ability twice. Substituting the H/V counter changes the
//! *values*; it must not change the *count*.

use super::action::resolve_attack;
use super::ai::{choose_ability, choose_target, targetable_party};
use super::chances::{ESCAPE, Verdict, calculate_chances};
use super::event::{BattleEvent, Outcome, Skipped};
use super::fighters::{ENEMY_SLOTS, FighterId, Roster, Side};
use super::order::{Priority, QueueEntry, build_queue, roll_priority};
use super::records::{BattleData, BattleDataError, CharacterRecord, FormationRecord};
use super::rewards::{Pools, split_rewards};
use super::rng::Rolls;
use super::stats::Stats;

/// One party member entering a battle.
///
/// Build one with [`PartyMember::seat`] from a character record and the item
/// table, or construct it directly when the character is carrying state from
/// the field rather than starting fresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMember {
    /// Index into `Character_Stats`, which is also the key into the level
    /// tables.
    pub character: u8,
    /// Display name.
    pub name: String,
    /// Live stats, carried in from the field.
    pub stats: Stats,
}

impl PartyMember {
    /// Seats a character from their initial record, deriving everything.
    ///
    /// This is `InitializeCharStats` (`$0044652`) for one character: copy the
    /// record across, then run both derivation passes — `UpdateCharModStats`
    /// for the stat totals and `UpdateCharElems` for the element properties and
    /// the weapon-element cache. The result matches the pack's `initialized`
    /// vector for all eleven starting characters.
    ///
    /// Fail-closed on equipment: every non-zero slot must name an item the data
    /// set knows, because an item that quietly resolved to nothing would take
    /// its stat bonuses and its element with it and the damage numbers would
    /// merely look plausible.
    ///
    /// # Errors
    /// [`BattleDataError::UnknownItem`] naming the first slot that does not
    /// resolve.
    pub fn seat(
        record: &CharacterRecord,
        data: &BattleData,
    ) -> Result<PartyMember, BattleDataError> {
        for id in record.equipment {
            if id != 0 {
                data.item(id)?;
            }
        }
        let item = |id: u8| data.item(id).ok().cloned();
        Ok(PartyMember {
            character: record.id,
            name: record.name.clone(),
            stats: Stats::from_character(record, item),
        })
    }
}

/// What a party member does with their turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Command {
    /// Swing. `target` is the cursor's choice; `None` means the default, which
    /// is the first living enemy — what the oracle's mash-C tape produced.
    /// A multi-target weapon ignores it.
    #[default]
    Attack,
    /// Swing at a named enemy.
    AttackTarget(FighterId),
    /// Take physical resistance until the round ends.
    Defend,
    /// Spend one saved vehicle skill use and resolve the turn.
    ///
    /// The skill effect itself remains a Tier-2 seam. The engine owns the
    /// cartridge-visible part of the command: it validates the selected slot
    /// against the mounted member's skill mask, decrements the current use
    /// count, and emits an explicit effect-boundary marker. It never converts
    /// a vehicle skill into a physical attack.
    VehicleSkill(u8),
}

/// What the party chose from the main menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundOrders {
    /// COMD. One command per **party slot**, in slot order. A slot with no
    /// entry — a short vector — defaults to [`Command::Attack`], which is the
    /// cartridge's own default and the whole reason a player can win by
    /// pressing C.
    Commands(Vec<Command>),
    /// RUN.
    Run,
}

impl RoundOrders {
    /// Everyone attacks with the default cursor.
    #[must_use]
    pub fn attack_all() -> RoundOrders {
        RoundOrders::Commands(Vec::new())
    }

    fn command_for(&self, slot: usize) -> Command {
        match self {
            RoundOrders::Commands(commands) => commands.get(slot).copied().unwrap_or_default(),
            RoundOrders::Run => Command::Attack,
        }
    }
}

/// A battle in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battle {
    roster: Roster,
    round: u16,
    /// Consumed by the first [`Battle::round`] and reset to
    /// [`Priority::Normal`] afterwards, exactly as `Battle_OrderTurns` clears
    /// `Battle_Priority` on its way out (`ps4.asm:7941`).
    pending_priority: Priority,
    pools: Pools,
    outcome: Option<Outcome>,
    /// `$FFFFEEA8`, the shared "previous ability index" every enemy rerolls
    /// against.
    last_ability_index: Option<u8>,
    /// `Enemy_Run_Chance`, or `None` for a formation at or above `$F0` that
    /// cannot be escaped at all.
    run_chance: Option<u8>,
    /// Whether this is the one-fighter vehicle battle surface.
    vehicle: bool,
}

impl Battle {
    /// Sets a battle up: expands the formation, seats the party, rolls the
    /// opening priority.
    ///
    /// Draws exactly one roll, for `loc_B62A`.
    ///
    /// # Errors
    /// [`BattleDataError`] for a formation naming an unknown enemy, an empty
    /// formation, or more enemies than there are slots.
    pub fn start(
        formation: &FormationRecord,
        party: Vec<PartyMember>,
        data: &BattleData,
        boss: bool,
        rolls: &mut impl Rolls,
    ) -> Result<(Battle, Vec<BattleEvent>), BattleDataError> {
        Self::start_inner(formation, party, data, boss, false, rolls)
    }

    /// Sets up the retail vehicle battle surface: one saved vehicle fighter,
    /// the ordinary formation expansion and the vehicle reward halving.
    /// `loc_78EE` replaces the party with one fighter; the runtime supplies
    /// that member from `VehicleRecord`.
    pub fn start_vehicle(
        formation: &FormationRecord,
        party: Vec<PartyMember>,
        data: &BattleData,
        rolls: &mut impl Rolls,
    ) -> Result<(Battle, Vec<BattleEvent>), BattleDataError> {
        Self::start_inner(formation, party, data, false, true, rolls)
    }

    fn start_inner(
        formation: &FormationRecord,
        party: Vec<PartyMember>,
        data: &BattleData,
        boss: bool,
        vehicle: bool,
        rolls: &mut impl Rolls,
    ) -> Result<(Battle, Vec<BattleEvent>), BattleDataError> {
        if formation.enemies.is_empty() {
            return Err(BattleDataError::EmptyFormation(formation.id));
        }
        if formation.enemies.len() > ENEMY_SLOTS {
            return Err(BattleDataError::TooManyEnemies {
                formation: formation.id,
                named: formation.enemies.len(),
            });
        }

        let mut roster = Roster::new();
        for member in party {
            roster.add_party_member(member.character, member.name, member.stats);
        }
        let mut enemies = Vec::new();
        for slot in &formation.enemies {
            let record = data.enemy(slot.enemy_id)?;
            if let Some(id) = roster.add_enemy(slot.slot, record) {
                enemies.push(id);
            }
        }

        let priority = roll_priority(
            roster.highest_party_agility(),
            formation.ambush_chance,
            boss,
            rolls,
        );

        let events = vec![BattleEvent::Started {
            priority,
            enemies: enemies.clone(),
        }];
        Ok((
            Battle {
                roster,
                round: 0,
                pending_priority: priority,
                pools: Pools::default(),
                outcome: None,
                last_ability_index: None,
                run_chance: formation.can_run().then_some(formation.run_chance),
                vehicle,
            },
            events,
        ))
    }

    /// The battlefield.
    #[must_use]
    pub const fn roster(&self) -> &Roster {
        &self.roster
    }

    /// The party's live stats, keyed by `Character_Stats` index.
    ///
    /// A borrowing view for inspecting a battle in progress. To *keep* the
    /// results, use [`Battle::into_party`] — the whole point of the shared
    /// [`Stats`] record is that what a battle leaves behind is what the field
    /// carries away, with no conversion in between.
    pub fn party_stats(&self) -> impl Iterator<Item = (u8, &Stats)> {
        self.roster
            .side(Side::Party)
            .filter_map(|fighter| fighter.character.map(|id| (id, &fighter.stats)))
    }

    /// Consumes the battle and hands the party back.
    ///
    /// Everything a battle changed about a character — HP spent, experience
    /// and levels won, `gain_exp_flag`, death — is in the returned [`Stats`],
    /// because it is the same record that went in. Nothing needs translating
    /// and nothing may be dropped: a caller that takes this and writes it back
    /// over `GameState`'s copy has round-tripped the battle correctly by
    /// construction.
    ///
    /// Enemies are discarded; they exist only for the length of the fight.
    #[must_use]
    pub fn into_party(self) -> Vec<PartyMember> {
        self.roster
            .into_iter_fighters()
            .filter_map(|fighter| {
                fighter.character.map(|character| PartyMember {
                    character,
                    name: fighter.name,
                    stats: fighter.stats,
                })
            })
            .collect()
    }

    /// How the battle finished, if it has.
    #[must_use]
    pub const fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// How many rounds have been resolved.
    #[must_use]
    pub const fn round_number(&self) -> u16 {
        self.round
    }

    /// The pools the dead have contributed so far.
    #[must_use]
    pub const fn pools(&self) -> Pools {
        self.pools
    }

    /// Whether this battle uses the saved vehicle fighter surface.
    #[must_use]
    pub const fn is_vehicle(&self) -> bool {
        self.vehicle
    }

    /// Resolves one round.
    ///
    /// Returns an empty timeline once the battle has ended.
    ///
    /// # Errors
    /// [`BattleDataError`] for an equipment or level-table lookup that fails.
    pub fn round(
        &mut self,
        orders: &RoundOrders,
        data: &BattleData,
        rolls: &mut impl Rolls,
    ) -> Result<Vec<BattleEvent>, BattleDataError> {
        if self.outcome.is_some() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        self.round += 1;

        let mut priority = self.pending_priority;
        self.pending_priority = Priority::Normal;

        if matches!(orders, RoundOrders::Run) {
            match self.try_escape(priority, rolls) {
                Escape::Succeeded => {
                    events.push(BattleEvent::Escaped);
                    self.outcome = Some(Outcome::Escaped);
                    events.push(BattleEvent::Ended {
                        outcome: Outcome::Escaped,
                    });
                    return Ok(events);
                }
                Escape::Failed => {
                    events.push(BattleEvent::EscapeFailed);
                    // `Battle_RunFailMsg`'s tail: `st (Battle_Priority).w`.
                    priority = Priority::Ambush;
                }
            }
        }

        let queue = build_queue(&self.roster, priority, rolls);

        // `loc_56A8`: four `Enemy_TargetCharacter` calls, one per enemy slot,
        // occupied or not — four rolls regardless of how many enemies live.
        let living = targetable_party(&self.roster);
        let enemy_targets: Vec<Option<FighterId>> = (0..ENEMY_SLOTS)
            .map(|_| choose_target(&living, rolls))
            .collect();

        events.push(BattleEvent::RoundBegan {
            round: self.round,
            order: queue.iter().map(|entry| entry.fighter).collect(),
        });

        for entry in &queue {
            if self.settle_outcome().is_some() {
                break;
            }
            self.take_turn(entry, orders, &enemy_targets, data, rolls, &mut events)?;
        }

        // `Battle_RestoreStatsAtTurnEnd` — Defend wears off here, which is why
        // it only protects against attacks that land after the defender's turn.
        for fighter in self.roster.iter_mut() {
            fighter.stats.restore_physical_prop();
        }
        events.push(BattleEvent::RoundEnded { round: self.round });

        if let Some(outcome) = self.settle_outcome() {
            self.outcome = Some(outcome);
            if outcome == Outcome::Victory {
                // Computed, not applied. The roster owns both award passes and
                // the level-up rise that reads what they wrote — see
                // `rewards`'s module note on where the seam sits.
                let split = split_rewards(&self.roster, self.pools, self.vehicle);
                events.push(BattleEvent::Rewarded {
                    experience_total: split.total,
                    experience_each: split.each,
                    meseta: split.meseta,
                    recipients: split.recipients,
                });
            }
            events.push(BattleEvent::Ended { outcome });
        }
        Ok(events)
    }

    fn take_turn(
        &mut self,
        entry: &QueueEntry,
        orders: &RoundOrders,
        enemy_targets: &[Option<FighterId>],
        data: &BattleData,
        rolls: &mut impl Rolls,
        events: &mut Vec<BattleEvent>,
    ) -> Result<(), BattleDataError> {
        let actor = entry.fighter;
        let Some(fighter) = self.roster.get(actor) else {
            return Ok(());
        };
        // The queue was built at the top of the round; anything can have
        // happened since.
        if !fighter.is_alive() {
            events.push(BattleEvent::TurnSkipped {
                actor,
                reason: Skipped::Dead,
            });
            return Ok(());
        }
        if !fighter.stats.can_act() {
            events.push(BattleEvent::TurnSkipped {
                actor,
                reason: Skipped::Incapacitated,
            });
            return Ok(());
        }

        let intended = match actor.side() {
            Side::Party => match orders.command_for(actor.slot()) {
                Command::Defend => {
                    let fighter = self.roster.get_mut(actor).expect("present");
                    fighter.stats.begin_defending();
                    events.push(BattleEvent::Defended { actor });
                    return Ok(());
                }
                Command::Attack => None,
                Command::AttackTarget(target) => Some(target),
                Command::VehicleSkill(skill) => {
                    let valid = self.vehicle
                        && skill
                            .checked_sub(1)
                            .and_then(|slot| {
                                fighter
                                    .stats
                                    .skills
                                    .get(slot as usize)
                                    .zip(fighter.stats.curr_skill_uses.get(slot as usize))
                            })
                            .is_some_and(|(&known, &uses)| known == skill && uses > 0);
                    if !valid {
                        events.push(BattleEvent::VehicleSkillRejected { actor, skill });
                        return Ok(());
                    }

                    let fighter = self.roster.get_mut(actor).expect("present");
                    let slot = usize::from(skill - 1);
                    fighter.stats.curr_skill_uses[slot] -= 1;
                    events.push(BattleEvent::VehicleSkillUsed {
                        actor,
                        skill,
                        remaining: fighter.stats.curr_skill_uses[slot],
                    });
                    // The full VehicleSkillData effect dispatcher is Tier 2.
                    // Do not call resolve_attack: the UI-visible use decrement
                    // is real, while fake physical damage would corrupt battle
                    // parity and make the open effect seam invisible.
                    events.push(BattleEvent::VehicleSkillEffectUnavailable { actor, skill });
                    return Ok(());
                }
            },
            Side::Enemy => {
                self.roll_enemy_ability(actor, data, rolls, events)?;
                // Enemy slot 6..=9 maps to enemy_targets 0..=3.
                enemy_targets
                    .get(actor.slot() - super::fighters::PARTY_SLOTS)
                    .copied()
                    .flatten()
            }
        };

        let died = resolve_attack(&mut self.roster, actor, intended, data, rolls, events)?;
        for fighter in died {
            if fighter.side() == Side::Enemy {
                let record = self
                    .roster
                    .get(fighter)
                    .map(|f| f.stats.enemy_id)
                    .and_then(|id| data.enemy(id).ok());
                if let Some(record) = record {
                    self.pools.add(record.experience, record.meseta);
                }
            }
        }
        Ok(())
    }

    /// `Enemy_Attack`'s opening — the ability roll, always taken.
    ///
    /// Tier 1 implements the plain attack (ability `0`) only. When the roll
    /// lands on a real ability the enemy still swings physically, and an
    /// [`BattleEvent::UnsupportedAbility`] says so rather than letting a wrong
    /// number pass for a right one. 53 of the cartridge's 153 enemies —
    /// including everything in both oracle tapes — carry an all-zero list and
    /// never reach that branch.
    fn roll_enemy_ability(
        &mut self,
        actor: FighterId,
        data: &BattleData,
        rolls: &mut impl Rolls,
        events: &mut Vec<BattleEvent>,
    ) -> Result<(), BattleDataError> {
        let enemy_id = self.roster.get(actor).map_or(0, |f| f.stats.enemy_id);
        let record = data.enemy(enemy_id)?;
        // Tier 2 walks `condition_ids` here and can substitute a conditional
        // ability; Tier 1's conditions never fire.
        let (_, ability) = choose_ability(record, &mut self.last_ability_index, rolls);
        if let Some(fighter) = self.roster.get_mut(actor) {
            fighter.ability = ability;
        }
        if ability != 0 {
            events.push(BattleEvent::UnsupportedAbility { actor, ability });
        }
        Ok(())
    }

    /// `Battle_ProcessRUN` — `ps4.asm:7672`.
    ///
    /// Four short-circuits before the roll, in the cartridge's order:
    /// nobody able to act fails outright; a preemptive strike escapes for
    /// free; an enemy side with nobody able to act escapes for free; and a
    /// formation whose run chance is `$F0` or more can never be escaped.
    /// Only the fall-through draws a roll.
    fn try_escape(&self, priority: Priority, rolls: &mut impl Rolls) -> Escape {
        if !self.roster.side(Side::Party).any(|f| f.stats.can_act()) {
            return Escape::Failed;
        }
        if priority == Priority::Preemptive {
            return Escape::Succeeded;
        }
        if !self.roster.side(Side::Enemy).any(|f| f.stats.can_act()) {
            return Escape::Succeeded;
        }
        let Some(chance) = self.run_chance else {
            return Escape::Failed;
        };
        let (scale, fail) = ESCAPE;
        // `d5` is never initialised at this call site; only the sign of the
        // result is read, and both non-negative verdicts mean the same thing.
        let verdict = calculate_chances(
            i16::from(self.roster.highest_party_agility()),
            i16::from(chance),
            scale,
            fail,
            0,
            rolls,
        );
        if verdict == Verdict::Miss {
            Escape::Failed
        } else {
            Escape::Succeeded
        }
    }

    fn settle_outcome(&self) -> Option<Outcome> {
        if !self.roster.any_alive(Side::Enemy) {
            Some(Outcome::Victory)
        } else if !self.roster.any_alive(Side::Party) {
            Some(Outcome::Defeat)
        } else {
            None
        }
    }
}

enum Escape {
    Succeeded,
    Failed,
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
