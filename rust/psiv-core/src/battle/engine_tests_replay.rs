//! The captured battles' replays, and where the machinery lives.
//!
//! Every piece of the harness - the fixture's shape, the battle it builds, the
//! verbatim-stream driver and the comparator - sits in
//! [`replay`](replay/index.html), the directory beside this file, so a new
//! capture is data: `oracle.fixture` writes it under
//! `replay_fixtures/` and [`replay`]'s data-driven test replays it. This module
//! is the wiring, and the two basement tapes' walks through it:
//!
//! * `engine_tests_replay_tape07.rs` - the first basement battle of
//!   `oracle/tapes/07_first_battle.tape`, round 1 walked in more detail.
//! * `engine_tests_replay_tape09.rs` - the second encounter, on its own seed
//!   path, with a critical (`oracle/tapes/09_second_battle.tape`).
//!
//! `docs/oracle/BATTLE_ORACLE_REPLAY.md` is the ledger for both tapes,
//! `docs/oracle/BATTLE_ORACLE_FORCED.md` for the forced captures whose enemy abilities
//! and vehicle battle the same machinery replays.

// The directory is named after this module's own subject; the lint against a
// module matching its parent's name is the price of `replay/mod.rs` sitting
// beside it, and the brief's layout is the clearer one.
#[allow(clippy::module_inception)]
#[path = "replay/mod.rs"]
mod replay;

#[path = "engine_tests_replay_tape07.rs"]
mod tape07;

#[path = "engine_tests_replay_tape09.rs"]
mod tape09;
