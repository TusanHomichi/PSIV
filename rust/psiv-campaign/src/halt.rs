//! Why a run stopped.
//!
//! The runner never papers over a failure: a missed expectation, an exhausted
//! frame budget, an ability the engine cannot run, a scene fault, a lost
//! battle, a target it cannot walk to, or a menu entry it cannot find each end
//! the run as a [`Halt`], and the runner writes a report around it
//! (`runner.rs`).

use std::fmt;

use serde::Serialize;

/// The class of a halt, as the report names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HaltKind {
    /// An `expect` or a chapter's `closing` did not hold.
    ExpectFailed,
    /// The objective used its whole frame budget.
    BudgetExhausted,
    /// The battle engine emitted `BattleEvent::UnsupportedAbility` (or its
    /// vehicle-skill twin): a port gap, not a route problem.
    UnsupportedAbility,
    /// A scene faulted, a scene or its dialogue is missing from the pack, a
    /// warp or map refresh failed, or a battle round was refused.
    SceneFault,
    /// The party was defeated.
    LostBattle,
    /// The planner has no walk to the target.
    Unreachable,
    /// A menu has no entry the objective needs (an item, a member, a slot).
    MenuEntryMissing,
    /// The objective's target is not what the route names: an object that does
    /// not exist, or a talk that reached a different object.
    WrongObject,
    /// The game is in a state the objective cannot continue from: a window the
    /// route did not ask for, a refused purchase, a pending field notice.
    UnexpectedState,
    /// The party cannot make progress (blocked on every attempt).
    Stuck,
    /// The save could not be written.
    SaveFailed,
}

/// A stop, with what the runner saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halt {
    /// The class.
    pub kind: HaltKind,
    /// What happened, specifically.
    pub detail: String,
}

impl Halt {
    /// A halt of `kind`.
    pub fn new(kind: HaltKind, detail: impl Into<String>) -> Halt {
        Halt {
            kind,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Halt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for Halt {}

/// What a controller returns: done, or a halt.
pub type Res<T = ()> = Result<T, Halt>;
