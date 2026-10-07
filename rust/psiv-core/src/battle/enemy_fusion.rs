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
    /// Whether the arm clears `$24(a4)` before it loads the object (the Air
    /// Castle's COMBINE and InfantWorm's do; Fusion, the Climate Center's
    /// COMBINE and FractOoze's FISSION do not).
    clears_ability: bool,
    /// The record's effect byte. Every one indexes a bare `rts` in
    /// `AbilityEffectsOffs` (`ps4.asm:9036`): `$1F` and `$25` are
    /// `AbilityEffect_None` (table lines 9075, 9081). No object calls
    /// `GetEnemySkillEffectAndRange`, so it is pinned only as the record's
    /// identity.
    effect: u8,
}

/// Every arm this module resolves.
const RELOADS: &[Reload] = &[
    // EnemyAttackOffs $22 (34 ZolSlug) -> EnemyAttack_Blob -> BattleObj_Fusion.
    Reload {
        carrier: 34,
        ability: 0x12,
        formation: "loc_1A2F4",
        clears_ability: false,
        effect: NO_EFFECT,
    },
    // EnemyAttackOffs $54 (84 BladeRight) -> EnemyAttack_Ripper -> $354.
    Reload {
        carrier: 84,
        ability: 0x3A,
        formation: "loc_23D00",
        clears_ability: true,
        effect: NO_EFFECT,
    },
    // EnemyAttackOffs $56 (86 HakenLeft) -> EnemyAttack_Piercer -> $354.
    Reload {
        carrier: 86,
        ability: 0x3B,
        formation: "loc_23D00",
        clears_ability: true,
        effect: NO_EFFECT,
    },
    // EnemyAttackOffs $17 (23 ArthroPod) and $19 (25 Wiredine) ->
    // EnemyAttack_WorkerPod (ps4.asm:23182) -> EnemyAttack_Tarantella
    // (23204): any nonzero id clears Current_Target_Index (23216, 23232) and
    // loads $FC/$100/$10C/$110/$114. $FC's state 4 (27536-27567) whites the
    // palette, clears the objects and copies loc_1308C (27571) - one
    // LifeDeletr - then loc_14D46. The arm keeps $24(a4).
    Reload {
        carrier: 23,
        ability: 0x0C,
        formation: "loc_1308C",
        clears_ability: false,
        effect: NO_EFFECT,
    },
    Reload {
        carrier: 25,
        ability: 0x0D,
        formation: "loc_1308C",
        clears_ability: false,
        effect: NO_EFFECT,
    },
    // EnemyAttackOffs $26 (38 FractOoze) -> EnemyAttack_MetaSlug (22931):
    // every nonzero id but $13 takes loc_10644 (22960), clears the target
    // (22969) and loads BattleObj_SlugFission..4 (34947-35184). Once
    // $FFFFEE80 reaches 4, $160 copies loc_1987E (35062-35076) - four JR.OOZE
    // - and calls loc_14D46. Effect $25 is AbilityEffect_None (9081).
    Reload {
        carrier: 38,
        ability: 0x1B,
        formation: "loc_1987E",
        clears_ability: false,
        effect: 0x25,
    },
    // EnemyAttackOffs $5E (94 InfantWorm) -> EnemyAttack_MiniWorm (21763):
    // loc_F590 (21768-21770) clears the target and $24(a4) and loads $398
    // (loc_223C0, 45734), whose state 4 (loc_2246C, 45787-45803) clears the
    // four enemy fighters, copies loc_224E8 (45818) - one SandWorm - and calls
    // loc_14D46. The record's power byte (80) is never read.
    Reload {
        carrier: 94,
        ability: 0x41,
        formation: "loc_224E8",
        clears_ability: true,
        effect: NO_EFFECT,
    },
];

/// `AbilityEffect_None`'s index, the effect byte both records carry.
const NO_EFFECT: u8 = 0x1F;

/// The reload arm `enemy` runs for `skill`, if it is one of [`RELOADS`] and the
/// record is the traced `AbilityEffect_None` one.
fn reload_of(enemy: u16, skill: &super::EnemySkill) -> Option<&'static Reload> {
    RELOADS
        .iter()
        .find(|r| r.carrier == enemy && r.ability == skill.id)
        .filter(|r| skill.effect == r.effect)
}

/// Whether [`resolve_fusion`] runs `skill` for `enemy`.
pub(super) fn owns(enemy: u16, skill: &super::EnemySkill) -> bool {
    reload_of(enemy, skill).is_some()
}

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
    let Some(skill) = data.enemy_skill(ability) else {
        return Ok(None);
    };
    let Some(reload) = reload_of(caster.stats.enemy_id, skill) else {
        return Ok(None);
    };
    let formation = data.inline_formation(reload.formation)?;
    if formation.enemies.is_empty() {
        return Ok(None);
    }
    let records = formation
        .enemies
        .iter()
        .map(|seat| data.enemy(seat.enemy_id))
        .collect::<Result<Vec<_>, _>>()?;
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
    let mut removed: Vec<FighterId> = roster.side(Side::Enemy).map(|f| f.id).collect();
    roster.clear_enemies();
    // `loc_14D46` seats every pair of the record in its slot; the first event
    // carries the fighters the reload removed.
    for (seat, record) in formation.enemies.iter().zip(records) {
        let fighter = roster
            .add_enemy(seat.slot, record)
            .expect("the side was just emptied");
        debug_assert_eq!(Stats::from_enemy(record).curr_hp, record.hp);
        events.push(BattleEvent::EnemiesFused {
            actor,
            removed: std::mem::take(&mut removed),
            fighter,
            enemy_id: record.id,
            position: seat.position,
            name: record.name.clone(),
            hp: record.hp,
            agility: record.agility,
        });
    }
    Ok(Some(formation.run_chance))
}
