//! Victory: the experience and meseta pools, and the level-up rise.
//!
//! # Where the seam is
//!
//! `Battle_VictoryMessage`'s reward pass is two loops over the *roster*, not
//! over the fighters in the battle, and it sets `gain_exp_flag` on characters
//! it then refuses to pay. Battle cannot run it: it holds five fighter slots,
//! not eleven character records, and it has never heard of `EventFlag_Reunion`.
//!
//! So this module computes and the roster applies:
//!
//! | | owner | why |
//! |---|---|---|
//! | the pools | battle | only battle sees which enemies died |
//! | the living-member divisor, the vehicle halving | battle | in-battle facts |
//! | `gain_exp_flag`, both award passes, the cap | [`crate::roster`] | needs all eleven records and the Reunion flag |
//! | the level-up rise | [`level_up`], called by the roster | needs the level tables *and* the just-awarded experience |
//!
//! **Battle mutates no character's experience, flag or level.** It emits
//! [`BattleEvent::Rewarded`] carrying the per-head share and stops. That is not
//! tidiness: the two award passes are one routine, and a routine split across
//! two layers is a routine with two half-right answers. Battle's own recipient
//! list is the *living*, so a battle-side award silently skips the character
//! who died in their first fight — who on the cartridge earns the flag and, once
//! benched, keeps getting paid.
//!
//! # Ordering, which is load-bearing
//!
//! The level-up rise reads experience the award just added
//! (`ps4.asm:4705` then `ps4.asm:5993`, adjacent stages of the same results
//! routine). At battle end the caller must therefore run, in order:
//!
//! 1. `CharacterRoster::absorb(battle.into_party())` — HP, TP, status
//! 2. `GameState::award_experience(split.each)` — both passes
//! 3. [`level_up`] per party member — reading the experience step 2 wrote
//!
//! Running 3 before 2 levels nobody; running it against battle's copies levels
//! off stale numbers.

use super::event::BattleEvent;
use super::fighters::{FighterId, Roster, Side};
use super::records::{BattleData, BattleDataError};
use super::stats::Stats;

/// The ceiling on both pools: `add.w` then `bcc` to `$FFFF` on carry
/// (`ps4.asm:59635`).
pub const POOL_CAP: u16 = u16::MAX;

/// The level at which the results loop stops looking (`ps4.asm:6008`).
pub const MAX_LEVEL: u16 = 99;

/// What the enemies left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pools {
    /// `$FFFF41CE`.
    pub experience: u16,
    /// `$FFFF41D0`.
    pub meseta: u16,
}

impl Pools {
    /// `loc_2D960` (`$0002D960`) — one enemy's contribution, saturating.
    ///
    /// The cartridge writes `$FFFF` on carry rather than wrapping, so a very
    /// long fight caps rather than resetting the reward.
    pub const fn add(&mut self, experience: u16, meseta: u16) {
        self.experience = self.experience.saturating_add(experience);
        self.meseta = self.meseta.saturating_add(meseta);
    }
}

/// How the pool divides.
///
/// A computation, not an application: nothing here has been paid to anyone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The pool before dividing.
    pub total: u16,
    /// The per-head share. **This is what both award passes add** — the
    /// cartridge divides once and the out-of-party pass reuses the quotient.
    pub each: u16,
    /// Meseta for the party's purse.
    pub meseta: u16,
    /// The living party members the divisor counted, in id order. Who actually
    /// gets paid is the roster's call, and it is not the same list — a dead
    /// member still earns `gain_exp_flag`.
    pub recipients: Vec<FighterId>,
}

/// `Battle_VictoryMessage`'s division — `ps4.asm:4705`.
///
/// ```text
///     ; d2 = party members whose status has no bit in $44
///     move.w  ($FFFF41CE).l, d1
///     tst.w   (Vehicle_Index).w
///     beq.s   +
///     lsr.w   #1, d1              ; a vehicle battle pays half
/// +   divu.w  d2, d1
///     andi.l  #$FFFF, d1          ; the quotient; the remainder is discarded
/// ```
///
/// The divisor is the **living** count, not the party size — the oracle saw
/// `24 / 3 = 8` and `21 / 3 = 7` with three alive. A member who died during the
/// battle does not dilute the share.
#[must_use]
pub fn split_rewards(roster: &Roster, pools: Pools, vehicle: bool) -> Split {
    let recipients: Vec<FighterId> = roster.living(Side::Party).map(|f| f.id).collect();
    let total = if vehicle {
        pools.experience >> 1
    } else {
        pools.experience
    };
    let each = if recipients.is_empty() {
        // `divu.w` by zero traps on a 68000. Unreachable: rewards are paid on
        // victory, which requires a survivor.
        0
    } else {
        total / recipients.len() as u16
    };
    Split {
        total: pools.experience,
        each,
        meseta: pools.meseta,
        recipients,
    }
}

/// `BattleResults_PartyExp`'s rise — `ps4.asm:5993`.
///
/// Applies **at most one level** to one character and returns the event, or
/// `None` when they are maxed, past the end of their table, or short of the
/// threshold. One level per battle per character is the cartridge's behaviour
/// and is kept, so a caller must call this once per character per battle and
/// must not loop it.
///
/// `stats.experience` must already hold what the award added — see the module
/// note on ordering.
///
/// # Two ratified fixes
///
/// 1. **The level-99 pointer desync.** Retail's early-out for a maxed character
///    jumps past the two `(a3)+` post-increments that walk `CharLevelTablePtrs`,
///    so every character *after* a maxed one reads somebody else's level table.
///    This takes the table by character id, so the desync cannot happen.
/// 2. **Stats not refreshed after a level-up.** Retail never re-runs
///    `UpdateCharModStats`, and the oracle watched `atk_pow` sit at 18 for 600
///    frames while strength went 8 to 9. This calls
///    [`Stats::update_mod_stats`] on the way out.
///
/// Both are in `docs/RUNTIME_DESIGN.md` "Battle bug policy".
///
/// # Errors
/// [`BattleDataError::UnknownLevelTable`] when the character has no table.
pub fn level_up(
    character: u8,
    stats: &mut Stats,
    data: &BattleData,
) -> Result<Option<BattleEvent>, BattleDataError> {
    let table = data.level_table(character)?;
    // FIX 1: the table came from an id, not from a walking pointer.
    if stats.level >= MAX_LEVEL {
        return Ok(None);
    }
    let Some(record) = table.next_after(stats.level).copied() else {
        return Ok(None);
    };
    if stats.experience < record.experience_required {
        return Ok(None);
    }

    stats.level += 1;
    stats.max_hp = record.hp;
    stats.max_tp = record.tp;
    stats.strength.base = record.strength;
    stats.mental.base = record.mental;
    stats.agility.base = record.agility;
    stats.dexterity.base = record.dexterity;
    // FIX 2: retail leaves the derived stats stale until the next refresh.
    let item = |id: u8| data.item(id).ok().cloned();
    stats.update_mod_stats(&item);

    Ok(Some(BattleEvent::LevelUp {
        character,
        level: stats.level,
        max_hp: record.hp,
        max_tp: record.tp,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures;
    use crate::battle::records::ItemRecord;
    use crate::battle::stats::status;

    fn lookup(id: u8) -> Option<ItemRecord> {
        fixtures::items().into_iter().find(|i| i.id == id)
    }

    fn three_member_party() -> Roster {
        let mut roster = Roster::new();
        for (index, record) in [fixtures::chaz(), fixtures::alys(), fixtures::hahn()]
            .iter()
            .enumerate()
        {
            roster.add_party_member(
                index as u8,
                record.name.clone(),
                Stats::from_character(record, lookup),
            );
        }
        roster
    }

    #[test]
    fn the_pools_are_the_sum_of_what_died() {
        let mut pools = Pools::default();
        pools.add(12, 3);
        assert_eq!(
            pools,
            Pools {
                experience: 12,
                meseta: 3
            }
        );
        pools.add(12, 3);
        assert_eq!(
            pools,
            Pools {
                experience: 24,
                meseta: 6
            },
            "tape 07: two ZoranBult"
        );
    }

    #[test]
    fn the_pools_saturate_rather_than_wrapping() {
        let mut pools = Pools {
            experience: POOL_CAP - 1,
            meseta: POOL_CAP,
        };
        pools.add(100, 100);
        assert_eq!(pools.experience, POOL_CAP);
        assert_eq!(pools.meseta, POOL_CAP);
    }

    #[test]
    fn tape_07_splits_twenty_four_three_ways() {
        // The oracle: two ZoranBult at 12 experience and 3 meseta each, a party
        // of three, each gaining +8 and the purse +6.
        let roster = three_member_party();
        let pools = Pools {
            experience: 24,
            meseta: 6,
        };
        let split = split_rewards(&roster, pools, false);
        assert_eq!(split.total, 24);
        assert_eq!(split.each, 8);
        assert_eq!(split.meseta, 6);
        assert_eq!(split.recipients.len(), 3);
    }

    #[test]
    fn tape_09_splits_twenty_one_three_ways() {
        // Xanafalgue 9 + ZoranBult 12 = 21, and meseta 2 + 3 = 5.
        let roster = three_member_party();
        let mut pools = Pools::default();
        pools.add(9, 2);
        pools.add(12, 3);
        let split = split_rewards(&roster, pools, false);
        assert_eq!((split.total, split.each, split.meseta), (21, 7, 5));
    }

    #[test]
    fn the_divisor_is_the_living_count_and_the_remainder_is_dropped() {
        let mut roster = three_member_party();
        let pools = Pools {
            experience: 25,
            meseta: 0,
        };
        assert_eq!(split_rewards(&roster, pools, false).each, 8, "25 / 3");

        // Kill one: the share goes up, and the corpse is not in the divisor.
        let hahn = FighterId::new(3).expect("id 3");
        roster.get_mut(hahn).expect("Hahn").stats.status = status::DEAD;
        let split = split_rewards(&roster, pools, false);
        assert_eq!(split.each, 12, "25 / 2");
        assert_eq!(split.recipients.len(), 2);
        assert!(
            !split.recipients.contains(&hahn),
            "the divisor counts the living; whether the dead earn the flag is \
             the roster's business, not this list's"
        );
    }

    #[test]
    fn a_vehicle_battle_pays_half_before_dividing() {
        let roster = three_member_party();
        let pools = Pools {
            experience: 24,
            meseta: 6,
        };
        assert_eq!(split_rewards(&roster, pools, true).each, 4, "12 / 3");
        assert_eq!(
            split_rewards(&roster, pools, true).total,
            24,
            "the pool itself is unchanged"
        );
    }

    #[test]
    fn splitting_pays_nobody() {
        // The whole point of the seam: this is arithmetic, not an award.
        let roster = three_member_party();
        let before: Vec<(u32, bool)> = roster
            .side(Side::Party)
            .map(|f| (f.stats.experience, f.stats.gain_exp_flag))
            .collect();
        let split = split_rewards(
            &roster,
            Pools {
                experience: 24,
                meseta: 6,
            },
            false,
        );
        assert_eq!(split.each, 8);
        let after: Vec<(u32, bool)> = roster
            .side(Side::Party)
            .map(|f| (f.stats.experience, f.stats.gain_exp_flag))
            .collect();
        assert_eq!(before, after, "no experience moved, no flag was set");
        assert!(before.iter().all(|(exp, flag)| *exp == 0 && !*flag));
    }

    #[test]
    fn a_level_needs_the_threshold_and_arrives_once() {
        let data = fixtures::data();
        let mut chaz = Stats::from_character(&fixtures::chaz(), lookup);

        chaz.experience = 20;
        assert_eq!(
            level_up(0, &mut chaz, &data).expect("resolves"),
            None,
            "one short of the 21 the level-2 record asks for"
        );
        assert_eq!(chaz.level, 1);

        // Experience enough for two levels still buys exactly one.
        chaz.experience = 200;
        assert_eq!(
            level_up(0, &mut chaz, &data).expect("resolves"),
            Some(BattleEvent::LevelUp {
                character: 0,
                level: 2,
                max_hp: 31,
                max_tp: 13,
            })
        );
        assert_eq!(chaz.level, 2);

        // The next battle takes the second.
        assert!(level_up(0, &mut chaz, &data).expect("resolves").is_some());
        assert_eq!(chaz.level, 3);
    }

    #[test]
    fn a_level_applies_the_record_the_oracle_measured() {
        // Tape 10: str 8 -> 9, agi 7 -> 8, dex 5 -> 6, hp 25 -> 31, tp 10 -> 13.
        let data = fixtures::data();
        let mut chaz = Stats::from_character(&fixtures::chaz(), lookup);
        chaz.experience = 26;
        level_up(0, &mut chaz, &data)
            .expect("resolves")
            .expect("a level");

        assert_eq!(chaz.level, 2);
        assert_eq!(chaz.strength.base, 9);
        assert_eq!(chaz.agility.base, 8);
        assert_eq!(chaz.dexterity.base, 6);
        assert_eq!(chaz.max_hp, 31);
        assert_eq!(chaz.max_tp, 13);
        assert_eq!(chaz.curr_hp, 25, "a level-up does not heal");
    }

    #[test]
    fn a_level_refreshes_the_derived_stats_where_retail_does_not() {
        // FIX 2, stated as a delta from retail. The oracle diffed 600 frames
        // and watched every one of these sit still.
        let data = fixtures::data();
        let mut chaz = Stats::from_character(&fixtures::chaz(), lookup);
        assert_eq!(chaz.attack.derived, 18);
        chaz.experience = 21;
        level_up(0, &mut chaz, &data)
            .expect("resolves")
            .expect("a level");

        assert_eq!(chaz.attack.derived, 19, "FIXED; retail leaves 18");
        assert_eq!(chaz.defence.derived, 11, "FIXED; retail leaves 10");
        assert_eq!(chaz.mental_defence.derived, 7, "FIXED; retail leaves 6");
        assert_eq!(chaz.strength.modified, 9, "FIXED; retail leaves 8");
        assert_eq!(chaz.agility.modified, 8, "FIXED; retail leaves 7");
        assert_eq!(chaz.dexterity.modified, 6, "FIXED; retail leaves 5");
    }

    #[test]
    fn a_maxed_character_levels_nobody_else_wrongly() {
        // FIX 1. Retail's early-out leaves the level-table pointer where it
        // was, so the next character is read out of this one's table. Taking
        // the table by id makes that unrepresentable — Alys reads her own,
        // which starts at 7.
        let data = fixtures::data();
        let mut chaz = Stats::from_character(&fixtures::chaz(), lookup);
        chaz.level = MAX_LEVEL;
        chaz.experience = 9_999_999;
        assert_eq!(level_up(0, &mut chaz, &data).expect("resolves"), None);
        assert_eq!(chaz.level, MAX_LEVEL);

        let mut alys = Stats::from_character(&fixtures::alys(), lookup);
        alys.experience = 100;
        assert_eq!(
            level_up(1, &mut alys, &data).expect("resolves"),
            Some(BattleEvent::LevelUp {
                character: 1,
                level: 8,
                max_hp: 60,
                max_tp: 44,
            }),
            "her table is indexed from 7, not from Chaz's 1"
        );
    }

    #[test]
    fn a_character_past_the_end_of_their_table_simply_stops() {
        let data = fixtures::data();
        let mut alys = Stats::from_character(&fixtures::alys(), lookup);
        alys.level = 8; // the fixture table holds one record, for level 8
        alys.experience = 9_999_999;
        assert_eq!(level_up(1, &mut alys, &data).expect("resolves"), None);
        assert_eq!(alys.level, 8);
    }

    #[test]
    fn a_character_with_no_table_is_an_error_not_a_silent_skip() {
        let data = fixtures::data();
        let mut chaz = Stats::from_character(&fixtures::chaz(), lookup);
        chaz.experience = 9_999_999;
        assert_eq!(
            level_up(9, &mut chaz, &data),
            Err(BattleDataError::UnknownLevelTable(9))
        );
    }
}
