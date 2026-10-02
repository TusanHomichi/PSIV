//! `$33` ACIDBREATH: the four `(enemy, ability)` pairs whose arm was traced to
//! `BattleObj_AcidBreath` or `loc_23998`.
//!
//! `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`) and `EnemyAttack_Piercer`
//! (`ps4.asm:21518`) are the two arms; both leave `Current_Target_Index` alone
//! for `$33` and both chains make exactly one `$38` request. `AbilityEffect_None`
//! is the record's effect, so the request is the whole turn.

use super::super::{ACID_BREATH, DamageClass, ObjectDraws};
use super::DamageRoute;

/// **Acid Breath `$33`.** `EnemyAttackOffs` (`ps4.asm:19206`) gives
/// `EnemyAttack_FlattrPlnt` to enemy ids `$4B`, `$4C` and `$4D` — 75 FlattrPlnt,
/// 76 FlyScreamr and 77 TechPlant — and `EnemyAttack_Piercer`
/// (`ps4.asm:21518`) to `$55` and `$56`, 85 Piercer and 86 HakenLeft. The gate
/// is the proven *carrier* set rather than the routine set: a routine can only
/// run for an enemy that rolled `$33`, and 77 TechPlant's US ability list is
/// `$2A`/`$2E` only (`generated/enemies.json`), so it never reaches the `$33`
/// arm even though it shares the routine.
///
/// `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`) keeps `Current_Target_Index` for
/// `$33`: the `loc_F5BE` arm has no write to it, while `$34` (`loc_F60A`),
/// `$2A` (`loc_F65E`) and the fallback (`loc_F722`) all clear it. The arm
/// converts the enemy's own attack object into `BattleObj_AcidBreath`
/// (`ps4.asm:38566`) and loads `BattleObj_AcidBreathChild` (`ps4.asm:38622`).
/// The child only animates and asks for the hit reaction
/// (`move.w #5, $2(a3)` / `$1C = $E`); the main object's `loc_24AEC` exit
/// (`ps4.asm:48507`) is the single damage request, `move.w #$C, $2(a3)` gated
/// on the target's hit timer and waited on through `($FFFF416C)`.
///
/// `EnemyAttack_Piercer`'s `$33` arm (`loc_F2A0`, `ps4.asm:21532`) also leaves
/// `Current_Target_Index` alone, but loads object `$35C` = `loc_23998`
/// (`ps4.asm:47264`) with the chosen party target in `$38(a1)` and then spawns
/// child `$360` = `loc_24FD2` (`ps4.asm:48883`). The child makes the same
/// hit-reaction write, and `loc_23AB6` (`ps4.asm:47344`) makes the same single
/// `move.w #$C, $2(a3)` damage request once the child releases
/// `($FFFFEE80)` — no attack, no status, one reaction and one hit for both
/// arms. Both write MoleAttack `$D5` and then EnemyAttack4 `$D8`.
///
/// `loc_B75A` supplies a normal hit without a chance roll and
/// `Enemy_DamageCharacter` (`ps4.asm:3775`) reads strength, defense and
/// physical resistance through the shared `EnemySkillData` record, so the
/// number depends on the caster's strength and not on which arm ran.
///
pub(super) const ROUTES: &[DamageRoute] = &[
    // `EnemyAttackOffs` `$4B` (`ps4.asm:19282`) → `EnemyAttack_FlattrPlnt`
    // (`ps4.asm:21778`), `$33` arm `loc_F5BE` → BattleObj_AcidBreath
    // (`ps4.asm:38566`); one request at `loc_24AEC` (`ps4.asm:48507`).
    DamageRoute {
        enemy_id: 75,
        ability: ACID_BREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // `EnemyAttackOffs` `$4C` (`ps4.asm:19283`) → the same
    // `EnemyAttack_FlattrPlnt` arm and object.
    DamageRoute {
        enemy_id: 76,
        ability: ACID_BREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // `EnemyAttackOffs` `$55` (`ps4.asm:19292`) → `EnemyAttack_Piercer`
    // (`ps4.asm:21518`), `$33` arm `loc_F2A0` → object `$35C` = `loc_23998`
    // (`ps4.asm:47264`); one request at `loc_23AB6` (`ps4.asm:47344`).
    DamageRoute {
        enemy_id: 85,
        ability: ACID_BREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // `EnemyAttackOffs` `$56` (`ps4.asm:19293`) → the same
    // `EnemyAttack_Piercer` arm and object.
    DamageRoute {
        enemy_id: 86,
        ability: ACID_BREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
];
