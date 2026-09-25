//! Specification of the event engine: state, triggers, and scenes.
//!
//! One target, one module per concern, so that a reader looking for "what does
//! the trigger table do" does not have to read "how a scene walks an actor":
//!
//! * `triggers` -- the retail trigger table evaluated against state and
//!   position, entry by entry and list by list;
//! * `scenes` -- one scene's ops driven through `SceneRunner`, tick by tick;
//! * `registry` -- the shipped scenes (`scene_for`), and the determinism a
//!   replay depends on;
//! * `sampling` -- what the oracle lane samples here: the step-duration ladder
//!   and a scene recast across a map change;
//! * `fixtures` -- the synthetic scene and the helpers those modules share.
//!
//! The target is still one check --
//! `cargo test --manifest-path rust/Cargo.toml -p psiv-core --test events` --
//! and the module names appear as prefixes on the test names.

#[path = "common/mod.rs"]
mod common;

#[path = "events/fixtures.rs"]
mod fixtures;

#[path = "events/registry.rs"]
mod registry;

#[path = "events/sampling.rs"]
mod sampling;

#[path = "events/scenes.rs"]
mod scenes;

#[path = "events/triggers.rs"]
mod triggers;
