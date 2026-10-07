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

#[cfg(test)]
#[path = "enemy_effect_endgame_tests.rs"]
mod endgame_tests;

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
    /// `$09`, `AbilityEffect_AttackUp` (`ps4.asm:9181-9198`): after the
    /// range's validity test and the chance step, `atk_pow_battle = atk_pow +
    /// d1` (a word add, wrapping) - from the derived value, so a second cast
    /// does not stack.
    AttackUp,
    /// `$0C`, `AbilityEffect_AgilityUp` (`ps4.asm:9237-9249`): `agility_battle =
    /// agility_mod + d1` as a byte add, wrapping.
    AgilityUp,
    /// `$21`, `AbilityEffect_DexterityDown` (`ps4.asm:9457-9467`): on a landed
    /// roll `dexterity_battle = dexterity_mod - d1`, and 1 when that borrows
    /// or reaches zero (`bhi.s`).
    DexterityDown,
    /// `$27`, `AbilityEffect_RestoreStats` (`ps4.asm:9472-9491`): the chance
    /// step, then - whatever it answered - the fourteen element bytes from
    /// their low copies, the four battle bytes from their `_mod` values and the
    /// three battle words from their derived values.
    RestoreStats,
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
            Handler::AttackUp => 0x09,
            Handler::AgilityUp => 0x0C,
            Handler::DexterityDown => 0x21,
            Handler::RestoreStats => 0x27,
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
    /// SHIFT's arms in `EnemyAttack_DarkMaraud` (`ps4.asm:22111-22116`) and
    /// `EnemyAttack_ShadowSabr`'s fall-through (`loc_F986`, 22024-22027):
    /// `move.w atk_pow, d3 / cmp.w atk_pow_battle, d3 / bcs` - derived attack
    /// below battle attack (already raised) sends the turn to the physical swing
    /// with the ability cleared (`loc_FA5A`, `loc_F826`).
    AttackNotRaised,
    /// SANER's arm in `EnemyAttack_DarkMaraud` (`ps4.asm:22134-22139`):
    /// `move.w $1E(a2), d3 / cmp.w $20(a2), d3 / bcs` compares the *words* at
    /// `$1E` and `$20` - agility base and `_mod`, against agility battle and
    /// dexterity base. An enemy's base bytes are never written
    /// (`Battle_FillEnemyStats`, `ps4.asm:11954-11961`, fills only `_mod` and
    /// battle; the battle load zeroes the block, 9985), so the test is
    /// `agility_mod < agility_battle << 8`: a swing whenever the battle byte
    /// is not zero.
    AgilityWord,
}

/// How the object chooses the fighter `AbilityRange_Single` visits, when it
/// is not the target `Enemy_Attack` drew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    /// `Current_Target_Index` as the roll left it (or the range ignores it).
    Drawn,
    /// Radhin's SHIFT (`$75C` state 4, `loc_2ABA4`, `ps4.asm:56127-56131`):
    /// `loc_25100` (48980) draws `rand & 3` and re-draws while that enemy
    /// fighter word is empty, then writes the slot to `Current_Target_Index`.
    /// The range is `Self`, so the pick only places the animation - but each
    /// draw moves the seed before the effect call.
    RandomEnemy,
    /// SPARK's arm (`loc_10148`, `ps4.asm:22600-22644`): the party's living
    /// androids (profession 5, `$16` bit 6 clear), first in `$FFFFEE90`, last in
    /// `$FFFFEE94`. None sends the turn to `loc_10016`'s cleared-ability swing;
    /// one is the target; two or more take one `UpdateRNGSeed2` (22633) whose
    /// bit 0 picks the first (clear) or the last (set).
    Android,
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
    /// Who the single range visits.
    pick: Pick,
    /// Whether the object removes its caster at the end: CYANICBOMB's `$1C8`
    /// state 8 (`ps4.asm:33621-33627`) clears the actor's group bits, palette
    /// object, fighter word and reaction byte.
    removes_caster: bool,
}

#[path = "enemy_effect_routes.rs"]
mod routes;
use routes::ROUTES;
pub(super) use routes::raises_reinforce_latch;

/// The route `enemy` runs for `skill`, when the record is the traced one: the
/// handler's effect id and the route's range nibble.
fn route_of(enemy: u16, skill: &EnemySkill) -> Option<&'static Route> {
    ROUTES.iter().find(|r| {
        r.enemy == enemy
            && r.ability == skill.id
            && skill.effect == r.handler.effect()
            && skill.target & 0x0F == r.range
    })
}

/// Whether [`resolve_effect_skill`] runs `skill` for `enemy`.
pub(super) fn owns(enemy: u16, skill: &EnemySkill) -> bool {
    route_of(enemy, skill).is_some()
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
    let Some(skill) = data.enemy_skill(ability) else {
        return EffectTurn::NotMine;
    };
    let Some(route) = route_of(caster.stats.enemy_id, skill) else {
        return EffectTurn::NotMine;
    };
    let raised = match route.guard {
        Guard::None => false,
        Guard::DefenceNotRaised => caster.stats.defence.derived < caster.stats.defence.battle,
        Guard::MagicDefenceNotRaised => {
            (caster.stats.mental_defence.battle as i16)
                > (caster.stats.mental_defence.derived as i16)
        }
        Guard::AttackNotRaised => caster.stats.attack.derived < caster.stats.attack.battle,
        Guard::AgilityWord => {
            let agility = caster.stats.agility;
            let dexterity = caster.stats.dexterity;
            u16::from_be_bytes([agility.base, agility.modified])
                < u16::from_be_bytes([agility.battle, dexterity.base])
        }
    };
    if raised {
        if let Some(fighter) = roster.get_mut(actor) {
            fighter.ability = 0;
        }
        return EffectTurn::Swing;
    }
    let intended = match route.pick {
        Pick::Drawn | Pick::RandomEnemy => intended,
        Pick::Android => {
            // `loc_10148`: the arm reads the party before it loads anything.
            let androids: Vec<FighterId> = roster
                .side(Side::Party)
                .filter(|f| {
                    f.stats.profession == super::PROFESSION_ANDROID
                        && f.stats.status & status::ANDROID_DEAD == 0
                })
                .map(|f| f.id)
                .collect();
            match androids.as_slice() {
                [] => {
                    if let Some(fighter) = roster.get_mut(actor) {
                        fighter.ability = 0;
                    }
                    return EffectTurn::Swing;
                }
                [only] => Some(*only),
                [first, .., last] => {
                    let roll = rolls.next_roll();
                    Some(if roll & 1 == 0 { *first } else { *last })
                }
            }
        }
    };
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
    if route.pick == Pick::RandomEnemy {
        // `loc_25100`: `rand & 3` until it names an enemy fighter whose
        // object word is set - a fallen enemy's is cleared at the end of its
        // death object (`loc_2D960`, `ps4.asm:59631`).
        let occupied: Vec<bool> = (6..=9u8)
            .map(|slot| {
                FighterId::new(slot)
                    .and_then(|id| roster.get(id))
                    .is_some_and(Fighter::is_alive)
            })
            .collect();
        if occupied.iter().any(|o| *o) {
            while !occupied[usize::from(rolls.next_roll() & 3)] {}
        }
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
    if route.removes_caster
        && let Some(fighter) = roster.get_mut(actor)
    {
        // `$1C8` state 8: the caster's object is cleared - not a death, so no
        // reward - and its reaction byte with it.
        let occupied = fighter.active;
        fighter.active = false;
        fighter.reaction_flags = 0;
        if occupied {
            events.push(BattleEvent::Died { fighter: actor });
        }
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
        Handler::AttackUp => {
            if landed(skill, power, stats, rolls) {
                stats.attack.battle = stats.attack.derived.wrapping_add(power);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Attack,
                    value: stats.attack.battle,
                });
            }
        }
        Handler::AgilityUp => {
            if landed(skill, power, stats, rolls) {
                stats.agility.battle = stats.agility.modified.wrapping_add(power as u8);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Agility,
                    value: stats.agility.battle.into(),
                });
            }
        }
        Handler::DexterityDown => {
            if landed(skill, power, stats, rolls) {
                stats.dexterity.battle =
                    stats.dexterity.modified.saturating_sub(power as u8).max(1);
                events.push(BattleEvent::StatChanged {
                    actor,
                    target,
                    stat: TechniqueStat::Dexterity,
                    value: stats.dexterity.battle.into(),
                });
            }
        }
        Handler::RestoreStats => {
            // `bsr.s Battle_ProcessEffect`, answer unread.
            landed(skill, power, stats, rolls);
            stats.element_props = stats.element_shadow;
            stats.strength.battle = stats.strength.modified;
            stats.mental.battle = stats.mental.modified;
            stats.agility.battle = stats.agility.modified;
            stats.dexterity.battle = stats.dexterity.modified;
            stats.attack.battle = stats.attack.derived;
            stats.defence.battle = stats.defence.derived;
            stats.mental_defence.battle = stats.mental_defence.derived;
            events.push(BattleEvent::StatsRestored { actor, target });
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
