//! Pure mapping from engine events to battle-screen narration and visual beats.

use std::collections::BTreeMap;

use psiv_core::battle::{BattleEvent, FighterId, Outcome, Priority, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Beat {
    None,
    Start,
    Attack(FighterId),
    Defense(FighterId),
    Damage {
        target: FighterId,
        amount: Option<u16>,
        critical: bool,
    },
    Hide(FighterId),
    Reward,
    LevelUp,
    End(Outcome),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Narration {
    pub(crate) line: String,
    pub(crate) beat: Beat,
}

pub(crate) const fn waits_for_confirm(beat: Beat) -> bool {
    matches!(
        beat,
        Beat::Reward | Beat::LevelUp | Beat::End(Outcome::Victory)
    )
}

fn fighter_name(id: FighterId, names: &BTreeMap<u8, String>) -> String {
    names
        .get(&id.get())
        .cloned()
        .unwrap_or_else(|| format!("Fighter {}", id.get()))
}

fn first_party_name(names: &BTreeMap<u8, String>) -> String {
    names
        .iter()
        .find(|(id, _)| **id < 6)
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| "Someone".into())
}

/// Every current [`BattleEvent`] variant gets one line and one beat. The
/// wildcard is required by the core's `non_exhaustive` promise and is kept as
/// a loud renderer error by the caller.
pub(crate) fn narration(
    event: &BattleEvent,
    names: &BTreeMap<u8, String>,
    character_names: &BTreeMap<u8, String>,
) -> Narration {
    match event {
        BattleEvent::Started {
            priority: Priority::Ambush,
            ..
        } => Narration {
            line: "Surprise Attack!".into(),
            beat: Beat::None,
        },
        BattleEvent::Started { .. } => Narration {
            line: String::new(),
            beat: Beat::Start,
        },
        BattleEvent::RoundBegan { .. } => Narration {
            line: String::new(),
            beat: Beat::None,
        },
        BattleEvent::Escaped => Narration {
            line: format!("{} retreated!", first_party_name(names)),
            beat: Beat::End(Outcome::Escaped),
        },
        BattleEvent::EscapeFailed => Narration {
            line: "Cannot escape!".into(),
            beat: Beat::None,
        },
        BattleEvent::TurnSkipped { .. } => Narration {
            line: String::new(),
            beat: Beat::None,
        },
        BattleEvent::Attacked { actor, .. } => Narration {
            line: "ATTACK".into(),
            beat: Beat::Attack(*actor),
        },
        BattleEvent::Defended { actor } => Narration {
            line: "DEFENSE".into(),
            beat: Beat::Defense(*actor),
        },
        BattleEvent::VehicleSkillUsed { .. } => Narration {
            line: String::new(),
            beat: Beat::None,
        },
        BattleEvent::VehicleSkillRejected { .. }
        | BattleEvent::VehicleSkillEffectUnavailable { .. } => Narration {
            line: String::new(),
            beat: Beat::None,
        },
        BattleEvent::VehicleSkillEffect { .. } => Narration {
            line: String::new(),
            beat: Beat::None,
        },
        BattleEvent::Resolved {
            actor: _,
            target,
            verdict,
            damage,
            ..
        } => Narration {
            // Retail draws the 5x2 damage tile block here. It does not print
            // prose such as "42 damage!" or "missed!".
            line: String::new(),
            beat: Beat::Damage {
                target: *target,
                amount: *damage,
                critical: *verdict == Verdict::Critical,
            },
        },
        BattleEvent::UnsupportedAbility { actor, .. } => Narration {
            line: "ATTACK".into(),
            beat: Beat::Attack(*actor),
        },
        BattleEvent::Died { fighter } => Narration {
            line: format!("{} defeated...!", fighter_name(*fighter, names)),
            beat: Beat::Hide(*fighter),
        },
        BattleEvent::RoundEnded { .. } => Narration {
            line: String::new(),
            beat: Beat::None,
        },
        BattleEvent::Rewarded { .. } => Narration {
            line: "Each got".into(),
            beat: Beat::Reward,
        },
        BattleEvent::LevelUp { character, .. } => {
            let name = character_names
                .get(character)
                .cloned()
                .unwrap_or_else(|| format!("Character {character}"));
            Narration {
                line: format!("{name} LV increased!"),
                beat: Beat::LevelUp,
            }
        }
        BattleEvent::Ended { outcome } => Narration {
            line: match outcome {
                Outcome::Victory | Outcome::Defeat | Outcome::Escaped => String::new(),
            },
            beat: Beat::End(*outcome),
        },
        _ => Narration {
            line: "Unhandled battle event.".into(),
            beat: Beat::None,
        },
    }
}

#[cfg(test)]
mod tests {
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
            Beat::Damage {
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
            Beat::Damage {
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
        assert_eq!(reward.beat, Beat::Reward);
    }

    #[test]
    fn narration_maps_end_and_death_beats() {
        let names = BTreeMap::from([(6, "MonsterFly".to_owned())]);
        let death = narration(
            &BattleEvent::Died { fighter: id(6) },
            &names,
            &BTreeMap::new(),
        );
        assert_eq!(death.beat, Beat::Hide(id(6)));
        let end = narration(
            &BattleEvent::Ended {
                outcome: Outcome::Victory,
            },
            &names,
            &BTreeMap::new(),
        );
        assert!(end.line.is_empty());
        assert_eq!(end.beat, Beat::End(Outcome::Victory));
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
        assert_eq!(defense.beat, Beat::Defense(id(1)));

        let failed = narration(&BattleEvent::EscapeFailed, &names, &BTreeMap::new());
        assert_eq!(failed.line, "Cannot escape!");
        assert_eq!(failed.beat, Beat::None);

        let escaped = narration(&BattleEvent::Escaped, &names, &BTreeMap::new());
        assert_eq!(escaped.line, "Chaz retreated!");
        assert_eq!(escaped.beat, Beat::End(Outcome::Escaped));

        let surprise = narration(
            &BattleEvent::Started {
                priority: Priority::Ambush,
                enemies: vec![id(6)],
            },
            &names,
            &BTreeMap::new(),
        );
        assert_eq!(surprise.line, "Surprise Attack!");
        assert_eq!(surprise.beat, Beat::None);
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
}
