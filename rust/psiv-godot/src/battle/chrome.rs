use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{Image, ImageTexture, image::Format};
use godot::prelude::*;

use psiv_data::{DialogueSet, Role};

const WINDOW_BASE_TILE: u16 = 0x680;
const FONT_BASE_TILE: u16 = 0x7c0;

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
    window_words: BTreeMap<(u16, bool, bool), Gd<ImageTexture>>,
    damage_words: BTreeMap<(u16, bool, bool), Gd<ImageTexture>>,
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
        let mut strip = load_image(pack_dir, &set.window.png)?;
        // The window tile art is shared with dialogue, but retail's battle
        // palette line sets colour index 14 (the window fill) to CRAM $0600
        // — pure blue, no green — where the field dialogue line uses $0620.
        // Receipt: oracle/states/battle_command_idle_vdp_25000.json, slot 14
        // of every line. The pack bakes the dialogue palette into the PNG,
        // so the battle chrome remaps that one colour at load.
        remap_color(
            &mut strip,
            Color::from_rgba8(0, 32, 98, 255),
            Color::from_rgba8(0, 0, 98, 255),
        );
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
        let mut menu_image =
            Image::load_from_file(&GString::from(menu_path.as_str())).or_else(|| {
                let path = Path::new(pack_dir)
                    .parent()?
                    .join("generated/gfx/ArtNem_Font.png");
                Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
            })?;
        // Same $0620 → $0600 battle-palette remap as the window strip: the
        // font sheet bakes the dialogue fill behind its glyphs.
        remap_color(
            &mut menu_image,
            Color::from_rgba8(0, 32, 98, 255),
            Color::from_rgba8(0, 0, 98, 255),
        );
        let font = ImageTexture::create_from_image(&menu_image)?;
        let window_words = retail_window_words(&strip)?;
        let damage_words = retail_damage_words()?;
        let glyph_at = retail_glyphs();
        let mut palette = [Color::BLACK; 16];
        for (index, rgb) in set.window.palette.colors.iter().take(16).enumerate() {
            palette[index] = Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255);
        }

        Some(BattleChrome {
            tiles,
            window_words,
            damage_words,
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
        self.frame_excluding(rect, &[])
    }

    /// Draws the retail nine-tile frame, omitting local cells occupied by a
    /// plane-A special tile. The battle status strip is one 36x6 window with
    /// four separator columns punched through it; five overlapping windows
    /// cannot reproduce those decoded cells.
    pub(super) fn frame_excluding(
        &self,
        rect: WindowRect,
        excluded: &[(i32, i32)],
    ) -> Option<Vec<Quad>> {
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
        let is_excluded = |x: i32, y: i32| excluded.iter().any(|&(ex, ey)| ex == x && ey == y);
        let mut push = |name: &'static str, x: i32, y: i32| {
            if is_excluded(x, y) {
                return;
            }
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

    /// A decoded window-plane word, addressed by its retail VRAM pattern.
    /// Battle cursor and status separator/label cells are window art, not
    /// font glyphs, so drawing them from the atlas preserves every pixel and
    /// the plane's flip bits.
    pub(super) fn window_word(
        &self,
        pattern: u16,
        flip_h: bool,
        flip_v: bool,
        dest: Rect2,
    ) -> Option<Quad> {
        Some(Quad {
            texture: self.window_words.get(&(pattern, flip_h, flip_v))?.clone(),
            dest,
            src: Rect2::new(Vector2::ZERO, Vector2::new(self.cell, self.cell)),
        })
    }

    /// A decoded menu-font word, addressed by the second VRAM load's base
    /// (`0x7c0`). This is used for the status-strip colon, whose plane word is
    /// `0x7f3`; regular text still goes through the character map below.
    pub(super) fn font_word(&self, pattern: u16, dest: Rect2) -> Option<Quad> {
        let index = pattern.checked_sub(FONT_BASE_TILE)? as i32;
        let columns = (self.font.get_width() / self.cell as i32).max(1);
        let source = Vector2::new(
            (index % columns) as f32 * self.cell,
            (index / columns) as f32 * self.cell,
        );
        Some(Quad {
            texture: self.font.clone(),
            dest,
            src: Rect2::new(source, Vector2::new(self.cell, self.cell)),
        })
    }

    pub(super) fn text(&self, text: &str, rect: WindowRect) -> Vec<Quad> {
        self.text_with_pitch(text, rect, self.glyph.y)
    }

    /// The battle status routine uses the second, wider number run in
    /// `ArtNem_Font`: source cells 36..45, VRAM patterns `0x7e4..0x7ed`.
    /// It is distinct from the window-charset digit bytes used by ordinary
    /// menu/name strings.
    pub(super) fn battle_number(&self, text: &str, rect: WindowRect) -> Vec<Quad> {
        let mut quads = Vec::new();
        for (col, ch) in text.chars().enumerate() {
            let Some(digit) = ch.to_digit(10) else {
                continue;
            };
            let index = 36 + digit as i32;
            let columns = (self.font.get_width() / self.cell as i32).max(1);
            let source = Vector2::new(
                (index % columns) as f32 * self.cell,
                (index / columns) as f32 * self.cell,
            );
            quads.push(Quad {
                texture: self.font.clone(),
                dest: Rect2::new(
                    Vector2::new(rect.x + col as f32 * self.glyph.x, rect.y),
                    self.glyph,
                ),
                src: Rect2::new(source, self.glyph),
            });
        }
        quads
    }

    /// The victory routine uses the active digit patterns at `0x7da..0x7e3`,
    /// rather than the status strip's widened `0x7e4..0x7ed` run.
    pub(super) fn victory_number(&self, text: &str, rect: WindowRect) -> Vec<Quad> {
        let mut quads = Vec::new();
        for (col, ch) in text.chars().enumerate() {
            let Some(digit) = ch.to_digit(10) else {
                continue;
            };
            let dest = Rect2::new(
                Vector2::new(rect.x + col as f32 * self.glyph.x, rect.y),
                self.glyph,
            );
            if let Some(quad) = self.font_word(0x7da + digit as u16, dest) {
                quads.push(quad);
            }
        }
        quads
    }

    pub(super) fn victory_quads(
        &self,
        frame_rect: WindowRect,
        victory_text_rect: WindowRect,
        rewards: Option<(u16, u16)>,
    ) -> Vec<Quad> {
        let Some(mut quads) = self.frame(frame_rect) else {
            return Vec::new();
        };
        if let Some((experience, meseta)) = rewards {
            quads.extend(self.text(
                "Each got",
                WindowRect {
                    x: 88.0,
                    y: 136.0,
                    w: 64.0,
                    h: 8.0,
                },
            ));
            quads.extend(self.victory_number(
                &experience.to_string(),
                WindowRect {
                    x: 160.0,
                    y: 136.0,
                    w: 8.0,
                    h: 8.0,
                },
            ));
            quads.extend(self.text(
                "EXP",
                WindowRect {
                    x: 176.0,
                    y: 136.0,
                    w: 24.0,
                    h: 8.0,
                },
            ));
            quads.extend(self.victory_number(
                &meseta.to_string(),
                WindowRect {
                    x: 88.0,
                    y: 152.0,
                    w: 8.0,
                    h: 8.0,
                },
            ));
            quads.extend(self.text(
                "meseta!",
                WindowRect {
                    x: 104.0,
                    y: 152.0,
                    w: 56.0,
                    h: 8.0,
                },
            ));
        } else {
            quads.extend(self.text("Victory!", victory_text_rect));
        }
        quads
    }

    /// `Fighter_OpenDamageWindow` writes a five-by-two tile block from the
    /// retail `$5e4` damage art. The first-battle captures exercise 0, 1, 2,
    /// 5, 6, and 11; unknown glyph art falls back to a readable number until
    /// another oracle capture supplies those source tiles.
    pub(super) fn damage_quads(&self, amount: u16, rect: WindowRect) -> Vec<Quad> {
        let value = amount.min(999);
        let mut top = [(0x5e5, false, true); 3];
        let mut bottom = [(0x5e5, false, false); 3];
        if value >= 100 {
            (top[0], bottom[0]) = damage_digit(value / 100 % 10);
        }
        if value >= 10 {
            (top[1], bottom[1]) = damage_digit(value / 10 % 10);
        }
        (top[2], bottom[2]) = damage_digit(value % 10);

        let cells = [
            (0x5e4, false, false),
            top[0],
            top[1],
            top[2],
            (0x5e4, true, false),
            (0x5e4, false, true),
            bottom[0],
            bottom[1],
            bottom[2],
            (0x5e4, true, true),
        ];
        if cells
            .iter()
            .any(|cell| !self.damage_words.contains_key(cell))
        {
            let mut fallback = self.frame(rect).unwrap_or_default();
            fallback.extend(self.text(
                &value.to_string(),
                WindowRect {
                    x: rect.x + self.cell,
                    y: rect.y + 4.0,
                    w: rect.w - self.cell * 2.0,
                    h: self.cell,
                },
            ));
            return fallback;
        }

        cells
            .into_iter()
            .enumerate()
            .filter_map(|(index, cell)| {
                let texture = self.damage_words.get(&cell)?.clone();
                let x = index as i32 % 5;
                let y = index as i32 / 5;
                Some(Quad {
                    texture,
                    dest: Rect2::new(
                        Vector2::new(rect.x + x as f32 * self.cell, rect.y + y as f32 * self.cell),
                        Vector2::new(self.cell, self.cell),
                    ),
                    src: Rect2::new(Vector2::ZERO, Vector2::new(self.cell, self.cell)),
                })
            })
            .collect()
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
            let dest = Rect2::new(
                Vector2::new(
                    rect.x + col as f32 * self.glyph.x,
                    rect.y + row as f32 * line_pitch,
                ),
                self.glyph,
            );
            // `oracle/layouts/battle_command_idle.json` decodes the space in
            // `ZORAN BULT` as window pattern 0x680, not font cell 0 (A).
            if ch == ' ' {
                if let Some(quad) = self.window_word(0x680, false, false, dest) {
                    quads.push(quad);
                }
                col += 1;
                continue;
            }
            let Some(source) = self.glyph_at.get(&ch).or_else(|| self.glyph_at.get(&'?')) else {
                col += 1;
                continue;
            };
            quads.push(Quad {
                texture: self.font.clone(),
                dest,
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
        // The window charset stores digits at byte values 27..36; the PNG
        // starts at the font's loaded tile 0x681, so source cells are 26..35.
        let index = index + 26;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
        );
    }
    for (index, ch) in ('a'..='z').enumerate() {
        let index = index + 56;
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
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
            Vector2::new((index % 16) as f32 * 8.0, (index / 16) as f32 * 8.0),
        );
    }
    glyphs
}

fn retail_window_words(strip: &Gd<Image>) -> Option<BTreeMap<(u16, bool, bool), Gd<ImageTexture>>> {
    // These are the non-role window patterns observed in the decoded battle
    // planes: the 0x680 space/fill, selected/disabled cursor, separator
    // top/middle/bottom, and the HP/TP label glyphs. The source PNG is the
    // `0x680` window-tile load.
    const PATTERNS: [u16; 8] = [0x680, 0x6e7, 0x6e8, 0x6f4, 0x6f5, 0x6f8, 0x6f9, 0x6fa];
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

fn damage_digit(digit: u16) -> ((u16, bool, bool), (u16, bool, bool)) {
    match digit {
        0 => ((0x5e6, false, false), (0x5e6, false, true)),
        1 => ((0x5e7, false, false), (0x5e8, false, false)),
        2 => ((0x5e9, false, false), (0x5ea, false, false)),
        3 => ((0x5eb, false, false), (0x5e9, false, true)),
        4 => ((0x5ec, false, false), (0x5ed, false, false)),
        5 => ((0x5ee, false, false), (0x5e9, false, true)),
        6 => ((0x5ef, false, false), (0x5e6, false, true)),
        7 => ((0x5f0, false, false), (0x5f1, false, false)),
        8 => ((0x5f2, false, false), (0x5e6, false, true)),
        9 => ((0x5e6, true, false), (0x5ef, true, true)),
        _ => ((0x5e5, false, true), (0x5e5, false, false)),
    }
}

fn retail_damage_words() -> Option<BTreeMap<(u16, bool, bool), Gd<ImageTexture>>> {
    let patterns = [
        0x5e4, 0x5e5, 0x5e6, 0x5e7, 0x5e8, 0x5e9, 0x5ea, 0x5ee, 0x5ef,
    ];
    let mut words = BTreeMap::new();
    for pattern in patterns {
        let rows = damage_pattern(pattern)?;
        for flip_h in [false, true] {
            for flip_v in [false, true] {
                let mut bytes = Vec::with_capacity(8 * 8 * 4);
                for y in 0..8 {
                    for x in 0..8 {
                        let row = rows[if flip_v { 7 - y } else { y }].as_bytes();
                        let symbol = row[if flip_h { 7 - x } else { x }];
                        bytes.extend_from_slice(match symbol {
                            b'B' => &[0, 0, 98, 255],
                            b'G' => &[172, 170, 172, 255],
                            b'W' => &[238, 238, 238, 255],
                            _ => return None,
                        });
                    }
                }
                let image = Image::create_from_data(
                    8,
                    8,
                    false,
                    Format::RGBA8,
                    &PackedByteArray::from(bytes),
                )?;
                words.insert(
                    (pattern, flip_h, flip_v),
                    ImageTexture::create_from_image(&image)?,
                );
            }
        }
    }
    Some(words)
}

fn damage_pattern(pattern: u16) -> Option<[&'static str; 8]> {
    Some(match pattern {
        0x5e4 => [
            "BBBBBBBB", "BBGGGGGG", "BGGBBBBB", "BGGBBBBB", "BGGBBBBB", "BGGBBBBB", "BGGBBBBB",
            "BGGBBBBB",
        ],
        0x5e5 => [
            "BBBBBBBB", "BBBBBBBB", "BBBBBBBB", "BBBBBBBB", "BBBBBBBB", "BBBBBBBB", "GGGGGGGG",
            "BBBBBBBB",
        ],
        0x5e6 => [
            "BBBBBBBB", "GGGGGGGG", "BBBBBBBB", "BBBBBBBB", "BBWWWWBB", "BWWBBWWB", "BWWBBWWB",
            "BWWBBWWB",
        ],
        0x5e7 => [
            "BBBBBBBB", "GGGGGGGG", "BBBBBBBB", "BBBBBBBB", "BBBWWBBB", "BBWWWBBB", "BBBWWBBB",
            "BBBWWBBB",
        ],
        0x5e8 => [
            "BBBWWBBB", "BBBWWBBB", "BBBWWBBB", "BBWWWWBB", "BBBBBBBB", "BBBBBBBB", "GGGGGGGG",
            "BBBBBBBB",
        ],
        0x5e9 => [
            "BBBBBBBB", "GGGGGGGG", "BBBBBBBB", "BBBBBBBB", "BBWWWWBB", "BWWBBWWB", "BWWBBWWB",
            "BBBBBWWB",
        ],
        0x5ea => [
            "BBBWWWBB", "BBWWWBBB", "BWWBBBBB", "BWWWWWWB", "BBBBBBBB", "BBBBBBBB", "GGGGGGGG",
            "BBBBBBBB",
        ],
        0x5ee => [
            "BBBBBBBB", "GGGGGGGG", "BBBBBBBB", "BBBBBBBB", "BWWWWWWB", "BWWBBBBB", "BWWWWWBB",
            "BWWBBWWB",
        ],
        0x5ef => [
            "BBBBBBBB", "GGGGGGGG", "BBBBBBBB", "BBBBBBBB", "BBWWWWBB", "BWWBBWWB", "BWWBBBBB",
            "BWWWWWBB",
        ],
        _ => return None,
    })
}

fn load_image(pack_dir: &str, name: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{name}");
    Image::load_from_file(&GString::from(path.as_str()))
}

/// Replaces every exactly-matching opaque pixel of `from` with `to` — the
/// baked-palette equivalent of pointing a tile at a different CRAM line.
fn remap_color(image: &mut Gd<Image>, from: Color, to: Color) {
    let (width, height) = (image.get_width(), image.get_height());
    for y in 0..height {
        for x in 0..width {
            if image.get_pixel(x, y) == from {
                image.set_pixel(x, y, to);
            }
        }
    }
}
