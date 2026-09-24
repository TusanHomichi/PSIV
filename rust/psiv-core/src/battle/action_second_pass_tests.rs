//! The second hit pass: Alys's and Kyra's swing draws `loc_B6A2` twice.
//!
//! `Character_Attack` (`ps4.asm:13018`) runs the pass for every attacker, and
//! `Character_AttackActionOffs` (`ps4.asm:13056-13068`) sends exactly two of
//! the eleven party members — Alys (`fighter_id` 2, index 1) and Kyra
//! (`fighter_id` 10, index 9) — to `CharAttack_AlysKyra` (`ps4.asm:13958`),
//! whose `AlysKyraAttack_Init` (`ps4.asm:13975`) opens with a second
//! `jsr loc_B6A2` (`ps4.asm:13976`). `loc_B6A2` presets all nine
//! `Fighters_Hit_Flags` to `$FF` before it rolls, so the second pass's verdicts
//! are the ones that survive, and `Fighter_TakeDamage` (`ps4.asm:3564-3571`)
//! is what reads them.
//!
//! What the tests here pin, and what they cannot:
//!
//! * the draw count per pass — one `loc_B716` roll per living target, both
//!   passes before any damage draw. Tape 07's Alys swing is the same shape: the
//!   log's frames hold 36 calls for it, four at f29489 (two passes over two
//!   enemies) and thirty-two damage draws at f29599.
//! * which pass decides. A first-pass miss that the second pass turns into a
//!   hit lands, and a first-pass hit the second pass misses is a miss.
//! * the negative control: an attacker `Character_AttackActionOffs` does not
//!   send to `CharAttack_AlysKyra` — Chaz, and an enemy's plain attack — keeps
//!   one pass.
//!
//! **Retail reachability** is why the port models the second pass over *its own*
//! target list. Everything Alys or Kyra can wear is item type 2, the
//! one-handed multi-target class (`Battle_AttackCommand`, `ps4.asm:2244-2248`,
//! and the pack's equip masks: seven weapon records, every one of them
//! Alys/Kyra-only), and `loc_168C` (`ps4.asm:2293-2301`) stores `$FFFF` as the
//! command's target for those — so in the cartridge both passes always walk all
//! four enemy slots (`ps4.asm:17504-17511`), which is the set the
//! port's `Reach::All` produces. A single-target weapon in either hand is
//! unreachable; docs/source-notes/battle-party.md records what the cartridge's window would do
//! with one, and [`a_one_target_alys_swing_draws_the_pass_twice`] drives the
//! port's own `Reach::Single` to make the count visible.

use super::*;
use crate::battle::damage::DAMAGE_DRAWS;
use crate::battle::rng::SliceRolls;
use crate::battle::{fixtures, party_fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).expect("a valid id")
}

/// Alys, Chaz and Hahn against two ZoranBults.
///
/// Each member is seated under the `Character_Stats` index its record carries —
/// the identity the second-pass rule reads — and those indices are the
/// cartridge's own (Alys 1, Chaz 0, Hahn 2).
fn party_and_enemies() -> (Roster, BattleData) {
    let data = fixtures::data();
    let items = fixtures::items();
    let lookup = |id: u8| items.iter().find(|i| i.id == id).cloned();
    let mut roster = Roster::new();
    for record in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()] {
        roster.add_party_member(
            record.id,
            record.name.clone(),
            Stats::from_character(&record, lookup),
        );
    }
    roster.add_enemy(1, &fixtures::zoran_bult());
    roster.add_enemy(2, &fixtures::zoran_bult());
    (roster, data)
}

/// A hit-pass stream and the damage runs for the targets that land.
///
/// Every damage draw is 7, so the sixteen of them sum to 112, and with Alys's
/// attack of 13 against the bult's defence of 2 and its physical factor of 2
/// that is `((112 + 8) * 13) >> 6 = 24`, `+ 13 = 37`, `* 2 >> 2 = 18`, `- 2` =
/// 16.
fn draws_for(hit_rolls: &[u16], landed: usize) -> Vec<u16> {
    let mut draws = hit_rolls.to_vec();
    draws.extend(std::iter::repeat_n(7u16, landed * DAMAGE_DRAWS));
    draws
}

fn resolutions(events: &[BattleEvent]) -> Vec<(FighterId, Verdict, Option<u16>)> {
    events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Resolved {
                target,
                verdict,
                damage,
                ..
            } => Some((*target, *verdict, *damage)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_second_pass_is_alys_and_kyra_only() {
    let (mut roster, data) = party_and_enemies();
    for actor in [id(1), id(2), id(3), id(6)] {
        let name = roster.get(actor).map_or(String::new(), |f| f.name.clone());
        assert_eq!(
            takes_second_hit_pass(&roster, actor),
            actor == id(1),
            "{name} at slot {}",
            actor.get()
        );
    }

    // Kyra is the table's tenth entry and carries a LacoSlashr — a type 2
    // weapon, so her reach is every enemy like Alys's. Her record names pack
    // items the fixture table does not hold, so the data set is the pack's.
    let data = data.with_items(party_fixtures::equipment());
    let kyra = party_fixtures::records()
        .into_iter()
        .find(|record| record.id == 9)
        .expect("the pack's Kyra record");
    let equipment = party_fixtures::equipment();
    let lookup = |id: u8| equipment.iter().find(|i| i.id == id).cloned();
    let kyra_id = roster
        .add_party_member(
            kyra.id,
            kyra.name.clone(),
            Stats::from_character(&kyra, lookup),
        )
        .expect("a free party slot");
    assert!(takes_second_hit_pass(&roster, kyra_id));

    // And her swing pays for it: two living enemies, two passes.
    let draws = draws_for(&[63, 63, 63, 63], 2);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(&mut roster, kyra_id, None, &data, &mut rolls, &mut events).expect("resolves");
    assert_eq!(rolls.drawn(), 4 + 2 * DAMAGE_DRAWS);
    assert_eq!(
        resolutions(&events).len(),
        2,
        "her LacoSlashr reaches both enemies"
    );
}

#[test]
fn a_two_target_alys_swing_draws_the_pass_twice() {
    let (mut roster, data) = party_and_enemies();
    // Her dexterity drops the first pass to a miss and the second to a hit, so
    // the count and the winner are both visible: v = (r + 1 - 6) * 2, a miss
    // for r = 0 and a normal hit for r = 40.
    roster.get_mut(id(1)).expect("Alys").stats.dexterity.battle = 1;
    let draws = draws_for(&[0, 0, 40, 40], 2);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(&mut roster, id(1), None, &data, &mut rolls, &mut events).expect("resolves");
    assert_eq!(
        rolls.drawn(),
        4 + 2 * DAMAGE_DRAWS,
        "two targets, two passes, then two sixteen-draw damage runs — the 36 \
         calls tape 07's f29489/f29599 window holds for her swing"
    );
    assert_eq!(
        resolutions(&events),
        vec![
            (id(6), Verdict::Normal, Some(16)),
            (id(7), Verdict::Normal, Some(16))
        ],
        "the second pass's rolls are the ones that decide"
    );
    assert_eq!(roster.get(id(6)).expect("enemy 1").stats.curr_hp, 9);
}

#[test]
fn a_one_target_alys_swing_draws_the_pass_twice() {
    let (mut roster, data) = party_and_enemies();
    // A Hunt-Knife in the right hand is item type 1, so the port's reach is one
    // enemy — a weapon the equip mask refuses Alys in retail, which is the only
    // way to ask this question at all. Only the reach changes: the stats stay
    // the ones `Stats::from_character` derived with the Boomerang in hand.
    roster.get_mut(id(1)).expect("Alys").stats.equipment[0] = 2;
    assert_eq!(
        weapon_reach(&roster.get(id(1)).expect("Alys").stats, &data),
        Ok(Some(Reach::Single))
    );
    let draws = draws_for(&[40, 40], 1);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        id(1),
        Some(id(6)),
        &data,
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(
        rolls.drawn(),
        2 + DAMAGE_DRAWS,
        "one target, two passes, then one damage run"
    );
    assert_eq!(
        events.iter().find_map(|event| match event {
            BattleEvent::Attacked { targets, .. } => Some(targets.clone()),
            _ => None,
        }),
        Some(vec![id(6)]),
        "the port's reach still decides who is swung at"
    );
}

#[test]
fn a_missed_first_pass_lands_on_the_second() {
    let (mut roster, data) = party_and_enemies();
    // One target, so the stream is unambiguously one roll per pass.
    roster.get_mut(id(1)).expect("Alys").stats.equipment[0] = 2;
    roster.get_mut(id(1)).expect("Alys").stats.dexterity.battle = 1;
    // One roll for the first pass (a miss), one for the second (a hit), then
    // the damage run.
    let draws = draws_for(&[0, 40], 1);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        id(1),
        Some(id(6)),
        &data,
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(rolls.drawn(), 2 + DAMAGE_DRAWS);
    assert_eq!(
        resolutions(&events),
        vec![(id(6), Verdict::Normal, Some(16))],
        "the first pass's miss does not survive: loc_B6A2 re-fills every flag"
    );
    assert_eq!(roster.get(id(6)).expect("enemy").stats.curr_hp, 9);
}

#[test]
fn a_landed_first_pass_misses_on_the_second() {
    let (mut roster, data) = party_and_enemies();
    roster.get_mut(id(1)).expect("Alys").stats.equipment[0] = 2;
    roster.get_mut(id(1)).expect("Alys").stats.dexterity.battle = 1;
    // The mirror image: the first pass rolls a hit, the second rolls a miss, so
    // the swing whiffs and costs no damage draws.
    let mut rolls = SliceRolls::new(&[40, 0]);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        id(1),
        Some(id(6)),
        &data,
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(rolls.drawn(), 2, "a miss costs no damage draws");
    assert_eq!(resolutions(&events), vec![(id(6), Verdict::Miss, None)]);
    assert_eq!(roster.get(id(6)).expect("enemy").stats.curr_hp, 25);
}

#[test]
fn a_dead_slot_costs_neither_pass_a_roll() {
    let (mut roster, data) = party_and_enemies();
    roster.get_mut(id(6)).expect("enemy 1").stats.status = crate::battle::stats::status::DEAD;
    // Alys keeps the Boomerang, so her reach is every living enemy: one roll
    // per pass over the one survivor.
    let draws = draws_for(&[40, 40], 1);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(&mut roster, id(1), None, &data, &mut rolls, &mut events).expect("resolves");
    assert_eq!(
        rolls.drawn(),
        2 + DAMAGE_DRAWS,
        "the dead slot is written off as $FF in both passes, without a roll"
    );
}

#[test]
fn an_unlisted_attacker_draws_one_pass() {
    // The negative control: Chaz is `Character_AttackActionOffs`'s first entry
    // (`CharAttack_Chaz`, `ps4.asm:13613`), which reaches `loc_B6A2` once, from
    // `Character_Attack` itself.
    let (mut roster, data) = party_and_enemies();
    assert!(!takes_second_hit_pass(&roster, id(2)));
    let draws = draws_for(&[40], 1);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        id(2),
        Some(id(6)),
        &data,
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(rolls.drawn(), 1 + DAMAGE_DRAWS, "one pass, one damage run");

    // And on the other side of the field: `Enemy_Attack` reaches `loc_D09E`
    // (`ps4.asm:19200-19201`), a single pass, for every enemy there is.
    let (mut roster, data) = party_and_enemies();
    assert!(!takes_second_hit_pass(&roster, id(6)));
    let draws = draws_for(&[40], 1);
    let mut rolls = SliceRolls::new(&draws);
    let mut events = Vec::new();
    resolve_attack(
        &mut roster,
        id(6),
        Some(id(1)),
        &data,
        &mut rolls,
        &mut events,
    )
    .expect("resolves");
    assert_eq!(rolls.drawn(), 1 + DAMAGE_DRAWS, "one pass, one damage run");
}
