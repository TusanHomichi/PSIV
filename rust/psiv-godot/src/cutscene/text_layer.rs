//! RunText2 plane text for the opening and staff roll.

use super::text_plane::TextPlane;
use crate::cram_art::{IndexedArt, SharedCram};
use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;
use psiv_core::SceneOp;
use psiv_data::color::IndexLayout;
use psiv_data::{DialogueEntry, DialogueSet};

/// The opening crawl is a separate drawable child so its CRAM text ramp does
/// not modulate the opaque black/title image behind it.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(super) struct TextPlaneLayer {
    base: Base<Node2D>,
    font: Option<Gd<ImageTexture>>,
    set: Option<DialogueSet>,
    plane: TextPlane,
    live_fonts: Vec<Option<IndexedArt>>,
    cram: Option<SharedCram>,
    intro: bool,
    flight_caption: Option<String>,
    colour: Color,
    fade_direction: i8,
    fade_tick: u8,
    fade_frames_left: u8,
}

#[godot_api]
impl INode2D for TextPlaneLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            font: None,
            set: None,
            plane: TextPlane::default(),
            live_fonts: Vec::new(),
            cram: None,
            intro: false,
            flight_caption: None,
            colour: Color::from_rgb(0.0, 0.0, 0.0),
            fade_direction: 0,
            fade_tick: 0,
            fade_frames_left: 0,
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
        let mut quads = Vec::new();
        let mut lines = Vec::new();
        for y in 0..28 {
            for x in 0..40 {
                let Some(cell) = self.plane.cell(x, y) else {
                    continue;
                };
                let Some(glyph) = self.set.as_ref().and_then(|set| set.glyph(cell.ch)) else {
                    continue;
                };
                let texture = if self.intro {
                    font.clone()
                } else {
                    let Some(cram) = self.cram.as_ref() else {
                        continue;
                    };
                    let Some(live) = self.live_fonts[usize::from(cell.line)]
                        .as_mut()
                        .and_then(|art| art.texture(&cram.borrow()))
                    else {
                        continue;
                    };
                    live
                };
                quads.push((
                    texture,
                    Rect2::new(
                        Vector2::new(x as f32 * 8.0, y as f32 * 8.0),
                        Vector2::new(8.0, 8.0),
                    ),
                    Rect2::new(
                        Vector2::new(glyph.x as f32, glyph.y as f32 + f32::from(cell.half) * 8.0),
                        Vector2::new(8.0, 8.0),
                    ),
                ));
            }
        }
        if let Some(caption) = self.flight_caption.as_deref() {
            // GetPlaneAOffset d3=4,d4=$17 (:134662-134678), 8x16 font.
            lines.push((caption, 32.0, 184.0));
        }
        for (line, x, y) in lines {
            for (column, ch) in line.chars().enumerate() {
                let Some(glyph) = self.set.as_ref().and_then(|set| set.glyph(ch)) else {
                    continue;
                };
                quads.push((
                    font.clone(),
                    Rect2::new(
                        Vector2::new(x + column as f32 * 8.0, y),
                        Vector2::new(8.0, 16.0),
                    ),
                    Rect2::new(
                        Vector2::new(glyph.x as f32, glyph.y as f32),
                        Vector2::new(8.0, 16.0),
                    ),
                ));
            }
        }
        for (texture, dest, src) in quads {
            self.base_mut()
                .draw_texture_rect_region(&texture, dest, src);
        }
    }
}

impl TextPlaneLayer {
    pub(super) fn flight_caption(&mut self, caption: Option<&str>) {
        if self.flight_caption.as_deref() == caption {
            return;
        }
        self.flight_caption = caption.map(str::to_owned);
        let visible = self.flight_caption.is_some() || self.plane.is_visible();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }
    pub(super) fn configure(&mut self, pack_dir: &str, set: DialogueSet) {
        let path = format!("{pack_dir}/{}", set.font.png);
        self.font = Image::load_from_file(&GString::from(path.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        self.live_fonts = (0..4)
            .map(|line| IndexedArt::load(&path, IndexLayout::Line(line)))
            .collect();
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

    pub(super) fn draw_entry(&mut self, entry: &DialogueEntry, plane: u32, tile: u32) {
        let line = entry
            .pages
            .first()
            .and_then(|p| p.lines.first())
            .map(String::as_str)
            .unwrap_or_default();
        if let Err(error) = self.plane.draw(plane, tile, line) {
            godot_error!("{error}: {plane:#010x}, {tile:#06x}");
        }
        self.refresh();
    }

    pub(super) fn consume(&mut self, op: &SceneOp) {
        self.plane.consume(op);
        if matches!(op, SceneOp::InitVramAndCram | SceneOp::LoadMap { .. }) {
            self.clear();
        }
        self.refresh();
    }

    pub(super) fn set_cram(&mut self, cram: SharedCram) {
        self.cram = Some(cram);
    }

    pub(super) fn refresh(&mut self) {
        let visible = self.plane.is_visible() || self.flight_caption.is_some();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }

    pub(super) fn clear(&mut self) {
        self.plane.clear();
        self.intro = false;
        self.fade_direction = 0;
        self.base_mut().set_modulate(Color::WHITE);
        self.refresh();
    }

    pub(super) fn set_colour(&mut self, raw: u16) {
        self.intro = true;
        self.colour = cram_colour(raw);
        let colour = self.colour;
        self.base_mut().set_modulate(colour);
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
        self.base_mut().set_modulate(next);
        self.base_mut().queue_redraw();
    }
}

fn cram_colour(raw: u16) -> Color {
    let channel = |shift: u16| f32::from((raw >> shift) & 0x7) / 7.0;
    Color::from_rgb(channel(1), channel(5), channel(9))
}
