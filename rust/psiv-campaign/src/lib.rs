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
//! It plans; it never moves a party.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::float_arithmetic)]

pub mod cell_plan;
pub mod map_plan;
pub mod route;
pub mod validate;

pub use cell_plan::{CellPlan, CellPlanError, Flood, Goal, plan_cells};
pub use map_plan::{Hop, Leg, MapGraph, Plan, PlanError, Position, Target};
