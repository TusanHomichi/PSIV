//! HEWN: all non-Zio carriers, each with no animation RNG.
//!
//! Phantom's $70C (loc_EB94,ps4.asm:21074; loc_2C6E2,58047) ends
//! in the five-slot loc_24BB6 (48562-48575).
//! ChaosSorcr's four carriers load $730 (loc_E926,20923;
//! loc_2B6F8,56987) and share CORRSION's five-slot loc_2C03A
//! (57574-57593). SaLews loads $798 (loc_E136,20414;
//! loc_299BE,54905) and ends in loc_24BB6.
//! Zio2 remains owned by the specialized Zio resolver.
use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    DamageRoute {
        enemy_id: 108,
        ability: 0x4F,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 111,
        ability: 0x4F,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 112,
        ability: 0x4F,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 113,
        ability: 0x4F,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 138,
        ability: 0x4F,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 121,
        ability: 0x4F,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
];
