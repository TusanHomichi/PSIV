//! The Motavia single-target pairs: the `$2E` GIWAT family, `$37` SAND STORM,
//! `$39` MAELSTROM, `$3F` FLODBREATH, `$40` WAT, `$44` FOI, `$6D` ROUND EYES
//! and `$6E` LOVEL EYES carriers.
//!
//! Every one of them makes exactly one `move.w #$C` request against the target
//! the arm stored in its object's `$38` — through one of the four single-target
//! shared tails (`loc_24A6C`, `loc_24AEC`, `loc_24B20`, `loc_24B64`) or through
//! a request in the object's own phase table.

use super::super::{
    DamageClass, FLODBREATH, FOI, GIWAT, LOVEL_EYES, MAELSTROM, ObjectDraws, ROUND_EYES,
    SAND_STORM, WAT,
};
use super::DamageRoute;

/// **GIWAT `$2E`.** One arm per routine, all loading a target-carrying object:
///
/// - 71 FrostSaber (`EnemyAttack_ShadowSabr`, `ps4.asm:21933`): `loc_F8F2`
///   (`ps4.asm:21987`) writes object `$2B4` (line 22003) = `loc_1D5AA`
///   (`ps4.asm:39951`; the `$2B4` entry of `BattleObjsGroup4Ptrs` is line 37748)
///   with `$38`; its
///   phase `loc_1D6A0` makes the one request at line 40042.
/// - 77 TechPlant (`EnemyAttack_FlattrPlnt`): `loc_F6C4` (`ps4.asm:21846`)
///   writes `$2FC` (line 21861) = `BattleObj_EnemyGiwat` (`ps4.asm:38195`) plus
///   the visual child `$2F0` (line 21867) = `BattleObj_AcidBreathChild`
///   (`ps4.asm:38622`). The phase table `loc_1BE1C` (`ps4.asm:38212`) jumps to
///   `loc_24AEC` (line 38215): the one request at line 48513.
/// - 91 HewGilla (`EnemyAttack_HewGilla`, `ps4.asm:21395`): the routine tests
///   only `$3F` (line 21400) and `$40` (line 21418), so `$2E` takes the else
///   arm `loc_F108` (`ps4.asm:21426`), writing `$390` (line 21430) =
///   `loc_22670` (`ps4.asm:45925`) with `$38`. Its phase table `loc_226A2`
///   (`ps4.asm:45940`) branches to `loc_24B20` (line 45943): the one request at
///   line 48529.
/// - 101 DarkWitch (`EnemyAttack_TechUser`, `ps4.asm:21156`): `loc_EE0E`
///   (`ps4.asm:21229`) writes `$3C4` (line 21244) = `loc_21852`
///   (`ps4.asm:45013`) with `$38`. Its phase table `loc_21886`
///   (`ps4.asm:45026`) sends phase 8 to the shared `loc_21D08`
///   (`ps4.asm:45302`): one request at line 45310, target `movea.l $38(a4), a3`
///   (line 45307).
/// - 122 DElmLars and 123 XeAThoul (`EnemyAttack_DElmLars`, `ps4.asm:20222`):
///   `loc_DFBE` (`ps4.asm:20300`) writes `$7B8` (line 20315) = `loc_28F76`
///   (`ps4.asm:54226`) with `$38`. Its phase table `loc_28FAC`
///   (`ps4.asm:54239`) jumps to `loc_24A6C` (line 54242): the one request at
///   line 48474.
///
/// **SAND STORM `$37` and MAELSTROM `$39`.** Record target byte 9 — the
/// all-party *nibble* — but both chains make exactly one request. 81 DesrtLeach
/// (`EnemyAttack_SandWorm`, `ps4.asm:21658`) takes `loc_F48A`
/// (`ps4.asm:21692`), writing `$328` (line 21707) = `BattleObj_SandStorm`
/// (`ps4.asm:48204`); 82 Leviathan takes the else arm `loc_F4FA`
/// (`ps4.asm:21720`), writing `$338` (line 21733) = `BattleObj_Maelstrom`
/// (`ps4.asm:47766`). Each phase table (`loc_246A6` `ps4.asm:48215`,
/// `loc_240C8` `ps4.asm:47778`) branches to `loc_24B64` (lines 48217 and
/// 47780): one request at line 48547 against `$38(a4)`, the target both arms
/// stored. `loc_F4D4` — the `$38` EARTHQUAKE arm that clears
/// `Current_Target_Index` and loads the five-slot `$330` object — is not part
/// of either route.
///
/// **FLODBREATH `$3F`.** 90 Depcen (`EnemyAttack_Ismounos`, `ps4.asm:21434`)
/// has no per-id arm: `tst.w $24(a4)` (line 21439) sends every nonzero ability
/// to `loc_F13C` (`ps4.asm:21443`), which writes `$380` (line 21451) =
/// `BattleObj_FlodBreath` (`ps4.asm:46319`). 91 HewGilla and 92 Elmelew share
/// `EnemyAttack_HewGilla`'s `$3F` arm (test at line 21400), writing `$384`
/// (line 21409) = `loc_22A90` (`ps4.asm:46199`). Both phase tables
/// (`loc_22C8E` `ps4.asm:46333`, `loc_22ABA` `ps4.asm:46211`) branch to
/// `loc_24B20`: one request at line 48529. Their chains write MoleAttack `$D5`
/// as the wind-up starts and EnemyAttack4 `$D8` before that request.
///
/// **WAT `$40`.** 91 HewGilla and 92 Elmelew take `loc_F0CC`
/// (`ps4.asm:21412`, the `$40` test at line 21418), writing `$388`
/// (line 21423) = `BattleObj_EnemyWat` (`ps4.asm:45978`), whose phase table
/// `loc_22766` (`ps4.asm:45992`) branches to `loc_24B20` at line 45995 — one
/// request at line 48529. 99 TechUser and 100 TechMaster take `loc_EDC4`
/// (`ps4.asm:21211`), writing `$3C0` (line 21226) = `loc_218D6`
/// (`ps4.asm:45047`), whose phase table `loc_2190A` (`ps4.asm:45060`) sends
/// phase 8 to `loc_21D08` — the one request at line 45310. 114 Juza takes
/// `loc_E3DE` (`ps4.asm:20593`), writing `$744` (line 20608) = `loc_2B006`
/// (`ps4.asm:56437`), whose phase table `loc_2B036` (`ps4.asm:56449`) sends
/// phase 8 to `loc_2B192` (line 56452) — the one request at line 56553.
///
/// **FOI `$44`.** 99 TechUser and 100 TechMaster take `EnemyAttack_TechUser`'s
/// first arm (test at line 21157), writing `$3B0` (line 21171) = `loc_21BF0`
/// (`ps4.asm:45229`); its phase table `loc_21C24` (`ps4.asm:45242`) sends
/// phase 8 to the same shared `loc_21D08`. 114 Juza takes
/// `EnemyAttack_Juza`'s first arm (`ps4.asm:20575`, test at line 20576),
/// writing `$740` (line 20590) = `loc_2B08E` (`ps4.asm:56477`), whose phase
/// table `loc_2B0BE` (`ps4.asm:56489`) sends phase 8 to the same `loc_2B192`.
/// Both therefore make their one request at the shared tails' lines (45310 and
/// 56553), and both chains write MoleAttack `$D5` at the wind-up and TechCast
/// `$BB` as the request phase starts.
///
/// **ROUND EYES `$6D` and LOVEL EYES `$6E`.** `EnemyAttack_Rappy`
/// (`ps4.asm:19578`) turns the attack object itself into the ability object:
/// nonzero, `$6D` writes `$8F8` (line 19590) = `BattleObj_RoundEyes`
/// (`ps4.asm:67760`), everything else writes `$8FC` (line 19593) =
/// `BattleObj_LovelEyes` (`ps4.asm:67729`), and both arms then reach
/// `loc_D200` (`ps4.asm:19395`) to set the object's `parent`. Their phase
/// tables (`loc_347B0` `ps4.asm:67772`, `loc_3474E` `ps4.asm:67741`) jump to
/// `loc_24B20` at phase 8 (lines 67775 and 67744): one request at line 48529
/// against `$38`, the target `Enemy_Attack` stored in the object. 148 BlueRappy
/// has no `$6D` in its list and 147 Rappy no `$6E`, so each pair owns its arm.
pub(super) const ROUTES: &[DamageRoute] = &[
    // `EnemyAttack_ShadowSabr` (`ps4.asm:21933`), arm `loc_F8F2`
    // (`ps4.asm:21987`) writing object `$2B4` (line 22003) = `loc_1D5AA`
    // (`ps4.asm:39951`) with the drawn target; one request at line 40042
    // behind `btst #3, $4(a4)`, target `movea.l $38(a4), a3` (line 40039).
    DamageRoute {
        enemy_id: 71,
        ability: GIWAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 77 TechPlant, `$2E` GIWAT: `EnemyAttackOffs` `$4D` (`ps4.asm:19284`) →
    // `EnemyAttack_FlattrPlnt` arm `loc_F6C4` (`ps4.asm:21846`), writing `$2FC`
    // (line 21861) = `BattleObj_EnemyGiwat` (`ps4.asm:38195`) plus the visual
    // child `$2F0` (line 21867); `jmp loc_24AEC` at line 38215, one request at
    // line 48513.
    DamageRoute {
        enemy_id: 77,
        ability: GIWAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 91 HewGilla, `$2E` GIWAT: `EnemyAttackOffs` `$5B` (`ps4.asm:19298`) →
    // `EnemyAttack_HewGilla` (`ps4.asm:21395`); `$2E` is the else arm
    // `loc_F108` (`ps4.asm:21426`), writing `$390` (line 21430) =
    // `loc_22670` (`ps4.asm:45925`); `bra.w loc_24B20` at line 45943, one
    // request at line 48529.
    DamageRoute {
        enemy_id: 91,
        ability: GIWAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 101 DarkWitch, `$2E` GIWAT: `EnemyAttackOffs` `$65` (`ps4.asm:19308`) →
    // `EnemyAttack_TechUser` (`ps4.asm:21156`), arm `loc_EE0E`
    // (`ps4.asm:21229`) writing `$3C4` (line 21244) = `loc_21852`
    // (`ps4.asm:45013`); phase 8 reaches `loc_21D08` (`ps4.asm:45302`), one
    // request at line 45310.
    DamageRoute {
        enemy_id: 101,
        ability: GIWAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 122 DElmLars, `$2E` GIWAT: `EnemyAttackOffs` `$7A` (`ps4.asm:19329`) →
    // `EnemyAttack_DElmLars` (`ps4.asm:20222`), arm `loc_DFBE`
    // (`ps4.asm:20300`) writing `$7B8` (line 20315) = `loc_28F76`
    // (`ps4.asm:54226`); `jmp loc_24A6C` at line 54242, one request at
    // line 48474.
    DamageRoute {
        enemy_id: 122,
        ability: GIWAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 123 XeAThoul, `$2E` GIWAT: `EnemyAttackOffs` `$7B` (`ps4.asm:19330`) →
    // the same `EnemyAttack_DElmLars` arm, object and request.
    DamageRoute {
        enemy_id: 123,
        ability: GIWAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 81 DesrtLeach, `$37` SAND STORM: `EnemyAttackOffs` `$51`
    // (`ps4.asm:19288`) → `EnemyAttack_SandWorm` (`ps4.asm:21658`), arm
    // `loc_F48A` (`ps4.asm:21692`) writing `$328` (line 21707) =
    // `BattleObj_SandStorm` (`ps4.asm:48204`); `bra.w loc_24B64` at line 48217,
    // one request at line 48547.
    DamageRoute {
        enemy_id: 81,
        ability: SAND_STORM,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 82 Leviathan, `$39` MAELSTROM: `EnemyAttackOffs` `$52`
    // (`ps4.asm:19289`) → `EnemyAttack_SandWorm`'s else arm `loc_F4FA`
    // (`ps4.asm:21720`), writing `$338` (line 21733) =
    // `BattleObj_Maelstrom` (`ps4.asm:47766`); `bra.w loc_24B64` at line 47780,
    // one request at line 48547.
    DamageRoute {
        enemy_id: 82,
        ability: MAELSTROM,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 90 Depcen, `$3F` FLODBREATH: `EnemyAttackOffs` `$5A`
    // (`ps4.asm:19297`) → `EnemyAttack_Ismounos` (`ps4.asm:21434`), whose
    // nonzero arm `loc_F13C` (`ps4.asm:21443`) writes `$380` (line 21451) =
    // `BattleObj_FlodBreath` (`ps4.asm:46319`); `bra.w loc_24B20` at line
    // 46335, one request at line 48529.
    DamageRoute {
        enemy_id: 90,
        ability: FLODBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 91 HewGilla, `$3F` FLODBREATH: `EnemyAttackOffs` `$5B`
    // (`ps4.asm:19298`) → `EnemyAttack_HewGilla`'s `$3F` arm (test at line
    // 21400), writing `$384` (line 21409) = `loc_22A90` (`ps4.asm:46199`);
    // `bra.w loc_24B20` at line 46213, one request at line 48529.
    DamageRoute {
        enemy_id: 91,
        ability: FLODBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 92 Elmelew, `$3F` FLODBREATH: `EnemyAttackOffs` `$5C`
    // (`ps4.asm:19299`) → the same `EnemyAttack_HewGilla` arm, object and
    // request.
    DamageRoute {
        enemy_id: 92,
        ability: FLODBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 91 HewGilla, `$40` WAT: `EnemyAttack_HewGilla`'s `$40` arm `loc_F0CC`
    // (`ps4.asm:21412`, test at line 21418) writes `$388` (line 21423) =
    // `BattleObj_EnemyWat` (`ps4.asm:45978`); `bra.w loc_24B20` at line 45995,
    // one request at line 48529.
    DamageRoute {
        enemy_id: 91,
        ability: WAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 92 Elmelew, `$40` WAT: `EnemyAttackOffs` `$5C` (`ps4.asm:19299`) → the
    // same arm and object.
    DamageRoute {
        enemy_id: 92,
        ability: WAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 99 TechUser, `$40` WAT: `EnemyAttackOffs` `$63` (`ps4.asm:19306`) →
    // `EnemyAttack_TechUser`'s `$40` arm `loc_EDC4` (`ps4.asm:21211`), writing
    // `$3C0` (line 21226) = `loc_218D6` (`ps4.asm:45047`); phase 8 reaches
    // `loc_21D08`, one request at line 45310.
    DamageRoute {
        enemy_id: 99,
        ability: WAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 100 TechMaster, `$40` WAT: `EnemyAttackOffs` `$64` (`ps4.asm:19307`) →
    // the same `EnemyAttack_TechUser` arm and object.
    DamageRoute {
        enemy_id: 100,
        ability: WAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 114 Juza, `$40` WAT: `EnemyAttackOffs` `$72` (`ps4.asm:19321`) →
    // `EnemyAttack_Juza` (`ps4.asm:20575`), arm `loc_E3DE` (`ps4.asm:20593`)
    // writing `$744` (line 20608) = `loc_2B006` (`ps4.asm:56437`); phase 8
    // reaches `loc_2B192` (line 56452), one request at line 56553.
    DamageRoute {
        enemy_id: 114,
        ability: WAT,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 99 TechUser, `$44` FOI: `EnemyAttack_TechUser`'s first arm (test at
    // line 21157) writes `$3B0` (line 21171) = `loc_21BF0`
    // (`ps4.asm:45229`); phase 8 reaches `loc_21D08`, one request at
    // line 45310.
    DamageRoute {
        enemy_id: 99,
        ability: FOI,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 100 TechMaster, `$44` FOI: `EnemyAttackOffs` `$64` (`ps4.asm:19307`) →
    // the same `EnemyAttack_TechUser` arm and object.
    DamageRoute {
        enemy_id: 100,
        ability: FOI,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 114 Juza, `$44` FOI: `EnemyAttack_Juza`'s first arm (`ps4.asm:20575`,
    // test at line 20576) writes `$740` (line 20590) = `loc_2B08E`
    // (`ps4.asm:56477`); phase 8 reaches `loc_2B192`, one request at
    // line 56553.
    DamageRoute {
        enemy_id: 114,
        ability: FOI,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 147 Rappy, `$6D` ROUND EYES: `EnemyAttackOffs` `$93`
    // (`ps4.asm:19354`) → `EnemyAttack_Rappy` (`ps4.asm:19578`), whose
    // nonzero arm `loc_D4BE` (`ps4.asm:19583`) writes object `$8F8`
    // (line 19590) = `BattleObj_RoundEyes` (`ps4.asm:67760`) into the attack
    // object itself and then reaches `loc_D200` (`ps4.asm:19395`) for the
    // parent pointer; phase 8 jumps to `loc_24B20` (line 67775), one request
    // at line 48529.
    DamageRoute {
        enemy_id: 147,
        ability: ROUND_EYES,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 148 BlueRappy, `$6E` LOVEL EYES: `EnemyAttackOffs` `$94`
    // (`ps4.asm:19355`) → the same routine's else arm `loc_D4DE`
    // (`ps4.asm:19592`), writing object `$8FC` (line 19593) =
    // `BattleObj_LovelEyes` (`ps4.asm:67729`); phase 8 jumps to `loc_24B20`
    // (line 67744), one request at line 48529.
    DamageRoute {
        enemy_id: 148,
        ability: LOVEL_EYES,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
];
