//! Which resolver owns an `(enemy, ability)` pair.
//!
//! [`super::Battle`]'s enemy turn (`engine_enemy.rs`) hands the rolled ability
//! to a chain of resolvers, each gated on its own table of traced arms. A pair
//! no resolver owns takes the explicit [`super::BattleEvent::UnsupportedAbility`]
//! fallback. [`owner`] asks the same gates without a battle, so a census can
//! hold every pair a record can roll - its eight regular ids and its four
//! conditional ones - against the dispatch: the per-carrier gaps an
//! ability-level ledger cannot see (one carrier of an ability routed, another
//! not) show up here by name.

use super::BattleData;

/// The resolver that runs a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// A routine that never dispatches the roll: the Zio family's phase arms
    /// (`zio.rs`) and Profound Darkness's form logic.
    Scripted,
    /// `enemy_fusion`: Fusion and COMBINE reload the side from an inline record.
    Reload,
    /// `enemy_skill::resolve_fission`: the refill behind `EnemyAI_EmptySpace`.
    Refill,
    /// `enemy_damage`: a traced damage request.
    Damage,
    /// `enemy_effect`: a traced status or stat handler.
    Effect,
    /// `enemy_skill::resolve_res`: TechUser's heals.
    Heal,
    /// `enemy_skill::resolve_no_effect_turn`: a roll the routine has no arm for.
    Wasted,
}

/// The resolver that runs `ability` for `enemy`, or `None` when the turn would
/// take the unsupported fallback. Ability 0 is the ordinary attack and has no
/// owner here.
#[must_use]
pub fn owner(data: &BattleData, enemy: u16, ability: u8) -> Option<Owner> {
    if ability == 0 {
        return None;
    }
    let skill = data.enemy_skill(ability)?;
    if super::zio::ignores_roll(enemy) {
        return Some(Owner::Scripted);
    }
    if super::enemy_form::owns(enemy, skill) {
        return Some(Owner::Scripted);
    }
    if super::enemy_fusion::owns(enemy, skill) {
        return Some(Owner::Reload);
    }
    if super::enemy_skill::owns_refill(skill) {
        return Some(Owner::Refill);
    }
    if super::enemy_damage::owns(enemy, skill) {
        return Some(Owner::Damage);
    }
    if super::enemy_effect::owns(enemy, skill) {
        return Some(Owner::Effect);
    }
    if super::enemy_skill::owns_heal(enemy, skill) {
        return Some(Owner::Heal);
    }
    if super::enemy_skill::owns_wasted(enemy, skill) {
        return Some(Owner::Wasted);
    }
    None
}
