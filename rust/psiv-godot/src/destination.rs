//! The ship's destination screen.
//!
//! [`DestinationScreen`] draws a [`DestinationView`] and nothing else: the
//! planetary map, the two windows, the cursor boxes and the typed message.
//! Which rows exist, where the cursor is, what has been typed and when the
//! windows close are the session's (`psiv-runtime/src/session/destination.rs`);
//! the map's pixels are the pack's (`ship_menu/`, built from the ROM by
//! `psiv_tools/ship_menu_pack.py`), turned into colours here with the palette
//! state the view names.
//!
//! The window art, the font and the cursor patterns are the shared window set
//! the camp draws with (`camp/chrome.rs`): `loc_63BC4` writes the same window
//! words (`$C000 + $680 + tile`, `ps4.asm:133556-133575`).

use godot::classes::image::Format;
use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::{DialogueSet, ShipMenu, ShipPalette};
use psiv_runtime::{DestinationPhase, DestinationView};

use crate::Field;
use crate::camp::chrome::{CampChrome, Quad};

/// The window the prompt (and then the typed message) sits in: `WinGroup_Event`
/// record 5 (`ps4.asm:141166`).
const PROMPT_WINDOW: u8 = 5;
/// `WinTiles_CursorBox`: byte `$67` plus the `$680` window base
/// (`ps4.asm:340368`).
const CURSOR_BOX_PATTERN: u16 = 0x6E7;
/// The pattern `loc_69A32` stamps over a chosen row's box (`$C6E8`,
/// `ps4.asm:141817`), and the red cursor object's own art (`$6E8`).
const SELECTED_PATTERN: u16 = 0x6E8;
/// The prompt window's text origin, inside its frame (`LoadWindowTiles` at the
/// window's x and y plus one, `ps4.asm:133548-133556`).
const MESSAGE_LINE_ROWS: i32 = 2;

/// The destination screen node. `Field` owns it and hands it the session's
/// view each frame.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct DestinationScreen {
    base: Base<Node2D>,
    chrome: Option<CampChrome>,
    menu: Option<ShipMenu>,
    /// The session's menu, as of the last frame; `None` while closed.
    view: Option<DestinationView>,
    picture: Option<Gd<ImageTexture>>,
    picture_key: Option<(usize, ShipPalette)>,
}

#[godot_api]
impl INode2D for DestinationScreen {
    fn init(base: Base<Node2D>) -> Self {
        DestinationScreen {
            base,
            chrome: None,
            menu: None,
            view: None,
            picture: None,
            picture_key: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(1000);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        if let Some(picture) = self.picture.clone() {
            self.base_mut().draw_texture(&picture, Vector2::ZERO);
        }
        for quad in self.quads() {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
    }
}

impl DestinationScreen {
    /// Loads the shared window art and keeps the pack's screen.
    pub(crate) fn configure(&mut self, pack_dir: &str, set: &DialogueSet, menu: Option<ShipMenu>) {
        self.chrome = CampChrome::build(pack_dir, set);
        if self.chrome.is_none() {
            godot_error!("destination menu: window art failed to load from {pack_dir}");
        }
        self.menu = menu;
    }

    /// Shows the session's menu, or hides the node when there is none.
    pub(crate) fn sync(&mut self, view: Option<&DestinationView>) {
        match view {
            Some(view) => {
                self.view = Some(view.clone());
                self.rebuild_picture();
                self.base_mut().set_visible(true);
                self.base_mut().queue_redraw();
            }
            None => {
                if self.view.take().is_some() {
                    self.picture = None;
                    self.picture_key = None;
                    self.base_mut().set_visible(false);
                    self.base_mut().queue_redraw();
                }
            }
        }
    }

    /// The planetary map for the view's cover set and palette state, built
    /// only when either changes.
    fn rebuild_picture(&mut self) {
        let (Some(view), Some(menu)) = (self.view.as_ref(), self.menu.as_ref()) else {
            return;
        };
        let key = (view.covers(), view.palette());
        if self.picture_key == Some(key) {
            return;
        }
        let Some(pixels) = menu.render(key.0, key.1) else {
            godot_error!("destination menu: no background for cover set {}", key.0);
            return;
        };
        let bytes = PackedByteArray::from(pixels);
        let image = Image::create_from_data(
            psiv_data::SCREEN_WIDTH as i32,
            psiv_data::SCREEN_HEIGHT as i32,
            false,
            Format::RGBA8,
            &bytes,
        );
        self.picture = image.and_then(|image| ImageTexture::create_from_image(&image));
        self.picture_key = Some(key);
    }

    /// The windows, text and cursor boxes, in the order the cartridge wrote
    /// them: the prompt window, the list window, the boxes, the names.
    fn quads(&self) -> Vec<Quad> {
        let (Some(chrome), Some(menu), Some(view)) =
            (self.chrome.as_ref(), self.menu.as_ref(), self.view.as_ref())
        else {
            return Vec::new();
        };
        let mut quads = Vec::new();
        if let Some(window) = menu.window(PROMPT_WINDOW) {
            let (x, y) = (i32::from(window.x), i32::from(window.y));
            quads.extend(chrome.frame_cells(
                x,
                y,
                i32::from(window.width),
                i32::from(window.height),
            ));
            if view.phase == DestinationPhase::Choosing {
                quads.extend(chrome.text(menu.prompt(), (x + 1, y + 1)));
            } else {
                for (line, text) in view.typed.split('\n').enumerate() {
                    quads.extend(
                        chrome.text(text, (x + 1, y + 1 + line as i32 * MESSAGE_LINE_ROWS)),
                    );
                }
            }
        }
        // `Window_Create` with `d0 = rows + 5`: the list window's id.
        let list_id = view.rows.len() as u8 + PROMPT_WINDOW;
        if !view.rows.is_empty()
            && let Some(window) = menu.window(list_id)
        {
            let (x, y) = (i32::from(window.x), i32::from(window.y));
            quads.extend(chrome.frame_cells(
                x,
                y,
                i32::from(window.width),
                i32::from(window.height),
            ));
            for (row, &world) in view.rows.iter().enumerate() {
                let line = y + 1 + row as i32 * 2;
                // The box is the hollow pattern; the red cursor object (while
                // it is in its visible half) and the stamp a confirm leaves
                // draw the solid one over it.
                let lit = row == view.cursor
                    && match view.phase {
                        DestinationPhase::Choosing => view.cursor_visible,
                        DestinationPhase::Confirmed | DestinationPhase::Closing => true,
                    };
                let pattern = if lit {
                    SELECTED_PATTERN
                } else {
                    CURSOR_BOX_PATTERN
                };
                quads.extend(chrome.window_word(pattern, (x + 1, line)));
                if let Some(name) = menu.name(world) {
                    quads.extend(chrome.text(name, (x + 3, line)));
                }
            }
        }
        quads
    }
}

impl Field {
    /// Hands the destination node what the session shows, and places it over
    /// the camera's frame.
    pub(crate) fn present_destination(&mut self) {
        let view = self
            .session
            .as_ref()
            .and_then(|session| session.destination_view())
            .cloned();
        if let Some(screen) = self.destination.as_mut() {
            screen.bind_mut().sync(view.as_ref());
        }
        if view.is_some()
            && let Some(camera) = self.camera.as_ref()
        {
            let center = camera.get_position();
            if let Some(screen) = self.destination.as_mut() {
                screen.set_position(center - Vector2::new(160.0, 112.0));
            }
        }
    }
}
