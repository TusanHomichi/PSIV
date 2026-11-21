//! Battle `MACR` selector (`Battle_OpenMacroLetters`, `Battle_OpenMacroChar`,
//! `Battle_MacroWindows`, `ps4.asm:6920-7330`).

use psiv_core::battle::{BattleData, RoundOrders, Side, weapon_reach};
use psiv_core::{MACRO_COUNT, MacroCommand, MacroRecord};

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::super::view::{MacroEntryView, MacroSlotState, MacroSlotView, MenuView};
use super::accept_pressed;

/// The battle macro selection window (`A`..`H` + active macro's character list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MacroWindow {
    /// Selected macro index (`0..8`, `Battle_Macro_Index`).
    pub(crate) cursor: usize,
}

impl MacroWindow {
    /// Reads one frame of input (`Battle_MacroWindows`, `ps4.asm:7260`).
    ///
    /// Up/Down wrap across all 8 slots (`A`..`H`). Accept chooses any non-empty
    /// macro (both fully ready and partially available macros are selectable,
    /// matching `loc_4F6A`). Cancel is handled by `BattleMode`.
    pub(crate) fn input(
        &mut self,
        runtime: &Runtime,
        previous: Pad,
        pad: Pad,
    ) -> Option<RoundOrders> {
        if pad.is_pressed(previous, Button::Up) {
            self.cursor = (self.cursor + MACRO_COUNT - 1) % MACRO_COUNT;
        }
        if pad.is_pressed(previous, Button::Down) {
            self.cursor = (self.cursor + 1) % MACRO_COUNT;
        }
        if !accept_pressed(previous, pad) {
            return None;
        }
        let record = runtime.game().macros()[self.cursor];
        if macro_is_empty(&record) {
            return None;
        }
        Some(RoundOrders::Macro(record))
    }

    /// Snapshot of all 8 macro slots for presentation (`loc_4C72` + `loc_4E22`).
    pub(crate) fn view(self, runtime: &Runtime) -> MenuView {
        let macros = runtime.game().macros();
        let slots = (0..MACRO_COUNT)
            .map(|index| build_slot_view(index, &macros[index], runtime))
            .collect();
        MenuView::Macros {
            cursor: self.cursor.min(MACRO_COUNT - 1),
            slots,
        }
    }
}

pub(crate) fn macro_is_empty(record: &MacroRecord) -> bool {
    record.commands[0].command_index == 0
}

/// Evaluates one macro slot against the live battle roster and shared pack
/// (`loc_4C72..loc_4DEA`, `ps4.asm:6977-7135`).
fn build_slot_view(index: usize, record: &MacroRecord, runtime: &Runtime) -> MacroSlotView {
    let letter = (b'A' + index as u8) as char;
    if macro_is_empty(record) {
        return MacroSlotView {
            letter,
            state: MacroSlotState::Empty,
            entries: Vec::new(),
        };
    }
    let roster = runtime.battle_roster();
    let data = runtime.battle_data();
    let inventory = runtime.game().inventory().slots();
    let mut reserved_inv = [false; 40];
    let mut entries = Vec::new();
    let mut all_available = true;

    for cmd in &record.commands {
        if cmd.command_index == 0 {
            break;
        }
        let fighter = roster.and_then(|r| {
            r.side(Side::Party)
                .find(|f| f.character == Some(cmd.character_id))
        });
        let character_name = fighter
            .map(|f| f.name.clone())
            .or_else(|| {
                runtime
                    .game()
                    .roster()
                    .get(psiv_core::CharId(cmd.character_id))
                    .map(psiv_core::battle::Stats::display_name)
            })
            .unwrap_or_else(|| format!("C{}", cmd.character_id));
        let command_label = macro_command_label(*cmd, data);
        let available = match (fighter, data) {
            (Some(fighter), Some(data)) => {
                command_available(fighter, *cmd, data, inventory, &mut reserved_inv)
            }
            _ => false,
        };
        if !available {
            all_available = false;
        }
        entries.push(MacroEntryView {
            character_id: cmd.character_id,
            character_name,
            command_label,
            available,
        });
    }

    let state = if entries.is_empty() {
        MacroSlotState::Empty
    } else if all_available {
        MacroSlotState::Ready
    } else {
        MacroSlotState::Partial
    };

    MacroSlotView {
        letter,
        state,
        entries,
    }
}

/// Formats one macro command's display text (`loc_4ECE`, `ps4.asm:7209-7258`).
pub(crate) fn macro_command_label(cmd: MacroCommand, data: Option<&BattleData>) -> String {
    match cmd.command_index {
        1 => "ATTACK".into(),
        2 => data
            .and_then(|d| d.technique(cmd.ability_id))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("TECH {}", cmd.ability_id)),
        3 => data
            .and_then(|d| d.skill(cmd.ability_id))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("SKILL {}", cmd.ability_id)),
        4 => data
            .and_then(|d| d.item(cmd.ability_id).ok().map(|i| i.name.clone()))
            .or_else(|| data.and_then(|d| d.battle_item(cmd.ability_id).map(|i| i.name.clone())))
            .unwrap_or_else(|| format!("ITEM {}", cmd.ability_id)),
        5 => "DEFENSE".into(),
        _ => String::new(),
    }
}

/// `loc_4CC4..loc_4DEA` (`ps4.asm:7013-7135`).
fn command_available(
    fighter: &psiv_core::battle::Fighter,
    cmd: MacroCommand,
    data: &BattleData,
    inventory: &[u8; 40],
    reserved_inv: &mut [bool; 40],
) -> bool {
    if !fighter.is_alive() || !fighter.stats.can_act() {
        return false;
    }
    match cmd.command_index {
        1 => weapon_reach(&fighter.stats, data).ok().flatten().is_some(),
        2 => data
            .technique(cmd.ability_id)
            .is_some_and(|t| fighter.stats.curr_tp >= u16::from(t.cost)),
        3 => {
            let Some(slot) = fighter
                .stats
                .skills
                .iter()
                .position(|&id| id == cmd.ability_id)
            else {
                return false;
            };
            if fighter.stats.curr_skill_uses[slot] == 0 {
                return false;
            }
            let Some(skill) = data.skill(cmd.ability_id) else {
                return false;
            };
            !skill.requires_weapon || weapon_reach(&fighter.stats, data).ok().flatten().is_some()
        }
        4 => {
            if fighter.stats.equipment.contains(&cmd.ability_id) {
                return true;
            }
            if let Some((slot, _)) = inventory
                .iter()
                .enumerate()
                .find(|&(slot, &id)| id == cmd.ability_id && !reserved_inv[slot])
            {
                reserved_inv[slot] = true;
                return true;
            }
            false
        }
        5 => true,
        _ => false,
    }
}
