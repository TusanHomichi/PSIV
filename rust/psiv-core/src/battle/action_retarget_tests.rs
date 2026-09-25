//! The retarget scan: what a party swing does when its commanded enemy has
//! fallen (`docs/oracle/BATTLE_ORACLE_SWEEP.md` §4.4 W1, W4).
//!
//! Every case here is one arm of `loc_5A98` -> `loc_5AE6`'s `loc_5B42` loop:
//! the aim kept with no roll, the unique maximum deficit taken with no roll,
//! the tie that costs exactly one `UpdateRNGSeed2`, and the empty enemy side
//! that leaves the caller's own "no target" outcome alone.

use super::*;
use crate::battle::{PartyMember, SliceRolls, fixtures, stats::status};

/// The maximum HP every enemy in these tests is seated with, so a slot's
/// deficit is `20 - hp` and the numbers in each assertion are readable.
const MAX_HP: u16 = 20;

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

/// Chaz against MonsterFlys in enemy slots 6, 7 and 8, at the current HP given.
fn battlefield(hp: [u16; 3]) -> Roster {
    let data = fixtures::data();
    let mut roster = Roster::new();
    for record in [fixtures::chaz(), fixtures::alys()] {
        let member = PartyMember::seat(&record, &data).expect("the member seats");
        roster.add_party_member(member.character, member.name, member.stats);
    }
    for (slot, hp) in hp.iter().enumerate() {
        let enemy = fixtures::monster_fly();
        let seated = roster
            .add_enemy(slot as u8 + 1, &enemy)
            .expect("the slot seats");
        let stats = &mut roster.get_mut(seated).expect("just seated").stats;
        stats.max_hp = MAX_HP;
        stats.curr_hp = *hp;
    }
    roster
}

/// Marks a slot fallen the way a lethal swing does: no HP left and the dead
/// bit, which is the `status & $44` a scan skips.
fn fall(roster: &mut Roster, fighter: FighterId) {
    let stats = &mut roster.get_mut(fighter).expect("a seated fighter").stats;
    stats.curr_hp = 0;
    stats.status |= status::DEAD;
}

/// Chaz's single-target swing on the given stream: the slots it reaches, and
/// how many rolls it took to decide.
fn swing(roster: &Roster, commanded: u8, draws: &[u16]) -> (Vec<FighterId>, usize) {
    let mut rolls = SliceRolls::new(draws);
    let targets = candidate_targets(
        roster,
        id(1),
        Some(id(commanded)),
        Reach::Single,
        &mut rolls,
    );
    (targets, rolls.drawn())
}

#[test]
fn a_living_commanded_enemy_is_kept_and_costs_no_draw() {
    // Slot 6 is the commanded one; slot 8 carries the largest deficit, and the
    // draw on the stream would move the swing off 6 if it were taken.
    let mut roster = battlefield([15, 9, 4]);
    fall(&mut roster, id(7));
    let (targets, drawn) = swing(&roster, 6, &[1]);
    assert_eq!(
        targets,
        vec![id(6)],
        "the aim is kept, whatever the other slots read"
    );
    assert_eq!(drawn, 0, "no roll for a kept aim");
}

#[test]
fn a_fallen_commanded_enemy_lands_on_the_largest_deficit() {
    let mut roster = battlefield([0, 9, 4]);
    fall(&mut roster, id(6));
    // Deficits 11 (slot 7) and 16 (slot 8): 8 wins outright.
    let (targets, drawn) = swing(&roster, 6, &[]);
    assert_eq!(targets, vec![id(8)]);
    assert_eq!(drawn, 0, "a unique maximum needs no tiebreak");

    // The scan ignores the dead, whichever slot the command named: a fallen
    // 8 leaves the smaller 7 to win.
    let mut roster = battlefield([0, 9, 0]);
    fall(&mut roster, id(6));
    fall(&mut roster, id(8));
    let (targets, drawn) = swing(&roster, 8, &[]);
    assert_eq!(targets, vec![id(7)]);
    assert_eq!(drawn, 0);
}

#[test]
fn a_tie_at_the_maximum_costs_one_draw_and_the_draw_decides_the_slot() {
    // Slots 7 and 8 tie at 12, and 6 is the fallen command.
    for (draw, expected) in [(0u16, 7u8), (1, 8)] {
        let mut roster = battlefield([0, 8, 8]);
        fall(&mut roster, id(6));
        let (targets, drawn) = swing(&roster, 6, &[draw]);
        assert_eq!(
            targets,
            vec![id(expected)],
            "an even draw keeps the earlier slot, an odd one takes the later \
             (`btst #0, d1`)"
        );
        assert_eq!(drawn, 1, "one tie, one `UpdateRNGSeed2`");
    }

    // A tie below the maximum is not a tie at the maximum: only a slot equal
    // to the running maximum is compared against it, so the equal pair here
    // (slots 8 and 9 at 12, under slot 7's 16) never draws.
    let mut roster = battlefield([0, 4, 8]);
    fall(&mut roster, id(6));
    roster
        .add_enemy(4, &fixtures::monster_fly())
        .expect("the fourth slot seats");
    let stats = &mut roster.get_mut(id(9)).expect("just seated").stats;
    stats.max_hp = MAX_HP;
    stats.curr_hp = 8;
    let (targets, drawn) = swing(&roster, 6, &[]);
    assert_eq!(
        (targets, drawn),
        (vec![id(7)], 0),
        "only slots equal to the running maximum draw"
    );

    // And a third slot equal to the maximum draws once more - one roll per
    // tied comparison, exactly as the loop's `cmp`/`btst` pair does.
    let mut roster = battlefield([0, 8, 16]);
    fall(&mut roster, id(6));
    roster
        .add_enemy(4, &fixtures::monster_fly())
        .expect("the fourth slot seats");
    let stats = &mut roster.get_mut(id(9)).expect("just seated").stats;
    stats.max_hp = MAX_HP;
    stats.curr_hp = 8;
    // Deficits 12 (7), 4 (8), 12 (9): the second tie draws, so the stream's
    // first value only ever reaches the 7/9 comparison after 8 was skipped.
    let (targets, drawn) = swing(&roster, 6, &[0]);
    assert_eq!((targets, drawn), (vec![id(7)], 1));
}

#[test]
fn no_living_enemy_reads_an_empty_target_set_and_draws_nothing() {
    let mut roster = battlefield([0, 0, 0]);
    for slot in [6, 7, 8] {
        fall(&mut roster, id(slot));
    }
    let (targets, drawn) = swing(&roster, 6, &[1]);
    assert!(targets.is_empty(), "there is nothing to re-aim at");
    assert_eq!(drawn, 0, "an empty side has no deficits to tie");

    // And the swing the caller resolves from it is still the port's own
    // "nobody to hit" turn, unchanged.
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        id(1),
        Some(id(6)),
        &fixtures::data(),
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(
        events,
        vec![BattleEvent::TurnSkipped {
            actor: id(1),
            reason: Skipped::NoTarget
        }]
    );
}

#[test]
fn an_enemy_attacker_keeps_the_ports_own_fallback() {
    // The character-target arm of `loc_5A98` is the weighted draw
    // `engine::take_turn` makes (`loc_56F0` / `Enemy_TargetCharacter`), so a
    // fallen character leaves this port's first-survivor fallback in place and
    // the scan - which is the party's arm - never runs for an enemy.
    let mut roster = battlefield([15, 9, 4]);
    fall(&mut roster, id(1));
    let mut rolls = SliceRolls::new(&[1]);
    let targets = candidate_targets(&roster, id(6), Some(id(1)), Reach::Single, &mut rolls);
    assert_eq!(targets, vec![id(2)]);
    assert_eq!(rolls.drawn(), 0, "not this function's draw to make");
}
