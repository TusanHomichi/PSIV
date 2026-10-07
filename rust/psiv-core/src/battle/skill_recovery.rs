//! The skill recovery objects. GetSkillEffectAndRange supplies eligible
//! recipients; animation completion calls the shared healing routine.

use super::*;
use crate::battle::calc_healing;

pub(super) fn resolve(
    roster: &mut Roster,
    actor: FighterId,
    target: FighterId,
    skill: &Skill,
    power: u16,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) {
    let stats = &mut roster.get_mut(target).expect("selected").stats;
    // loc_2ED8 -> loc_2F02 (ps4.asm:4601-4647): one sixteen-draw healing
    // run per recipient, including a full recipient. ATARAXIA branches to
    // loc_2F40 (4649-4656) and writes TP instead of HP.
    let amount = calc_healing(power, u16::from(skill.power), rolls);
    if skill.id == 46 {
        let before = stats.curr_tp;
        stats.curr_tp = stats.curr_tp.wrapping_add(amount).min(stats.max_tp);
        events.push(BattleEvent::TpRestored {
            actor,
            target,
            amount: stats.curr_tp.saturating_sub(before),
            remaining_tp: stats.curr_tp,
        });
        return;
    }
    let before = stats.curr_hp;
    let was_out = stats.is_out();
    stats.curr_hp = stats.curr_hp.wrapping_add(amount).min(stats.max_hp);
    // MEDIC PW includes downed humans, then its child clears status with
    // ANDI.B #$90 (ps4.asm:78736-78761). It does not restore AGI or DEX.
    if skill.id == 45 {
        let removed = stats.status & !status::TECH_SEALED;
        stats.status &= status::TECH_SEALED;
        if removed != 0 {
            events.push(BattleEvent::StatusRestored {
                actor,
                target,
                removed,
            });
        }
    }
    if was_out && !stats.is_out() {
        events.push(BattleEvent::Revived {
            actor,
            target,
            remaining_hp: stats.curr_hp,
        });
    } else {
        events.push(BattleEvent::Healed {
            actor,
            target,
            amount: stats.curr_hp.saturating_sub(before),
            remaining_hp: stats.curr_hp,
        });
    }
}
