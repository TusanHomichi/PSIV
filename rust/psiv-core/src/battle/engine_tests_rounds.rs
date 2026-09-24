//! Round structure: the draws a round takes, its priority, and escape/RUN.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

// ---------------------------------------------------------------------
// Round structure
// ---------------------------------------------------------------------

#[test]
fn a_round_draws_the_rolls_the_cartridge_draws() {
    // One party member against one enemy, both too tough to die, so every
    // stage of the round runs exactly once:
    //   9  jitter draws            — one per fighter SLOT, not per fighter
    //   4  enemy target draws      — one per enemy SLOT, not per enemy
    //   1  enemy ability roll
    //   1  hit roll + 16 damage draws, twice
    let data = fixtures::data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.enemies.truncate(1);

    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &formation,
        vec![member(&fixtures::chaz(), &data)],
        &data,
        &mut rolls,
    );
    for id in [id(1), id(6)] {
        let fighter = battle.roster.get_mut(id).expect("present");
        fighter.stats.curr_hp = 500;
        fighter.stats.max_hp = 500;
    }

    // 30 masks to a normal hit for both sides and to ability index 6.
    let draws: Vec<u16> = std::iter::repeat_n(30u16, 400).collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");

    let landed = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                BattleEvent::Resolved {
                    damage: Some(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(landed, 2, "one swing each, both connecting");
    assert_eq!(
        rolls.drawn(),
        FIGHTER_SLOTS + ENEMY_SLOTS + 1 + 2 + 2 * DAMAGE_DRAWS,
        "9 jitter + 4 targets + 1 ability + 2 hits + 2 damage rolls"
    );
}

#[test]
fn a_dead_enemy_costs_no_ability_roll() {
    // The turn is skipped before `Enemy_Attack` is reached, so the roll it
    // would have taken is never drawn.
    let data = fixtures::data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.enemies.truncate(1);

    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &formation,
        vec![member(&fixtures::chaz(), &data)],
        &data,
        &mut rolls,
    );
    battle.roster.get_mut(id(6)).expect("enemy").stats.status = status::DEAD;

    let draws: Vec<u16> = std::iter::repeat_n(30u16, 400).collect();
    let mut rolls = SliceRolls::new(&draws);
    battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");
    // Only the jitter and target draws: a corpse is not queued, and the
    // battle is already won so nobody else acts either.
    assert_eq!(rolls.drawn(), FIGHTER_SLOTS + ENEMY_SLOTS);
    assert_eq!(battle.outcome(), Some(Outcome::Victory));
}

#[test]
fn the_same_seed_gives_the_same_timeline() {
    let data = fixtures::data();
    let run = || {
        let mut seed = Lcg41::new(0x0102_0304);
        let mut rolls = Rng2::with_surrogate(&mut seed, 7);
        let mut battle = start(
            &fixtures::formation_two_zoran_bults(),
            basement_party(&data),
            &data,
            &mut rolls,
        );
        let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
        (timeline, battle)
    };
    let (first, first_battle) = run();
    let (second, second_battle) = run();
    assert_eq!(first, second);
    assert_eq!(first_battle, second_battle);
    assert!(first.len() > 10, "and it was a real fight");
}

#[test]
fn an_ambush_gives_the_party_no_turn_at_all() {
    let data = fixtures::data();
    // Roll 0 against ambush chance $10 with agility 15: (0 + 15 - 16) * 2
    // is negative, well under $C.
    let mut rolls = SliceRolls::new(&[0]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    assert_eq!(battle.pending_priority, Priority::Ambush);

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
    assert!(order.iter().all(|f| f.side() == Side::Enemy));

    // And the next round is normal again — Battle_Priority is cleared.
    assert_eq!(battle.pending_priority, Priority::Normal);
}

#[test]
fn a_preemptive_strike_lets_the_party_run_for_free() {
    let data = fixtures::data();
    let mut fast = fixtures::alys();
    fast.agility = 90;
    let mut rolls = SliceRolls::new(&[63]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        vec![member(&fast, &data)],
        &data,
        &mut rolls,
    );
    assert_eq!(battle.pending_priority, Priority::Preemptive);

    let mut rolls = SliceRolls::new(&[0]);
    let events = battle
        .round(&RoundOrders::Run, &data, &mut rolls)
        .expect("resolves");
    assert_eq!(rolls.drawn(), 0, "no roll is taken at all");
    assert_eq!(battle.outcome(), Some(Outcome::Escaped));
    assert_eq!(
        events,
        vec![
            BattleEvent::Escaped,
            BattleEvent::Ended {
                outcome: Outcome::Escaped
            }
        ]
    );
}

#[test]
fn a_failed_escape_hands_the_round_to_the_enemies() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );

    // Escape roll 0 with agility 15 against run chance 0: (0 + 15) * 2 = 30
    // is above $28, so make the party slow instead.
    for slot in 1..=3u8 {
        battle
            .roster
            .get_mut(id(slot))
            .expect("present")
            .stats
            .agility
            .battle = 1;
    }
    let draws: Vec<u16> = [0u16] // the escape roll: (0 + 1) * 2 = 2, a failure
        .into_iter()
        .chain(std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS))
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::Run, &data, &mut rolls)
        .expect("resolves");

    assert!(events.contains(&BattleEvent::EscapeFailed));
    assert_eq!(battle.outcome(), None);
    let order = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::RoundBegan { order, .. } => Some(order.clone()),
            _ => None,
        })
        .expect("a round began");
    assert!(
        order.iter().all(|f| f.side() == Side::Enemy),
        "st (Battle_Priority).w — the failure is an ambush"
    );
}

#[test]
fn an_unrunnable_formation_can_never_be_escaped() {
    let data = fixtures::data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.run_chance = 0xF0;
    assert!(!formation.can_run());

    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(&formation, basement_party(&data), &data, &mut rolls);
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::Run, &data, &mut rolls)
        .expect("resolves");
    assert!(events.contains(&BattleEvent::EscapeFailed));
    assert!(rolls.drawn() > 0, "but the round still runs");
}
