//! The route table: one row per proven `(enemy, ability)` pair.
//!
//! The table is split by carrier family — one submodule per family, each with
//! its own reading of the arm, the object chain and the request shape — and
//! [`DAMAGE_SKILL_ROUTES`] is the registry of those slices in the order a
//! reader meets them. [`proven`] is the lookup the resolver uses, and it walks
//! every family, so a pair in any of them resolves and a pair in none of them
//! does not.

mod acid;
mod all_party;
mod dark_force;
mod firebreath;
mod flaeli;
mod flame;
mod machines;
mod motavia;
mod organic;
mod techniques;
mod zio;

use super::{DamageClass, ObjectDraws};

/// One `(enemy, ability)` pair whose `EnemyAttack_*` arm has been traced to a
/// proven damage-request shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DamageRoute {
    /// `stats.enemy_id` of the carrier: the `fighter_id` `Enemy_Attack`
    /// (`ps4.asm:19173`) indexes `EnemyAttackOffs` with.
    pub(super) enemy_id: u16,
    /// Raw ability byte the arm runs for, as `$24(a4)` holds it.
    pub(super) ability: u8,
    /// What the traced chain does with `move.w #$C`.
    pub(super) class: DamageClass,
    /// What the same chain takes off the shared stream before it does that.
    pub(super) draws: ObjectDraws,
}

/// Every family's rows, one slice per carrier family.
///
/// The order is the order the families were read, and nothing depends on it:
/// no pair appears in two families (the completeness test asserts it), so a
/// lookup is a search and not a precedence.
pub(crate) const DAMAGE_SKILL_ROUTES: &[&[DamageRoute]] = &[
    acid::ROUTES,
    flame::ROUTES,
    motavia::ROUTES,
    all_party::ROUTES,
    firebreath::ROUTES,
    zio::ROUTES,
    flaeli::ROUTES,
    machines::ROUTES,
    organic::ROUTES,
    techniques::ROUTES,
    dark_force::ROUTES,
];

/// Every row of every family, in registry order.
pub(crate) fn all() -> impl Iterator<Item = &'static DamageRoute> {
    DAMAGE_SKILL_ROUTES.iter().flat_map(|family| family.iter())
}

/// The route the table has proven for this exact pair, if any.
pub(crate) fn proven(enemy_id: u16, ability: u8) -> Option<&'static DamageRoute> {
    all().find(|route| route.enemy_id == enemy_id && route.ability == ability)
}
