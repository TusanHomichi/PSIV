//! Pure command selection. No battle rolls or resource spending happen here.
//! The compact native menu uses the extracted window/font; its layout is not
//! yet a certified reproduction of the cartridge's character command windows.

use psiv_core::battle::{
    BattleItem, Command, FighterId, ItemSource, Roster, RoundOrders, Side, Skill, Technique,
    item_targets, skill_targets, status, technique_targets,
};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Actions,
    Techniques,
    Skills,
    Items,
    Targets(TargetAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetAction {
    Attack,
    Technique(u8),
    Skill(u8),
    Item { item: u8, source: ItemSource },
}

pub(super) struct CommandsMenu {
    roster: Roster,
    techniques: BTreeMap<u8, Technique>,
    skills: BTreeMap<u8, Skill>,
    armed_fighters: Vec<FighterId>,
    items: BTreeMap<u8, BattleItem>,
    inventory: psiv_core::Inventory,
    actors: Vec<FighterId>,
    actor: usize,
    page: Page,
    cursor: usize,
    orders: Vec<Command>,
    pub(super) open: bool,
}

impl CommandsMenu {
    pub(super) fn debug_menu(&self) -> serde_json::Value {
        let fighter = self.actor_id().and_then(|id| self.roster.get(id));
        let targets = match self.page {
            Page::Targets(action) => self.targets(action),
            _ => Vec::new(),
        };
        serde_json::json!({
            "title": self.title(), "rows": self.rows(), "cursor": self.cursor(),
            "page": format!("{:?}", self.page),
            "actor": fighter.map(|f| f.id.get()),
            "character": fighter.and_then(|f| f.character),
            "party": self.roster.side(Side::Party).map(|f| serde_json::json!({"id": f.id.get(), "name": f.name, "hp": f.stats.curr_hp, "max_hp": f.stats.max_hp, "tp": f.stats.curr_tp, "status": f.stats.status})).collect::<Vec<_>>(),
            "enemies": self.roster.living(Side::Enemy).map(|f| serde_json::json!({"id": f.id.get(), "hp": f.stats.curr_hp, "status": f.stats.status})).collect::<Vec<_>>(),
            "techniques": self.known().iter().map(|id| {
                let tech = &self.techniques[id];
                serde_json::json!({"id": id, "name": tech.name, "cost": tech.cost, "available": tech.supported() && fighter.is_some_and(|f| f.stats.curr_tp >= u16::from(tech.cost) && f.stats.status & status::TECH_SEALED == 0)})
            }).collect::<Vec<_>>(),
            "skills": self.known_skills().iter().map(|(slot, id)| {
                let skill = &self.skills[id];
                let remaining = fighter.map_or(0, |f| f.stats.curr_skill_uses[*slot]);
                serde_json::json!({"id": id, "name": skill.name, "remaining": remaining,
                    "available": skill.supported() && remaining > 0 && (!skill.requires_weapon || self.actor_id().is_some_and(|id| self.armed_fighters.contains(&id)))})
            }).collect::<Vec<_>>(),
            "targets": targets.iter().map(FighterId::get).collect::<Vec<_>>(),
        })
    }

    pub(super) fn new(
        roster: Roster,
        techniques: BTreeMap<u8, Technique>,
        skills: BTreeMap<u8, Skill>,
        armed_fighters: Vec<FighterId>,
        items: BTreeMap<u8, BattleItem>,
        inventory: psiv_core::Inventory,
    ) -> Self {
        let actors = roster
            .side(Side::Party)
            .filter(|f| f.is_alive() && f.stats.can_act())
            .map(|f| f.id)
            .collect();
        Self {
            roster,
            techniques,
            skills,
            armed_fighters,
            items,
            inventory,
            actors,
            actor: 0,
            page: Page::Actions,
            cursor: 0,
            orders: vec![Command::Defend; 5],
            open: true,
        }
    }

    fn actor_id(&self) -> Option<FighterId> {
        self.actors.get(self.actor).copied()
    }

    fn known(&self) -> Vec<u8> {
        self.actor_id()
            .and_then(|id| self.roster.get(id))
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

    fn known_skills(&self) -> Vec<(usize, u8)> {
        self.actor_id()
            .and_then(|id| self.roster.get(id))
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

    fn targets(&self, action: TargetAction) -> Vec<FighterId> {
        let Some(actor) = self.actor_id() else {
            return Vec::new();
        };
        match action {
            TargetAction::Technique(id) => {
                technique_targets(&self.roster, actor, &self.techniques[&id])
            }
            TargetAction::Skill(id) => skill_targets(&self.roster, actor, &self.skills[&id]),
            TargetAction::Item { item, .. } => {
                item_targets(&self.roster, actor, &self.items[&item])
            }
            TargetAction::Attack => self
                .roster
                .side(Side::Enemy)
                .filter(|f| f.is_alive())
                .map(|f| f.id)
                .collect(),
        }
    }

    pub(super) fn title(&self) -> String {
        self.actor_id()
            .and_then(|id| self.roster.get(id))
            .map(|f| match self.page {
                Page::Targets(TargetAction::Technique(id)) => {
                    format!("{}: {}", f.name, self.techniques[&id].name)
                }
                Page::Targets(TargetAction::Skill(id)) => {
                    format!("{}: {}", f.name, self.skills[&id].name)
                }
                Page::Targets(TargetAction::Item { item, .. }) => {
                    format!("{}: {}", f.name, self.items[&item].name)
                }
                Page::Targets(TargetAction::Attack) => format!("{}: ATTACK", f.name),
                _ => format!("{}  TP {}", f.name, f.stats.curr_tp),
            })
            .unwrap_or_else(|| "NO ORDERS".into())
    }

    pub(super) fn rows(&self) -> Vec<(String, bool)> {
        match self.page {
            Page::Actions => vec![
                ("ATTACK".into(), true),
                ("TECH".into(), !self.known().is_empty()),
                ("SKILL".into(), !self.known_skills().is_empty()),
                ("ITEM".into(), !self.known_items().is_empty()),
                ("DEFEND".into(), true),
            ],
            Page::Techniques => self
                .known()
                .iter()
                .map(|id| {
                    let tech = &self.techniques[id];
                    let enabled = self
                        .actor_id()
                        .and_then(|id| self.roster.get(id))
                        .is_some_and(|f| {
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
            Page::Skills => self
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
            Page::Items => self
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
            Page::Targets(action) => self
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

    pub(super) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(super) fn move_cursor(&mut self, delta: isize) {
        self.cursor = self
            .cursor
            .saturating_add_signed(delta)
            .min(self.rows().len().saturating_sub(1));
    }

    pub(super) fn cancel(&mut self) {
        match self.page {
            Page::Actions if self.actor == 0 => self.open = false,
            Page::Actions => {
                self.actor -= 1;
                self.cursor = 0;
            }
            Page::Techniques | Page::Skills | Page::Items | Page::Targets(TargetAction::Attack) => {
                self.page = Page::Actions;
                self.cursor = 0;
            }
            Page::Targets(TargetAction::Technique(id)) => {
                self.page = Page::Techniques;
                self.cursor = self.known().iter().position(|t| *t == id).unwrap_or(0);
            }
            Page::Targets(TargetAction::Skill(id)) => {
                self.page = Page::Skills;
                self.cursor = self
                    .known_skills()
                    .iter()
                    .position(|(_, s)| *s == id)
                    .unwrap_or(0);
            }
            Page::Targets(TargetAction::Item { item, source }) => {
                self.page = Page::Items;
                self.cursor = self
                    .known_items()
                    .iter()
                    .position(|entry| *entry == (source, item))
                    .unwrap_or(0);
            }
        }
    }

    pub(super) fn accept(&mut self) -> Option<RoundOrders> {
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
            Page::Actions => match self.cursor {
                0 => {
                    self.page = Page::Targets(TargetAction::Attack);
                    self.cursor = 0;
                    return None;
                }
                1 => {
                    self.page = Page::Techniques;
                    self.cursor = 0;
                    return None;
                }
                2 => {
                    self.page = Page::Skills;
                    self.cursor = 0;
                    return None;
                }
                3 => {
                    self.page = Page::Items;
                    self.cursor = 0;
                    return None;
                }
                _ => Command::Defend,
            },
            Page::Techniques => {
                let id = *self.known().get(self.cursor)?;
                if self.techniques[&id].single_target() {
                    self.page = Page::Targets(TargetAction::Technique(id));
                    self.cursor = 0;
                    return None;
                }
                Command::Technique {
                    technique: id,
                    target: None,
                }
            }
            Page::Skills => {
                let (_, id) = *self.known_skills().get(self.cursor)?;
                if self.skills[&id].single_target() {
                    self.page = Page::Targets(TargetAction::Skill(id));
                    self.cursor = 0;
                    return None;
                }
                Command::Skill {
                    skill: id,
                    target: None,
                }
            }
            Page::Items => {
                let (source, item) = *self.known_items().get(self.cursor)?;
                if self.items[&item].single_target() {
                    self.page = Page::Targets(TargetAction::Item { item, source });
                    self.cursor = 0;
                    return None;
                }
                Command::Item {
                    item,
                    source,
                    target: None,
                }
            }
            Page::Targets(action) => {
                let target = *self.targets(action).get(self.cursor)?;
                match action {
                    TargetAction::Attack => Command::AttackTarget(target),
                    TargetAction::Technique(technique) => Command::Technique {
                        technique,
                        target: Some(target),
                    },
                    TargetAction::Skill(skill) => Command::Skill {
                        skill,
                        target: Some(target),
                    },
                    TargetAction::Item { item, source } => Command::Item {
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
        self.page = Page::Actions;
        self.cursor = 0;
        if self.actor == self.actors.len() {
            self.open = false;
            Some(RoundOrders::Commands(self.orders.clone()))
        } else {
            None
        }
    }

    fn known_items(&self) -> Vec<(ItemSource, u8)> {
        let equipment = self
            .actor_id()
            .and_then(|id| self.roster.get(id))
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

    fn reserved(&self, source: ItemSource) -> bool {
        let ItemSource::Inventory(slot) = source else {
            return false;
        };
        let previous = self.actor_id().map_or(0, |actor| actor.slot());
        self.orders.iter().take(previous).any(|order| matches!(order, Command::Item { source: ItemSource::Inventory(s), .. } if *s == slot))
    }
}

pub(super) fn draw(
    chrome: &super::chrome::BattleChrome,
    menu: &CommandsMenu,
    quads: &mut Vec<super::chrome::Quad>,
) {
    use super::chrome::WindowRect;
    let rows = menu.rows();
    let page_size = if menu.page == Page::Actions { 5 } else { 4 };
    let rect = WindowRect {
        x: 96.0,
        y: 8.0,
        w: 208.0,
        h: 48.0 + 16.0 * rows.len().min(page_size) as f32,
    };
    if let Some(frame) = chrome.frame(rect) {
        quads.extend(frame);
    }
    quads.extend(chrome.text(
        &menu.title(),
        WindowRect {
            x: 104.0,
            y: 16.0,
            w: 192.0,
            h: 8.0,
        },
    ));
    let first = menu.cursor() / page_size * page_size;
    for (i, (label, enabled)) in rows.iter().enumerate().skip(first).take(page_size) {
        let y = 32.0 + (i - first) as f32 * 16.0;
        let pattern = if i == menu.cursor() { 0x6e8 } else { 0x6e7 };
        if let Some(quad) = chrome.window_word(
            pattern,
            false,
            false,
            godot::prelude::Rect2::new(
                godot::prelude::Vector2::new(104.0, y),
                godot::prelude::Vector2::new(8.0, 8.0),
            ),
        ) {
            quads.push(quad);
        }
        let text = format!("{}{}", if *enabled { "" } else { "-" }, label);
        quads.extend(chrome.text(
            &text,
            WindowRect {
                x: 120.0,
                y,
                w: 176.0,
                h: 8.0,
            },
        ));
    }
    let footer = if rows.len() > page_size {
        format!(
            "PAGE {} OF {} CANCEL: BACK",
            first / page_size + 1,
            rows.len().div_ceil(page_size)
        )
    } else {
        "CANCEL: BACK".into()
    };
    quads.extend(chrome.text(
        &footer,
        WindowRect {
            x: 104.0,
            y: rect.y + rect.h - 16.0,
            w: 192.0,
            h: 8.0,
        },
    ));
}
