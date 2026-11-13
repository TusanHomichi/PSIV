//! Whether a mounted party RUNs: the live battle, rolled forward on copies
//! with the engine's own rounds.
//!
//! On foot, a running temperament RUNs from every random encounter to keep the
//! party's HP, TP and skill uses for the walk on (`policy.rs`). Mounted, a
//! battle's cost ends with it: the vehicle starts every battle at its static
//! HP and only its skill uses are copied back (`ps4.asm:11309-11310`,
//! `ps4.asm:6372-6389`; `docs/field/VEHICLES.md`), and the driver's ATTAC
//! spends no use. A RUN saves nothing a later battle needs, so the only
//! question is which choice loses the vehicle less often: a RUN that fails
//! hands the whole round to the enemies (`Battle_ProcessRUN`,
//! `ps4.asm:7672`), and a single machine has no one to cure it.
//!
//! [`weigh`] answers it by playing the battle out from where it
//! stands: [`RUN_ROLLOUTS`] copies that RUN every round and [`FIGHT_ROLLOUTS`]
//! copies that ATTAC every round, each through `Battle::round`, so the escape
//! odds, the enemies' abilities and damage and the vehicle's own swing are the
//! engine's rules rather than restated ones. The copies draw from a local
//! [`Lcg41`] with a fixed seed per copy, so the answer is a function of the
//! battle's state alone: the session's random stream is never read or
//! advanced, and the same state always gets the same answer.
//!
//! The party fights only when the copies lose strictly less often fighting.
//! A battle neither choice loses (a free escape, a harmless formation) is run
//! from, as before.

use psiv_core::battle::{Battle, BattleData, Lcg41, Outcome, RoundOrders};

/// Copies that RUN every round. Most end in the first round, so they are
/// cheap, and a loss needs several failed escapes in a row: enough copies to
/// see it.
pub const RUN_ROLLOUTS: u32 = 4096;

/// Copies that ATTAC every round.
pub const FIGHT_ROLLOUTS: u32 = 1024;

/// A copy still going after this many rounds counts as no loss.
pub const ROLLOUT_ROUNDS: u16 = 32;

/// How often one choice lost the vehicle across its copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Losses {
    /// Copies that ended in a defeat.
    pub defeats: u32,
    /// Copies played.
    pub copies: u32,
}

impl Losses {
    /// Whether `self` lost strictly less often than `other`.
    #[must_use]
    pub fn less_than(self, other: Losses) -> bool {
        u64::from(self.defeats) * u64::from(other.copies)
            < u64::from(other.defeats) * u64::from(self.copies)
    }
}

/// Both choices' losses from the battle as it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    /// RUN every round.
    pub run: Losses,
    /// ATTAC every round.
    pub fight: Losses,
}

impl Choice {
    /// Whether fighting lost strictly less often than running.
    #[must_use]
    pub fn fight(self) -> bool {
        self.fight.less_than(self.run)
    }
}

/// Plays `copies` copies of `battle` with `orders` every round.
#[must_use]
pub fn losses(battle: &Battle, data: &BattleData, orders: &RoundOrders, copies: u32) -> Losses {
    let mut defeats = 0;
    for copy in 0..copies {
        // Spread the seeds over the generator's period; any fixed set would
        // do, so long as it never comes from the session.
        let mut rolls = Lcg41::new(0x9E37_79B9_u32.wrapping_mul(copy + 1) | 1);
        let mut copy = battle.clone();
        for _ in 0..ROLLOUT_ROUNDS {
            if copy.outcome().is_some() || copy.round(orders, data, &mut rolls).is_err() {
                break;
            }
        }
        if copy.outcome() == Some(Outcome::Defeat) {
            defeats += 1;
        }
    }
    Losses { defeats, copies }
}

/// RUN against ATTAC from `battle` as it stands.
#[must_use]
pub fn weigh(battle: &Battle, data: &BattleData) -> Choice {
    Choice {
        run: losses(battle, data, &RoundOrders::Run, RUN_ROLLOUTS),
        fight: losses(battle, data, &RoundOrders::attack_all(), FIGHT_ROLLOUTS),
    }
}

#[cfg(test)]
mod tests {
    use psiv_core::VehicleRecord;
    use psiv_core::battle::{
        ELEMENT_SLOTS, EnemyRecord, FormationEnemy, FormationRecord, SliceRolls,
    };

    use super::*;

    /// A synthetic enemy that only swings (no ability ids), with the given HP
    /// and attack. The numbers are invented for the test, not read from the
    /// cartridge.
    fn swinger(id: u16, hp: u16, attack: u16) -> EnemyRecord {
        EnemyRecord {
            id,
            name: format!("E{id}"),
            hp,
            strength: 40,
            mental: 0,
            agility: 30,
            dexterity: 60,
            attack,
            defence: 20,
            mental_defence: 10,
            attack_element: 1,
            attack_status: 0,
            properties: [2; ELEMENT_SLOTS],
            regular_abilities: [0; 8],
            condition_ids: [0; 4],
            conditional_abilities: [0; 4],
            experience: 1,
            meseta: 1,
        }
    }

    fn formation(enemy: u16, count: u8, run_chance: u8) -> FormationRecord {
        FormationRecord {
            id: 900,
            // Against the Ice Digger's agility 48, a byte of 40 opens neither
            // an ambush nor a preemptive strike on the start roll below.
            ambush_chance: 40,
            run_chance,
            drop_rate: 0,
            drop_item: None,
            enemies: (1..=count)
                .map(|slot| FormationEnemy {
                    slot,
                    enemy_id: enemy,
                    position: slot * 8,
                })
                .collect(),
        }
    }

    /// The Ice Digger (`Vehicle_Index` 2) as a battle starts it.
    fn mounted(formation: &FormationRecord, data: &BattleData) -> Battle {
        let digger = psiv_core::vehicle::battle_member(2, VehicleRecord::default())
            .expect("the Ice Digger's profile");
        // A middle roll: a plain start (a preemptive strike escapes for free).
        let mut rolls = SliceRolls::new(&[32]);
        Battle::start_vehicle(formation, vec![digger], data, &mut rolls)
            .expect("the battle starts")
            .0
    }

    /// Four weak enemies that hit hard enough to fell the vehicle in a few
    /// rounds of failed escapes: one swing kills each, so fighting removes the
    /// fire that running only hopes to leave.
    #[test]
    fn a_mounted_party_fights_what_it_beats_more_safely_than_it_flees() {
        let data = BattleData::new().with_enemies([swinger(1, 60, 350)]);
        let battle = mounted(&formation(1, 4, 40), &data);
        let choice = weigh(&battle, &data);
        assert!(choice.run.defeats > 0, "{choice:?}");
        assert!(choice.fight(), "{choice:?}");
    }

    /// Negative control: the same four enemies hitting harder, so hard that a
    /// fighting vehicle falls before it can clear them. The escape odds are
    /// the same as above, and running stays the choice.
    #[test]
    fn a_mounted_party_still_runs_from_what_it_cannot_outlast() {
        let data = BattleData::new().with_enemies([swinger(1, 60, 500)]);
        let battle = mounted(&formation(1, 4, 40), &data);
        let choice = weigh(&battle, &data);
        assert!(choice.run.defeats > 0, "{choice:?}");
        assert!(!choice.fight(), "{choice:?}");
    }

    /// Negative control: a formation neither choice loses to is run from.
    #[test]
    fn a_harmless_formation_is_still_run_from() {
        let data = BattleData::new().with_enemies([swinger(1, 60, 1)]);
        let battle = mounted(&formation(1, 1, 40), &data);
        let choice = weigh(&battle, &data);
        assert_eq!((choice.run.defeats, choice.fight.defeats), (0, 0));
        assert!(!choice.fight(), "a tie runs");
    }

    #[test]
    fn the_comparison_is_strict_and_scaled_by_copies() {
        let at = |defeats, copies| Losses { defeats, copies };
        assert!(at(0, 1024).less_than(at(1, 4096)));
        assert!(!at(1, 1024).less_than(at(4, 4096)), "equal rates tie");
        assert!(at(1, 1024).less_than(at(5, 4096)));
        assert!(!at(0, 1024).less_than(at(0, 4096)));
    }
}
