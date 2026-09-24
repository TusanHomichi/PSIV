//! Enemy object-side effects, enabled only after their dispatch is traced.

use super::{
    BattleData, BattleDataError, BattleEvent, EnemyRecord, FighterId, Rolls, Roster, Side, Stats,
};

#[cfg(test)]
#[path = "enemy_skill_tests.rs"]
mod tests;

/// An eight-byte enemy ability, independent of player skills and techniques.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemySkill {
    /// One-based table id.
    pub id: u8,
    /// Cartridge display name.
    pub name: String,
    /// Effect dispatcher id.
    pub effect: u8,
    /// Actor stat selector.
    pub power_stat: u8,
    /// Raw byte 2, interpreted by the enemy's object dispatcher.
    pub target: u8,
    /// Power or hit threshold.
    pub power: u8,
    /// Target stat selector.
    pub resistance: u8,
    /// Resistance element.
    pub element: u8,
}

impl EnemySkill {
    /// Fission, Acid Breath and the crawler family's THREAD. Other routines
    /// remain explicitly unsupported until their gameplay has been transcribed.
    #[must_use]
    pub const fn supported(&self) -> bool {
        self.is_fission() || self.is_acid_breath() || self.is_thread()
    }

    const fn is_fission(&self) -> bool {
        matches!(
            (self.id, self.effect, self.target),
            (6, 30, 9) | (7, 30, 10)
        ) && self.power_stat == 0
            && self.power == 0
            && self.resistance == 0
            && self.element == 0
    }

    const fn is_acid_breath(&self) -> bool {
        self.id == 51
            && self.effect == 1
            && self.power_stat == 1
            && self.target == 8
            && self.power == 24
            && self.resistance == 6
            && self.element == 1
    }

    const fn is_thread(&self) -> bool {
        self.id == 16
            && self.effect == 6
            && self.power_stat == 1
            && self.target == 8
            && self.power == 64
            && self.resistance == 3
            && self.element == 1
    }
}

/// EnemyAttack_Crawler selects object $134 for THREAD, retaining the chosen
/// party target. Its animation calls GetEnemySkillEffectAndRange once and
/// never requests a physical damage reaction. Effect 6 uses STR versus live
/// AGI, then subtracts STR from modified AGI (not from the previous debuff).
pub(super) fn resolve_thread(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    let Some(skill) = data.enemy_skill(ability).filter(|s| s.is_thread()) else {
        return false;
    };
    let Some(caster) = roster.get(actor).filter(|f| {
        f.is_alive() && f.id.side() == Side::Enemy && matches!(f.stats.enemy_id, 30..=32)
    }) else {
        return false;
    };
    let power = super::technique::stat(&caster.stats, skill.power_stat);
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    let Some(fighter) = intended
        .filter(|id| id.side() == Side::Party)
        .and_then(|id| roster.get_mut(id))
        .filter(|f| f.is_alive())
    else {
        return true;
    };
    let stats = &mut fighter.stats;
    if super::calculate_chances(
        power as i16,
        super::technique::stat(stats, skill.resistance) as i16,
        i16::from(stats.element_factor(skill.element).unwrap_or(0)),
        i16::from(skill.power),
        i16::from(skill.effect),
        rolls,
    ) == super::Verdict::Miss
    {
        events.push(BattleEvent::Resolved {
            actor,
            target: fighter.id,
            verdict: super::Verdict::Miss,
            damage: None,
            remaining_hp: stats.curr_hp,
        });
    } else {
        stats.agility.battle = stats.agility.modified.saturating_sub(power as u8).max(1);
        events.push(BattleEvent::StatChanged {
            actor,
            target: fighter.id,
            stat: super::technique::TechniqueStat::Agility,
            value: stats.agility.battle.into(),
        });
    }
    true
}

/// The enemies whose `EnemyAttackOffs` entry is one of the two `$33` routines
/// transcribed below.
///
/// `EnemyAttackOffs` (`ps4.asm:19206`) gives `EnemyAttack_FlattrPlnt` to enemy
/// ids `$4B`, `$4C` and `$4D` — 75 FlattrPlnt, 76 FlyScreamr and 77 TechPlant —
/// and `EnemyAttack_Piercer` (`ps4.asm:21518`) to `$55` and `$56`, 85 Piercer
/// and 86 HakenLeft. The gate is the proven *carrier* set rather than the
/// routine set: a routine can only run for an enemy that rolled `$33`, and
/// 77 TechPlant's US ability list is `$2A`/`$2E` only
/// (`generated/enemies.json`), so it never reaches the `$33` arm even though it
/// shares the routine.
const ACID_BREATH_CARRIERS: [u16; 4] = [75, 76, 85, 86];

/// Ability `$33` for every carrier whose attack routine was traced.
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
pub(super) fn resolve_acid_breath(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    let Some(skill) = data.enemy_skill(ability).filter(|s| s.is_acid_breath()) else {
        return false;
    };
    let Some(caster) = roster.get(actor).filter(|f| {
        f.is_alive()
            && f.id.side() == Side::Enemy
            && ACID_BREATH_CARRIERS.contains(&f.stats.enemy_id)
    }) else {
        return false;
    };
    let power = super::technique::stat(&caster.stats, skill.power_stat);
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

/// EnemyInit_Igglanova clears the neighboring fighter objects, but keeps
/// their formation identity and stats ready for Fission. Position bit 7 is
/// a palette selector; it is unrelated to this initialization routine.
pub(super) fn initialize_enemies(roster: &mut Roster) {
    let parents: Vec<_> = roster
        .side(Side::Enemy)
        .filter(|f| matches!(f.stats.enemy_id, 12 | 13))
        .map(|f| f.id)
        .collect();
    for parent in parents {
        for neighbor in [parent.get().checked_sub(1), parent.get().checked_add(1)] {
            if let Some(fighter) = neighbor
                .and_then(FighterId::new)
                .filter(|id| id.side() == Side::Enemy)
                .and_then(|id| roster.get_mut(id))
            {
                fighter.active = false;
            }
        }
    }
}

/// `EnemyAI_EmptySpace`: only the adjacent formation slots can be replaced.
/// Two empty sides cost one parity draw, even left and odd right; one side
/// costs none. Dead fighters retain their formation metadata in the port.
pub(super) fn fission_neighbor(
    roster: &Roster,
    actor: FighterId,
    record: &EnemyRecord,
    ability: &mut u8,
    rolls: &mut impl Rolls,
) -> Option<FighterId> {
    if !matches!(record.id, 12 | 13) {
        return None;
    }
    for (&condition, &replacement) in record
        .condition_ids
        .iter()
        .zip(&record.conditional_abilities)
    {
        if condition == 0 {
            break;
        }
        if condition != 1 {
            continue;
        }
        let eligible = |id: u8| {
            FighterId::new(id).filter(|id| {
                id.side() == Side::Enemy
                    && roster.get(*id).is_some_and(|fighter| !fighter.is_alive())
            })
        };
        let left = actor.get().checked_sub(1).and_then(eligible);
        let right = actor.get().checked_add(1).and_then(eligible);
        let slot = match (left, right) {
            (Some(left), Some(right)) => Some(if rolls.next_roll() & 1 == 0 {
                left
            } else {
                right
            }),
            (left, right) => left.or(right),
        };
        if slot.is_some() {
            *ability = replacement;
            return slot;
        }
    }
    None
}

/// `loc_14CBE` calls Battle_FillEnemyStats using the chosen neighbor's cached
/// enemy id, NOT the ability's target byte. Thus Guilgenova recreates the
/// Gicefalgue its formation contains, despite Fission2's byte naming ZoranBult.
pub(super) fn resolve_fission(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    target: FighterId,
    data: &BattleData,
    events: &mut Vec<BattleEvent>,
) -> Result<bool, BattleDataError> {
    let Some(skill) = data.enemy_skill(ability).filter(|skill| skill.is_fission()) else {
        return Ok(false);
    };
    let Some(fighter) = roster.get_mut(target).filter(|f| !f.is_alive()) else {
        return Ok(false);
    };
    let original = data.enemy(fighter.stats.enemy_id)?;
    fighter.stats = Stats::from_enemy(original);
    fighter.ability = 0;
    fighter.active = true;
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    events.push(BattleEvent::EnemyReplenished {
        actor,
        fighter: target,
        enemy_id: original.id,
        name: original.name.clone(),
        hp: original.hp,
    });
    Ok(true)
}
