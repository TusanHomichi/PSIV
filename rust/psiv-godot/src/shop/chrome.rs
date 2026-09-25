//! Retail shop window and menu-font blits.
//!
//! [`ShopChrome`] turns the decoded window strip and menu font into the tiles,
//! window words and glyph quads a shop page is assembled from. It owns no
//! layout: callers ask for a frame by `CellRect` and words by pattern.

use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;

use psiv_data::{DialogueSet, Role};

use super::layout::{CELL, CellRect};

pub(super) struct Quad {
    pub(super) texture: Gd<ImageTexture>,
    pub(super) dest: Rect2,
    pub(super) src: Rect2,
}

pub(super) struct ShopChrome {
    tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    words: BTreeMap<u16, Gd<ImageTexture>>,
    font: Gd<ImageTexture>,
    glyph_at: BTreeMap<char, Vector2>,
}

impl ShopChrome {
    pub(super) fn build(pack_dir: &str, set: &DialogueSet) -> Option<ShopChrome> {
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
        let menu_path = format!("{pack_dir}/dialogue/menu_font.png");
        let menu = Image::load_from_file(&GString::from(menu_path.as_str())).or_else(|| {
            let path = Path::new(pack_dir)
                .parent()?
                .join("generated/gfx/ArtNem_Font.png");
            Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
        })?;
        let font = ImageTexture::create_from_image(&menu)?;
        let mut words = BTreeMap::new();
        for pattern in [0x680, 0x6E7, 0x6E8] {
            let source = Rect2i::new(
                Vector2i::new(i32::from(pattern - 0x680) * 8, 0),
                Vector2i::new(8, 8),
            );
            words.insert(
                pattern,
                ImageTexture::create_from_image(&strip.get_region(source)?)?,
            );
        }
        Some(ShopChrome {
            tiles,
            words,
            font,
            glyph_at: retail_glyphs(),
        })
    }

    pub(super) fn frame(&self, rect: CellRect) -> Vec<Quad> {
        let mut quads = Vec::new();
        let mut push = |name: &'static str, x: i32, y: i32| {
            if let Some(texture) = self.tiles.get(name) {
                quads.push(Quad {
                    texture: texture.clone(),
                    dest: Rect2::new(
                        Vector2::new(x as f32 * CELL, y as f32 * CELL),
                        Vector2::new(CELL, CELL),
                    ),
                    src: Rect2::new(Vector2::ZERO, Vector2::new(CELL, CELL)),
                });
            }
        };
        for y in 1..rect.h - 1 {
            for x in 1..rect.w - 1 {
                push("fill", rect.x + x, rect.y + y);
            }
        }
        for x in 1..rect.w - 1 {
            push("edge_top", rect.x + x, rect.y);
            push("edge_bottom", rect.x + x, rect.y + rect.h - 1);
        }
        for y in 1..rect.h - 1 {
            push("edge_left", rect.x, rect.y + y);
            push("edge_right", rect.x + rect.w - 1, rect.y + y);
        }
        push("corner_top_left", rect.x, rect.y);
        push("corner_top_right", rect.x + rect.w - 1, rect.y);
        push("corner_bottom_left", rect.x, rect.y + rect.h - 1);
        push(
            "corner_bottom_right",
            rect.x + rect.w - 1,
            rect.y + rect.h - 1,
        );
        quads
    }

    pub(super) fn word(&self, pattern: u16, cell: (i32, i32)) -> Option<Quad> {
        Some(Quad {
            texture: self.words.get(&pattern)?.clone(),
            dest: Rect2::new(
                Vector2::new(cell.0 as f32 * CELL, cell.1 as f32 * CELL),
                Vector2::new(CELL, CELL),
            ),
            src: Rect2::new(Vector2::ZERO, Vector2::new(CELL, CELL)),
        })
    }

    pub(super) fn text(&self, text: &str, cell: (i32, i32)) -> Vec<Quad> {
        let mut quads = Vec::new();
        for (row, line) in text.lines().enumerate() {
            for (column, character) in line.chars().enumerate() {
                let dest_cell = (cell.0 + column as i32, cell.1 + row as i32);
                if character == ' ' {
                    if let Some(quad) = self.word(0x680, dest_cell) {
                        quads.push(quad);
                    }
                    continue;
                }
                let Some(source) = self
                    .glyph_at
                    .get(&character)
                    .or_else(|| self.glyph_at.get(&'?'))
                else {
                    continue;
                };
                quads.push(Quad {
                    texture: self.font.clone(),
                    dest: Rect2::new(
                        Vector2::new(dest_cell.0 as f32 * CELL, dest_cell.1 as f32 * CELL),
                        Vector2::new(CELL, CELL),
                    ),
                    src: Rect2::new(*source, Vector2::new(CELL, CELL)),
                });
            }
        }
        quads
    }
}

fn retail_glyphs() -> BTreeMap<char, Vector2> {
    let mut glyphs = BTreeMap::new();
    for (index, ch) in ('A'..='Z').enumerate() {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    for (index, ch) in ('0'..='9').enumerate() {
        let index = index + 26;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    for (index, ch) in ('a'..='z').enumerate() {
        let index = index + 56;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    for (index, ch) in [
        (48, '-'),
        (49, '!'),
        (50, '?'),
        (51, ':'),
        (52, ','),
        (53, '.'),
        (54, '<'),
        (55, '>'),
    ] {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    glyphs
}

fn load_image(pack_dir: &str, name: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{name}");
    Image::load_from_file(&GString::from(path.as_str()))
}
