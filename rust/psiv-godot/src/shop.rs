//! Retail shop and inn windows.
//!
//! The rectangles here are the camera-remapped Piata decode in
//! `docs/camp/SHOP_LAYOUT_DECODED.md`. The window draws a [`ShopView`]: the
//! counter, the stock, the cursors and the result line all come from the
//! runtime's session, which owns the pages, the rules and every transaction.
//!
//! [`ShopWindow`] is the node the field shell hands that view to. The window
//! art and the retail rectangles live in the submodules, one concern each;
//! this file owns the node's lifecycle and the shell's per-frame hook.

mod chrome;
mod draw;
mod layout;

use std::path::Path;

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::DialogueSet;
use psiv_runtime::{Runtime, ShopView};

use chrome::ShopChrome;

/// A modal shop/inn window owned by the field shell.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct ShopWindow {
    base: Base<Node2D>,
    pack_dir: String,
    chrome: Option<ShopChrome>,
    /// The session's window, as of the last frame; `None` while closed.
    view: Option<ShopView>,
    portrait: Option<Gd<ImageTexture>>,
    /// The counter the portrait was loaded for.
    portrait_for: Option<usize>,
    /// A counter the debug selector asked for, handed to the session by the
    /// field's next frame.
    debug_open: Option<usize>,
}

#[godot_api]
impl INode2D for ShopWindow {
    fn init(base: Base<Node2D>) -> Self {
        ShopWindow {
            base,
            pack_dir: String::new(),
            chrome: None,
            view: None,
            portrait: None,
            portrait_for: None,
            debug_open: None,
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
    /// Loads the shared window art.
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
    }

    pub(crate) fn is_open(&self) -> bool {
        self.view.is_some()
    }

    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        let Some(view) = self.view.as_ref() else {
            return serde_json::Value::Null;
        };
        serde_json::json!({
            "mode": format!("{:?}", view.page),
            "root": view.root_selection,
            "item": view.item_selection,
            "confirm": view.confirm_selection,
            "stock": view.stock.iter().map(|item| serde_json::json!({
                "id": item.item_id, "name": item.name, "price": item.price,
            })).collect::<Vec<_>>(),
            "message": view.message,
            "money": view.money,
        })
    }

    /// The `PSIV_DEBUG_SHOP` selector: asks for counter row `index` to open on
    /// the field's next frame. Whether the row exists is answered now.
    pub(crate) fn open_index(&mut self, index: usize, runtime: &Runtime) -> bool {
        if runtime
            .data()
            .shops()
            .and_then(|shops| shops.counter_index(index))
            .is_none()
        {
            godot_error!("shop debug selector {index} did not name a live counter");
            return false;
        }
        self.debug_open = Some(index);
        true
    }

    /// The counter the debug selector asked for, once.
    pub(crate) fn take_debug_open(&mut self) -> Option<usize> {
        self.debug_open.take()
    }

    /// Shows the session's window, or hides the node when there is none.
    pub(crate) fn sync(&mut self, view: Option<&ShopView>) {
        match view {
            Some(view) => {
                if self.portrait_for != Some(view.counter.id) {
                    self.portrait = view
                        .counter
                        .portrait_png
                        .as_deref()
                        .and_then(|png| self.load_portrait(png));
                    self.portrait_for = Some(view.counter.id);
                }
                self.view = Some(view.clone());
                self.base_mut().set_visible(true);
                self.base_mut().queue_redraw();
            }
            None => {
                if self.view.take().is_some() {
                    self.portrait = None;
                    self.portrait_for = None;
                    self.base_mut().set_visible(false);
                    self.base_mut().queue_redraw();
                }
            }
        }
    }

    fn load_portrait(&self, relative: &str) -> Option<Gd<ImageTexture>> {
        let path = Path::new(&self.pack_dir).join(relative);
        Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
            .and_then(|image| ImageTexture::create_from_image(&image))
    }
}

impl crate::Field {
    /// Hands the shop window what the session shows, and places it. Run after
    /// the session's frame, whichever mode owned it.
    pub(crate) fn present_shop(&mut self) {
        let view = self
            .session
            .as_ref()
            .and_then(|session| session.shop_view())
            .cloned();
        if let Some(shop) = self.shop.as_mut() {
            shop.bind_mut().sync(view.as_ref());
        }
        if view.is_some() {
            self.place_shop_window();
        }
    }

    /// Opens a counter the debug selector asked for, in the session.
    pub(crate) fn open_debug_shop(&mut self) {
        let Some(index) = self
            .shop
            .as_mut()
            .and_then(|shop| shop.bind_mut().take_debug_open())
        else {
            return;
        };
        if let Some(session) = self.session.as_mut() {
            session.open_shop_counter(index);
        }
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
