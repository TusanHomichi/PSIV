//! What the per-character command window draws: the strip, a list page or
//! the opening box, built from [`CommandsMenu`]'s state. Split from
//! `commands.rs`, which keeps the window's rules and input.

use super::*;

impl CommandsMenu {
    /// What the window draws and what the native input drivers probe.
    pub(crate) fn view(&self) -> CommandMenuView {
        let fighter = self.actor_fighter();
        let targets = match self.page {
            MenuPage::Targets(action) => self.targets(action),
            _ => Vec::new(),
        };
        let growing = self
            .growth
            .zip(window_width(self.page))
            .and_then(|(width, full_width)| {
                Some(GrowingWindow {
                    window: self.page,
                    slot: self.comd_slot()?,
                    width,
                    full_width,
                })
            });
        let strip_growing = growing.is_some() && self.page == MenuPage::Actions;
        CommandMenuView {
            growing,
            strip: self.strip_view().filter(|_| !strip_growing),
            list: self.list_view().filter(|_| growing.is_none()),
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
                        available: fighter
                            .is_some_and(|f| CommandsMenu::technique_enabled(&f.stats, tech)),
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
                        available: fighter.is_some_and(|f| {
                            CommandsMenu::skill_enabled(
                                &f.stats,
                                *slot,
                                skill,
                                self.armed_fighters.contains(&f.id),
                            )
                        }),
                    }
                })
                .collect(),
            targets: targets.iter().map(FighterId::get).collect(),
        }
    }

    /// The acting fighter's slot minus one: `Battle_Total_Comd_Input`.
    fn comd_slot(&self) -> Option<u8> {
        self.actor_id().map(|id| id.get() - 1)
    }

    /// The strip stays on the plane under the list windows: `Battle_TechCommand`
    /// and its twins only open a window over it (`ps4.asm:2326`).
    fn strip_view(&self) -> Option<StripView> {
        if !matches!(
            self.page,
            MenuPage::Actions | MenuPage::Techniques | MenuPage::Skills | MenuPage::Items
        ) {
            return None;
        }
        let actor = self.actor_id()?;
        Some(StripView {
            slot: self.comd_slot()?,
            present: [
                self.armed_fighters.contains(&actor),
                !self.known().is_empty(),
                !self.known_skills().is_empty(),
                !self.known_items().is_empty(),
                true,
            ],
            cursor: self.cursor.min(4) as u8,
            age: self.age,
        })
    }

    fn list_view(&self) -> Option<ListView> {
        if !matches!(
            self.page,
            MenuPage::Techniques | MenuPage::Skills | MenuPage::Items
        ) {
            return None;
        }
        let fighter = self.actor_fighter()?;
        let rows = self.rows();
        let base = self.cursor / PAGE_ROWS * PAGE_ROWS;
        let entries = (base..rows.len().min(base + PAGE_ROWS))
            .map(|row| match self.page {
                MenuPage::Techniques => {
                    let id = self.known()[row];
                    ListEntry {
                        name: self.techniques[&id].name.clone(),
                        value: Some(self.techniques[&id].cost),
                        enabled: rows[row].1,
                    }
                }
                MenuPage::Skills => {
                    let (slot, id) = self.known_skills()[row];
                    ListEntry {
                        name: self.skills[&id].name.clone(),
                        value: Some(fighter.stats.curr_skill_uses[slot]),
                        enabled: rows[row].1,
                    }
                }
                _ => {
                    let (_, id) = self.known_items()[row];
                    ListEntry {
                        name: self.items[&id].name.clone(),
                        value: None,
                        enabled: rows[row].1,
                    }
                }
            })
            .collect();
        Some(ListView {
            window: self.page,
            slot: self.comd_slot()?,
            page: base / PAGE_ROWS,
            entries,
            cursor: (self.cursor % PAGE_ROWS) as u8,
            more_before: base > 0,
            more_after: rows.len() > base + PAGE_ROWS,
        })
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
