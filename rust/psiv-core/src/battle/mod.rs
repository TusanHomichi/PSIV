//! The battle math kernel — Tier 0 of `docs/BATTLE_SCOUT.md`.
//!
//! Damage, healing and the hit/miss/critical roll, transcribed from the
//! cartridge. Integer-only like the rest of the crate, and **decision-free**:
//! it computes, it does not choose.
//!
//! # Rolls are inputs
//!
//! Nothing here owns a random number generator. Every formula takes a
//! [`Rolls`] and masks the raw word where the cartridge masks it. That is not
//! fastidiousness — it is the open question in scout §16.1. Retail's battle
//! rolls come from `UpdateRNGSeed2`, whose entropy is the VDP's H/V counter,
//! so bit-exact battle randomness is off the table for a headless runtime and
//! the substitute has not been chosen. Until it is, this module refuses to
//! pick one, and the formulas stay verifiable regardless of what does.
//!
//! [`Lcg41`] is provided because it is the *other* generator — the portable
//! `UpdateRNGSeed` that encounter rolls already use — not as a
//! recommendation.
//!
//! # What is here, and where it came from
//!
//! | item | retail | scout |
//! |---|---|---|
//! | [`Lcg41`] | `$04236C` | §2 |
//! | [`calculate_damage`] | `$00B5CA` | §4.5 |
//! | [`clamp_damage`] | `$00266C` | §4.5 |
//! | [`calc_healing`] | `$00B5FE` | §4.5 |
//! | [`calculate_chances`] | `$00B5A6` | §5 |
//! | [`STAT_OFFSETS`] | `$00275A` | §4.1 |
//! | [`ELEMENT_OFFSETS`] | `$00276A` | §4.1 |
//!
//! # What is deliberately absent
//!
//! Stat *plumbing*. The tables say which offset to read and how wide; reading
//! it needs a fighter struct this tier does not define. The physical-skill
//! weapon-element fallback (`element_id >= $10`) is the visible edge of that:
//! [`element_offset`] returns `None` rather than defaulting to physical, so a
//! caller cannot get plausible wrong numbers out of it.

mod chances;
mod damage;
mod rng;
mod tables;

pub use chances::{CHANCE_ROLL_MASK, ESCAPE, PHYSICAL, START_PRIORITY, Verdict, calculate_chances};
pub use damage::{
    DAMAGE_DRAWS, DAMAGE_ROLL_MASK, MAX_DAMAGE, MIN_DAMAGE, calc_healing, calculate_damage,
    clamp_damage,
};
pub use rng::{Lcg41, RESEED, Rolls, SliceRolls};
pub use tables::{
    ELEMENT_NAMES, ELEMENT_OFFSETS, STAT_INDEX_MASK, STAT_OFFSETS, StatSlot, StatWidth,
    WEAPON_ELEMENT_SENTINEL, WORD_STAT_THRESHOLD, element_name, element_offset, stat_slot,
};
