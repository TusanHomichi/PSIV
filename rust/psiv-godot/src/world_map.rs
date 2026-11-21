//! Overworld planet map screen (`FieldRoutine_WorldMap`, #122).
//!
//! Draws the decoded Motavia (256x256) or Dezolis (256x128) overview image
//! wrapped smoothly at `(scroll_x, scroll_y)`, a blinking party marker at
//! `(marker_x, marker_y)`, and a small top-left planet label box.

use std::collections::HashMap;
use std::path::PathBuf;

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::{DialogueSet, SCREEN_HEIGHT, SCREEN_WIDTH};
use psiv_runtime::WorldMapView;

use crate::Field;
use crate::camp::chrome::{CampChrome, Quad};
use crate::window_layer::WindowLayer;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct WorldMapScreen {
    base: Base<Node2D>,
    pack_dir: PathBuf,
    chrome: Option<CampChrome>,
    windows: Option<Gd<WindowLayer>>,
    textures: HashMap<String, Gd<ImageTexture>>,
    view: Option<WorldMapView>,
}

#[godot_api]
impl INode2D for WorldMapScreen {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            pack_dir: PathBuf::new(),
            chrome: None,
            windows: None,
            textures: HashMap::new(),
            view: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(1000);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        let Some(view) = self.view.clone() else {
            return;
        };
        let screen = Rect2::new(
            Vector2::ZERO,
            Vector2::new(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32),
        );
        self.base_mut()
            .draw_rect(screen, Color::from_rgb(0.0, 0.0, 0.0));

        let screen_w = SCREEN_WIDTH as i32;
        let screen_h = SCREEN_HEIGHT as i32;
        let texture = self.texture_for(&view.png);
        if let Some(tex) = texture {
            let w = i32::from(view.width).max(1);
            let h = i32::from(view.height).max(1);
            let ox = -i32::from(view.scroll_x).rem_euclid(w);
            let mut tx = ox;
            while tx < screen_w {
                if view.scroll_y_enabled {
                    let oy = -i32::from(view.scroll_y).rem_euclid(h);
                    let mut ty = oy;
                    while ty < screen_h {
                        self.base_mut()
                            .draw_texture(&tex, Vector2::new(tx as f32, ty as f32));
                        ty += h;
                    }
                } else {
                    let ty = -i32::from(view.scroll_y);
                    self.base_mut()
                        .draw_texture(&tex, Vector2::new(tx as f32, ty as f32));
                }
                tx += w;
            }
        }

        if view.blink_on {
            let w = i32::from(view.width).max(1);
            let h = i32::from(view.height).max(1);
            let mx = (i32::from(view.marker_x) - i32::from(view.scroll_x)).rem_euclid(w);
            let my = if view.scroll_y_enabled {
                (i32::from(view.marker_y) - i32::from(view.scroll_y)).rem_euclid(h)
            } else {
                i32::from(view.marker_y) - i32::from(view.scroll_y)
            };
            let mut sx = mx;
            while sx < screen_w {
                if view.scroll_y_enabled {
                    let mut sy = my;
                    while sy < screen_h {
                        self.draw_marker(sx, sy);
                        sy += h;
                    }
                } else if (0..screen_h).contains(&my) {
                    self.draw_marker(sx, my);
                }
                sx += w;
            }
        }
    }
}

impl WorldMapScreen {
    pub(crate) fn configure(&mut self, pack_dir: &str, set: &DialogueSet) {
        self.pack_dir = PathBuf::from(pack_dir);
        self.chrome = CampChrome::build(pack_dir, set);
    }

    pub(crate) fn sync(&mut self, view: Option<WorldMapView>) {
        let changed = self.view != view;
        self.view = view;
        let visible = self.view.is_some();
        self.base_mut().set_visible(visible);
        if changed {
            self.sync_label();
            self.base_mut().queue_redraw();
        }
    }

    fn draw_marker(&mut self, x: i32, y: i32) {
        self.base_mut().draw_rect(
            Rect2::new(
                Vector2::new((x - 1) as f32, (y - 1) as f32),
                Vector2::new(4.0, 4.0),
            ),
            Color::from_rgb(1.0, 1.0, 1.0),
        );
        self.base_mut().draw_rect(
            Rect2::new(Vector2::new(x as f32, y as f32), Vector2::new(2.0, 2.0)),
            Color::from_rgb(0.95, 0.2, 0.2),
        );
    }

    fn texture_for(&mut self, rel: &str) -> Option<Gd<ImageTexture>> {
        if let Some(cached) = self.textures.get(rel) {
            return Some(cached.clone());
        }
        let path = self.pack_dir.join(rel);
        let bytes = std::fs::read(&path).ok()?;
        let mut image = Image::new_gd();
        if image.load_png_from_buffer(&PackedByteArray::from(bytes.as_slice()))
            != godot::global::Error::OK
        {
            return None;
        }
        let texture = ImageTexture::create_from_image(&image)?;
        self.textures.insert(rel.to_owned(), texture.clone());
        Some(texture)
    }

    fn sync_label(&mut self) {
        let Some(chrome) = self.chrome.as_ref() else {
            return;
        };
        let quads = match self.view.as_ref() {
            Some(view) => {
                let cols = (view.name.chars().count() as i32).max(7) + 2;
                let mut out: Vec<Quad> = chrome.frame_cells(1, 1, cols, 3);
                out.extend(chrome.text(&view.name, (2, 2)));
                out
            }
            None => Vec::new(),
        };
        let mut layer = self.ensure_windows();
        layer.bind_mut().show(quads);
    }

    fn ensure_windows(&mut self) -> Gd<WindowLayer> {
        if let Some(layer) = self.windows.as_ref() {
            return layer.clone();
        }
        let mut layer = WindowLayer::new_alloc();
        layer.set_z_index(10);
        self.base_mut().add_child(&layer);
        self.windows = Some(layer.clone());
        layer
    }
}

impl Field {
    pub(crate) fn present_world_map(&mut self) {
        let view = self
            .session
            .as_ref()
            .and_then(|session| session.world_map_view());
        let visible = view.is_some();
        if let Some(screen) = self.world_map.as_mut() {
            screen.bind_mut().sync(view);
        }
        if visible && let Some(camera) = self.camera.as_ref() {
            let center = camera.get_position();
            if let Some(screen) = self.world_map.as_mut() {
                screen.set_position(center - Vector2::new(160.0, 112.0));
            }
        }
    }
}
