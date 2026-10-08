//! Reading a [`Board`] off the runtime: the live roster, the battle records
//! and the pack, through `&Runtime` only.
//!
//! The actor's techniques and skills are the open window's own entries
//! ([`CommandMenuView`]). The other members' windows are not open yet, so their
//! kits follow the window's rule (`session/battle/menu/commands.rs`,
//! `CommandMenu::view`): a supported technique the member has the TP for and is
//! not sealed from, a supported skill with a use left and a weapon in hand when
//! the record wants one. Items are the pack's copies and the member's own
//! equipment; a consumable is used from the pack only (`resolve_item` refuses
//! an equipped copy). Targets are the core's own lists
//! (`technique_targets`, `skill_targets`, `item_targets`).

use psiv_core::battle::{
    BattleData, FighterId, ItemKind, Reach, Roster, Side, Stats, item_targets, skill_targets,
    status, technique_targets, weapon_reach,
};
use psiv_runtime::{CommandMenuView, Runtime};

use crate::policy_board::{Ability, Board, Combatant, Kit, Source, Weapon};

impl Board {
    /// The board of the open window, or `None` outside a battle with a roster.
    #[must_use]
    pub fn read(menu: &CommandMenuView, runtime: &Runtime) -> Option<Board> {
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
                let me = Combatant::from_fighter(fighter, None);
                let weapon = weapon(&fighter.stats, data);
                let (techs, skills) = if fighter.id == actor {
                    (
                        menu.techniques
                            .iter()
                            .filter(|e| e.available)
                            .map(|e| e.id)
                            .collect(),
                        menu.skills
                            .iter()
                            .filter(|e| e.available && e.remaining > 0)
                            .map(|e| e.id)
                            .collect(),
                    )
                } else {
                    usable(&fighter.stats, data, weapon.is_some())
                };
                let mut abilities = Vec::new();
                techniques(&techs, roster, data, fighter.id, &me, &mut abilities);
                skill_list(&skills, roster, data, fighter.id, &me, &mut abilities);
                items(
                    &pack,
                    roster,
                    data,
                    fighter.id,
                    &fighter.stats,
                    &mut abilities,
                );
                Kit {
                    id: fighter.id.get(),
                    abilities,
                    weapon,
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
            actor: actor.get(),
            party,
            enemies,
            kits,
            stock,
        })
    }
}

/// The techniques and skills a member's window would enable.
fn usable(stats: &Stats, data: &BattleData, armed: bool) -> (Vec<u8>, Vec<u8>) {
    let techs = stats
        .techniques
        .iter()
        .copied()
        .filter(|id| *id != 0)
        .filter(|id| {
            data.technique(*id).is_some_and(|t| {
                t.supported()
                    && stats.curr_tp >= u16::from(t.cost)
                    && stats.status & status::TECH_SEALED == 0
            })
        })
        .collect();
    let skills = stats
        .skills
        .iter()
        .zip(stats.curr_skill_uses)
        .filter(|(id, uses)| **id != 0 && *uses > 0)
        .filter(|(id, _)| {
            data.skill(**id)
                .is_some_and(|s| s.supported() && (!s.requires_weapon || armed))
        })
        .map(|(id, _)| *id)
        .collect();
    (techs, skills)
}

fn ids(targets: &[FighterId]) -> Vec<u8> {
    targets.iter().map(FighterId::get).collect()
}

fn techniques(
    known: &[u8],
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Combatant,
    out: &mut Vec<Ability>,
) {
    for tech in known.iter().filter_map(|id| data.technique(*id)) {
        out.push(Ability {
            source: Source::Technique(tech.id),
            effect: tech.effect,
            range: tech.targeting & 15,
            // `resolve_technique`: the caster's mental, always.
            power_stat: me.mental,
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
    me: &Combatant,
    out: &mut Vec<Ability>,
) {
    for skill in known.iter().filter_map(|id| data.skill(*id)) {
        out.push(Ability {
            source: Source::Skill(skill.id),
            effect: skill.effect,
            range: skill.targeting & 15,
            // `resolve_skill`: the record's selected stat (VISION's fixed 8
            // has no damage or healing use here).
            power_stat: me.stat(skill.power_stat),
            power: skill.power,
            resistance: skill.resistance,
            element: skill.element,
            tp_cost: 0,
            targets: ids(&skill_targets(roster, actor, skill)),
        });
    }
}

fn items(
    pack: &[u8],
    roster: &Roster,
    data: &BattleData,
    actor: FighterId,
    me: &Stats,
    out: &mut Vec<Ability>,
) {
    let mut seen = Vec::new();
    for id in me
        .equipment
        .iter()
        .chain(pack)
        .copied()
        .filter(|id| *id != 0)
    {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let Some(item) = data.battle_item(id) else {
            continue;
        };
        if !item.supported() || (item.consumable && !pack.contains(&id)) {
            continue;
        }
        out.push(Ability {
            source: Source::Item {
                id,
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
/// `weapon_reach`) and the elements the hands carry, shields skipped
/// (`Character_DamageEnemy`'s larger-of-two-hands rule, `ps4.asm:3910`).
fn weapon(me: &Stats, data: &BattleData) -> Option<Weapon> {
    let reach = weapon_reach(me, data).ok().flatten()?;
    let elements = me.equipment[..2]
        .iter()
        .filter(|id| **id != 0)
        .filter_map(|id| data.item(*id).ok())
        .filter(|item| item.kind != ItemKind::Shield)
        .map(|item| item.element)
        .collect();
    Some(Weapon {
        elements,
        all: reach == Reach::All,
    })
}
