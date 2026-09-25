//! Replaying a captured battle against the cartridge's own rolls.
//!
//! `oracle/battle_fixture.py` turns one oracle run's RNG trace and RAM log into
//! a fixture under `replay_fixtures/`: the battle's start state, the rolls the
//! cartridge drew (each with the frame it was drawn in and the role it played)
//! and what the log shows every action doing. This directory is the harness the
//! replays share: it builds the battle a fixture describes - the vehicle battle
//! included, through the port's own vehicle path - feeds those rolls through
//! [`SliceRolls`], and compares the port's timeline with the cartridge's,
//! action by action.
//!
//! * [`fixture`] - the fixture's shape, as the extractor writes it.
//! * [`build`] - the battle a fixture describes, started on its own priority
//!   roll.
//! * [`stream`] - the verbatim stream: the cartridge's rolls, in its order, and
//!   the per-action accounting that says the port consumed them.
//! * [`divergence`] - the finding itself: what a comparator can report.
//! * [`compare`] - the comparator: the first place a round's timeline disagrees
//!   with the log.
//! * [`pack`] - the pack records the forced captures' enemies and abilities
//!   need, beside the hand-built fixtures.
//! * [`data`] - the one data-driven test: every fixture in `replay_fixtures/`,
//!   against its entry in `replay_fixtures/divergences.json` (or exact).
//! * [`crate::battle::engine_tests_replay_tape07`] and its tape 09 sibling - the
//!   two basement battles walked in more detail.
//!
//! A roll is `hv + frame_count - high_word(RNG_Seed)`: `UpdateRNGSeed2`
//! (`ps4.asm:86097`) subtracts the word at `$FFFFEF0C`, which on a big-endian
//! 68000 is the seed longword's *high* half. The fixture derives each roll from
//! the trace's raw columns for that reason; `docs/oracle/BATTLE_ORACLE_REPLAY.md` has
//! the proof, the trace's own `roll` column being the low-half subtraction.
//!
//! # One stream, and what it costs
//!
//! The cartridge's stream is replayed verbatim: every call the battle's frames
//! hold, in the order it drew them. The two draw-count divergences this port
//! used to carry are closed, so no roll has to be filtered out any more.
//! `AlysKyraAttack_Init` re-runs `loc_B6A2` (`ps4.asm:13975-13976`) and
//! `resolve_attack` draws the second pass for the two attackers the cartridge
//! sends there; `Enemy_Attack`'s ability re-roll against `$FFFFEEA8`
//! (`ps4.asm:19146-19151`) is modelled, so a battle begins with the word at
//! zero (the `GameMode_LoadBattle` wipe of the page it lives in,
//! `ps4.asm:9992-9994`) and pays a second call for a first draw of zero.
//! Two draws stay outside the battle's own state machine and out of its stream:
//! the encounter's formation draw and the post-victory item drop, which the
//! fixture keeps in `outside_rolls`.
//!
//! What the harness asserts, per fixture:
//!
//! * [`stream::replay_verbatim`] - the port resolves what the log resolved,
//!   action for action, and consumes the cartridge's rolls and no more;
//! * [`stream::account_for_every_roll`] - every call the log's frames hold is
//!   one the port's own consumers account for, down to the ability re-roll;
//! * [`data::every_fixture_replays_as_recorded`] - a fixture either replays
//!   exactly or diverges exactly where `divergences.json` says it does.

mod build;
mod compare;
mod data;
mod divergence;
mod fixture;
mod pack;
mod stream;

pub(crate) use build::*;
pub(crate) use compare::*;
pub(crate) use divergence::*;
pub(crate) use fixture::*;
pub(crate) use stream::*;
