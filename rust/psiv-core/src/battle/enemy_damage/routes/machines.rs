//! Machine damage arms. All loaded children were checked against the 43
//! UpdateRNGSeed2 sites (docs/source-notes/battle-enemy-abilities.md).
//!
//! LASRCANNON: ProtectBit's $60 object (ps4.asm:23637,30592) requests
//! $38 at 30683; Seeker/Sweeper's $6C (23663-23676,30851) shares the
//! single-request tail at 30818. Loader/Debugger's $20C (loc_FF86,
//! 22470; loc_16C04,31936) requests $38 at 31985.
//! FLARE SHOT: Warren/Siren/Browren's $1D8 (loc_1006E,22545;
//! loc_1786E,32828) requests $38 at 32877.
//! DBL SLASH: Debugger/Dominator's $204 (loc_FF5E,22459;
//! loc_16D1C,32015) and visual child $208 have one request at 32125.
//! PHONONMASR: Dominator's $210 (loc_FFBE,22484; loc_169C6,31782)
//! requests all party slots at 31881.
//! CHARGCNNON: C-RayTube's non-WARNING arm (22850-22888) loads
//! $17C/$180/$184/$188/$18C. The two beam objects enter loc_18FA2
//! (34439-34453) together: the same frame writes the same five routine
//! words, so each fighter runs routine $C once, not once per beam.
//! EXPLOSION: FloatMine/FloatMine2's $19C (22692; loc_18478,33630)
//! requests $38 at loc_18574 (33694-33724), then removes the actor.
//! DETONATION: CommndBall's $1A0-$1C4 (22705-22766) ends in the
//! five-slot loop at loc_17C9E (33114-33127), then removes both neighbor
//! objects at loc_17D4A (33154-33177). $1C4 = loc_18344 (33550) jumps
//! BACK to loc_182C8. It does not reach adjacent loc_18478 (EXPLOSION).
use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    DamageRoute {
        enemy_id: 4,
        ability: 0x04,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 7,
        ability: 0x04,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 8,
        ability: 0x04,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 51,
        ability: 0x04,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 52,
        ability: 0x04,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 47,
        ability: 0x1C,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 48,
        ability: 0x1C,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 49,
        ability: 0x1C,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 52,
        ability: 0x1F,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 53,
        ability: 0x1F,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 53,
        ability: 0x20,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 40,
        ability: 0x15,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 44,
        ability: 0x18,
        class: DamageClass::SingleRemoveActor,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 50,
        ability: 0x18,
        class: DamageClass::SingleRemoveActor,
        draws: ObjectDraws::None,
        sealable: false,
    },
    DamageRoute {
        enemy_id: 45,
        ability: 0x19,
        class: DamageClass::AllPartyRemoveNeighbours,
        draws: ObjectDraws::None,
        sealable: false,
    },
];
