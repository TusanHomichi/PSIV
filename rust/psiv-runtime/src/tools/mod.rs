//! The package's development tools: whole operations that need the runtime's
//! crate-private seams.
//!
//! A game is played through a [`Session`](crate::Session) — one pad per frame,
//! and no way to reach a runtime mutator. Two jobs in this package are not
//! games, and both need those seams:
//!
//! * [`oracle_replay`]: replay a recorded oracle tape through the field engine
//!   and hand back one row per frame (`rust/psiv-runtime/src/bin/psiv-replay.rs`
//!   is its command line). It ticks the field with the tape's own `Input`, seeds
//!   the shared RNG and parks the camera — state a cartridge-tape comparison
//!   inherits rather than plays.
//! * [`repair`]: repair a legacy save whose level-ups dropped learned abilities
//!   (`docs/camp/PROGRESSION.md`), through
//!   `rust/psiv-runtime/examples/repair_progression.rs`.
//!
//! Each entry point performs one whole documented operation and returns a
//! report. None of them hands out a `&mut Runtime`, and none is reachable from
//! a session's frame: they exist so the crate's own binaries and examples do
//! not need one.

pub mod oracle_replay;
pub mod repair;

pub use repair::{ProgressionRepairReport, repair_legacy_progression};
