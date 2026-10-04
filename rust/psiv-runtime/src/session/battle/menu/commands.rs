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
    CommandMenuView, ListEntry, ListView, MenuPage, MenuRow, PartyStatus, SkillEntry, StripView,
    TargetKind, TechniqueEntry,
};
use super::{accept_pressed, list_step};

/// Rows per list-window page: four entries at `$100`-byte spacing
/// (`Battle_OpenTechs`, `ps4.asm:2506`).
const PAGE_ROWS: usize = 4;

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
    /// Frames the window has been open (the command cursor object's clock).
    age: u32,
    /// `Battle_Command_Data`'s command byte per party slot, which the pane
    /// icons show (`ps4.asm:11056`). The window writes the byte the cartridge
    /// writes at each step and the battle mode carries it between rounds.
    bytes: [u8; 5],
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
    #[cfg(test)]
    pub(crate) fn new(runtime: &Runtime) -> CommandsMenu {
        CommandsMenu::with_command_bytes(runtime, [0; 5])
    }

    /// [`CommandsMenu::new`] over the command bytes the battle already holds.
    pub(crate) fn with_command_bytes(runtime: &Runtime, bytes: [u8; 5]) -> CommandsMenu {
        let roster = runtime.battle_roster().cloned().unwrap_or_else(Roster::new);
        let actors = roster
            .side(Side::Party)
            .filter(|f| f.is_alive() && f.stats.can_act())
            .map(|f| f.id)
            .collect();
        let mut menu = CommandsMenu {
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
            age: 0,
            bytes,
            open: true,
        };
        menu.open_actor();
        menu
    }

    /// The fighter whose window this is.
    pub(crate) fn actor(&self) -> Option<FighterId> {
        self.actor_id()
    }

    /// Whether the open page's cursor is a red-cursor routine (the lists) as
    /// opposed to the strip's sprite and the target pickers.
    pub(crate) fn red_cursor_window(&self) -> bool {
        matches!(
            self.page,
            MenuPage::Techniques | MenuPage::Skills | MenuPage::Items
        )
    }

    /// Fixture entry: the window as the cartridge leaves it once `page` is
    /// open with the cursor on `cursor`, command byte included.
    pub(crate) fn debug_open(&mut self, page: MenuPage, cursor: usize, age: u32) {
        self.page = page;
        self.cursor = cursor;
        self.age = age;
        self.set_command_byte(match page {
            MenuPage::Techniques => 2,
            MenuPage::Skills => 3,
            MenuPage::Items => 4,
            _ => 0,
        });
    }

    /// The command bytes, for the pane icons and the next round.
    pub(crate) fn command_bytes(&self) -> [u8; 5] {
        self.bytes
    }

    /// The acting fighter's record index in `Battle_Command_Data`.
    fn command_slot(&self) -> Option<usize> {
        self.actor_id().map(|id| id.slot())
    }

    fn set_command_byte(&mut self, value: u8) {
        if let Some(slot) = self.command_slot() {
            self.bytes[slot] = value;
        }
    }

    /// `Battle_OpenCharComd` (`ps4.asm:2085`) clears the *next* record's
    /// command byte as the strip opens.
    fn open_actor(&mut self) {
        if let Some(slot) = self.command_slot()
            && let Some(next) = self.bytes.get_mut(slot + 1)
        {
            *next = 0;
        }
    }

    fn actor_id(&self) -> Option<FighterId> {
        self.actors.get(self.actor).copied()
    }

    /// `Battle_ProcessCOMD` skips every party slot with a `$6E` status and
    /// enters `Battle_OrderTurns` as soon as the scan is exhausted
    /// (`ps4.asm:7636-7660`). The main COMD choice already supplied the
    /// player's press; an empty actor list needs no second one.
    pub(crate) fn no_actor_orders(&self) -> Option<RoundOrders> {
        self.actors
            .is_empty()
            .then(|| RoundOrders::Commands(self.orders.clone()))
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
                    // The window refuses an entry on TP alone: `Battle_FillTechList`
                    // sets the sign bit when the cost exceeds the member's TP
                    // (`ps4.asm:1721`) and `Battle_TechWindow` ignores a negative
                    // id (`ps4.asm:2628`). A sealed member may still choose; the
                    // cast is paid and wasted (`CharTech_Cast`, `ps4.asm:14256`).
                    let enabled = self.actor_fighter().is_some_and(|f| {
                        tech.supported() && f.stats.curr_tp >= u16::from(tech.cost)
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
                self.open_actor();
            }
            // `Battle_BackFromTechs` (`ps4.asm:2777`) and its twins reopen the
            // strip with `Battle_Char_Comd_Index` untouched, so the cursor is
            // still on the icon that opened the list.
            MenuPage::Techniques | MenuPage::Skills | MenuPage::Items => {
                self.cursor = match self.page {
                    MenuPage::Techniques => 1,
                    MenuPage::Skills => 2,
                    _ => 3,
                };
                self.page = MenuPage::Actions;
                self.set_command_byte(0);
            }
            MenuPage::Targets(TargetKind::Attack) => {
                self.page = MenuPage::Actions;
                self.cursor = 0;
                self.set_command_byte(0);
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
        if let Some(orders) = self.no_actor_orders() {
            self.open = false;
            return Some(orders);
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
                    self.set_command_byte(1);
                    self.page = MenuPage::Targets(TargetKind::Attack);
                    self.cursor = 0;
                    return None;
                }
                1 => {
                    self.set_command_byte(2);
                    self.page = MenuPage::Techniques;
                    self.cursor = 0;
                    return None;
                }
                2 => {
                    self.set_command_byte(3);
                    self.page = MenuPage::Skills;
                    self.cursor = 0;
                    return None;
                }
                3 => {
                    self.set_command_byte(4);
                    self.page = MenuPage::Items;
                    self.cursor = 0;
                    return None;
                }
                _ => {
                    self.set_command_byte(5);
                    Command::Defend
                }
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
            self.open_actor();
            None
        }
    }

    /// Reads one frame of the window's own input, in the order the
    /// cartridge's routine tests its buttons.
    ///
    /// The strip (`Battle_CharCommand`, `ps4.asm:2192`) moves its cursor with
    /// `Battle_UpdateCursor` (`ps4.asm:70646`): Left or Right, wrapping over
    /// five icons, then Cancel, then accept. The list windows
    /// (`Battle_TechWindow` `ps4.asm:2593`, `Battle_SkillWindow` `ps4.asm:2933`,
    /// `Battle_ItemWindow` `ps4.asm:3325`) run `Battle_UpdateRedCursor2`
    /// (`ps4.asm:1572`): Up or Down inside the page's four rows, then a Right
    /// that flips to the next page only when one exists, a Left likewise, then
    /// Cancel, then accept. The target lists keep this port's own one-list
    /// mapping (the cartridge picks targets with a cursor over the enemies).
    pub(crate) fn input(&mut self, previous: Pad, pad: Pad) -> Option<RoundOrders> {
        self.age = self.age.saturating_add(1);
        match self.page {
            MenuPage::Actions => self.strip_step(previous, pad),
            MenuPage::Techniques | MenuPage::Skills | MenuPage::Items => {
                if self.page_step(previous, pad) {
                    return None;
                }
            }
            MenuPage::Targets(_) => {
                let delta = list_step(previous, pad);
                if delta != 0 {
                    self.move_cursor(delta);
                }
            }
        }
        if pad.is_pressed(previous, Button::Cancel) {
            self.cancel();
            return None;
        }
        if accept_pressed(previous, pad) {
            self.accept()
        } else {
            None
        }
    }

    /// `Battle_UpdateCursor` with `d1 = 4`: Left or Right, wrapping.
    fn strip_step(&mut self, previous: Pad, pad: Pad) {
        const ICONS: usize = 5;
        if pad.is_pressed(previous, Button::Left) {
            self.cursor = (self.cursor + ICONS - 1) % ICONS;
        } else if pad.is_pressed(previous, Button::Right) {
            self.cursor = (self.cursor + 1) % ICONS;
        }
    }

    /// The list windows' cursor and page flips; `true` when a page flip
    /// consumed the frame (the cartridge branches away from its Cancel and
    /// accept tests there).
    fn page_step(&mut self, previous: Pad, pad: Pad) -> bool {
        let base = self.cursor / PAGE_ROWS * PAGE_ROWS;
        let mut row = self.cursor % PAGE_ROWS;
        if pad.is_pressed(previous, Button::Up) {
            row = (row + PAGE_ROWS - 1) % PAGE_ROWS;
        } else if pad.is_pressed(previous, Button::Down) {
            row = (row + 1) % PAGE_ROWS;
        }
        self.cursor = base + row;
        let entries = self.rows().len();
        if pad.is_pressed(previous, Button::Right) && entries > base + PAGE_ROWS {
            self.cursor = base + PAGE_ROWS;
            return true;
        }
        if pad.is_pressed(previous, Button::Left) && base > 0 {
            self.cursor = base - PAGE_ROWS;
            return true;
        }
        false
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
            strip: self.strip_view(),
            list: self.list_view(),
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

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
