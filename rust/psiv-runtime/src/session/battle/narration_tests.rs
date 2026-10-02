//! The narration table's own tests, moved here from
//! `psiv-godot/src/battle/timeline.rs` with the strings they pin.

use super::*;

fn id(value: u8) -> FighterId {
    FighterId::new(value).expect("valid fighter id")
}

#[test]
fn narration_uses_retail_damage_and_reward_strings() {
    let names = BTreeMap::from([(1, "Chaz".to_owned()), (6, "MonsterFly".to_owned())]);
    let miss = narration(
        &BattleEvent::Resolved {
            actor: id(1),
            target: id(6),
            verdict: Verdict::Miss,
            damage: None,
            remaining_hp: 20,
        },
        &names,
        &BTreeMap::new(),
    );
    assert!(miss.line.is_empty());
    assert_eq!(
        miss.beat,
        BattleBeat::Damage {
            target: id(6),
            amount: None,
            critical: false
        }
    );

    let critical = narration(
        &BattleEvent::Resolved {
            actor: id(1),
            target: id(6),
            verdict: Verdict::Critical,
            damage: Some(42),
            remaining_hp: 8,
        },
        &names,
        &BTreeMap::new(),
    );
    assert!(critical.line.is_empty());
    assert_eq!(
        critical.beat,
        BattleBeat::Damage {
            target: id(6),
            amount: Some(42),
            critical: true
        }
    );

    let reward = narration(
        &BattleEvent::Rewarded {
            experience_total: 30,
            experience_each: 15,
            meseta: 12,
            recipients: vec![id(1)],
        },
        &names,
        &BTreeMap::new(),
    );
    assert_eq!(reward.line, "Each got");
    assert_eq!(reward.beat, BattleBeat::Reward);
}

#[test]
fn narration_maps_end_and_death_beats() {
    let names = BTreeMap::from([(6, "MonsterFly".to_owned())]);
    let death = narration(
        &BattleEvent::Died { fighter: id(6) },
        &names,
        &BTreeMap::new(),
    );
    assert_eq!(death.beat, BattleBeat::Hide(id(6)));
    let end = narration(
        &BattleEvent::Ended {
            outcome: Outcome::Victory,
        },
        &names,
        &BTreeMap::new(),
    );
    assert!(end.line.is_empty());
    assert_eq!(end.beat, BattleBeat::End(Outcome::Victory));
}

#[test]
fn narration_matches_retail_transient_strings() {
    let names = BTreeMap::from([(1, "Chaz".to_owned())]);
    let defense = narration(
        &BattleEvent::Defended { actor: id(1) },
        &names,
        &BTreeMap::new(),
    );
    assert_eq!(defense.line, "DEFENSE");
    assert_eq!(defense.beat, BattleBeat::Defense(id(1)));

    let failed = narration(&BattleEvent::EscapeFailed, &names, &BTreeMap::new());
    assert_eq!(failed.line, "Cannot escape!");
    assert_eq!(failed.beat, BattleBeat::None);

    let escaped = narration(&BattleEvent::Escaped, &names, &BTreeMap::new());
    assert_eq!(escaped.line, "Chaz retreated!");
    assert_eq!(escaped.beat, BattleBeat::End(Outcome::Escaped));

    let surprise = narration(
        &BattleEvent::Started {
            priority: Priority::Ambush,
            enemies: vec![id(6)],
        },
        &names,
        &BTreeMap::new(),
    );
    assert_eq!(surprise.line, "Surprise Attack!");
    assert_eq!(surprise.beat, BattleBeat::None);
}

#[test]
fn skipped_turns_are_silent_and_attack_is_a_command_label() {
    let names = BTreeMap::from([(1, "Chaz".to_owned())]);
    let event = narration(
        &BattleEvent::TurnSkipped {
            actor: id(1),
            reason: psiv_core::battle::Skipped::Unarmed,
        },
        &names,
        &BTreeMap::new(),
    );
    assert!(event.line.is_empty());
    let attack = narration(
        &BattleEvent::Attacked {
            actor: id(1),
            targets: vec![id(6)],
        },
        &names,
        &BTreeMap::new(),
    );
    assert_eq!(attack.line, "ATTACK");
}

/// The pages that wait for a press are the post-battle ones: retail's
/// `Battle_VictoryMessage` (`ps4.asm:4706`), its results pages and
/// `Battle_LastMessage` (`ps4.asm:6351`).
#[test]
fn only_the_post_battle_pages_wait_for_a_confirm() {
    for beat in [
        BattleBeat::Reward,
        BattleBeat::LevelUp,
        BattleBeat::End(Outcome::Victory),
    ] {
        assert!(waits_for_confirm(beat), "{beat:?} waits for a press");
    }
    for beat in [
        BattleBeat::None,
        BattleBeat::Start,
        BattleBeat::Attack(id(1)),
        BattleBeat::Defense(id(1)),
        BattleBeat::Damage {
            target: id(6),
            amount: Some(3),
            critical: false,
        },
        BattleBeat::Hide(id(6)),
        BattleBeat::End(Outcome::Defeat),
        BattleBeat::End(Outcome::Escaped),
        BattleBeat::End(Outcome::ScriptedExit),
    ] {
        assert!(!waits_for_confirm(beat), "{beat:?} times out instead");
    }
}
