//! The traced `(enemy, ability)` arms `super::resolve_effect_skill` runs: the
//! route table, split from `enemy_effect.rs` (the 1,000-line rule).

use super::{Calls, Guard, Handler, Pick, Route};

/// Every `(enemy, ability)` arm this module resolves - the ones a capture saw run
/// (`docs/oracle/BATTLE_ORACLE_ARC.md`, `docs/oracle/BATTLE_ORACLE_ZELAN.md`).
/// Radhin's SEALS and DEBAN (`EnemyAttack_Juza` arms at 20729 and `loc_E666`,
/// 20747) are the same reads as Greneris's and are deliberately absent: no
/// capture of Radhin has a prefix of abilities the port runs.
///
/// | pair | routine and arm | object | handler |
/// |---|---|---|---|
/// | 31 CarrionCr `$10` THREAD | `EnemyAttack_Crawler` (`ps4.asm:23081`), arm `loc_1084A` (23113) | `BattleObj_Thread` (`ps4.asm:36186`) | `$06`, range 8 |
/// | 32 Caterpillr `$11` POISON | the same routine, arm `loc_10836` (23107) | `BattleObj_Poison` (`ps4.asm:36279`) | `$1B`, range 8 |
/// | 57 Mistralgec `$24` POISONMIST | `EnemyAttack_SandNewt` (`ps4.asm:22267`), arm `loc_FD06` (22287) | object `$244`, `BattleObj_PoisonMist` (`ps4.asm:42202`): one call (42293) | `$1B`, range 8 |
/// | 63 GerotLux `$25` SLEEP GAS | `EnemyAttack_AbeFrog` (`ps4.asm:22246`), arm `loc_FCA0` (22257) | object `$250`, `BattleObj_SleepGas` (`ps4.asm:41714`) -> `loc_24D76` (`ps4.asm:48702`): one call (48706), then `bset #3` on every landed slot | `$07`, range 9 |
/// | 111 ChaosSorcr, 138 ChaosSorcr2 `$4B` SHADOWBIND | `EnemyAttack_ChaosSorcr` (`ps4.asm:20816`), arm `loc_E83E` (20868) | object `$724`, `loc_2BCEA` (`ps4.asm:57360`): one call (57411) at frame `$F` | `$06`, range 9 |
/// | 76 FlyScreamr `$34` VOICE | `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`), arm at 21803 | `BattleObj_Voice` (`ps4.asm:38465`) | `$07`, range 9 |
/// | 19 Blauzen, 21 Goldine `$0B` STASISBALL | `EnemyAttack_Blauzen` (`ps4.asm:23346`), `.stasisball` (23404) | `BattleObj_BlauzenStasisBall` (`ps4.asm:28127`) and four children | `$1C`, range 8 |
/// | 26 LifeDeletr `$0B` | `EnemyAttack_LifeDeletr` (`ps4.asm:23124`), `.ability` (23140) | `BattleObj_LifeDeletrStasisBall` (`ps4.asm:26956`) and two children | `$1C`, range 8 |
/// | 115 Greneris `$28` DORAN | `EnemyAttack_Juza` (`ps4.asm:20575`), arm at 20709 | object `$768`, `loc_2A814` (`ps4.asm:55873`) | `$06`, range 9 |
/// | 115 Greneris `$57` GELUN | the same routine, arm at 20700, same tail | `loc_2A814` | `$03`, range 9 |
/// | 115 Greneris `$29` SEALS | arm at 20729 | object `$76C`, `loc_2A74A` (`ps4.asm:55820`) | `$08`, range 9 |
/// | 115 Greneris `$2A` RIMIT | arm 20648 (`loc_E4C4`) | object `$754`, `loc_2ACD6` (`ps4.asm:56212`); its child `$758`, `loc_2AC22` (`ps4.asm:56160`), makes the call and `loc_25074` sets the sleep bit | `$07` + `loc_25074`, range 9 |
/// | 77 TechPlant `$2A` RIMIT | `EnemyAttack_FlattrPlnt`, arm `loc_F65E` (`ps4.asm:21822`) | `BattleObj_EnemyRimit` (`ps4.asm:38366`): one call (38414), `loc_24CFE` sets bit `$FFFFEEA2` = 3 | `$07`, range 9 |
/// | 106 Haunt, 107 Spector `$4C` EVIL EYE | `EnemyAttack_Haunt` (`ps4.asm:21005`), arm `loc_EAC6` (21023) | object `$700`, `loc_2CB1C` (`ps4.asm:58346`): one call (58375), then `bset #3` on the stored target (58495) | `$07`, range 8 |
/// | 115 Greneris, 72 BloodSaber, 88 SoldrFiend `$2F` VOL | arms 20630 (`loc_E47A`), 22005 (`loc_F93C`), 21484 (`loc_F1E2`) | `loc_2AE8E` (`ps4.asm:56333`) and its child `loc_2AE0C` (56296); `loc_1D2F4` (`ps4.asm:39774`); `loc_23216` (`ps4.asm:46736`): one call, then `loc_25048` (`ps4.asm:48916`) kills | `$02`, range 8 |
/// | 48 Siren386, 49 Browren486 `$1D` BARRIER | `EnemyAttack_Warren286` (`ps4.asm:22515`), `loc_100A6` (22559) | object `$1E0`, `loc_1769E` (`ps4.asm:32706`), one call (32771) | `$0B`, range 2, signed caster MDEF guard |
/// | 70 ShadowSabr, 71 FrostSaber, 72 BloodSaber `$2D` DEBAN | `EnemyAttack_ShadowSabr` (`ps4.asm:21933`, entries `$46`-`$48`), arm `loc_F89A` (21966) | object `$2A8`, `loc_1D7D8` (`ps4.asm:40098`): seal test at frame `$14` (40125-40130) | `$0A`, range 2, sealable |
/// | 107 Spector `$4E` DTHSPELL | `EnemyAttack_Haunt`, arm `loc_EB50` (`ps4.asm:21057`) | object `$708`, `loc_2C854` (`ps4.asm:58148`): `loc_250A2` (58181), then the kill at `loc_2C928` (58201-58219) | `$02`, range 8 |
/// | 128 Lashiec `$60` POSESSION | `EnemyAttack_Lashiec`, arm `loc_DD2C` (`ps4.asm:20136`) | object `$7E8`, `loc_2805C` (`ps4.asm:53215`): `loc_250A2` (53268), then `bset #3` on the stored target (53271-53273) | `$07`, range 8 |
/// | 128 Lashiec `$62` REINFORCE | `EnemyAttack_Lashiec`, arm `loc_DD84` (`ps4.asm:20156`) | object `$7EC`, `loc_27ED0` (`ps4.asm:53099`): raises `$FFFFEE86` (53102), then its phase 8 jumps to `loc_24C68` (48613), one call (48617) | `$2B`, range 3 |
/// | 131 DarkForce2 `$4C` EVIL EYE | `EnemyAttack_DarkForce2`, fall-through `loc_DB30` (`ps4.asm:20013`) | object `$850`, `loc_30DC0` (`ps4.asm:63323`): two calls (63417, 63418 via `loc_250A2`), `loc_25074` at 63456 | `$07`, range 8, [`Calls::TwiceLastDecides`] |
pub(super) const ROUTES: &[Route] = &[
    // EnemyAttack_Warren286 -> $1E0 (loc_1769E, ps4.asm:32706), whose
    // loc_17788 calls GetEnemySkillEffectAndRange once (32771), range 2.
    Route {
        enemy: 48,
        ability: 0x1D,
        handler: Handler::MagicDefenseUp,
        range: 2,
        guard: Guard::MagicDefenceNotRaised,
        sealable: false,
        calls: Calls::Once,
        raises_reinforce_latch: false,
        pick: Pick::Drawn,
        removes_caster: false,
    },
    Route {
        enemy: 49,
        ability: 0x1D,
        handler: Handler::MagicDefenseUp,
        range: 2,
        guard: Guard::MagicDefenceNotRaised,
        sealable: false,
        calls: Calls::Once,
        raises_reinforce_latch: false,
        pick: Pick::Drawn,
        removes_caster: false,
    },
    route(31, 0x10, Handler::AgilityDown, 8),
    route(32, 0x11, Handler::Poison, 8),
    route(57, 0x24, Handler::Poison, 8),
    route(63, 0x25, Handler::SleepParalyze, 9),
    route(111, 0x4B, Handler::AgilityDown, 9),
    route(138, 0x4B, Handler::AgilityDown, 9),
    route(76, 0x34, Handler::SleepParalyze, 9),
    route(19, 0x0B, Handler::Paralyze, 8),
    route(26, 0x0B, Handler::Paralyze, 8),
    route(21, 0x0B, Handler::Paralyze, 8),
    // Greneris's objects share EnemyAttack_Juza's caster prelude, whose seal
    // test ends a sealed caster's turn at frame $1D through loc_2B1CC: DORAN
    // and GELUN's $768 and SEALS's $76C reach it at 56109, RIMIT's $754 at
    // 56242 (ps4.asm:56081-56123, 56238-56258, 56563-56574).
    sealable(route(115, 0x28, Handler::AgilityDown, 9)),
    sealable(route(115, 0x57, Handler::AttackDown, 9)),
    sealable(route(115, 0x29, Handler::SealTech, 9)),
    sealable(route(115, 0x2A, Handler::SleepParalyze, 9)),
    // BattleObj_EnemyRimit's frame $24: sealed, it clears $FFFFEE80 and
    // itself before the effect call (ps4.asm:38384-38393).
    sealable(route(77, 0x2A, Handler::SleepParalyze, 9)),
    route(107, 0x4C, Handler::SleepParalyze, 8),
    route(106, 0x4C, Handler::SleepParalyze, 8),
    // VOL: Greneris's $74C reaches the Juza prelude's test at 56505 (exit
    // loc_2B1CC); BloodSaber's $2C0 shares the ShadowSabr prelude loc_1D5E2,
    // whose sealed caster ends at frame $19 (ps4.asm:39980-40011);
    // SoldrFiend's $374 tests at frame $E and, sealed, ends at $24 with no
    // effect call (ps4.asm:46774-46804).
    sealable(route(115, 0x2F, Handler::Death, 8)),
    sealable(route(72, 0x2F, Handler::Death, 8)),
    sealable(route(88, 0x2F, Handler::Death, 8)),
    // EnemyAttack_ShadowSabr is EnemyAttackOffs $46, $47 and $48
    // (ps4.asm:19277-19279): one arm, one object, three carriers.
    deban(70),
    deban(71),
    deban(72),
    // DTHSPELL: loc_250A2's single call, then the stored target's kill. The
    // ChaosSorcr family's arm (112 Illusionst, 113 ImagioMage) loads another
    // object, $72C, and stays off the table until a capture sees it.
    route(107, 0x4E, Handler::Death, 8),
    route(128, 0x60, Handler::SleepParalyze, 8),
    Route {
        raises_reinforce_latch: true,
        ..route(128, 0x62, Handler::IncreaseStats, 3)
    },
    Route {
        calls: Calls::TwiceLastDecides,
        ..route(131, 0x4C, Handler::SleepParalyze, 8)
    },
    // Lane A6, after the Air Castle (`docs/battle/ENEMY_ABILITIES_ENDGAME.md`).
    // FLASH: EnemyAttack_ArmDrone's $194 (loc_1886C, ps4.asm:33905), one call
    // (33947), no request.
    route(43, 0x16, Handler::DexterityDown, 9),
    // CYANICBOMB: EnemyAttack_FloatMine's $1A arm (22767), $1C8 (loc_1836A,
    // 33562): loc_250A2, the kill (loc_25048, 33599), then the caster leaves.
    Route {
        removes_caster: true,
        ..route(46, 0x1A, Handler::Death, 8)
    },
    // SPARK: EnemyAttack_Warren286's loc_10148 (22600) picks a living android;
    // $1F0's loc_250A2 (32511), $1E8's kill (32622).
    Route {
        pick: Pick::Android,
        ..route(49, 0x1E, Handler::Death, 8)
    },
    // POISONMIST: FlameNewt takes Mistralgec's arm (loc_FD06, 22287-22294,
    // only the palette differs) and object $244.
    route(58, 0x24, Handler::Poison, 8),
    // EnemyAttack_DarkMaraud ($40-$42, 22083): every object starts in
    // loc_1E94C, whose frame $14 tests the caster's seal (41371-41375).
    sealable(route(64, 0x28, Handler::AgilityDown, 9)),
    shift(64),
    shift(65),
    shift(66),
    Route {
        guard: Guard::AgilityWord,
        ..sealable(route(65, 0x27, Handler::AgilityUp, 2))
    },
    sealable(route(65, 0x29, Handler::SealTech, 9)),
    sealable(route(66, 0x29, Handler::SealTech, 9)),
    sealable(route(66, 0x2A, Handler::SleepParalyze, 9)),
    // BloodSaber's SHIFT: ShadowSabr's fall-through loc_F986 (22023-22028),
    // $2C4 (loc_1D0C6, 39638) in DEBAN's seal-testing state 0 (40125-40129).
    shift(72),
    // STRNGLIGHT: EnemyAttack_ToadStool's loc_F562 (21750), $31C -> loc_24D76.
    route(79, 0x36, Handler::SleepParalyze, 8),
    // BAD SMELL: EnemyAttack_Zombie's loc_EA4A, $714 (loc_2C42C, 57853) ->
    // loc_24D76.
    route(110, 0x50, Handler::SleepParalyze, 9),
    // ChaosSorcr family: DTHSPELL's $72C (loc_2B8C6, 57104: loc_250A2 at 57151,
    // the kill at 57193-57201) and MINDBLST's $71C (loc_2C0D4, 57615: the call
    // at 57663, loc_25074 at 57706). No seal test.
    route(112, 0x4E, Handler::Death, 8),
    route(113, 0x4E, Handler::Death, 8),
    route(112, 0x51, Handler::SleepParalyze, 9),
    route(113, 0x51, Handler::SleepParalyze, 9),
    // Radhin (EnemyAttack_Juza): the caster prelude loc_2AB2E tests the seal at
    // 56109. SHIFT's $75C picks a random enemy first (loc_25100); SANER's $764,
    // SEALS's $76C and DEBAN's $770 have no guard.
    Route {
        pick: Pick::RandomEnemy,
        ..sealable(route(116, 0x26, Handler::AttackUp, 3))
    },
    sealable(route(116, 0x27, Handler::AgilityUp, 2)),
    sealable(route(116, 0x29, Handler::SealTech, 9)),
    sealable(route(116, 0x2D, Handler::DefenseUp, 2)),
    // SHADOWBIND: ShadMirage's $3F0 (loc_2078E, 43835; call 43889) and Dark
    // Force 3's $858 (loc_30A38, 63083; call 63106). MINDBLST: $860
    // (loc_30792, 62902; call 62925, loc_25074 62985).
    route(105, 0x4B, Handler::AgilityDown, 9),
    route(132, 0x4B, Handler::AgilityDown, 9),
    route(132, 0x51, Handler::SleepParalyze, 9),
    // Profound Darkness 3: EVIL EYE's $8A4 (loc_2E524, 60482) makes one call
    // through loc_250A2 (60563) and sets the sleep bit (loc_25074, 60628);
    // CANCELING's $8A8 (loc_2E2DE, 60310) one call at 60472.
    route(135, 0x4C, Handler::SleepParalyze, 8),
    route(135, 0x69, Handler::RestoreStats, 9),
];

/// A SHIFT arm with the derived-attack guard: `EnemyAttack_DarkMaraud`'s
/// (22111-22127, object `$260`) and ShadowSabr's fall-through.
const fn shift(enemy: u16) -> Route {
    Route {
        guard: Guard::AttackNotRaised,
        ..sealable(route(enemy, 0x26, Handler::AttackUp, 3))
    }
}

/// The ShadowSabr family's DEBAN arm: the routine's own defence test
/// (`loc_F89A`), then the object's seal test.
const fn deban(enemy: u16) -> Route {
    Route {
        guard: Guard::DefenceNotRaised,
        sealable: true,
        ..route(enemy, 0x2D, Handler::DefenseUp, 2)
    }
}

/// Whether the arm of `enemy` for `ability` loads an object that raises
/// `$FFFFEE86` ([`super::super::enemy_ai::AiFlags::reinforced`]).
pub(in crate::battle) fn raises_reinforce_latch(enemy: u16, ability: u8) -> bool {
    ROUTES
        .iter()
        .any(|r| r.enemy == enemy && r.ability == ability && r.raises_reinforce_latch)
}

/// The same arm, with an object that tests the caster's seal first.
const fn sealable(route: Route) -> Route {
    Route {
        sealable: true,
        ..route
    }
}

/// One arm with no guard.
const fn route(enemy: u16, ability: u8, handler: Handler, range: u8) -> Route {
    Route {
        enemy,
        ability,
        handler,
        range,
        guard: Guard::None,
        sealable: false,
        calls: Calls::Once,
        raises_reinforce_latch: false,
        pick: Pick::Drawn,
        removes_caster: false,
    }
}
