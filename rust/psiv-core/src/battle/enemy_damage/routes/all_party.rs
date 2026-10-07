//! The two `AllParty` routes the Motavia sweep proved first: `$08` SPIRAL BLD
//! on 15 Fanbite and `$38` EARTHQUAKE on 80 SandWorm and 149 KingRappy.
//!
//! All three request one hit per party slot from a five-slot loop over
//! `Obj_Fighters` — Fanbite's in its own object, SandWorm's inline in
//! `loc_243C8`, KingRappy's through the shared tail `loc_24BB6` — and SandWorm's
//! is the one chain in the whole table whose object reads `UpdateRNGSeed2`.

use super::super::{DamageClass, EARTHQUAKE, ObjectDraws, SPIRAL_BLD};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    // 15 Fanbite, `$08` SPIRAL BLD — the first all-party route.
    // `EnemyAttackOffs` `$0F` (`ps4.asm:19222`) → `EnemyAttack_Locusta`
    // (`ps4.asm:23472`). The arm has no ability-id test: it loads the three
    // approach objects (`$98`/`$9C`/`$A4`), then `tst.w ability(a4)` at line
    // 23486 picks the nonzero-ability body, which clears
    // `Current_Target_Index` (line 23488), loads object `$A0` into the second
    // object bank (`ps4.asm:26770`) = `BattleObj_LocustaSpiralBld`
    // (`ps4.asm:29175`) and copies the stored target pointer into its `$38`
    // (line 23491). Fanbite's regular list is `$08` in slots 7-8 and zeros
    // elsewhere, so every nonzero roll runs this body.
    //
    // The object's state 4 (`loc_1468A`, `ps4.asm:29259`) walks in until
    // `$2C(a4) >= $1BF` and then ORs the five party slots' `$1C` timers
    // (lines 29266-29274); only when all of them read zero does it write
    // `#$C` to the five slots from `$FF4400` (lines 29275-29280) and set
    // `($FFFF416C)`. While it walks, `loc_146F2` also flinches each live
    // member it passes (`$1C = $C`, routine 5, line 14756) in the `loc_147A0`
    // order, which is what the `$1C` gate is waiting for; the flinch is not a
    // damage request.
    DamageRoute {
        enemy_id: 15,
        ability: SPIRAL_BLD,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 80 SandWorm, `$38` EARTHQUAKE. `EnemyAttackOffs` `$50`
    // (`ps4.asm:19287`) → `EnemyAttack_SandWorm` (`ps4.asm:21658`); `loc_F4D4`
    // (`ps4.asm:21710`) tests `$38` (line 21711), clears
    // `Current_Target_Index` (line 21713) and writes object `$330` into the
    // attack object itself (line 21718) — `BattleObjsGroup5Ptrs` line 43704 =
    // `BattleObj_Earthquake` (`ps4.asm:47884`). SandWorm lists `$38` in slots
    // 6-8 only, and `loc_F48A` owns `$37` and `loc_F4FA` the else arm, so this
    // body is `$38`'s alone.
    //
    // The object's state table (`loc_2427A`, `ps4.asm:47897`) reaches
    // `loc_243C8` (line 47992) for the request: it writes `#$C` to the five
    // slots from `Obj_Fighters` (lines 47995-48000) and sets `($FFFF416C)`,
    // then waits for the flag to clear (line 48003) before its follow-through
    // phase. Unlike the two tails and the Fanbite object it does not
    // test the `$1C` timers, and `Enemy_Attack` cleared them for all five
    // slots at line 19143 anyway.
    //
    // Before that request it shakes the screen: state 4 (`loc_24356`, line
    // 47957) is entered from state 0 with `$11(a4) = $3C` and `$10(a4) = 0`
    // (lines 47943-47945), and `loc_2438E` (line 47972) draws twice on every
    // fourth frame of the sixty — 28 calls, all of them before the `#$C`
    // writes. That is the route's `draws`, and the only one in the table.
    DamageRoute {
        enemy_id: 80,
        ability: EARTHQUAKE,
        class: DamageClass::AllParty,
        draws: ObjectDraws::EarthquakeShake,
        sealable: false,
    },
    // 149 KingRappy, `$38` EARTHQUAKE. `EnemyAttackOffs` `$95`
    // (`ps4.asm:19356`) → `EnemyAttack_KingRappy` (`ps4.asm:19596`), whose
    // nonzero arm `loc_D516` (line 19608) clears `Current_Target_Index`
    // (line 19609) and writes object `$904` (line 19610) —
    // `BattleObjsGroup10Ptrs` line 66419 = `BattleObj_KingRappyEarthquake`
    // (`ps4.asm:67513`). KingRappy's regular list is `$38` in slots 3-4 and
    // zeros elsewhere, so every nonzero roll takes that arm.
    //
    // The object's state table (`loc_34428`, `ps4.asm:67523`) sends states 0,
    // 4 and 8 through its own wind-up — which loads sound object `$8EC`
    // (line 67541, with `SFXID_Slasher` at line 67544), writes
    // `SFXID_GraveOpening` (line 67567) and clears the party's `$1C` timers
    // (lines 67603-67607) — and state `$C` straight to `jmp (loc_24BB6).l`
    // (line 67527), the shared all-party tail at `ps4.asm:48562`: its `$1C`
    // gate at lines 48565-48571, the five `#$C` writes at lines 48572-48577 and
    // `($FFFF416C)` at line 48578.
    //
    // KingRappy's object shakes the camera too, but from a table rather than
    // the generator: state 4 (`loc_344AC`, line 67554) walks a fixed byte list
    // of eight camera offsets (`loc_3451C`, line 67590) and calls nothing, so
    // this route takes no call the damage runs do not. It is the control for
    // SandWorm's above: same ability, same class, one object that draws and one
    // that does not.
    DamageRoute {
        enemy_id: 149,
        ability: EARTHQUAKE,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
];
