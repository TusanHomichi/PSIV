//! Victory: the experience and meseta pools, and the level-up pass.
//!
//! Two of the ratified bug fixes live here. Both are in
//! `docs/RUNTIME_DESIGN.md` "Battle bug policy"; each is marked at the line it
//! changes.

use super::event::BattleEvent;
use super::fighters::{FighterId, Roster, Side};
use super::records::{BattleData, BattleDataError};

/// The ceiling on both pools: `add.w` then `bcc` to `$FFFF` on carry
/// (`ps4.asm:59635`).
pub const POOL_CAP: u16 = u16::MAX;

/// The ceiling on a character's experience and on the party's purse
/// (`ps4.asm:4752`).
pub const CURRENCY_CAP: u32 = 9_999_999;

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

/// How the pool was split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The pool before dividing.
    pub total: u16,
    /// What each living member received.
    pub each: u16,
    /// Meseta added to the purse.
    pub meseta: u16,
    /// Who collected, in id order.
    pub recipients: Vec<FighterId>,
}

/// `Battle_VictoryMessage`'s reward pass — `ps4.asm:4705`.
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
/// battle gets nothing and does not dilute the share.
///
/// # Not implemented
///
/// `ps4.asm:4757-4782` also pays characters *outside* the party who have
/// `gain_exp_flag` set, gated on `EventFlag_Reunion` being clear. That needs
/// the roster of everyone recruited so far, which battle does not own.
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

/// Awards the split and returns the events.
///
/// Experience is capped at [`CURRENCY_CAP`] per character, and every living
/// member's `gain_exp_flag` is set whether or not they gained anything —
/// `st gain_exp_flag(a0)` runs before the status check (`ps4.asm:4747`).
pub fn award(roster: &mut Roster, split: &Split) -> Vec<BattleEvent> {
    for id in &split.recipients {
        let Some(fighter) = roster.get_mut(*id) else {
            continue;
        };
        fighter.stats.gain_exp_flag = true;
        fighter.stats.experience = fighter
            .stats
            .experience
            .saturating_add(u32::from(split.each))
            .min(CURRENCY_CAP);
    }
    vec![BattleEvent::Rewarded {
        experience_total: split.total,
        experience_each: split.each,
        meseta: split.meseta,
        recipients: split.recipients.clone(),
    }]
}

/// `BattleResults_PartyExp` — `ps4.asm:5993`.
///
/// Walks the party in slot order and gives each member **at most one level**,
/// which is the cartridge's behaviour and is kept.
///
/// # Two ratified fixes
///
/// 1. **The level-99 pointer desync.** Retail's early-out for a level-99
///    character jumps past the two `(a3)+` post-increments that walk
///    `CharLevelTablePtrs`, so every character *after* a maxed one in the loop
///    reads somebody else's level table — the wrong experience thresholds and
///    the wrong stats. Here each member's table is looked up by id, so the
///    desync cannot happen. (`docs/RUNTIME_DESIGN.md` "Battle bug policy".)
/// 2. **Stats not refreshed after a level-up.** Retail never re-runs
///    `UpdateCharModStats`, so a character's equipment-derived attack and
///    defence lag one level behind until something else refreshes them. This
///    calls [`Stats::update_mod_stats`](super::stats::Stats::update_mod_stats)
///    on the way out.
///
/// # Errors
/// [`BattleDataError::UnknownLevelTable`] when a party member has no table.
pub fn apply_level_ups(
    roster: &mut Roster,
    data: &BattleData,
) -> Result<Vec<BattleEvent>, BattleDataError> {
    let item = |id: u8| data.item(id).ok().cloned();
    let members: Vec<(FighterId, u8)> = roster
        .side(Side::Party)
        .filter_map(|f| f.character.map(|c| (f.id, c)))
        .collect();

    let mut events = Vec::new();
    for (id, character) in members {
        let table = data.level_table(character)?;
        let Some(fighter) = roster.get_mut(id) else {
            continue;
        };
        // FIX 1: retail's `beq.w loc_3FC0` skips the pointer advance here. This
        // loop indexes per character, so a maxed member cannot corrupt the next.
        if fighter.stats.level >= MAX_LEVEL {
            continue;
        }
        let Some(record) = table.next_after(fighter.stats.level) else {
            continue;
        };
        if fighter.stats.experience < record.experience_required {
            continue;
        }

        fighter.stats.level += 1;
        fighter.stats.max_hp = record.hp;
        fighter.stats.max_tp = record.tp;
        fighter.stats.strength.base = record.strength;
        fighter.stats.mental.base = record.mental;
        fighter.stats.agility.base = record.agility;
        fighter.stats.dexterity.base = record.dexterity;
        // FIX 2: retail leaves the derived stats stale until the next refresh.
        fighter.stats.update_mod_stats(&item);

        events.push(BattleEvent::LevelUp {
            character,
            level: fighter.stats.level,
            max_hp: record.hp,
            max_tp: record.tp,
        });
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures;
    use crate::battle::records::ItemRecord;
    use crate::battle::stats::{Stats, status};

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

        // Kill one: the share goes up, and the corpse collects nothing.
        let hahn = FighterId::new(3).expect("id 3");
        roster.get_mut(hahn).expect("Hahn").stats.status = status::DEAD;
        let split = split_rewards(&roster, pools, false);
        assert_eq!(split.each, 12, "25 / 2");
        assert_eq!(split.recipients.len(), 2);
        assert!(!split.recipients.contains(&hahn));
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
    fn awarding_sets_the_gain_flag_and_caps_the_total() {
        let mut roster = three_member_party();
        let split = Split {
            total: 24,
            each: 8,
            meseta: 6,
            recipients: (1..=3).map(|n| FighterId::new(n).expect("an id")).collect(),
        };
        let events = award(&mut roster, &split);
        assert_eq!(events.len(), 1);
        for id in &split.recipients {
            let stats = &roster.get(*id).expect("present").stats;
            assert_eq!(stats.experience, 8);
            assert!(stats.gain_exp_flag);
        }

        let chaz = FighterId::new(1).expect("id 1");
        roster.get_mut(chaz).expect("Chaz").stats.experience = CURRENCY_CAP;
        award(&mut roster, &split);
        assert_eq!(
            roster.get(chaz).expect("Chaz").stats.experience,
            CURRENCY_CAP,
            "capped, not wrapped"
        );
    }

    #[test]
    fn a_level_up_needs_the_threshold_and_happens_once() {
        let mut roster = three_member_party();
        let data = fixtures::data();
        let chaz = FighterId::new(1).expect("id 1");

        // Chaz needs 21 for level 2 and 109 for level 3.
        roster.get_mut(chaz).expect("Chaz").stats.experience = 20;
        let events = apply_level_ups(&mut roster, &data).expect("levels up");
        assert!(events.is_empty(), "one short of the threshold");
        assert_eq!(roster.get(chaz).expect("Chaz").stats.level, 1);

        roster.get_mut(chaz).expect("Chaz").stats.experience = 200;
        let events = apply_level_ups(&mut roster, &data).expect("levels up");
        assert_eq!(
            events,
            vec![BattleEvent::LevelUp {
                character: 0,
                level: 2,
                max_hp: 31,
                max_tp: 13,
            }],
            "one level per battle even with experience for two"
        );
        assert_eq!(roster.get(chaz).expect("Chaz").stats.level, 2);

        // The next battle takes the second level.
        let events = apply_level_ups(&mut roster, &data).expect("levels up");
        assert_eq!(events.len(), 1);
        assert_eq!(roster.get(chaz).expect("Chaz").stats.level, 3);
    }

    #[test]
    fn a_level_up_refreshes_the_derived_stats() {
        // FIX 2. Chaz at level 2 has strength 9, so his attack must become
        // 9 + 5 + 5 = 19 immediately rather than staying at 18.
        let mut roster = three_member_party();
        let data = fixtures::data();
        let chaz = FighterId::new(1).expect("id 1");
        assert_eq!(roster.get(chaz).expect("Chaz").stats.attack.battle, 18);

        roster.get_mut(chaz).expect("Chaz").stats.experience = 21;
        apply_level_ups(&mut roster, &data).expect("levels up");

        let stats = &roster.get(chaz).expect("Chaz").stats;
        assert_eq!(stats.strength.base, 9);
        assert_eq!(stats.attack.battle, 19, "refreshed, not stale at 18");
        assert_eq!(stats.defence.battle, 11, "agility 8 + helm 1 + cloth 2");
        assert_eq!(stats.max_hp, 31);
        assert_eq!(stats.curr_hp, 25, "a level-up does not heal");
    }

    #[test]
    fn a_maxed_character_cannot_corrupt_the_next_one() {
        // FIX 1. Retail's early-out leaves the level-table pointer where it
        // was, so Alys would be read out of Chaz's table. Here each member is
        // looked up by id, so pinning Chaz at 99 leaves Alys correct.
        let mut roster = three_member_party();
        let data = fixtures::data();
        let chaz = FighterId::new(1).expect("id 1");
        let alys = FighterId::new(2).expect("id 2");

        roster.get_mut(chaz).expect("Chaz").stats.level = MAX_LEVEL;
        roster.get_mut(chaz).expect("Chaz").stats.experience = CURRENCY_CAP;
        roster.get_mut(alys).expect("Alys").stats.experience = 100;

        let events = apply_level_ups(&mut roster, &data).expect("levels up");
        assert_eq!(
            events,
            vec![BattleEvent::LevelUp {
                character: 1,
                level: 8,
                max_hp: 60,
                max_tp: 44,
            }],
            "Alys reads her own table, starting level 7"
        );
        assert_eq!(
            roster.get(chaz).expect("Chaz").stats.level,
            MAX_LEVEL,
            "and the maxed character stays put"
        );
    }

    #[test]
    fn a_character_past_the_end_of_their_table_simply_stops() {
        let mut roster = three_member_party();
        let data = fixtures::data();
        let alys = FighterId::new(2).expect("id 2");
        // Alys's fixture table holds one record, for level 8.
        roster.get_mut(alys).expect("Alys").stats.level = 8;
        roster.get_mut(alys).expect("Alys").stats.experience = CURRENCY_CAP;
        let events = apply_level_ups(&mut roster, &data).expect("levels up");
        assert!(events.is_empty());
        assert_eq!(roster.get(alys).expect("Alys").stats.level, 8);
    }
}
