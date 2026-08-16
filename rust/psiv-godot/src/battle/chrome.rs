use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;

use psiv_data::{DialogueSet, Role};

#[derive(Clone, Copy)]
pub(super) struct WindowRect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
}

impl WindowRect {
    pub(super) fn inset(self, amount: f32) -> WindowRect {
        WindowRect {
            x: self.x + amount,
            y: self.y + amount,
            w: (self.w - amount * 2.0).max(0.0),
            h: (self.h - amount * 2.0).max(0.0),
        }
    }
}

pub(super) struct Quad {
    pub(super) texture: Gd<ImageTexture>,
    pub(super) dest: Rect2,
    pub(super) src: Rect2,
}

pub(super) struct BattleChrome {
    tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    font: Gd<ImageTexture>,
    glyph_at: BTreeMap<char, Vector2>,
    glyph: Vector2,
    pub(super) cell: f32,
    pub(super) palette: [Color; 16],
}

impl BattleChrome {
    /// Uses the same pack assets and role flips as `dialogue.rs`, without
    /// borrowing the dialogue renderer or changing its >1k-line module.
    pub(super) fn build(pack_dir: &str, set: &DialogueSet) -> Option<BattleChrome> {
        let strip = load_image(pack_dir, &set.window.png)?;
        let mut tiles = BTreeMap::new();
        for role in Role::ALL {
            let tile = set.window.role(role)?;
            let region = Rect2i::new(
                Vector2i::new(tile.x, tile.y),
                Vector2i::new(tile.width as i32, tile.height as i32),
            );
            let mut cell = strip.get_region(region)?;
            if tile.flip_h {
                cell.flip_x();
            }
            if tile.flip_v {
                cell.flip_y();
            }
            tiles.insert(role.as_str(), ImageTexture::create_from_image(&cell)?);
        }

        // Battle/menu text is the retail 8x8 ArtNem_Font, not the 8x16
        // dialogue font. The pack asset is preferred; the generated copy is
        // a development fallback until older packs are regenerated.
        let menu_path = format!("{pack_dir}/dialogue/menu_font.png");
        let menu_image =
            Image::load_from_file(&GString::from(menu_path.as_str())).or_else(|| {
                let path = Path::new(pack_dir)
                    .parent()?
                    .join("generated/gfx/ArtNem_Font.png");
                Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
            })?;
        let font = ImageTexture::create_from_image(&menu_image)?;
        let glyph_at = retail_glyphs();
        let mut palette = [Color::BLACK; 16];
        for (index, rgb) in set.window.palette.colors.iter().take(16).enumerate() {
            palette[index] = Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255);
        }

        Some(BattleChrome {
            tiles,
            font,
            glyph_at,
            glyph: Vector2::new(8.0, 8.0),
            cell: set.window.geometry.cell_pixels as f32,
            palette,
        })
    }

    fn tile(&self, name: &'static str) -> Option<Gd<ImageTexture>> {
        self.tiles.get(name).cloned()
    }

    pub(super) fn frame(&self, rect: WindowRect) -> Option<Vec<Quad>> {
        let cols = (rect.w / self.cell).round() as i32;
        let rows = (rect.h / self.cell).round() as i32;
        if cols < 2 || rows < 2 {
            return None;
        }
        let mut quads = Vec::new();
        let at = |x: i32, y: i32| {
            Rect2::new(
                Vector2::new(rect.x + x as f32 * self.cell, rect.y + y as f32 * self.cell),
                Vector2::new(self.cell, self.cell),
            )
        };
        let src = Rect2::new(Vector2::ZERO, Vector2::new(self.cell, self.cell));
        let mut push = |name: &'static str, x: i32, y: i32| {
            if let Some(texture) = self.tile(name) {
                quads.push(Quad {
                    texture,
                    dest: at(x, y),
                    src,
                });
            }
        };

        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                push("fill", x, y);
            }
        }
        for x in 1..cols - 1 {
            push("edge_top", x, 0);
            push("edge_bottom", x, rows - 1);
        }
        for y in 1..rows - 1 {
            push("edge_left", 0, y);
            push("edge_right", cols - 1, y);
        }
        push("corner_top_left", 0, 0);
        push("corner_top_right", cols - 1, 0);
        push("corner_bottom_left", 0, rows - 1);
        push("corner_bottom_right", cols - 1, rows - 1);
        Some(quads)
    }

    pub(super) fn text(&self, text: &str, rect: WindowRect) -> Vec<Quad> {
        self.text_with_pitch(text, rect, self.glyph.y)
    }

    pub(super) fn text_with_pitch(
        &self,
        text: &str,
        rect: WindowRect,
        line_pitch: f32,
    ) -> Vec<Quad> {
        let cols = (rect.w / self.glyph.x.max(1.0)).floor() as usize;
        let rows = (rect.h / line_pitch.max(1.0)).floor() as usize;
        if cols == 0 || rows == 0 {
            return Vec::new();
        }
        let mut quads = Vec::new();
        let mut col = 0usize;
        let mut row = 0usize;
        for ch in text.chars() {
            if ch == '\n' {
                col = 0;
                row += 1;
                continue;
            }
            if col >= cols {
                col = 0;
                row += 1;
            }
            if row >= rows {
                break;
            }
            let Some(source) = self.glyph_at.get(&ch).or_else(|| self.glyph_at.get(&'?')) else {
                col += 1;
                continue;
            };
            quads.push(Quad {
                texture: self.font.clone(),
                dest: Rect2::new(
                    Vector2::new(
                        rect.x + col as f32 * self.glyph.x,
                        rect.y + row as f32 * line_pitch,
                    ),
                    self.glyph,
                ),
                src: Rect2::new(*source, self.glyph),
            });
            col += 1;
        }
        quads
    }
}

fn retail_glyphs() -> BTreeMap<char, Vector2> {
    let mut glyphs = BTreeMap::new();
    for (index, ch) in ('A'..='Z').enumerate() {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
        );
    }
    for (index, ch) in ('0'..='9').enumerate() {
        let index = index + 27;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
        );
    }
    for (index, ch) in ('a'..='z').enumerate() {
        let index = index + 57;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
        );
    }
    for (index, ch) in [
        (0, ' '),
        (49, '-'),
        (50, '!'),
        (51, '?'),
        (52, ':'),
        (83, '.'),
        (84, '\''),
        (85, ','),
    ] {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
        );
    }
    glyphs
}

fn load_image(pack_dir: &str, name: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{name}");
    Image::load_from_file(&GString::from(path.as_str()))
}
