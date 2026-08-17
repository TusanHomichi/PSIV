//! Opening crawl text rendering for the scene presentation layer.

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;
use psiv_data::{DialogueEntry, DialogueSet};

use super::SCREEN;

/// The opening crawl is a separate drawable child so its CRAM text ramp does
/// not modulate the opaque black/title image behind it.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(super) struct OpeningTextLayer {
    base: Base<Node2D>,
    font: Option<Gd<ImageTexture>>,
    set: Option<DialogueSet>,
    entries: [Option<DialogueEntry>; 4],
    next_slot: usize,
    colour: Color,
    fade_direction: i8,
    fade_tick: u8,
    fade_frames_left: u8,
}

#[godot_api]
impl INode2D for OpeningTextLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            font: None,
            set: None,
            entries: std::array::from_fn(|_| None),
            next_slot: 0,
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
        for (row, entry) in self.entries.iter().enumerate() {
            let Some(entry) = entry else { continue };
            let Some(page) = entry.pages.first() else {
                continue;
            };
            let line = page.lines.first().map(String::as_str).unwrap_or_default();
            // Plane offset $40A is tile column 5: every line starts at x=40
            // and any centring is baked into the entry text itself.
            let x = 40.0;
            // DrawTextToPlane targets $840A, then advances 0x180 bytes per
            // entry: plane offset $40A is tile row 8 (y=64) and 0x180 bytes
            // is three 0x80-byte plane rows, a 24-pixel line pitch — both
            // confirmed against oracle/frames/opening/frame_4000.png.
            let y = 64.0 + row as f32 * 24.0;
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
                ));
            }
        }
        for (dest, src) in quads {
            self.base_mut().draw_texture_rect_region(&font, dest, src);
        }
    }
}

impl OpeningTextLayer {
    pub(super) fn configure(&mut self, pack_dir: &str, set: DialogueSet) {
        let path = format!("{pack_dir}/{}", set.font.png);
        self.font = Image::load_from_file(&GString::from(path.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        self.set = Some(set);
        self.base_mut().set_visible(false);
    }

    pub(super) fn entry(&self, tree_rom_addr: Option<u32>, entry: u16) -> Option<&DialogueEntry> {
        // The opening's SetDialogueTree points at retail DialogueTree17. The
        // pack carries the same tree by its stable 1-based number; other
        // scene tree addresses belong to ordinary dialogue windows.
        if tree_rom_addr != Some(0x001E_BA90) {
            return None;
        }
        self.set.as_ref()?.entry(17, entry)
    }

    pub(super) fn place(&mut self) {
        let viewport = self.base().get_viewport_rect();
        let canvas = self.base().get_canvas_transform().affine_inverse();
        let top_left = canvas * viewport.position;
        let bottom_right = canvas * (viewport.position + viewport.size);
        let visible = bottom_right - top_left;
        self.base_mut().set_position(Vector2::new(
            (top_left.x + (visible.x - SCREEN.x) / 2.0).floor(),
            (top_left.y + (visible.y - SCREEN.y) / 2.0).floor(),
        ));
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
