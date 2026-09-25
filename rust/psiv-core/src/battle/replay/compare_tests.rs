//! The comparator's own checks, on hand-built rounds and timelines.
//!
//! The data-driven test (`super::data::every_fixture_replays_as_recorded`)
//! walks every fixture the repository has, but a fixture is a capture: it can
//! only show what the cartridge did, and a check whose input is a port
//! resolution no capture happens to contain would never be exercised. These
//! tests build that input by hand - a round whose ability action resolves a
//! slot without damaging it, and a timeline where the port does or does not
//! damage it - which is what pins the ability branch's unchecked-slot scan
//! (`super::divergence`'s "second half").
//!
//! The battle around them is not simulated: `divergence` reads the log's own
//! record and the port's timeline, so those two are the whole input.

use super::*;

/// One enemy ability action: enemy 6's POISON (`$11`) on FighterId(1), the
/// slot its pass resolved with no damage word behind it.
///
/// The shape is `formation_38`'s (f25019): the pass covers FighterId(1), the
/// poison's own roll follows, and `alys_status` never moves - the log says the
/// slot was reached and nothing came of it.
const POISON_ON_A_CLEAN_SLOT: &str = r#"{
  "round": 1,
  "order_frame": 10,
  "order": [6],
  "ordering": [20],
  "roll_count": 1,
  "actions": [{
    "actor": 6,
    "start_frame": 12,
    "end_frame": 20,
    "hit_frame": 12,
    "roll_count": 1,
    "kind": "ability",
    "ability": 17,
    "targets": [{
      "id": 1,
      "hit": "00",
      "damage": null,
      "damage_frame": null,
      "hp_after": 999,
      "died": false
    }],
    "living_opponents": [1, 2, 3]
  }]
}"#;

fn fighter(number: u8) -> FighterId {
    FighterId::new(number).expect("a seated fighter id")
}

fn round() -> Round {
    serde_json::from_str(POISON_ON_A_CLEAN_SLOT).expect("the round parses")
}

/// The port's turn: the ability it used, then whatever it did about the slot.
fn timeline(resolution: BattleEvent) -> Vec<BattleEvent> {
    vec![
        BattleEvent::RoundBegan {
            round: 1,
            order: vec![fighter(6)],
        },
        BattleEvent::EnemySkillUsed {
            actor: fighter(6),
            skill: 17,
            name: "POISON".to_owned(),
        },
        resolution,
        BattleEvent::RoundEnded { round: 1 },
    ]
}

fn resolved(verdict: Verdict, damage: Option<u16>) -> BattleEvent {
    BattleEvent::Resolved {
        actor: fighter(6),
        target: fighter(1),
        verdict,
        damage,
        remaining_hp: 999,
    }
}

#[test]
fn an_ability_that_damages_a_slot_the_log_left_clean_is_a_value_divergence() {
    let finding = divergence(&round(), &timeline(resolved(Verdict::Normal, Some(5))))
        .expect("the port damaged a slot the log shows no damage on");
    assert_eq!(finding.kind(), "value");
    assert_eq!(finding.frame(), 12);
    let Divergence::Value {
        actor,
        target,
        port,
        port_damage,
        log_hit,
        log_damage,
        ..
    } = finding
    else {
        panic!("the finding is the value comparison");
    };
    assert_eq!(actor, fighter(6));
    assert_eq!(target, fighter(1));
    assert_eq!(port, Verdict::Normal);
    assert_eq!(port_damage, Some(5));
    // The log's own byte for the slot: resolved, `$00`, no damage word.
    assert_eq!(log_hit, 0x00);
    assert_eq!(log_damage, None);
}

#[test]
fn an_ability_that_misses_a_slot_the_log_left_clean_is_not_a_divergence() {
    // The ability's pass covers its whole range, so a slot it reaches and
    // misses - or finds nothing to do with - is one the log has no damage for
    // and the port has nothing to report but the miss.
    assert_eq!(
        divergence(&round(), &timeline(resolved(Verdict::Miss, None))),
        None
    );
}

#[test]
fn an_ability_that_lands_a_critical_on_an_unread_slot_diverges() {
    // The verdict alone is enough: nothing in the log says a critical happened
    // there, and the damage the port dealt is not the point.
    let finding = divergence(&round(), &timeline(resolved(Verdict::Critical, None)))
        .expect("the port landed a critical the log does not show");
    assert_eq!(finding.kind(), "value");
}
