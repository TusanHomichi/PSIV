//! Tape 07's first basement battle, replayed against the cartridge's own rolls.
//!
//! `oracle/tapes/07_first_battle.tape`, frames 24794-30428; the capture, the
//! trace and what it proves are in `docs/BATTLE_ORACLE_REPLAY.md`. Three
//! claims, one test each:
//!
//! * [`tape07_orders_its_rounds_the_way_the_cartridge_did`] - the fixture's own
//!   structure, and round 1 walked action by action.
//! * [`tape07_replays_the_cartridges_battle_on_the_verbatim_stream`] - the whole
//!   battle on the cartridge's own stream, nothing filtered out.
//! * [`every_roll_the_frames_hold_is_one_the_port_consumes`] - the per-action
//!   draw accounting the first two rest on.

use super::replay::*;

use crate::battle::fixtures;
use crate::battle::*;

const FIXTURE: &str = include_str!("replay_fixtures/tape07_first_battle.json");

#[test]
fn tape07_orders_its_rounds_the_way_the_cartridge_did() {
    let fixture = fixture(FIXTURE);
    let data = fixtures::data();

    // The trace carries 136 calls; the battle's own stream is 134 of them: the
    // encounter's formation draw (f24807) and the post-victory item drop draw
    // (f30306) sit outside every battle routine.
    assert_eq!(fixture.provenance.roll_count, 136);
    assert_eq!(fixture.provenance.battle_roll_count, 134);
    assert_eq!(
        fixture.provenance.roll_column.agrees, 136,
        "every trace row's own roll column is the cartridge's roll, so the \
         fixture checks each row against the raw columns; see \
         docs/BATTLE_ORACLE_REPLAY.md"
    );
    assert_eq!(fixture.provenance.roll_column.subtracts_low_word, 0);
    assert_eq!(fixture.provenance.trace_sha256.len(), 64);

    // The two draws that are not the battle's own: the encounter's formation
    // draw, before the enemies existed in RAM, and the item drop draw after the
    // victory. They are recorded, and they are not replayed.
    let outside = fixture.outside_rolls.rolls();
    assert_eq!(
        outside
            .iter()
            .map(|roll| (roll.role.as_str(), roll.frame))
            .collect::<Vec<(&str, u32)>>(),
        vec![("formation", 24807), ("item_drop", 30306)]
    );

    let logged_rolls = fixture.rolls.rolls();
    let priority = priority_roll(&fixture);
    assert_eq!(priority.len(), 1, "the battle opens on one draw");
    let mut start_rolls = SliceRolls::new(&priority);
    let mut battle = start(&fixture, &data, &mut start_rolls);
    assert_eq!(
        start_rolls.drawn(),
        1,
        "Battle::start draws loc_B62A's roll"
    );

    let mut first_round = true;
    for round in &fixture.rounds {
        let stream = verbatim_round(&fixture, round.round);
        assert_eq!(
            stream.len() as u32,
            round.roll_count,
            "round {}: the fixture's own roll count",
            round.round
        );
        let (timeline, drawn) = play(&mut battle, &fixture, &data, round, &stream);
        let first = divergence(round, &timeline, &logged_rolls);

        if first_round {
            first_round = false;
            // The queue build is the last thing both sides do before an action
            // resolves, so a queue that matches is every roll of
            // `Battle_OrderTurns` matching: nine jitter draws and one target
            // draw per enemy slot, in the log's own order.
            assert!(
                !matches!(first, Some(Divergence::Queue { .. })),
                "round 1's queue is the log's Battle_Turn_Order at f{}: {first:?}",
                round.order_frame
            );
            // The log's own pair list says the same thing: sorting the
            // fighters by the ordering value it recorded, stable, reproduces
            // the order it recorded them in.
            let mut by_ordering: Vec<(u8, u16)> = round
                .order
                .iter()
                .copied()
                .zip(round.ordering.iter().copied())
                .collect();
            by_ordering.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            assert_eq!(
                by_ordering
                    .iter()
                    .map(|(fighter, _)| *fighter)
                    .collect::<Vec<u8>>(),
                round.order,
                "f{}: Battle_Turn_Order's ids and ordering values agree",
                round.order_frame
            );

            // And the round is the log's end to end, on the log's own stream.
            // This is where the two sides used to part: `Enemy_Attack` re-rolls
            // an ability equal to `$FFFFEEA8` (`ps4.asm:19146-19151`), and this
            // battle loads with that word at zero — `GameMode_LoadBattle` wipes
            // the page it lives in (`ps4.asm:9992-9994`) — so the cartridge
            // spends a second call on the draw of zero. The port did not draw
            // it, so its hit roll read the re-roll's value and every later
            // window was one early.
            assert_eq!(first, None, "round 1 on the cartridge's own stream");

            // The counts behind it. Alys's swing's frames hold 36 calls — four
            // hit rolls at f29489 (two passes over two enemies) and thirty-two
            // damage draws at f29599 — and the enemy's turn holds 19: one
            // ability draw, the re-roll that zero costs it, one hit roll and
            // sixteen damage draws.
            let alys = &round.actions[0];
            assert_eq!(alys.actor, 1);
            assert_eq!(alys.roll_count, 36);
            let alys_rolls = action_rolls(&fixture, round, alys);
            assert_eq!(alys_rolls.len(), 36);
            // The four hit rolls at f29489 are the two passes her swing makes,
            // each over both enemies: `Character_Attack`'s, then the animation's
            // (`AlysKyraAttack_Init`, ps4.asm:13975-13976).
            assert_eq!(
                alys_rolls
                    .iter()
                    .filter(|roll| roll.role == "hit")
                    .map(|roll| (roll.target, roll.pass_number))
                    .collect::<Vec<(Option<u8>, u8)>>(),
                vec![(Some(6), 1), (Some(7), 1), (Some(6), 2), (Some(7), 2)],
                "two passes over both enemies"
            );

            let enemy = &round.actions[2];
            assert_eq!(enemy.actor, 7);
            assert_eq!(enemy.roll_count, 19);
            let enemy_rolls = action_rolls(&fixture, round, enemy);
            assert_eq!(enemy_rolls.len(), 19, "the re-roll included");
            let reroll = enemy_rolls
                .iter()
                .find(|roll| roll.role == "ability_reroll")
                .expect("one draw of zero, one re-roll");
            assert_eq!(reroll.pass_number, 0, "the re-roll is not a hit pass");
            assert_eq!(
                enemy_rolls
                    .iter()
                    .find(|roll| roll.role == "ability")
                    .map(|roll| roll.roll & 7),
                Some(0),
                "the draw that cost the re-roll is the zero $FFFFEEA8 held"
            );
            assert_eq!(
                reroll.roll & 7,
                3,
                "and the re-roll is the index the RAM log's word moves to at \
                 f29789 (docs/BATTLE_ORACLE_REPLAY.md)"
            );

            // Thirteen order draws, then 36 + 17 + 19 + 17: the port reads the
            // round's 102 calls, all of them, and stops.
            assert_eq!(drawn, 102);

            // The fixture's own windows: each action sits inside the battle and
            // the actions follow each other.
            for pair in round.actions.windows(2) {
                assert!(
                    pair[0].end_frame < pair[1].start_frame,
                    "f{} closes before f{} opens",
                    pair[0].end_frame,
                    pair[1].start_frame
                );
            }
            assert!(round.actions[0].start_frame > round.order_frame);
        }
    }

    // A battle that went differently is still a battle: the engine carries the
    // divergence to an outcome instead of stalling.
    assert!(battle.outcome().is_some(), "the replay finishes");
}

#[test]
fn tape07_replays_the_cartridges_battle_on_the_verbatim_stream() {
    let fixture = fixture(FIXTURE);
    let data = fixtures::data();
    let (mut battle, timelines) = replay_verbatim(&fixture, &data, "tape 07");

    assert_eq!(battle.outcome(), Some(Outcome::Victory));
    // The word the battle leaves behind is the RAM log's: `$FFFFEEA8` reads
    // `0003` from f29789 to the end of the tape, and the fixture's re-roll of
    // `58235 & 7` is that 3 (see docs/BATTLE_ORACLE_REPLAY.md).
    assert_eq!(battle.last_ability_index(), 3);
    let outcome = &fixture.outcome;
    assert!(outcome.victory, "the log has every enemy down");
    assert_eq!(
        outcome.dead_enemy_ids,
        fixture
            .formation
            .enemies
            .iter()
            .map(|enemy| enemy.id)
            .collect::<Vec<u8>>(),
        "and the log has each of them at zero HP or below"
    );

    // The rewards the port computed, against the log's own accumulators: 24
    // experience over the three who lived (the log shows Chaz 0 -> 8, Hahn
    // 0 -> 8 and Alys 2457 -> 2465) and 6 meseta (600 -> 606).
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
        experience_each,
        meseta,
        recipients,
    } = rewarded
    else {
        unreachable!("just matched");
    };
    assert_eq!(*experience_total, 24);
    assert_eq!(*experience_total, outcome.experience_total);
    assert_eq!(*meseta, 6);
    assert_eq!(*meseta, outcome.meseta);
    assert_eq!(recipients.len(), fixture.party.len());
    assert_eq!(
        *experience_each,
        outcome.experience_total / fixture.party.len() as u16,
        "the log's divisor rule: the pool over the living"
    );

    let timeline = battle
        .round(&RoundOrders::attack_all(), &data, &mut SliceRolls::new(&[]))
        .expect("a battle that has ended resolves to nothing");
    assert!(timeline.is_empty(), "the battle is over: no third round");
}

#[test]
fn every_roll_the_frames_hold_is_one_the_port_consumes() {
    account_for_every_roll(&fixture(FIXTURE), "tape 07");
}
