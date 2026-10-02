//! The retail yes/no window's art: two rows, a cursor, and nothing else.
//!
//! The answer rules — up/down edges, Speak on the cursor's row, Cancel as the
//! direct NO — live in `psiv-runtime`'s dialogue choice state. This module
//! only knows how the prompt looks: the `yes_no` record's window from the
//! pack, the menu font, and `ArtNem_Window` pattern $6E8 as the cursor.
use super::{Quad, WindowView, load_image};
use godot::classes::ImageTexture;
use godot::prelude::*;
use psiv_data::{DialogueSet, Role};

pub(super) struct ChoiceView {
    font: Gd<ImageTexture>,
    cursor: Gd<ImageTexture>,
    origin: Vector2,
    cells: (i32, i32),
}

impl ChoiceView {
    pub(super) fn build(pack: &str, set: &DialogueSet) -> Option<Self> {
        let record = set.window.windows.iter().find(|w| w.name == "yes_no")?;
        let text = set.window.text_window.rect;
        let strip = load_image(pack, &set.window.png)?;
        // ArtNem_Window pattern $6E8, relative to its $680 VRAM base.
        let cursor =
            strip.get_region(Rect2i::new(Vector2i::new(0x68 * 8, 0), Vector2i::new(8, 8)))?;
        Some(Self {
            font: ImageTexture::create_from_image(&load_image(pack, "dialogue/menu_font.png")?)?,
            cursor: ImageTexture::create_from_image(&cursor)?,
            origin: Vector2::new(
                (record.rect.x - text.x) as f32,
                (record.rect.y - text.y) as f32,
            ),
            cells: (record.width_cells as i32, record.height_cells as i32),
        })
    }

    pub(super) fn quads(&self, view: &WindowView, selected: usize) -> Vec<Quad> {
        let mut out = Vec::new();
        let cell = Vector2::new(8.0, 8.0);
        for y in 0..self.cells.1 {
            for x in 0..self.cells.0 {
                let role = match (x == 0, x == self.cells.0 - 1, y == 0, y == self.cells.1 - 1) {
                    (true, _, true, _) => Role::CornerTopLeft,
                    (_, true, true, _) => Role::CornerTopRight,
                    (true, _, _, true) => Role::CornerBottomLeft,
                    (_, true, _, true) => Role::CornerBottomRight,
                    (_, _, true, _) => Role::EdgeTop,
                    (_, _, _, true) => Role::EdgeBottom,
                    (true, _, _, _) => Role::EdgeLeft,
                    (_, true, _, _) => Role::EdgeRight,
                    _ => Role::Fill,
                };
                if let Some(texture) = view.tile(role) {
                    out.push(Quad {
                        texture: texture.clone(),
                        dest: Rect2::new(
                            self.origin + Vector2::new(x as f32 * 8.0, y as f32 * 8.0),
                            cell,
                        ),
                        src: Rect2::new(Vector2::ZERO, cell),
                    });
                }
            }
        }
        for (row, label) in ["YES", "NO"].iter().enumerate() {
            for (column, ch) in label.bytes().enumerate() {
                let glyph = ch - b'A';
                out.push(Quad {
                    texture: self.font.clone(),
                    dest: Rect2::new(
                        self.origin
                            + Vector2::new(16.0 + column as f32 * 8.0, 8.0 + row as f32 * 16.0),
                        cell,
                    ),
                    src: Rect2::new(
                        Vector2::new(f32::from(glyph % 16) * 8.0, f32::from(glyph / 16) * 8.0),
                        cell,
                    ),
                });
            }
        }
        out.push(Quad {
            texture: self.cursor.clone(),
            dest: Rect2::new(
                self.origin + Vector2::new(8.0, 8.0 + selected as f32 * 16.0),
                cell,
            ),
            src: Rect2::new(Vector2::ZERO, cell),
        });
        out
    }
}
