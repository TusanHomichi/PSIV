//! Reading a [`Board`] off the runtime: the live roster, the battle records
//! and the pack, through `&Runtime` only.
//!
//! Every member's techniques, skills and items are what their command window
//! would enable, asked of the window's own rule (`Runtime::window_techniques`,
//! `window_skills`, `window_items`). The window still lets a sealed member
//! choose a technique (#131); declining that wasted cast is the planner's
//! judgment (`Planner::in_stock`). A consumable is used from the pack only
//! (`resolve_item` refuses an equipped copy). Targets are the core's own lists
//! (`technique_targets`, `skill_targets`, `item_targets`).

use psiv_core::battle::{
    BattleData, FighterId, ItemSource, Reach, Roster, Side, Stats, item_targets, skill_targets,
    technique_targets, weapon_reach,
};
use psiv_runtime::{CommandMenuView, Runtime};

use psiv_core::battle::stat;

use crate::policy_board::{Ability, Board, Combatant, Kit, Source, Weapon};

impl<'r> Board<'r> {
    /// The board of the open window, or `None` outside a battle with a roster.
    #[must_use]
    pub fn read(menu: &CommandMenuView, runtime: &'r Runtime) -> Option<Board<'r>> {
        let actor = FighterId::new(menu.actor?)?;
        let roster = runtime.battle_roster()?;
        let data = runtime.battle_data()?;
        roster.get(actor)?;
        let party = roster
            .side(Side::Party)
            .map(|f| Combatant::from_fighter(f, None))
            .collect();
        let enemies = roster
            .living(Side::Enemy)
            .map(|f| Combatant::from_fighter(f, data.enemy(f.stats.enemy_id).ok()))
            .collect();
        let held = runtime.game().inventory();
        let pack: Vec<u8> = (0..40).filter_map(|slot| held.get(slot)).collect();
        let kits: Vec<Kit> = roster
            .living(Side::Party)
            .map(|fighter| {
                let me = &fighter.stats;
                let techs = runtime.window_techniques(fighter.id);
                let skills = runtime.window_skills(fighter.id);
                let offered = runtime.window_items(fighter.id);
                let mut abilities = Vec::new();
                techniques(&techs, roster, data, fighter.id, me, &mut abilities);
                skill_list(&skills, roster, data, fighter.id, me, &mut abilities);
                items(&offered, roster, data, fighter.id, &mut abilities);
                Kit {
                    id: fighter.id.get(),
                    abilities,
                    weapon: weapon(me, data),
                }
            })
            .collect();
        let mut stock: Vec<(u8, u16)> = Vec::new();
        for ability in kits.iter().flat_map(|k| k.abilities.iter()) {
            if let Source::Item {
                id,
                consumable: true,
                ..
            } = ability.source
                && !stock.iter().any(|(item, _)| *item == id)
            {
                let copies = pack.iter().filter(|held| **held == id).count();
                stock.push((id, u16::try_from(copies).unwrap_or(u16::MAX)));
            }
        }
        Some(Board {
            data,
            actor: actor.get(),
            party,
            enemies,
            kits,
            stock,
        })
    }
}

fn ids(targets: &[FighterId]) -> Vec<u8> {
    targets.iter().map(FighterId::get).collect()
}

fn techniques(
    known: &[u8],
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Stats,
    out: &mut Vec<Ability>,
) {
    for tech in known.iter().filter_map(|id| data.technique(*id)) {
        out.push(Ability {
            source: Source::Technique(tech.id),
            effect: tech.effect,
            range: tech.targeting & 15,
            // `resolve_technique`: the caster's mental, always.
            power_stat: me.mental.battle.into(),
            power: tech.power,
            resistance: tech.resistance,
            element: tech.element,
            tp_cost: tech.cost,
            targets: ids(&technique_targets(roster, actor, tech)),
        });
    }
}

fn skill_list(
    known: &[u8],
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Stats,
    out: &mut Vec<Ability>,
) {
    for skill in known.iter().filter_map(|id| data.skill(*id)) {
        out.push(Ability {
            source: Source::Skill(skill.id),
            effect: skill.effect,
            range: skill.targeting & 15,
            // `resolve_skill`: the record's selected stat (VISION's fixed 8
            // has no damage or healing use here).
            power_stat: stat(me, skill.power_stat),
            power: skill.power,
            resistance: skill.resistance,
            element: skill.element,
            tp_cost: 0,
            targets: ids(&skill_targets(roster, actor, skill)),
        });
    }
}

fn items(
    offered: &[(ItemSource, u8)],
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    out: &mut Vec<Ability>,
) {
    let mut seen = Vec::new();
    for (source, id) in offered {
        if seen.contains(id) {
            continue;
        }
        let Some(item) = data.battle_item(*id) else {
            continue;
        };
        // `resolve_item` refuses an equipped copy of a consumable; the pack's
        // copy of the same item may still be offered later in the list.
        if item.consumable && matches!(source, ItemSource::Equipment(_)) {
            continue;
        }
        seen.push(*id);
        out.push(Ability {
            source: Source::Item {
                id: *id,
                name: item.name.clone(),
                consumable: item.consumable,
            },
            effect: item.effect,
            range: item.targeting,
            power_stat: item.actor_power.into(),
            power: item.power,
            resistance: item.resistance,
            element: item.element,
            tp_cost: 0,
            targets: ids(&item_targets(roster, actor, item)),
        });
    }
}

/// Whether ATTACK is offered (`Battle_AttackCommand`'s weapon check,
/// `weapon_reach`) and whether a swing reaches every enemy.
fn weapon(me: &Stats, data: &BattleData) -> Option<Weapon> {
    let reach = weapon_reach(me, data).ok().flatten()?;
    Some(Weapon {
        all: reach == Reach::All,
    })
}
