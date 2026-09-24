//! The Scout §12 worked example, end to end.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

// ---------------------------------------------------------------------
// Scout §12: the worked example, end to end
// ---------------------------------------------------------------------

#[test]
fn the_worked_example_opens_the_way_the_scout_says_it_does() {
    // Chaz alone against two MonsterFly, formation 0 of block 1.
    let data = fixtures::data();
    let formation = fixtures::formation_two_monster_flies();

    let ambushes = (0..=63u16)
        .filter(|roll| {
            let draws = [*roll];
            let mut rolls = SliceRolls::new(&draws);
            let battle = start(
                &formation,
                vec![member(&fixtures::chaz(), &data)],
                &data,
                &mut rolls,
            );
            assert_eq!(rolls.drawn(), 1, "setup costs exactly one roll");
            battle.pending_priority == Priority::Ambush
        })
        .count();
    assert_eq!(ambushes, 16, "25% at agility 7 against $10");
}

#[test]
fn a_monsterfly_always_outruns_a_level_one_chaz() {
    // Agility 7 against 12 with a divisor of 6: Chaz's best jitter is +5
    // and the fly's worst is +0, so 12 is never beaten. Worth pinning
    // because the turn order decides who dies first in the worked example.
    let data = fixtures::data();
    let formation = fixtures::formation_two_monster_flies();
    for roll in 0..64u16 {
        let mut rolls = SliceRolls::new(&[20]);
        let mut battle = start(
            &formation,
            vec![member(&fixtures::chaz(), &data)],
            &data,
            &mut rolls,
        );
        let draws: Vec<u16> = std::iter::repeat_n(roll, 400).collect();
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
        assert_eq!(order.first(), Some(&id(6)), "roll {roll}");
    }
}

#[test]
fn chaz_deals_damage_inside_the_worked_examples_band() {
    // Scout §12: attack 18, defence 0, physical 2, mean S -> 18 damage
    // against 20 HP.
    let data = fixtures::data();
    let formation = fixtures::formation_two_monster_flies();
    // Priority normal, no jitter, hit roll 40 (a normal hit), damage draws
    // of 3 and 4 alternating to land S = 56.
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &formation,
        vec![member(&fixtures::chaz(), &data)],
        &data,
        &mut rolls,
    );
    assert_eq!(battle.pending_priority, Priority::Normal);

    let band = achievable(18, 0, 2, 0);
    let mut seed = Lcg41::new(0xC0FF_EE00);
    let mut rolls = Rng2::with_surrogate(&mut seed, 4);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");

    let chaz_hits: Vec<u16> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Resolved {
                actor,
                damage: Some(amount),
                ..
            } if *actor == id(1) => Some(*amount),
            _ => None,
        })
        .collect();
    assert!(!chaz_hits.is_empty(), "Chaz landed something");
    for amount in chaz_hits {
        assert!(band.contains(&amount), "{amount} outside {band:?}");
    }
}

#[test]
fn the_worked_examples_damage_band_is_ten_to_twenty_five() {
    // Scout §12: Chaz attack 18 against a MonsterFly's defence 0 and
    // physical 2.
    let band = achievable(18, 0, 2, 0);
    assert_eq!(*band.first().expect("S = 0"), 10, "the minimum roll");
    assert_eq!(*band.last().expect("S = 112"), 25, "the maximum roll");
    assert!(band.contains(&18), "and 18 at the mean");

    // The return swing: attack 14 against Chaz's defence 10.
    let back = achievable(14, 10, 2, 0);
    assert_eq!(*back.first().expect("S = 0"), 1, "clamped up from -3");
    assert_eq!(*back.last().expect("S = 112"), 10);
}
