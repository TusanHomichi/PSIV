//! The Zio-arc pairs: `$47` ZAN, `$56` FORCEFLASH, `$4D` CORRSION, and the
//! three rolled arms of 140 Zio2 behind its phase counter.
//!
//! These are the abilities the campaign runner meets in Zio's fort and behind
//! it — Juza's own draws (`docs/campaign/RUNNER_LOG.md` H16), the Ripper and
//! Haunt rooms around him, and Nurvus's TechMasters and Greneris — and all
//! three resolve as **one hit per party slot**: every arm clears
//! `Current_Target_Index` before it loads its object, and every object either
//! runs the five-slot loop over `Obj_Fighters` itself or exits through the
//! shared all-party tail `loc_24BB6` (`ps4.asm:48562`).
//!
//! `EnemySkillData` bytes:
//!
//! | ability | record | effect · stat · target · power · resistance · element |
//! |---|---|---|
//! | `$47` ZAN | `01 82 09 10 07 01 00 00` at `$28359C` | `$01` · `$82`→mental · tgt 9 · 16 · `$07` magic defense · `1` physical |
//! | `$56` FORCEFLASH | `01 02 09 24 07 02 00 00` at `$283614` | `$01` · `$02` mental · tgt 9 · 36 · `$07` magic defense · `2` energy |
//! | `$4D` CORRSION | `01 02 09 40 07 01 00 00` at `$2835CC` | `$01` · `$02` mental · tgt 9 · 64 · `$07` magic defense · `1` physical |
//!
//! All three carry effect `$01` (`AbilityEffect_None`, `ps4.asm:9092`), which is
//! what lets the request be the whole turn; target nibble 9
//! (`AbilityRange_MultiChars`) only multiplies that no-op handler.
//!
//! 140 Zio2's rows are the last three: its routine runs a scripted first action
//! (`zio::step`, the Magic Barrier) and dispatches the rolled ability from the
//! second on, so a row here is only ever reached with the phase counter
//! `$FFFFEE98` past zero. 114 Juza's own `EnemyAttack_Juza` arms are the same
//! ones 115 Greneris and 116 Radhin dispatch, and his event battle is reachable
//! by the oracle only through the fort's event script, not by `oracle.force`.

use super::super::{BLACK_WAVE2, CORRSION, DamageClass, FORCEFLASH, HEWN, ObjectDraws, ZAN};
use super::DamageRoute;

pub(super) const ROUTES: &[DamageRoute] = &[
    // 100 TechMaster, `$47` ZAN. `EnemyAttackOffs` `$64` (`ps4.asm:19307`) →
    // `EnemyAttack_TechUser` (`ps4.asm:21156`), whose `$47` arm is `loc_EE58`
    // (`ps4.asm:21247`): `cmpi.w #$47, $24(a4)` at line 21248, then
    // `move.w #$FFFF, (Current_Target_Index).l` (line 21250) and object `$3D0`
    // (line 21263) = `loc_215D6` (`ps4.asm:44836`) with `$38` still holding the
    // drawn target. The object's phase table `loc_2160E` (`ps4.asm:44851`) sends
    // state 8 to `loc_21674` (`ps4.asm:44880`): the five-slot loop's `move.w
    // #$C, $2(a3)` is at line 44895, behind the same all-slots `$1C` gate the
    // other all-party routes use.
    DamageRoute {
        enemy_id: 100,
        ability: ZAN,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 114 Juza, `$47` ZAN. `EnemyAttackOffs` `$72` (`ps4.asm:19321`) →
    // `EnemyAttack_Juza` (`ps4.asm:20575`); the `$47` test is at line 20612 in
    // `loc_E428` (`ps4.asm:20611`), which clears `Current_Target_Index` (line
    // 20614) and writes object `$748` (line 20627) = `loc_2AF18`
    // (`ps4.asm:56372`). Its phase table `loc_2AF4C` (`ps4.asm:56385`) sends
    // state 8 to `loc_2AFB6` (`ps4.asm:56413`) — `loc_2AF9E` (`ps4.asm:56404`)
    // writes `$14(a4) = 8` at line 56409 once `($FFFFEE80)` clears — and the
    // five-slot request is at line 56426. Juza's battle is event battle 3
    // (`generated/formations.json`'s boss block), which no map's group reaches.
    DamageRoute {
        enemy_id: 114,
        ability: ZAN,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 114 Juza, `$56` FORCEFLASH. The carrier routine tests `$44`, `$40`, `$47`,
    // `$2F`, `$2A`, `$27`, `$26`, `$57`, `$28`, `$29`, `$2D` and `$49`, and the
    // else arm `loc_E6FE` (`ps4.asm:20783`) is what a `$56` draw takes: it
    // clears `Current_Target_Index` (line 20789) and writes object `$778` (line
    // 20798) = `loc_2A300` (`ps4.asm:55527`). Its phase table `loc_2A330`
    // (`ps4.asm:55539`) sends state 8 to the same `loc_2AFB6` tail — `loc_2A47E`
    // (`ps4.asm:55631`) writes `$14(a4) = 8` at line 55634 — so the request is
    // again the five-slot `move.w #$C` at line 56426. Juza's regular list is
    // `$40`/`$44`/`$47`/`$56` twice each, so this arm is half his draws.
    DamageRoute {
        enemy_id: 114,
        ability: FORCEFLASH,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 115 Greneris, `$56` FORCEFLASH: `EnemyAttackOffs` `$73` (`ps4.asm:19322`)
    // → the same `EnemyAttack_Juza`, the same else arm and the same request.
    // Greneris's list is `$28`/`$29`/`$2A`/`$2F`/`$2F`/`$57`/`$56`/`$56`, all of
    // it either tested by the routine or this arm.
    DamageRoute {
        enemy_id: 115,
        ability: FORCEFLASH,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 116 Radhin, `$56` FORCEFLASH: `EnemyAttackOffs` `$74` (`ps4.asm:19323`) →
    // the same routine, arm and request (`$26`/`$27`/`$29`/`$2D` tested, `$56`
    // the else arm).
    DamageRoute {
        enemy_id: 116,
        ability: FORCEFLASH,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: true,
    },
    // 106 Haunt, `$4D` CORRSION. `EnemyAttackOffs` `$6A` (`ps4.asm:19313`) →
    // `EnemyAttack_Haunt` (`ps4.asm:21005`), whose `$4D` arm `loc_EB04`
    // (`ps4.asm:21039`) clears `Current_Target_Index` (line 21042) and writes
    // object `$704` (line 21054) = `loc_2C98A` (`ps4.asm:58230`, the `$704`
    // entry of `BattleObjsGroup8Ptrs` is line 51951). Its phase table
    // `loc_2C9BC` (`ps4.asm:58242`) sends the state `loc_2CAFC`
    // (`ps4.asm:58328`) advances to straight to `jmp (loc_24BB6).l` (line
    // 58245): one `move.w #$C, $2(a3)` per party slot at line 48575, the shared
    // all-party tail. The flinches it writes on the way (`move.w #5, $2(a2)`,
    // line 58295) are not requests.
    DamageRoute {
        enemy_id: 106,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 107 Spector, `$4D` CORRSION: `EnemyAttackOffs` `$6B` (`ps4.asm:19314`) →
    // the same `EnemyAttack_Haunt` arm, object and tail.
    DamageRoute {
        enemy_id: 107,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 111 ChaosSorcr, `$4D` CORRSION. `EnemyAttackOffs` `$6F` (`ps4.asm:19318`)
    // → `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`), arm `loc_E7EC`
    // (`ps4.asm:20849`): `Current_Target_Index` cleared at line 20852, object
    // `$720` written at line 20865 = `loc_2BED2` (`ps4.asm:57485`). Its phase
    // table `loc_2BEEE` (`ps4.asm:57494`) sends state 4 — which `loc_2C026`
    // (`ps4.asm:57566`) writes at line 57570 — to `loc_2C03A`
    // (`ps4.asm:57574`), whose own five-slot loop makes the one request per slot
    // at line 57589. Unlike the `loc_24BB6` rows this object writes its request
    // itself, from `lea (Obj_Fighters).l, a3` (line 57579) rather than a jump.
    DamageRoute {
        enemy_id: 111,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 112 Illusionst, `$4D` CORRSION: `EnemyAttackOffs` `$70` (`ps4.asm:19319`)
    // → the same `EnemyAttack_ChaosSorcr` arm, object and loop.
    DamageRoute {
        enemy_id: 112,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 113 ImagioMage, `$4D` CORRSION: `EnemyAttackOffs` `$71` (`ps4.asm:19320`)
    // → the same routine, arm and loop.
    DamageRoute {
        enemy_id: 113,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 132 DarkForce3, `$4D` CORRSION. `EnemyAttackOffs` `$84` (`ps4.asm:19339`)
    // → `EnemyAttack_DarkForce3` (`ps4.asm:19907`), arm `loc_D9FE`
    // (`ps4.asm:19934`): `Current_Target_Index` cleared at line 19937, object
    // `$85C` written at line 19949 = `loc_308FA` (`ps4.asm:62998`). Its phase
    // table `loc_30946` (`ps4.asm:63015`) sends state 8 to `jmp (loc_24BB6).l`
    // (line 63018) — the same one request per slot at line 48575. This routine
    // has no scripted-battle gate (unlike DarkForce2's `$4C`, `ps4.asm:19971`).
    DamageRoute {
        enemy_id: 132,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 140 Zio2, `$4D` CORRSION. `EnemyAttackOffs` `$8C` → `EnemyAttack_Zio2`
    // (`ps4.asm:19519`): with the phase counter past zero (the first action is
    // the scripted Magic Barrier, `zio::step`) it tests the rolled `$24(a4)` in
    // turn — `tst.w` at line 19532 (zero is object `$920`), `cmpi.w #$6C` at
    // line 19545 — and everything else reaches `loc_D458` (line 19556), which
    // clears `Current_Target_Index` to `$FFFF` (line 19557) before it tests
    // `$4D` (line 19562) and writes object `$928` (line 19568) = `loc_33646`
    // (`ps4.asm:66537`; `BattleObjsGroup10Ptrs` line 66428). Its phase table
    // `loc_336A0` (line 66560) sends state 16 to `jmp (loc_24BB6).l` (line
    // 66565): the shared all-party tail, one `move.w #$C, $2(a3)` per party
    // slot at line 48575. The flinches on the way are not requests, and no
    // call in the chain reaches `UpdateRNGSeed2`.
    DamageRoute {
        enemy_id: 140,
        ability: CORRSION,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 140 Zio2, `$4F` HEWN: the same `loc_D458` clears the target index, and
    // `$4F` fails the `$4D` test, so it takes `loc_D498` (line 19570): object
    // `$930` (line 19575) = `loc_335A6` (`ps4.asm:66498`; `BattleObjsGroup10Ptrs`
    // line 66430). Its phase table `loc_33630` (line 66531) differs from
    // CORRSION's only at state 12, `loc_2B7EE` (`ps4.asm:57045`) in place of
    // `loc_33750`: the same flinch-and-effect state, which advances the state
    // by four through `loc_25232` once `($FFFFEE85)` clears (lines 57098-57101
    // of `loc_2B8B4`) into state 16, again `jmp (loc_24BB6).l` (line 66536).
    // The same five-slot request, again with no draw.
    DamageRoute {
        enemy_id: 140,
        ability: HEWN,
        class: DamageClass::AllParty,
        draws: ObjectDraws::None,
        sealable: false,
    },
    // 140 Zio2, `$6C` BLACK WAVE2, the weak Black Wave. `loc_D426`
    // (`ps4.asm:19544`) tests `$6C` (line 19545) and writes object `$91C`
    // (line 19554) = `BattleObj_BlackWave2` (`ps4.asm:66922`; the comment above
    // it reads "the weak Black Wave (Psycho-wanded Zio)"). It does **not**
    // clear `Current_Target_Index`, so the object keeps the drawn target in
    // `$38`. Its phase table `loc_33BB6` (line 66934) reaches `loc_33BD0`
    // (line 66941) at state 12, which flinches that target (`move.w #5, $2(a3)`,
    // line 66945) and jumps into `loc_24A6C` (line 66948) with the state at 16,
    // where the table's own `jmp (loc_24A6C).l` (line 66939) keeps entering it
    // until the target's `$1C` timer clears: the single-target tail
    // (`ps4.asm:48468`) makes its one request at line 48474 and, once
    // `($FFFF416C)` clears, advances the state by six to the end entry
    // (`loc_33BF6`, line 66949). No call reaches `UpdateRNGSeed2`.
    DamageRoute {
        enemy_id: 140,
        ability: BLACK_WAVE2,
        class: DamageClass::Single,
        draws: ObjectDraws::None,
        sealable: false,
    },
];
