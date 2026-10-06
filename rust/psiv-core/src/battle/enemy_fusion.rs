//! Fusion and COMBINE: an object replaces the whole enemy side with a formation
//! it keeps inline.
//!
//! Two arms load such an object:
//!
//! - **Fusion.** `EnemyAI_ZolSlugs` (arm `$06`, [`super::enemy_ai`]) writes the
//!   conditional `$12` when every occupied enemy slot holds a ZolSlug and there
//!   are exactly two. `EnemyAttack_Blob` (`ps4.asm:23043`) sends a nonzero
//!   ability to `loc_10796`, which loads `BattleObj_Fusion` (`ps4.asm:35675`) and
//!   `BattleObj_Fusion2` (`ps4.asm:35850`) and clears `Current_Target_Index`.
//!   At their frame `$14` (`ps4.asm:35817`) they reload the side from
//!   `loc_1A2F4` (`ps4.asm:35820-35841`).
//! - **COMBINE.** `EnemyAI_HakenLeftExists` (`$0D`, named by 84 BladeRight) and
//!   `EnemyAI_BladeRightExists` (`$0E`, named by 86 HakenLeft) write `$3A` and
//!   `$3B` when the pair stands alone. `EnemyAttack_Ripper`'s fall-through
//!   `loc_F3B6` (`ps4.asm:21620-21624`) and `EnemyAttack_Piercer`'s `loc_F2E4`
//!   (`ps4.asm:21549-21553`) clear the ability (`clr.w $24(a4)`) and
//!   `Current_Target_Index` and turn the attack object into `$354` (`loc_23C84`,
//!   `ps4.asm:47469`), which does everything on its first frame
//!   (`ps4.asm:47483-47503`): it reloads the side from `loc_23D00`.
//!
//! The reload is the same in both objects:
//!
//! ```text
//!     clear the 32 objects at $FFFFD800 and the four Fighter_Enemy_n words
//!     lea (<record>).l, a0 / lea (Enemy_Formation_Data).l, a1 / moveq #4, d7
//!     trap #1                                       ; ten bytes over the header
//!     jsr loc_14D46                                 ; loc_7F22 -> Battle_FillEnemyStats
//!     move.w #$16, (Battle_Routine).l
//! ```
//!
//! `loc_14D46` (`ps4.asm:29735`) feeds the new record through `loc_7F22` ->
//! `Battle_FillEnemyStats` (`ps4.asm:11939`) exactly as the battle's own load
//! does, so whatever HP the old fighters had and whichever slots they stood in,
//! the fight continues against the record's full-HP enemy in slot 1. The copy
//! takes the header too: `Enemy_Run_Chance` is `Enemy_Formation_Data + 1`
//! (`ps4.constants.asm:2039-2041`), so the escape test reads the record's run
//! byte from then on. The records are the pack's (`psiv_tools.formations`'s
//! `INLINE_FORMATIONS`), looked up by their label.
//!
//! Nothing in either chain draws from the RNG (no `UpdateRNGSeed2` site in
//! either object or the routines they call), and the records' own range and hit
//! bytes are never read: the effect byte `$1F` is `AbilityEffect_None` and no
//! object calls `GetEnemySkillEffectAndRange`.

use super::{BattleData, BattleDataError, BattleEvent, FighterId, Roster, Side, Stats};

/// One arm that reloads the enemy side from an inline record.
struct Reload {
    /// The carrier's enemy id.
    carrier: u16,
    /// The ability its condition writes.
    ability: u8,
    /// The label of the record the object copies.
    formation: &'static str,
    /// Whether the arm clears `$24(a4)` before it loads the object (COMBINE's
    /// do, Fusion's does not).
    clears_ability: bool,
}

/// Every arm this module resolves.
const RELOADS: &[Reload] = &[
    // EnemyAttackOffs $22 (34 ZolSlug) -> EnemyAttack_Blob -> BattleObj_Fusion.
    Reload {
        carrier: 34,
        ability: 0x12,
        formation: "loc_1A2F4",
        clears_ability: false,
    },
    // EnemyAttackOffs $54 (84 BladeRight) -> EnemyAttack_Ripper -> $354.
    Reload {
        carrier: 84,
        ability: 0x3A,
        formation: "loc_23D00",
        clears_ability: true,
    },
    // EnemyAttackOffs $56 (86 HakenLeft) -> EnemyAttack_Piercer -> $354.
    Reload {
        carrier: 86,
        ability: 0x3B,
        formation: "loc_23D00",
        clears_ability: true,
    },
];

/// `AbilityEffect_None`'s index, the effect byte both records carry.
const NO_EFFECT: u8 = 0x1F;

/// Fusion or COMBINE, if `ability` is one of [`RELOADS`] for `actor`.
///
/// Returns the run byte of the new record when the side was reloaded - the
/// caller's `Enemy_Run_Chance` from then on - and `None` when the pair is not
/// one of these arms.
///
/// # Errors
/// [`BattleDataError::UnknownInlineFormation`] or
/// [`BattleDataError::UnknownEnemy`] when the pack lacks the record or the
/// enemy it seats.
pub(super) fn resolve_fusion(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    data: &BattleData,
    events: &mut Vec<BattleEvent>,
) -> Result<Option<u8>, BattleDataError> {
    let Some(caster) = roster.get(actor).filter(|f| f.is_alive()) else {
        return Ok(None);
    };
    let Some(reload) = RELOADS
        .iter()
        .find(|r| r.carrier == caster.stats.enemy_id && r.ability == ability)
    else {
        return Ok(None);
    };
    let Some(skill) = data.enemy_skill(ability).filter(|s| s.effect == NO_EFFECT) else {
        return Ok(None);
    };
    let formation = data.inline_formation(reload.formation)?;
    // Both records seat one enemy, in slot 1; the event says which.
    let [seat] = formation.enemies.as_slice() else {
        return Ok(None);
    };
    let record = data.enemy(seat.enemy_id)?;
    if reload.clears_ability {
        if let Some(fighter) = roster.get_mut(actor) {
            fighter.ability = 0;
        }
    } else {
        events.push(BattleEvent::EnemySkillUsed {
            actor,
            skill: ability,
            name: skill.name.clone(),
        });
    }
    let removed: Vec<FighterId> = roster.side(Side::Enemy).map(|f| f.id).collect();
    roster.clear_enemies();
    let fighter = roster
        .add_enemy(seat.slot, record)
        .expect("the side was just emptied");
    debug_assert_eq!(Stats::from_enemy(record).curr_hp, record.hp);
    events.push(BattleEvent::EnemiesFused {
        actor,
        removed,
        fighter,
        enemy_id: record.id,
        position: seat.position,
        name: record.name.clone(),
        hp: record.hp,
        agility: record.agility,
    });
    Ok(Some(formation.run_chance))
}
