//! The staged panel-plane stack, the palette effects that act on it and the
//! red-flash overlay.
//!
//! `staged` mirrors the cartridge's panel stack and `visible` changes only on
//! `DmaPlanes`, so a scene can build the next frame without exposing half of it
//! for a single Godot draw. Palette actions target CRAM, which this pack bakes
//! into the panel PNGs, so redrawing is the faithful operation at this
//! presentation boundary; the red-flash overlay is the one effect written
//! straight to CRAM.

use godot::prelude::*;

use super::{CutsceneLayer, SCREEN};

/// Retail `grand_cross=0` keeps the plane origin one pixel right/down of the
/// window coordinate origin in the settled MeetingRika receipt.  Camera and
/// VDP scroll values are decoded in `oracle/scroll_state.py`; this is the
/// remaining hardware-origin term, not a camera adjustment.
const RETAIL_PLANE_SCROLL_RESIDUE: Vector2 = Vector2::new(1.0, 1.0);

fn retail_plane_scroll_offset() -> Vector2 {
    RETAIL_PLANE_SCROLL_RESIDUE
}

impl CutsceneLayer {
    /// Draws the panels the last `DmaPlanes` made visible.
    pub(super) fn draw_planes(&mut self) {
        let visible = self.visible.clone();
        for id in visible {
            if let Some(texture) = self.textures.get(&id).cloned() {
                self.base_mut().draw_texture_rect(
                    &texture,
                    Rect2::new(retail_plane_scroll_offset(), SCREEN),
                    true,
                );
            }
        }
    }

    pub(crate) fn panel_create(&mut self, id: u16) {
        if self.textures.contains_key(&id) {
            self.staged.push(id);
            godot_print!("scene panel staged: {id:#05x}");
        } else {
            godot_error!("scene panel {id:#05x} is not decoded in the runtime pack");
        }
    }

    pub(crate) fn load_palette(&self, rom_addr: u32, words: u16) {
        match self.palettes.get(&rom_addr) {
            Some(decoded) if decoded.len() >= usize::from(words) => {
                godot_print!("scene palette loaded: {words} words from {rom_addr:#08x}");
            }
            Some(decoded) => godot_error!(
                "scene palette {rom_addr:#08x} has {} words, requested {words}",
                decoded.len()
            ),
            None => godot_error!("scene palette {rom_addr:#08x} is not in the runtime pack"),
        }
    }

    pub(crate) fn panel_destroy(&mut self, id: u16) {
        if let Some(actual) = self.staged.pop()
            && actual != id
        {
            godot_warn!(
                "Panel_Destroy({id:#05x}) popped staged panel {actual:#05x}; retail allocator is stack based"
            );
        }
        godot_print!("scene panel destroyed: {id:#05x}");
    }

    pub(crate) fn panel_destroy_last(&mut self) {
        if let Some(id) = self.staged.pop() {
            godot_print!("scene panel destroyed: last {id:#05x}");
        }
    }

    /// `$F2` palette actions target CRAM, while this pack stores panels as
    /// palette-baked PNGs. Redrawing is the faithful operation available at
    /// this presentation boundary; the raw palette records remain auditable
    /// in `presentation/panels.json`.
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
        self.staged.clear();
        self.visible.clear();
        let opening_visible = self.opening_visible;
        let generic_window_visible = self.generic_window_visible;
        self.base_mut()
            .set_visible(opening_visible || generic_window_visible);
        self.base_mut().queue_redraw();
        godot_print!("scene panels destroyed: all");
    }

    pub(crate) fn dma_planes(&mut self) {
        self.visible.clone_from(&self.staged);
        let visible =
            self.opening_visible || self.generic_window_visible || !self.visible.is_empty();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
        godot_print!("scene DMA planes: {} visible panel(s)", self.visible.len());
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
