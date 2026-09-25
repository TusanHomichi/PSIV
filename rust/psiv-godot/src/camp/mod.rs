//! The field camp menu: retail root, ITEM, STATE, STATUS, and equipment view.
//!
//! [`CampMenu`] owns only presentation state and input routing. Persistent
//! values and item effects stay in `psiv-runtime`; opening the node is also
//! the place where `Field` applies the cartridge's field-suspension seam.

mod abilities;
mod chrome;
mod draw;
mod equipment;
mod input;
mod layout;
mod loot;
mod order;
mod status;
mod travel;

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;
use std::collections::BTreeMap;

use psiv_runtime::{CampAbility, CampAbilityKind, CampState, Runtime};

use crate::Field;

use self::chrome::{CampChrome, Quad};

/// Tier-1 browse geometry for inventory and target pages. The tape only
/// decoded the empty ITEM branch; these child surfaces keep live inventory
/// usable while leaving that documented gap explicit.
const ITEM_LIST: layout::CellRect = layout::CellRect::new(14, 2, 24, 20);
const ITEM_TARGET: layout::CellRect = layout::CellRect::new(18, 6, 18, 10);

const ROOT_OPTIONS: [&str; 7] = ["ITEM", "TECH", "SKILL", "EQUIP", "STATE", "MUMBL", "MACRO"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Closed,
    LootMessage,
    LootItems,
    LootDiscardConfirm,
    LootReturnConfirm,
    LootBlocked,
    Root,
    ItemEmpty,
    ItemList,
    ItemTarget,
    ItemResult,
    AbilityCharacters,
    AbilityList,
    AbilityTarget,
    AbilityResult,
    TravelTowns,
    TravelReady,
    EquipCharacters,
    EquipStats,
    EquipItems,
    EquipHands,
    EquipResult,
    State,
    Order,
    OrderDone,
    OrderAlone,
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
    portraits: BTreeMap<u8, Gd<ImageTexture>>,
    mode: Mode,
    root_selection: usize,
    state_selection: usize,
    save_selection: usize,
    item_selection: usize,
    target_selection: usize,
    ability_kind: CampAbilityKind,
    ability_character_selection: usize,
    ability_selection: usize,
    ability_options: Vec<CampAbility>,
    travel_towns: Vec<psiv_data::TownDestination>,
    travel_selection: usize,
    travel_item_slot: Option<usize>,
    runtime_events: Vec<psiv_runtime::RuntimeEvent>,
    status_selection: usize,
    status_from_state: bool,
    order_draft: order::OrderDraft,
    order_wait: u16,
    equipment_character_selection: usize,
    equipment_slot_selection: usize,
    equipment_item_selection: usize,
    equipment_hand_selection: usize,
    equipment_options: Vec<psiv_runtime::CampItem>,
    snapshot: CampState,
    message: String,
    sound_request: Option<u8>,
    x_down: bool,
}

#[godot_api]
impl INode2D for CampMenu {
    fn init(base: Base<Node2D>) -> Self {
        CampMenu {
            base,
            chrome: None,
            portraits: BTreeMap::new(),
            mode: Mode::Closed,
            root_selection: 0,
            state_selection: 0,
            save_selection: 0,
            item_selection: 0,
            target_selection: 0,
            ability_kind: CampAbilityKind::Technique,
            ability_character_selection: 0,
            ability_selection: 0,
            ability_options: Vec::new(),
            travel_towns: Vec::new(),
            travel_selection: 0,
            travel_item_slot: None,
            runtime_events: Vec::new(),
            status_selection: 0,
            status_from_state: false,
            order_draft: order::OrderDraft::default(),
            order_wait: 0,
            equipment_character_selection: 0,
            equipment_slot_selection: 0,
            equipment_item_selection: 0,
            equipment_hand_selection: 0,
            equipment_options: Vec::new(),
            snapshot: CampState::default(),
            message: String::new(),
            sound_request: None,
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
            self.base_mut().draw_texture_rect(&portrait, rect, false);
        }
    }
}

impl CampMenu {
    /// Loads shared window/font assets and the original STATUS portrait maps.
    pub(crate) fn configure(&mut self, pack_dir: &str, set: psiv_data::DialogueSet) {
        self.chrome = CampChrome::build(pack_dir, &set);
        if self.chrome.is_none() {
            godot_error!("camp menu chrome failed to load");
        }
        self.portraits.clear();
        if let Ok(bytes) = std::fs::read(format!("{pack_dir}/battle/characters.json"))
            && let Ok(characters) = serde_json::from_slice::<psiv_data::CharactersFile>(&bytes)
        {
            for character in characters.characters {
                let Some(portrait) = character.status_portrait else {
                    continue;
                };
                let path = format!("{pack_dir}/{}", portrait.png);
                if let Some(texture) = Image::load_from_file(&GString::from(path.as_str()))
                    .and_then(|image| ImageTexture::create_from_image(&image))
                {
                    self.portraits.insert(character.character_id, texture);
                } else {
                    godot_warn!("camp status portrait failed to load: {path}");
                }
            }
        } else {
            godot_warn!("camp STATUS character data failed to load");
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.mode != Mode::Closed
    }

    pub(crate) fn take_sound_request(&mut self) -> Option<u8> {
        self.sound_request.take()
    }

    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": format!("{:?}", self.mode), "root": self.root_selection,
            "state": self.state_selection, "save": self.save_selection,
            "item": self.item_selection, "target": self.target_selection,
            "caster": self.ability_character_selection, "ability": self.ability_selection,
            "town": self.travel_selection,
            "status_selection": self.status_selection,
            "order_cursor": self.order_draft.cursor,
            "order_remaining": self.order_draft.remaining,
            "order_chosen": self.order_draft.chosen,
            "equipment_character": self.equipment_character_selection,
            "equipment_slot": self.equipment_slot_selection,
            "equipment_item": self.equipment_item_selection,
            "equipment_hand": self.equipment_hand_selection,
            "equipment_options": self.equipment_options.iter().map(|item| serde_json::json!({"id": item.id, "slot": item.slot, "name": item.name})).collect::<Vec<_>>(),
            "towns": self.travel_towns.iter().map(|town| serde_json::json!({"index": town.index, "name": town.name})).collect::<Vec<_>>(),
            "abilities": self.ability_options.iter().map(|a| serde_json::json!({"id": a.id, "name": a.name, "cost": a.cost, "remaining": a.remaining})).collect::<Vec<_>>(),
            "party": self.snapshot.party.iter().map(|c| serde_json::json!({"id": c.id, "name": c.name, "profession": c.profession, "age": c.age, "hp": c.current_hp, "max_hp": c.max_hp, "tp": c.current_tp, "status": c.status, "equipment": c.equipment, "attack": c.attack_power, "defense": c.defense_power})).collect::<Vec<_>>(),
            "message": self.message,
        })
    }

    pub(crate) fn open(&mut self, runtime: &Runtime) {
        self.mode = Mode::Root;
        self.root_selection = 0;
        self.state_selection = 0;
        self.save_selection = 0;
        self.item_selection = 0;
        self.target_selection = 0;
        self.ability_character_selection = 0;
        self.ability_selection = 0;
        self.ability_options.clear();
        self.travel_towns.clear();
        self.runtime_events.clear();
        self.status_selection = 0;
        self.status_from_state = false;
        self.equipment_character_selection = 0;
        self.equipment_slot_selection = 0;
        self.equipment_item_selection = 0;
        self.equipment_options = runtime.camp_equipment(0);
        self.message.clear();
        self.sound_request = None;
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
        if let Some(character) = self.snapshot.party.get(self.ability_character_selection) {
            self.ability_options = runtime.camp_abilities(character.party_slot, self.ability_kind);
            self.ability_selection = self
                .ability_selection
                .min(self.ability_options.len().saturating_sub(1));
        }
        self.equipment_options = runtime.camp_equipment(self.equipment_character_selection);
        self.equipment_item_selection = self
            .equipment_item_selection
            .min(self.equipment_options.len().saturating_sub(1));
        self.base_mut().queue_redraw();
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
        let loot_waiting = self
            .runtime
            .as_ref()
            .is_some_and(|rt| rt.loot_state().is_some());
        if !is_open && loot_waiting {
            if let (Some(menu), Some(runtime)) = (self.camp_menu.as_mut(), self.runtime.as_ref()) {
                menu.bind_mut().open_loot(runtime);
            }
        } else if !is_open {
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
        }
        let menu_events = self.camp_menu.as_mut().map_or_else(Vec::new, |menu| {
            std::mem::take(&mut menu.bind_mut().runtime_events)
        });
        self.process_events(menu_events);
        if let Some(sound) = self
            .camp_menu
            .as_mut()
            .and_then(|menu| menu.bind_mut().take_sound_request())
        {
            godot_print!("camp SFX dispatch: {sound:#04x}");
            self.play_sound(sound);
        }
        if self
            .camp_menu
            .as_ref()
            .is_some_and(|menu| !menu.bind().is_open())
        {
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.set_field_suspended(false);
            }
            self.accept_blocked = true;
            self.sync_visuals(false);
            return true;
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
