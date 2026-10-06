//! Enemy abilities that change a status bit or a battle stat.
//!
//! An enemy object that applies an ability's *effect* — as opposed to asking for
//! damage (`super::enemy_damage`) — calls `GetEnemySkillEffectAndRange`
//! (`ps4.asm:8687`) once, from its handoff state. That routine copies the
//! record's byte 2 (low nibble) into `Battle_Ability_Range`, clears the nine
//! `Battle_Ability_Effects` words, sets `Ability_Effect_Type` to 1 and runs
//! `Ability_GetEffectAndRange` (`ps4.asm:8886`), whose `AbilityRangeOffs`
//! (`ps4.asm:8903`) pick the fighters `Ability_ProcessRange` (`ps4.asm:8975`)
//! visits, in slot order, skipping an empty slot (`tst.w (a0)`) and a dead or
//! android-dead one (`andi.b #$44, d0`). For each visited fighter the record's
//! effect byte indexes `AbilityEffectsOffs` (`ps4.asm:9036`), and that handler
//! runs:
//!
//! | effect | handler | what it does to the visited fighter |
//! |---|---|---|
//! | `$02` | `AbilityEffect_Death` (`ps4.asm:9098`) | nothing if dead; else one chance roll; the object then clears HP and sets the death bit |
//! | `$03` | `AbilityEffect_AttackDown` (`ps4.asm:9109`) | chance roll; on success `atk_pow_battle = max(0, atk_pow - d1)` |
//! | `$06` | `AbilityEffect_AgilityDown` (`ps4.asm:9139`) | chance roll; on success `agility_battle = max(1, agility_mod - d1)` |
//! | `$07` | `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) | nothing if asleep or paralyzed; else one chance roll, which only writes the effect word |
//! | `$08` | `AbilityEffect_SealTech` (`ps4.asm:9167`) | nothing if sealed; else chance roll, then `bset #StatusTechSealed` |
//! | `$0A` | `AbilityEffect_DefenseUp` (`ps4.asm:9203`) | `dfs_pow_battle = dfs_pow + d1` after the range's validity test and, when the record has a resistance selector, a chance roll |
//! | `$0B` | `AbilityEffect_MagicDefenseUp` (`ps4.asm:9220`) | `mdfs_pow_battle = mdfs_pow + d1`, wrapping a word, after the range/chance test |
//! | `$1B` | `AbilityEffect_Poison` (`ps4.asm:9410`) | nothing if poisoned; else chance roll, then `bset #StatusPoisoned` |
//! | `$1C` | `AbilityEffect_Paralyze` (`ps4.asm:9424`) | nothing if paralyzed; else chance roll, then paralyzed set, sleep cleared, `agility_battle` and `dexterity_battle` both 1 |
//! | `$2B` | `AbilityEffect_IncreaseStats` (`ps4.asm:9496`) | the chance step (`Battle_ProcessEffect`, whose answer it never tests), then 20 added to the four battle bytes and the three battle words |
//!
//! `d1` is the actor's power stat, which `Effect_SetupSkillParams`
//! (`ps4.asm:9576`) reads through record byte 1 **masked with `$7F`** (line
//! 9579): the `$82` of a magic record is selector 2, `mental_battle`. The chance
//! roll is `Battle_CalculateChances` (`ps4.asm:17338`) with the target's stat
//! named by byte 4, the target's element factor named by byte 5, byte 3 as the
//! miss threshold and the effect id as the critical threshold — the same
//! arguments [`super::enemy_skill`]'s THREAD and POISON pass. A record with
//! byte 4 zero skips the roll (`tst.b $4(a0,d0.w) / beq.s loc_6564`,
//! `ps4.asm:9594-9595`) and the handler always lands.
//!
//! [`ROUTES`] is the gate: one `(enemy, ability)` pair per arm whose object
//! chain was read out of the disassembly and found to call the handler exactly
//! once with the range the record names. The record still drives the numbers,
//! but only for a record whose effect id and range nibble are the route's own.
//! Anything outside the table keeps the explicit
//! [`BattleEvent::UnsupportedAbility`] path.
//!
//! The ledger entries — chains, citations, the captures that observed each
//! pair — are `docs/battle/ENEMY_EFFECT_ABILITIES.md` and
//! `docs/oracle/BATTLE_ORACLE_ARC.md`.

use super::stats::status;
use super::technique::{TechniqueStat, stat};
use super::{
    BattleData, BattleEvent, EnemySkill, Fighter, FighterId, Rolls, Roster, Side, Stats, Verdict,
    calculate_chances,
};

#[cfg(test)]
#[path = "enemy_effect_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "enemy_effect_crawler_tests.rs"]
mod crawler_tests;

#[cfg(test)]
#[path = "enemy_effect_barrier_tests.rs"]
mod barrier_tests;

#[cfg(test)]
#[path = "enemy_effect_air_castle_tests.rs"]
mod air_castle_tests;

/// An `AbilityEffectsOffs` handler this module implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handler {
    /// `$02`, `AbilityEffect_Death` (`ps4.asm:9098`), with the kill the VOL
    /// objects make once the effect word is set: `clr.w $E(a3)` (HP) and
    /// `bset #2, $16(a3)` - or `bset #6` for an android (`cmpi.w #5, $6(a3)`) -
    /// at `loc_25048` (`ps4.asm:48916`), `loc_1D420` (`ps4.asm:39858`) and the
    /// siblings, after which the fighter's routine 7 is `Character_Dead`.
    Death,
    /// `$03`, `AbilityEffect_AttackDown` (`ps4.asm:9109`).
    AttackDown,
    /// `$06`, `AbilityEffect_AgilityDown` (`ps4.asm:9139`).
    AgilityDown,
    /// `$07`, `AbilityEffect_SleepParalyze` (`ps4.asm:9154`), with the sleep bit
    /// the VOICE object's `loc_25074` (`ps4.asm:48929`) sets from the effect
    /// words afterwards.
    SleepParalyze,
    /// `$08`, `AbilityEffect_SealTech` (`ps4.asm:9167`).
    SealTech,
    /// `$0A`, `AbilityEffect_DefenseUp` (`ps4.asm:9203`).
    DefenseUp,
    /// `$0B`, `AbilityEffect_MagicDefenseUp` (`ps4.asm:9220-9233`).
    MagicDefenseUp,
    /// `$1B`, `AbilityEffect_Poison` (`ps4.asm:9410`).
    Poison,
    /// `$1C`, `AbilityEffect_Paralyze` (`ps4.asm:9424`).
    Paralyze,
    /// `$2B`, `AbilityEffect_IncreaseStats` (`ps4.asm:9496-9506`): `bsr.s
    /// Battle_ProcessEffect` and then, without testing its answer, `moveq #20,
    /// d1` added with `add.b` to `strength_battle`, `mental_battle`,
    /// `agility_battle` and `dexterity_battle` and with `add.w` to
    /// `atk_pow_battle`, `dfs_pow_battle` and `magic_dfs_battle` - each from its
    /// battle value, so it stacks and wraps.
    IncreaseStats,
}

impl Handler {
    /// The `AbilityEffectsOffs` index the record must carry.
    const fn effect(self) -> u8 {
        match self {
            Handler::Death => 0x02,
            Handler::AttackDown => 0x03,
            Handler::AgilityDown => 0x06,
            Handler::SleepParalyze => 0x07,
            Handler::SealTech => 0x08,
            Handler::DefenseUp => 0x0A,
            Handler::MagicDefenseUp => 0x0B,
            Handler::Poison => 0x1B,
            Handler::Paralyze => 0x1C,
            Handler::IncreaseStats => 0x2B,
        }
    }
}

/// A test an arm makes before it loads the effect's object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Guard {
    /// The arm loads the object unconditionally.
    None,
    /// `EnemyAttack_ShadowSabr`'s `$2D` arm (`loc_F89A`, `ps4.asm:21966`):
    /// `move.w $28(a2), d3 / cmp.w $2A(a2), d3 / bcs.w loc_F826` — when the
    /// actor's derived defence is below its battle defence (already raised),
    /// the arm branches to `loc_F826` (`ps4.asm:21936`), which clears `$24(a4)`
    /// and loads the ordinary attack objects. The turn becomes a physical
    /// swing with no ability.
    DefenceNotRaised,
    /// Warren286/Siren386/Browren486's BARRIER arm, `loc_100A6`
    /// (`ps4.asm:22559`, compare at 22567-22570): signed `battle > derived` falls back to
    /// `loc_10016`'s cleared-ability physical swing.
    MagicDefenceNotRaised,
}

/// How many times the route's object runs `GetEnemySkillEffectAndRange`
/// (`ps4.asm:8687`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Calls {
    /// One call, which the object (or a shared tail such as `loc_250A2`,
    /// `ps4.asm:48945`) reads afterwards.
    Once,
    /// DarkForce2's EVIL EYE (`loc_30F10`, `ps4.asm:63411-63430`): `jsr
    /// GetEnemySkillEffectAndRange` at 63417 and then `jsr loc_250A2` at
    /// 63418, which calls it again (48947). The second call's
    /// `Battle_ClearEffects` (`ps4.asm:8695`) wipes the words the first one
    /// wrote, so each call takes its own chance roll and only the second
    /// decides; `loc_25074` (48929) reads its words at the end (63456).
    /// Only [`Handler::SleepParalyze`] uses it: that handler writes nothing but
    /// the effect word, so the first call changes no state.
    TwiceLastDecides,
}

/// One proven `(enemy, ability)` arm.
#[derive(Debug, Clone, Copy)]
struct Route {
    /// Enemy record id.
    enemy: u16,
    /// Ability id the arm compares `$24(a4)` against.
    ability: u8,
    /// The handler its object's single `GetEnemySkillEffectAndRange` runs.
    handler: Handler,
    /// The record's byte 2 low nibble: the `AbilityRangeOffs` entry. `8` is
    /// `AbilityRange_Single` (the drawn `Current_Target_Index`), `9`
    /// `AbilityRange_MultiChars` (party slots 1-5), `2` `AbilityRange_MultiEnemies`
    /// (enemy slots 6-9).
    range: u8,
    /// What the arm tests first.
    guard: Guard,
    /// Whether the object tests the caster's `StatusTechSealed` before the
    /// effect - `btst #4, $16(a1)` on `$3C(a4)`'s stats (`ps4.asm:40129` for
    /// DEBAN) - and, when it is set, ends the turn with no effect call.
    sealable: bool,
    /// How many effect calls the object makes.
    calls: Calls,
    /// Whether the object raises `$FFFFEE86`, Lashiec's REINFORCE latch
    /// (`loc_27ED0`, `st ($FFFFEE86).w` at `ps4.asm:53102`).
    raises_reinforce_latch: bool,
}

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
const ROUTES: &[Route] = &[
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
];

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
/// `$FFFFEE86` ([`super::enemy_ai::AiFlags::reinforced`]).
pub(super) fn raises_reinforce_latch(enemy: u16, ability: u8) -> bool {
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
    }
}

/// What the engine does after [`resolve_effect_skill`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EffectTurn {
    /// The pair is not one of [`ROUTES`] (or its record is not the traced one).
    NotMine,
    /// The ability's object ran and the turn is spent.
    Resolved,
    /// The arm's guard sent the turn to the ordinary attack objects: the
    /// ability slot is cleared and the engine takes the physical swing.
    Swing,
}

/// Runs an enemy's status or stat ability if its `(enemy, ability)` arm is one
/// of [`ROUTES`].
pub(super) fn resolve_effect_skill(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> EffectTurn {
    let Some(caster) = roster
        .get(actor)
        .filter(|f| f.is_alive() && f.id.side() == Side::Enemy)
    else {
        return EffectTurn::NotMine;
    };
    let enemy = caster.stats.enemy_id;
    let Some(route) = ROUTES
        .iter()
        .find(|r| r.enemy == enemy && r.ability == ability)
    else {
        return EffectTurn::NotMine;
    };
    let Some(skill) = data.enemy_skill(ability).filter(|s| {
        s.effect == route.handler.effect() && s.target & 0x0F == route.range && s.id == ability
    }) else {
        return EffectTurn::NotMine;
    };
    let raised = match route.guard {
        Guard::None => false,
        Guard::DefenceNotRaised => caster.stats.defence.derived < caster.stats.defence.battle,
        Guard::MagicDefenceNotRaised => {
            (caster.stats.mental_defence.battle as i16)
                > (caster.stats.mental_defence.derived as i16)
        }
    };
    if raised {
        if let Some(fighter) = roster.get_mut(actor) {
            fighter.ability = 0;
        }
        return EffectTurn::Swing;
    }
    // `$7F`: `Effect_SetupSkillParams` masks the selector before the stat table.
    let power = stat(&caster.stats, skill.power_stat & 0x7F);
    let sealed = route.sealable && caster.stats.status & status::TECH_SEALED != 0;
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    if sealed {
        // The object animates, finds the seal and ends the turn before its
        // effect call: no roll, no change.
        return EffectTurn::Resolved;
    }
    for target in visited(route.range, actor, intended) {
        let Some(fighter) = roster.get_mut(target) else {
            continue;
        };
        // `Ability_ProcessRange`: an empty slot, or one whose status holds a
        // death bit, is skipped before the handler.
        if !fighter.is_alive() {
            continue;
        }
        if route.calls == Calls::TwiceLastDecides {
            // The first call's roll, whose word the second call wipes.
            debug_assert_eq!(route.handler, Handler::SleepParalyze);
            if fighter.stats.status & (status::ASLEEP | status::PARALYZED) == 0 {
                landed(skill, power, &fighter.stats, rolls);
            }
        }
        apply(
            route.handler,
            skill,
            power,
            actor,
            (target, fighter),
            rolls,
            events,
        );
    }
    EffectTurn::Resolved
}

/// `AbilityRangeOffs` (`ps4.asm:8903`): the fighters the range visits, in
/// slot order.
fn visited(range: u8, actor: FighterId, intended: Option<FighterId>) -> Vec<FighterId> {
    match range {
        // `AbilityRange_Self` (`ps4.asm:8946`): the actor.
        3 => vec![actor],
        // `AbilityRange_Single`: `Current_Target_Index`.
        8 => intended
            .into_iter()
            .filter(|id| id.side() == Side::Party)
            .collect(),
        // `AbilityRange_MultiChars`: `moveq #1, d6 / moveq #4, d7`.
        9 => (1..=5).filter_map(FighterId::new).collect(),
        // `AbilityRange_MultiEnemies`: `moveq #6, d6 / moveq #3, d7`.
        2 => (6..=9).filter_map(FighterId::new).collect(),
        _ => Vec::new(),
    }
}

/// `Effect_DoEnemySkill` (`ps4.asm:9570`) for one visited fighter: the chance
/// roll, or none when the record names no resistance stat. Returns whether the
/// effect landed — `tst.w d0 / bmi` in every handler that tests it.
fn landed(skill: &EnemySkill, power: u16, stats: &Stats, rolls: &mut impl Rolls) -> bool {
    if skill.resistance == 0 {
        // `loc_6564`: the effect word is written without a roll, and `d0` still
        // holds the non-negative `ability << 3`.
        return true;
    }
    calculate_chances(
        power as i16,
        stat(stats, skill.resistance & 0x7F) as i16,
        i16::from(stats.element_factor(skill.element).unwrap_or(0)),
        i16::from(skill.power),
        i16::from(skill.effect),
        rolls,
    ) != Verdict::Miss
}

/// One handler on one visited fighter.
fn apply(
    handler: Handler,
    skill: &EnemySkill,
    power: u16,
    actor: FighterId,
    (target, fighter): (FighterId, &mut Fighter),
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) {
    let stats = &mut fighter.stats;
    let inflicted = |bit: u8| BattleEvent::StatusInflicted {
        actor,
        target,
        status: bit,
    };
    match handler {
        Handler::Death => {
            // `btst #StatusDead` ends the handler before the roll.
            if stats.status & status::DEAD != 0 {
                return;
            }
            if landed(skill, power, stats, rolls) {
                fighter.mark_defeated();
                events.push(BattleEvent::Died { fighter: target });
            }
        }
        Handler::AttackDown => {
            if landed(skill, power, stats, rolls) {
                // `move.w atk_pow, atk_pow_battle / sub.w d1 / bhi.s / clr.w`.
                stats.attack.battle = stats.attack.derived.saturating_sub(power);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Attack,
                    value: stats.attack.battle,
                });
            }
        }
        Handler::AgilityDown => {
            if landed(skill, power, stats, rolls) {
                // `move.b agility_mod, agility_battle / sub.b d1 / bhi.s / move.b #1`.
                stats.agility.battle = stats.agility.modified.saturating_sub(power as u8).max(1);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Agility,
                    value: stats.agility.battle.into(),
                });
            }
        }
        Handler::DefenseUp => {
            if landed(skill, power, stats, rolls) {
                // `move.w dfs_pow, dfs_pow_battle / add.w d1, dfs_pow_battle`:
                // from the derived value, so a second cast does not stack.
                stats.defence.battle = stats.defence.derived.wrapping_add(power);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Defence,
                    value: stats.defence.battle,
                });
            }
        }
        Handler::MagicDefenseUp => {
            if landed(skill, power, stats, rolls) {
                // AbilityEffect_MagicDefenseUp: derived + d1, not stacking
                // and not saturating (ps4.asm:9230-9231).
                stats.mental_defence.battle = stats.mental_defence.derived.wrapping_add(power);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::MentalDefence,
                    value: stats.mental_defence.battle,
                });
            }
        }
        Handler::SleepParalyze => {
            // `btst #StatusAsleep` / `btst #StatusParalyzed`: either bit ends
            // the handler before the roll. Bit 3 only — `StatusAsleep2` is not
            // tested.
            if stats.status & (status::ASLEEP | status::PARALYZED) != 0 {
                return;
            }
            if landed(skill, power, stats, rolls) {
                // The handler only writes the effect word; `loc_25074` turns
                // every nonzero word into `bset #3, $16(a1)`.
                stats.status |= status::ASLEEP;
                events.push(inflicted(status::ASLEEP));
            }
        }
        Handler::SealTech => {
            if stats.status & status::TECH_SEALED != 0 {
                return;
            }
            if landed(skill, power, stats, rolls) {
                stats.status |= status::TECH_SEALED;
                events.push(inflicted(status::TECH_SEALED));
            }
        }
        Handler::Poison => {
            // `btst #StatusPoisoned` ends the handler before the roll; the
            // bit is the handler's own (`bset #StatusPoisoned, status(a4)`).
            if stats.status & status::POISONED != 0 {
                return;
            }
            if landed(skill, power, stats, rolls) {
                stats.status |= status::POISONED;
                events.push(inflicted(status::POISONED));
            }
        }
        Handler::IncreaseStats => {
            // `Battle_ProcessEffect`'s chance step runs (none for a record with
            // no resistance byte) and its answer is not read.
            landed(skill, power, stats, rolls);
            const RAISE: u8 = 20;
            stats.strength.battle = stats.strength.battle.wrapping_add(RAISE);
            stats.mental.battle = stats.mental.battle.wrapping_add(RAISE);
            stats.agility.battle = stats.agility.battle.wrapping_add(RAISE);
            stats.dexterity.battle = stats.dexterity.battle.wrapping_add(RAISE);
            stats.attack.battle = stats.attack.battle.wrapping_add(RAISE.into());
            stats.defence.battle = stats.defence.battle.wrapping_add(RAISE.into());
            stats.mental_defence.battle = stats.mental_defence.battle.wrapping_add(RAISE.into());
            for (stat, value) in [
                (TechniqueStat::Attack, stats.attack.battle),
                (TechniqueStat::Defence, stats.defence.battle),
                (TechniqueStat::MentalDefence, stats.mental_defence.battle),
                (TechniqueStat::Agility, stats.agility.battle.into()),
                (TechniqueStat::Dexterity, stats.dexterity.battle.into()),
            ] {
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat,
                    value,
                });
            }
        }
        Handler::Paralyze => {
            if stats.status & status::PARALYZED != 0 {
                return;
            }
            if landed(skill, power, stats, rolls) {
                stats.status |= status::PARALYZED;
                stats.status &= !status::ASLEEP;
                stats.agility.battle = 1;
                stats.dexterity.battle = 1;
                events.push(inflicted(status::PARALYZED));
            }
        }
    }
}
