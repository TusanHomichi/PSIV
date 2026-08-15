//! Tests driven by measurements from the running cartridge.
//!
//! Everything asserted here was observed in RAM by the Genesis Plus GX harness
//! and written up in `oracle/README.md` — tapes 07 and 09 for the two basement
//! battles, 10 through 14 for the level-up, miss, escape and Defend samples.
//! Helpers come from the parent module.
//!
//! Damage is checked against the *set* of values the formula can produce rather
//! than against one roll: substituting the H/V counter (see
//! `docs/RUNTIME_DESIGN.md` "RNG design") makes the exact stream unreproducible
//! by design, so "this number is reachable" is the strongest true claim.

use super::*;

// ---------------------------------------------------------------------
// Oracle ground truth
// ---------------------------------------------------------------------

#[test]
fn every_damage_row_the_oracle_logged_is_reachable() {
    use crate::battle::action::critical_bonus;

    // (attack, defence, critical, observed) — tapes 07 and 09.
    let rows: [(u16, u16, bool, u16, &str); 12] = [
        (13, 2, false, 12, "Alys -> ZoranBult (multi)"),
        (13, 2, false, 10, "Alys -> ZoranBult (multi)"),
        (18, 2, false, 15, "Chaz -> ZoranBult, a kill"),
        (16, 9, false, 6, "ZoranBult -> Hahn"),
        (8, 2, false, 5, "Hahn -> ZoranBult"),
        (13, 2, false, 11, "Alys -> ZoranBult, a kill"),
        (13, 0, false, 13, "Alys -> Xanafalgue (multi)"),
        (13, 2, false, 10, "Alys -> ZoranBult (multi)"),
        (13, 18, false, 1, "Xanafalgue -> Alys, the floor"),
        (18, 0, false, 18, "Chaz -> Xanafalgue, a kill"),
        (8, 2, true, 7, "Hahn -> ZoranBult, the critical sample"),
        (13, 2, false, 10, "Alys -> ZoranBult, a kill"),
    ];
    for (attack, defence, critical, observed, what) in rows {
        let bonus = if critical { critical_bonus(attack) } else { 0 };
        let band = achievable(attack, defence, 2, bonus);
        assert!(
            band.contains(&observed),
            "{what}: {observed} is not in {band:?}"
        );
    }
}

#[test]
fn xanafalgue_can_never_do_more_than_one_to_alys() {
    use crate::battle::action::critical_bonus;
    use crate::battle::chances::{PHYSICAL, Verdict, calculate_chances};

    // atk 13 against dfs 18: even the maximum roll lands on the floor, so
    // the oracle's single observation is the only value it can produce.
    assert_eq!(achievable(13, 18, 2, 0), vec![1], "every roll clamps to 1");

    // A critical would break out of the floor — it reaches 3 at the top of
    // the range — but Xanafalgue can never roll one against Alys: dexterity
    // 7 against agility 15 is a margin of -8, past the -5 critical cliff.
    assert_eq!(
        *achievable(13, 18, 2, critical_bonus(13)).last().unwrap(),
        3
    );
    let (scale, miss, crit) = PHYSICAL;
    for roll in 0..=63u16 {
        let draws = [roll];
        let mut rolls = SliceRolls::new(&draws);
        assert_ne!(
            calculate_chances(7, 15, scale, miss, crit, &mut rolls),
            Verdict::Critical,
            "roll {roll}"
        );
    }
}

#[test]
fn tape_07_splits_its_rewards_the_way_the_cartridge_did() {
    // Two ZoranBult, a party of three, mash-attack until they fall.
    let data = fixtures::data();
    let mut seed = Lcg41::new(0x1234_5678);
    let mut rolls = Rng2::with_surrogate(&mut seed, 0);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    assert_eq!(
        rewarded(&timeline),
        Some((24, 8, 6, 3)),
        "24 experience over three living members, and 6 meseta"
    );
}

#[test]
fn tape_09_splits_its_rewards_the_way_the_cartridge_did() {
    let data = fixtures::data();
    let mut seed = Lcg41::new(0x0BAD_F00D);
    let mut rolls = Rng2::with_surrogate(&mut seed, 3);
    let mut battle = start(
        &fixtures::formation_xanafalgue_and_zoran_bult(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    assert_eq!(
        rewarded(&timeline),
        Some((21, 7, 5, 3)),
        "Xanafalgue 9 + ZoranBult 12, and 2 + 3 meseta"
    );
}

#[test]
fn alyss_boomerang_hits_both_enemies_in_one_swing() {
    // The oracle's headline observation: both enemy hit flags set in the
    // same frame, with different damage for each.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let mut seed = Lcg41::new(0xFEED_BEEF);
    let mut rolls = Rng2::with_surrogate(&mut seed, 1);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");

    let swing = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::Attacked { actor, targets } if *actor == id(1) => Some(targets.clone()),
            _ => None,
        })
        .expect("Alys swings");
    assert_eq!(swing, vec![id(6), id(7)], "both enemies at once");

    // And Chaz's Hunt-Knives reach exactly one.
    let chaz = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::Attacked { actor, targets } if *actor == id(2) => Some(targets.clone()),
            _ => None,
        })
        .expect("Chaz swings");
    assert_eq!(chaz.len(), 1);
}

// ---------------------------------------------------------------------------
// Oracle tapes 10-14
// ---------------------------------------------------------------------------

#[test]
fn tape_10s_critical_sample_is_reachable() {
    // f38908: Chaz hits for 22 with `hit_05 = $01`. Attack 18, critical bonus
    // 18 >> 2 = 4, against either of the two enemy types the tape fights.
    use crate::battle::action::critical_bonus;
    let bonus = critical_bonus(18);
    assert_eq!(bonus, 4);
    assert!(
        achievable(18, 0, 2, bonus).contains(&22),
        "against Xanafalgue's defence 0"
    );
    assert!(
        achievable(18, 2, 2, bonus).contains(&22),
        "against ZoranBult's defence 2"
    );
    // 22 is reachable without the bonus too, so the damage alone does not
    // identify it — `hit_05 = $01` is what makes it a critical sample. What the
    // bonus does is shift the whole band up by two.
    assert!(achievable(18, 2, 2, 0).contains(&22));
    // The bonus is doubled before the element multiply and the multiply
    // halves it back at factor 2, so a critical is worth exactly `bonus` more
    // damage: the band's top moves from 23 to 27.
    assert_eq!(achievable(18, 2, 2, 0).last(), Some(&23));
    assert_eq!(achievable(18, 2, 2, bonus).last(), Some(&27));
}

#[test]
fn tape_10s_misses_are_reachable_from_both_sides() {
    use crate::battle::chances::{PHYSICAL, Verdict, calculate_chances};
    let (scale, miss, crit) = PHYSICAL;
    let misses = |actor: i16, target: i16| {
        (0..=63u16)
            .filter(|roll| {
                let draws = [*roll];
                let mut rolls = SliceRolls::new(&draws);
                calculate_chances(actor, target, scale, miss, crit, &mut rolls) == Verdict::Miss
            })
            .count()
    };
    // f51294, party side: Hahn's dexterity 5 against ZoranBult's agility 6.
    assert!(misses(5, 6) > 0, "Hahn can miss");
    // f51340, enemy side: ZoranBult's dexterity 8 against a party agility.
    assert!(misses(8, 15) > 0, "and so can an enemy");
}

#[test]
fn a_miss_is_reported_per_resolution_rather_than_as_a_persistent_flag() {
    // The oracle's warning: `Fighters_Hit_Flags` persists between actions, so
    // reading it across a whole battle invents criticals. The timeline models
    // the verdict per attacker-target pair instead, so there is nothing to
    // scan and nothing to get wrong.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let mut seed = Lcg41::new(0x4242_4242);
    let mut rolls = Rng2::with_surrogate(&mut seed, 6);
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);

    for event in &timeline {
        if let BattleEvent::Resolved {
            verdict, damage, ..
        } = event
        {
            // A verdict and its damage can never disagree.
            assert_eq!(
                *verdict == Verdict::Miss,
                damage.is_none(),
                "a miss deals nothing and a hit deals something"
            );
        }
    }
}

#[test]
fn tape_12s_escape_threshold_is_fifty_three_in_sixty_four() {
    // The basement formations carry `run_chance = 5` and Alys's agility is 15,
    // so escape needs `roll > 10`.
    let data = fixtures::data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.run_chance = 5;

    let escaped = (0..=63u16)
        .filter(|roll| {
            let mut setup = SliceRolls::new(&[20]);
            let mut battle = start(&formation, basement_party(&data), &data, &mut setup);
            let draws = [*roll];
            let mut rolls = SliceRolls::new(&draws);
            let events = battle
                .round(&RoundOrders::Run, &data, &mut rolls)
                .expect("resolves");
            events.contains(&BattleEvent::Escaped)
        })
        .count();
    assert_eq!(escaped, 53, "53 of 64, the ~83% the oracle measured");
}

#[test]
fn both_escape_branches_are_reachable_with_a_forced_roll() {
    let data = fixtures::data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.run_chance = 5;

    // roll 11: (11 + 15 - 5) * 2 = 42 > $28 — away.
    let mut setup = SliceRolls::new(&[20]);
    let mut battle = start(&formation, basement_party(&data), &data, &mut setup);
    let draws = [11u16];
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::Run, &data, &mut rolls)
        .expect("resolves");
    assert_eq!(battle.outcome(), Some(Outcome::Escaped));
    assert_eq!(rolls.drawn(), 1, "one roll, and the round stops there");
    assert!(events.contains(&BattleEvent::Escaped));

    // roll 10: (10 + 15 - 5) * 2 = 40, exactly the bound — caught.
    let mut setup = SliceRolls::new(&[20]);
    let mut battle = start(&formation, basement_party(&data), &data, &mut setup);
    let draws: Vec<u16> = [10u16]
        .into_iter()
        .chain(std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS))
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::Run, &data, &mut rolls)
        .expect("resolves");
    assert!(
        events.contains(&BattleEvent::EscapeFailed),
        "$28 is inclusive"
    );
    assert_eq!(battle.outcome(), None);
}

#[test]
fn tape_10s_level_up_is_driven_by_the_share_battle_reports() {
    // The tape crosses 21 experience and takes level 2. Battle's half is the
    // share; `rewards::level_up` applies the record and is tested there, and
    // the seam between them is tested in the parent module.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        vec![member(&fixtures::chaz(), &data)],
        &data,
        &mut rolls,
    );
    battle.roster.get_mut(id(1)).expect("Chaz").stats.curr_hp = 500;
    battle.roster.get_mut(id(1)).expect("Chaz").stats.max_hp = 500;

    let mut seed = Lcg41::new(0x1357_9BDF);
    let mut rolls = Rng2::with_surrogate(&mut seed, 8);
    let timeline = play_out(&mut battle, &RoundOrders::attack_all(), &data, &mut rolls);
    assert_eq!(battle.outcome(), Some(Outcome::Victory));

    // Two ZoranBult at 12 each, one survivor collecting the lot.
    let (total, each, _, recipients) = rewarded(&timeline).expect("a victory");
    assert_eq!((total, each, recipients), (24, 24, 1));
    // 17 going in plus 24 clears the 21 the level-2 record asks for, which is
    // the arithmetic the tape's `exp 17 -> 26` step performs on its own party.
    assert!(17 + u32::from(each) >= 21);
}

#[test]
fn tape_14s_defend_changes_the_damage_class_and_not_the_defence_power() {
    // f30641 `$0202` -> f31391 `$0102` -> f31707 `$0202`: only the high byte
    // of the defender's own `physical_prop` moves, and `dfs_pow` never does.
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
    let before: Vec<(u8, u8, u16)> = battle
        .roster
        .side(Side::Party)
        .map(|f| (f.id.get(), f.stats.element_props[0], f.stats.defence.battle))
        .collect();
    assert_eq!(before, vec![(1, 2, 18), (2, 2, 10), (3, 2, 9)]);

    // Only Alys defends, and she is first in the queue.
    let orders = RoundOrders::Commands(vec![Command::Defend]);
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 400))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    battle.round(&orders, &data, &mut rolls).expect("resolves");

    // f31707: the round is over and the property is back, for everyone.
    let after: Vec<(u8, u8, u16)> = battle
        .roster
        .side(Side::Party)
        .map(|f| (f.id.get(), f.stats.element_props[0], f.stats.defence.battle))
        .collect();
    assert_eq!(after, before, "restored, and dfs_pow never moved at all");
}

#[test]
fn only_the_defender_takes_the_property_change() {
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    // Reach in at the point the tape samples: mid-round, defender committed.
    battle
        .roster
        .get_mut(id(1))
        .expect("Alys")
        .stats
        .begin_defending();
    let props: Vec<u8> = battle
        .roster
        .side(Side::Party)
        .map(|f| f.stats.element_props[0])
        .collect();
    assert_eq!(props, vec![1, 2, 2], "chaz and hahn are untouched");
}
