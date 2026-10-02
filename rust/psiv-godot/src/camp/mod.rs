//! The field camp menu: retail root, ITEM, STATE, STATUS, and equipment view.
//!
//! [`CampMenu`] draws a [`CampView`] and nothing else. Which page is up, where
//! the cursors are, what a command did and how a chest resolves are the
//! runtime session's (`psiv-runtime/src/session/camp`); this node keeps the
//! window art and the STATUS portraits, and the shell hands it the session's
//! view each frame.

mod abilities;
mod chrome;
mod draw;
mod equipment;
mod layout;
mod loot;
mod order;
mod status;

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;
use std::collections::BTreeMap;

use psiv_runtime::{CampView, Frame, FrameMode};

use crate::Field;

use self::chrome::{CampChrome, Quad};

/// Tier-1 browse geometry for inventory and target pages. The tape only
/// decoded the empty ITEM branch; these child surfaces keep live inventory
/// usable while leaving that documented gap explicit.
const ITEM_LIST: layout::CellRect = layout::CellRect::new(14, 2, 24, 20);
const ITEM_TARGET: layout::CellRect = layout::CellRect::new(18, 6, 18, 10);

struct DrawList {
    quads: Vec<Quad>,
    portrait: Option<(Gd<ImageTexture>, Rect2)>,
}

/// A visible camp menu node. `Field` owns the node and gives it the session's
/// view; this type never owns game rules or mutable game state.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct CampMenu {
    base: Base<Node2D>,
    chrome: Option<CampChrome>,
    portraits: BTreeMap<u8, Gd<ImageTexture>>,
    /// The session's menu, as of the last frame; `None` while closed.
    view: Option<CampView>,
}

#[godot_api]
impl INode2D for CampMenu {
    fn init(base: Base<Node2D>) -> Self {
        CampMenu {
            base,
            chrome: None,
            portraits: BTreeMap::new(),
            view: None,
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
        self.view.is_some()
    }

    /// Whether the window is a chest's rather than the camp's.
    pub(crate) fn is_loot(&self) -> bool {
        self.view.as_ref().is_some_and(CampView::is_loot)
    }

    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        let Some(view) = self.view.as_ref() else {
            return serde_json::Value::Null;
        };
        serde_json::json!({
            "mode": format!("{:?}", view.page), "root": view.root_selection,
            "state": view.state_selection, "save": view.save_selection,
            "item": view.item_selection, "target": view.target_selection,
            "caster": view.ability_character_selection, "ability": view.ability_selection,
            "town": view.travel_selection,
            "status_selection": view.status_selection,
            "order_cursor": view.order_draft.cursor,
            "order_remaining": view.order_draft.remaining,
            "order_chosen": view.order_draft.chosen,
            "equipment_character": view.equipment_character_selection,
            "equipment_slot": view.equipment_slot_selection,
            "equipment_item": view.equipment_item_selection,
            "equipment_hand": view.equipment_hand_selection,
            "equipment_options": view.equipment_options.iter().map(|item| serde_json::json!({"id": item.id, "slot": item.slot, "name": item.name})).collect::<Vec<_>>(),
            "towns": view.travel_towns.iter().map(|town| serde_json::json!({"index": town.index, "name": town.name})).collect::<Vec<_>>(),
            "abilities": view.ability_options.iter().map(|a| serde_json::json!({"id": a.id, "name": a.name, "cost": a.cost, "remaining": a.remaining})).collect::<Vec<_>>(),
            "party": view.snapshot.party.iter().map(|c| serde_json::json!({"id": c.id, "name": c.name, "profession": c.profession, "age": c.age, "hp": c.current_hp, "max_hp": c.max_hp, "tp": c.current_tp, "status": c.status, "equipment": c.equipment, "attack": c.attack_power, "defense": c.defense_power})).collect::<Vec<_>>(),
            "message": view.message,
        })
    }

    /// Shows the session's menu, or hides the node when there is none.
    pub(crate) fn sync(&mut self, view: Option<&CampView>) {
        match view {
            Some(view) => {
                self.view = Some(view.clone());
                self.base_mut().set_visible(true);
                self.base_mut().queue_redraw();
            }
            None => {
                if self.view.take().is_some() {
                    self.base_mut().set_visible(false);
                    self.base_mut().queue_redraw();
                }
            }
        }
    }
}

impl Field {
    /// Presents a frame the shop or the camp owned: a camp command's events and
    /// sound first, then the tick's own events, then the window the session
    /// shows now. Nothing here decides anything; the session already did.
    pub(crate) fn present_menu_frame(&mut self, frame: Frame) {
        debug_assert!(frame.mode != FrameMode::Field);
        self.process_events(frame.menu_events);
        if let Some(sound) = frame.sound {
            godot_print!("camp SFX dispatch: {sound:#04x}");
            self.play_sound(sound);
        }
        self.present_routed(&frame.routed);
        self.process_events(frame.events);
        // The camp's SAVE wrote its slot inside the session, which owns the
        // run's save directory; a failed write is the frame's one report, so
        // the shell logs it exactly as it logged its own attempt before.
        if let Some(failure) = frame.camp_save_error {
            godot_error!(
                "camp: save slot {} failed: {}",
                failure.slot + 1,
                failure.error
            );
        }
        self.present_shop();
        self.present_camp();
        self.sync_visuals(false);
    }

    /// Hands the camp node what the session shows, and places it.
    pub(crate) fn present_camp(&mut self) {
        let view = self
            .session
            .as_ref()
            .and_then(|session| session.camp_view())
            .cloned();
        if let Some(menu) = self.camp_menu.as_mut() {
            menu.bind_mut().sync(view.as_ref());
        }
        if view.is_some() {
            self.place_camp_menu();
        }
    }

    /// Debug hook entry point used by `PSIV_DEBUG_CAMP=1`.
    pub(crate) fn open_camp_menu(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.open_camp_menu();
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
