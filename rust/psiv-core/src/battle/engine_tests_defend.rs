//! The Defend command: what it halves, and when it takes effect.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

/// Runs one round of the basement fight with a fixed roll stream, and
/// reports what Alys took.
///
/// The target draws are forced to `$80`, which with three living members
/// selects the party's front slot — Alys — from `EnemyTargetRates`.
fn damage_to_alys(orders: &RoundOrders) -> Vec<u16> {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    // Nobody dies this round, so every fighter takes its turn.
    for slot in [1u8, 2, 3, 6, 7] {
        let fighter = battle.roster.get_mut(id(slot)).expect("present");
        fighter.stats.curr_hp = 500;
        fighter.stats.max_hp = 500;
    }
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS)
        .chain(std::iter::repeat_n(0x80u16, ENEMY_SLOTS))
        .chain(std::iter::repeat_n(30u16, 400))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle.round(orders, &data, &mut rolls).expect("resolves");
    events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved {
                actor,
                target,
                damage: Some(amount),
                ..
            } if *target == id(1) && actor.side() == Side::Enemy => Some(*amount),
            _ => None,
        })
        .collect()
}

#[test]
fn defending_halves_the_physical_factor() {
    // Alys is first in the queue at agility 15, so her Defend is in place
    // before either enemy swings. Attack 16 against her defence 18:
    //   normal   (factor 2): 42 * 2 >> 2 = 21, minus 18 -> 3
    //   defended (factor 1): 42 * 1 >> 2 = 10, minus 18 -> clamped to 1
    let undefended = damage_to_alys(&RoundOrders::attack_all());
    assert_eq!(undefended, vec![3, 3], "both enemies, unhalved");

    let defended = damage_to_alys(&RoundOrders::Commands(vec![Command::Defend]));
    assert_eq!(defended, vec![1, 1], "halved into the floor");
}

#[test]
fn defending_does_nothing_against_a_swing_that_lands_first() {
    // `Character_Defend` sets the physical property when the defender's
    // turn comes up, not when the command is entered — so a slow defender
    // is hit at full strength before the resistance exists. Hahn is last in
    // the queue at agility 4.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    for slot in [1u8, 2, 3, 6, 7] {
        let fighter = battle.roster.get_mut(id(slot)).expect("present");
        fighter.stats.curr_hp = 500;
        fighter.stats.max_hp = 500;
    }
    // Target roll 0 falls off the end of the rate row, taking the last
    // living member: Hahn.
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 400))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let orders = RoundOrders::Commands(vec![Command::Defend, Command::Defend, Command::Defend]);
    let events = battle.round(&orders, &data, &mut rolls).expect("resolves");

    let order = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::RoundBegan { order, .. } => Some(order.clone()),
            _ => None,
        })
        .expect("a round began");
    assert_eq!(order.last(), Some(&id(3)), "Hahn acts last");

    let taken: Vec<u16> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved {
                target,
                damage: Some(amount),
                ..
            } if *target == id(3) => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(taken, vec![12, 12], "hit at full strength, twice");
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Defended { .. }))
            .count(),
        3,
        "he did defend — it was just too late to matter"
    );
}

#[test]
fn a_defend_wears_off_when_the_round_ends() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    for slot in [1u8, 2, 3, 6, 7] {
        let fighter = battle.roster.get_mut(id(slot)).expect("present");
        fighter.stats.curr_hp = 500;
        fighter.stats.max_hp = 500;
    }
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 400))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend, Command::Defend, Command::Defend]),
            &data,
            &mut rolls,
        )
        .expect("resolves");

    for fighter in battle.roster.side(Side::Party) {
        assert_eq!(
            fighter.stats.element_factor(1),
            Some(2),
            "Battle_RestoreStatsAtTurnEnd put it back"
        );
    }
}
