//! The record seam: the data `Battle::start` refuses, and the party a
//! battle hands back.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

#[test]
fn a_formation_the_data_cannot_resolve_is_an_error_not_a_panic() {
    let data = BattleData::new();
    let mut rolls = SliceRolls::new(&[0]);
    assert_eq!(
        Battle::start(
            &fixtures::formation_two_zoran_bults(),
            Vec::new(),
            &data,
            false,
            0,
            &mut rolls
        ),
        Err(BattleDataError::UnknownEnemy(10))
    );

    let empty = FormationRecord {
        id: 7,
        ambush_chance: 0,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: Vec::new(),
    };
    assert_eq!(
        Battle::start(&empty, Vec::new(), &data, false, 0, &mut rolls),
        Err(BattleDataError::EmptyFormation(7))
    );
}

#[test]
fn a_battle_hands_back_the_same_records_it_was_given() {
    // `Stats` is the persistent per-character record, so what goes in is what
    // comes out — identity preserved, no conversion layer, nobody dropped.
    let data = fixtures::data();
    let party = basement_party(&data);
    let going_in: Vec<(u8, String)> = party
        .iter()
        .map(|m| (m.character, m.name.clone()))
        .collect();

    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        party,
        &data,
        &mut rolls,
    );
    let mut seed = Lcg41::new(0x7777_1111);
    let mut rolls = Rng2::with_surrogate(&mut seed, 11);
    play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    let coming_out: Vec<(u8, String)> = battle
        .into_party()
        .into_iter()
        .map(|m| (m.character, m.name))
        .collect();
    assert_eq!(coming_out, going_in, "same members, same order");
}

#[test]
fn the_fields_the_field_carries_away_survive_a_battle() {
    // The invariant `docs/FIELD_STATE.md` records: HP spent, experience and
    // levels won, and death all persist in the record the battle gives back.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        vec![member(&fixtures::chaz(), &data)],
        &data,
        &mut rolls,
    );
    {
        let chaz = battle.roster.get_mut(id(1)).expect("Chaz");
        chaz.stats.curr_hp = 400;
        chaz.stats.max_hp = 500;
        chaz.stats.curr_tp = 7;
        chaz.stats.experience = 17;
        assert!(!chaz.stats.gain_exp_flag);
    }

    let mut seed = Lcg41::new(0x2468_ACE0);
    let mut rolls = Rng2::with_surrogate(&mut seed, 12);
    play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
    assert_eq!(battle.outcome(), Some(Outcome::Victory));

    let party = battle.into_party();
    let chaz = &party[0].stats;
    assert!(chaz.curr_hp < 400, "the enemies got some hits in");
    assert!(chaz.curr_hp > 0);
    assert_eq!(chaz.max_hp, 500, "battle does not level anyone");
    assert_eq!(chaz.curr_tp, 7, "TP is untouched by a Tier 1 battle");
    assert_eq!(chaz.status, 0, "and he is still standing");

    // The reward-shaped fields are the roster's, and a battle must leave them
    // exactly as it found them — see `rewards`'s note on the seam.
    assert_eq!(chaz.experience, 17, "unchanged: the roster awards");
    assert_eq!(chaz.level, 1, "unchanged: the roster levels");
    assert!(!chaz.gain_exp_flag, "unchanged: the roster sets the flag");
}
