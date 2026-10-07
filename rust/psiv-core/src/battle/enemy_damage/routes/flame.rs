//! `$02` FLAME BOLT: 0 Helex and 5 ForcedFly, one object chain and one request.

use super::super::{DamageClass, FLAME_BOLT, ObjectDraws};
use super::DamageRoute;

/// **FLAME BOLT `$02`.** `EnemyAttack_ForcedFly` (`ps4.asm:23567`) branches to
/// `EnemyAttack_MonsterFly` (`ps4.asm:23591`) only for ability 0 and otherwise
/// falls through into `EnemyAttack_Helex` (`ps4.asm:23574`), which writes
/// object `$48` (line 23575) — `BattleObj_HelexFlameBolt` (`ps4.asm:30279`) —
/// and, like both Acid Breath arms, never touches `Current_Target_Index`. That
/// object only animates; on animation frame 2 (`cmpi.b #2, $10(a4)`, line
/// 30294) it loads `BattleObj_HelexFlameBolt2` (`ps4.asm:30315`, the `$4C`
/// write at line 30300) and copies `$38`/`$3C` into it (lines 30301-30302),
/// keeping the target the loading path stored in the parent. The child waits
/// out `$2E` = `$110`, writes the hit reaction (`move.w #5, $2(a3)`,
/// `$1C = $C`, lines 30334-30335) and then makes the single damage request
/// `move.w #$C, $2(a3)` (`ps4.asm:30342`) behind the `btst #1, $4(a4)` /
/// `bset #1, $4(a4)` once-guard, waited on through `($FFFF416C)`. That is one
/// request against the chosen party target, and the arm writes EnemyAttack3
/// `$D7` at the parent's load (line 30285) and FireBreath `$C2` at the child's
/// (line 30321).
///
/// FLAME BOLT's carriers are 0 Helex, whose eight regular slots are all `$02`,
/// and 5 ForcedFly, whose slots 5-8 are `$02` and whose lower slots are zero
/// (`generated/enemies.json`): ForcedFly's zero roll takes the
/// `EnemyAttack_MonsterFly` branch, so `$02` is the only nonzero ability either
/// carrier can dispatch.
///
pub(super) const ROUTES: &[DamageRoute] = &[
    // `EnemyAttackOffs` `$00` (`ps4.asm:19207`) → `EnemyAttack_Helex`
    // (`ps4.asm:23574`) → object `$48` = BattleObj_HelexFlameBolt
    // (`ps4.asm:30279`), child `$4C` = BattleObj_HelexFlameBolt2
    // (`ps4.asm:30315`); one request at `ps4.asm:30342`.
    DamageRoute {
        enemy_id: 0,
        ability: FLAME_BOLT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // `EnemyAttackOffs` `$05` (`ps4.asm:19212`) → `EnemyAttack_ForcedFly`
    // (`ps4.asm:23567`) falls through into `EnemyAttack_Helex` for a nonzero
    // ability: the same object chain and the same single request.
    DamageRoute {
        enemy_id: 5,
        ability: FLAME_BOLT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 71 FrostSaber, `$2E` GIWAT: `EnemyAttackOffs` `$47` (`ps4.asm:19278`) →
];
