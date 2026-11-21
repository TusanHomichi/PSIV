//! Combination attacks (`ComboData`, `ComboSetupData`, `Character_DoCombo`,
//! `loc_27DE7C`, `loc_2864`, `loc_5FB4`).
//!
//! Before each living party member executes in `Battle_Turn_Order`, retail's
//! `loc_5B8E` jumps to `loc_27DE7C` (`ps4.asm:315760`), which peeks up to four
//! consecutive ready party Tech/Skill turns and checks them against the 14
//! `ComboSetupData` entries. On a match, all participating turns collapse into
//! one `Character_DoCombo` (`ps4.asm:16493`): every participant pays their
//! Tech TP or Skill use, and the combo resolves either summed damage
//! (`loc_2864`, `ps4.asm:4046`) or an instant-death chance roll (`loc_5FB4`,
//! `ps4.asm:8786`).

use super::action::weapon_reach;
use super::chances::{Verdict, calculate_chances};
use super::damage::{calculate_damage, clamp_damage};
use super::engine::{Command, RoundOrders};
use super::event::BattleEvent;
use super::fighters::{FighterId, Roster, Side, reaction};
use super::order::QueueEntry;
use super::records::{BattleData, BattleDataError};
use super::retarget;
use super::rng::Rolls;
use super::stats::status;
use super::technique::stat;

/// Normalized retail combination attack record (`ComboData` + `ComboSetupData` + `ComboNames`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboRecord {
    /// One-based combo id (`1..=14`).
    pub id: u8,
    /// Cartridge display name from `ComboNames`.
    pub name: String,
    /// Byte 0 of `ComboData`: `1` = damage (`loc_2864`), `2` = instant death (`loc_5FB4`).
    pub effect: u8,
    /// Byte 2 of `ComboData`: `1` = single enemy, `2` = all enemies.
    pub range: u8,
    /// Byte 3 of `ComboData`: instant-death threshold (`0x40`), `0` for damage.
    pub power: u8,
    /// Byte 4 of `ComboData`: target resistance stat selector (`1..=7`).
    pub resistance: u8,
    /// Byte 5 of `ComboData`: target element selector (`1..=14`).
    pub element: u8,
    /// Number of consecutive participants (`2`, `3`, or `4`).
    pub participants: u8,
    /// Whether participant sequence order is fixed (`ComboSetupData` bit 7).
    pub ordered: bool,
    /// Valid ability sequences; techniques are `id` (`< 0x80`), skills are `0x80 | id`.
    pub setups: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ComboAbility {
    Technique(u8),
    Skill(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ComboStep {
    pub fighter: FighterId,
    pub ability: ComboAbility,
    pub target: Option<FighterId>,
    pub code: u8,
}

pub(super) struct MatchedCombo<'a> {
    pub combo: &'a ComboRecord,
    pub steps: Vec<ComboStep>,
}

/// `loc_27DE7C` (`ps4.asm:315760`): inspects up to four consecutive ready party
/// turns at the front of `queue_tail` and returns the first matching retail combo.
pub(super) fn detect_combo<'a>(
    roster: &Roster,
    queue_tail: &[QueueEntry],
    orders: &RoundOrders,
    data: &'a BattleData,
) -> Result<Option<MatchedCombo<'a>>, BattleDataError> {
    if data.combos_empty() || queue_tail.len() < 2 {
        return Ok(None);
    }
    let mut steps = Vec::with_capacity(4);
    for entry in queue_tail.iter().take(4) {
        let actor = entry.fighter;
        if actor.side() != Side::Party {
            break;
        }
        let Some(fighter) = roster.get(actor) else {
            break;
        };
        // `andi.b #$CE, d0 / bne loc_27DF78`: dead, android-dead, paralyzed,
        // asleep, or transient stops scanning immediately.
        if !fighter.is_alive() || !fighter.stats.can_act() || fighter.transient {
            break;
        }
        match orders.command_for(actor.slot()) {
            Command::Technique { technique, target } => {
                // `loc_27DEFE`: tech-sealed or insufficient TP stops scanning.
                if fighter.stats.status & status::TECH_SEALED != 0 {
                    break;
                }
                if !fighter.stats.techniques.contains(&technique) {
                    break;
                }
                let Some(tech) = data.technique(technique) else {
                    break;
                };
                if fighter.stats.curr_tp < u16::from(tech.cost) {
                    break;
                }
                steps.push(ComboStep {
                    fighter: actor,
                    ability: ComboAbility::Technique(technique),
                    target,
                    code: technique,
                });
            }
            Command::Skill { skill, target } => {
                // `loc_27DF1E`: must know skill, have uses > 0, and hold a weapon if required.
                let Some(slot) = fighter.stats.skills.iter().position(|&k| k == skill) else {
                    break;
                };
                if fighter.stats.curr_skill_uses[slot] == 0 {
                    break;
                }
                let Some(skill_rec) = data.skill(skill) else {
                    break;
                };
                if skill_rec.requires_weapon && weapon_reach(&fighter.stats, data)?.is_none() {
                    break;
                }
                steps.push(ComboStep {
                    fighter: actor,
                    ability: ComboAbility::Skill(skill),
                    target,
                    code: 0x80 | skill,
                });
            }
            _ => break,
        }
    }

    if steps.len() < 2 {
        return Ok(None);
    }

    for combo in data.combos() {
        let count = usize::from(combo.participants);
        if count < 2 || steps.len() < count {
            continue;
        }
        let codes: Vec<u8> = steps[..count].iter().map(|s| s.code).collect();
        if matches_combo(combo, &codes) {
            return Ok(Some(MatchedCombo {
                combo,
                steps: steps[..count].to_vec(),
            }));
        }
    }

    Ok(None)
}

fn matches_combo(combo: &ComboRecord, codes: &[u8]) -> bool {
    if combo.ordered || combo.participants == 4 {
        return combo.setups.iter().any(|seq| seq.as_slice() == codes);
    }
    match codes {
        [a, b] => combo
            .setups
            .iter()
            .any(|seq| seq.as_slice() == [*a, *b] || seq.as_slice() == [*b, *a]),
        [a, b, c] => {
            let perms = [
                [*a, *b, *c],
                [*a, *c, *b],
                [*b, *a, *c],
                [*b, *c, *a],
                [*c, *a, *b],
                [*c, *b, *a],
            ];
            combo
                .setups
                .iter()
                .any(|seq| perms.iter().any(|perm| seq.as_slice() == perm))
        }
        _ => combo.setups.iter().any(|seq| seq.as_slice() == codes),
    }
}

/// `Character_DoCombo` (`ps4.asm:16493`): deducts every participant's cost,
/// emits [`BattleEvent::ComboUsed`], and resolves damage (`loc_2864`) or
/// instant death (`loc_5FB4`).
pub(super) fn resolve_combo(
    roster: &mut Roster,
    matched: &MatchedCombo<'_>,
    data: &BattleData,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Vec<FighterId> {
    let combo = matched.combo;
    let lead = matched.steps[0].fighter;
    let mut participants = Vec::with_capacity(matched.steps.len());
    let mut tp_after = Vec::new();

    // `loc_ABAA`: charge TP or a skill use for every participant.
    for step in &matched.steps {
        participants.push(step.fighter);
        let fighter = roster.get_mut(step.fighter).expect("participant present");
        match step.ability {
            ComboAbility::Technique(tech_id) => {
                let tech = data.technique(tech_id).expect("validated tech");
                fighter.stats.curr_tp = fighter.stats.curr_tp.saturating_sub(u16::from(tech.cost));
                tp_after.push((step.fighter, fighter.stats.curr_tp));
            }
            ComboAbility::Skill(skill_id) => {
                if let Some(slot) = fighter.stats.skills.iter().position(|&k| k == skill_id) {
                    fighter.stats.curr_skill_uses[slot] =
                        fighter.stats.curr_skill_uses[slot].saturating_sub(1);
                }
            }
        }
    }

    events.push(BattleEvent::ComboUsed {
        actor: lead,
        participants,
        tp_after,
        combo: combo.id,
        name: combo.name.clone(),
    });

    // `loc_57C4` -> `loc_5BDC` -> `loc_5A98` runs on the lead participant's
    // own command before `loc_5B8E` -> `loc_27DE7C` detects the combo. A
    // fallen enemy aim therefore consumes its retarget tie draws even when
    // `loc_AF74` later switches `Current_Target_Index` to `-1` for an
    // all-enemy combo.
    let (lead_range, lead_effect) = match matched.steps[0].ability {
        ComboAbility::Technique(id) => {
            let tech = data.technique(id).expect("validated tech");
            (tech.targeting & 0x0F, tech.effect)
        }
        ComboAbility::Skill(id) => {
            let skill = data.skill(id).expect("validated skill");
            (skill.targeting & 0x0F, skill.effect)
        }
    };
    let retargeted_lead = retarget::ability_target(
        roster,
        matched.steps[0].target,
        lead_range,
        lead_effect,
        rolls,
    );

    let multi = combo.range == 2;
    let targets: Vec<FighterId> = if multi {
        roster.living(Side::Enemy).map(|f| f.id).collect()
    } else {
        let intended = retargeted_lead
            .filter(|id| roster.get(*id).is_some_and(|f| f.is_alive()))
            .or_else(|| roster.living(Side::Enemy).next().map(|f| f.id));
        intended.into_iter().collect()
    };

    let mut died = Vec::new();
    match combo.effect {
        1 => {
            // `loc_2864` (`ps4.asm:4046-4106`): sum attacker stats & ability powers.
            let mut attack_sum: u16 = 0;
            let mut power_sum: u16 = 0;
            let mut has_tech = false;
            for step in &matched.steps {
                let stats = &roster.get(step.fighter).expect("present").stats;
                match step.ability {
                    ComboAbility::Skill(skill_id) => {
                        let skill = data.skill(skill_id).expect("validated skill");
                        // Both byte stats (`moveq #0, d3 / move.b / add.w d3, d1`)
                        // and word stats (`add.w (a0,d2.w), d1`) add at 16-bit width.
                        attack_sum = attack_sum.wrapping_add(stat(stats, skill.power_stat));
                        power_sum = power_sum.wrapping_add(u16::from(skill.power));
                    }
                    ComboAbility::Technique(tech_id) => {
                        let tech = data.technique(tech_id).expect("validated tech");
                        has_tech = true;
                        // `add.b $1D(a0), d1` (`0x0028BC: D228 001D`): 8-bit low-byte add.
                        let lo = (attack_sum as u8).wrapping_add(stats.mental.battle);
                        attack_sum = (attack_sum & 0xFF00) | u16::from(lo);
                        power_sum = power_sum.wrapping_add(u16::from(tech.power));
                    }
                }
            }

            for target in targets {
                if let Some(enemy) = roster.get_mut(target) {
                    enemy.reaction_flags |= reaction::MAGIC
                        | if has_tech { reaction::TECHNIQUE } else { 0 }
                        | if multi { reaction::MULTI_TARGET } else { 0 };
                    let defence = stat(&enemy.stats, combo.resistance);
                    let element = u16::from(enemy.stats.element_factor(combo.element).unwrap_or(0));
                    let damage = clamp_damage(calculate_damage(
                        attack_sum, defence, element, power_sum, rolls,
                    ));
                    enemy.stats.curr_hp = enemy.stats.curr_hp.saturating_sub(damage);
                    events.push(BattleEvent::Resolved {
                        actor: lead,
                        target,
                        verdict: Verdict::Normal,
                        damage: Some(damage),
                        remaining_hp: enemy.stats.curr_hp,
                    });
                    if enemy.stats.curr_hp == 0 {
                        enemy.stats.status |= status::DEAD;
                        events.push(BattleEvent::Died { fighter: target });
                        died.push(target);
                    }
                }
            }
        }
        2 => {
            // `loc_5FB4` (`ps4.asm:8786-8869`): sum attacker stats for instant-death roll.
            let mut attack_sum: u16 = 0;
            for step in &matched.steps {
                let stats = &roster.get(step.fighter).expect("present").stats;
                match step.ability {
                    ComboAbility::Skill(skill_id) => {
                        let skill = data.skill(skill_id).expect("validated skill");
                        let value = stat(stats, skill.power_stat);
                        if skill.power_stat <= 4 {
                            // `add.b (a0,d2.w), d1` (`0x005FFA: D230 2000`): 8-bit low-byte add.
                            let lo = (attack_sum as u8).wrapping_add(value as u8);
                            attack_sum = (attack_sum & 0xFF00) | u16::from(lo);
                        } else {
                            // `add.w (a0,d2.w), d1` (`0x006000: D270 2000`): 16-bit add.
                            attack_sum = attack_sum.wrapping_add(value);
                        }
                    }
                    ComboAbility::Technique(_) => {
                        // `add.b $1D(a0), d1` (`0x00600E: D228 001D`): 8-bit low-byte add.
                        let lo = (attack_sum as u8).wrapping_add(stats.mental.battle);
                        attack_sum = (attack_sum & 0xFF00) | u16::from(lo);
                    }
                }
            }

            for target in targets {
                if let Some(enemy) = roster.get_mut(target) {
                    let defence = stat(&enemy.stats, combo.resistance);
                    let element = i16::from(enemy.stats.element_factor(combo.element).unwrap_or(0));
                    let verdict = calculate_chances(
                        attack_sum as i16,
                        defence as i16,
                        element,
                        i16::from(combo.power),
                        0,
                        rolls,
                    );
                    if verdict != Verdict::Miss {
                        enemy.stats.curr_hp = 0;
                        enemy.stats.status |= status::DEAD;
                        events.push(BattleEvent::Died { fighter: target });
                        died.push(target);
                    } else {
                        events.push(BattleEvent::Resolved {
                            actor: lead,
                            target,
                            verdict: Verdict::Miss,
                            damage: None,
                            remaining_hp: enemy.stats.curr_hp,
                        });
                    }
                }
            }
        }
        _ => {}
    }

    died
}

#[cfg(test)]
#[path = "combo_tests.rs"]
mod tests;
