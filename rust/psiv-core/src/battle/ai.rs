//! Enemy decisions: what to do, and to whom.
//!
//! Battle_OrderTurns fills Enemy_Command_Data with targets before the round
//! runs. A dead or newly revived target gets a fresh weighted draw at action
//! time (loc_5ACE). The ability roll follows inside Enemy_Attack.

use super::fighters::{FighterId, Roster, Side};
use super::records::{EnemyRecord, REGULAR_ABILITIES};
use super::rng::Rolls;

/// `EnemyTargetRates` — `ps4.asm:8002`.
///
/// One row per living-party size from two to five, each holding
/// `size - 1` thresholds. The roll is compared with `cmp.b`, so only the low
/// **byte** of the raw word matters — not the six-bit mask the hit roll uses.
///
/// A row is walked front to back and the first slot whose threshold the roll
/// meets or beats is the target; falling off the end takes the last living
/// member. The thresholds descend, so the party's front slot is the most
/// likely target: with five alive it is picked on `roll >= $AC`, about a third
/// of the time.
pub const TARGET_RATES: [&[u8]; 4] = [
    &[0x54],
    &[0x80, 0x2C],
    &[0x99, 0x4D, 0x1A],
    &[0xAC, 0x66, 0x33, 0x12],
];

/// `Enemy_TargetCharacter` — `ps4.asm:7978`.
///
/// `living` is the party's living members in slot order, as `loc_56F0`
/// (`ps4.asm:7952`) collects them: not dead and not android-dead, with sleep
/// and paralysis no obstacle to being hit.
///
/// **Always draws exactly one roll**, including when the party is down to one
/// member and the answer is foregone — the `jsr` sits above the size check.
pub fn choose_target(living: &[FighterId], rolls: &mut impl Rolls) -> Option<FighterId> {
    let roll = rolls.next_roll() as u8;
    let first = *living.first()?;
    // `subq.w #2, d1 / blt` — one living member needs no roll to resolve.
    let Some(row) = living
        .len()
        .checked_sub(2)
        .and_then(|i| TARGET_RATES.get(i))
    else {
        return Some(first);
    };
    for (index, threshold) in row.iter().enumerate() {
        if roll >= *threshold {
            // The row is one shorter than the candidate list, so this index is
            // always in range.
            return living.get(index).copied();
        }
    }
    // Fell off the end of the row: the loop advanced past every threshold, so
    // the last living member is the target.
    living.last().copied()
}

/// The party members an enemy may target, in slot order — `loc_56F0`.
#[must_use]
pub fn targetable_party(roster: &Roster) -> Vec<FighterId> {
    roster.living(Side::Party).map(|f| f.id).collect()
}

/// How many regular abilities the roll picks between, and the mask that does
/// it: `andi.w #7, d0`.
pub const ABILITY_ROLL_MASK: u16 = (REGULAR_ABILITIES - 1) as u16;

/// `Enemy_Attack`'s ability roll — `ps4.asm:19146`.
///
/// ```text
/// loc_CFE6:
///     jsr     (UpdateRNGSeed2).l
///     andi.w  #7, d0
///     cmp.w   ($FFFFEEA8).w, d0
///     beq.s   loc_CFE6            ; REROLL if it matches the previous pick
///     move.w  d0, ($FFFFEEA8).w
///     move.b  $58(a3,d0.w), ability+1(a4)
/// ```
///
/// The reroll is on the **index**, not the ability, and the "previous" index is
/// a single global (`$FFFFEEA8`) shared by every enemy in the battle — so one
/// enemy's pick constrains the next enemy's. `last_index` is that global.
///
/// An enemy whose eight slots all hold the same ability still rerolls until the
/// index differs, burning rolls to reach the same answer. That is faithful and
/// it matters for the stream.
///
/// # Termination
///
/// The cartridge would spin forever if the mask could only ever produce the
/// previous value; it cannot, because `& 7` spans eight values. This
/// implementation still bounds the loop, and on exhausting the bound returns
/// the last index drawn rather than looping — a deviation that is unreachable
/// with any generator whose low three bits vary.
pub fn choose_ability(
    record: &EnemyRecord,
    last_index: &mut Option<u8>,
    rolls: &mut impl Rolls,
) -> (u8, u8) {
    const REROLL_BOUND: usize = 64;
    let mut index = 0u8;
    for _ in 0..REROLL_BOUND {
        index = (rolls.next_roll() & ABILITY_ROLL_MASK) as u8;
        if *last_index != Some(index) {
            break;
        }
    }
    *last_index = Some(index);
    (index, record.regular_abilities[usize::from(index)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures;
    use crate::battle::rng::SliceRolls;
    use crate::battle::stats::{Stats, status};

    fn ids(indices: &[u8]) -> Vec<FighterId> {
        indices
            .iter()
            .map(|i| FighterId::new(*i).expect("a valid id"))
            .collect()
    }

    fn target_at(roll: u16, living: &[FighterId]) -> Option<FighterId> {
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let chosen = choose_target(living, &mut rolls);
        assert_eq!(rolls.drawn(), 1, "exactly one roll, always");
        chosen
    }

    #[test]
    fn the_rate_table_is_the_cartridge_bytes() {
        assert_eq!(TARGET_RATES[0], [0x54]);
        assert_eq!(TARGET_RATES[1], [0x80, 0x2C]);
        assert_eq!(TARGET_RATES[2], [0x99, 0x4D, 0x1A]);
        assert_eq!(TARGET_RATES[3], [0xAC, 0x66, 0x33, 0x12]);
        // Row `n` serves a party of `n + 2`, so each holds one fewer threshold
        // than there are candidates.
        for (index, row) in TARGET_RATES.iter().enumerate() {
            assert_eq!(row.len(), index + 1, "row {index}");
        }
    }

    #[test]
    fn a_lone_survivor_is_targeted_without_consulting_the_table() {
        let living = ids(&[3]);
        for roll in [0u16, 0x54, 0xFF, 0x1234] {
            assert_eq!(
                target_at(roll, &living),
                living.first().copied(),
                "{roll:#X}"
            );
        }
    }

    #[test]
    fn five_alive_front_loads_the_leader() {
        let living = ids(&[1, 2, 3, 4, 5]);
        // $AC and above take slot 1; the bands descend from there.
        assert_eq!(target_at(0xAC, &living), Some(living[0]));
        assert_eq!(target_at(0xFF, &living), Some(living[0]));
        assert_eq!(target_at(0xAB, &living), Some(living[1]));
        assert_eq!(target_at(0x66, &living), Some(living[1]));
        assert_eq!(target_at(0x65, &living), Some(living[2]));
        assert_eq!(target_at(0x33, &living), Some(living[2]));
        assert_eq!(target_at(0x32, &living), Some(living[3]));
        assert_eq!(target_at(0x12, &living), Some(living[3]));
        assert_eq!(target_at(0x11, &living), Some(living[4]), "off the end");
        assert_eq!(target_at(0x00, &living), Some(living[4]));

        // The documented share for the leader: 256 - $AC = 84 of 256 bytes.
        let leader = (0..=0xFFu16)
            .filter(|roll| target_at(*roll, &living) == Some(living[0]))
            .count();
        assert_eq!(leader, 84);
    }

    #[test]
    fn only_the_low_byte_of_the_roll_reaches_the_comparison() {
        // `cmp.b`, not the six-bit mask the hit roll uses.
        let living = ids(&[1, 2, 3]);
        assert_eq!(target_at(0x0080, &living), target_at(0xFF80, &living));
        // And it is a different convention from every `calculate_chances`
        // site, which masks to six bits.
        assert_ne!(crate::battle::CHANCE_ROLL_MASK, 0xFF);
    }

    #[test]
    fn the_living_list_skips_the_dead_but_not_the_asleep() {
        let mut roster = Roster::new();
        for (index, record) in [fixtures::chaz(), fixtures::alys(), fixtures::hahn()]
            .iter()
            .enumerate()
        {
            let stats = Stats::from_character(record, |id| {
                fixtures::items().into_iter().find(|i| i.id == id)
            });
            roster.add_party_member(index as u8, record.name.clone(), stats);
        }
        assert_eq!(targetable_party(&roster).len(), 3);

        let second = FighterId::new(2).expect("id 2");
        roster.get_mut(second).expect("present").stats.status = status::ASLEEP;
        assert_eq!(
            targetable_party(&roster).len(),
            3,
            "asleep is still a target"
        );

        roster.get_mut(second).expect("present").stats.status = status::DEAD;
        let living = targetable_party(&roster);
        assert_eq!(living.len(), 2);
        assert_eq!(
            living.iter().map(|i| i.get()).collect::<Vec<_>>(),
            vec![1, 3],
            "slot order, with the gap closed"
        );
    }

    #[test]
    fn an_empty_party_yields_no_target() {
        assert_eq!(target_at(0x80, &[]), None);
    }

    #[test]
    fn the_ability_roll_never_repeats_its_previous_index() {
        let mut record = fixtures::zoran_bult();
        record.regular_abilities = [10, 11, 12, 13, 14, 15, 16, 17];

        // Two identical draws in a row: the second must be rerolled away.
        let draws = [3u16, 3, 3, 5];
        let mut rolls = SliceRolls::new(&draws);
        let mut last = None;

        let (index, ability) = choose_ability(&record, &mut last, &mut rolls);
        assert_eq!((index, ability), (3, 13));
        assert_eq!(rolls.drawn(), 1);

        let (index, ability) = choose_ability(&record, &mut last, &mut rolls);
        assert_eq!((index, ability), (5, 15), "rerolled past the repeats");
        assert_eq!(rolls.drawn(), 4, "and it cost three more draws");
        assert_eq!(last, Some(5));
    }

    #[test]
    fn identical_abilities_still_cost_a_reroll() {
        // ZoranBult's eight slots are all ability 0. The reroll is on the
        // index, so it fires even though the answer cannot change.
        let record = fixtures::zoran_bult();
        assert_eq!(record.regular_abilities, [0; REGULAR_ABILITIES]);

        let draws = [2u16, 2, 6];
        let mut rolls = SliceRolls::new(&draws);
        let mut last = None;
        assert_eq!(choose_ability(&record, &mut last, &mut rolls).1, 0);
        assert_eq!(choose_ability(&record, &mut last, &mut rolls), (6, 0));
        assert_eq!(rolls.drawn(), 3, "the repeat was rerolled anyway");
    }

    #[test]
    fn the_mask_keeps_the_index_inside_the_eight_slots() {
        let record = fixtures::zoran_bult();
        for raw in [0u16, 7, 8, 0xFFFF, 0x1234] {
            let draws = [raw];
            let mut rolls = SliceRolls::new(&draws);
            let mut last = None;
            let (index, _) = choose_ability(&record, &mut last, &mut rolls);
            assert!(usize::from(index) < REGULAR_ABILITIES, "raw {raw:#X}");
        }
    }

    #[test]
    fn a_degenerate_source_terminates_rather_than_spinning() {
        // A source that only ever yields one value would spin the cartridge
        // forever. The bound gives up and takes it.
        let draws = [4u16];
        let mut rolls = SliceRolls::new(&draws);
        let record = fixtures::zoran_bult();
        let mut last = Some(4);
        let (index, _) = choose_ability(&record, &mut last, &mut rolls);
        assert_eq!(index, 4);
    }
}
