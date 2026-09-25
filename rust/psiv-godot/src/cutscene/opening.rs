//! The opening image and the narration text drawn over it.
//!
//! `LoadTitleImage` puts the stage on the black frame and hands the crawl to
//! [`OpeningTextLayer`](super::text_layer::OpeningTextLayer), a child node with
//! its own CRAM text ramp. This module is the only place that raises and clears
//! the opening; the layer's `draw` calls it first, in frame order.

use godot::prelude::*;

use super::{CutsceneLayer, SCREEN};

impl CutsceneLayer {
    /// Draws the black stage and the narration image while the opening runs.
    pub(super) fn draw_opening(&mut self) {
        if self.opening_visible {
            self.base_mut().draw_rect(
                Rect2::new(Vector2::ZERO, SCREEN),
                Color::from_rgb(0.0, 0.0, 0.0),
            );
            if let Some(texture) = self.opening_background.clone() {
                let screen_y = self.opening_screen_y;
                self.base_mut().draw_texture_rect(
                    &texture,
                    Rect2::new(Vector2::new(0.0, screen_y), Vector2::new(320.0, 128.0)),
                    true,
                );
            }
        }
    }

    pub(crate) fn begin_opening(&mut self) {
        self.opening_visible = true;
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().clear();
        }
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn end_opening(&mut self) {
        self.opening_visible = false;
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().clear();
        }
        let visible = self.generic_window_visible || !self.visible.is_empty();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn draw_text(&mut self, tree_rom_addr: Option<u32>, entry: u16) {
        if let Some(text) = self.text_layer.as_mut() {
            let decoded = { text.bind().entry(tree_rom_addr, entry).cloned() };
            if let Some(entry) = decoded {
                text.bind_mut().draw_entry(&entry);
            } else {
                godot_warn!("opening text entry {entry} has no decoded tree record");
            }
        }
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn set_text_colour(&mut self, colour: u16) {
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().set_colour(colour);
        }
    }

    pub(crate) fn fade_text(&mut self, direction: i8) {
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().set_fade(direction);
        }
    }

    /// Advances the narration layer's own typewriter and fade clock.
    pub(super) fn tick_opening_text(&mut self) {
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().tick();
        }
    }
}
