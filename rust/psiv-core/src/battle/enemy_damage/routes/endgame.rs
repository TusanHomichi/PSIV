//! The damage arms after the Air Castle (lane A6): the Weapon Plant, Vahal Fort
//! and Climate Center machines, the towers' and The Edge's soldiers and
//! casters, and the random enemies of Rykros, Island Cave and Garuberk Tower.
//! The bosses are `endgame_bosses.rs`.
//!
//! LIGHTNING: EnemyAttack_Sweeper (EnemyAttackOffs $08, ps4.asm:19215)
//! sends every id but $04 to loc_10EA8 (23679-23688): palette, then
//! BattleObj_SweeperAtk ($70, 30911) with the drawn target. State 8 is
//! Seeker's loc_15C1E, one request on $38 at 30818. No call.
//! MOTRCANNON: EnemyAttack_Slave ($12, 19225) writes $FFFF to
//! Current_Target_Index for a nonzero id (23452-23454) and loads $AC
//! (BattleObj_SlaveMotrCannon, 28736) and $B0 (28957). $AC's state 0 ends
//! with one UpdateRNGSeed2 (28770) whose `&3` picks the flinch row of
//! loc_140E4; its state $C writes #$C to the five party slots at 28921.
//! TWIN CLAW: EnemyAttack_Blauzen ($13/$15, 19227-19228) sends every id but
//! 0 and $0B to its claw arm (23362-23389). The arm writes $C4 to one slot and
//! overwrites it with $C8 (23368, 23370), so two $C8 objects (28682) run the
//! same code from the same frame: both write #$C to $38 at 28719 in one frame,
//! and the fighter runs routine $C once. No call.
//! MICROMISSL: LifeDeletr (EnemyAttack_LifeDeletr, $1A, 19233), arm
//! .micromissl (23161-23162: $FFFF), object $80C (52229), which loads $810
//! (loc_27094, 52071): eight loc_270F0 calls with one UpdateRNGSeed2 each
//! (52107; two on each of $810's frames 1, 9, 19 and 29), all before $80C's
//! state $C jumps to loc_24A9E (48496). DragerDuel and JurafaDuel
//! (EnemyAttack_BalDuel, $1C/$1D, 19235-19236): loc_10AD2 (23325, $FFFF)
//! loads two $E8 (loc_378FC, 72007), whose state 5 writes the five slots at
//! 72120 in one frame. No call.
//! FLAMLAUNCH: BalDuel's `bgt.s loc_10B06` (23319, 23337) loads $E4
//! (loc_37704, 71820), not the zero arm's $E0: one request on $38 at 71974.
//! RAIL-GUN: EnemyAttack_GunnerBit ($02, 19209, 23604-23617) reads no ability
//! and loads $50 (30361), one request on $38 at 30418. No call.
//! SUPERSONIC (61 BlindHeads): EnemyAttack_StoneHeads tests $21 and $22
//! (22370, 22389); $23 falls to loc_FE9E (22408), $FFFF (22413), objects
//! $234/$23C/$238 (31041, 31208, 31159); $234 state 4 writes the five slots
//! at 31146. No call.
//! BLADESHINE: EnemyAttack_TwinArms ($57/$58, 19294-19295), arm 21456 ($FFFF),
//! $364 (loc_237AA, 47129): five slots at 47237. HAKEN BOLT: arm 21469,
//! $36C (loc_23544, 46959): one request on $38 at 47032. No call.
//! RAY-SPEAR: EnemyAttack_Centaur ($61/$62, 19304-19305), arm 21361, $3A4
//! (loc_220B8, 45533) -> loc_24B20 (45558, 48529). THROWLANCR: loc_F03C
//! (21377, $FFFF), $3A8 (loc_21ED2, 45414) -> loc_24BB6 (45439, 48575).
//! GIFOI: 101 DarkWitch (EnemyAttack_TechUser, $65), loc_ED7A (21193), $3B8
//! (loc_21960, 45087): TechUser's prelude loc_21C30 tests the caster's seal
//! at 45257; unsealed, one request on $38 at 45310. 124 LeFawGan
//! (EnemyAttack_DElmLars, $7C), loc_DF74 (20282), $7B4 (loc_2900E, 54263):
//! the loc_291D2 prelude's seal test at 54387, then loc_24A6C (48474).
//! TANDIL: the same routine's loc_DECE (20240, $FFFF), $7AC (loc_29306,
//! 54450) -> loc_24BB6. No seal test, so a sealed GiLeFarg - whose arm 12
//! fires *because* it is sealed - still lands it.
//! TANDLE: EnemyAttack_ChaosSorcr ($6F-$71, 19318-19320) has no $52 compare:
//! the else arm loc_E9CC (20961, $FFFF) loads $73C (loc_2B23C, 56594), whose
//! state 8 is loc_2C03A (five slots at 57589). LEGEON: loc_E978 (20942,
//! $FFFF), $734 (loc_2B5FA, 56873); retail (revision 1) state 4 is the same
//! loc_2C03A - the revision-0 arm at 56959 and Rune's BattleObj_Legeon are
//! not on this chain. No call, no seal test on either.
//! LGHTBREATH: EnemyAttack_GyLaguiah's else arm loc_E35E (20560), $788
//! (loc_29DC6, 55162) -> loc_24A6C. No call.
//! STAR DUST: EnemyAttack_Acacia ($69, 19312), arm 21109 ($FFFF), $3EC
//! (loc_20B96, 44101): five slots at 44267. ShadMirage is in no US formation.
use super::super::{DamageClass, ObjectDraws};
use super::DamageRoute;

/// One row with no seal test.
const fn row(enemy_id: u16, ability: u8, class: DamageClass, draws: ObjectDraws) -> DamageRoute {
    DamageRoute {
        enemy_id,
        ability,
        class,
        draws,
        sealable: false,
    }
}

const fn single(enemy_id: u16, ability: u8) -> DamageRoute {
    row(enemy_id, ability, DamageClass::Single, ObjectDraws::None)
}

const fn all_party(enemy_id: u16, ability: u8) -> DamageRoute {
    row(enemy_id, ability, DamageClass::AllParty, ObjectDraws::None)
}

/// A GIFOI arm: one request, behind its prelude's seal test.
const fn gifoi(enemy_id: u16) -> DamageRoute {
    DamageRoute {
        sealable: true,
        ..single(enemy_id, 0x48)
    }
}

pub(super) const ROUTES: &[DamageRoute] = &[
    single(2, 0x03),
    single(8, 0x05),
    row(18, 0x09, DamageClass::AllParty, ObjectDraws::MotorCannon),
    single(20, 0x0A),
    single(21, 0x0A),
    row(26, 0x0E, DamageClass::AllParty, ObjectDraws::MicroMissile),
    all_party(28, 0x0E),
    all_party(29, 0x0E),
    single(29, 0x0F),
    all_party(61, 0x23),
    all_party(87, 0x3C),
    all_party(88, 0x3C),
    single(87, 0x3D),
    single(88, 0x3D),
    single(97, 0x42),
    single(98, 0x42),
    all_party(98, 0x43),
    gifoi(101),
    gifoi(124),
    all_party(124, 0x5D),
    all_party(125, 0x5D),
    all_party(111, 0x52),
    all_party(112, 0x52),
    all_party(113, 0x52),
    all_party(113, 0x55),
    single(119, 0x58),
    all_party(105, 0x4A),
];
