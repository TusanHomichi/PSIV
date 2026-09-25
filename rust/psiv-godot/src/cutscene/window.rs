//! The generic cutscene dialogue window: its tiles, its portrait and the
//! rectangle one `DrawPortrait` names.
//!
//! Retail reuses one nine-tile window for scene dialogue that is not a panel;
//! `window_create`/`window_destroy` own its visibility and `draw_window` emits
//! it with the portrait in the layer's frame order.

use godot::prelude::*;
use psiv_core::PresentationAsset;

use super::CutsceneLayer;

impl CutsceneLayer {
    /// Draws the generic window and its portrait, when the window is up.
    pub(super) fn draw_window(&mut self) {
        if self.generic_window_visible {
            self.draw_generic_window();
        }
        if let (Some(texture), Some(rect)) =
            (self.generic_portrait.clone(), self.generic_portrait_rect)
        {
            self.base_mut().draw_texture_rect(&texture, rect, true);
        }
    }

    pub(crate) fn window_destroy(&mut self) {
        self.generic_window_visible = false;
        self.generic_portrait_rect = None;
        let visible = self.opening_visible || !self.visible.is_empty();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn window_create(&mut self) {
        self.generic_window_visible = true;
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn load_window_tiles(&self, asset: PresentationAsset) {
        godot_print!("scene generic window tiles loaded: {asset:?}");
    }

    pub(crate) fn load_portrait(&mut self, asset: PresentationAsset) {
        if asset == PresentationAsset::ShopkeeperDialPortrait2 && self.generic_portrait.is_none() {
            godot_error!("scene shopkeeper portrait was not emitted in the presentation pack");
        }
    }

    pub(crate) fn draw_portrait(&mut self, x: u8, y: u8, width: u8, height: u8) {
        self.generic_portrait_rect = Some(Rect2::new(
            Vector2::new(f32::from(x) * 8.0, f32::from(y) * 8.0),
            Vector2::new(f32::from(width) * 8.0, f32::from(height) * 8.0),
        ));
        self.base_mut().queue_redraw();
    }

    fn draw_generic_window(&mut self) {
        let rect = self.generic_window_rect;
        let columns = (rect.size.x / 8.0).round() as i32;
        let rows = (rect.size.y / 8.0).round() as i32;
        let cell = |role: &str, x: i32, y: i32, this: &mut CutsceneLayer| {
            let Some(texture) = this.window_tiles.get(role).cloned() else {
                return;
            };
            this.base_mut().draw_texture_rect(
                &texture,
                Rect2::new(
                    rect.position + Vector2::new(x as f32 * 8.0, y as f32 * 8.0),
                    Vector2::new(8.0, 8.0),
                ),
                false,
            );
        };
        for y in 1..rows - 1 {
            for x in 1..columns - 1 {
                cell("fill", x, y, self);
            }
        }
        for x in 1..columns - 1 {
            cell("edge_top", x, 0, self);
            cell("edge_bottom", x, rows - 1, self);
        }
        for y in 1..rows - 1 {
            cell("edge_left", 0, y, self);
            cell("edge_right", columns - 1, y, self);
        }
        cell("corner_top_left", 0, 0, self);
        cell("corner_top_right", columns - 1, 0, self);
        cell("corner_bottom_left", 0, rows - 1, self);
        cell("corner_bottom_right", columns - 1, rows - 1, self);
    }
}
