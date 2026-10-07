//! The Air Castle stretch's damage arms (lane A5), from the worms and sabres of
//! group 52 to the three fixed battles behind Xe-A-Thoul's room.
//!
//! GRA: EnemyAttack_DimensWorm (EnemyAttackOffs $49/$4A, ps4.asm:19280-
//! 19281) sends every id but 0 and $30 to loc_F7BE (21909-21931), which
//! clears Current_Target_Index and loads $2E0/$2DC/$2D8. $2DC's state 4
//! (loc_1C920, 39010) writes #$C to the five Obj_Fighters slots at 39021
//! once $2E0 has set $FFFFEE80 to $A; its sparks draw (`super::super::sparks`).
//! GIGRA ($32, 74 OuterBeast) reaches the same else arm and the same three
//! objects; the only reader of the id on the way is a palette choice
//! (`cmpi.w #$31, $24(a1)`, 39168-39173), so its request and draws are GRA's.
//! AIRSLASH: EnemyAttack_ShadowSabr ($46-$48, 19277-19279), arm loc_F85E
//! (21957-21965), clears the target and loads $29C/$2A0. $2A0's state 8
//! (loc_1DC0C, 40391) writes #$C to the five slots at 40404. No call.
//! GIZAN: EnemyAttack_DElmLars ($7A-$7D, 19329-19332), arm loc_DF22
//! (20263-20281), clears the target and loads $7B0 (loc_2912C, 54326). At
//! frame $26 it tests the caster's StatusTechSealed (54385-54388): set, it
//! waits to $30 and ends at loc_29288 (54413) with no request; clear, $3E0
//! runs and state 8 jumps to the five-slot loc_24A9E (54342, 48496).
//! The other two GIZAN carriers have their own objects and the same shape.
//! 77 TechPlant: EnemyAttack_FlattrPlnt's fall-through loc_F722
//! (21869-21891) loads BattleObj_EnemyGizan ($304, 38024), whose frame $24
//! tests the seal (38045-38051: set, it clears itself before any request) and
//! whose state 4 writes #$C to the five slots at 38142. 101 DarkWitch:
//! EnemyAttack_TechUser ($65, 19308), arm loc_ED28 (21174-21192), loads $3DC
//! (loc_21176, 44547); its state 0 is TechUser's shared loc_21C30, whose
//! frame $F seal test (45255-45258) sends a sealed caster to state 8 with bit
//! 7 set, where loc_21674 ends the turn (44881, 44908-44914); unsealed, state
//! 8 writes the five slots at 44895. Neither calls UpdateRNGSeed2.
//! THNDRBLAST: EnemyAttack_DElmLars's fall-through loc_E052 (20336-20346) clears
//! the target and turns the attack object into $7C0 (loc_28A76, 53888).
//! Its state 4 (loc_28B7E) sets StatusParalyzed on Fighter_Enemy_1..3's stats
//! (53955-53961) and state 8 jumps to the five-slot loc_24BB6 (53903, 48575).
//! THNDHALBRT: EnemyAttack_Lashiec ($80, 19335), first arm (20117-20135),
//! loads $7E4 (loc_281F6, 53309); state $10 jumps to loc_24A9E (53324).
//! ANOTHRGATE: Lashiec's fall-through loc_DDD6 (20175-20191) loads $7F4
//! (loc_27B02, 52835); state 4 loads $7F8 (loc_27C80, 52942), whose first 59
//! frames call UpdateRNGSeed2 four times each (52951-52979) before it raises
//! $FFFFEE80 (52957) and state $10 jumps to loc_24A9E (52852).
//! SHDWBREATH: EnemyAttack_DarkForce2 ($83, 19338), arm loc_DAA8
//! (19978-19995), loads $844 (loc_31378, 63744); state 8 jumps to the
//! single-target loc_24B20 (63758, 48529). No call.
//! LIGHTSHOWR: arm loc_DAEC (19996-20012) clears the target and loads $848
//! (loc_30FD2, 63467); state 8 jumps to loc_24A9E (63482). Its ten $84C
//! children (loc_310CA, 63537-63559) draw nothing.
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

/// A GIZAN arm: all party, no call, and a seal test in the object.
const fn gizan(enemy_id: u16) -> DamageRoute {
    DamageRoute {
        sealable: true,
        ..row(enemy_id, 0x35, DamageClass::AllParty, ObjectDraws::None)
    }
}

pub(super) const ROUTES: &[DamageRoute] = &[
    row(73, 0x31, DamageClass::AllParty, ObjectDraws::GraSparks),
    row(74, 0x31, DamageClass::AllParty, ObjectDraws::GraSparks),
    row(74, 0x32, DamageClass::AllParty, ObjectDraws::GraSparks),
    row(70, 0x2C, DamageClass::AllParty, ObjectDraws::None),
    row(71, 0x2C, DamageClass::AllParty, ObjectDraws::None),
    row(72, 0x2C, DamageClass::AllParty, ObjectDraws::None),
    gizan(77),
    gizan(101),
    gizan(122),
    gizan(123),
    gizan(124),
    gizan(125),
    row(
        123,
        0x5C,
        DamageClass::AllPartyParalyzingTrio,
        ObjectDraws::None,
    ),
    row(128, 0x5F, DamageClass::AllParty, ObjectDraws::None),
    row(128, 0x61, DamageClass::AllParty, ObjectDraws::AnotherGate),
    row(131, 0x64, DamageClass::Single, ObjectDraws::None),
    row(131, 0x65, DamageClass::AllParty, ObjectDraws::None),
];
