//! The per-character command window: rows, pages, targets and reservations.
//!
//! Split out of `menu.rs` with the S3 node so each file stays reviewable.

use std::collections::BTreeMap;

use psiv_core::battle::{
    BattleItem, Command, FighterId, ItemSource, Roster, RoundOrders, Side, Skill, Stats, Technique,
    item_targets, skill_targets, technique_targets,
};

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::super::view::{
    CommandMenuView, GrowingWindow, ListEntry, ListView, MenuPage, MenuRow, PartyStatus,
    SkillEntry, StripView, TargetKind, TechniqueEntry,
};
use super::{accept_pressed, list_step};

/// Rows per list-window page: four entries at `$100`-byte spacing
/// (`Battle_OpenTechs`'s `loc_18F2`, `ps4.asm:2499-2511`).
const PAGE_ROWS: usize = 4;

/// Where each member's strip and list cursors stand, which the cartridge keeps
/// per party slot (`Battle_Total_Comd_Input`) and reopens at.
///
/// `Battle_CharCommand` reads and writes `Battle_Char_Comd_Index`
/// (`$FFFF41A2`, `ps4.asm:2195-2203`), and `Battle_TechWindow` and
/// `Battle_SkillWindow` hand `Battle_UpdateRedCursor2` the member's
/// `Battle_Tech_Index` and `Battle_Skill_Index` words (`$FFFF41AE`,
/// `$FFFF41BA`, `ps4.asm:2598-2601`, `2938-2941`): nothing clears them during
/// a battle, so a member's next strip and lists open where they left them.
/// `Battle_Item_Index` (`$FFFF41C4`) is per member too, but `loc_5BAE` clears
/// its five words at each round's end (`ps4.asm:8422-8425`,
/// [`CursorMemory::end_round`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CursorMemory {
    strip: [usize; 5],
    techniques: [usize; 5],
    skills: [usize; 5],
    items: [usize; 5],
}

impl CursorMemory {
    /// `loc_5BAE`: the round is over, and the item cursors start again.
    pub fn end_round(&mut self) {
        self.items = [0; 5];
    }

    /// Where party slot `slot`'s (0-4) `page` reopens: the strip's icon or
    /// the list's row across its pages. A target picker keeps no word, and
    /// opens at 0.
    #[must_use]
    pub fn cursor(&self, slot: usize, page: MenuPage) -> usize {
        let list = match page {
            MenuPage::Actions => &self.strip,
            MenuPage::Techniques => &self.techniques,
            MenuPage::Skills => &self.skills,
            MenuPage::Items => &self.items,
            MenuPage::Targets(_) => return 0,
        };
        list.get(slot).copied().unwrap_or(0)
    }

    /// The window wrote `cursor` to party slot `slot`'s word for `page`, as
    /// the cartridge's cursor routines do on every move.
    pub fn remember(&mut self, slot: usize, page: MenuPage, cursor: usize) {
        if let Some(word) = self.list(page).and_then(|list| list.get_mut(slot)) {
            *word = cursor;
        }
    }

    fn list(&mut self, page: MenuPage) -> Option<&mut [usize; 5]> {
        match page {
            MenuPage::Actions => Some(&mut self.strip),
            MenuPage::Techniques => Some(&mut self.techniques),
            MenuPage::Skills => Some(&mut self.skills),
            MenuPage::Items => Some(&mut self.items),
            MenuPage::Targets(_) => None,
        }
    }
}

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
    /// Each member's remembered cursors, which the battle mode carries
    /// between rounds.
    memory: CursorMemory,
    /// `$FFFF418C`: the width the opening window has reached, while
    /// `Battle_CreateWindow` is still growing it. `None` once it is whole.
    growth: Option<u8>,
    /// The frame a window became whole: `Battle_CreateWindow` drew it, but
    /// the window's own routine, which writes the red or blue cursor tile,
    /// has not run yet. A list draws its selected row blue then.
    whole_unread: bool,
    /// Whether the window is still open. `false` hands the orders back.
    pub(crate) open: bool,
}

/// The width in cells `Battle_CreateWindow` grows a page's window to: its
/// `d1`. The strip `$10` (`ps4.asm:2170`); the technique window `$C`
/// (`2489`), the skill window `$F` (`2830`) and the item window `$E`
/// (`3186`). The target pickers are the port's own and do not grow.
const fn window_width(page: MenuPage) -> Option<u8> {
    match page {
        MenuPage::Actions => Some(16),
        MenuPage::Techniques => Some(12),
        MenuPage::Skills => Some(15),
        MenuPage::Items => Some(14),
        MenuPage::Targets(_) => None,
    }
}

/// Cells `Battle_CreateWindow` adds a frame (`addq.w #2`, `ps4.asm:1658`).
const GROWTH_STEP: u8 = 2;

impl CommandsMenu {
    /// Opens the window over the battle's live data.
    ///
    /// A roster the runtime does not have (battles never enabled) opens an
    /// empty window rather than refusing: with no actor to answer it submits
    /// the Defend default, which is what the shell's own menu did with an
    /// empty roster.
    #[cfg(test)]
    pub(crate) fn new(runtime: &Runtime) -> CommandsMenu {
        CommandsMenu::with_command_bytes(runtime, [0; 5], CursorMemory::default())
    }

    /// [`CommandsMenu::new`] over the command bytes and cursor memory the
    /// battle already holds.
    pub(crate) fn with_command_bytes(
        runtime: &Runtime,
        bytes: [u8; 5],
        memory: CursorMemory,
    ) -> CommandsMenu {
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
            memory,
            growth: None,
            whole_unread: false,
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
        // The window routine that runs the red cursor's timers is not
        // running while `Battle_CreateWindow` grows the window.
        !self.growing()
            && matches!(
                self.page,
                MenuPage::Techniques | MenuPage::Skills | MenuPage::Items
            )
    }

    /// Whether a list's selected row may show the red cursor this frame.
    ///
    /// The cursor tile is written by the list routine
    /// (`Battle_UpdateRedCursor2`, `ps4.asm:1572`), which first runs the
    /// frame after `Battle_CreateWindow` draws the window whole. Until then
    /// the row is blue whatever the blink timers last decided; they stay
    /// paused through that frame too ([`CommandsMenu::red_cursor_window`]).
    pub(crate) fn list_cursor_shown(&self) -> bool {
        !self.growing() && !self.whole_unread
    }

    /// Fixture entry: the window as the cartridge leaves it once `page` is
    /// open with the cursor on `cursor`, command byte included.
    pub(crate) fn debug_open(&mut self, page: MenuPage, cursor: usize, age: u32) {
        self.page = page;
        self.cursor = cursor;
        self.age = age;
        self.growth = None;
        self.whole_unread = false;
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

    /// The members' remembered cursors, for the next window.
    pub(crate) fn cursor_memory(&self) -> CursorMemory {
        self.memory
    }

    /// Opens `page` for the actor at the cursor it left there. That can be an
    /// empty row: Up and Down wrap inside a page whether or not a row holds an
    /// entry (`Battle_UpdateRedCursor2`, `ps4.asm:1572`), and the word keeps
    /// what they wrote. A battle's technique and skill lists do not change,
    /// and the item words are cleared before the pack can.
    ///
    /// The window then grows into place (`Battle_OpenCharComd`,
    /// `Battle_OpenTechs` and twins, through `Battle_CreateWindow`).
    fn open_page(&mut self, page: MenuPage) {
        self.page = page;
        self.cursor = self
            .command_slot()
            .map_or(0, |slot| self.memory.cursor(slot, page));
        self.start_growth();
    }

    /// `clr.w ($FFFF418C)` before a window's first `Battle_CreateWindow`.
    fn start_growth(&mut self) {
        self.growth = window_width(self.page).map(|_| 0);
        self.whole_unread = false;
    }

    /// Whether a window is still growing, and reads no input.
    pub(crate) fn growing(&self) -> bool {
        self.growth.is_some()
    }

    /// One `Battle_CreateWindow` call: two cells wider, and whole once the
    /// width reaches the window's (`cmp.w ($FFFF418C),d1 / ble`), which is
    /// also the frame the routine moves on to the window's input routine.
    fn grow(&mut self) {
        let Some(width) = self.growth else { return };
        let width = width.saturating_add(GROWTH_STEP);
        self.growth = match window_width(self.page) {
            Some(full) if width < full => Some(width),
            _ => None,
        };
        if self.growth.is_none() {
            // The strip's cursor object is created now (`move.w #4,
            // (Command_Cursor).w`, `ps4.asm:2175`) and shows from the next
            // frame. A list's red cursor routine likewise first runs then.
            self.age = 0;
            self.whole_unread = true;
        }
    }

    /// Writes the cursor back to the actor's word for the open page.
    fn remember(&mut self) {
        if let Some(slot) = self.command_slot() {
            self.memory.remember(slot, self.page, self.cursor);
        }
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
        self.open_page(MenuPage::Actions);
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
                CommandsMenu::listed_techniques(&f.stats, |id| self.techniques.get(&id))
                    .iter()
                    .map(|t| t.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The character skills the actor knows, newest learned first.
    fn known_skills(&self) -> Vec<(usize, u8)> {
        self.actor_fighter()
            .map(|f| {
                CommandsMenu::listed_skills(&f.stats, |id| self.skills.get(&id))
                    .iter()
                    .map(|(slot, s)| (*slot, s.id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The techniques a member's TECH window lists, newest learned first (the
    /// order `Battle_FillTechList` walks the learned slots in): every learned
    /// technique whose record is a battle technique (`targeting & $10`).
    pub(crate) fn listed_techniques<'a>(
        stats: &Stats,
        record: impl Fn(u8) -> Option<&'a Technique>,
    ) -> Vec<&'a Technique> {
        stats
            .techniques
            .iter()
            .rev()
            .copied()
            .filter(|id| *id != 0)
            .filter_map(&record)
            .filter(|t| t.targeting & 0x10 != 0)
            .collect()
    }

    /// Whether the TECH window enables `tech` for a member with `stats`.
    ///
    /// The window refuses an entry on TP alone: `Battle_FillTechList` sets the
    /// sign bit when the cost exceeds the member's TP (`ps4.asm:1721`) and
    /// `Battle_TechWindow` ignores a negative id (`ps4.asm:2628`). A sealed
    /// member may still choose; the cast is paid and wasted (`CharTech_Cast`,
    /// `ps4.asm:14256`).
    pub(crate) fn technique_enabled(stats: &Stats, tech: &Technique) -> bool {
        tech.supported() && stats.curr_tp >= u16::from(tech.cost)
    }

    /// The character skills a member's SKILL window lists, newest learned
    /// first, with their slot: every learned skill whose record is a battle
    /// skill (`targeting & $10`).
    pub(crate) fn listed_skills<'a>(
        stats: &Stats,
        record: impl Fn(u8) -> Option<&'a Skill>,
    ) -> Vec<(usize, &'a Skill)> {
        stats
            .skills
            .iter()
            .copied()
            .enumerate()
            .rev()
            .filter(|(_, id)| *id != 0)
            .filter_map(|(slot, id)| record(id).map(|s| (slot, s)))
            .filter(|(_, s)| s.targeting & 0x10 != 0)
            .collect()
    }

    /// Whether the SKILL window enables the skill in `slot` for a member with
    /// `stats`: a supported record with a use left, and a weapon in hand when
    /// the record wants one (`armed`).
    pub(crate) fn skill_enabled(stats: &Stats, slot: usize, skill: &Skill, armed: bool) -> bool {
        skill.supported() && stats.curr_skill_uses[slot] > 0 && (!skill.requires_weapon || armed)
    }

    /// The items a member's ITEM window lists: their equipment, then the
    /// pack's slots, each a record the battle knows.
    pub(crate) fn listed_items<'a>(
        equipment: &[u8],
        inventory: &[u8],
        record: impl Fn(u8) -> Option<&'a BattleItem>,
    ) -> Vec<(ItemSource, &'a BattleItem)> {
        equipment
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, id)| (ItemSource::Equipment(slot as u8), id))
            .chain(
                inventory
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(slot, id)| (ItemSource::Inventory(slot as u8), id)),
            )
            .filter(|(_, id)| *id != 0)
            .filter_map(|(source, id)| record(id).map(|item| (source, item)))
            .collect()
    }

    /// Whether the ITEM window enables `item` before any reservation: a
    /// supported record. The open window also refuses a pack copy an earlier
    /// actor's order reserved ([`CommandsMenu::reserved`]).
    pub(crate) fn item_enabled(item: &BattleItem) -> bool {
        item.supported()
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
                    let enabled = self
                        .actor_fighter()
                        .is_some_and(|f| CommandsMenu::technique_enabled(&f.stats, tech));
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
                        CommandsMenu::skill_enabled(
                            stats,
                            *slot,
                            skill,
                            self.armed_fighters.contains(&actor),
                        ),
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
                        CommandsMenu::item_enabled(item) && !self.reserved(*source),
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
                self.open_actor();
            }
            // `Battle_BackFromTechs` (`ps4.asm:2792`) and its twins reopen the
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
                    self.open_page(MenuPage::Techniques);
                    return None;
                }
                2 => {
                    self.set_command_byte(3);
                    self.open_page(MenuPage::Skills);
                    return None;
                }
                3 => {
                    self.set_command_byte(4);
                    self.open_page(MenuPage::Items);
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
    ///
    /// While a window grows, the frame is `Battle_CreateWindow`'s and no
    /// button is read; the cursor object is created on the frame the window
    /// is whole, so its clock starts after.
    pub(crate) fn input(&mut self, previous: Pad, pad: Pad) -> Option<RoundOrders> {
        if self.growing() {
            self.grow();
            return None;
        }
        self.whole_unread = false;
        self.age = self.age.saturating_add(1);
        match self.page {
            MenuPage::Actions => {
                self.strip_step(previous, pad);
                self.remember();
            }
            MenuPage::Techniques | MenuPage::Skills | MenuPage::Items => {
                let flipped = self.page_step(previous, pad);
                self.remember();
                if flipped {
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
            // `Battle_TechNextWin` and twins (`ps4.asm:2739`, `3078`,
            // `3503`) grow the next page's window from nothing.
            self.start_growth();
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
        CommandsMenu::listed_items(&equipment, self.inventory.slots(), |id| self.items.get(&id))
            .iter()
            .map(|(source, item)| (*source, item.id))
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
}

#[path = "commands_view.rs"]
mod view;

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
