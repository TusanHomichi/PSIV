//! End-to-end tests for [`super::Battle`].
//!
//! Split out of `engine.rs` under the repo's 1,000-line rule; included with
//! `#[path]` so it stays a child module and can reach the engine's private
//! state, which several of these assert against directly.
//!
//! The cartridge-derived numbers come from `docs/BATTLE_SCOUT.md` §12 (the
//! worked example) and `oracle/README.md` "Battle ground truth" (tapes 07 and
//! 09), by way of [`crate::battle::fixtures`].

use super::*;
use crate::battle::damage::{DAMAGE_DRAWS, calculate_damage, clamp_damage};
use crate::battle::fighters::FIGHTER_SLOTS;
use crate::battle::fixtures;
use crate::battle::records::{CharacterRecord, FormationRecord};
use crate::battle::rewards::level_up;
use crate::battle::rng::{Lcg41, Rng2, SliceRolls};
use crate::battle::stats::status;
use crate::state::VehicleRecord;

fn id(n: u8) -> FighterId {
    FighterId::new(n).expect("a valid id")
}

fn member(record: &CharacterRecord, data: &BattleData) -> PartyMember {
    let item = |id: u8| data.item(id).ok().cloned();
    PartyMember {
        character: record.id,
        name: record.name.clone(),
        stats: Stats::from_character(record, item),
    }
}

/// Tape 07 and 09's party: Alys, Chaz, Hahn, in that slot order.
fn basement_party(data: &BattleData) -> Vec<PartyMember> {
    vec![
        member(&fixtures::alys(), data),
        member(&fixtures::chaz(), data),
        member(&fixtures::hahn(), data),
    ]
}

fn start(
    formation: &FormationRecord,
    party: Vec<PartyMember>,
    data: &BattleData,
    rolls: &mut impl Rolls,
) -> Battle {
    Battle::start(formation, party, data, false, rolls)
        .expect("the fixtures resolve")
        .0
}

/// Runs a battle to its end on a deterministic source, returning the whole
/// timeline. Bounded so a stalled engine fails loudly instead of hanging.
fn play_out(
    battle: &mut Battle,
    orders: &RoundOrders,
    data: &BattleData,
    rolls: &mut impl Rolls,
) -> Vec<BattleEvent> {
    let mut timeline = Vec::new();
    for _ in 0..64 {
        if battle.outcome().is_some() {
            return timeline;
        }
        timeline.extend(battle.round(orders, data, rolls).expect("resolves"));
    }
    panic!("a battle that will not end in 64 rounds");
}

fn rewarded(timeline: &[BattleEvent]) -> Option<(u16, u16, u16, usize)> {
    timeline.iter().find_map(|event| match event {
        BattleEvent::Rewarded {
            experience_total,
            experience_each,
            meseta,
            recipients,
        } => Some((
            *experience_total,
            *experience_each,
            *meseta,
            recipients.len(),
        )),
        _ => None,
    })
}

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

#[test]
fn an_unimplemented_ability_is_announced_rather_than_faked() {
    let mut record = fixtures::zoran_bult();
    record.regular_abilities = [7; 8];
    let data = fixtures::data().with_enemies([record]);

    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { ability: 7, .. }))
    );
}

#[test]
fn a_vehicle_skill_consumes_a_use_and_runs_its_retail_dispatcher() {
    let data = fixtures::data();
    let vehicle = crate::vehicle::battle_member(
        1,
        VehicleRecord {
            current_hp: 500,
            max_hp: 500,
            skill_mask: 0x03,
            current_skill_uses: [1, 0, 0, 0, 0, 0, 0, 0],
            max_skill_uses: [1, 0, 0, 0, 0, 0, 0, 0],
            ..VehicleRecord::default()
        },
    )
    .expect("Land Rover battle member");
    let mut setup_rolls = SliceRolls::new(&[20]);
    let (mut battle, _) = Battle::start_vehicle(
        &fixtures::formation_two_zoran_bults(),
        vec![vehicle],
        &data,
        &mut setup_rolls,
    )
    .expect("vehicle battle starts");
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::VehicleSkill(1)]),
            &data,
            &mut rolls,
        )
        .expect("vehicle round resolves");

    assert!(events.contains(&BattleEvent::VehicleSkillUsed {
        actor: id(1),
        skill: 1,
        remaining: 0,
    }));
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::VehicleSkillEffect {
            actor,
            skill: 1,
            effect: crate::battle::VehicleSkillEffectKind::Damage,
            ..
        } if *actor == id(1)
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        BattleEvent::VehicleSkillEffectUnavailable { actor, skill: 1 } if *actor == id(1)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::Attacked { actor, .. } if *actor == id(1)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::Resolved { actor, .. } if *actor == id(1)
    )));
    assert_eq!(
        battle
            .party_stats()
            .next()
            .expect("vehicle stats")
            .1
            .curr_skill_uses[0],
        0
    );
}

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
        Battle::start(&empty, Vec::new(), &data, false, &mut rolls),
        Err(BattleDataError::EmptyFormation(7))
    );
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

#[path = "engine_tests_oracle.rs"]
mod oracle;

#[path = "engine_tests_tail.rs"]
mod tail;

use tail::achievable_impl as achievable;
