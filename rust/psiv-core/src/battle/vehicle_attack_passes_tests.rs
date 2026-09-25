//! The per-vehicle `loc_B6A2` pass count, and the Ice Digger's two-pass swing.
//!
//! A vehicle's swing re-enters state 5 only when its attack object hands
//! `action_routine` back (`move.w #5, $32(a0)`), and whether that write lands
//! after the vehicle has left state 5 is the object's `$1C` wind-up:
//!
//! * `BattleObj_LandRoverAtk` (`$644`) and `BattleObj_HydrofoilAtk` (`$64C`)
//!   are created with `move.w #$C, $1C(a4)` (`ps4.asm:82945`, `83071`) and
//!   hand back thirteen countdown frames later — three passes;
//! * `BattleObj_IceDiggerAtk` (`$648`) is created with `clr.w $1C(a4)`
//!   (`ps4.asm:82996`) and hands back in its creation frame, while state 4's
//!   own `addq.w #1, $32(a4)` is still to come — two passes.
//!
//! What the tests here pin, in that order: the timer each object is created
//! with and the count it yields ([`hit_passes`]); the draw count each vehicle
//! spends through the engine's own path; the Ice Digger's capture replayed on
//! its own rolls, both as recorded and as the pre-change three-pass rule would
//! have read them (the negative control); and that the swing's *last* pass —
//! the second, for the Digger — is the one whose verdict survives.
//!
//! The rolls are `replay_fixtures/forced_53_icedigger.json`'s own rows, read
//! by frame: f25058, f25059 and the sixteen at f25075.

use super::tests::{data, hp, id, resolution, roster, swing_at};
use super::*;
use crate::battle::damage::{DAMAGE_DRAWS, calculate_damage, clamp_damage};
use crate::battle::resolve_attack;
use crate::battle::rng::SliceRolls;
use crate::vehicle;

/// The Ice Digger's two hit-pass rolls, f25058 and f25059.
const ICEDIGGER_HITS: [u16; 2] = [17827, 58833];

/// Its sixteen damage draws, f25075.
const ICEDIGGER_DRAWS: [u16; DAMAGE_DRAWS] = [
    33410, 32810, 65302, 49111, 40627, 3771, 50904, 41982, 37205, 2179, 50227, 8739, 53859, 43266,
    38123, 35576,
];

/// The next roll in the capture's stream after that swing: round 2's first, the
/// ability roll at f25160. A third pass would reach into it.
const NEXT_ROLL: u16 = 14171;

#[test]
fn each_vehicles_pass_count_comes_from_its_attack_object() {
    // `loc_B15C`'s three entries (`ps4.asm:16964-16968`) in `Vehicle_Index`
    // order, and the object ids they hand the loader (`ps4.asm:74660-74662`).
    for (index, object_id, routine, wind_up) in [
        (1u16, 0x644u16, "BattleObj_LandRoverAtk", 12u16),
        (2, 0x648, "BattleObj_IceDiggerAtk", 0),
        (3, 0x64C, "BattleObj_HydrofoilAtk", 12),
    ] {
        let object = attack_object(index).expect("a retail vehicle's attack object");
        assert_eq!(object.object_id, object_id);
        assert_eq!(object.routine, routine);
        assert_eq!(object.wind_up, wind_up, "the creator's own `$1C` write");
        // The count is that timer: a zero wind-up hands state 5 back in the
        // creation frame, a nonzero one thirteen countdown frames later, once
        // the vehicle is already in 6.
        assert_eq!(hit_passes(index), if wind_up == 0 { 2 } else { 3 });
    }
    assert_eq!(VEHICLE_ATTACK_OBJECTS.len(), 3, "every VehicleData record");
    assert_eq!(hit_passes(1), 3);
    assert_eq!(hit_passes(2), 2);
    assert_eq!(hit_passes(3), 3);

    // A selector `VehicleData` does not cover is answered with the longest
    // swing: `resolve_attack` dispatches on `is_vehicle_fighter`, so the guard
    // is only reachable by calling that resolver directly.
    for index in [0u16, 4, 0xFFFF] {
        assert_eq!(attack_object(index), None);
        assert_eq!(hit_passes(index), 3);
    }
}

#[test]
fn a_swing_draws_its_own_pass_count_then_the_damage_run() {
    // The engine's own path for each retail vehicle: one roll per pass, then
    // the sixteen `Battle_CalculateDamage` draws, and nothing after them.
    for index in 1..=3u16 {
        let (mut roster, actor) = roster(index, 740);
        let stream = vec![0u16; hit_passes(index) + DAMAGE_DRAWS + 4];
        let mut rolls = SliceRolls::new(&stream);
        let mut events = Vec::new();
        resolve_attack(&mut roster, actor, None, &data(), &mut rolls, &mut events)
            .expect("a vehicle swing resolves");
        assert_eq!(
            rolls.drawn(),
            hit_passes(index) + DAMAGE_DRAWS,
            "{}: its own passes, then one damage run",
            vehicle::profile(index).expect("a retail vehicle").name
        );
        assert_eq!(resolution(&events, id(6)).0, Verdict::Normal);
    }
}

#[test]
fn the_ice_digger_replays_the_captures_swing() {
    // `r` is the roll's low six bits: 17827 & $3F = 35 and 58833 & $3F = 17,
    // both normal against the Digger's dexterity 70 and the Leach's agility 56
    // (normal ⟺ r ≤ 44, critical ⟺ r ≥ 45, no miss).
    let (roster, events) = swing_at(2, 960, &ICEDIGGER_HITS, &ICEDIGGER_DRAWS);
    assert_eq!(
        ICEDIGGER_DRAWS.iter().map(|r| r & 7).sum::<u16>(),
        51,
        "the log's own S"
    );
    // ((51+8)*250)>>6 + 250 = 480, *2 >> 2 = 240, -28 = 212 — the log's 212
    // from the Digger's own 250 attack byte. The Land Rover's 200 on the same
    // draws would land 164, which is what makes the byte the Digger's own.
    assert_eq!(resolution(&events, id(6)), (Verdict::Normal, Some(212)));
    assert_eq!(hp(&roster, id(6)), 828, "1040 - 212, the log's own HP");
}

#[test]
fn the_ice_diggers_last_pass_is_its_second() {
    // `loc_B6A2` blanks all nine `Fighters_Hit_Flags` before every pass
    // (ps4.asm:17493-17496), so the swing's own last pass is what
    // `Fighter_TakeDamage` reads. With two passes that is the second: r = 63
    // (a critical) on the first is discarded, and a critical on the second
    // adds the bonus `atk >> 2` = 62 — ((51+8)*250)>>6 + 250 + 2*62 = 604,
    // *2 >> 2 = 302, -28 = 274.
    let (_, discarded) = swing_at(2, 960, &[63, 58833], &ICEDIGGER_DRAWS);
    let (_, earned) = swing_at(2, 960, &[17827, 63], &ICEDIGGER_DRAWS);
    assert_eq!(resolution(&discarded, id(6)), (Verdict::Normal, Some(212)));
    assert_eq!(resolution(&earned, id(6)), (Verdict::Critical, Some(274)));
}

#[test]
fn the_uniform_three_pass_rule_diverges_on_the_ice_digger() {
    // The negative control for the per-vehicle count. `hit_passes(2)` is 2, and
    // the rule this replaced — three passes for every vehicle — reads the
    // Digger's own stream one roll late. Reproduced here rather than toggled in
    // the rule, so a regression is a unit failure and not only a replay
    // failure; these are the numbers the deleted
    // `replay_fixtures/divergences.json` entry for `forced_53_icedigger`
    // carried.
    let (mut roster, actor) = roster(2, 960);
    let mut stream: Vec<u16> = ICEDIGGER_HITS
        .iter()
        .chain(&ICEDIGGER_DRAWS)
        .copied()
        .collect();
    stream.push(NEXT_ROLL);

    // The rule as it stands: two passes, then the damage run — 18 rolls, the
    // eighteen the capture's round 1 holds for this swing, ending on 212.
    assert_eq!(hit_passes(2), 2);
    let mut rolls = SliceRolls::new(&stream);
    let mut events = Vec::new();
    resolve_attack(&mut roster, actor, None, &data(), &mut rolls, &mut events).expect("resolves");
    assert_eq!(rolls.drawn(), 18, "two passes and sixteen draws");
    assert_eq!(resolution(&events, id(6)), (Verdict::Normal, Some(212)));

    // Three passes, the pre-change rule: the third pass eats the first damage
    // draw, so the sixteen draws start one roll late and pick up round 2's
    // first — `& 7` sum 52 instead of 51, and 214 instead of 212.
    let targets = candidate_targets(&roster, actor, None, Reach::Single, &mut rolls);
    let mut rolls = SliceRolls::new(&stream);
    let mut pass = roll_hits(&roster, actor, &targets, false, &mut rolls);
    for _ in 1..3 {
        pass = roll_hits(&roster, actor, &targets, false, &mut rolls);
    }
    assert_eq!(pass.verdicts, vec![(id(6), Verdict::Normal)]);
    assert_eq!(rolls.drawn(), 3, "three passes, one target");
    let shifted: Vec<u16> = (0..DAMAGE_DRAWS).map(|_| rolls.next_roll()).collect();
    assert_eq!(
        rolls.drawn(),
        3 + DAMAGE_DRAWS,
        "19, where the log holds 18"
    );
    assert_eq!(shifted.iter().map(|r| r & 7).sum::<u16>(), 52);
    let raw = calculate_damage(
        250,
        28,
        u16::from(VEHICLE_ATTACK_ELEMENT),
        0,
        &mut SliceRolls::new(&shifted),
    );
    assert_eq!(clamp_damage(raw), 214, "the deleted entry's damage");
}
