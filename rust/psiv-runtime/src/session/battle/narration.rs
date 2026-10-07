//! Retail's battle narration: one line and one beat per engine event.
//!
//! The mapping is deliberately exhaustive over [`BattleEvent`]: the core's
//! event enum is not `non_exhaustive`, so a new event fails to compile here
//! until it has a line and a beat. Lines are the cartridge's own transient
//! strings (`ATTACK`, `DEFENSE`, `Each got`, `Victory!`) or the composed
//! sentences the port has always shown; the damage block prints no prose at
//! all, because retail draws five-by-two tile blocks there instead.
//!
//! The module moved here from `psiv-godot/src/battle/timeline.rs` with the
//! battle mode: the runtime raises the beats and the shell only draws them.

use std::collections::BTreeMap;

use psiv_core::battle::{BattleEvent, FighterId, FirstZioAction, Outcome, Priority, Verdict};

use super::view::BattleBeat;

/// The narration one event produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Narration {
    pub(crate) line: String,
    pub(crate) beat: BattleBeat,
}

/// Whether a beat waits for a confirm press instead of timing out.
///
/// `Battle_VictoryMessage` (`ps4.asm:4706`), `Battle_LastMessage`
/// (`ps4.asm:6351`) and the results pages they lead to all advance on
/// `ButtonCancel_Mask|ButtonSpeak_Mask|ButtonCamp_Mask` — retail's own "any
/// face button continues" rule for the post-battle pages.
pub(crate) const fn waits_for_confirm(beat: BattleBeat) -> bool {
    matches!(
        beat,
        BattleBeat::Reward | BattleBeat::LevelUp | BattleBeat::End(Outcome::Victory)
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

/// Every current [`BattleEvent`] variant gets one line and one beat.
pub(crate) fn narration(
    event: &BattleEvent,
    names: &BTreeMap<u8, String>,
    character_names: &BTreeMap<u8, String>,
) -> Narration {
    match event {
        BattleEvent::TpRestored { target, amount, .. } => Narration {
            line: format!("{} restored {amount} TP", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::ResistanceChanged { target, .. } => Narration {
            line: format!("{} resistance changed", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::CharacterSleepCleared => Narration {
            line: String::new(),
            beat: BattleBeat::None,
        },
        BattleEvent::EnemyStatsReloaded { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::None,
        },
        BattleEvent::FirstZioAction { action, .. } => Narration {
            line: match action {
                FirstZioAction::MagicBarrier => "MAG.BARRIR",
                FirstZioAction::Nightmare => "NIGHTMARE",
                FirstZioAction::BlackWave => "BLACK WAVE",
                FirstZioAction::Invocation
                | FirstZioAction::Pause
                | FirstZioAction::DarkForceCharge => "",
            }
            .into(),
            beat: BattleBeat::None,
        },
        BattleEvent::StatusInflicted { target, status, .. } => Narration {
            line: format!(
                "{} {}!",
                fighter_name(*target, names),
                match *status {
                    psiv_core::battle::status::POISONED => "poisoned",
                    psiv_core::battle::status::ASLEEP => "asleep",
                    psiv_core::battle::status::TECH_SEALED => "sealed",
                    _ => "paralyzed",
                }
            ),
            beat: BattleBeat::None,
        },
        BattleEvent::EnemySkillUsed { name, .. } => Narration {
            line: name.clone(),
            beat: BattleBeat::None,
        },
        BattleEvent::EnemyReplenished { name, .. } => Narration {
            line: format!("{name} appears!"),
            beat: BattleBeat::None,
        },
        BattleEvent::EnemiesFused { name, .. } => Narration {
            line: format!("{name} appears!"),
            beat: BattleBeat::None,
        },
        BattleEvent::ItemUsed { actor, name, .. } => Narration {
            line: format!("{}: {name}", fighter_name(*actor, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::ItemRejected { reason, .. } => Narration {
            line: match reason {
                psiv_core::battle::ItemRejection::Missing => "Item is gone!",
                psiv_core::battle::ItemRejection::InvalidTarget => "Invalid target!",
                psiv_core::battle::ItemRejection::Unavailable => "Cannot use item!",
            }
            .into(),
            beat: BattleBeat::None,
        },
        BattleEvent::ItemIneffective { .. } | BattleEvent::TechniqueIneffective { .. } => {
            Narration {
                line: "No effect!".into(),
                beat: BattleBeat::None,
            }
        }
        BattleEvent::Revived { target, .. } => Narration {
            line: format!("{} revived!", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::StatusRestored { target, .. } => Narration {
            line: format!("{} cured!", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::StatsRestored { target, .. } => Narration {
            line: format!("{} stats restored", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::SkillUsed { actor, name, .. } => Narration {
            line: format!("{}: {name}", fighter_name(*actor, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::SkillRejected { reason, .. } => Narration {
            line: match reason {
                psiv_core::battle::SkillRejection::Exhausted => "No uses left!",
                psiv_core::battle::SkillRejection::Unarmed => "No weapon!",
                _ => "Cannot use skill!",
            }
            .into(),
            beat: BattleBeat::None,
        },
        BattleEvent::SkillIneffective { .. } => Narration {
            line: "No effect!".into(),
            beat: BattleBeat::None,
        },
        BattleEvent::FellAsleep { target, .. } => Narration {
            line: format!("{} asleep!", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::WokeUp { fighter } => Narration {
            line: format!("{} awake!", fighter_name(*fighter, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::ParalysisCleared { fighter } => Narration {
            line: format!("{} moves!", fighter_name(*fighter, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::TechniqueUsed { actor, name, .. } => Narration {
            line: format!("{}: {name}", fighter_name(*actor, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::TechniqueRejected { reason, .. } => Narration {
            line: match reason {
                psiv_core::battle::TechniqueRejection::Sealed => "Tech sealed!",
                psiv_core::battle::TechniqueRejection::InsufficientTp => "Not enough TP!",
                _ => "Cannot use tech!",
            }
            .into(),
            beat: BattleBeat::None,
        },
        BattleEvent::Healed { target, amount, .. } => Narration {
            line: format!("{} healed {amount}", fighter_name(*target, names)),
            beat: BattleBeat::None,
        },
        BattleEvent::StatChanged {
            target,
            stat,
            value,
            ..
        } => Narration {
            line: format!(
                "{} {} {value}",
                fighter_name(*target, names),
                match stat {
                    psiv_core::battle::TechniqueStat::Attack => "ATK",
                    psiv_core::battle::TechniqueStat::Defence => "DEF",
                    psiv_core::battle::TechniqueStat::MentalDefence => "MDF",
                    psiv_core::battle::TechniqueStat::Agility => "AGI",
                    psiv_core::battle::TechniqueStat::Dexterity => "DEX",
                }
            ),
            beat: BattleBeat::None,
        },
        BattleEvent::Started {
            priority: Priority::Ambush,
            ..
        } => Narration {
            line: "Surprise Attack!".into(),
            beat: BattleBeat::None,
        },
        BattleEvent::Started { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::Start,
        },
        BattleEvent::RoundBegan { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::None,
        },
        BattleEvent::Escaped => Narration {
            line: format!("{} retreated!", first_party_name(names)),
            beat: BattleBeat::End(Outcome::Escaped),
        },
        BattleEvent::EscapeFailed => Narration {
            line: "Cannot escape!".into(),
            beat: BattleBeat::None,
        },
        BattleEvent::TurnSkipped { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::None,
        },
        BattleEvent::Attacked { actor, .. } => Narration {
            line: "ATTACK".into(),
            beat: BattleBeat::Attack(*actor),
        },
        BattleEvent::Defended { actor } => Narration {
            line: "DEFENSE".into(),
            beat: BattleBeat::Defense(*actor),
        },
        BattleEvent::VehicleSkillUsed { .. }
        | BattleEvent::VehicleSkillRejected { .. }
        | BattleEvent::VehicleSkillEffectUnavailable { .. }
        | BattleEvent::VehicleSkillEffect { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::None,
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
            beat: BattleBeat::Damage {
                target: *target,
                amount: *damage,
                critical: *verdict == Verdict::Critical,
            },
        },
        BattleEvent::UnsupportedAbility { actor, .. } => Narration {
            line: "ATTACK".into(),
            beat: BattleBeat::Attack(*actor),
        },
        BattleEvent::Died { fighter } => Narration {
            line: format!("{} defeated...!", fighter_name(*fighter, names)),
            beat: BattleBeat::Hide(*fighter),
        },
        BattleEvent::RoundEnded { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::None,
        },
        BattleEvent::Rewarded { .. } => Narration {
            line: "Each got".into(),
            beat: BattleBeat::Reward,
        },
        BattleEvent::LearnedAbility { character, name } => Narration {
            line: format!(
                "{} learned {name}!",
                character_names
                    .get(character)
                    .cloned()
                    .unwrap_or_else(|| format!("Character {character}"))
            ),
            beat: BattleBeat::LevelUp,
        },
        BattleEvent::LevelUp { character, .. } => Narration {
            line: format!(
                "{} LV increased!",
                character_names
                    .get(character)
                    .cloned()
                    .unwrap_or_else(|| format!("Character {character}"))
            ),
            beat: BattleBeat::LevelUp,
        },
        BattleEvent::Ended { outcome } => Narration {
            line: match outcome {
                Outcome::Victory | Outcome::Defeat | Outcome::Escaped | Outcome::ScriptedExit => {
                    String::new()
                }
            },
            beat: BattleBeat::End(*outcome),
        },
        // `EnemyAttack_FloatMine`'s fall-through (`loc_10406`,
        // `ps4.asm:22781`) spends the actor's turn with nothing to show: no
        // message window, no `Sound_Index` write and no battle object. The
        // 16-frame `$FFFF418A` pause `loc_6672` sets for the empty ability slot
        // is not modelled; the turn simply passes.
        BattleEvent::EnemyAbilityWasted { .. } => Narration {
            line: String::new(),
            beat: BattleBeat::None,
        },
    }
}

#[cfg(test)]
#[path = "narration_tests.rs"]
mod tests;
