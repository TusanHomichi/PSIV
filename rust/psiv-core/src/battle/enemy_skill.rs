//! Enemy object-side effects, enabled only after their dispatch is traced.

use super::{BattleData, BattleDataError, BattleEvent, FighterId, Rolls, Roster, Side, Stats};

#[cfg(test)]
#[path = "enemy_skill_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "enemy_skill_wasted_tests.rs"]
mod wasted_tests;

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
    /// Fission and TechUser's RES. Enemy skills that make one damage request
    /// live in [`super::enemy_damage`], the ones that change a status or a stat
    /// in [`super::enemy_effect`]; other routines remain explicitly unsupported
    /// until their gameplay has been transcribed.
    #[must_use]
    pub const fn supported(&self) -> bool {
        self.is_refill() || self.is_tech_heal()
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

    /// Record 23 at `0x28341C` is `22 00 00 00 00 00 00 00` — effect `$22`,
    /// every selector and the element zero. `resolve_no_effect_turn` reads none
    /// of those bytes; the id is pinned here so a pack whose `$17` became
    /// something else cannot pass for the traced WAITING record.
    const fn is_waiting(&self) -> bool {
        self.id == 23
            && self.effect == 34
            && self.power_stat == 0
            && self.target == 0
            && self.power == 0
            && self.resistance == 0
            && self.element == 0
    }

    /// Record 20 `$14` WARNING at `0x283404` is `1e 00 2c 00 00 00 00 00`:
    /// effect `$1E` like the two Fission records, no stat selector, byte 2
    /// `$2C`, no power, resistance or element. Its objects end in the same
    /// refill the Fission objects do (see [`resolve_fission`]); the record is
    /// pinned so a pack whose `$14` became something else cannot pass for it.
    const fn is_warning(&self) -> bool {
        self.id == 20
            && self.effect == 30
            && self.power_stat == 0
            && self.target == 44
            && self.power == 0
            && self.resistance == 0
            && self.element == 0
    }

    /// The abilities whose object ends by refilling the neighbour slot
    /// `EnemyAI_EmptySpace` named: Fission and Fission2 and WARNING.
    const fn is_refill(&self) -> bool {
        self.is_fission() || self.is_warning()
    }

    /// The four heal records the [`HEALS`] arms cast.
    ///
    /// Record 69 `$45` RES at `0x28358C` is `12 82 01 10 00 00 00 00`, record
    /// 62 `$3E` GIRES at `0x283554` is `12 82 01 40 00 00 00 00`, and records 70
    /// `$46` SAR and 73 `$49` GISAR are `12 82 02 10 00 00 00 00`: effect `$12`
    /// (`AbilityEffect_NormalLogic`), the MEN selector `$82`, target nibble 1
    /// or 2, a power byte (16 or 64) and neither a resistance selector nor an
    /// element. The whole records are pinned because the effect byte alone
    /// covers 44 ids.
    const fn is_tech_heal(&self) -> bool {
        matches!(
            (self.id, self.power, self.target),
            (69, 16, 1) | (62, 64, 1) | (70, 16, 2) | (73, 16, 2)
        ) && self.effect == 18
            && self.power_stat == 130
            && self.resistance == 0
            && self.element == 0
    }
}

/// Whether [`resolve_fission`] can run `skill`: the refill records. The arm
/// that reaches it is `EnemyAI_EmptySpace`, whatever the carrier.
pub(super) const fn owns_refill(skill: &EnemySkill) -> bool {
    skill.is_refill()
}

/// Whether [`resolve_no_effect_turn`] spends `skill`'s turn for `enemy`.
pub(super) fn owns_wasted(enemy: u16, skill: &EnemySkill) -> bool {
    (skill.is_fission() && FLOAT_MINE_CARRIERS.contains(&enemy))
        || (skill.is_waiting()
            && (FLOAT_MINE_CARRIERS.contains(&enemy) || ARM_DRONE_CARRIERS.contains(&enemy)))
}

/// Whether [`resolve_res`] heals with `skill` for `enemy`.
pub(super) fn owns_heal(enemy: u16, skill: &EnemySkill) -> bool {
    heal_of(enemy, skill).is_some()
}

/// Whom a heal arm's object hands to `GetEnemySkillEffectAndRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Healed {
    /// `$3D4`'s `loc_21504` (`ps4.asm:44774`): the occupied enemy slot with the
    /// lowest current HP, the earlier slot on a tie.
    LowestHp,
    /// Range 2, `AbilityRange_MultiEnemies`: every living enemy, slot order.
    /// TechUser's `$3D8` (`loc_2132A`, 44654; its state 4 `loc_2136A` names
    /// no single target) and Radhin's `$774` (`loc_2A4EC`, 55664).
    AllEnemies,
    /// SoldrFiend's `$378` (`loc_22FC0`, 46569): `move.w Current_Actor_Index,
    /// Current_Target_Index` (46698) before the call, so the caster.
    Caster,
}

/// One heal arm: the carrier, the ability, whom it heals.
const HEALS: &[(u16, u8, Healed)] = &[
    // EnemyAttack_TechUser ($63-$65): loc_EEAA (21266) clears the target; $3E
    // and $45 load $3D4, $46 and $49 (loc_EF22 21295, loc_EF4A 21304) load $3D8.
    (99, 0x45, Healed::LowestHp),
    (100, 0x3E, Healed::LowestHp),
    (100, 0x46, Healed::AllEnemies),
    (101, 0x49, Healed::AllEnemies),
    // EnemyAttack_Juza's loc_E6B6 (20766): $774.
    (116, 0x49, Healed::AllEnemies),
    // EnemyAttack_TwinArms's fall-through loc_F228 (21501): $378.
    (88, 0x3E, Healed::Caster),
];

fn heal_of(enemy: u16, skill: &EnemySkill) -> Option<Healed> {
    if !skill.is_tech_heal() {
        return None;
    }
    HEALS
        .iter()
        .find(|(carrier, ability, _)| *carrier == enemy && *ability == skill.id)
        .map(|(_, _, healed)| *healed)
}

/// EnemyInit_Igglanova and EnemyInit_Tower (`ps4.asm:18275`, `18243`) clear the
/// neighboring fighter objects: 12 Igglanova and 13 Guilgenova take the first,
/// 39 Tower and 45 CommndBall the second. The port keeps their formation
/// identity and stats ready for the refill. Position bit 7 is a palette
/// selector; it is unrelated to this initialization routine.
///
/// `EnemyInit_Tower` opens with `cmpi.w #$2C, $52(a4) / bne / rts`
/// (`ps4.asm:18244-18246`): it returns without clearing anything when
/// `$52(a4)` reads `$2C`. The fighter objects sit `next_obj = $40` apart
/// (`ps4.constants.asm:99`) and `fighter_id` is `$12`, so `$52(a4)` is the
/// *next* fighter's enemy id and `$2C` is 44 FloatMine: a Tower or CommndBall
/// whose right-hand neighbor is a FloatMine keeps both neighbors. Two captured
/// CommndBall formations show both outcomes: `$124` (FloatMine2 on both sides)
/// opens with a queue of the CommndBall alone, `$125` (FloatMine on both sides)
/// queues all three (`docs/oracle/BATTLE_ORACLE_ZELAN.md` and
/// `docs/oracle/BATTLE_ORACLE_REPLAY.md`). Reading the word as the enemy's
/// condition ids (`$0101`) cannot explain the second capture.
pub(super) fn initialize_enemies(roster: &mut Roster) {
    const FLOAT_MINE: u16 = 44;
    let parents: Vec<_> = roster
        .side(Side::Enemy)
        .filter(|f| match f.stats.enemy_id {
            12 | 13 => true,
            39 | 45 => {
                let next = f.id.get().checked_add(1).and_then(FighterId::new);
                !next
                    .filter(|id| id.side() == Side::Enemy)
                    .and_then(|id| roster.get(id))
                    .is_some_and(|neighbor| neighbor.stats.enemy_id == FLOAT_MINE)
            }
            _ => false,
        })
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

/// `loc_14CBE` calls Battle_FillEnemyStats using the chosen neighbor's cached
/// enemy id, NOT the ability's target byte. Thus Guilgenova recreates the
/// Gicefalgue its formation contains, despite Fission2's byte naming ZoranBult.
///
/// The target comes from the AI instruction block's own `$01` arm
/// (`enemy_ai::instruction_block`), which is where `$FFFFEE81` and the parity
/// draw now live: `EnemyAttack_Igglanova` clears `Current_Target_Index` and
/// hands the object the slot the arm named.
///
/// The refill is shared by every ability whose object ends in `loc_14CBE`
/// ([`EnemySkill::is_refill`]): the two Fission records, and `$14` WARNING of 45
/// CommndBall (and 39 Tower), whose `$1CC` object (`loc_1879A`,
/// `ps4.asm:33852`) finishes with `clr.w (a4) / movea.l $7C(a4), a1 /
/// jmp loc_14CBE` (`ps4.asm:33846-33850`) after an alarm animation
/// (`loc_185EC`, `ps4.asm:33725`). The captured fight
/// (`docs/oracle/BATTLE_ORACLE_ZELAN.md`) shows it: round 1's queue holds the
/// CommndBall alone - `EnemyInit_Tower` (`ps4.asm:18243`) cleared both
/// FloatMine2 beside it - and round 2 holds the one WARNING named.
pub(super) fn resolve_fission(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    target: FighterId,
    data: &BattleData,
    events: &mut Vec<BattleEvent>,
) -> Result<bool, BattleDataError> {
    let Some(skill) = data.enemy_skill(ability).filter(|skill| skill.is_refill()) else {
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

/// The `EnemyAttackOffs` entries that point at `EnemyAttack_FloatMine`
/// (`ps4.asm:22675`): `$2C` 44 FloatMine, `$2D` 45 CommndBall, `$2E` 46
/// VopalSphre and `$32` 50 FloatMine2. Their eight regular slots hold `$07`
/// Fission2 (50 only) and `$17` Waiting (44, 46, 50), plus `$19` Detonation
/// (45), which the routine has an arm for; the conditional ids their records
/// name — `$18` Explosion (44, 50), `$1A` CyanicBomb (46) and `$14` Warning
/// (45) — are arms too.
const FLOAT_MINE_CARRIERS: [u16; 4] = [44, 45, 46, 50];

/// `EnemyAttack_ArmDrone` (`ps4.asm:22789`) is the `EnemyAttackOffs` entry of
/// `$29` 41 ArmDrone, `$2A` 42 SatMinion and `$2B` 43 StarDrone
/// (`ps4.asm:19248-19250`). Its `$17` arm `loc_10468` (`ps4.asm:22806-22813`)
/// is the same six instructions as `loc_10406` (`movea.l $38(a1), a0 / clr.w
/// Current_Target_Index / clr.w $24(a4) / move.w #$16, Battle_Routine / subq.w
/// #2, $2(a4) / rts`), and **only** `$17` takes it: any other non-zero
/// ability falls through `loc_10434` (`ps4.asm:22795-22805`) to object `$194`, so
/// FISSION2 must not enter this arm. SatMinion reaches `$17` through its
/// conditional (`EnemyAI_CRayTubeNearSatMinion`, which overwrites the ability
/// when slots 1/2/3 hold SatMinion, CRayTube, SatMinion: formation `$123`), and
/// the captured fight carries CRayTube's CHARGCNNON `$15` in the same round.
const ARM_DRONE_CARRIERS: [u16; 2] = [41, 42];

/// The roll this routine has nothing to load for: `$07` Fission2 on 50
/// FloatMine2 and `$17` Waiting on 44 FloatMine, 46 VopalSphre and 50
/// FloatMine2 spend the turn without an effect.
///
/// `EnemyAttack_FloatMine` tests `$24(a4)` for `$14` (`ps4.asm:22677`), `$18`
/// (`22690`), `$19` (`22707`) and `$1A` (`22766`) and has no other arm, so
/// every remaining id — the two above are the only ones these four enemies can
/// roll — reaches `loc_10406` (`ps4.asm:22781`):
///
/// ```text
/// loc_10406:
///     movea.l $38(a1), a0
///     clr.w   (Current_Target_Index).l
///     clr.w   $24(a4)
///     move.w  #$16, (Battle_Routine).l
///     subq.w  #2, $2(a4)
///     rts
/// ```
///
/// No object, no `LoadPLC1`, no palette write, no `Sound_Index`. `$16` is
/// `Battle_DoAttackEffect` (`ps4.asm:8553`); the ability it reads is now 0 and
/// `loc_B6A2` left all nine `Fighters_Hit_Flags` at `$FF`, so it takes
/// `loc_5DD2` (`ps4.asm:8625`) — `Battle_Routine` `$12` — without ever calling
/// `Ability_GetEffectAndRange`. No damage, no status, no message: the `$12`/
/// `$1E` pair only waits and advances the turn order. Emitting this instead of
/// a physical swing is the whole point: the actor really does act and do
/// nothing.
///
/// The ArmDrone family has the same clear-and-return arm for WAITING only;
/// FISSION2 must not enter it. Damage arms are owned by `enemy_damage`, and
/// neither their existence nor another carrier's no-effect arm proves a pair.
pub(super) fn resolve_no_effect_turn(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    data: &BattleData,
    events: &mut Vec<BattleEvent>,
) -> bool {
    let Some(skill) = data
        .enemy_skill(ability)
        .filter(|skill| skill.is_fission() || skill.is_waiting())
    else {
        return false;
    };
    let carrier = roster.get(actor).is_some_and(|fighter| {
        fighter.is_alive()
            && fighter.id.side() == Side::Enemy
            && owns_wasted(fighter.stats.enemy_id, skill)
    });
    if !carrier {
        return false;
    }
    if let Some(fighter) = roster.get_mut(actor) {
        // `clr.w $24(a4)`. `loc_6672` reads this slot back when it decides how
        // long the end-of-action wait is; an ability still in it means the
        // ordinary attack wait instead of the longer empty one.
        fighter.ability = 0;
    }
    events.push(BattleEvent::EnemyAbilityWasted {
        actor,
        ability: skill.id,
        name: skill.name.clone(),
    });
    true
}

/// RES `$45` and GIRES `$3E` - the two heals of `EnemyAttack_TechUser`'s
/// fall-through. RES is the conditional ability TechUser's own AI instruction
/// picks, written when any enemy is at or below half HP
/// (`EnemyAI_HalfHPOrLower_AllEnemies`, `ps4.asm:21320`); GIRES is TechMaster's
/// regular first slot. The arm compares `$3E` and `$45` one after the other
/// (`ps4.asm:21272`, `21281`) and both load the same object, so one
/// implementation serves both records (id 69 power 16, id 62 power 64).
///
/// `EnemyAttack_TechUser` reaches `loc_EEAA` (`ps4.asm:21266`) for it: it
/// clears `Current_Target_Index` and loads object `$3D4` (`loc_213DC`,
/// `ps4.asm:44701`), whose `loc_21504` (`ps4.asm:44774`) picks the **enemy with
/// the lowest current HP**, slot 1 upwards, keeping the earlier slot on a tie,
/// then hands that id to `GetEnemySkillEffectAndRange`. In the swept fixtures
/// the caster is that slot — 38 of 80 against its neighbour's 65 — which is why
/// their logs show the heal landing on the caster itself.
///
/// The record (`12 82 01 10 00 00 00 00`, `0x28358C`) is effect `$12`
/// (`AbilityEffect_NormalLogic`, `ps4.asm:9282`) with the target nibble `1` and
/// byte 4 zero, so `Effect_SetupSkillParams` skips the chance roll entirely
/// (`tst.b $4(a0,d0.w) / beq.s loc_6564`, `ps4.asm:9594-9595`) and the heal is
/// `Battle_CalcHealing` (`ps4.asm:17411`) on the caster's MEN with byte 3 as
/// `d3` — the same 16-draw formula `technique` effects use, via
/// [`super::calc_healing`] — added to the target's HP and capped at its
/// maximum (`loc_2F3A`).
pub(super) fn resolve_res(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> bool {
    let Some(skill) = data.enemy_skill(ability) else {
        return false;
    };
    let Some(caster) = roster
        .get(actor)
        .filter(|f| f.is_alive() && f.id.side() == Side::Enemy)
    else {
        return false;
    };
    let Some(healed) = heal_of(caster.stats.enemy_id, skill) else {
        return false;
    };
    // `AbilityStatsOffs[$82 & $7F]` is `mental_battle`; the mask is
    // `Effect_SetupSkillParams`' own (`ps4.asm:9580`).
    let power = super::technique::stat(&caster.stats, skill.power_stat & 0x7F);
    let sealed = caster.stats.status & super::stats::status::TECH_SEALED != 0;
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    if sealed {
        // Every heal object tests the caster first: `$3D4`/`$3D8`'s frame `$F`
        // (`ps4.asm:44725-44730`; sealed, phase 8 with bit 7 at `$1D` and
        // `loc_21558` ends the turn, 44803-44805), Radhin's `$774` (55692)
        // and SoldrFiend's `$378` (46632-46636). No target, no effect call.
        return true;
    }
    let targets: Vec<FighterId> = match healed {
        // `loc_21504`: the occupied enemy slot with the lowest `curr_hp`, the
        // earlier one on a tie — an unoccupied slot never wins.
        Healed::LowestHp => roster
            .side(Side::Enemy)
            .filter(|f| f.is_alive())
            .min_by_key(|f| f.stats.curr_hp)
            .map(|f| f.id)
            .into_iter()
            .collect(),
        // `Ability_ProcessRange` over slots 6-9; `loc_2D0E` then heals each
        // fighter whose effect word is set, in slot order (`ps4.asm:4447`).
        Healed::AllEnemies => roster
            .side(Side::Enemy)
            .filter(|f| f.is_alive())
            .map(|f| f.id)
            .collect(),
        Healed::Caster => vec![actor],
    };
    for target in targets {
        let heal = super::calc_healing(power, u16::from(skill.power), rolls);
        let fighter = roster.get_mut(target).expect("just selected");
        let before = fighter.stats.curr_hp;
        fighter.stats.curr_hp = before.saturating_add(heal).min(fighter.stats.max_hp);
        events.push(BattleEvent::Healed {
            actor,
            target,
            amount: fighter.stats.curr_hp - before,
            remaining_hp: fighter.stats.curr_hp,
        });
    }
    true
}
