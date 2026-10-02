//! `$21` FIREBREATH: the six `EnemyAttack_*` arms that load a fire-breath
//! object, five of them proven here.
//!
//! `EnemySkillData` `$21` is `01 01 08 20 07 03 00 00` at `$28346C`: effect
//! `$01` (`AbilityEffect_None`, `ps4.asm:9092`), byte 1 `$01` (strength), target
//! nibble 8 (`AbilityRange_Single`), hit chance `$20`, resistance `$07` (magic
//! defense) and element `3` (fire). Every arm below keeps
//! `Current_Target_Index` — no `$21` carrier clears it — and every chain makes
//! exactly one `move.w #$C` request against the `$38` pointer the arm stored, so
//! all five rows are [`DamageClass::Single`] with no [`ObjectDraws`].
//!
//! **58 FlameNewt** and **59 StoneHeads** reach the ability through their own
//! routines' *else* arms (`EnemyAttack_SandNewt` tests only `$24`,
//! `EnemyAttack_StoneHeads` tests `$00`/`$21`/`$22`), **83 Ripper** and
//! **84 BladeRight** share `EnemyAttack_Ripper`'s `$21` arm, and
//! **117 GyLaguiah** takes `EnemyAttack_GyLaguiah`'s. The one carrier left out
//! is 133 ProfoundDarkness1: its routine is gated (see below).
//!
//! 83 Ripper is the carrier the campaign runner meets first — it appears on
//! fourteen of the Zio Fort's and Ladea Tower's maps (`docs/battle/
//! ENEMY_ABILITIES.md` §4) with a regular list of five zeros and three `$21`
//! slots, so every roll it makes is either FIREBREATH or a plain attack.

use super::super::{DamageClass, FIREBREATH, ObjectDraws};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    // 58 FlameNewt, `$21` FIREBREATH. `EnemyAttackOffs` `$3A`
    // (`ps4.asm:19265`) → `EnemyAttack_SandNewt` (`ps4.asm:22267`), which
    // tests `$24` first (`bne.s loc_FD46`, line 22269) and so sends `$21` to
    // the else arm `loc_FD46` (`ps4.asm:22307`). That arm writes object `$248`
    // (line 22314) = `BattleObj_FireBreath` (`ps4.asm:42121`; the `$248` entry
    // of `BattleObjsGroup4Ptrs` is line 37721) with the drawn target still in
    // `$38`. Its phase table `loc_1F49E` (`ps4.asm:42135`) has three entries,
    // and its third — reached when the charge counter `($FFFFEE83)` that
    // `loc_1F4C4` counts down (line 42146) empties — writes `$14(a4) = 8`
    // (line 42149), which is `loc_1F3C6` (`ps4.asm:42072`). That handler's
    // `loc_1F40E` (`ps4.asm:42091`) loads the stored target
    // (`movea.l $38(a4), a3`, line 42094), waits on the target's `$1C` timer and
    // writes one `move.w #$C, $2(a3)` at line 42097. The flinches on the way
    // (`move.w #5, $2(a3)`, lines 42064 and 42174) are not requests, and
    // nothing in the chain calls `UpdateRNGSeed2`.
    DamageRoute {
        enemy_id: 58,
        ability: FIREBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // 59 StoneHeads, `$21` FIREBREATH: `EnemyAttackOffs` `$3B`
    // (`ps4.asm:19266`) → `EnemyAttack_StoneHeads` (`ps4.asm:22350`), whose
    // `$21` test (`cmpi.w #$21`, line 22371) takes `loc_FE06` (line 22370).
    // The arm turns the attack object into `$228` (line 22381) = `loc_16366`
    // (`ps4.asm:31333`, `BattleObjsGroup3Ptrs` line 31032), keeps the target in
    // `$38`, and spawns child `$22C` (line 22387) = `loc_16406`
    // (`ps4.asm:31380`) with the target pointer copied into the child's `$38`
    // (`move.l a0, $38(a1)`, line 22386). The parent only flinches its target
    // (`move.w #5, $2(a3)`, line 31377); the child's phase table `loc_16428`
    // (`ps4.asm:31390`) sends state 4 to `loc_1650E` (`ps4.asm:31447`), the one
    // request, at line 31453, behind the `$1C` gate. No `UpdateRNGSeed2` call
    // is in either object.
    DamageRoute {
        enemy_id: 59,
        ability: FIREBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // 83 Ripper, `$21` FIREBREATH: `EnemyAttackOffs` `$53` (`ps4.asm:19290`) →
    // `EnemyAttack_Ripper` (`ps4.asm:21587`), `$21` test at line 21604 taking
    // `loc_F372` (line 21603), which writes `$34C` (line 21617) =
    // `loc_23D0A` (`ps4.asm:47507`) with `$38`. Its phase table `loc_23D32`
    // (`ps4.asm:47518`) sends state 4 to `loc_23E46` (`ps4.asm:47592`): the one
    // request at line 47600, against `$38(a4)` and behind the same `$1C` gate.
    DamageRoute {
        enemy_id: 83,
        ability: FIREBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // 84 BladeRight, `$21` FIREBREATH: `EnemyAttackOffs` `$54` (`ps4.asm:19291`)
    // → the same `EnemyAttack_Ripper` arm, object and request. BladeRight's
    // regular list is the same five zeros and three `$21` slots; its conditional
    // `$3A` COMBINE3 is a separate row (§3 of the census) and not this one.
    DamageRoute {
        enemy_id: 84,
        ability: FIREBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
    // 117 GyLaguiah, `$21` FIREBREATH: `EnemyAttackOffs` `$75`
    // (`ps4.asm:19324`) → `EnemyAttack_GyLaguiah` (`ps4.asm:20519`), `$21` test
    // at line 20528 taking `loc_E2DC` (line 20527), which writes `$780` (line
    // 20541) = `loc_2A01A` (`ps4.asm:55324`; `BattleObjsGroup8Ptrs` line 51982)
    // with `$38`. That object's phase table `loc_2A042` (`ps4.asm:55335`) is
    // indexed by **byte offset** — its entry at 4 is `jmp (loc_24A6C).l` (line
    // 55337) — and `loc_2A114` (`ps4.asm:55384`) writes `$14(a4) = 4` (line
    // 55388) once the child it spawned finishes, so the single-target tail
    // (`ps4.asm:48468`) makes the route's one request at line 48474. The
    // tail's own `$1C` gate and `($FFFF416C)` wait are shared with every other
    // `loc_24A6C` row.
    DamageRoute {
        enemy_id: 117,
        ability: FIREBREATH,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
    },
];

// **133 ProfoundDarkness1 is not a row.** `EnemyAttack_ProfoundDarkness1`
// (`ps4.asm:19823`) tests the scripted-battle flag `$FFFFEE87` first (line
// 19824) and, when it is set, clears `$24(a4)` and loads object `$864` instead
// of dispatching the rolled id at all. `EnemyInit_Lashiec` (`ps4.asm:18114`)
// clears that flag and battle object `$7EC` (`ps4.asm:53102`) sets it, and this
// port models no battle objects, so it cannot tell which arm the cartridge
// would take: the `$21` arm below the gate (`loc_D8BE`, `ps4.asm:19849`, object
// `$870` = `loc_30022` at line 62419, whose child `$874` = `loc_30092` at line
// 62449 reaches the same `loc_24A6C` request through its own phase table's
// entry at line 62466) is therefore left for a lane that models the flag.
