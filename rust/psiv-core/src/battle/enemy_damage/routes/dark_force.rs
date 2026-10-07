//! DarkForce1's post-introduction damage arms. The shared first-action
//! latch belongs to engine_enemy; these routes are reached only afterwards.
//!
//! EnemyAttack_DarkForce1 (ps4.asm:20038-20095) loads $820 for FLARE
//! SHOT, $824 for PHONONMASR, otherwise $82C for BURSTROC.
//! $820 = loc_31F60 (64584), phase 3 -> loc_24A6C (48474), single.
//! $824 = loc_31CDC (64401), phase 3 -> loc_24A9E (48496), all party.
//! $82C = loc_31AB6 (64247), phase 3 -> loc_24A9E, all party.
//! Its $830/$838/$2AC children are visual only. No chain calls RNG;
//! nearby loc_320C2 (64847) belongs to death object $81C, not these arms.
use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    DamageRoute {
        enemy_id: 130,
        ability: 0x1C,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 130,
        ability: 0x20,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 130,
        ability: 0x63,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
];
