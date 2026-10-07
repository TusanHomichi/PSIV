//! Campaign route planning and the route-file format.
//!
//! The campaign runner plays New Game to the Ending from a route file with
//! ordinary joypad input (`docs/campaign/CAMPAIGN_RUNNER.md`). This crate is the
//! part of it that needs no running game:
//!
//! * [`cell_plan`]: the pad directions that cross one map, built only from
//!   `psiv_core::FieldMap`'s own walkability, adjacency and warp rules;
//! * [`map_plan`]: the chain of warps between two positions over the pack's
//!   whole map graph;
//! * [`route`]: the serde types of a route file, with a documented example;
//! * [`validate`]: a checker that a route's ids exist and its walks are
//!   statically possible.
//!
//! The rest plays the game:
//!
//! * [`driver`]: one [`psiv_runtime::Session`], one pad per frame, the tape, the
//!   frame budget and the halt detector;
//! * [`field`], [`walk`], [`talk`], [`shopping`], [`ship`], [`camping`], [`battle`],
//!   [`expect`], [`menu`]: one controller per objective kind, each producing
//!   pads from the session's read-only views;
//! * [`policy`]: how battles are fought, by name;
//! * [`exec`], [`runner`]: objectives, chapters, saves and the halt report;
//! * [`tape`], [`digest`], [`replay`], [`split`], [`prefix`]: the pad tape, its
//!   replay, its cut at chapter boundaries and the check that a longer route
//!   left an earlier run's chapters alone.
//!
//! The planner plans; the runner moves the party, with pad presses only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::float_arithmetic)]

pub mod battle;
pub mod camping;
pub mod cell_plan;
pub mod digest;
pub mod driver;
pub mod exec;
pub mod expect;
pub mod field;
pub mod halt;
pub mod inspect;
pub mod map_plan;
pub mod menu;
pub mod policy;
pub mod policy_board;
pub mod policy_estimate;
pub mod policy_opening;
pub mod policy_plan;
pub mod prefix;
pub mod recovery;
pub mod replay;
pub mod route;
pub mod runner;
pub mod ship;
pub mod shopping;
pub mod split;
pub mod start;
pub mod talk;
pub mod tape;
pub mod validate;
pub mod walk;

pub use cell_plan::{
    CellPlan, CellPlanError, Flood, Goal, Mover, held_by_npc, plan_cells, plan_cells_for,
};
pub use map_plan::{Hop, Leg, MapGraph, Plan, PlanError, Position, Target};
