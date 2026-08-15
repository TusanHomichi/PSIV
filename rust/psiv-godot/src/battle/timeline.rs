//! Pure mapping from engine events to battle-screen narration and visual beats.

use std::collections::BTreeMap;

use psiv_core::battle::{BattleEvent, FighterId, Outcome, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Beat {
    None,
    Start,
    Attack(FighterId),
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

fn fighter_name(id: FighterId, names: &BTreeMap<u8, String>) -> String {
    names
        .get(&id.get())
        .cloned()
        .unwrap_or_else(|| format!("Fighter {}", id.get()))
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
        BattleEvent::Started { .. } => Narration {
            line: "Battle start!".into(),
            beat: Beat::Start,
        },
        BattleEvent::RoundBegan { round, .. } => Narration {
            line: format!("Round {round}"),
            beat: Beat::None,
        },
        BattleEvent::Escaped => Narration {
            line: "Got away!".into(),
            beat: Beat::End(Outcome::Escaped),
        },
        BattleEvent::EscapeFailed => Narration {
            line: "Can't escape!".into(),
            beat: Beat::None,
        },
        BattleEvent::TurnSkipped { actor, reason } => Narration {
            line: format!("{} {:?}.", fighter_name(*actor, names), reason),
            beat: Beat::None,
        },
        BattleEvent::Attacked { actor, .. } => Narration {
            line: format!("{} attacks!", fighter_name(*actor, names)),
            beat: Beat::Attack(*actor),
        },
        BattleEvent::Defended { actor } => Narration {
            line: format!("{} defends.", fighter_name(*actor, names)),
            beat: Beat::None,
        },
        BattleEvent::Resolved {
            actor,
            target,
            verdict,
            damage,
            ..
        } => {
            let line = match verdict {
                Verdict::Miss => format!("{} missed!", fighter_name(*actor, names)),
                Verdict::Critical => format!("Critical! {} damage!", damage.unwrap_or(0)),
                Verdict::Normal => format!("{} damage!", damage.unwrap_or(0)),
            };
            Narration {
                line,
                beat: Beat::Damage {
                    target: *target,
                    amount: *damage,
                    critical: *verdict == Verdict::Critical,
                },
            }
        }
        BattleEvent::UnsupportedAbility { actor, ability } => Narration {
            line: format!(
                "{} ability {} unsupported!",
                fighter_name(*actor, names),
                ability
            ),
            beat: Beat::Attack(*actor),
        },
        BattleEvent::Died { fighter } => Narration {
            line: format!("{} falls!", fighter_name(*fighter, names)),
            beat: Beat::Hide(*fighter),
        },
        BattleEvent::RoundEnded { .. } => Narration {
            line: "Ready.".into(),
            beat: Beat::None,
        },
        BattleEvent::Rewarded {
            experience_each,
            meseta,
            ..
        } => Narration {
            line: format!("Got {experience_each} EXP! Got {meseta} meseta!"),
            beat: Beat::Reward,
        },
        BattleEvent::LevelUp {
            character, level, ..
        } => {
            let name = character_names
                .get(character)
                .cloned()
                .unwrap_or_else(|| format!("Character {character}"));
            Narration {
                line: format!("{name} reached level {level}!"),
                beat: Beat::LevelUp,
            }
        }
        BattleEvent::Ended { outcome } => Narration {
            line: match outcome {
                Outcome::Victory => "Victory!".into(),
                Outcome::Defeat => "Defeat!".into(),
                Outcome::Escaped => "Escaped.".into(),
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
    fn narration_covers_damage_flavours_and_rewards() {
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
        assert_eq!(miss.line, "Chaz missed!");
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
        assert_eq!(critical.line, "Critical! 42 damage!");
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
        assert_eq!(reward.line, "Got 15 EXP! Got 12 meseta!");
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
        assert_eq!(end.line, "Victory!");
        assert_eq!(end.beat, Beat::End(Outcome::Victory));
    }

    #[test]
    fn skipped_turns_are_not_silent() {
        let names = BTreeMap::from([(1, "Chaz".to_owned())]);
        let event = narration(
            &BattleEvent::TurnSkipped {
                actor: id(1),
                reason: psiv_core::battle::Skipped::Unarmed,
            },
            &names,
            &BTreeMap::new(),
        );
        assert!(event.line.contains("Unarmed"));
    }
}
