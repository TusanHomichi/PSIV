//! Tape 09's second basement battle: the same replay on a second data point.
//!
//! `oracle/tapes/09_second_battle.tape`, frames 25002-31908. A different
//! formation (`oracle/battle_fixture.py` extracted it with nothing but
//! `--battle-first`/`--battle-last` and `--tape`, exactly as it extracted tape
//! 07's), a different seed path, and a round order that puts an enemy between
//! Alys and Chaz. `docs/BATTLE_ORACLE_REPLAY.md` records what the pair of
//! fixtures adds up to; the fixture itself carries the trace's sha256 so the
//! numbers below can be traced back to one oracle run.

use super::replay::*;

use crate::battle::fixtures;
use crate::battle::*;

const FIXTURE: &str = include_str!("replay_fixtures/tape09_second_battle.json");

#[test]
fn tape09_replays_the_cartridges_battle_on_the_verbatim_stream() {
    let fixture = fixture(FIXTURE);
    let data = fixtures::data();

    // The trace's 137 calls, 135 of them the battle's own.
    assert_eq!(fixture.provenance.roll_count, 137);
    assert_eq!(fixture.provenance.battle_roll_count, 135);
    assert_eq!(
        fixture.provenance.roll_column.agrees, 137,
        "every trace row's own roll column is the cartridge's roll, so the \
         fixture checks each row against the raw columns; see \
         docs/BATTLE_ORACLE_REPLAY.md"
    );
    assert_eq!(fixture.provenance.roll_column.subtracts_low_word, 0);
    let outside = fixture.outside_rolls.rolls();
    assert_eq!(
        outside
            .iter()
            .map(|roll| (roll.role.as_str(), roll.frame))
            .collect::<Vec<(&str, u32)>>(),
        vec![("formation", 25015), ("item_drop", 31786)]
    );

    // Two rounds, and the second one has an enemy between Alys and Chaz: this
    // battle's queue is not tape 07's, which is the point of a second fixture.
    assert_eq!(
        fixture
            .rounds
            .iter()
            .map(|round| (round.round, round.order.clone()))
            .collect::<Vec<(u16, Vec<u8>)>>(),
        vec![(1, vec![1, 6, 2, 3, 7]), (2, vec![1, 7, 2, 3])]
    );

    // A critical, which tape 07's battle does not have: Hahn's swing at
    // f31294 leaves `Fighters_Hit_Flags` at `$01` on Enemy2.
    let critical = fixture
        .rounds
        .iter()
        .flat_map(|round| round.actions.iter())
        .flat_map(|action| {
            action
                .targets
                .iter()
                .filter(|target| target.hit == "01")
                .map(move |target| (action.actor, action.start_frame, target.id))
        })
        .collect::<Vec<(u8, u32, u8)>>();
    assert_eq!(
        critical,
        vec![(3, 31294, 7)],
        "the log's one critical, and the port resolves it the same way"
    );

    let (mut battle, timelines) = replay_verbatim(&fixture, &data, "tape 09");

    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    // `$FFFFEEA8` again, against the RAM log: tape 09's word moves `0000 ->
    // 0004` at f31044 and `0004 -> 0001` at f31381 - the two enemies' ability
    // draws, `8084 & 7` and `19569 & 7` - and the battle ends holding that 1.
    assert_eq!(battle.last_ability_index(), 1);
    assert!(fixture.outcome.victory, "the log has every enemy down");
    assert_eq!(
        fixture.outcome.dead_enemy_ids,
        fixture
            .formation
            .enemies
            .iter()
            .map(|enemy| enemy.id)
            .collect::<Vec<u8>>()
    );

    // 21 experience over the three who lived and 5 meseta, from the log's own
    // accumulators.
    let rewarded = timelines
        .last()
        .and_then(|timeline| {
            timeline
                .iter()
                .find(|event| matches!(event, BattleEvent::Rewarded { .. }))
        })
        .expect("the battle pays out");
    let BattleEvent::Rewarded {
        experience_total,
        meseta,
        ..
    } = rewarded
    else {
        unreachable!("just matched");
    };
    assert_eq!(*experience_total, fixture.outcome.experience_total);
    assert_eq!(*meseta, fixture.outcome.meseta);
    assert_eq!(fixture.outcome.experience_total, 21);
    assert_eq!(fixture.outcome.meseta, 5);

    let timeline = battle
        .round(&RoundOrders::attack_all(), &data, &mut SliceRolls::new(&[]))
        .expect("a battle that has ended resolves to nothing");
    assert!(timeline.is_empty(), "the battle is over: no third round");
}

#[test]
fn every_roll_tape09s_frames_hold_is_one_the_port_consumes() {
    account_for_every_roll(&fixture(FIXTURE), "tape 09");
}
