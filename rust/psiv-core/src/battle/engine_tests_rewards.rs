//! What a battle pays: the split it reports, the roster seam that awards
//! and levels, and the nothing a wipe pays.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

#[test]
fn a_defeat_pays_nothing() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        vec![member(&fixtures::hahn(), &data)],
        &data,
        &mut rolls,
    );
    battle.roster.get_mut(id(1)).expect("Hahn").stats.curr_hp = 1;

    let mut seed = Lcg41::new(0x5EED_5EED);
    let mut rolls = Rng2::with_surrogate(&mut seed, 2);
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
    assert_eq!(battle.outcome(), Some(Outcome::Defeat));
    assert_eq!(rewarded(&timeline), None, "no rewards for a wipe");
    assert!(timeline.contains(&BattleEvent::Ended {
        outcome: Outcome::Defeat
    }));
}

#[test]
fn a_finished_battle_resolves_no_further_rounds() {
    let data = fixtures::data();
    let mut seed = Lcg41::new(0x9999_0001);
    let mut rolls = Rng2::with_surrogate(&mut seed, 0);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
    assert!(battle.outcome().is_some());

    let before = battle.clone();
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");
    assert!(events.is_empty());
    assert_eq!(battle, before, "and nothing moved");
}

#[test]
fn battle_reports_the_split_and_pays_nobody() {
    // The contract the roster relies on: a won battle emits `Rewarded` with the
    // arithmetic and mutates no experience, no flag and no level.
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
        chaz.stats.curr_hp = 500;
        chaz.stats.max_hp = 500;
        chaz.stats.experience = 17;
    }

    let mut seed = Lcg41::new(0x2222_3333);
    let mut rolls = Rng2::with_surrogate(&mut seed, 5);
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    // Two ZoranBult at 12 apiece, one living member.
    assert_eq!(rewarded(&timeline), Some((24, 24, 6, 1)));
    assert!(
        !timeline
            .iter()
            .any(|event| matches!(event, BattleEvent::LevelUp { .. })),
        "battle cannot level: the experience it would read has not been awarded"
    );

    let chaz = &battle.into_party()[0].stats;
    assert_eq!(chaz.experience, 17);
    assert_eq!(chaz.level, 1);
    assert!(!chaz.gain_exp_flag);
}

#[test]
fn the_battle_roster_seam_composes_in_the_cartridges_order() {
    // The three stages a battle end runs, in the order `rewards`'s module note
    // fixes: absorb what the battle changed, award over the whole roster, then
    // level off the experience the award just wrote.
    use crate::roster::CharacterRoster;
    use crate::state::CharId;

    let data = fixtures::data();
    let mut roster = CharacterRoster::new();
    let mut chaz = Stats::from_character(&fixtures::chaz(), |id| data.item(id).ok().cloned());
    chaz.experience = 17;
    roster.seat(CharId(0), chaz).expect("a real seat");

    // Fight.
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        vec![PartyMember {
            character: 0,
            name: "CHAZ".into(),
            stats: roster.get(CharId(0)).expect("seated").clone(),
        }],
        &data,
        &mut rolls,
    );
    battle.roster.get_mut(id(1)).expect("Chaz").stats.curr_hp = 500;
    battle.roster.get_mut(id(1)).expect("Chaz").stats.max_hp = 500;
    let mut seed = Lcg41::new(0x2222_3333);
    let mut rolls = Rng2::with_surrogate(&mut seed, 5);
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
    let (_, each, meseta, _) = rewarded(&timeline).expect("a victory");
    assert_eq!((each, meseta), (24, 6));

    // 1. absorb: HP and status cross over, experience does not.
    assert_eq!(roster.absorb(&battle.into_party()), 1);
    assert_eq!(roster.get(CharId(0)).expect("seated").experience, 17);

    // 2. award: the roster's two passes.
    let paid = roster.award_party(&[CharId(0)], each);
    assert_eq!(paid, vec![CharId(0)]);
    let seated = roster.get(CharId(0)).expect("seated");
    assert_eq!(seated.experience, 41, "17 + 24");
    assert!(seated.gain_exp_flag, "and the flag the roster sets");

    // 3. level: reading what step 2 wrote.
    let event = level_up(0, roster.get_mut(CharId(0)).expect("seated"), &data)
        .expect("resolves")
        .expect("41 is past the 21 the level-2 record asks for");
    assert_eq!(
        event,
        BattleEvent::LevelUp {
            character: 0,
            level: 2,
            max_hp: 31,
            max_tp: 13,
        }
    );
    let seated = roster.get(CharId(0)).expect("seated");
    assert_eq!(seated.level, 2);
    assert_eq!(seated.attack.derived, 19, "and the stats refreshed");
}
