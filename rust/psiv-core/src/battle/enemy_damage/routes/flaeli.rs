//! FLAELI: the Zelan sabotage carrier and the other five retail carriers.
//!
//! `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`), arm `loc_E890`
//! (20887-20905), keeps `Current_Target_Index` and loads `$728` with the
//! stored `$38` target. `loc_2BAA2` (57218) and its visual children `$2AC`
//! and `$340` make one request, at `loc_2BC6C` (57328-57339), after the
//! target's flinch timer clears. The four ChaosSorcr-family carriers share
//! that exact chain. No reachable routine calls `UpdateRNGSeed2`.
//!
//! SaLews's `$5A` arm (20396-20413) instead loads `$794` = `loc_29A76`
//! (54949), whose phase 8 exits through `loc_24B20` (48523-48529).
//! GiLeFarg's `loc_E008` arm (20318-20335) loads `$7BC` = `loc_28E36`
//! (54155), whose phase 8 exits through `loc_24A6C` (48468-48474).
//! Both are also one stored-target request, with no animation RNG calls.

use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    DamageRoute {
        enemy_id: 111,
        ability: 0x5A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 112,
        ability: 0x5A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 113,
        ability: 0x5A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 138,
        ability: 0x5A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 121,
        ability: 0x5A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 125,
        ability: 0x5A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
];
