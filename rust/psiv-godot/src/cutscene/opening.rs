//! The opening image and the narration text drawn over it.
//!
//! `LoadTitleImage` puts the stage on the black frame and hands the crawl to
//! [`OpeningTextLayer`](super::text_layer::OpeningTextLayer), a child node with
//! its own CRAM text ramp. This module is the only place that raises and clears
//! the opening; the layer's `draw` calls it first, in frame order.
//!
//! On a live palette the stage and the image draw from the runtime's CRAM
//! (the stage is the backdrop, word 0; the image's indices are its slots,
//! [`IndexLayout::Plane`]), so the prologue's `Pal_FadeIn` and
//! `PalFadeOut_ClrSpriteTbl` fade them as they fade the field (#637). Off a
//! live palette the baked image draws, under the shell's cover.

use godot::classes::ImageTexture;
use godot::prelude::*;
use psiv_data::color::IndexLayout;

use super::{CutsceneLayer, SCREEN};
use crate::cram_art::{CramState, IndexedArt};

impl CutsceneLayer {
    /// The Session's full RunText2 record; the shell never types or waits.
    pub(crate) fn sync_flight_caption(&mut self, caption: Option<&str>) {
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().flight_caption(caption);
        }
    }
    /// Draws the black stage and the narration image while the opening runs.
    pub(super) fn draw_opening(&mut self) {
        if self.opening_visible {
            let stage = stage_colour(self.cram.as_ref().map(|cram| cram.borrow()).as_deref());
            self.base_mut()
                .draw_rect(Rect2::new(Vector2::ZERO, SCREEN), stage);
            if let Some(texture) = self
                .live_opening()
                .or_else(|| self.opening_background.clone())
            {
                let screen_y = self.opening_screen_y;
                self.base_mut().draw_texture_rect(
                    &texture,
                    Rect2::new(Vector2::new(0.0, screen_y), Vector2::new(320.0, 128.0)),
                    true,
                );
            }
        }
    }

    /// The image in the live palette; `None` while no palette is live or the
    /// indexed file did not load.
    fn live_opening(&mut self) -> Option<Gd<ImageTexture>> {
        let cram = self.cram.clone()?;
        let cram = cram.borrow();
        if !cram.live {
            return None;
        }
        if self.opening_art.is_none() {
            let art = self
                .opening_background_path
                .as_deref()
                .and_then(|path| IndexedArt::load(path, IndexLayout::Plane));
            self.opening_art = Some(art);
        }
        self.opening_art.as_mut()?.as_mut()?.texture(&cram)
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
        let visible = self.shows_anything();
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

/// The opening's stage: the backdrop, CRAM word 0 (register 7 `$8700`),
/// while the palette is live; black otherwise.
fn stage_colour(cram: Option<&CramState>) -> Color {
    match cram {
        Some(cram) if cram.live => {
            let [r, g, b] = psiv_data::color::cram_rgb(cram.words[0]);
            Color::from_rgba8(r, g, b, 255)
        }
        _ => Color::from_rgb(0.0, 0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::{CramState, stage_colour};
    use godot::prelude::Color;

    /// The stage is the backdrop word while the palette is live, so a fade
    /// reaches it; without one it is black (negative control: the same
    /// words, not live).
    #[test]
    fn the_stage_is_the_live_backdrop_word() {
        let mut cram = CramState {
            live: true,
            ..CramState::default()
        };
        cram.words[0] = 0x0222;
        let [r, g, b] = psiv_data::color::cram_rgb(0x0222);
        assert_eq!(stage_colour(Some(&cram)), Color::from_rgba8(r, g, b, 255));
        cram.live = false;
        assert_eq!(stage_colour(Some(&cram)), Color::from_rgb(0.0, 0.0, 0.0));
        assert_eq!(stage_colour(None), Color::from_rgb(0.0, 0.0, 0.0));
    }
}
