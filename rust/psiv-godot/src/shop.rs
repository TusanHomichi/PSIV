//! Retail shop and inn windows.
//!
//! The rectangles here are the camera-remapped Piata decode in
//! `docs/camp/SHOP_LAYOUT_DECODED.md`. The window owns cursors and text only;
//! transaction mutations go through `psiv_runtime::Runtime`.
//!
//! [`ShopWindow`] is the node the field shell drives. The decoded tables, the
//! window art, the retail rectangles and the input/draw halves live in the
//! submodules, one concern each; this file owns the node's lifecycle, the
//! runtime snapshots it reads from and the shell that drives it.

mod catalog;
mod chrome;
mod draw;
mod input;
mod layout;

#[path = "shop/portraits.rs"]
mod portraits;

use std::path::Path;

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::DialogueSet;
use psiv_runtime::Runtime;

use catalog::{ShopCatalog, ShopCounter, ShopItem};
use chrome::ShopChrome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Closed,
    Greeting,
    InnGreeting,
    Root,
    BuyList,
    BuyConfirm,
    SellList,
    SellConfirm,
    InnConfirm,
    Message,
}

#[derive(Clone, Debug, Default)]
struct Snapshot {
    money: u32,
    party_slots: usize,
    items: Vec<OwnedItem>,
}

#[derive(Clone, Debug)]
struct OwnedItem {
    slot: usize,
    id: u8,
    name: String,
}

/// A modal shop/inn window owned by the field shell.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct ShopWindow {
    base: Base<Node2D>,
    pack_dir: String,
    chrome: Option<ShopChrome>,
    catalog: Option<ShopCatalog>,
    counter: Option<ShopCounter>,
    portrait: Option<Gd<ImageTexture>>,
    mode: Mode,
    root_selection: usize,
    item_selection: usize,
    confirm_selection: usize,
    snapshot: Snapshot,
    message: String,
    accept_blocked: bool,
    cancel_down: bool,
}

#[godot_api]
impl INode2D for ShopWindow {
    fn init(base: Base<Node2D>) -> Self {
        ShopWindow {
            base,
            pack_dir: String::new(),
            chrome: None,
            catalog: None,
            counter: None,
            portrait: None,
            mode: Mode::Closed,
            root_selection: 0,
            item_selection: 0,
            confirm_selection: 0,
            snapshot: Snapshot::default(),
            message: String::new(),
            accept_blocked: false,
            cancel_down: false,
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
        for quad in list.0 {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        if let Some((portrait, rect)) = list.1 {
            self.base_mut().draw_texture_rect(&portrait, rect, true);
        }
    }
}

impl ShopWindow {
    /// Loads the shared window art and the shop tables.
    pub(crate) fn configure(&mut self, pack_dir: &str) {
        self.pack_dir = pack_dir.to_owned();
        match DialogueSet::load(Path::new(pack_dir)) {
            Ok(set) => {
                self.chrome = ShopChrome::build(pack_dir, &set);
                if self.chrome.is_none() {
                    godot_error!("shop: window/font art failed to load from {pack_dir}");
                }
            }
            Err(error) => godot_error!("shop: dialogue art failed to load: {error}"),
        }
        match ShopCatalog::load(pack_dir) {
            Ok(catalog) => self.catalog = Some(catalog),
            Err(error) => godot_error!("shop: {error}"),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.mode != Mode::Closed
    }

    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": format!("{:?}", self.mode),
            "root": self.root_selection,
            "item": self.item_selection,
            "confirm": self.confirm_selection,
            "stock": self.buy_items().iter().map(|item| serde_json::json!({
                "id": item.item_id, "name": item.display_name, "price": item.buy_price,
            })).collect::<Vec<_>>(),
            "message": self.message,
            "money": self.snapshot.money,
        })
    }

    pub(crate) fn counter_at(&self, map_id: u16, x: u16, y: u16) -> Option<ShopCounter> {
        self.catalog.as_ref()?.counter_at(map_id, x, y)
    }

    pub(crate) fn open_index(&mut self, index: usize, runtime: &Runtime) -> bool {
        let Some(counter) = self.catalog.as_ref().and_then(|c| c.counter_index(index)) else {
            godot_error!("shop debug selector {index} did not name a live counter");
            return false;
        };
        self.open(counter, runtime)
    }

    pub(crate) fn open(&mut self, counter: ShopCounter, runtime: &Runtime) -> bool {
        self.root_selection = 0;
        self.item_selection = 0;
        self.confirm_selection = 0;
        self.message.clear();
        self.snapshot = self.snapshot(runtime);
        self.portrait = self.load_portrait(&counter.portrait);
        self.mode = if counter.is_inn() {
            Mode::InnGreeting
        } else {
            Mode::Greeting
        };
        self.accept_blocked = true;
        self.counter = Some(counter);
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
        true
    }

    pub(crate) fn close(&mut self) {
        self.mode = Mode::Closed;
        self.counter = None;
        self.portrait = None;
        self.message.clear();
        self.base_mut().set_visible(false);
        self.base_mut().queue_redraw();
    }

    fn snapshot(&self, runtime: &Runtime) -> Snapshot {
        let inventory = runtime.game().inventory();
        let names = self.catalog.as_ref();
        Snapshot {
            money: runtime.game().money(),
            party_slots: runtime.game().party_len(),
            items: inventory
                .slots()
                .iter()
                .enumerate()
                .filter(|&(_, &id)| id != 0)
                .map(|(slot, &id)| OwnedItem {
                    slot,
                    id,
                    name: names.map_or_else(|| format!("ITEM {id}"), |c| c.item_name(id)),
                })
                .collect(),
        }
    }

    fn buy_items(&self) -> Vec<ShopItem> {
        let Some(counter) = self.counter.as_ref() else {
            return Vec::new();
        };
        let Some(index) = counter.shop_inventory_index else {
            return Vec::new();
        };
        self.catalog
            .as_ref()
            .and_then(|catalog| catalog.inventory(index))
            .map_or_else(Vec::new, |inventory| inventory.items.clone())
    }

    fn load_portrait(&self, raw: &str) -> Option<Gd<ImageTexture>> {
        let relative = self
            .catalog
            .as_ref()?
            .portraits
            .get(raw)
            .map(String::as_str)
            .or_else(|| (raw == "0x29FC66").then_some("dialogue/portraits/12_Baker.png"))?;
        let path = Path::new(&self.pack_dir).join(relative);
        Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
            .and_then(|image| ImageTexture::create_from_image(&image))
    }
}

impl crate::Field {
    pub(crate) fn drive_shop_if_active(&mut self) -> bool {
        if !self.shop.as_ref().is_some_and(|shop| shop.bind().is_open()) {
            return false;
        }
        if let (Some(runtime), Some(shop)) = (self.runtime.as_mut(), self.shop.as_mut()) {
            shop.bind_mut().handle_input(runtime);
        }
        if !self.shop.as_ref().is_some_and(|shop| shop.bind().is_open()) {
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.set_field_suspended(false);
            }
            self.sync_visuals(false);
            return true;
        }
        let events = self
            .runtime
            .as_mut()
            .map(|runtime| {
                runtime.set_field_suspended(true);
                runtime.tick(psiv_core::Input::Neutral)
            })
            .unwrap_or_default();
        self.process_events(events);
        self.place_shop_window();
        self.sync_visuals(false);
        true
    }

    pub(crate) fn place_shop_window(&mut self) {
        let Some(camera) = self.camera.as_ref() else {
            return;
        };
        let center = camera.get_position();
        if let Some(shop) = self.shop.as_mut() {
            shop.set_position(center - Vector2::new(160.0, 112.0));
        }
    }
}
