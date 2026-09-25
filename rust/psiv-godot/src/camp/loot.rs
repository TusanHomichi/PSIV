//! Ordinary field chest windows using the cartridge's font and window tiles.
use super::draw::{draw_text, frame};
use super::layout::{CHILD_CURSOR_PATTERN, CellRect};
use super::{CampMenu, DrawList, Mode, chrome::CampChrome};
use godot::classes::Input;
use godot::prelude::*;
use psiv_core::ChestOutcome;
use psiv_runtime::{LootResult, Runtime};

impl CampMenu {
    pub(crate) fn is_loot(&self) -> bool {
        matches!(
            self.mode,
            Mode::LootMessage
                | Mode::LootItems
                | Mode::LootDiscardConfirm
                | Mode::LootReturnConfirm
                | Mode::LootBlocked
        )
    }

    pub(super) fn open_loot(&mut self, runtime: &Runtime) {
        self.open(runtime);
        self.show_loot_message(runtime);
        self.item_selection = 0;
        if runtime
            .loot_state()
            .is_some_and(|loot| loot.outcome != ChestOutcome::AlreadyOpen)
        {
            self.sound_request = Some(0xE1);
        }
    }

    fn show_loot_message(&mut self, runtime: &Runtime) {
        let Some(loot) = runtime.loot_state() else {
            self.close();
            return;
        };
        let opened = if loot.white {
            "Box has been opened!"
        } else {
            "Chest has been opened!"
        };
        self.message = match loot.outcome {
            ChestOutcome::AlreadyOpen => "It is already open.".into(),
            ChestOutcome::Took { .. } => format!("{opened}\n{} is procured!", loot.item_name),
            ChestOutcome::Meseta { amount } => format!("{opened}\n{amount} meseta is procured!"),
            ChestOutcome::Full { .. } => format!("{opened}\nItempack is full!\n{}", loot.item_name),
        };
        self.mode = Mode::LootMessage;
    }

    pub(super) fn handle_loot_input(&mut self, runtime: &mut Runtime, cancel: bool) {
        let input = Input::singleton();
        let accept = input.is_action_just_pressed("ui_accept");
        let up = input.is_action_just_pressed("ui_up");
        let down = input.is_action_just_pressed("ui_down");
        match self.mode {
            Mode::LootMessage if accept || cancel => {
                if runtime.acknowledge_loot() {
                    self.close();
                } else {
                    self.mode = Mode::LootItems;
                }
            }
            Mode::LootBlocked if accept || cancel => {
                self.show_loot_message(runtime);
                self.mode = Mode::LootItems;
            }
            Mode::LootItems => {
                let len = self.snapshot.inventory.len();
                if up {
                    self.item_selection = super::wrap(self.item_selection, len, false);
                } else if down {
                    self.item_selection = super::wrap(self.item_selection, len, true);
                } else if input.is_action_just_pressed("ui_left") {
                    self.item_selection = self.item_selection.saturating_sub(8);
                } else if input.is_action_just_pressed("ui_right") {
                    self.item_selection = (self.item_selection + 8).min(len.saturating_sub(1));
                } else if cancel {
                    self.mode = Mode::LootReturnConfirm;
                    self.target_selection = 1; // NO is the cartridge's default.
                } else if accept && len > 0 {
                    self.mode = Mode::LootDiscardConfirm;
                    self.target_selection = 1;
                }
            }
            Mode::LootReturnConfirm | Mode::LootDiscardConfirm => {
                if up || down {
                    self.target_selection ^= 1;
                } else if cancel || (accept && self.target_selection == 1) {
                    self.mode = Mode::LootItems;
                } else if accept {
                    if self.mode == Mode::LootReturnConfirm {
                        if runtime.return_loot() {
                            self.sound_request = Some(0xE1);
                            self.close();
                        }
                    } else if let Some(item) = self.snapshot.inventory.get(self.item_selection) {
                        let name = item.name.clone();
                        match runtime.discard_for_loot(item.slot) {
                            LootResult::Took { .. } => {
                                self.show_loot_message(runtime);
                                let found = runtime
                                    .loot_state()
                                    .map(|loot| loot.item_name.as_str())
                                    .unwrap_or("");
                                self.message = format!("{name} discarded\n{found} is procured.");
                            }
                            LootResult::Necessary => {
                                self.message = "This item may\nbecome necessary.".into();
                                self.mode = Mode::LootBlocked;
                            }
                            LootResult::Invalid => self.mode = Mode::LootItems,
                        }
                    }
                }
            }
            _ => {}
        }
        self.sync(runtime);
    }

    pub(super) fn draw_loot(&self, chrome: &CampChrome, list: &mut DrawList) {
        frame(chrome, &mut list.quads, CellRect::new(1, 1, 38, 8));
        for (row, text) in self.message.lines().enumerate().take(3) {
            draw_text(chrome, &mut list.quads, text, (3, 3 + row as i32 * 2));
        }
        if self.mode == Mode::LootMessage || self.mode == Mode::LootBlocked {
            return;
        }
        frame(chrome, &mut list.quads, CellRect::new(13, 9, 26, 18));
        let page = self.item_selection / 8;
        for (row, item) in self
            .snapshot
            .inventory
            .iter()
            .skip(page * 8)
            .take(8)
            .enumerate()
        {
            draw_text(
                chrome,
                &mut list.quads,
                &item.name,
                (16, 10 + row as i32 * 2),
            );
        }
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (14, 10 + (self.item_selection % 8) as i32 * 2),
        ) {
            list.quads.push(quad);
        }
        if matches!(
            self.mode,
            Mode::LootReturnConfirm | Mode::LootDiscardConfirm
        ) {
            frame(chrome, &mut list.quads, CellRect::new(1, 9, 38, 10));
            let prompt = if self.mode == Mode::LootReturnConfirm {
                "Give up this item?"
            } else {
                "Discard the selected item?"
            };
            if self.mode == Mode::LootDiscardConfirm
                && let Some(item) = self.snapshot.inventory.get(self.item_selection)
            {
                draw_text(chrome, &mut list.quads, &item.name, (3, 10));
            }
            draw_text(chrome, &mut list.quads, prompt, (3, 12));
            draw_text(chrome, &mut list.quads, "YES", (6, 14));
            draw_text(chrome, &mut list.quads, "NO", (6, 16));
            if let Some(quad) = chrome.window_word(
                CHILD_CURSOR_PATTERN,
                (4, 14 + self.target_selection as i32 * 2),
            ) {
                list.quads.push(quad);
            }
        }
    }
}
