//! The staged panel-plane stack, the palette effects that act on it and the
//! red-flash overlay.
//!
//! `staged` mirrors the cartridge's panel stack and `visible` changes only on
//! `DmaPlanes`, so a scene can build the next frame without exposing half of it
//! for a single Godot draw. A panel draws from the field's live palette
//! (`cram_art`): each layer's pixels on the line its art tile selects, so a
//! palette record reaches the panel on the frame the runtime's CRAM model
//! applies it. The file's baked colours draw only while no live palette is
//! known.

use godot::classes::ImageTexture;
use godot::prelude::*;

use crate::cram_art::{IndexedArt, SharedCram};

use super::{CutsceneLayer, SCREEN};

impl CutsceneLayer {
    /// Draws the panels the last `DmaPlanes` made visible.
    pub(super) fn draw_planes(&mut self) {
        let visible = self.panels.visible.clone();
        for id in visible {
            let live = self.live_panel(id);
            if let Some(texture) = live.or_else(|| self.textures.get(&id).cloned()) {
                self.base_mut().draw_texture_rect(
                    &texture,
                    Rect2::new(Vector2::ZERO, SCREEN),
                    true,
                );
            }
        }
    }

    /// Hands over the field's live palette.
    pub(crate) fn set_cram(&mut self, cram: SharedCram) {
        self.cram = Some(cram);
    }

    /// The live palette changed: redraw any visible panel or picture in it.
    pub(crate) fn palette_changed(&mut self) {
        if !self.panels.visible.is_empty() {
            self.base_mut().queue_redraw();
        }
        if self.picture_visible() {
            self.sync_picture();
        }
    }

    /// Panel `id` in the live palette, its indices read on its first draw;
    /// `None` while no live palette is known or the indices did not load.
    fn live_panel(&mut self, id: u16) -> Option<Gd<ImageTexture>> {
        let cram = self.cram.clone()?;
        let cram = cram.borrow();
        if !cram.live {
            return None;
        }
        if !self.panel_art.contains_key(&id) {
            let art = self
                .indexed_panels
                .get(&id)
                .and_then(|(path, layout)| IndexedArt::load(path, *layout));
            self.panel_art.insert(id, art);
        }
        self.panel_art.get_mut(&id)?.as_mut()?.texture(&cram)
    }

    pub(crate) fn panel_create(&mut self, id: u16) {
        if self.textures.contains_key(&id) {
            self.panels.staged.push(id);
            godot_print!("scene panel staged: {id:#05x}");
        } else {
            godot_error!("scene panel {id:#05x} is not decoded in the runtime pack");
        }
    }

    pub(crate) fn panel_destroy_last(&mut self) {
        match self.panels.pop() {
            Some(id) => godot_print!("scene panel destroyed: last {id:#05x}"),
            None => {
                godot_warn!("Panel_Destroy stack underflow: no staged panel");
            }
        }
    }

    /// A `$F2` palette action: the runtime's CRAM model applied it; redraw
    /// so the panels show it.
    pub(crate) fn refresh_palette(&mut self) {
        self.base_mut().queue_redraw();
        godot_print!("dialogue action: palette refresh");
    }

    /// Small red-palette overlay used by the three retail dialogue effects
    /// whose source writes CRAM synchronously instead of loading a panel.
    pub(crate) fn red_flash(&mut self, frames: u8) {
        self.red_flash_frames = self.red_flash_frames.max(frames);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn panel_destroy_all(&mut self) {
        self.panels.clear();
        let visible = self.shows_anything();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
        godot_print!("scene panels destroyed: all");
    }

    pub(crate) fn dma_planes(&mut self) {
        self.panels.dma();
        let visible = self.shows_anything();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
        godot_print!(
            "scene DMA planes: {} visible panel(s){}",
            self.panels.visible.len(),
            if self.picture_visible() {
                ", plane A picture"
            } else {
                ""
            }
        );
    }

    /// Whether the layer has anything to draw: the opening, the generic
    /// window, a visible panel or a plane-A picture.
    pub(super) fn shows_anything(&self) -> bool {
        self.opening_visible
            || self.generic_window_visible
            || !self.panels.visible.is_empty()
            || self.picture_visible()
    }

    /// Draws the red-palette overlay while `red_flash` frames remain.
    pub(super) fn draw_red_flash(&mut self) {
        if self.red_flash_frames > 0 {
            self.base_mut().draw_rect(
                Rect2::new(Vector2::ZERO, SCREEN),
                Color::from_rgba(0.8, 0.0, 0.0, 0.55),
            );
        }
    }
}
