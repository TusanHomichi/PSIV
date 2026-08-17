//! Retail camp window and menu-font blits.
//!
//! This is intentionally a camp-local copy of the small texture assembler in
//! the battle UI. The battle module is an owned lane and is not modified by
//! the field-menu work.

use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;

use psiv_data::{DialogueSet, Role};

use super::layout::{CELL, CellRect};

const WINDOW_BASE_TILE: u16 = 0x680;
/// Retail's HP/TP separator is loaded from the window charset, not the
/// ordinary menu font. `WinTiles_CharStatsOverview` emits raw byte `$77`,
/// which becomes pattern `$6F7` after the `$680` window base is added.
pub(super) const STATUS_SLASH_PATTERN: u16 = 0x6F7;

pub(super) struct Quad {
    pub(super) texture: Gd<ImageTexture>,
    pub(super) dest: Rect2,
    pub(super) src: Rect2,
}

pub(super) struct CampChrome {
    tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    window_words: BTreeMap<(u16, bool, bool), Gd<ImageTexture>>,
    font: Gd<ImageTexture>,
    glyph_at: BTreeMap<char, Vector2>,
}

impl CampChrome {
    pub(super) fn build(pack_dir: &str, set: &DialogueSet) -> Option<CampChrome> {
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
        let menu_image =
            Image::load_from_file(&GString::from(menu_path.as_str())).or_else(|| {
                let path = Path::new(pack_dir)
                    .parent()?
                    .join("generated/gfx/ArtNem_Font.png");
                Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
            })?;
        Some(CampChrome {
            window_words: retail_window_words(&strip)?,
            font: ImageTexture::create_from_image(&menu_image)?,
            glyph_at: retail_glyphs(),
            tiles,
        })
    }

    pub(super) fn frame(&self, rect: CellRect) -> Vec<Quad> {
        let mut quads = Vec::new();
        let push = |quads: &mut Vec<Quad>,
                    tiles: &BTreeMap<&'static str, Gd<ImageTexture>>,
                    name: &'static str,
                    x: i32,
                    y: i32| {
            if let Some(texture) = tiles.get(name) {
                quads.push(Quad {
                    texture: texture.clone(),
                    dest: Rect2::new(
                        Vector2::new((rect.x + x) as f32 * CELL, (rect.y + y) as f32 * CELL),
                        Vector2::new(CELL, CELL),
                    ),
                    src: Rect2::new(Vector2::ZERO, Vector2::new(CELL, CELL)),
                });
            }
        };
        for y in 1..rect.h - 1 {
            for x in 1..rect.w - 1 {
                push(&mut quads, &self.tiles, "fill", x, y);
            }
        }
        for x in 1..rect.w - 1 {
            push(&mut quads, &self.tiles, "edge_top", x, 0);
            push(&mut quads, &self.tiles, "edge_bottom", x, rect.h - 1);
        }
        for y in 1..rect.h - 1 {
            push(&mut quads, &self.tiles, "edge_left", 0, y);
            push(&mut quads, &self.tiles, "edge_right", rect.w - 1, y);
        }
        push(&mut quads, &self.tiles, "corner_top_left", 0, 0);
        push(&mut quads, &self.tiles, "corner_top_right", rect.w - 1, 0);
        push(&mut quads, &self.tiles, "corner_bottom_left", 0, rect.h - 1);
        push(
            &mut quads,
            &self.tiles,
            "corner_bottom_right",
            rect.w - 1,
            rect.h - 1,
        );
        quads
    }

    pub(super) fn window_word(&self, pattern: u16, cell: (i32, i32)) -> Option<Quad> {
        Some(Quad {
            texture: self.window_words.get(&(pattern, false, false))?.clone(),
            dest: Rect2::new(
                Vector2::new(cell.0 as f32 * CELL, cell.1 as f32 * CELL),
                Vector2::new(CELL, CELL),
            ),
            src: Rect2::new(Vector2::ZERO, Vector2::new(CELL, CELL)),
        })
    }

    pub(super) fn text(&self, text: &str, cell: (i32, i32)) -> Vec<Quad> {
        let mut quads = Vec::new();
        for (column, character) in text.chars().enumerate() {
            let dest_cell = (cell.0 + column as i32, cell.1);
            if character == '/' {
                if let Some(quad) = self.window_word(STATUS_SLASH_PATTERN, dest_cell) {
                    quads.push(quad);
                }
                continue;
            }
            let dest = Rect2::new(
                Vector2::new(dest_cell.0 as f32 * CELL, dest_cell.1 as f32 * CELL),
                Vector2::new(CELL, CELL),
            );
            if character == ' ' {
                if let Some(quad) = self.window_word(0x680, dest_cell) {
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
                dest,
                src: Rect2::new(*source, Vector2::new(CELL, CELL)),
            });
        }
        quads
    }

    /// The status routine's current/max HP and TP values use the second
    /// numeric run in `menu_font.png` (source cells 36..45). Level, money,
    /// and ordinary menu strings retain the first run used by `text`.
    pub(super) fn number(&self, text: &str, cell: (i32, i32)) -> Vec<Quad> {
        let columns = (self.font.get_width() / CELL as i32).max(1);
        let mut quads = Vec::new();
        for (column, character) in text.chars().enumerate() {
            let Some(digit) = character.to_digit(10) else {
                continue;
            };
            let index = 36 + digit as i32;
            let source = Vector2::new(
                (index % columns) as f32 * CELL,
                (index / columns) as f32 * CELL,
            );
            quads.push(Quad {
                texture: self.font.clone(),
                dest: Rect2::new(
                    Vector2::new((cell.0 + column as i32) as f32 * CELL, cell.1 as f32 * CELL),
                    Vector2::new(CELL, CELL),
                ),
                src: Rect2::new(source, Vector2::new(CELL, CELL)),
            });
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

fn retail_window_words(strip: &Gd<Image>) -> Option<BTreeMap<(u16, bool, bool), Gd<ImageTexture>>> {
    const PATTERNS: [u16; 4] = [0x680, 0x6E7, 0x6E8, STATUS_SLASH_PATTERN];
    let mut words = BTreeMap::new();
    for pattern in PATTERNS {
        let index = i32::from(pattern - WINDOW_BASE_TILE);
        for flip_h in [false, true] {
            for flip_v in [false, true] {
                let region = Rect2i::new(Vector2i::new(index * 8, 0), Vector2i::new(8, 8));
                let mut cell = strip.get_region(region)?;
                if flip_h {
                    cell.flip_x();
                }
                if flip_v {
                    cell.flip_y();
                }
                words.insert(
                    (pattern, flip_h, flip_v),
                    ImageTexture::create_from_image(&cell)?,
                );
            }
        }
    }
    Some(words)
}

fn load_image(pack_dir: &str, name: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{name}");
    Image::load_from_file(&GString::from(path.as_str()))
}
