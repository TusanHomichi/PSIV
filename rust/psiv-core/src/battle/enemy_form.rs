//! Profound Darkness's form changes.
//!
//! `EnemyAttack_ProfoundDarkness1` (`ps4.asm:19823`) compares `$24(a4)`
//! against `$64`, `$21`, `$22` and `$66`; anything else - in practice the `$67`
//! its condition `$02` (`EnemyAI_HalfHPOrLower`) writes, since every regular
//! slot holds one of the first three - takes the else arm (19901-19905):
//! `clr.w $24(a4)`, `clr.w Current_Target_Index`, object `$888` (`loc_2F560`,
//! 61652). `EnemyAttack_ProfoundDarkness2` (19750) does the same for its `$68`
//! (19817-19821, object `$89C`, `loc_2E9DA`, 60824). Neither object requests
//! damage, calls the effect or the generator; each animates for hundreds of
//! frames, and on one of them writes the next form's id into
//! `Enemy_Positions` and calls `loc_7F22` (`loc_2F8D8`, 61916-61918;
//! `loc_2EDD8`, 61119-61121), which refills every listed enemy's stats from its
//! record (`Battle_FillEnemyStats`, `ps4.asm:11939`): full HP, battle stats
//! from the record, status cleared. No init routine runs (`EnemyInit_*` is
//! fighter routine 1, which `loc_7F22` does not set), so the fighter object -
//! its reaction byte and the `$FFFFEEA8` re-roll word included - carries on:
//! the Ambush bit Profound Darkness 1's latch set at the battle's start
//! (`loc_B62A`, 17456-17463) is still there when the third form first acts,
//! and its arm `$0B` turns that turn into MEGID.
//!
//! The forms can still die before they change: nothing floors their HP
//! (`FighterShowDamage_DecreaseHP`, 3640-3659) and the first two take the
//! ordinary boss death object `$840` (`loc_2D936`'s table, 59623).

use super::{BattleData, BattleDataError, BattleEvent, FighterId, Roster, Side, Stats};

/// `(form, ability its condition writes, next form)`.
const FORMS: &[(u16, u8, u16)] = &[(133, 0x67, 134), (134, 0x68, 135)];

/// The form `enemy` changes into on `ability`, when the pair is a form change
/// and the record is the traced one: effect `$1F` (`AbilityEffect_None`), no
/// stat, no range, the next form's id in byte 3 (the objects never read it).
fn next_form(enemy: u16, skill: &super::EnemySkill) -> Option<u16> {
    FORMS
        .iter()
        .find(|(form, ability, next)| {
            *form == enemy
                && *ability == skill.id
                && skill.effect == 0x1F
                && u16::from(skill.power) == *next
        })
        .map(|(_, _, next)| *next)
}

/// Whether [`resolve_form_change`] runs `skill` for `enemy`.
pub(super) fn owns(enemy: u16, skill: &super::EnemySkill) -> bool {
    next_form(enemy, skill).is_some()
}

/// The form change, if `ability` is one for `actor`: the ability is cleared
/// and the actor's slot reloaded from the next form's record.
///
/// # Errors
/// [`BattleDataError::UnknownEnemy`] when the pack lacks the next form.
pub(super) fn resolve_form_change(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    data: &BattleData,
    events: &mut Vec<BattleEvent>,
) -> Result<bool, BattleDataError> {
    let Some(skill) = data.enemy_skill(ability) else {
        return Ok(false);
    };
    let Some(next) = roster
        .get(actor)
        .filter(|f| f.is_alive() && f.id.side() == Side::Enemy)
        .and_then(|f| next_form(f.stats.enemy_id, skill))
    else {
        return Ok(false);
    };
    let record = data.enemy(next)?;
    let fighter = roster.get_mut(actor).expect("checked above");
    fighter.ability = 0;
    fighter.stats = Stats::from_enemy(record);
    fighter.name = record.name.clone();
    events.push(BattleEvent::EnemyStatsReloaded {
        actor,
        fighter: actor,
        enemy_id: record.id,
        name: record.name.clone(),
        hp: record.hp,
    });
    Ok(true)
}
