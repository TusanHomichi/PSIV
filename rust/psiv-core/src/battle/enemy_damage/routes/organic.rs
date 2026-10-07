//! Organic damage chains; no reachable UpdateRNGSeed2 call.
//!
//! CELL SPLIT: Slug's $14C (loc_10644,ps4.asm:22959; BattleObj_CellSplit,
//! 35240), with $150/$154/$158/$15C visual children, requests all party
//! slots at 35320. SnowSlug and both FractOoze carriers share it. This is ability
//! $13, not the conditional FISSION arm.
//! DBL SLASH: Mole's $8E0 (loc_D59E,19646; BattleObj_EnemyDblSlash,
//! 68007) ends in the single-target loc_24A6C (48468-48474).
//! RAY BREATH: Heads' $230/$22C (loc_FE52,22389; loc_1628A,31273;
//! loc_16406,31380) requests $38 at 31453. LwAddmer's $784
//! (loc_E320,20544; loc_29EEE,55243) ends in loc_24A6C.
//! SUPERSONIC: BiterFly's Scorpirus arm (loc_FA2A,22072-22085) loads
//! $28C = BattleObj_Supersonic (40549), phase 3 -> loc_24BB6
//! (48562-48575). Skytiara's $8C4 (loc_D5D4,19663;
//! BattleObj_OwlSupersonic,68761) reaches the same five-slot tail.
//! NEEDLE: Rajago/BiterFly's $288 (loc_FA1A,22067; BattleObj_Needle,
//! 40740) reaches single-target loc_24B64 (48547). HungryMole's $8D4
//! (loc_D56E,19633; BattleObj_MoleNeedle,68370) reaches loc_24B20 (48529).
//! WIND STORM: Owltalon's $8CC (loc_D61E,19680; loc_3526A,68568)
//! reaches loc_24B20: one target despite record target nibble 9.
//! DISTORTION: DimensWorm/OuterBeast share EnemyAttack_DimensWorm
//! (19280-19281). Its $30 arm (loc_F7A6,21903-21909) clears the target
//! and loads $2D4 = loc_1CC64 (39283). State 8 reaches loc_24BB6
//! (39299-39300), the five-slot damage request. Its palette/sine animation
//! (loc_1CD52/loc_1CE1E,39369-39516) consumes no UpdateRNGSeed2 draws.
use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    DamageRoute {
        enemy_id: 73,
        ability: 0x30,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 74,
        ability: 0x30,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 37,
        ability: 0x13,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 38,
        ability: 0x13,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 137,
        ability: 0x13,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 145,
        ability: 0x1F,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 146,
        ability: 0x1F,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 60,
        ability: 0x22,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 61,
        ability: 0x22,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 118,
        ability: 0x22,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 69,
        ability: 0x23,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 142,
        ability: 0x23,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 68,
        ability: 0x2B,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 69,
        ability: 0x2B,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 146,
        ability: 0x2B,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    DamageRoute {
        enemy_id: 143,
        ability: 0x6A,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
];
