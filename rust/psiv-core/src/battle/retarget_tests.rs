//! The retarget owner: which fighter a single-target command resolves against
//! (`docs/oracle/BATTLE_ORACLE_SWEEP.md` §4.4 W1).
//!
//! Every case below names the arm of `loc_5A98` it pins: the aim kept with no
//! roll, the unique extreme taken with no roll, the one `UpdateRNGSeed2` a tie
//! costs and the slot each of its outcomes picks, the scan that takes no slot,
//! the dispatch that picks the loop, and one real command of each kind that
//! enters the routine - a swing, a technique, a skill and an item. The sixth
//! kind, the vehicle's own attack, is the same arm as a swing and is pinned in
//! `vehicle_attack_tests.rs`.

use super::*;
use crate::Inventory;
use crate::battle::item::resolve_item;
use crate::battle::skill::{Skill, resolve_skill};
use crate::battle::technique::{Technique, resolve_technique};
use crate::battle::{
    BattleData, BattleEvent, BattleItem, ItemSource, PartyMember, Reach, SliceRolls,
    candidate_targets, fixtures, status,
};

/// The maximum HP every enemy in these tests is seated with, so a slot's
/// deficit is `20 - hp` and the numbers in each assertion are readable.
const MAX_HP: u16 = 20;

/// The draws one damage run takes (`DAMAGE_DRAWS` is sixteen; the owner's own
/// tests name the count so a scan that stole a draw from it is visible).
const DAMAGE_RUN: usize = 16;

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

/// The records the four command kinds need: a damaging and a non-damaging
/// single-enemy technique, a damaging and a non-damaging single-enemy skill,
/// and an attack item (`DYNAMITE`, effect 1, target range 1).
fn data() -> BattleData {
    fixtures::data()
        .with_techniques([
            Technique {
                id: 1,
                name: "FOI".into(),
                effect: 1,
                cost: 3,
                targeting: 0x11,
                power: 24,
                resistance: 7,
                element: 3,
            },
            Technique {
                id: 18,
                name: "VOL".into(),
                effect: 2,
                cost: 8,
                targeting: 0x11,
                power: 48,
                resistance: 2,
                element: 10,
            },
        ])
        .with_skills([
            Skill {
                id: 1,
                name: "CROSSCUT".into(),
                effect: 1,
                power_stat: 5,
                requires_weapon: true,
                targeting: 0x11,
                power: 80,
                resistance: 6,
                element: 16,
            },
            Skill {
                id: 31,
                name: "EARTH".into(),
                effect: 7,
                power_stat: 2,
                requires_weapon: true,
                targeting: 0x11,
                power: 32,
                resistance: 3,
                element: 11,
            },
        ])
        .with_battle_items([BattleItem {
            id: 139,
            name: "DYNAMITE".into(),
            effect: 1,
            actor_power: 64,
            targeting: 1,
            power: 64,
            resistance: 6,
            element: 3,
            object: 21,
            consumable: true,
        }])
}

/// Chaz, Alys and Hahn against enemies whose current HP per slot is given, out
/// of [`MAX_HP`], one entry per enemy slot from 6.
fn battlefield(hp: &[u16]) -> (Roster, BattleData) {
    let data = data();
    let mut roster = Roster::new();
    for record in [fixtures::chaz(), fixtures::alys(), fixtures::hahn()] {
        let member = PartyMember::seat(&record, &data).expect("the member seats");
        roster.add_party_member(member.character, member.name, member.stats);
    }
    for (slot, hp) in hp.iter().enumerate() {
        let seated = roster
            .add_enemy(slot as u8 + 1, &fixtures::zoran_bult())
            .expect("the slot seats");
        let stats = &mut roster.get_mut(seated).expect("just seated").stats;
        stats.max_hp = MAX_HP;
        stats.curr_hp = *hp;
    }
    (roster, data)
}

/// Marks a slot fallen the way a lethal action does: no HP left and the dead
/// bit, which is the `status & $44` both loops skip.
fn fall(roster: &mut Roster, fighter: FighterId) {
    let stats = &mut roster.get_mut(fighter).expect("a seated fighter").stats;
    stats.curr_hp = 0;
    stats.status |= status::DEAD;
}

/// One aim through the owner, and how many rolls deciding it took.
fn aim(
    roster: &Roster,
    commanded: u8,
    deficit: Deficit,
    draws: &[u16],
) -> (Option<FighterId>, usize) {
    let mut rolls = SliceRolls::new(draws);
    let chosen = single_target(roster, id(commanded), deficit, &mut rolls);
    (chosen, rolls.drawn())
}

/// Lets `slot` cast `technique` this turn.
fn teach(roster: &mut Roster, slot: u8, technique: u8) {
    let caster = &mut roster.get_mut(id(slot)).expect("a seated caster").stats;
    caster.techniques[0] = technique;
    caster.curr_tp = 40;
}

/// Chaz's cast of `technique` at `commanded`: the events it produced and the
/// rolls it took, scan draw included.
fn cast(
    roster: &mut Roster,
    data: &BattleData,
    technique: u8,
    commanded: u8,
    draws: &[u16],
) -> (Vec<BattleEvent>, usize) {
    let mut rolls = SliceRolls::new(draws);
    let mut events = Vec::new();
    resolve_technique(
        roster,
        id(1),
        technique,
        Some(id(commanded)),
        data,
        &mut rolls,
        &mut events,
    );
    (events, rolls.drawn())
}

/// The fighter an event names as the one that was reached, if any.
fn reached(events: &[BattleEvent]) -> Option<FighterId> {
    events.iter().find_map(|event| match event {
        BattleEvent::Resolved { target, .. }
        | BattleEvent::FellAsleep { target, .. }
        | BattleEvent::Died { fighter: target } => Some(*target),
        _ => None,
    })
}

#[test]
fn a_living_aim_is_kept_by_both_loops_and_costs_no_draw() {
    // Slot 6 is the commanded one and stands at 15 of 20; slots 7 and 8 carry
    // 9 and 4, so both loops would move the aim off 6 if they ever ran - and
    // the draw on the stream would decide that move.
    let (roster, _) = battlefield(&[15, 9, 4]);
    for deficit in [Deficit::Largest, Deficit::Smallest] {
        assert_eq!(
            aim(&roster, 6, deficit, &[1]),
            (Some(id(6)), 0),
            "{deficit:?}: `status & $C4` clear keeps the aim, no roll"
        );
    }
}

#[test]
fn a_fallen_aim_lands_on_the_unique_extreme_with_no_draw() {
    // Deficits 4 (slot 7), 12 (8) and 9 (9): the largest is 8, the smallest 7.
    let (mut roster, _) = battlefield(&[0, 16, 8, 11]);
    fall(&mut roster, id(6));
    assert_eq!(aim(&roster, 6, Deficit::Largest, &[]), (Some(id(8)), 0));
    assert_eq!(aim(&roster, 6, Deficit::Smallest, &[]), (Some(id(7)), 0));

    // The scan ignores the dead, whichever slot the command named: a fallen 6
    // and a fallen 7 leave the next-smaller slot to win.
    let (mut roster, _) = battlefield(&[0, 0, 8, 11]);
    fall(&mut roster, id(6));
    fall(&mut roster, id(7));
    assert_eq!(aim(&roster, 7, Deficit::Smallest, &[]), (Some(id(9)), 0));
}

#[test]
fn a_tie_costs_one_draw_and_the_draw_picks_the_slot() {
    // Both loops, both outcomes: slots 7 and 8 hold the extreme and the draw
    // decides - an even one keeps the earlier slot, an odd one takes the later
    // (`btst #0, d1`, `ps4.asm:8367-8372` and `8395-8400`).
    for (deficit, hp) in [
        (Deficit::Largest, [0, 4, 4, 16]),
        (Deficit::Smallest, [0, 16, 16, 4]),
    ] {
        for (draw, slot) in [(0u16, 7u8), (1, 8)] {
            let (mut roster, _) = battlefield(&hp);
            fall(&mut roster, id(6));
            assert_eq!(
                aim(&roster, 6, deficit, &[draw]),
                (Some(id(slot)), 1),
                "{deficit:?}: one tie, one `UpdateRNGSeed2`"
            );
        }
    }
}

#[test]
fn only_a_slot_equal_to_the_running_extreme_draws() {
    // Losers that happen to be level with each other are never compared with
    // each other: at the smallest deficit, slot 7 is the extreme and 8's and
    // 9's 9s are both simply larger, so neither draws.
    let (mut roster, _) = battlefield(&[0, 16, 11, 11]);
    fall(&mut roster, id(6));
    assert_eq!(aim(&roster, 6, Deficit::Smallest, &[]), (Some(id(7)), 0));

    // And a third slot equal to the extreme draws again, because the
    // comparison the draw belongs to runs again: 7 and 9 tie at the smallest
    // deficit with the wounded 8 between them, so the stream's value reaches
    // the second tie.
    let (mut roster, _) = battlefield(&[0, 16, 4, 16]);
    fall(&mut roster, id(6));
    assert_eq!(
        aim(&roster, 6, Deficit::Smallest, &[1]),
        (Some(id(9)), 1),
        "an odd draw takes the later of the tied pair"
    );
}

#[test]
fn a_scan_that_takes_no_slot_reports_nothing_to_re_aim_at() {
    // Every enemy is down: retail's `d3` default is slot 6, which this walk has
    // just shown holds an out fighter, so this port answers `None` and the
    // caller's own turn decides.
    let (mut roster, _) = battlefield(&[0, 0, 0]);
    for slot in [6, 7, 8] {
        fall(&mut roster, id(slot));
    }
    for deficit in [Deficit::Largest, Deficit::Smallest] {
        assert_eq!(aim(&roster, 6, deficit, &[1]), (None, 0), "{deficit:?}");
    }

    // The technique path's own outcome from there: TP paid, one event, nobody
    // touched - the port's own "nobody to hit" turn by another route.
    let (mut roster, data) = battlefield(&[0, 0, 0]);
    for slot in [6, 7, 8] {
        fall(&mut roster, id(slot));
    }
    teach(&mut roster, 1, 1);
    let (events, drawn) = cast(&mut roster, &data, 1, 6, &[1]);
    assert_eq!(drawn, 0, "an empty enemy side has no deficits to tie");
    assert_eq!(
        events,
        vec![BattleEvent::TechniqueUsed {
            actor: id(1),
            technique: 1,
            name: "FOI".into(),
            remaining_tp: 37,
        }]
    );
}

#[test]
fn the_records_own_effect_nibble_selects_the_loop() {
    // `loc_5C8A` -> `loc_5CA2`: byte 0 of the ability record, masked with `$F`,
    // against `1` (`ps4.asm:8534-8537`, `8350-8351`).
    for (effect, expected) in [
        (1u8, Deficit::Largest),
        (0x11, Deficit::Largest),
        (0x21, Deficit::Largest),
        (0, Deficit::Smallest),
        (2, Deficit::Smallest),
        (7, Deficit::Smallest),
        (0x12, Deficit::Smallest),
        (18, Deficit::Smallest),
        (22, Deficit::Smallest),
        (0x0F, Deficit::Smallest),
    ] {
        assert_eq!(deficit_for_effect(effect), expected, "effect ${effect:02X}");
    }
}

#[test]
fn a_swing_re_aims_by_the_largest_deficit() {
    // The attack's own kind takes `loc_5B42`: `loc_5CA2`'s index 0 is
    // `loc_5CB0`'s `moveq #1, d0` (`ps4.asm:8515`, `8523-8524`).
    let (mut roster, _) = battlefield(&[0, 16, 8, 11]);
    fall(&mut roster, id(6));
    let mut rolls = SliceRolls::new(&[]);
    let targets = candidate_targets(&roster, id(1), Some(id(6)), Reach::Single, &mut rolls);
    assert_eq!(targets, vec![id(8)], "deficits 4/12/9: 8 is the largest");
    assert_eq!(rolls.drawn(), 0);
}

#[test]
fn a_damaging_technique_re_aims_by_the_largest_deficit() {
    // FOI's effect byte is 1, so its Kind-2 arm asks for `loc_5B42`.
    let (mut roster, data) = battlefield(&[0, 16, 8, 11]);
    fall(&mut roster, id(6));
    teach(&mut roster, 1, 1);
    let (events, drawn) = cast(&mut roster, &data, 1, 6, &[0]);
    assert_eq!(drawn, DAMAGE_RUN, "one damage run, no tiebreak");
    assert_eq!(reached(&events), Some(id(8)));
    assert_eq!(
        roster.get(id(7)).expect("present").stats.curr_hp,
        16,
        "the smallest deficit was not the one hit"
    );
}

#[test]
fn a_non_damaging_technique_re_aims_by_the_smallest_deficit() {
    // VOL's effect byte is 2, so its Kind-2 arm asks for the mirror
    // (`loc_5AFA`), and the enemy it reaches is the least damaged one.
    // Deficits 12 (slot 7), 4 (8) and 9 (9): the smallest is 8, which is also
    // *not* the first living enemy the old fallback reached for.
    let (mut roster, data) = battlefield(&[0, 8, 16, 11]);
    fall(&mut roster, id(6));
    teach(&mut roster, 1, 18);
    let (events, drawn) = cast(&mut roster, &data, 18, 6, &[63]);
    assert_eq!(drawn, 1, "one instant-death chance, no tiebreak");
    assert_eq!(reached(&events), Some(id(8)));
    assert_eq!(
        roster.get(id(7)).expect("present").stats.curr_hp,
        8,
        "the largest deficit was not the one reached"
    );
}

#[test]
fn a_technique_tie_costs_one_draw_and_the_draw_picks_the_slot() {
    // FOI's loop is the largest-deficit one: slots 7 and 8 are level at the
    // maximum (4 of 20 left each) while slot 9 is lighter, so the tiebreak is
    // the command's only scan draw and it lands before the damage run.
    for (draw, slot) in [(0u16, 7u8), (1, 8)] {
        let (mut roster, data) = battlefield(&[0, 4, 4, 16]);
        fall(&mut roster, id(6));
        teach(&mut roster, 1, 1);
        let (events, drawn) = cast(&mut roster, &data, 1, 6, &[draw]);
        assert_eq!(drawn, 1 + DAMAGE_RUN, "the tiebreak, then the damage run");
        assert_eq!(
            reached(&events),
            Some(id(slot)),
            "an even draw keeps the earlier slot, an odd one takes the later"
        );
    }

    // VOL's loop is the mirror: slots 7 and 8 are level at the smallest deficit
    // (16 of 20 left each) and slot 9 is the wounded one.
    for (draw, slot) in [(0u16, 7u8), (1, 8)] {
        let (mut roster, data) = battlefield(&[0, 16, 16, 4]);
        fall(&mut roster, id(6));
        teach(&mut roster, 1, 18);
        let (events, drawn) = cast(&mut roster, &data, 18, 6, &[draw]);
        assert_eq!(drawn, 1 + 1, "the tiebreak, then the instant-death chance");
        assert_eq!(reached(&events), Some(id(slot)));
    }
}

#[test]
fn a_skill_re_aims_by_the_loop_its_effect_selects() {
    // Crosscut's effect is 1 (largest, `loc_5CA2` index 2 -> `loc_5CBC`'s
    // `SkillData` read, `ps4.asm:8517`, `8533-8537`); Earth's is 7 (smallest).
    // Both shapes are run with the *other* extreme out of reach of the old
    // "first living enemy" fallback: Crosscut's largest deficit is slot 8 and
    // Earth's smallest is too, while slot 7 - the fallback's answer - holds a
    // mid-range one.
    for (skill, hp, expected) in [(1u8, [0, 16, 8, 11], 8u8), (31, [0, 8, 16, 11], 8)] {
        let (mut roster, data) = battlefield(&hp);
        fall(&mut roster, id(6));
        let caster = &mut roster.get_mut(id(1)).expect("present").stats;
        caster.skills[0] = skill;
        caster.curr_skill_uses[0] = 4;
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_skill(
            &mut roster,
            id(1),
            skill,
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events,
        )
        .expect("resolves");
        assert_eq!(reached(&events), Some(id(expected)), "skill {skill}");
    }
}

#[test]
fn an_attack_item_re_aims_by_the_largest_deficit() {
    // DYNAMITE's effect is 1 and its target range is 1, the one item shape
    // whose target nibble enters `loc_5A98` (`loc_5CA2` index 3 ->
    // `loc_5CCE`, `ps4.asm:8518`, `8540-8544`).
    let (mut roster, data) = battlefield(&[0, 16, 8, 11]);
    fall(&mut roster, id(6));
    let mut inventory = Inventory::new();
    inventory.add(139).expect("a free slot");
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (139, ItemSource::Inventory(0), Some(id(6))),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(
        reached(&events),
        Some(id(8)),
        "the largest deficit took the item"
    );
    assert_eq!(
        roster.get(id(7)).expect("present").stats.curr_hp,
        16,
        "the smallest deficit was not the one hit"
    );
    assert_eq!(inventory.occupied(), 0, "the item was spent");
}
