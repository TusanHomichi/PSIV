//! The endgame bosses' damage arms (lane A6): De Vars, Sa Lews, Re Faze and
//! Profound Darkness's three forms.
//!
//! DISRUPTARM: EnemyAttack_DeVars (EnemyAttackOffs $78, ps4.asm:19327,
//! 20509) sends every nonzero id to $790 (loc_29C1E, 55045) with $FFFF
//! (20515-20516); its state 4 is loc_24BB6 (48575). No call.
//! TANDLE and LEGEON (121 SaLews, EnemyAttack_SaLews 20396): loc_E188 (20436,
//! $FFFF) loads $79C (loc_29910, 54862), loc_E1DC (20455, $FFFF) loads $7A0
//! (loc_297D0, 54790); both end in loc_24BB6. No call, no seal test.
//! MEGID (127 ReFaze, EnemyAttack_ReFaze 20193): no ability test, $FFFF
//! (20194) and $7E0 (loc_284C0, 53503) every turn, ending in loc_24BB6. No call.
//! Profound Darkness 1 (EnemyAttack_ProfoundDarkness1, 19823), once its
//! first action has spent $FFFFEE87: SHDWBREATH -> $86C (loc_301B6, 62539) ->
//! loc_24B20; FIREBREATH -> $870 (loc_30022, 62419), whose $874 (62449)
//! requests through loc_24A6C; RAY BREATH -> $878 (loc_2FE7C, 62296) with
//! $87C (62326), loc_24A6C. All three keep the drawn target. No call.
//! Profound Darkness 2 (EnemyAttack_ProfoundDarkness2, 19750): DISTORTION
//! (19767, $FFFF) -> $890 (loc_2F1B2, 61394) -> loc_24BB6; ANOTHRGATE
//! (19783, $FFFF) -> $894 (loc_2F060, 61289), which loads Lashiec's own $7F8
//! (61338): its 236 calls, then loc_24BB6; LIGHTSHOWR (19801, $FFFF) -> $898
//! (loc_2EF0E, 61209) -> loc_24A9E. No other call.
//! Profound Darkness 3 (EnemyAttack_ProfoundDarkness3, 19692): every id but
//! 0, $4C and $69 - MEGID - takes loc_D6F2 ($FFFF) and $8AC (loc_2DF4A,
//! 60068): `ObjectDraws::Megid`, then loc_24BB6.
use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

const fn row(enemy_id: u16, ability: u8, class: DamageClass, draws: ObjectDraws) -> DamageRoute {
    DamageRoute {
        enemy_id,
        ability,
        class,
        draws,
        sealable: false,
    }
}

pub(super) const ROUTES: &[DamageRoute] = &[
    row(120, 0x59, DamageClass::AllParty, ObjectDraws::None),
    row(121, 0x52, DamageClass::AllParty, ObjectDraws::None),
    row(121, 0x55, DamageClass::AllParty, ObjectDraws::None),
    row(127, 0x5E, DamageClass::AllParty, ObjectDraws::None),
    row(133, 0x64, DamageClass::Single, ObjectDraws::None),
    row(133, 0x21, DamageClass::Single, ObjectDraws::None),
    row(133, 0x22, DamageClass::Single, ObjectDraws::None),
    row(134, 0x30, DamageClass::AllParty, ObjectDraws::None),
    row(134, 0x61, DamageClass::AllParty, ObjectDraws::AnotherGate),
    row(134, 0x65, DamageClass::AllParty, ObjectDraws::None),
    row(135, 0x5E, DamageClass::AllParty, ObjectDraws::Megid),
];
