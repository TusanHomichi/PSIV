//! Resolving one fighter's action.
//!
//! The order of rolls inside an action is part of the behaviour, not an
//! implementation detail. The cartridge does **every hit roll first**
//! (`loc_B6A2`, `$00B6A2`, fills all nine `Fighters_Hit_Flags` before anything
//! is damaged) and only then walks the targets doing damage
//! (`Fighter_TakeDamage` per fighter). This module keeps that order, so a
//! multi-target swing draws two hit rolls and then two sixteen-draw damage
//! rolls, never interleaved.
//!
//! # Two attackers draw the hit pass twice
//!
//! `Character_Attack` (`ps4.asm:13018`) runs `loc_B6A2` as the Attack command
//! initialises, and for Alys and Kyra that is not the pass that counts: their
//! action routine `CharAttack_AlysKyra` (`ps4.asm:13958`) dispatches
//! `AlysKyraAttack_Init` (`ps4.asm:13975`), whose first instruction is a second
//! `jsr loc_B6A2` (`ps4.asm:13976`), in the same frame and still before any
//! damage. `loc_B6A2` presets all nine flags to `$FF` and re-fills them, so the
//! **second** pass's verdicts are the ones `Fighter_TakeDamage`
//! (`ps4.asm:3564-3571`) reads. The first pass is drawn and thrown away — see
//! [`takes_second_hit_pass`] for the attacker set and [`SECOND_HIT_PASS_CHARACTERS`]
//! for why it is exactly those two.

use super::chances::{PHYSICAL, Verdict, calculate_chances};
use super::damage::{calculate_damage, clamp_damage};
use super::event::{BattleEvent, Skipped};
use super::fighters::{FighterId, Roster, Side};
use super::records::{BattleData, BattleDataError, ItemKind};
use super::rng::Rolls;
use super::stats::Stats;
use super::tables::WEAPON_ELEMENT_SENTINEL;

#[cfg(test)]
#[path = "attack_status_tests.rs"]
mod status_tests;

#[cfg(test)]
#[path = "action_second_pass_tests.rs"]
mod second_pass_tests;

/// How far an attack reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    /// One enemy — weapon types 1 and 3, and every enemy's plain attack.
    Single,
    /// Every living enemy — weapon types 2 and 4.
    All,
}

/// `Battle_AttackCommand`'s weapon check — `ps4.asm:2235`.
///
/// The right hand decides unless it holds something that is not a weapon, in
/// which case the left hand does; if neither is a weapon there is no Attack
/// command at all. A shield contributes its bonuses and nothing else.
///
/// # Errors
/// [`BattleDataError::UnknownItem`] when a hand holds an id the data does not
/// define.
pub fn weapon_reach(stats: &Stats, data: &BattleData) -> Result<Option<Reach>, BattleDataError> {
    for hand in [stats.equipment[0], stats.equipment[1]] {
        if hand == 0 {
            continue;
        }
        let kind = data.item(hand)?.kind;
        if kind.is_multi_target() {
            return Ok(Some(Reach::All));
        }
        if kind.is_weapon() {
            return Ok(Some(Reach::Single));
        }
        // Not a weapon: fall through to the other hand, exactly as the two
        // `loc_1618` / `loc_1682` branches do.
    }
    Ok(None)
}

/// The element factor a character's swing presents against `target`.
///
/// `Character_DamageEnemy` (`$00277A`) reads each hand's weapon element, looks
/// the target's resistance to it up, and keeps the **larger** of the two — so
/// dual-wielding takes whichever element the target is worse against.
///
/// # A ratified bug fix
///
/// Retail reads a *shield's* element byte as if it were a weapon's, so the
/// off-hand shield can change what element your sword swings with. That is the
/// shield-element read in `docs/RUNTIME_DESIGN.md` "Battle bug policy", listed
/// there as a fix rather than a quirk to keep. Shields are skipped here.
///
/// # Errors
/// [`BattleDataError::UnknownItem`] for an undefined equipment id.
pub fn character_element_factor(
    attacker: &Stats,
    target: &Stats,
    data: &BattleData,
) -> Result<u16, BattleDataError> {
    let mut factor = 0u16;
    for hand in [attacker.equipment[0], attacker.equipment[1]] {
        if hand == 0 {
            continue;
        }
        let item = data.item(hand)?;
        if item.kind == ItemKind::Shield {
            continue;
        }
        let hand_factor = u16::from(target.element_factor(item.element).unwrap_or(0));
        factor = factor.max(hand_factor);
    }
    Ok(factor)
}

/// The element factor an ability presents, resolving the physical-skill
/// fallback.
///
/// An ability record's byte 5 is normally an element id the target resists
/// directly. At [`WEAPON_ELEMENT_SENTINEL`] and above it means "this is a
/// physical skill — use the attacker's weapons instead", and `loc_26F8`
/// (`ps4.asm:3814`) then runs the same larger-of-the-two-hands rule that a
/// plain attack does. Crosscut is the obvious example: element `$10`, so a
/// Crosscut swung with a fire sword is a fire attack.
///
/// This is the function to call when an element id came out of an ability
/// record. [`element_offset`](crate::battle::element_offset) deliberately
/// refuses the sentinel rather than guessing.
///
/// # Errors
/// [`BattleDataError::UnknownItem`] for an undefined equipment id.
pub fn ability_element_factor(
    attacker: &Stats,
    target: &Stats,
    element_id: u8,
    data: &BattleData,
) -> Result<u16, BattleDataError> {
    if element_id >= WEAPON_ELEMENT_SENTINEL {
        return character_element_factor(attacker, target, data);
    }
    Ok(u16::from(target.element_factor(element_id).unwrap_or(0)))
}

/// The element factor an enemy's plain attack presents.
///
/// The enemy record's attack element lives in the `curr_tp` slot
/// (`ps4.asm:11952`), which is why [`Stats::curr_tp`] means something different
/// on the two sides.
#[must_use]
pub fn enemy_element_factor(attacker: &Stats, target: &Stats) -> u16 {
    let element = attacker.curr_tp as u8;
    u16::from(target.element_factor(element).unwrap_or(0))
}

/// The bonus a critical hit adds: `move.b d1, d4 / lsr.w #2, d4`.
///
/// Note it is the **attack power**, not the damage, that gets quartered — and
/// the damage formula then doubles it before the element multiply.
#[must_use]
pub const fn critical_bonus(attack: u16) -> u16 {
    attack >> 2
}

/// Who an attack lands on, and what the hit roll said about each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitPass {
    /// One entry per target, in id order.
    pub verdicts: Vec<(FighterId, Verdict)>,
}

/// `loc_B6A2` (`$00B6A2`) — roll the hit flags for every target.
///
/// One [`calculate_chances`] roll per living target: the actor's
/// `dexterity_battle` against the target's `agility_battle`, with the physical
/// constants. Absent or dead slots are written off as `$FF` **without** a roll,
/// which is why a multi-target swing costs one roll per surviving enemy rather
/// than a fixed four.
///
/// A multi-target swing demotes a rolled critical to a normal hit
/// (`loc_B754`): `smi ($FFFFEE49).w` records that the target index was
/// negative and the demotion keys off it.
pub fn roll_hits(
    roster: &Roster,
    actor: FighterId,
    targets: &[FighterId],
    multi_target: bool,
    rolls: &mut impl Rolls,
) -> HitPass {
    let dexterity = roster.get(actor).map_or(0, |f| f.stats.dexterity.battle);
    let (scale, miss, crit) = PHYSICAL;

    let mut verdicts = Vec::with_capacity(targets.len());
    for target in targets {
        let Some(fighter) = roster.get(*target) else {
            continue;
        };
        if fighter.stats.is_out() {
            continue;
        }
        let verdict = calculate_chances(
            i16::from(dexterity),
            i16::from(fighter.stats.agility.battle),
            scale,
            miss,
            crit,
            rolls,
        );
        let verdict = if multi_target && verdict == Verdict::Critical {
            Verdict::Normal
        } else {
            verdict
        };
        verdicts.push((*target, verdict));
    }
    HitPass { verdicts }
}

/// The `Character_Stats` indices whose Attack animation re-runs the hit pass.
///
/// `Character_AttackActionOffs` (`ps4.asm:13056-13068`) is indexed by
/// `fighter_id - 1` and dispatched at `ps4.asm:13044-13050`. Two of its eleven
/// entries point at `CharAttack_AlysKyra`: the second (Alys, `fighter_id` 2)
/// and the tenth (Kyra, `fighter_id` 10), which are `Character_Stats` indices
/// 1 and 9. The routine they reach forces `Current_Target_Index` to `$FFFF`
/// (`ps4.asm:13959`) and calls `AlysKyraAttack_Init`, and it is that
/// call — not the weapon, not the command — that runs `loc_B6A2` a second time.
///
/// Nothing else can reach it. A technique, skill, item or combo is a different
/// fighter routine (`Character_DoTech`, `Character_DoSkill`, `Character_DoItem`
/// and `Character_DoCombo`, `ps4.asm:1043-1050`), so `Character_Attack` — and
/// with it the first pass — never runs for them; each of those chains runs
/// `loc_B6A2` once (the shared step `loc_9848`, `ps4.asm:14964`). Weapon type
/// picks the reach and which of `AlysKyraAttack_Init`'s animation frames load
/// (`ps4.asm:13986-14002`), never the routine.
pub const SECOND_HIT_PASS_CHARACTERS: [u8; 2] = [1, 9];

/// Whether this attacker's swing draws `loc_B6A2` twice.
///
/// True for Alys and Kyra only (see [`SECOND_HIT_PASS_CHARACTERS`]), whatever
/// weapon they hold: `AlysKyraAttack_Init`'s second `jsr loc_B6A2`
/// (`ps4.asm:13976`) is unconditional. Enemies are never party characters, so a
/// plain enemy attack keeps its single pass (`Enemy_Attack` → `loc_D09E`,
/// `ps4.asm:19138`, `ps4.asm:19200-19201`).
#[must_use]
pub fn takes_second_hit_pass(roster: &Roster, actor: FighterId) -> bool {
    roster
        .get(actor)
        .and_then(|fighter| fighter.character)
        .is_some_and(|character| SECOND_HIT_PASS_CHARACTERS.contains(&character))
}

/// Which living fighters an attack from `actor` can reach.
#[must_use]
pub fn candidate_targets(
    roster: &Roster,
    actor: FighterId,
    intended: Option<FighterId>,
    reach: Reach,
) -> Vec<FighterId> {
    let opposing = actor.side().opposing();
    match reach {
        Reach::All => roster.living(opposing).map(|f| f.id).collect(),
        Reach::Single => {
            // The cursor holds whatever was chosen at command time; if that
            // fighter has died since, the swing falls on the first survivor.
            let still_valid = intended.filter(|id| {
                id.side() == opposing
                    && roster
                        .get(*id)
                        .is_some_and(super::fighters::Fighter::is_alive)
            });
            still_valid
                .or_else(|| roster.first_living(opposing))
                .into_iter()
                .collect()
        }
    }
}

/// Resolves one attack end to end, mutating the roster.
///
/// Appends to `events` and returns the fighters that died, in resolution order.
///
/// A vehicle fighter's Attack is command 6, whose routine is `loc_AF9C` rather
/// than `Character_Attack`; it resolves through
/// [`resolve_vehicle_attack`](super::vehicle_attack::resolve_vehicle_attack) and
/// never reaches the weapon check below. See that module for the rule.
///
/// # Errors
/// Propagates equipment lookups.
pub fn resolve_attack(
    roster: &mut Roster,
    actor: FighterId,
    intended: Option<FighterId>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Result<Vec<FighterId>, BattleDataError> {
    if actor.side() == Side::Party
        && roster
            .get(actor)
            .is_some_and(super::vehicle_attack::is_vehicle_fighter)
    {
        return Ok(super::vehicle_attack::resolve_vehicle_attack(
            roster, actor, intended, rolls, events,
        ));
    }
    let reach = match actor.side() {
        Side::Party => {
            let stats = &roster.get(actor).expect("the actor is present").stats;
            match weapon_reach(stats, data)? {
                Some(reach) => reach,
                None => {
                    events.push(BattleEvent::TurnSkipped {
                        actor,
                        reason: Skipped::Unarmed,
                    });
                    return Ok(Vec::new());
                }
            }
        }
        // Tier 1 enemies only ever throw a plain single-target attack; the
        // multi-target abilities arrive with the ability dispatch in Tier 2.
        Side::Enemy => Reach::Single,
    };

    let targets = candidate_targets(roster, actor, intended, reach);
    if targets.is_empty() {
        events.push(BattleEvent::TurnSkipped {
            actor,
            reason: Skipped::NoTarget,
        });
        return Ok(Vec::new());
    }

    events.push(BattleEvent::Attacked {
        actor,
        targets: targets.clone(),
    });

    let multi = reach == Reach::All;
    // `Character_Attack` runs the pass for every attacker; Alys's and Kyra's
    // animation routine then runs it again and overwrites all nine flags, so
    // the first pass is drawn for its rolls alone. Both passes are one roll per
    // living target (`loc_B716`, `ps4.asm:17536-17554`), and they come before
    // any damage draw.
    let first = roll_hits(roster, actor, &targets, multi, rolls);
    let pass = if takes_second_hit_pass(roster, actor) {
        roll_hits(roster, actor, &targets, multi, rolls)
    } else {
        first
    };

    let attack = roster.get(actor).map_or(0, |f| f.stats.attack.battle);
    let mut died = Vec::new();
    let mut hit_targets = Vec::new();

    for (target, verdict) in pass.verdicts {
        if verdict == Verdict::Miss {
            let remaining = roster.get(target).map_or(0, |f| f.stats.curr_hp);
            events.push(BattleEvent::Resolved {
                actor,
                target,
                verdict,
                damage: None,
                remaining_hp: remaining,
            });
            continue;
        }
        hit_targets.push(target);

        let element = {
            let attacker_stats = &roster.get(actor).expect("actor present").stats;
            let target_stats = &roster.get(target).expect("target present").stats;
            match actor.side() {
                Side::Party => character_element_factor(attacker_stats, target_stats, data)?,
                Side::Enemy => enemy_element_factor(attacker_stats, target_stats),
            }
        };
        let defence = roster.get(target).map_or(0, |f| f.stats.defence.battle);
        let bonus = if verdict == Verdict::Critical {
            critical_bonus(attack)
        } else {
            0
        };

        let raw = calculate_damage(attack, defence, element, bonus, rolls);
        let damage = clamp_damage(raw);

        let fighter = roster.get_mut(target).expect("target present");
        // `sub.w d1, curr_hp(a0)` — the stored value is allowed to go negative,
        // which is why a comparator reading enemy HP unsigned sees 65534.
        // Saturating here reports what a player sees; the death test is the
        // cartridge's `tst.w / bgt`, i.e. "not strictly positive".
        let signed = fighter.stats.curr_hp as i32 - i32::from(damage);
        fighter.stats.curr_hp = signed.max(0) as u16;
        let remaining = fighter.stats.curr_hp;
        let killed = signed <= 0 && !fighter.stats.is_out();
        if killed {
            fighter.mark_defeated();
        }

        events.push(BattleEvent::Resolved {
            actor,
            target,
            verdict,
            damage: Some(damage),
            remaining_hp: remaining,
        });
        if killed {
            events.push(BattleEvent::Died { fighter: target });
            died.push(target);
        }
    }

    // Battle_DoAttackEffect runs after all damage/death reactions. A plain
    // enemy attack can inflict poison or paralysis; every extracted nonzero
    // attack_status uses one of these two effects. Skills take another path.
    if actor.side() == Side::Enemy {
        for target in hit_targets {
            resolve_enemy_attack_status(roster, actor, target, rolls, events);
        }
    }

    Ok(died)
}

fn resolve_enemy_attack_status(
    roster: &mut Roster,
    actor: FighterId,
    target: FighterId,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) {
    use super::stats::status;
    let Some(attacker) = roster.get(actor) else {
        return;
    };
    let effect = attacker.stats.max_tp;
    let bit = match effect {
        27 => status::POISONED,
        28 => status::PARALYZED,
        _ => return,
    };
    let strength = i16::from(attacker.stats.strength.battle);
    let Some(fighter) = roster.get_mut(target).filter(|f| f.is_alive()) else {
        return;
    };
    if fighter.stats.status & bit != 0 {
        return;
    }
    // AbilityEffect_Poison/Paralyze both pass $48 (poison property).
    // Effect_DoPhysicalAttack compares strength against strength, miss <=$70.
    let verdict = calculate_chances(
        strength,
        i16::from(fighter.stats.strength.battle),
        i16::from(fighter.stats.element_factor(13).unwrap_or(0)),
        0x70,
        effect as i16,
        rolls,
    );
    if verdict == Verdict::Miss {
        return;
    }
    fighter.stats.status |= bit;
    if bit == status::PARALYZED {
        fighter.stats.status &= !status::ASLEEP;
        fighter.stats.agility.battle = 1;
        fighter.stats.dexterity.battle = 1;
    }
    events.push(BattleEvent::StatusInflicted {
        actor,
        target,
        status: bit,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::damage::DAMAGE_DRAWS;
    use crate::battle::fixtures;
    use crate::battle::rng::SliceRolls;

    fn party_and_enemies() -> (Roster, BattleData) {
        let data = fixtures::data();
        let items = fixtures::items();
        let lookup = |id: u8| items.iter().find(|i| i.id == id).cloned();
        let mut roster = Roster::new();
        // Seated under the record's own `Character_Stats` index, which is the
        // identity the second-pass rule reads: Alys is 1, and a test that
        // seated her anywhere else would model an attacker the cartridge has
        // no attack animation for.
        for record in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()] {
            roster.add_party_member(
                record.id,
                record.name.clone(),
                Stats::from_character(&record, lookup),
            );
        }
        roster.add_enemy(1, &fixtures::zoran_bult());
        roster.add_enemy(2, &fixtures::zoran_bult());
        (roster, data)
    }

    fn id(n: u8) -> FighterId {
        FighterId::new(n).expect("a valid id")
    }

    #[test]
    fn the_boomerang_reaches_everyone_and_the_hunt_knives_do_not() {
        let (roster, data) = party_and_enemies();
        let alys = &roster.get(id(1)).expect("Alys").stats;
        let chaz = &roster.get(id(2)).expect("Chaz").stats;
        assert_eq!(weapon_reach(alys, &data), Ok(Some(Reach::All)));
        assert_eq!(weapon_reach(chaz, &data), Ok(Some(Reach::Single)));
    }

    #[test]
    fn a_shield_in_the_off_hand_does_not_decide_the_reach() {
        // Hahn holds a Dagger and a Leather Shield. The right hand is a weapon,
        // so it decides; the shield never gets a say.
        let (roster, data) = party_and_enemies();
        let hahn = &roster.get(id(3)).expect("Hahn").stats;
        assert_eq!(hahn.equipment[1], 10, "a shield in the left hand");
        assert_eq!(weapon_reach(hahn, &data), Ok(Some(Reach::Single)));
    }

    #[test]
    fn shields_only_means_no_attack_command() {
        let (mut roster, data) = party_and_enemies();
        let hahn = roster.get_mut(id(3)).expect("Hahn");
        hahn.stats.equipment[0] = 10; // both hands shields
        assert_eq!(weapon_reach(&hahn.stats, &data), Ok(None));

        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        let died = resolve_attack(&mut roster, id(3), None, &data, &mut rolls, &mut events);
        assert_eq!(died, Ok(Vec::new()));
        assert_eq!(
            events,
            vec![BattleEvent::TurnSkipped {
                actor: id(3),
                reason: Skipped::Unarmed
            }]
        );
        assert_eq!(rolls.drawn(), 0, "an unarmed turn costs no rolls");
    }

    #[test]
    fn the_left_hand_decides_when_the_right_is_not_a_weapon() {
        let (mut roster, data) = party_and_enemies();
        let alys = roster.get_mut(id(1)).expect("Alys");
        alys.stats.equipment = [10, 3, 6, 4]; // shield right, Boomerang left
        assert_eq!(weapon_reach(&alys.stats, &data), Ok(Some(Reach::All)));
    }

    #[test]
    fn a_multi_target_swing_rolls_both_hit_passes_then_damages_each() {
        let (mut roster, data) = party_and_enemies();
        // Roll 40: (40 + 13 - 6) * 2 = 94, a normal hit for Alys against both,
        // and she rolls the pass twice — once from Character_Attack and once
        // from AlysKyraAttack_Init. Then two sixteen-draw damage rolls.
        let hits = [40u16; 4];
        let holds = [7u16; 2 * DAMAGE_DRAWS];
        let draws: Vec<u16> = hits.iter().chain(holds.iter()).copied().collect();
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        let died = resolve_attack(&mut roster, id(1), None, &data, &mut rolls, &mut events)
            .expect("resolves");
        assert!(died.is_empty());
        assert_eq!(
            rolls.drawn(),
            4 + 2 * DAMAGE_DRAWS,
            "two hits and two more hits, then two damages"
        );

        let attacked = events
            .iter()
            .find_map(|e| match e {
                BattleEvent::Attacked { targets, .. } => Some(targets.clone()),
                _ => None,
            })
            .expect("an Attacked event");
        assert_eq!(attacked, vec![id(6), id(7)], "both enemies, in id order");

        let resolutions: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Resolved { .. }))
            .collect();
        assert_eq!(resolutions.len(), 2);
    }

    #[test]
    fn a_multi_target_swing_can_never_crit() {
        // Alys dexterity 13 against agility 6 is a margin of +7: roll 63 gives
        // (63 + 7) * 2 = 140, well past $74, so single-target would crit.
        let (mut roster, data) = party_and_enemies();
        let single = roll_hits(&roster, id(1), &[id(6)], false, &mut SliceRolls::new(&[63]));
        assert_eq!(single.verdicts, vec![(id(6), Verdict::Critical)]);

        let multi = roll_hits(
            &roster,
            id(1),
            &[id(6), id(7)],
            true,
            &mut SliceRolls::new(&[63]),
        );
        assert_eq!(
            multi.verdicts,
            vec![(id(6), Verdict::Normal), (id(7), Verdict::Normal)],
            "the demotion in loc_B754"
        );

        // And it shows in the damage: no critical bonus. Four hit rolls,
        // because her animation runs the pass twice, then two damage runs.
        let hits = [63u16; 4];
        let holds = [3u16; 2 * DAMAGE_DRAWS];
        let draws: Vec<u16> = hits.iter().chain(holds.iter()).copied().collect();
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        resolve_attack(&mut roster, id(1), None, &data, &mut rolls, &mut events).expect("resolves");
        assert_eq!(rolls.drawn(), 4 + 2 * DAMAGE_DRAWS);
        assert!(events.iter().all(|e| !matches!(
            e,
            BattleEvent::Resolved {
                verdict: Verdict::Critical,
                ..
            }
        )));
    }

    #[test]
    fn a_dead_slot_costs_neither_a_roll_nor_a_verdict() {
        let (mut roster, data) = party_and_enemies();
        roster.get_mut(id(6)).expect("enemy 1").stats.status = crate::battle::stats::status::DEAD;
        let mut rolls = SliceRolls::new(&[40]);
        let pass = roll_hits(&roster, id(1), &[id(6), id(7)], true, &mut rolls);
        assert_eq!(pass.verdicts.len(), 1, "only the survivor rolls");
        assert_eq!(rolls.drawn(), 1);

        // And the target set skips it in the first place.
        assert_eq!(
            candidate_targets(&roster, id(1), None, Reach::All),
            vec![id(7)]
        );
        let _ = data;
    }

    #[test]
    fn a_single_target_swing_falls_through_to_a_survivor() {
        let (mut roster, data) = party_and_enemies();
        roster.get_mut(id(6)).expect("enemy 1").stats.status = crate::battle::stats::status::DEAD;
        // The cursor still points at the corpse; the swing lands on the next.
        assert_eq!(
            candidate_targets(&roster, id(2), Some(id(6)), Reach::Single),
            vec![id(7)]
        );
        // With nobody left it reaches nothing at all.
        roster.get_mut(id(7)).expect("enemy 2").stats.status = crate::battle::stats::status::DEAD;
        assert!(candidate_targets(&roster, id(2), Some(id(6)), Reach::Single).is_empty());

        let mut events = Vec::new();
        resolve_attack(
            &mut roster,
            id(2),
            Some(id(6)),
            &data,
            &mut SliceRolls::new(&[0]),
            &mut events,
        )
        .expect("resolves");
        assert_eq!(
            events,
            vec![BattleEvent::TurnSkipped {
                actor: id(2),
                reason: Skipped::NoTarget
            }]
        );
    }

    #[test]
    fn an_attack_cannot_reach_its_own_side() {
        let (roster, _) = party_and_enemies();
        for target in candidate_targets(&roster, id(1), None, Reach::All) {
            assert_eq!(target.side(), Side::Enemy);
        }
        for target in candidate_targets(&roster, id(6), None, Reach::All) {
            assert_eq!(target.side(), Side::Party);
        }
        // A cursor pointing at a friend is discarded rather than obeyed.
        assert_eq!(
            candidate_targets(&roster, id(1), Some(id(2)), Reach::Single),
            vec![id(6)]
        );
    }

    #[test]
    fn a_shields_element_is_not_read_as_a_weapons() {
        // The ratified fix. Hahn's Dagger is element 1 (physical) and his
        // shield is element 0; retail would fold the shield in.
        let (roster, data) = party_and_enemies();
        let hahn = &roster.get(id(3)).expect("Hahn").stats;
        let target = &roster.get(id(6)).expect("enemy").stats;
        assert_eq!(
            character_element_factor(hahn, target, &data),
            Ok(2),
            "the Dagger's physical element against a normal resistance"
        );

        // Making the shield's element one the target is immune to must not
        // change the answer.
        let mut shielded = data.clone();
        let mut shield = data.item(10).expect("the shield").clone();
        shield.element = 3;
        shielded = shielded.with_items([shield]);
        assert_eq!(character_element_factor(hahn, target, &shielded), Ok(2));
    }

    #[test]
    fn dual_wielding_keeps_the_better_element() {
        let (mut roster, data) = party_and_enemies();
        // Give the enemy a weakness to fire and Chaz one fire knife.
        roster.get_mut(id(6)).expect("enemy").stats.element_props[2] = 4;
        let mut data = data;
        let mut fire_knife = data.item(2).expect("Hunt-Knife").clone();
        fire_knife.id = 20;
        fire_knife.element = 3; // fire
        data = data.with_items([fire_knife]);
        roster.get_mut(id(2)).expect("Chaz").stats.equipment[1] = 20;

        let chaz = &roster.get(id(2)).expect("Chaz").stats;
        let target = &roster.get(id(6)).expect("enemy").stats;
        assert_eq!(
            character_element_factor(chaz, target, &data),
            Ok(4),
            "physical 2 against fire 4 — the larger wins"
        );
    }

    #[test]
    fn an_enemy_swings_with_the_element_in_its_tp_slot() {
        let (roster, _) = party_and_enemies();
        let enemy = &roster.get(id(6)).expect("enemy").stats;
        let chaz = &roster.get(id(2)).expect("Chaz").stats;
        assert_eq!(enemy.curr_tp, 1, "physical");
        assert_eq!(enemy_element_factor(enemy, chaz), 2);

        // Chaz is immune to holyword; an enemy that swung with it would be
        // reduced to the 1-damage floor.
        let mut holy = enemy.clone();
        holy.curr_tp = 8;
        assert_eq!(enemy_element_factor(&holy, chaz), 0);
    }

    #[test]
    fn a_critical_bonus_is_a_quarter_of_the_attack_power() {
        assert_eq!(critical_bonus(18), 4, "Chaz");
        assert_eq!(critical_bonus(13), 3, "Alys");
        assert_eq!(critical_bonus(8), 2, "Hahn");
        assert_eq!(critical_bonus(16), 4, "ZoranBult");
        assert_eq!(critical_bonus(3), 0, "rounded down, and it can vanish");
    }

    #[test]
    fn a_kill_marks_the_target_dead_and_reports_it_once() {
        let (mut roster, data) = party_and_enemies();
        roster.get_mut(id(6)).expect("enemy").stats.curr_hp = 1;
        let draws = [40u16, 7];
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        let died = resolve_attack(
            &mut roster,
            id(2),
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events,
        )
        .expect("resolves");
        assert_eq!(died, vec![id(6)]);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, BattleEvent::Died { .. }))
                .count(),
            1
        );
        let enemy = roster.get(id(6)).expect("enemy");
        assert!(!enemy.is_alive());
        assert_eq!(enemy.stats.curr_hp, 0, "reported floored, not negative");
    }

    #[test]
    fn a_miss_reports_no_damage_and_leaves_hp_alone() {
        let (mut roster, data) = party_and_enemies();
        // Chaz dexterity 5 against agility 6: v = (r - 1) * 2, miss for r <= 5.
        let draws = [0u16];
        let mut rolls = SliceRolls::new(&draws);
        let mut events = Vec::new();
        resolve_attack(
            &mut roster,
            id(2),
            Some(id(6)),
            &data,
            &mut rolls,
            &mut events,
        )
        .expect("resolves");
        assert_eq!(rolls.drawn(), 1, "a miss costs no damage draws");
        assert_eq!(roster.get(id(6)).expect("enemy").stats.curr_hp, 25);
        assert!(events.iter().any(|e| matches!(
            e,
            BattleEvent::Resolved {
                verdict: Verdict::Miss,
                damage: None,
                ..
            }
        )));
    }
}
