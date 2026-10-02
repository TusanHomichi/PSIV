//! The per-character command window: rows, pages, targets and reservations.
//!
//! Split out of `menu.rs` with the S3 node so each file stays reviewable.

use std::collections::BTreeMap;

use psiv_core::battle::{
    BattleItem, Command, FighterId, ItemSource, Roster, RoundOrders, Side, Skill, Technique,
    item_targets, skill_targets, status, technique_targets,
};

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::super::view::{
    CommandMenuView, MenuPage, MenuRow, PartyStatus, SkillEntry, TargetKind, TechniqueEntry,
};
use super::{accept_pressed, list_step};

/// The per-character command window: rows, pages, targets and reservations.
#[derive(Debug)]
pub(crate) struct CommandsMenu {
    roster: Roster,
    techniques: BTreeMap<u8, Technique>,
    skills: BTreeMap<u8, Skill>,
    armed_fighters: Vec<FighterId>,
    items: BTreeMap<u8, BattleItem>,
    inventory: psiv_core::Inventory,
    actors: Vec<FighterId>,
    actor: usize,
    page: MenuPage,
    cursor: usize,
    orders: Vec<Command>,
    /// Whether the window is still open. `false` hands the orders back.
    pub(crate) open: bool,
}

impl CommandsMenu {
    /// Opens the window over the battle's live data.
    ///
    /// A roster the runtime does not have (battles never enabled) opens an
    /// empty window rather than refusing: with no actor to answer it submits
    /// the Defend default, which is what the shell's own menu did with an
    /// empty roster.
    pub(crate) fn new(runtime: &Runtime) -> CommandsMenu {
        let roster = runtime.battle_roster().cloned().unwrap_or_else(Roster::new);
        let actors = roster
            .side(Side::Party)
            .filter(|f| f.is_alive() && f.stats.can_act())
            .map(|f| f.id)
            .collect();
        CommandsMenu {
            roster,
            techniques: runtime
                .battle_techniques()
                .map(|t| (t.id, t.clone()))
                .collect(),
            skills: runtime.battle_skills().map(|s| (s.id, s.clone())).collect(),
            armed_fighters: runtime.battle_armed_fighters(),
            items: runtime
                .battle_items()
                .map(|item| (item.id, item.clone()))
                .collect(),
            inventory: runtime.game().inventory().clone(),
            actors,
            actor: 0,
            page: MenuPage::Actions,
            cursor: 0,
            orders: vec![Command::Defend; 5],
            open: true,
        }
    }

    fn actor_id(&self) -> Option<FighterId> {
        self.actors.get(self.actor).copied()
    }

    fn actor_fighter(&self) -> Option<&psiv_core::battle::Fighter> {
        self.actor_id().and_then(|id| self.roster.get(id))
    }

    /// The techniques the actor knows, newest learned first — the order
    /// `Battle_FillTechList` walks the learned slots in.
    fn known(&self) -> Vec<u8> {
        self.actor_fighter()
            .map(|f| {
                f.stats
                    .techniques
                    .iter()
                    .rev()
                    .copied()
                    .filter(|id| {
                        *id != 0
                            && self
                                .techniques
                                .get(id)
                                .is_some_and(|t| t.targeting & 0x10 != 0)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The character skills the actor knows, newest learned first.
    fn known_skills(&self) -> Vec<(usize, u8)> {
        self.actor_fighter()
            .map(|f| {
                f.stats
                    .skills
                    .iter()
                    .copied()
                    .enumerate()
                    .rev()
                    .filter(|(_, id)| {
                        *id != 0 && self.skills.get(id).is_some_and(|s| s.targeting & 0x10 != 0)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn targets(&self, action: TargetKind) -> Vec<FighterId> {
        let Some(actor) = self.actor_id() else {
            return Vec::new();
        };
        match action {
            TargetKind::Technique(id) => {
                technique_targets(&self.roster, actor, &self.techniques[&id])
            }
            TargetKind::Skill(id) => skill_targets(&self.roster, actor, &self.skills[&id]),
            TargetKind::Item { item, .. } => item_targets(&self.roster, actor, &self.items[&item]),
            TargetKind::Attack => self
                .roster
                .side(Side::Enemy)
                .filter(|f| f.is_alive())
                .map(|f| f.id)
                .collect(),
        }
    }

    /// The window title: the actor and what they are choosing.
    fn title(&self) -> String {
        self.actor_fighter()
            .map(|f| match self.page {
                MenuPage::Targets(TargetKind::Technique(id)) => {
                    format!("{}: {}", f.name, self.techniques[&id].name)
                }
                MenuPage::Targets(TargetKind::Skill(id)) => {
                    format!("{}: {}", f.name, self.skills[&id].name)
                }
                MenuPage::Targets(TargetKind::Item { item, .. }) => {
                    format!("{}: {}", f.name, self.items[&item].name)
                }
                MenuPage::Targets(TargetKind::Attack) => format!("{}: ATTACK", f.name),
                _ => format!("{}  TP {}", f.name, f.stats.curr_tp),
            })
            .unwrap_or_else(|| "NO ORDERS".into())
    }

    /// The rows the window draws, with their enabled state.
    fn rows(&self) -> Vec<(String, bool)> {
        match self.page {
            MenuPage::Actions => vec![
                ("ATTACK".into(), true),
                ("TECH".into(), !self.known().is_empty()),
                ("SKILL".into(), !self.known_skills().is_empty()),
                ("ITEM".into(), !self.known_items().is_empty()),
                ("DEFEND".into(), true),
            ],
            MenuPage::Techniques => self
                .known()
                .iter()
                .map(|id| {
                    let tech = &self.techniques[id];
                    let enabled = self.actor_fighter().is_some_and(|f| {
                        tech.supported()
                            && f.stats.curr_tp >= u16::from(tech.cost)
                            && f.stats.status & status::TECH_SEALED == 0
                    });
                    (
                        format!(
                            "{} {:>2}{}",
                            tech.name,
                            tech.cost,
                            if tech.supported() { "" } else { " --" }
                        ),
                        enabled,
                    )
                })
                .collect(),
            MenuPage::Skills => self
                .known_skills()
                .iter()
                .map(|(slot, id)| {
                    let skill = &self.skills[id];
                    let actor = self.actor_id().expect("skill owner");
                    let stats = &self.roster.get(actor).expect("present").stats;
                    (
                        format!(
                            "{} {} OF {}",
                            skill.name, stats.curr_skill_uses[*slot], stats.max_skill_uses[*slot]
                        ),
                        skill.supported()
                            && stats.curr_skill_uses[*slot] > 0
                            && (!skill.requires_weapon || self.armed_fighters.contains(&actor)),
                    )
                })
                .collect(),
            MenuPage::Items => self
                .known_items()
                .iter()
                .map(|(source, id)| {
                    let item = &self.items[id];
                    (
                        item.name.clone(),
                        item.supported() && !self.reserved(*source),
                    )
                })
                .collect(),
            MenuPage::Targets(action) => self
                .targets(action)
                .iter()
                .map(|id| {
                    let f = self.roster.get(*id).expect("listed");
                    (
                        if id.side() == Side::Enemy {
                            format!("{} {}", f.name, id.get() - 5)
                        } else {
                            f.name.clone()
                        },
                        true,
                    )
                })
                .collect(),
        }
    }

    fn move_cursor(&mut self, delta: isize) {
        self.cursor = self
            .cursor
            .saturating_add_signed(delta)
            .min(self.rows().len().saturating_sub(1));
    }

    fn cancel(&mut self) {
        match self.page {
            // The first actor's first page closes the window itself; that is
            // the one path that hands the main options the frame back.
            MenuPage::Actions if self.actor == 0 => self.open = false,
            MenuPage::Actions => {
                self.actor -= 1;
                self.cursor = 0;
            }
            MenuPage::Techniques
            | MenuPage::Skills
            | MenuPage::Items
            | MenuPage::Targets(TargetKind::Attack) => {
                self.page = MenuPage::Actions;
                self.cursor = 0;
            }
            MenuPage::Targets(TargetKind::Technique(id)) => {
                self.page = MenuPage::Techniques;
                self.cursor = self.known().iter().position(|t| *t == id).unwrap_or(0);
            }
            MenuPage::Targets(TargetKind::Skill(id)) => {
                self.page = MenuPage::Skills;
                self.cursor = self
                    .known_skills()
                    .iter()
                    .position(|(_, s)| *s == id)
                    .unwrap_or(0);
            }
            MenuPage::Targets(TargetKind::Item { item, source }) => {
                self.page = MenuPage::Items;
                self.cursor = self
                    .known_items()
                    .iter()
                    .position(|entry| *entry == (source, item))
                    .unwrap_or(0);
            }
        }
    }

    fn accept(&mut self) -> Option<RoundOrders> {
        if self.actors.is_empty() {
            self.open = false;
            return Some(RoundOrders::Commands(self.orders.clone()));
        }
        if !self
            .rows()
            .get(self.cursor)
            .is_some_and(|(_, enabled)| *enabled)
        {
            return None;
        }
        let command = match self.page {
            MenuPage::Actions => match self.cursor {
                0 => {
                    self.page = MenuPage::Targets(TargetKind::Attack);
                    self.cursor = 0;
                    return None;
                }
                1 => {
                    self.page = MenuPage::Techniques;
                    self.cursor = 0;
                    return None;
                }
                2 => {
                    self.page = MenuPage::Skills;
                    self.cursor = 0;
                    return None;
                }
                3 => {
                    self.page = MenuPage::Items;
                    self.cursor = 0;
                    return None;
                }
                _ => Command::Defend,
            },
            MenuPage::Techniques => {
                let id = *self.known().get(self.cursor)?;
                if self.techniques[&id].single_target() {
                    self.page = MenuPage::Targets(TargetKind::Technique(id));
                    self.cursor = 0;
                    return None;
                }
                Command::Technique {
                    technique: id,
                    target: None,
                }
            }
            MenuPage::Skills => {
                let (_, id) = *self.known_skills().get(self.cursor)?;
                if self.skills[&id].single_target() {
                    self.page = MenuPage::Targets(TargetKind::Skill(id));
                    self.cursor = 0;
                    return None;
                }
                Command::Skill {
                    skill: id,
                    target: None,
                }
            }
            MenuPage::Items => {
                let (source, item) = *self.known_items().get(self.cursor)?;
                if self.items[&item].single_target() {
                    self.page = MenuPage::Targets(TargetKind::Item { item, source });
                    self.cursor = 0;
                    return None;
                }
                Command::Item {
                    item,
                    source,
                    target: None,
                }
            }
            MenuPage::Targets(action) => {
                let target = *self.targets(action).get(self.cursor)?;
                match action {
                    TargetKind::Attack => Command::AttackTarget(target),
                    TargetKind::Technique(technique) => Command::Technique {
                        technique,
                        target: Some(target),
                    },
                    TargetKind::Skill(skill) => Command::Skill {
                        skill,
                        target: Some(target),
                    },
                    TargetKind::Item { item, source } => Command::Item {
                        item,
                        source,
                        target: Some(target),
                    },
                }
            }
        };
        let slot = self.actor_id()?.slot();
        self.orders[slot] = command;
        self.actor += 1;
        self.page = MenuPage::Actions;
        self.cursor = 0;
        if self.actor == self.actors.len() {
            self.open = false;
            Some(RoundOrders::Commands(self.orders.clone()))
        } else {
            None
        }
    }

    /// Reads one frame of the window's own input.
    pub(crate) fn input(&mut self, previous: Pad, pad: Pad) -> Option<RoundOrders> {
        if pad.is_pressed(previous, Button::Cancel) {
            self.cancel();
            return None;
        }
        let delta = list_step(previous, pad);
        if delta != 0 {
            self.move_cursor(delta);
        }
        if accept_pressed(previous, pad) {
            self.accept()
        } else {
            None
        }
    }

    fn known_items(&self) -> Vec<(ItemSource, u8)> {
        let equipment = self
            .actor_fighter()
            .map(|f| f.stats.equipment)
            .unwrap_or_default();
        equipment
            .into_iter()
            .enumerate()
            .map(|(slot, id)| (ItemSource::Equipment(slot as u8), id))
            .chain(
                self.inventory
                    .slots()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(slot, id)| (ItemSource::Inventory(slot as u8), id)),
            )
            .filter(|(_, id)| *id != 0 && self.items.contains_key(id))
            .collect()
    }

    /// Whether an earlier actor's order already reserved this inventory copy.
    fn reserved(&self, source: ItemSource) -> bool {
        let ItemSource::Inventory(slot) = source else {
            return false;
        };
        let previous = self.actor_id().map_or(0, |actor| actor.slot());
        self.orders.iter().take(previous).any(|order| matches!(order, Command::Item { source: ItemSource::Inventory(s), .. } if *s == slot))
    }

    /// What the window draws and what the native input drivers probe.
    pub(crate) fn view(&self) -> CommandMenuView {
        let fighter = self.actor_fighter();
        let targets = match self.page {
            MenuPage::Targets(action) => self.targets(action),
            _ => Vec::new(),
        };
        CommandMenuView {
            title: self.title(),
            page: self.page,
            rows: self
                .rows()
                .into_iter()
                .map(|(label, enabled)| MenuRow { label, enabled })
                .collect(),
            cursor: self.cursor,
            actor: fighter.map(|f| f.id.get()),
            character: fighter.and_then(|f| f.character),
            party: self.party_status(),
            enemies: self.alive_enemies(),
            techniques: self
                .known()
                .iter()
                .map(|id| {
                    let tech = &self.techniques[id];
                    TechniqueEntry {
                        id: *id,
                        name: tech.name.clone(),
                        cost: tech.cost,
                        available: tech.supported()
                            && fighter.is_some_and(|f| {
                                f.stats.curr_tp >= u16::from(tech.cost)
                                    && f.stats.status & status::TECH_SEALED == 0
                            }),
                    }
                })
                .collect(),
            skills: self
                .known_skills()
                .iter()
                .map(|(slot, id)| {
                    let skill = &self.skills[id];
                    let remaining = fighter.map_or(0, |f| f.stats.curr_skill_uses[*slot]);
                    SkillEntry {
                        id: *id,
                        name: skill.name.clone(),
                        remaining,
                        available: skill.supported()
                            && remaining > 0
                            && (!skill.requires_weapon
                                || self
                                    .actor_id()
                                    .is_some_and(|id| self.armed_fighters.contains(&id))),
                    }
                })
                .collect(),
            targets: targets.iter().map(FighterId::get).collect(),
        }
    }

    fn party_status(&self) -> Vec<PartyStatus> {
        self.roster
            .side(Side::Party)
            .map(|f| PartyStatus {
                fighter: f.id.get(),
                name: f.name.clone(),
                hp: f.stats.curr_hp,
                max_hp: f.stats.max_hp,
                tp: f.stats.curr_tp,
                status: f.stats.status,
            })
            .collect()
    }

    fn alive_enemies(&self) -> Vec<u8> {
        self.roster
            .living(Side::Enemy)
            .map(|f| f.id.get())
            .collect()
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
