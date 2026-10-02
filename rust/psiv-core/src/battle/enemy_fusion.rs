//! Fusion: two Zol slugs become one MetaSlug.
//!
//! `EnemyAI_ZolSlugs` (arm `$06` of `EnemyAIInstructionsOffs`, see
//! [`super::enemy_ai`]) replaces the slug's rolled ability with the record's
//! conditional ability `$12` when every occupied enemy slot holds a ZolSlug and
//! there are exactly two of them. `EnemyAttack_Blob` (`ps4.asm:23043`) sends a
//! nonzero ability to `loc_10796`, which loads `BattleObj_Fusion`
//! (`ps4.asm:35675`) and `BattleObj_Fusion2` (`ps4.asm:35850`) and clears
//! `Current_Target_Index`. The objects slide the two sprites together and, at
//! their frame `$14` (`ps4.asm:35817`):
//!
//! ```text
//!     clear the 32 objects at $FFFFD800 and the four Fighter_Enemy_n words
//!                                                  (ps4.asm:35820-35831)
//!     lea (loc_1A2F4).l, a0 / lea (Enemy_Formation_Data).l, a1 / trap #1
//!                                                  (ps4.asm:35832-35835)
//!     jsr loc_14D46(pc)                            (ps4.asm:35839)
//!     move.w #$16, (Battle_Routine).l              (ps4.asm:35841)
//! ```
//!
//! `loc_1A2F4` (`ps4.asm:35846-35847`) is ten bytes of formation data - `00 00 00
//! 00 01 01 00 24 14 FF`: ambush 0, run 0, drop rate 0, drop item none, **one**
//! enemy, enemy id `$24` (36, MetaSlug) at position `$14` - which `loc_14D46`
//! (`ps4.asm:29735`) feeds through `loc_7F22` -> `Battle_FillEnemyStats`
//! (`ps4.asm:11939`) exactly as the battle's own load does. So whatever the two
//! slugs' HP was, and whichever slots they stood in, the fight continues against
//! one full-HP MetaSlug in slot 1. The header's run and drop bytes equal the
//! slugs' own formations' (`0xD2`-`0xD4`: run 0, drop rate 0, no item), so
//! nothing about running or drops changes.
//!
//! Nothing in the chain draws from the RNG (no `UpdateRNGSeed2` site between the
//! two objects' labels), and the record's own range and hit bytes are never
//! read: the effect byte `$1F` is `AbilityEffect_None` and no object calls
//! `GetEnemySkillEffectAndRange`.

use super::{BattleData, BattleDataError, BattleEvent, FighterId, Roster, Side, Stats};

/// `EnemyAttackOffs` entry `$22`: 34 ZolSlug, the only enemy whose conditional
/// ability is Fusion.
const ZOL_SLUG: u16 = 34;
/// The conditional ability id the arm writes.
const FUSION: u8 = 0x12;
/// `loc_1A2F4`'s enemy byte: `$24`, MetaSlug.
const META_SLUG: u16 = 0x24;

/// Fusion, if `ability` is it and `actor` is a Zol slug.
///
/// # Errors
/// [`BattleDataError::UnknownEnemy`] when the pack has no MetaSlug record.
pub(super) fn resolve_fusion(
    roster: &mut Roster,
    actor: FighterId,
    ability: u8,
    data: &BattleData,
    events: &mut Vec<BattleEvent>,
) -> Result<bool, BattleDataError> {
    let Some(skill) = data.enemy_skill(ability).filter(|s| {
        // Record 18: `1F 00 22 24 00 00 00 00` - effect `$1F`, no stat, and the
        // two ids the object compares and loads (`$22` ZolSlug, `$24`).
        s.id == FUSION
            && s.effect == 0x1F
            && s.power_stat == 0
            && s.target == 0x22
            && s.power == 0x24
            && s.resistance == 0
            && s.element == 0
    }) else {
        return Ok(false);
    };
    if !roster
        .get(actor)
        .is_some_and(|f| f.is_alive() && f.stats.enemy_id == ZOL_SLUG)
    {
        return Ok(false);
    }
    let record = data.enemy(META_SLUG)?;
    let removed: Vec<FighterId> = roster.side(Side::Enemy).map(|f| f.id).collect();
    events.push(BattleEvent::EnemySkillUsed {
        actor,
        skill: ability,
        name: skill.name.clone(),
    });
    roster.clear_enemies();
    let fighter = roster
        .add_enemy(1, record)
        .expect("slot 1 was just emptied");
    debug_assert_eq!(Stats::from_enemy(record).curr_hp, record.hp);
    events.push(BattleEvent::EnemiesFused {
        actor,
        removed,
        fighter,
        enemy_id: record.id,
        name: record.name.clone(),
        hp: record.hp,
        agility: record.agility,
    });
    Ok(true)
}
