//! The field camp menu: retail root, ITEM, STATE, STATUS, and equipment view.
//!
//! [`CampMenu`] owns only presentation state and input routing. Persistent
//! values and item effects stay in `psiv-runtime`; opening the node is also
//! the place where `Field` applies the cartridge's field-suspension seam.

mod chrome;
mod equipment;
mod layout;
mod status;

use godot::classes::{INode2D, Image, ImageTexture, Input, Node2D};
use godot::global::Key;
use godot::prelude::*;
use std::path::PathBuf;

use psiv_runtime::{CampCharacter, CampState, CampUseResult, Runtime};

use crate::Field;

use self::chrome::{CampChrome, Quad};
use self::layout::{
    CHARACTER_SUMMARY, CHILD_CURSOR_PATTERN, ITEM_EMPTY_TEXT, ITEM_MESSAGE, MESETA,
    ROOT_CURSOR_CELL, ROOT_MENU, ROOT_TEXT, SAVE_SLOT_TEXT, SAVE_SLOTS_OPTIONS,
    SELECTED_CURSOR_PATTERN, STATE_CURSOR_CELL, STATE_SAVE_OPTIONS, STATE_SAVE_TEXT, STATE_TEXT,
    STATUS_EQUIPMENT, STATUS_EXP, STATUS_INFO, STATUS_PORTRAIT, STATUS_STATS, STATUS_TEXT,
};
use self::status::{draw_level, draw_status_pair, draw_status_text};

/// Tier-1 browse geometry for inventory and target pages. The tape only
/// decoded the empty ITEM branch; these child surfaces keep live inventory
/// usable while leaving that documented gap explicit.
const ITEM_LIST: layout::CellRect = layout::CellRect::new(14, 2, 24, 20);
const ITEM_TARGET: layout::CellRect = layout::CellRect::new(18, 6, 18, 10);

const ROOT_OPTIONS: [&str; 7] = ["ITEM", "TECH", "SKILL", "EQUIP", "STATE", "MUMBL", "MACRO"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Closed,
    Root,
    ItemEmpty,
    ItemList,
    ItemTarget,
    ItemResult,
    EquipCharacters,
    EquipStats,
    EquipItems,
    EquipResult,
    State,
    SaveSlots,
    SaveResult,
    Status,
    Unsupported,
}

struct DrawList {
    quads: Vec<Quad>,
    portrait: Option<(Gd<ImageTexture>, Rect2)>,
}

/// A visible camp menu node. `Field` owns the node and gives it runtime
/// snapshots; this type never owns game rules or mutable game state.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct CampMenu {
    base: Base<Node2D>,
    chrome: Option<CampChrome>,
    portrait: Option<Gd<ImageTexture>>,
    mode: Mode,
    root_selection: usize,
    state_selection: usize,
    save_selection: usize,
    item_selection: usize,
    target_selection: usize,
    status_selection: usize,
    status_from_state: bool,
    equipment_character_selection: usize,
    equipment_slot_selection: usize,
    equipment_item_selection: usize,
    equipment_options: Vec<psiv_runtime::CampItem>,
    snapshot: CampState,
    message: String,
    x_down: bool,
}

#[godot_api]
impl INode2D for CampMenu {
    fn init(base: Base<Node2D>) -> Self {
        CampMenu {
            base,
            chrome: None,
            portrait: None,
            mode: Mode::Closed,
            root_selection: 0,
            state_selection: 0,
            save_selection: 0,
            item_selection: 0,
            target_selection: 0,
            status_selection: 0,
            status_from_state: false,
            equipment_character_selection: 0,
            equipment_slot_selection: 0,
            equipment_item_selection: 0,
            equipment_options: Vec::new(),
            snapshot: CampState::default(),
            message: String::new(),
            x_down: false,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(1000);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        let Some(list) = self.draw_list() else {
            return;
        };
        for quad in list.quads {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        if let Some((portrait, rect)) = list.portrait {
            self.base_mut().draw_texture_rect(&portrait, rect, true);
        }
    }
}

impl CampMenu {
    /// Loads the shared retail window/font assets and the Chaz portrait used
    /// by the decoded STATUS capture.
    pub(crate) fn configure(&mut self, pack_dir: &str, set: psiv_data::DialogueSet) {
        self.chrome = CampChrome::build(pack_dir, &set);
        if self.chrome.is_none() {
            godot_error!("camp menu chrome failed to load");
        }
        let path = format!("{pack_dir}/dialogue/portraits/01_Chaz.png");
        self.portrait = Image::load_from_file(&GString::from(path.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        if self.portrait.is_none() {
            godot_warn!("camp status portrait failed to load: {path}");
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.mode != Mode::Closed
    }

    pub(crate) fn open(&mut self, runtime: &Runtime) {
        self.mode = Mode::Root;
        self.root_selection = 0;
        self.state_selection = 0;
        self.save_selection = 0;
        self.item_selection = 0;
        self.target_selection = 0;
        self.status_selection = 0;
        self.status_from_state = false;
        self.equipment_character_selection = 0;
        self.equipment_slot_selection = 0;
        self.equipment_item_selection = 0;
        self.equipment_options = runtime.camp_equipment(0);
        self.message.clear();
        self.snapshot = runtime.camp_state();
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn close(&mut self) {
        self.mode = Mode::Closed;
        self.message.clear();
        self.base_mut().set_visible(false);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn sync(&mut self, runtime: &Runtime) {
        if !self.is_open() {
            return;
        }
        self.snapshot = runtime.camp_state();
        self.item_selection = self
            .item_selection
            .min(self.snapshot.inventory.len().saturating_sub(1));
        self.status_selection = self
            .status_selection
            .min(self.snapshot.party.len().saturating_sub(1));
        self.target_selection = self
            .target_selection
            .min(self.snapshot.party.len().saturating_sub(1));
        self.equipment_character_selection = self
            .equipment_character_selection
            .min(self.snapshot.party.len().saturating_sub(1));
        self.equipment_slot_selection = self.equipment_slot_selection.min(3);
        self.equipment_options = runtime.camp_equipment(self.equipment_character_selection);
        self.equipment_item_selection = self
            .equipment_item_selection
            .min(self.equipment_options.len().saturating_sub(1));
        self.base_mut().queue_redraw();
    }

    /// Returns one edge-triggered X/ui_cancel press. X is checked as a
    /// physical key as well as through Godot's action so the keyboard path is
    /// usable in a stock project with no hand-edited InputMap.
    pub(crate) fn cancel_requested(&mut self) -> bool {
        let input = Input::singleton();
        let x = input.is_physical_key_pressed(Key::X);
        let x_edge = x && !self.x_down;
        self.x_down = x;
        input.is_action_just_pressed("ui_cancel") || x_edge
    }

    /// Handles one frame of camp input and runtime item commands.
    pub(crate) fn handle_input(&mut self, runtime: &mut Runtime) {
        if self.cancel_requested() {
            if self.go_back() {
                self.close();
                return;
            }
            self.sync(runtime);
            return;
        }
        let input = Input::singleton();
        let up = input.is_action_just_pressed("ui_up");
        let down = input.is_action_just_pressed("ui_down");
        let accept = input.is_action_just_pressed("ui_accept");
        match self.mode {
            Mode::Root => {
                if up {
                    self.root_selection = wrap(self.root_selection, ROOT_OPTIONS.len(), false);
                } else if down {
                    self.root_selection = wrap(self.root_selection, ROOT_OPTIONS.len(), true);
                } else if accept {
                    self.confirm_root(runtime);
                }
            }
            Mode::ItemEmpty => {
                if accept {
                    self.mode = Mode::Root;
                }
            }
            Mode::ItemList => {
                if self.snapshot.inventory.is_empty() {
                    self.mode = Mode::ItemEmpty;
                } else if up {
                    self.item_selection =
                        wrap(self.item_selection, self.snapshot.inventory.len(), false);
                } else if down {
                    self.item_selection =
                        wrap(self.item_selection, self.snapshot.inventory.len(), true);
                } else if accept {
                    self.confirm_item(runtime);
                }
            }
            Mode::ItemTarget => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::ItemResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.target_selection =
                        wrap(self.target_selection, self.snapshot.party.len(), false);
                } else if down {
                    self.target_selection =
                        wrap(self.target_selection, self.snapshot.party.len(), true);
                } else if accept {
                    self.use_selected_item(runtime);
                }
            }
            Mode::ItemResult => {
                if accept {
                    self.mode = if self.snapshot.inventory.is_empty() {
                        Mode::Root
                    } else {
                        Mode::ItemList
                    };
                }
            }
            Mode::EquipCharacters => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::EquipResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.equipment_character_selection = wrap(
                        self.equipment_character_selection,
                        self.snapshot.party.len(),
                        false,
                    );
                } else if down {
                    self.equipment_character_selection = wrap(
                        self.equipment_character_selection,
                        self.snapshot.party.len(),
                        true,
                    );
                } else if accept {
                    self.equipment_slot_selection = 0;
                    self.equipment_options =
                        runtime.camp_equipment(self.equipment_character_selection);
                    self.mode = Mode::EquipStats;
                }
            }
            Mode::EquipStats => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::EquipResult;
                    self.message = "NO PARTY MEMBER".to_owned();
                } else if up {
                    self.equipment_slot_selection = wrap(self.equipment_slot_selection, 4, false);
                } else if down {
                    self.equipment_slot_selection = wrap(self.equipment_slot_selection, 4, true);
                } else if accept {
                    self.confirm_equipment_slot(runtime);
                }
            }
            Mode::EquipItems => {
                if self.equipment_options.is_empty() {
                    self.mode = Mode::EquipResult;
                    self.message = "NO EQUIPMENT".to_owned();
                } else if up {
                    self.equipment_item_selection = wrap(
                        self.equipment_item_selection,
                        self.equipment_options.len(),
                        false,
                    );
                } else if down {
                    self.equipment_item_selection = wrap(
                        self.equipment_item_selection,
                        self.equipment_options.len(),
                        true,
                    );
                } else if accept {
                    self.equip_selected_item(runtime);
                }
            }
            Mode::EquipResult => {
                if accept {
                    self.mode = Mode::EquipStats;
                }
            }
            Mode::State => {
                if up {
                    self.state_selection = wrap(self.state_selection, 3, false);
                } else if down {
                    self.state_selection = wrap(self.state_selection, 3, true);
                } else if accept {
                    if self.state_selection == 0 {
                        self.status_selection = 0;
                        self.status_from_state = true;
                        self.mode = Mode::Status;
                    } else if self.state_selection == 1 {
                        self.mode = Mode::Unsupported;
                        self.message = "ORDER NOT READY".to_owned();
                    } else {
                        self.save_selection = 0;
                        self.mode = Mode::SaveSlots;
                    }
                }
            }
            Mode::Status => {
                if self.snapshot.party.is_empty() {
                    self.mode = Mode::State;
                } else if up {
                    self.status_selection =
                        wrap(self.status_selection, self.snapshot.party.len(), false);
                } else if down {
                    self.status_selection =
                        wrap(self.status_selection, self.snapshot.party.len(), true);
                }
            }
            Mode::Unsupported => {
                if accept {
                    self.mode = Mode::Root;
                }
            }
            Mode::SaveSlots => {
                if up {
                    self.save_selection = wrap(self.save_selection, 3, false);
                } else if down {
                    self.save_selection = wrap(self.save_selection, 3, true);
                } else if accept {
                    self.save_selected(runtime);
                }
            }
            Mode::SaveResult => {
                if accept {
                    self.mode = Mode::State;
                }
            }
            Mode::Closed => {}
        }
        self.sync(runtime);
    }

    fn go_back(&mut self) -> bool {
        match self.mode {
            Mode::Closed => true,
            Mode::Root => true,
            Mode::ItemEmpty | Mode::ItemList | Mode::ItemResult | Mode::Unsupported => {
                self.mode = Mode::Root;
                false
            }
            Mode::ItemTarget => {
                self.mode = Mode::ItemList;
                false
            }
            Mode::EquipCharacters => {
                self.mode = Mode::Root;
                false
            }
            Mode::EquipStats => {
                self.mode = Mode::EquipCharacters;
                false
            }
            Mode::EquipItems => {
                self.mode = Mode::EquipStats;
                false
            }
            Mode::EquipResult => {
                self.mode = Mode::EquipStats;
                false
            }
            Mode::State => {
                self.mode = Mode::Root;
                false
            }
            Mode::Status => {
                self.mode = if self.status_from_state {
                    Mode::State
                } else {
                    Mode::Root
                };
                false
            }
            Mode::SaveSlots | Mode::SaveResult => {
                self.mode = Mode::State;
                false
            }
        }
    }

    fn confirm_root(&mut self, runtime: &Runtime) {
        match self.root_selection {
            0 => {
                self.mode = if runtime.camp_state().inventory.is_empty() {
                    Mode::ItemEmpty
                } else {
                    Mode::ItemList
                };
            }
            3 => {
                self.equipment_character_selection = 0;
                self.equipment_slot_selection = 0;
                self.equipment_item_selection = 0;
                self.equipment_options = runtime.camp_equipment(0);
                self.mode = Mode::EquipCharacters;
            }
            4 => self.mode = Mode::State,
            _ => {
                self.mode = Mode::Unsupported;
                self.message = format!("{} NOT READY", ROOT_OPTIONS[self.root_selection]);
            }
        }
    }

    fn confirm_item(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            self.mode = Mode::ItemEmpty;
            return;
        };
        if !item.usable {
            self.mode = Mode::ItemResult;
            self.message = format!("{} NOT USABLE", item.name);
            return;
        }
        if self.snapshot.party.len() == 1 || item.targeting == 5 {
            self.target_selection = 0;
            self.use_selected_item(runtime);
        } else {
            self.target_selection = 0;
            self.mode = Mode::ItemTarget;
        }
    }

    fn use_selected_item(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            self.mode = Mode::ItemEmpty;
            return;
        };
        let Some(character) = self.snapshot.party.get(self.target_selection) else {
            self.mode = Mode::ItemResult;
            self.message = "NO PARTY MEMBER".to_owned();
            return;
        };
        let result = runtime.use_camp_item(item.slot, character.party_slot);
        self.message = use_result_message(result);
        self.mode = Mode::ItemResult;
        self.sync(runtime);
    }

    fn save_selected(&mut self, runtime: &Runtime) {
        match runtime.save_slot(&save_directory(), self.save_selection) {
            Ok(_) => self.message = "FILE SAVED".to_owned(),
            Err(error) => self.message = format!("SAVE ERROR: {error}"),
        }
        self.mode = Mode::SaveResult;
    }

    fn draw_list(&self) -> Option<DrawList> {
        let chrome = self.chrome.as_ref()?;
        let mut list = DrawList {
            quads: Vec::new(),
            portrait: None,
        };
        match self.mode {
            Mode::Root => self.draw_root(chrome, &mut list, true),
            Mode::ItemEmpty => self.draw_item_empty(chrome, &mut list),
            Mode::ItemList => self.draw_item_list(chrome, &mut list),
            Mode::ItemTarget => self.draw_item_target(chrome, &mut list),
            Mode::ItemResult => self.draw_item_result(chrome, &mut list),
            Mode::EquipCharacters => self.draw_equip_characters(chrome, &mut list),
            Mode::EquipStats => self.draw_equip_stats(chrome, &mut list),
            Mode::EquipItems => self.draw_equip_items(chrome, &mut list),
            Mode::EquipResult => self.draw_equip_result(chrome, &mut list),
            Mode::State => self.draw_state(chrome, &mut list),
            Mode::SaveSlots => self.draw_save_slots(chrome, &mut list),
            Mode::SaveResult => self.draw_save_result(chrome, &mut list),
            Mode::Status => self.draw_status(chrome, &mut list),
            Mode::Unsupported => self.draw_unsupported(chrome, &mut list),
            Mode::Closed => return None,
        }
        Some(list)
    }

    fn draw_root(&self, chrome: &CampChrome, list: &mut DrawList, meseta: bool) {
        frame(chrome, &mut list.quads, ROOT_MENU);
        frame(chrome, &mut list.quads, CHARACTER_SUMMARY);
        if meseta {
            draw_meseta(chrome, &mut list.quads, self.snapshot.money);
        }
        draw_root_options(chrome, &mut list.quads);
        let cursor = (
            ROOT_CURSOR_CELL.0,
            ROOT_CURSOR_CELL.1 + self.root_selection as i32 * 2,
        );
        if let Some(quad) = chrome.window_word(SELECTED_CURSOR_PATTERN, cursor) {
            list.quads.push(quad);
        }
        if let Some(character) = self.snapshot.party.first() {
            draw_summary(chrome, &mut list.quads, character);
        }
    }

    fn draw_item_empty(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(
            chrome,
            &mut list.quads,
            ITEM_EMPTY_TEXT.text,
            ITEM_EMPTY_TEXT.cell,
        );
    }

    fn draw_item_list(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_LIST);
        for (row, item) in self.snapshot.inventory.iter().enumerate() {
            if row >= 16 {
                break;
            }
            draw_text(chrome, &mut list.quads, &item.name, (16, 3 + row as i32));
        }
        draw_selectors(
            chrome,
            &mut list.quads,
            (15, 3),
            self.snapshot.inventory.len().min(16),
        );
        if !self.snapshot.inventory.is_empty()
            && let Some(quad) =
                chrome.window_word(CHILD_CURSOR_PATTERN, (15, 3 + self.item_selection as i32))
        {
            list.quads.push(quad);
        }
    }

    fn draw_item_target(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_item_list(chrome, list);
        frame(chrome, &mut list.quads, ITEM_TARGET);
        for (row, character) in self.snapshot.party.iter().enumerate() {
            draw_text(
                chrome,
                &mut list.quads,
                &character.name,
                (20, 7 + row as i32),
            );
        }
        draw_selectors(chrome, &mut list.quads, (19, 7), self.snapshot.party.len());
        if !self.snapshot.party.is_empty()
            && let Some(quad) =
                chrome.window_word(CHILD_CURSOR_PATTERN, (19, 7 + self.target_selection as i32))
        {
            list.quads.push(quad);
        }
    }

    fn draw_item_result(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }

    fn draw_state(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, true);
        // STATE_OPTIONS is the oracle-pinned retail two-row rectangle. This
        // modern branch adds SAVE without rewriting that retail constant.
        frame(chrome, &mut list.quads, STATE_SAVE_OPTIONS);
        for text in STATE_TEXT {
            draw_text(chrome, &mut list.quads, text.text, text.cell);
        }
        draw_text(
            chrome,
            &mut list.quads,
            STATE_SAVE_TEXT.text,
            STATE_SAVE_TEXT.cell,
        );
        draw_selectors(chrome, &mut list.quads, STATE_CURSOR_CELL, 3);
        let cursor = (
            STATE_CURSOR_CELL.0,
            STATE_CURSOR_CELL.1 + self.state_selection as i32 * 2,
        );
        if let Some(quad) = chrome.window_word(CHILD_CURSOR_PATTERN, cursor) {
            list.quads.push(quad);
        }
    }

    fn draw_save_slots(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, true);
        frame(chrome, &mut list.quads, SAVE_SLOTS_OPTIONS);
        for text in SAVE_SLOT_TEXT {
            draw_text(chrome, &mut list.quads, text.text, text.cell);
        }
        draw_selectors(chrome, &mut list.quads, (10, 8), 3);
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (10, 8 + self.save_selection as i32 * 2),
        ) {
            list.quads.push(quad);
        }
    }

    fn draw_save_result(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_save_slots(chrome, list);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }

    fn draw_status(&self, chrome: &CampChrome, list: &mut DrawList) {
        let Some(character) = self.snapshot.party.get(self.status_selection) else {
            return;
        };
        for rect in [
            STATUS_PORTRAIT,
            STATUS_INFO,
            STATUS_STATS,
            STATUS_EQUIPMENT,
            STATUS_EXP,
            MESETA,
        ] {
            frame(chrome, &mut list.quads, rect);
        }
        draw_text(
            chrome,
            &mut list.quads,
            STATUS_TEXT[19].text,
            STATUS_TEXT[19].cell,
        );
        draw_status_text(chrome, &mut list.quads, character, self.snapshot.money);
        if let Some(portrait) = self.portrait.as_ref() {
            list.portrait = Some((
                portrait.clone(),
                Rect2::new(Vector2::new(32.0, 24.0), Vector2::new(64.0, 64.0)),
            ));
        }
    }

    fn draw_unsupported(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, true);
        frame(chrome, &mut list.quads, ITEM_MESSAGE);
        draw_text(chrome, &mut list.quads, &self.message, (8, 22));
    }
}

impl Field {
    /// Opens camp on X/ui_cancel and drives an open camp window. While true,
    /// the caller must return before feeding normal field input.
    pub(crate) fn drive_camp_if_active(&mut self) -> bool {
        if self.camp_menu.is_none() {
            return false;
        }
        let is_open = self
            .camp_menu
            .as_ref()
            .is_some_and(|menu| menu.bind().is_open());
        if !is_open {
            if self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.scene_active())
            {
                return false;
            }
            if !self
                .camp_menu
                .as_mut()
                .is_some_and(|menu| menu.bind_mut().cancel_requested())
            {
                return false;
            }
            let Some(runtime) = self.runtime.as_ref() else {
                return false;
            };
            if let Some(menu) = self.camp_menu.as_mut() {
                menu.bind_mut().open(runtime);
            }
        } else if let Some(runtime) = self.runtime.as_mut()
            && let Some(menu) = self.camp_menu.as_mut()
        {
            menu.bind_mut().handle_input(runtime);
            if !menu.bind().is_open() {
                runtime.set_field_suspended(false);
                self.sync_visuals(false);
                return true;
            }
        }
        let events = if let Some(runtime) = self.runtime.as_mut() {
            runtime.set_field_suspended(true);
            runtime.tick(psiv_core::Input::Neutral)
        } else {
            Vec::new()
        };
        self.process_events(events);
        if let (Some(runtime), Some(menu)) = (self.runtime.as_ref(), self.camp_menu.as_mut()) {
            menu.bind_mut().sync(runtime);
        }
        self.place_camp_menu();
        self.sync_visuals(false);
        true
    }

    /// Debug hook entry point used by `PSIV_DEBUG_CAMP=1`.
    pub(crate) fn open_camp_menu(&mut self) {
        if self
            .runtime
            .as_ref()
            .is_none_or(|runtime| runtime.scene_active())
        {
            return;
        }
        let Some(menu) = self.camp_menu.as_mut() else {
            return;
        };
        if let Some(runtime) = self.runtime.as_ref() {
            menu.bind_mut().open(runtime);
        }
    }

    fn place_camp_menu(&mut self) {
        let Some(camera) = self.camera.as_ref() else {
            return;
        };
        let center = camera.get_position();
        let position = center - Vector2::new(160.0, 112.0);
        if let Some(menu) = self.camp_menu.as_mut() {
            menu.set_position(position);
        }
    }
}

fn wrap(current: usize, count: usize, forward: bool) -> usize {
    if count == 0 {
        return 0;
    }
    if forward {
        (current + 1) % count
    } else if current == 0 {
        count - 1
    } else {
        current - 1
    }
}

fn save_directory() -> PathBuf {
    std::env::var_os("PSIV_SAVE_DIR").map_or_else(|| PathBuf::from("saves"), PathBuf::from)
}

fn frame(chrome: &CampChrome, quads: &mut Vec<Quad>, rect: layout::CellRect) {
    quads.extend(chrome.frame(rect));
}

fn draw_text(chrome: &CampChrome, quads: &mut Vec<Quad>, text: &str, cell: (i32, i32)) {
    quads.extend(chrome.text(text, cell));
}

fn draw_root_options(chrome: &CampChrome, quads: &mut Vec<Quad>) {
    for text in &ROOT_TEXT[4..11] {
        draw_text(chrome, quads, text.text, text.cell);
    }
    draw_selectors(chrome, quads, ROOT_CURSOR_CELL, ROOT_OPTIONS.len());
}

fn draw_selectors(
    chrome: &CampChrome,
    quads: &mut Vec<Quad>,
    first_cell: (i32, i32),
    count: usize,
) {
    for row in 0..count {
        if let Some(quad) = chrome.window_word(
            CHILD_CURSOR_PATTERN,
            (first_cell.0, first_cell.1 + row as i32 * 2),
        ) {
            quads.push(quad);
        }
    }
}

fn draw_meseta(chrome: &CampChrome, quads: &mut Vec<Quad>, money: u32) {
    frame(chrome, quads, MESETA);
    draw_text(chrome, quads, &format!("{money} MST"), ROOT_TEXT[11].cell);
}

fn draw_summary(chrome: &CampChrome, quads: &mut Vec<Quad>, character: &CampCharacter) {
    draw_text(chrome, quads, &character.name, ROOT_TEXT[0].cell);
    draw_level(chrome, quads, character.level, ROOT_TEXT[1].cell);
    draw_status_pair(
        chrome,
        quads,
        "HP: ",
        character.current_hp,
        character.max_hp,
        ROOT_TEXT[2].cell,
    );
    draw_status_pair(
        chrome,
        quads,
        "TP: ",
        character.current_tp,
        character.max_tp,
        ROOT_TEXT[3].cell,
    );
}

fn use_result_message(result: CampUseResult) -> String {
    match result {
        CampUseResult::Used {
            item_name,
            character_name,
            amount,
        } if amount > 0 => format!("USED {item_name} +{amount} {character_name}"),
        CampUseResult::Used {
            item_name,
            character_name,
            ..
        } => format!("USED {item_name} {character_name}"),
        CampUseResult::NoEffect { reason, .. } => reason,
        CampUseResult::Unavailable { reason } => reason,
    }
}
