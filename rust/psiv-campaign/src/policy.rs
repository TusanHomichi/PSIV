//! Battle policies: what each party member does when their command window
//! opens, and when the party runs.
//!
//! A policy decides; it never presses a button. It reads the command window's
//! view and the runtime's read-only battle data, builds a
//! [`Board`](crate::policy_board::Board), and returns an [`Intent`];
//! `battle.rs` walks the menu cursor to it with ordinary pad presses. Policies
//! are looked up by the name a route chapter gives in `random_battle_policy`
//! ([`by_name`]).
//!
//! # One player, three temperaments
//!
//! Every name resolves to one [`PartyPolicy`], whose command choices are the
//! rules of [`crate::policy_plan`]: every technique, skill and item the member
//! can use, chosen by effect class, element factors and the engine's own
//! damage formula, with a scripted battle fought with everything and a random
//! encounter fought cheaply. The names differ only in what the party does on
//! the main options:
//!
//! * `default`, `attack_all`, `heal_then_attack` and `fight_to_win` fight every
//!   battle;
//! * `run_unless_boss`, `run_then_win` and `bioplant_survival` RUN from every
//!   random encounter (every round until it works) and fight scripted battles;
//! * `train_with_inn` fights, and RUNs from a random encounter once a member
//!   has fallen or a living one is below [`RETREAT_PERCENT`] of their HP: the
//!   inn or the house that follows the patrol cures what a retreat leaves, and
//!   a wiped party cures nothing.
//!
//! `psycho_wand_then_win` is `run_then_win` with an item used in the first
//! player-command round of a scripted battle ([`crate::policy_opening`]).
//!
//! The running names exist because the cartridge's random draws are one
//! stream: any frame the runtime spends or saves anywhere moves every later
//! encounter, so a route that survives only on one stream is not a route.
//!
//! After a battle ends and the party stands at rest, the runner also cures the
//! hurt through the camp (`recovery.rs`): [`Policy::recovers`].

use psiv_core::battle::status;
use psiv_runtime::{CommandMenuView, PartyStatus, Runtime};

use crate::policy_board::Board;
use crate::policy_opening::OpeningItemPolicy;
use crate::policy_plan::Planner;

/// A living member below this share of their maximum HP makes `train_with_inn`
/// leave a random encounter.
pub const RETREAT_PERCENT: u32 = 33;

/// What one actor chooses to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// ATTACK; `target` is the enemy fighter id for a single-target weapon,
    /// `None` for the first one the list offers.
    Attack {
        /// The enemy to swing at.
        target: Option<u8>,
    },
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

impl Intent {
    /// A plain attack on the first enemy the list offers.
    pub const ATTACK: Intent = Intent::Attack { target: None };
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

    /// [`Policy::wants_run`], asked on the main options with the party strip in
    /// view, for a policy whose answer depends on how the party stands.
    fn wants_run_with(&self, boss: bool, _party: &[PartyStatus]) -> bool {
        self.wants_run(boss)
    }

    /// Whether the party is cured through the camp after each battle
    /// (`recovery.rs`).
    fn recovers(&self) -> bool {
        true
    }

    /// The actor's choice could not be carried out (a row that is disabled):
    /// drop to a plain attack for this window.
    fn refuse(&mut self);
}

/// Policy names the route files use, with the temperament each resolves to.
pub const NAMES: [(&str, &str); 9] = [
    ("default", "fight"),
    ("attack_all", "fight"),
    ("heal_then_attack", "fight"),
    ("fight_to_win", "fight"),
    ("run_unless_boss", "run_encounters"),
    ("run_then_win", "run_encounters"),
    ("bioplant_survival", "run_encounters"),
    ("train_with_inn", "train"),
    ("psycho_wand_then_win", "psycho_wand_then_win"),
];

/// The policy a route names, or `None` for a name nobody defines.
#[must_use]
pub fn by_name(name: &str) -> Option<Box<dyn Policy>> {
    let (route_name, temperament) = NAMES.iter().find(|(route_name, _)| *route_name == name)?;
    Some(match *temperament {
        "psycho_wand_then_win" => Box::new(OpeningItemPolicy::new(
            "psycho_wand_then_win",
            crate::policy_opening::PSYCHO_WAND,
        )),
        "run_encounters" => Box::new(PartyPolicy::running(route_name)),
        "train" => Box::new(PartyPolicy::training(route_name)),
        _ => Box::new(PartyPolicy::fighting(route_name)),
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

fn alive(member: &PartyStatus) -> bool {
    member.hp > 0 && member.status & status::OUT == 0
}

/// The party policy: [`crate::policy_plan`]'s choices, with a temperament on
/// the main options.
#[derive(Debug, Clone)]
pub struct PartyPolicy {
    name: &'static str,
    planner: Planner,
    current: Option<(u8, Intent)>,
    /// Run from random encounters instead of fighting them.
    run_encounters: bool,
    /// Run from a random encounter once a member has fallen or a living one is
    /// below [`RETREAT_PERCENT`].
    retreat: bool,
}

impl Default for PartyPolicy {
    fn default() -> PartyPolicy {
        PartyPolicy::fighting("default")
    }
}

impl PartyPolicy {
    /// Fights every battle.
    #[must_use]
    pub fn fighting(name: &'static str) -> PartyPolicy {
        PartyPolicy {
            name,
            planner: Planner::default(),
            current: None,
            run_encounters: false,
            retreat: false,
        }
    }

    /// Runs from random encounters and fights scripted battles.
    #[must_use]
    pub fn running(name: &'static str) -> PartyPolicy {
        PartyPolicy {
            run_encounters: true,
            ..PartyPolicy::fighting(name)
        }
    }

    /// Fights, and leaves a random encounter that has hurt the party.
    #[must_use]
    pub fn training(name: &'static str) -> PartyPolicy {
        PartyPolicy {
            retreat: true,
            ..PartyPolicy::fighting(name)
        }
    }
}

impl Policy for PartyPolicy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn battle_begins(&mut self, boss: bool) {
        self.planner.begin_battle(boss);
        self.current = None;
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
        let intent =
            Board::read(menu, runtime).map_or(Intent::ATTACK, |board| self.planner.decide(&board));
        trace_choice(menu, actor, &intent);
        self.current = Some((actor, intent.clone()));
        intent
    }

    fn wants_run(&self, boss: bool) -> bool {
        self.run_encounters && !boss
    }

    fn wants_run_with(&self, boss: bool, party: &[PartyStatus]) -> bool {
        self.wants_run(boss)
            || (self.retreat
                && !boss
                && party.iter().any(|m| {
                    !alive(m) || u32::from(m.hp) * 100 < u32::from(m.max_hp) * RETREAT_PERCENT
                }))
    }

    fn end_round(&mut self) {
        self.current = None;
        self.planner.end_round();
    }

    fn refuse(&mut self) {
        if let Some((actor, _)) = self.current {
            self.current = Some((actor, Intent::ATTACK));
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

    /// `train_with_inn` fights a healthy party, and leaves once a member has
    /// fallen or a living one is under a third; a boss is never left, and
    /// `bioplant_survival` runs from every random encounter.
    #[test]
    fn the_survival_policies_retreat_when_the_party_cannot_win() {
        let healthy = [member(1, 100, 100), member(2, 40, 100)];
        let low = [member(1, 100, 100), member(2, 32, 100)];
        let mut dead = member(2, 0, 100);
        dead.status = status::DEAD;
        let fallen = [member(1, 100, 100), dead];
        let training = by_name("train_with_inn").unwrap();
        assert!(!training.wants_run_with(false, &healthy));
        assert!(training.wants_run_with(false, &low));
        assert!(training.wants_run_with(false, &fallen));
        assert!(!training.wants_run_with(true, &fallen), "a boss is fought");
        let survival = by_name("bioplant_survival").unwrap();
        assert!(survival.wants_run_with(false, &healthy));
        assert!(!survival.wants_run_with(true, &healthy));
        // The plain policy never runs, whatever the strip shows.
        assert!(!by_name("default").unwrap().wants_run_with(false, &fallen));
    }

    #[test]
    fn route_policy_names_resolve() {
        for (name, _) in NAMES {
            let policy = by_name(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(policy.name(), name);
            assert!(is_known(name));
        }
        assert!(by_name("berserk").is_none());
    }
}
