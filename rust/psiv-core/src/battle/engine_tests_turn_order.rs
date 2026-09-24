//! Turn order: who reaches the queue, and who loses a turn already queued.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

#[test]
fn a_dead_fighter_loses_its_queued_turn() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    // Enemy 2 is one hit from death and slower than Alys, who hits both.
    battle.roster.get_mut(id(7)).expect("enemy 2").stats.curr_hp = 1;
    battle.roster.get_mut(id(6)).expect("enemy 1").stats.curr_hp = 200;

    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");

    assert!(events.contains(&BattleEvent::Died { fighter: id(7) }));
    assert!(
        events.contains(&BattleEvent::TurnSkipped {
            actor: id(7),
            reason: Skipped::Dead
        }),
        "it was in the queue but never got to swing"
    );
}

#[test]
fn an_incapacitated_fighter_never_reaches_the_queue() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    battle.roster.get_mut(id(2)).expect("Chaz").stats.status = status::PARALYZED;

    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");
    let order = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::RoundBegan { order, .. } => Some(order.clone()),
            _ => None,
        })
        .expect("a round began");
    assert!(!order.contains(&id(2)));
}
