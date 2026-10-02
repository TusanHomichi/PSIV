//! Battle policies: what each party member does when their command window
//! opens.
//!
//! A policy decides; it never presses a button. It reads the command window's
//! view and the runtime's read-only battle data and returns an [`Intent`], and
//! `battle.rs` walks the menu cursor to it with ordinary pad presses. Policies
//! are looked up by the name a route chapter gives in `random_battle_policy`
//! ([`by_name`]), so a new behaviour is a new type and one line in [`NAMES`],
//! with no change to the menu driver.
//!
//! # The `default` policy
//!
//! Everyone attacks the first living enemy the target list offers. When a
//! party member is below [`HURT_PERCENT`] of their maximum HP, the first actor
//! of the round who can cast a healing technique (battle effect 18, "heal HP")
//! with the TP for it casts the cheapest one on the most hurt member; when
//! nobody can, the first actor who can use a healing item from the pack does.
//! One heal a round: the rest attack. Losing is not a policy question: a
//! defeated party halts the run.
//!
//! After a battle ends and the party stands at rest, the runner also cures the
//! hurt through the camp (`recovery.rs`), which is the same policy's business:
//! [`Policy::recovers`].
//!
//! # `run_unless_boss`
//!
//! The same, except that a random encounter is run from (RUN on the main
//! options, every round until it works) and a scripted battle (a boss, an event
//! battle) is fought. The Alshline chapters name it: their party is far below
//! the basement's weight.
//!
//! # Route names
//!
//! The route files name nine names. `run_unless_boss` is its own behaviour;
//! `fight_to_win` and `run_then_win` are the boss policy of
//! [`crate::policy_boss`] (the second runs from random encounters);
//! `psycho_wand_then_win` is `run_then_win` with an item used in the first
//! round of a scripted battle ([`crate::policy_opening`]);
//! `attack_all`, `heal_then_attack`, `train_with_inn` and `bioplant_survival`
//! resolve to `default` (the walk, the patrol and the inn they were named for
//! are route objectives, not battle decisions). A route that needs another
//! behaviour gets a type here first.

use crate::policy_boss::BossPolicy;
use crate::policy_opening::OpeningItemPolicy;
use psiv_core::battle::status;
use psiv_runtime::{CommandMenuView, PartyStatus, Runtime};

/// A member below this share of their maximum HP is hurt enough to heal.
pub const HURT_PERCENT: u32 = 50;

/// What one actor chooses to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// ATTACK the first living enemy.
    Attack,
    /// TECH: cast technique `id`, on `target` (a fighter id) when it takes one.
    Technique {
        /// One-based technique id.
        id: u8,
        /// The fighter to cast it on, for a single-target technique.
        target: Option<u8>,
    },
    /// ITEM: use the item the window lists under `name`, on `target` when it
    /// takes one.
    Item {
        /// The item's display name, as the item page's row shows it.
        name: String,
        /// The fighter to use it on, for a single-target item.
        target: Option<u8>,
    },
    /// SKILL: use skill `id`, on `target` (a fighter id) when it takes one.
    Skill {
        /// One-based skill id.
        id: u8,
        /// The fighter to use it on, for a single-target skill.
        target: Option<u8>,
    },
    /// DEFEND.
    Defend,
}

/// A battle policy.
pub trait Policy {
    /// The name a route gives it.
    fn name(&self) -> &'static str;

    /// A battle begins; `boss` is true for a scripted (event) battle. Called
    /// once, before the first window opens.
    fn battle_begins(&mut self, _boss: bool) {}

    /// The actor's choice. Called while their command window is open; the same
    /// answer is returned until [`Policy::end_round`].
    fn choose(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Intent;

    /// The command window closed: the next one starts a new round.
    fn end_round(&mut self);

    /// Whether to RUN instead of fighting this battle. `boss` is true for a
    /// scripted (event) battle.
    fn wants_run(&self, _boss: bool) -> bool {
        false
    }

    /// Whether the party is cured through the camp after each battle
    /// (`recovery.rs`).
    fn recovers(&self) -> bool {
        true
    }

    /// The actor's choice could not be carried out (an item row that is
    /// disabled): drop to a plain attack for this window.
    fn refuse(&mut self);
}

/// Policy names the route files use, with the policy each resolves to.
pub const NAMES: [(&str, &str); 9] = [
    ("default", "default"),
    ("attack_all", "default"),
    ("heal_then_attack", "default"),
    ("run_unless_boss", "run_unless_boss"),
    ("fight_to_win", "fight_to_win"),
    ("run_then_win", "run_then_win"),
    ("psycho_wand_then_win", "psycho_wand_then_win"),
    ("train_with_inn", "default"),
    ("bioplant_survival", "default"),
];

/// The policy a route names, or `None` for a name nobody defines.
#[must_use]
pub fn by_name(name: &str) -> Option<Box<dyn Policy>> {
    let (_, policy) = NAMES.iter().find(|(route_name, _)| *route_name == name)?;
    Some(match *policy {
        "fight_to_win" => Box::new(BossPolicy::default()),
        "run_then_win" => Box::new(BossPolicy::running()),
        "psycho_wand_then_win" => Box::new(OpeningItemPolicy::new(
            "psycho_wand_then_win",
            crate::policy_opening::PSYCHO_WAND,
        )),
        "run_unless_boss" => Box::new(DefaultPolicy {
            run_encounters: true,
            ..DefaultPolicy::default()
        }),
        _ => Box::new(DefaultPolicy::default()),
    })
}

/// Whether `name` is a policy a route may name.
#[must_use]
pub fn is_known(name: &str) -> bool {
    NAMES.iter().any(|(route_name, _)| *route_name == name)
}

/// Prints one actor's choice and the party's state when `PSIV_CAMPAIGN_TRACE`
/// is set.
pub(crate) fn trace_choice(menu: &CommandMenuView, actor: u8, intent: &Intent) {
    if std::env::var_os("PSIV_CAMPAIGN_TRACE").is_none() {
        return;
    }
    let party: Vec<String> = menu
        .party
        .iter()
        .map(|m| {
            format!(
                "{} {}/{} tp{} st{:#x}",
                m.name, m.hp, m.max_hp, m.tp, m.status
            )
        })
        .collect();
    eprintln!("  battle actor {actor} {intent:?} | {}", party.join(" | "));
}

/// Attack with everyone; heal the hurt.
#[derive(Debug, Default)]
pub struct DefaultPolicy {
    current: Option<(u8, Intent)>,
    healed_this_round: bool,
    /// Run from random encounters instead of fighting them.
    run_encounters: bool,
}

fn alive(member: &PartyStatus) -> bool {
    member.hp > 0 && member.status & status::DEAD == 0
}

fn hurt(member: &PartyStatus) -> bool {
    alive(member) && u32::from(member.hp) * 100 < u32::from(member.max_hp) * HURT_PERCENT
}

/// The most hurt member: the lowest share of their maximum HP.
pub(crate) fn most_hurt(party: &[PartyStatus]) -> Option<&PartyStatus> {
    party.iter().filter(|m| hurt(m)).min_by(|a, b| {
        (u32::from(a.hp) * u32::from(b.max_hp)).cmp(&(u32::from(b.hp) * u32::from(a.max_hp)))
    })
}

impl DefaultPolicy {
    fn decide(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Intent {
        self.heal(menu, runtime).unwrap_or(Intent::Attack)
    }

    /// The healing this actor does now, if the round still needs one.
    pub(crate) fn heal(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Option<Intent> {
        if self.healed_this_round {
            return None;
        }
        let patient = most_hurt(&menu.party)?;
        // A healing technique first: effect 18 is "heal HP".
        let healing = runtime
            .battle_techniques()
            .filter(|technique| technique.effect == 18)
            .map(|technique| technique.id)
            .collect::<Vec<_>>();
        let cure = menu
            .techniques
            .iter()
            .filter(|entry| entry.available && healing.contains(&entry.id))
            .min_by_key(|entry| entry.cost);
        if let Some(cure) = cure {
            let single = runtime
                .battle_techniques()
                .find(|technique| technique.id == cure.id)
                .is_some_and(psiv_core::battle::Technique::single_target);
            self.healed_this_round = true;
            return Some(Intent::Technique {
                id: cure.id,
                target: single.then_some(patient.fighter),
            });
        }
        // Then a healing item the pack holds.
        let held = runtime.game().inventory();
        let item = runtime
            .battle_items()
            .filter(|item| item.effect == 18 && item.supported() && item.consumable)
            .find(|item| held.contains(item.id));
        if let Some(item) = item {
            self.healed_this_round = true;
            return Some(Intent::Item {
                name: item.name.clone(),
                target: item.single_target().then_some(patient.fighter),
            });
        }
        None
    }
}

impl Policy for DefaultPolicy {
    fn name(&self) -> &'static str {
        if self.run_encounters {
            "run_unless_boss"
        } else {
            "default"
        }
    }

    fn choose(&mut self, menu: &CommandMenuView, runtime: &Runtime) -> Intent {
        let Some(actor) = menu.actor else {
            return Intent::Defend;
        };
        if let Some((who, intent)) = &self.current
            && *who == actor
        {
            return intent.clone();
        }
        let intent = self.decide(menu, runtime);
        trace_choice(menu, actor, &intent);
        self.current = Some((actor, intent.clone()));
        intent
    }

    fn wants_run(&self, boss: bool) -> bool {
        self.run_encounters && !boss
    }

    fn end_round(&mut self) {
        self.current = None;
        self.healed_this_round = false;
    }

    fn refuse(&mut self) {
        if let Some((actor, _)) = self.current {
            self.current = Some((actor, Intent::Attack));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(fighter: u8, hp: u16, max_hp: u16) -> PartyStatus {
        PartyStatus {
            fighter,
            name: format!("M{fighter}"),
            hp,
            max_hp,
            tp: 0,
            status: 0,
        }
    }

    #[test]
    fn the_most_hurt_member_is_the_lowest_share_not_the_lowest_hp() {
        let party = [member(1, 30, 100), member(2, 5, 50), member(3, 90, 100)];
        assert_eq!(most_hurt(&party).map(|m| m.fighter), Some(2));
        assert_eq!(most_hurt(&[member(1, 50, 100)]).map(|m| m.fighter), None);
        assert_eq!(most_hurt(&[member(1, 49, 100)]).map(|m| m.fighter), Some(1));
    }

    #[test]
    fn the_dead_are_not_patients() {
        let mut dead = member(1, 0, 100);
        dead.status = status::DEAD;
        assert_eq!(most_hurt(&[dead, member(2, 90, 100)]), None);
    }

    #[test]
    fn route_policy_names_resolve() {
        for (name, _) in NAMES {
            assert!(by_name(name).is_some(), "{name}");
            assert!(is_known(name));
        }
        assert!(by_name("berserk").is_none());
    }
}
