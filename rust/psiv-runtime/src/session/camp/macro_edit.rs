//! Camp `MACRO` editor (`MenuOptions_Macro` at `ps4.asm:117452`, `Win_MacroList`,
//! `Win_MacroEraseConfirm`, `Win_MacroCharList` at `ps4.asm:131860-131979`).

use psiv_core::{CharId, MACRO_COMMANDS, MACRO_COUNT, MacroCommand, MacroRecord};

use crate::Runtime;
use crate::session::battle::menu::macros::{macro_command_label, macro_is_empty};
use crate::session::battle::{MacroEntryView, MacroSlotState, MacroSlotView};

use super::{CampPage, CampView, wrap};

/// The five action labels offered when building a macro entry.
pub const MACRO_ACTIONS: [&str; 5] = ["ATTACK", "TECH", "SKILL", "ITEM", "DEFENSE"];

/// State of the Camp `MACRO` viewer and builder.
#[derive(Clone, Debug, Default)]
pub struct MacroDraft {
    /// Selected macro slot (`0..8`, `'A'`..=`'H'`).
    pub slot: usize,
    /// Formatted preview of all 8 saved macros for drawing.
    pub slots: Vec<MacroSlotView>,
    /// Unassigned `(character_id, name)` party members left to assign.
    pub remaining_members: Vec<(u8, String)>,
    /// Cursor on `remaining_members`.
    pub member_cursor: usize,
    /// Cursor on `MACRO_ACTIONS` (`0..5`).
    pub action_cursor: usize,
    /// Currently selected character id while picking action/ability.
    pub selected_character: Option<u8>,
    /// Selected retail command index (`2` tech, `3` skill, `4` item) on `MacroPickAbility`.
    pub selected_command_index: u8,
    /// Available `(ability_or_item_id, name)` choices on `MacroPickAbility`.
    pub ability_options: Vec<(u8, String)>,
    /// Cursor on `ability_options`.
    pub ability_cursor: usize,
    /// Commands chosen so far in the in-progress macro draft.
    pub commands: Vec<MacroCommand>,
    /// YES (`0`) or NO (`1`) on `MacroEraseConfirm`.
    pub erase_confirm: usize,
}

impl CampView {
    /// Opens Camp `MACRO` (`MenuOptions_Macro`, `ps4.asm:117452`): requires at
    /// least two party members, matching `tst.b (Current_Party_Slot_2).w; bmi`.
    pub(super) fn begin_macro_menu(&mut self, runtime: &Runtime) {
        if self.snapshot.party.len() < 2 {
            self.message = "ONLY ONE MEMBER!".to_owned();
            self.page = CampPage::MacroNotice;
            return;
        }
        self.macro_draft = MacroDraft {
            slot: 0,
            slots: build_camp_macro_slots(runtime),
            ..MacroDraft::default()
        };
        self.page = CampPage::MacroList;
    }

    pub(super) fn sync_macros(&mut self, runtime: &Runtime) {
        if matches!(
            self.page,
            CampPage::MacroList
                | CampPage::MacroEraseConfirm
                | CampPage::MacroPickMember
                | CampPage::MacroPickAction
                | CampPage::MacroPickAbility
        ) {
            self.macro_draft.slots = build_camp_macro_slots(runtime);
        }
    }

    pub(super) fn macro_frame(
        &mut self,
        runtime: &mut Runtime,
        up: bool,
        down: bool,
        accept: bool,
    ) {
        match self.page {
            CampPage::MacroNotice => {
                if accept {
                    self.page = CampPage::Root;
                }
            }
            CampPage::MacroList => {
                if up {
                    self.macro_draft.slot = wrap(self.macro_draft.slot, MACRO_COUNT, false);
                } else if down {
                    self.macro_draft.slot = wrap(self.macro_draft.slot, MACRO_COUNT, true);
                } else if accept {
                    let record = runtime.game().macros()[self.macro_draft.slot];
                    if !macro_is_empty(&record) {
                        let letter = (b'A' + self.macro_draft.slot as u8) as char;
                        self.message = format!("ERASE MACRO {letter}?");
                        self.macro_draft.erase_confirm = 1; // default NO
                        self.page = CampPage::MacroEraseConfirm;
                    } else {
                        self.start_macro_builder(runtime);
                    }
                }
            }
            CampPage::MacroEraseConfirm => {
                if up || down {
                    self.macro_draft.erase_confirm = 1 - self.macro_draft.erase_confirm;
                } else if accept {
                    if self.macro_draft.erase_confirm == 0 {
                        runtime.game.macros_mut()[self.macro_draft.slot] = MacroRecord::default();
                        self.macro_draft.slots = build_camp_macro_slots(runtime);
                    }
                    self.page = CampPage::MacroList;
                }
            }
            CampPage::MacroPickMember => {
                let len = self.macro_draft.remaining_members.len();
                if len == 0 {
                    self.commit_macro_draft(runtime);
                } else if up {
                    self.macro_draft.member_cursor =
                        wrap(self.macro_draft.member_cursor, len, false);
                } else if down {
                    self.macro_draft.member_cursor =
                        wrap(self.macro_draft.member_cursor, len, true);
                } else if accept {
                    let (char_id, _) =
                        self.macro_draft.remaining_members[self.macro_draft.member_cursor];
                    self.macro_draft.selected_character = Some(char_id);
                    self.macro_draft.action_cursor = 0;
                    self.page = CampPage::MacroPickAction;
                }
            }
            CampPage::MacroPickAction => {
                if up {
                    self.macro_draft.action_cursor =
                        wrap(self.macro_draft.action_cursor, MACRO_ACTIONS.len(), false);
                } else if down {
                    self.macro_draft.action_cursor =
                        wrap(self.macro_draft.action_cursor, MACRO_ACTIONS.len(), true);
                } else if accept {
                    let Some(char_id) = self.macro_draft.selected_character else {
                        self.page = CampPage::MacroPickMember;
                        return;
                    };
                    match self.macro_draft.action_cursor {
                        0 => self.push_macro_entry(runtime, char_id, 1, 0),
                        4 => self.push_macro_entry(runtime, char_id, 5, 0),
                        action @ (1..=3) => {
                            let cmd_index = (action + 1) as u8; // 2=TECH, 3=SKILL, 4=ITEM
                            let options = collect_ability_options(runtime, char_id, cmd_index);
                            if !options.is_empty() {
                                self.macro_draft.selected_command_index = cmd_index;
                                self.macro_draft.ability_options = options;
                                self.macro_draft.ability_cursor = 0;
                                self.page = CampPage::MacroPickAbility;
                            }
                        }
                        _ => {}
                    }
                }
            }
            CampPage::MacroPickAbility => {
                let len = self.macro_draft.ability_options.len();
                if len == 0 {
                    self.page = CampPage::MacroPickAction;
                } else if up {
                    self.macro_draft.ability_cursor =
                        wrap(self.macro_draft.ability_cursor, len, false);
                } else if down {
                    self.macro_draft.ability_cursor =
                        wrap(self.macro_draft.ability_cursor, len, true);
                } else if accept {
                    let Some(char_id) = self.macro_draft.selected_character else {
                        self.page = CampPage::MacroPickMember;
                        return;
                    };
                    let (ability_id, _) =
                        self.macro_draft.ability_options[self.macro_draft.ability_cursor];
                    let cmd_index = self.macro_draft.selected_command_index;
                    self.push_macro_entry(runtime, char_id, cmd_index, ability_id);
                }
            }
            _ => {}
        }
    }

    pub(super) fn macro_go_back(&mut self) -> bool {
        match self.page {
            CampPage::MacroNotice | CampPage::MacroList => {
                self.page = CampPage::Root;
                false
            }
            CampPage::MacroEraseConfirm => {
                self.page = CampPage::MacroList;
                false
            }
            CampPage::MacroPickMember => {
                if let Some(last) = self.macro_draft.commands.pop() {
                    let name = self
                        .snapshot
                        .party
                        .iter()
                        .find(|m| m.id == last.character_id)
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| format!("C{}", last.character_id));
                    self.macro_draft
                        .remaining_members
                        .insert(0, (last.character_id, name));
                    self.macro_draft.member_cursor = 0;
                } else {
                    self.page = CampPage::MacroList;
                }
                false
            }
            CampPage::MacroPickAction => {
                self.page = CampPage::MacroPickMember;
                false
            }
            CampPage::MacroPickAbility => {
                self.page = CampPage::MacroPickAction;
                false
            }
            _ => false,
        }
    }

    fn start_macro_builder(&mut self, runtime: &Runtime) {
        self.macro_draft.commands.clear();
        self.macro_draft.remaining_members = runtime
            .camp_state()
            .party
            .into_iter()
            .map(|m| (m.id, m.name))
            .collect();
        self.macro_draft.member_cursor = 0;
        self.macro_draft.action_cursor = 0;
        self.macro_draft.selected_character = None;
        self.macro_draft.ability_options.clear();
        self.macro_draft.ability_cursor = 0;
        self.page = CampPage::MacroPickMember;
    }

    fn push_macro_entry(
        &mut self,
        runtime: &mut Runtime,
        character_id: u8,
        command_index: u8,
        ability_id: u8,
    ) {
        self.macro_draft.commands.push(MacroCommand {
            character_id,
            command_index,
            ability_id,
            reserved: 0,
        });
        if let Some(pos) = self
            .macro_draft
            .remaining_members
            .iter()
            .position(|(id, _)| *id == character_id)
        {
            self.macro_draft.remaining_members.remove(pos);
        }
        self.macro_draft.member_cursor = 0;
        self.macro_draft.selected_character = None;
        if self.macro_draft.remaining_members.is_empty()
            || self.macro_draft.commands.len() >= MACRO_COMMANDS
        {
            self.commit_macro_draft(runtime);
        } else {
            self.page = CampPage::MacroPickMember;
        }
    }

    fn commit_macro_draft(&mut self, runtime: &mut Runtime) {
        let mut record = MacroRecord::default();
        for (slot, cmd) in self
            .macro_draft
            .commands
            .iter()
            .take(MACRO_COMMANDS)
            .enumerate()
        {
            record.commands[slot] = *cmd;
        }
        runtime.game.macros_mut()[self.macro_draft.slot] = record;
        self.macro_draft.commands.clear();
        self.macro_draft.slots = build_camp_macro_slots(runtime);
        self.page = CampPage::MacroList;
    }
}

fn collect_ability_options(
    runtime: &Runtime,
    character_id: u8,
    command_index: u8,
) -> Vec<(u8, String)> {
    let Some(data) = runtime.battle_data() else {
        return Vec::new();
    };
    let Some(character) = runtime.game().roster().get(CharId(character_id)) else {
        return Vec::new();
    };
    match command_index {
        2 => character
            .techniques
            .iter()
            .copied()
            .filter(|&id| id != 0)
            .filter_map(|id| {
                let tech = data.technique(id)?;
                // Battle-usable techniques have targeting high nibble 1 or 3.
                matches!(tech.targeting >> 4, 1 | 3).then(|| (id, tech.name.clone()))
            })
            .collect(),
        3 => character
            .skills
            .iter()
            .copied()
            .filter(|&id| id != 0)
            .filter_map(|id| {
                let skill = data.skill(id)?;
                matches!(skill.targeting >> 4, 1 | 3).then(|| (id, skill.name.clone()))
            })
            .collect(),
        4 => {
            let mut seen = Vec::new();
            let mut out = Vec::new();
            for id in character
                .equipment
                .iter()
                .copied()
                .chain(runtime.game().inventory().slots().iter().copied())
            {
                if id == 0 || seen.contains(&id) {
                    continue;
                }
                if let Some(item) = data.battle_item(id)
                    && item.effect != 0
                {
                    seen.push(id);
                    out.push((id, item.name.clone()));
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn build_camp_macro_slots(runtime: &Runtime) -> Vec<MacroSlotView> {
    let data = runtime.battle_data();
    let macros = runtime.game().macros();
    (0..MACRO_COUNT)
        .map(|index| {
            let letter = (b'A' + index as u8) as char;
            let record = &macros[index];
            if macro_is_empty(record) {
                return MacroSlotView {
                    letter,
                    state: MacroSlotState::Empty,
                    entries: Vec::new(),
                };
            }
            let entries = record
                .commands
                .iter()
                .take_while(|c| c.command_index != 0)
                .map(|cmd| {
                    let character_name = runtime
                        .game()
                        .roster()
                        .get(CharId(cmd.character_id))
                        .map(psiv_core::battle::Stats::display_name)
                        .unwrap_or_else(|| format!("C{}", cmd.character_id));
                    MacroEntryView {
                        character_id: cmd.character_id,
                        character_name,
                        command_label: macro_command_label(*cmd, data),
                        available: true,
                    }
                })
                .collect();
            MacroSlotView {
                letter,
                state: MacroSlotState::Ready,
                entries,
            }
        })
        .collect()
}
