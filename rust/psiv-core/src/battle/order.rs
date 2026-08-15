//! Who goes first, and in what order — `Battle_OrderTurns` (`ps4.asm:7723`).
//!
//! The queue is rebuilt every round. Its shape is nine four-byte entries at
//! `Battle_Turn_Order` (`$FFFFEFB0`), each a fighter id word followed by an
//! ordering agility word, sorted highest first.
//!
//! Two things here are easy to get subtly wrong and are pinned by tests: the
//! jitter draws a roll for **every one of the nine slots**, occupied or not,
//! because the `jsr` sits above the emptiness check; and the sort is a specific
//! bubble sort whose comparison keeps the earlier entry on a tie, which is what
//! makes ties resolve by slot order.

use super::chances::{START_PRIORITY, Verdict, calculate_chances};
use super::fighters::{FIGHTER_SLOTS, FighterId, Roster, Side};
use super::rng::Rolls;

/// How the battle opens — `Battle_Priority` (`$FFFFEE45`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Priority {
    /// `$FF`. The enemies act first and the party does not act at all this
    /// round; every enemy also gets `reaction_flags` bit 3.
    Ambush,
    /// `0`. Everyone queues together.
    #[default]
    Normal,
    /// `1`. The party acts and the enemies do not.
    Preemptive,
}

/// `loc_B62A` (`$00B62A`) — the opening roll.
///
/// Highest living party agility against the formation's byte 0, scale 2,
/// ambush at or below `$C`, preemptive above `$74`. A boss battle
/// (`Event_Battle_Index >= 0`) forces [`Priority::Normal`]; Tier 1 has no boss
/// battles, so `boss` is threaded through rather than assumed.
///
/// Draws exactly one roll.
pub fn roll_priority(
    highest_party_agility: u8,
    ambush_chance: u8,
    boss: bool,
    rolls: &mut impl Rolls,
) -> Priority {
    let (scale, ambush, preempt) = START_PRIORITY;
    let verdict = calculate_chances(
        i16::from(highest_party_agility),
        i16::from(ambush_chance),
        scale,
        ambush,
        preempt,
        rolls,
    );
    if boss {
        // `tst.b (Event_Battle_Index).l / bmi / clr.b d0` — a boss battle
        // discards the roll it just made, draw and all.
        return Priority::Normal;
    }
    match verdict {
        Verdict::Miss => Priority::Ambush,
        Verdict::Normal => Priority::Normal,
        Verdict::Critical => Priority::Preemptive,
    }
}

/// One entry of `Battle_Turn_Order`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueEntry {
    /// The fighter, by one-based id.
    pub fighter: FighterId,
    /// The ordering value: `agility_battle` plus this round's jitter.
    pub ordering_agility: u16,
    /// The unjittered `agility_battle`, which the cartridge leaves untouched in
    /// the stats struct — the oracle checked that it never moves.
    pub agility: u8,
}

/// `Battle_OrderTurns` — `ps4.asm:7723`.
///
/// Which fighters are enqueued depends on `priority`: an ambush queues only
/// enemies, a preemptive strike only the party, and a normal round everyone.
/// Anyone whose status has a bit in `$6E` — paralysed, dead, asleep,
/// asleep-2, android-dead — is skipped. Tech-sealed is not in that mask.
///
/// # Rolls drawn
///
/// Exactly [`FIGHTER_SLOTS`] (nine), one per slot whether or not it holds
/// anyone, because `Battle_OrderTurns` draws before it checks
/// (`ps4.asm:7793-7804`). A port that draws only for real fighters desyncs the
/// stream the moment somebody dies.
pub fn build_queue(roster: &Roster, priority: Priority, rolls: &mut impl Rolls) -> Vec<QueueEntry> {
    let mut queue: Vec<QueueEntry> = Vec::with_capacity(FIGHTER_SLOTS);
    let sides: &[Side] = match priority {
        Priority::Ambush => &[Side::Enemy],
        Priority::Preemptive => &[Side::Party],
        Priority::Normal => &[Side::Party, Side::Enemy],
    };
    for side in sides {
        for fighter in roster.side(*side) {
            if !fighter.stats.can_act() {
                continue;
            }
            queue.push(QueueEntry {
                fighter: fighter.id,
                ordering_agility: u16::from(fighter.stats.agility.battle),
                agility: fighter.stats.agility.battle,
            });
        }
    }

    apply_jitter(&mut queue, rolls);
    sort_descending(&mut queue);
    queue
}

/// The random addend, and the divisor it is taken modulo.
///
/// ```text
///     move.w  (a1), d1            ; scan the nine entries for the largest
///     ...
///     lsr.w   #1, d1              ; divisor = max agility / 2
/// -   clr.w   d0
///     swap    d0                  ; force d0's high word to zero for divu
///     jsr     (UpdateRNGSeed2).l
///     divu.w  d1, d0              ; 32 / 16
///     swap    d0                  ; keep the REMAINDER
///     tst.w   (a1)
///     beq.s   +                   ; agility 0 gets no bonus
///     add.w   d0, (a1)
/// +   addq.w  #4, a1
///     dbf     d7, -
/// ```
///
/// The `clr.w`/`swap` dance is not decoration: `divu.w` is a 32-by-16 divide,
/// and zeroing the high word is what keeps the quotient from overflowing and
/// leaving the operands untouched.
///
/// The oracle measured nine addends across two rounds, all in `+2..=+5` with a
/// maximum agility of 15 — consistent with `roll % 7`.
///
/// # A quirk this does not reproduce
///
/// The cartridge's max-agility scan reads **ten** words for a nine-entry table
/// (`moveq #8, d7` with the pointer advanced before each compare,
/// `ps4.asm:7779-7788`), so the divisor is the larger of the real maximum and
/// whatever stale word sits past the end of the queue — a slot `TRAP #0` never
/// clears. It is an out-of-bounds read of uninitialised RAM and it can only
/// widen the jitter. This takes the maximum over the nine real entries, which
/// is what the routine intends and what it computes whenever that word is not
/// larger. Filed for the ledger rather than reproduced.
fn apply_jitter(queue: &mut [QueueEntry], rolls: &mut impl Rolls) {
    let divisor = queue
        .iter()
        .map(|entry| entry.ordering_agility)
        .max()
        .unwrap_or(0)
        >> 1;

    for slot in 0..FIGHTER_SLOTS {
        // Drawn unconditionally — see the note on this function.
        let roll = u32::from(rolls.next_roll());
        let Some(entry) = queue.get_mut(slot) else {
            continue;
        };
        if divisor == 0 {
            // `divu.w` by zero raises a division-by-zero exception on a 68000.
            // Unreachable with retail data — any fighter that reaches the queue
            // has at least 1 agility, and paralysis floors it at 1 rather than
            // 0 — so the core declines to jitter rather than to panic.
            continue;
        }
        if entry.ordering_agility == 0 {
            continue;
        }
        let remainder = (roll % u32::from(divisor)) as u16;
        entry.ordering_agility = entry.ordering_agility.wrapping_add(remainder);
    }
}

/// The exact bubble sort at `ps4.asm:7807-7819`.
///
/// Eight outer passes over eight adjacent pairs, descending, comparing the
/// ordering agility as an unsigned word. `bcc` keeps the earlier entry when the
/// two are equal, so the sort is stable and ties fall out in queue order —
/// which is party-then-enemy, each in slot order.
///
/// Eight passes over nine entries is a complete sort, so the loop bounds are
/// reproduced for their tie behaviour rather than because a partial sort would
/// show.
fn sort_descending(queue: &mut [QueueEntry]) {
    let passes = FIGHTER_SLOTS - 1;
    for _ in 0..passes {
        for index in 0..passes {
            let (Some(left), Some(right)) =
                (queue.get(index).copied(), queue.get(index + 1).copied())
            else {
                continue;
            };
            if left.ordering_agility < right.ordering_agility {
                queue.swap(index, index + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::fixtures;
    use crate::battle::rng::SliceRolls;
    use crate::battle::stats::{Stats, status};

    fn party_roster() -> Roster {
        let mut roster = Roster::new();
        let items = fixtures::items();
        let lookup = |id: u8| items.iter().find(|i| i.id == id).cloned();
        for (index, record) in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()]
            .iter()
            .enumerate()
        {
            let stats = Stats::from_character(record, lookup);
            roster.add_party_member(index as u8, record.name.clone(), stats);
        }
        roster.add_enemy(1, &fixtures::zoran_bult());
        roster.add_enemy(2, &fixtures::zoran_bult());
        roster
    }

    fn queue_with(rolls: &[u16], priority: Priority) -> Vec<QueueEntry> {
        let mut source = SliceRolls::new(rolls);
        let queue = build_queue(&party_roster(), priority, &mut source);
        assert_eq!(source.drawn(), FIGHTER_SLOTS, "nine draws, always");
        queue
    }

    #[test]
    fn the_queue_draws_for_empty_slots_too() {
        // Three party members and two enemies, but nine rolls consumed.
        queue_with(&[0], Priority::Normal);
    }

    #[test]
    fn a_zero_jitter_orders_purely_by_agility() {
        // Alys 15, Chaz 7, ZoranBult 6, ZoranBult 6, Hahn 4.
        let queue = queue_with(&[0], Priority::Normal);
        let order: Vec<(u8, u16)> = queue
            .iter()
            .map(|e| (e.fighter.get(), e.ordering_agility))
            .collect();
        assert_eq!(order, vec![(1, 15), (2, 7), (6, 6), (7, 6), (3, 4)]);
    }

    #[test]
    fn ties_fall_out_in_queue_order() {
        // The two ZoranBults tie at 6. `bcc` keeps the earlier one, so slot 6
        // stays ahead of slot 7 — and the party is enqueued before the enemies,
        // so a party/enemy tie goes to the party.
        let queue = queue_with(&[0], Priority::Normal);
        let sixth = queue.iter().position(|e| e.fighter.get() == 6);
        let seventh = queue.iter().position(|e| e.fighter.get() == 7);
        assert!(sixth < seventh, "the earlier slot keeps its place");

        // Force a party/enemy tie: Hahn at 4 against an enemy also at 4.
        let mut roster = party_roster();
        for id in [6u8, 7] {
            let id = FighterId::new(id).expect("an enemy id");
            roster.get_mut(id).expect("present").stats.agility.battle = 4;
        }
        let mut source = SliceRolls::new(&[0]);
        let queue = build_queue(&roster, Priority::Normal, &mut source);
        let hahn = queue
            .iter()
            .position(|e| e.fighter.get() == 3)
            .expect("Hahn");
        let enemy = queue
            .iter()
            .position(|e| e.fighter.get() == 6)
            .expect("enemy");
        assert!(hahn < enemy, "party members are enqueued first");
    }

    #[test]
    fn the_jitter_is_the_roll_modulo_half_the_highest_agility() {
        // The oracle's round one: max agility 15, so the divisor is 7 and the
        // addends it measured (+5, +4, +5, +3, +2) are all reachable.
        let queue = queue_with(&[12], Priority::Normal);
        for entry in &queue {
            let addend = entry.ordering_agility - u16::from(entry.agility);
            assert_eq!(addend, 12 % 7, "roll 12 mod 7");
            assert!(addend < 7, "bounded by max_agility / 2");
        }
    }

    #[test]
    fn every_oracle_addend_is_inside_the_bound() {
        // Nine samples across two rounds, all with Alys's 15 leading.
        let divisor = 15u16 >> 1;
        for addend in [5u16, 4, 5, 3, 2, 4, 5, 2, 3] {
            assert!(
                addend < divisor,
                "the oracle saw +{addend}, bound {divisor}"
            );
        }
        // And the bound is reachable from both ends.
        for roll in 0..64u16 {
            let queue = queue_with(&[roll], Priority::Normal);
            let addend = queue[0].ordering_agility - u16::from(queue[0].agility);
            assert!(addend < divisor, "roll {roll}");
        }
    }

    #[test]
    fn the_stored_agility_never_moves() {
        // The oracle: "The stored `agility_battle` never changed." The jitter
        // lives in the queue, not in the stats struct.
        let roster = party_roster();
        let before: Vec<u8> = roster.iter().map(|f| f.stats.agility.battle).collect();
        let mut source = SliceRolls::new(&[9]);
        let queue = build_queue(&roster, Priority::Normal, &mut source);
        let after: Vec<u8> = roster.iter().map(|f| f.stats.agility.battle).collect();
        assert_eq!(before, after);
        assert!(
            queue
                .iter()
                .any(|e| e.ordering_agility > u16::from(e.agility))
        );
    }

    #[test]
    fn an_ambush_queues_only_enemies_and_a_preemptive_only_the_party() {
        let ambushed = queue_with(&[0], Priority::Ambush);
        assert!(
            ambushed.iter().all(|e| e.fighter.side() == Side::Enemy),
            "an ambush gives the party no turn at all"
        );
        assert_eq!(ambushed.len(), 2);

        let preempt = queue_with(&[0], Priority::Preemptive);
        assert!(preempt.iter().all(|e| e.fighter.side() == Side::Party));
        assert_eq!(preempt.len(), 3);
    }

    #[test]
    fn a_status_in_the_no_turn_mask_costs_the_queue_slot() {
        let mut roster = party_roster();
        let chaz = FighterId::new(2).expect("id 2");
        roster.get_mut(chaz).expect("present").stats.status = status::ASLEEP;
        let mut source = SliceRolls::new(&[0]);
        let queue = build_queue(&roster, Priority::Normal, &mut source);
        assert!(!queue.iter().any(|e| e.fighter == chaz), "asleep, no turn");
        assert_eq!(source.drawn(), FIGHTER_SLOTS, "still nine draws");

        // Poison is not in the mask.
        roster.get_mut(chaz).expect("present").stats.status = status::POISONED;
        let mut source = SliceRolls::new(&[0]);
        let queue = build_queue(&roster, Priority::Normal, &mut source);
        assert!(queue.iter().any(|e| e.fighter == chaz), "poison still acts");
    }

    #[test]
    fn a_queue_of_agility_zero_declines_to_divide_rather_than_dying() {
        let mut roster = Roster::new();
        let mut stats = Stats::from_enemy(&fixtures::zoran_bult());
        stats.agility.battle = 0;
        roster.add_party_member(0, "still".into(), stats);
        let mut source = SliceRolls::new(&[40]);
        let queue = build_queue(&roster, Priority::Normal, &mut source);
        assert_eq!(queue[0].ordering_agility, 0, "no jitter, no panic");
        assert_eq!(source.drawn(), FIGHTER_SLOTS);
    }

    fn priority_at(roll: u16, agility: u8, chance: u8, boss: bool) -> Priority {
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        let priority = roll_priority(agility, chance, boss, &mut rolls);
        assert_eq!(rolls.drawn(), 1, "the roll happens even for a boss");
        priority
    }

    #[test]
    fn a_level_one_party_can_be_ambushed_but_never_strikes_first() {
        // Scout §12: Chaz alone at agility 7 against formation byte $10.
        let ambushes = (0..=63u16)
            .filter(|roll| priority_at(*roll, 7, 0x10, false) == Priority::Ambush)
            .count();
        assert_eq!(ambushes, 16, "16 of 64, the documented 25%");
        assert!(
            (0..=63u16).all(|roll| priority_at(roll, 7, 0x10, false) != Priority::Preemptive),
            "unreachable at agility 7 against $10"
        );
    }

    #[test]
    fn a_fast_party_can_strike_first() {
        // The same roll and formation, with agility high enough to clear $74.
        assert_eq!(priority_at(63, 60, 0x10, false), Priority::Preemptive);
        assert_eq!(priority_at(0, 60, 0x10, false), Priority::Normal);
    }

    #[test]
    fn a_boss_battle_is_always_normal() {
        for roll in 0..=63u16 {
            assert_eq!(
                priority_at(roll, 60, 0x10, true),
                Priority::Normal,
                "{roll}"
            );
            assert_eq!(priority_at(roll, 1, 0xFF, true), Priority::Normal, "{roll}");
        }
    }
}
