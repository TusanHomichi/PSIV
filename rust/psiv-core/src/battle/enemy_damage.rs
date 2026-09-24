//! Record-driven enemy damage skills.
//!
//! `Enemy_Attack` (`ps4.asm:19138`) loads one attack object per enemy turn,
//! stores the drawn `Current_Target_Index` in that object's `$38(a1)`
//! (lines 19170-19172) and hands the object to `EnemyAttackOffs[enemy_id]`
//! (`ps4.asm:19206`). A traced arm then ends in one `move.w #$C, $2(a3)` —
//! routine `$C`, `Fighter_TakeDamage` (`ps4.asm:3564`) — aimed at the stored
//! target. When an arm makes exactly one such request, the ability's
//! `EnemySkillData` record alone decides the number: the request reaches
//! `Enemy_DamageCharacter`'s enemy-skill branch (`ps4.asm:3775`), reads the
//! caster's stat through record byte 1, the target's stat through byte 4, the
//! target's element factor through byte 5 and the record's byte 3 as the bonus,
//! and calls `Battle_CalculateDamage` (`ps4.asm:17374`) once.
//!
//! [`DAMAGE_SKILL_ROUTES`] is therefore the whole gate: one `(enemy, ability)`
//! pair per arm whose single request has been read out of the disassembly. The
//! pair proves the *route*; the record still drives the arithmetic, and no
//! record-byte predicate is part of the proof. Anything outside the table keeps
//! the explicit [`BattleEvent::UnsupportedAbility`] path.

use super::{BattleData, BattleEvent, FighterId, Rolls, Roster, Side};

#[cfg(test)]
#[path = "enemy_damage_tests.rs"]
mod tests;

/// Record byte 2 value the routes below are proven against.
///
/// `GetEnemySkillEffectAndRange` (`ps4.asm:8687`) copies the byte to
/// `Battle_Ability_Range` and masks its low nibble (`andi.b #$F`, line 8694);
/// `AbilityRangeOffs` (`ps4.asm:8903`) resolves index 8 to `AbilityRange_Single`
/// (`ps4.asm:8938`), which is `Current_Target_Index` with a count of zero — one
/// target, the drawn one. Both proven records carry byte 2 = 8, and the check
/// below compares the whole byte: a record whose high nibble is set is not
/// guessed at, it stays on the unsupported path.
const SINGLE_TARGET: u8 = 8;

/// One `(enemy, ability)` pair whose `EnemyAttack_*` arm has been traced to
/// exactly one damage request against the chosen party target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DamageRoute {
    /// `stats.enemy_id` of the carrier: the `fighter_id` `Enemy_Attack`
    /// (`ps4.asm:19173`) indexes `EnemyAttackOffs` with.
    enemy_id: u16,
    /// Raw ability byte the arm runs for, as `$24(a4)` holds it.
    ability: u8,
}

/// `EnemySkillData` `$33` ACIDBREATH, record `01 01 08 18 06 01 00 00` at
/// `$2834FC`.
const ACID_BREATH: u8 = 0x33;

/// `EnemySkillData` `$02` FLAME BOLT, record `01 01 08 50 07 03 00 00` at
/// `$283374`: effect `$01`, stat `$01` (strength), tgt 8, pow 80, res `$07`
/// (magic defense), el `3` (fire).
const FLAME_BOLT: u8 = 0x02;

/// Every `(enemy, ability)` pair whose arm has been traced.
///
/// **Acid Breath `$33`.** `EnemyAttackOffs` (`ps4.asm:19206`) gives
/// `EnemyAttack_FlattrPlnt` to enemy ids `$4B`, `$4C` and `$4D` — 75 FlattrPlnt,
/// 76 FlyScreamr and 77 TechPlant — and `EnemyAttack_Piercer`
/// (`ps4.asm:21518`) to `$55` and `$56`, 85 Piercer and 86 HakenLeft. The gate
/// is the proven *carrier* set rather than the routine set: a routine can only
/// run for an enemy that rolled `$33`, and 77 TechPlant's US ability list is
/// `$2A`/`$2E` only (`generated/enemies.json`), so it never reaches the `$33`
/// arm even though it shares the routine.
///
/// `EnemyAttack_FlattrPlnt` (`ps4.asm:21778`) keeps `Current_Target_Index` for
/// `$33`: the `loc_F5BE` arm has no write to it, while `$34` (`loc_F60A`),
/// `$2A` (`loc_F65E`) and the fallback (`loc_F722`) all clear it. The arm
/// converts the enemy's own attack object into `BattleObj_AcidBreath`
/// (`ps4.asm:38566`) and loads `BattleObj_AcidBreathChild` (`ps4.asm:38622`).
/// The child only animates and asks for the hit reaction
/// (`move.w #5, $2(a3)` / `$1C = $E`); the main object's `loc_24AEC` exit
/// (`ps4.asm:48507`) is the single damage request, `move.w #$C, $2(a3)` gated
/// on the target's hit timer and waited on through `($FFFF416C)`.
///
/// `EnemyAttack_Piercer`'s `$33` arm (`loc_F2A0`, `ps4.asm:21532`) also leaves
/// `Current_Target_Index` alone, but loads object `$35C` = `loc_23998`
/// (`ps4.asm:47264`) with the chosen party target in `$38(a1)` and then spawns
/// child `$360` = `loc_24FD2` (`ps4.asm:48883`). The child makes the same
/// hit-reaction write, and `loc_23AB6` (`ps4.asm:47344`) makes the same single
/// `move.w #$C, $2(a3)` damage request once the child releases
/// `($FFFFEE80)` — no attack, no status, one reaction and one hit for both
/// arms. Both write MoleAttack `$D5` and then EnemyAttack4 `$D8`.
///
/// `loc_B75A` supplies a normal hit without a chance roll and
/// `Enemy_DamageCharacter` (`ps4.asm:3775`) reads strength, defense and
/// physical resistance through the shared `EnemySkillData` record, so the
/// number depends on the caster's strength and not on which arm ran.
///
/// **FLAME BOLT `$02`.** `EnemyAttack_ForcedFly` (`ps4.asm:23567`) branches to
/// `EnemyAttack_MonsterFly` (`ps4.asm:23591`) only for ability 0 and otherwise
/// falls through into `EnemyAttack_Helex` (`ps4.asm:23574`), which writes
/// object `$48` (line 23575) — `BattleObj_HelexFlameBolt` (`ps4.asm:30279`) —
/// and, like both Acid Breath arms, never touches `Current_Target_Index`. That
/// object only animates; on animation frame 2 (`cmpi.b #2, $10(a4)`, line
/// 30294) it loads `BattleObj_HelexFlameBolt2` (`ps4.asm:30315`, the `$4C`
/// write at line 30300) and copies `$38`/`$3C` into it (lines 30301-30302),
/// keeping the target the loading path stored in the parent. The child waits
/// out `$2E` = `$110`, writes the hit reaction (`move.w #5, $2(a3)`,
/// `$1C = $C`, lines 30334-30335) and then makes the single damage request
/// `move.w #$C, $2(a3)` (`ps4.asm:30342`) behind the `btst #1, $4(a4)` /
/// `bset #1, $4(a4)` once-guard, waited on through `($FFFF416C)`. That is one
/// request against the chosen party target, and the arm writes EnemyAttack3
/// `$D7` at the parent's load (line 30285) and FireBreath `$C2` at the child's
/// (line 30321).
///
/// FLAME BOLT's carriers are 0 Helex, whose eight regular slots are all `$02`,
/// and 5 ForcedFly, whose slots 5-8 are `$02` and whose lower slots are zero
/// (`generated/enemies.json`): ForcedFly's zero roll takes the
/// `EnemyAttack_MonsterFly` branch, so `$02` is the only nonzero ability either
/// carrier can dispatch.
const DAMAGE_SKILL_ROUTES: &[DamageRoute] = &[
    // `EnemyAttackOffs` `$4B` (`ps4.asm:19282`) → `EnemyAttack_FlattrPlnt`
    // (`ps4.asm:21778`), `$33` arm `loc_F5BE` → BattleObj_AcidBreath
    // (`ps4.asm:38566`); one request at `loc_24AEC` (`ps4.asm:48507`).
    DamageRoute {
        enemy_id: 75,
        ability: ACID_BREATH,
    },
    // `EnemyAttackOffs` `$4C` (`ps4.asm:19283`) → the same
    // `EnemyAttack_FlattrPlnt` arm and object.
    DamageRoute {
        enemy_id: 76,
        ability: ACID_BREATH,
    },
    // `EnemyAttackOffs` `$55` (`ps4.asm:19292`) → `EnemyAttack_Piercer`
    // (`ps4.asm:21518`), `$33` arm `loc_F2A0` → object `$35C` = `loc_23998`
    // (`ps4.asm:47264`); one request at `loc_23AB6` (`ps4.asm:47344`).
    DamageRoute {
        enemy_id: 85,
        ability: ACID_BREATH,
    },
    // `EnemyAttackOffs` `$56` (`ps4.asm:19293`) → the same
    // `EnemyAttack_Piercer` arm and object.
    DamageRoute {
        enemy_id: 86,
        ability: ACID_BREATH,
    },
    // `EnemyAttackOffs` `$00` (`ps4.asm:19207`) → `EnemyAttack_Helex`
    // (`ps4.asm:23574`) → object `$48` = BattleObj_HelexFlameBolt
    // (`ps4.asm:30279`), child `$4C` = BattleObj_HelexFlameBolt2
    // (`ps4.asm:30315`); one request at `ps4.asm:30342`.
    DamageRoute {
        enemy_id: 0,
        ability: FLAME_BOLT,
    },
    // `EnemyAttackOffs` `$05` (`ps4.asm:19212`) → `EnemyAttack_ForcedFly`
    // (`ps4.asm:23567`) falls through into `EnemyAttack_Helex` for a nonzero
    // ability: the same object chain and the same single request.
    DamageRoute {
        enemy_id: 5,
        ability: FLAME_BOLT,
    },
];

/// Whether the table has proven this exact pair.
fn proven(enemy_id: u16, ability: u8) -> bool {
    DAMAGE_SKILL_ROUTES.contains(&DamageRoute { enemy_id, ability })
}

/// One damage request against the chosen party target, for every pair in
/// [`DAMAGE_SKILL_ROUTES`].
///
/// Returns `false`, leaving the ability to
/// [`BattleEvent::UnsupportedAbility`], when the `(enemy, ability)` pair is not
/// in the table, when the ability has no record, when the record's target byte
/// is not the proven single target, or when the actor is not a living enemy.
/// Nothing is drawn and no event is emitted on any of those paths, so the
/// caller's fallback starts from the state the ability roll left behind.
///
/// A resolved skill emits [`BattleEvent::EnemySkillUsed`] first and then, when
/// the target is on the party side and alive, exactly one
/// [`BattleEvent::Resolved`] carrying the clamped damage and the fighter's
/// remaining hit points — plus [`BattleEvent::Died`] when that reaches zero.
pub(super) fn resolve_damage_skill(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    let Some(skill) = data
        .enemy_skill(ability)
        .filter(|s| s.target == SINGLE_TARGET)
    else {
        return false;
    };
    let Some(caster) = roster.get(actor).filter(|f| {
        f.is_alive() && f.id.side() == Side::Enemy && proven(f.stats.enemy_id, ability)
    }) else {
        return false;
    };
    // `Effect_SetupSkillParams` (`ps4.asm:9576`) masks the record's stat byte
    // with `$7F` (line 9580) before indexing `AbilityStatsOffs`, which is how a
    // record written as `$82` selects mental. Byte 4 is read raw there (line
    // 9605) and in `Enemy_DamageCharacter`'s enemy-skill branch, so only the
    // power stat is masked.
    let power = super::technique::stat(&caster.stats, skill.power_stat & super::STAT_INDEX_MASK);
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    let Some(target) = intended.filter(|id| id.side() == Side::Party) else {
        return true;
    };
    let Some(fighter) = roster.get_mut(target).filter(|f| f.is_alive()) else {
        return true;
    };
    let damage = super::clamp_damage(super::calculate_damage(
        power,
        super::technique::stat(&fighter.stats, skill.resistance),
        u16::from(fighter.stats.element_factor(skill.element).unwrap_or(0)),
        u16::from(skill.power),
        rolls,
    ));
    fighter.stats.curr_hp = fighter.stats.curr_hp.saturating_sub(damage);
    events.push(BattleEvent::Resolved {
        actor,
        target,
        verdict: super::Verdict::Normal,
        damage: Some(damage),
        remaining_hp: fighter.stats.curr_hp,
    });
    if fighter.stats.curr_hp == 0 {
        fighter.mark_defeated();
        events.push(BattleEvent::Died { fighter: target });
    }
    true
}
