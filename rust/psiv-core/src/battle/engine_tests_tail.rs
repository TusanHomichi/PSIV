//! Battle persistence and roster-seam tests kept separate from the main engine
//! tests so each test module stays below the repository's 1,000-line limit.

use super::*;

#[test]
fn a_wiped_party_comes_back_marked_dead_rather_than_missing() {
    // The field needs to know who fell, so death is state on the record, not
    // an absence from the handoff.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        vec![member(&fixtures::hahn(), &data)],
        &data,
        &mut rolls,
    );
    battle.roster.get_mut(id(1)).expect("Hahn").stats.curr_hp = 1;

    let mut seed = Lcg41::new(0x0BAD_0BAD);
    let mut rolls = Rng2::with_surrogate(&mut seed, 13);
    play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
    assert_eq!(battle.outcome(), Some(Outcome::Defeat));

    let party = battle.into_party();
    assert_eq!(party.len(), 1, "still handed back");
    assert_eq!(party[0].stats.status & status::DEAD, status::DEAD);
    assert_eq!(party[0].stats.curr_hp, 0);
    assert!(!party[0].stats.gain_exp_flag, "a wipe pays nothing");
}

#[test]
fn nothing_transient_leaks_out_of_a_finished_battle() {
    // A caller must not have to know which fields were battle-only. The
    // `battle` copies still agree with their derived values, and a Defend that
    // ran during the fight has been undone.
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
    // Everyone defends for a round, then the fight is played out normally.
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
    let mut seed = Lcg41::new(0x3141_5926);
    let mut rolls = Rng2::with_surrogate(&mut seed, 14);
    play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    for member in battle.into_party() {
        let stats = &member.stats;
        assert_eq!(
            stats.element_props[0], stats.physical_prop_save,
            "character {}: a Defend was left in place",
            member.character
        );
        assert_eq!(stats.attack.battle, stats.attack.derived);
        assert_eq!(stats.defence.battle, stats.defence.derived);
        assert_eq!(stats.strength.battle, stats.strength.modified);
        assert_eq!(stats.agility.battle, stats.agility.modified);
    }
}

#[test]
fn party_stats_and_into_party_agree() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let mut seed = Lcg41::new(0x5A5A_5A5A);
    let mut rolls = Rng2::with_surrogate(&mut seed, 15);
    play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    let viewed: Vec<(u8, Stats)> = battle
        .party_stats()
        .map(|(id, stats)| (id, stats.clone()))
        .collect();
    let taken: Vec<(u8, Stats)> = battle
        .into_party()
        .into_iter()
        .map(|m| (m.character, m.stats))
        .collect();
    assert_eq!(viewed, taken, "the borrow and the move see the same thing");
}

/// Sixteen draws whose masked values sum to `sum`.
fn draws_for(sum: u16) -> Vec<u16> {
    assert!(sum <= 112, "S maxes out at 112");
    let mut out = vec![0u16; DAMAGE_DRAWS];
    let mut left = sum;
    for slot in &mut out {
        let take = left.min(7);
        *slot = take;
        left -= take;
    }
    out
}

/// Every damage figure reachable for a stat pairing, over every possible sum
/// of sixteen draws.
pub(super) fn achievable_impl(attack: u16, defence: u16, element: u16, bonus: u16) -> Vec<u16> {
    let mut seen: Vec<u16> = (0..=112u16)
        .map(|sum| {
            let draws = draws_for(sum);
            let mut rolls = SliceRolls::new(&draws);
            clamp_damage(calculate_damage(
                attack, defence, element, bonus, &mut rolls,
            ))
        })
        .collect();
    seen.dedup();
    seen
}
