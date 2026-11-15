//! Opening crawl text rendering for the scene presentation layer.

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;
use psiv_data::{DialogueEntry, DialogueSet};

use crate::cram_art::{CramState, SharedCram};

/// The opening crawl is a separate drawable child so its CRAM text ramp does
/// not modulate the opaque black/title image behind it.
///
/// Both texts are drawn in `Palette_Table_Buffer+$5E`
/// ([`psiv_runtime::TEXT_COLOUR_SLOT`]). The flight's caption reads it from
/// the runtime's CRAM while that is live, so `PalFadeOut_ClrSpriteTbl` takes
/// it down with everything else (#637). The crawl keeps the layer's own ramp
/// (`IntroTextFadeUp`/`Down` are not CRAM records); it is black before the
/// prologue's `Pal_FadeIn` draws it and after its last fade down, so no
/// fixed fade runs while it shows.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(super) struct OpeningTextLayer {
    base: Base<Node2D>,
    font: Option<Gd<ImageTexture>>,
    set: Option<DialogueSet>,
    entries: [Option<DialogueEntry>; 4],
    flight_caption: Option<String>,
    next_slot: usize,
    colour: Color,
    fade_direction: i8,
    fade_tick: u8,
    fade_frames_left: u8,
    cram: Option<SharedCram>,
}

#[godot_api]
impl INode2D for OpeningTextLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            font: None,
            set: None,
            entries: std::array::from_fn(|_| None),
            flight_caption: None,
            next_slot: 0,
            colour: Color::from_rgb(0.0, 0.0, 0.0),
            fade_direction: 0,
            fade_tick: 0,
            fade_frames_left: 0,
            cram: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(560);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        let Some(font) = self.font.clone() else {
            return;
        };
        let crawl = self.colour;
        let caption = caption_colour(
            self.cram.as_ref().map(|cram| cram.borrow()).as_deref(),
            crawl,
        );
        let mut quads = Vec::new();
        let mut lines = Vec::new();
        for (row, entry) in self.entries.iter().enumerate() {
            let Some(entry) = entry else { continue };
            let Some(page) = entry.pages.first() else {
                continue;
            };
            let line = page.lines.first().map(String::as_str).unwrap_or_default();
            // Plane offset $40A is tile column 5: every line starts at x=40
            // and any centring is baked into the entry text itself.
            // DrawTextToPlane targets $840A, then advances 0x180 bytes per
            // entry: plane offset $40A is tile row 8 (y=64) and 0x180 bytes
            // is three 0x80-byte plane rows, a 24-pixel line pitch — both
            // confirmed against oracle/frames/opening/frame_4000.png.
            let y = 64.0 + row as f32 * 24.0;
            lines.push((line, 40.0, y, crawl));
        }
        if let Some(text) = self.flight_caption.as_deref() {
            // GetPlaneAOffset d3=4,d4=$17 (:134662-134678), 8x16 font.
            lines.push((text, 32.0, 184.0, caption));
        }
        for (line, x, y, colour) in lines {
            for (column, ch) in line.chars().enumerate() {
                let Some(glyph) = self.set.as_ref().and_then(|set| set.glyph(ch)) else {
                    continue;
                };
                quads.push((
                    Rect2::new(
                        Vector2::new(x + column as f32 * 8.0, y),
                        Vector2::new(8.0, 16.0),
                    ),
                    Rect2::new(
                        Vector2::new(glyph.x as f32, glyph.y as f32),
                        Vector2::new(8.0, 16.0),
                    ),
                    colour,
                ));
            }
        }
        for (dest, src, colour) in quads {
            self.base_mut()
                .draw_texture_rect_region_ex(&font, dest, src)
                .modulate(colour)
                .done();
        }
    }
}

impl OpeningTextLayer {
    pub(super) fn flight_caption(&mut self, caption: Option<&str>) {
        if self.flight_caption.as_deref() == caption {
            return;
        }
        self.flight_caption = caption.map(str::to_owned);
        let visible = self.flight_caption.is_some() || self.entries.iter().any(Option::is_some);
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }
    /// Hands over the field's live palette.
    pub(super) fn set_cram(&mut self, cram: SharedCram) {
        self.cram = Some(cram);
    }

    /// The live palette changed: a caption drawn in it is redrawn.
    pub(super) fn palette_changed(&mut self) {
        if self.flight_caption.is_some() {
            self.base_mut().queue_redraw();
        }
    }

    pub(super) fn configure(&mut self, pack_dir: &str, set: DialogueSet) {
        let path = format!("{pack_dir}/{}", set.font.png);
        self.font = Image::load_from_file(&GString::from(path.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        self.set = Some(set);
        self.base_mut().set_visible(false);
    }

    pub(super) fn entry(&self, tree_rom_addr: Option<u32>, entry: u16) -> Option<&DialogueEntry> {
        let set = self.set.as_ref()?;
        let address = tree_rom_addr?;
        let tree = set
            .trees
            .trees
            .iter()
            .find(|tree| tree.rom_address() == Some(address))?;
        tree.entry(entry)
    }

    pub(super) fn place(&mut self) {
        let position = crate::retail_surface::surface_origin(
            self.base().get_canvas_transform().affine_inverse(),
            self.base().get_viewport_rect(),
        );
        self.base_mut().set_position(position);
    }

    pub(super) fn draw_entry(&mut self, entry: &DialogueEntry) {
        self.entries[self.next_slot] = Some(entry.clone());
        self.next_slot = (self.next_slot + 1) % self.entries.len();
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(super) fn clear(&mut self) {
        self.entries = std::array::from_fn(|_| None);
        self.next_slot = 0;
        self.base_mut().set_visible(false);
        self.base_mut().queue_redraw();
    }

    pub(super) fn set_colour(&mut self, raw: u16) {
        self.colour = cram_colour(raw);
        self.base_mut().queue_redraw();
    }

    pub(super) fn set_fade(&mut self, direction: i8) {
        self.fade_direction = direction;
        self.fade_tick = 0;
        self.fade_frames_left = 20;
    }

    pub(super) fn tick(&mut self) {
        if self.fade_direction == 0 || self.fade_frames_left == 0 {
            return;
        }
        self.fade_frames_left = self.fade_frames_left.saturating_sub(1);
        self.fade_tick = self.fade_tick.wrapping_add(1);
        if !self.fade_tick.is_multiple_of(4) {
            return;
        }
        let channels = self.colour;
        let step = 2.0 / 7.0;
        let next = if self.fade_direction > 0 {
            Color::from_rgb(
                (channels.r + step).min(1.0),
                (channels.g + step).min(1.0),
                (channels.b + step).min(1.0),
            )
        } else {
            Color::from_rgb(
                (channels.r - step).max(0.0),
                (channels.g - step).max(0.0),
                (channels.b - step).max(0.0),
            )
        };
        self.colour = next;
        self.base_mut().queue_redraw();
    }
}

fn cram_colour(raw: u16) -> Color {
    let channel = |shift: u16| f32::from((raw >> shift) & 0x7) / 7.0;
    Color::from_rgb(channel(1), channel(5), channel(9))
}

/// The colour the font's glyphs are baked in: `Pal_Init_Line_3`'s word 15,
/// `$EEE`, the text word's slot on line 2 (`dialogue/font.png`'s opaque
/// index).
const GLYPH_WORD: u16 = 0x0EEE;

/// The caption's modulate: the CRAM's text word over the glyphs' baked
/// colour while the palette is live, so a glyph pixel draws that word's
/// colour; else the layer's own colour (`crawl`), as a pack without a live
/// palette draws it.
fn caption_colour(cram: Option<&CramState>, crawl: Color) -> Color {
    match cram {
        Some(cram) if cram.live => {
            let word = psiv_data::color::cram_rgb(cram.words[psiv_runtime::TEXT_COLOUR_SLOT]);
            let baked = psiv_data::color::cram_rgb(GLYPH_WORD);
            let channel = |index: usize| f32::from(word[index]) / f32::from(baked[index]);
            Color::from_rgb(channel(0), channel(1), channel(2))
        }
        _ => crawl,
    }
}

#[cfg(test)]
mod tests {
    use super::{CramState, GLYPH_WORD, caption_colour, cram_colour};

    fn live(word: u16) -> CramState {
        let mut cram = CramState {
            live: true,
            ..CramState::default()
        };
        cram.words[psiv_runtime::TEXT_COLOUR_SLOT] = word;
        cram
    }

    /// What a glyph pixel, baked in [`GLYPH_WORD`], draws under `colour`.
    fn glyph(colour: godot::prelude::Color) -> [u8; 3] {
        let baked = psiv_data::color::cram_rgb(GLYPH_WORD);
        let scale = |index: usize, by: f32| (f32::from(baked[index]) * by).round() as u8;
        [scale(0, colour.r), scale(1, colour.g), scale(2, colour.b)]
    }

    /// The caption draws the CRAM's text word: `$EEE` as written, each
    /// level a fade-out pass leaves, black at its end.
    #[test]
    fn the_caption_is_drawn_in_the_live_text_word() {
        let crawl = cram_colour(0x0EEE);
        for word in [0x0EEE, 0x0DDD, 0x0CCC, 0x0864, 0x0222, 0x0000] {
            assert_eq!(
                glyph(caption_colour(Some(&live(word)), crawl)),
                psiv_data::color::cram_rgb(word),
                "{word:#06x}"
            );
        }
    }

    /// Negative control: without a live palette the layer's own colour
    /// stands, whatever the words hold, so a fade on the words reaches no
    /// caption.
    #[test]
    fn a_palette_that_is_not_live_keeps_the_layer_colour() {
        let crawl = cram_colour(0x0EEE);
        let mut cram = live(0);
        cram.live = false;
        assert_eq!(caption_colour(Some(&cram), crawl), crawl);
        assert_eq!(caption_colour(None, crawl), crawl);
        assert_ne!(glyph(crawl), psiv_data::color::cram_rgb(0));
    }
}
