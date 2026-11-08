//! The retail window font's glyph positions on `dialogue/menu_font.png`, the
//! one map the camp, shop and battle windows share.

use std::collections::BTreeMap;

use godot::prelude::Vector2;

/// One glyph cell on the sheet.
const CELL: f32 = 8.0;

/// Where each character's glyph sits on the window-font sheet.
pub(crate) fn retail_glyphs() -> BTreeMap<char, Vector2> {
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
    // The sheet is the window font from VRAM tile `$681`, so a glyph's index
    // is its window-charset byte less one (`psiv_tools/text.py`, `WINDOW`):
    // `-` `$31`, `!` `$32`, `?` `$33`, `:` `$34`, `.` `$53`, `'` `$54`,
    // `,` `$55` (#128: `.` and `,` were read from two other glyphs).
    for (index, ch) in [
        (48, '-'),
        (49, '!'),
        (50, '?'),
        (51, ':'),
        (54, '<'),
        (55, '>'),
        (82, '.'),
        (83, '\''),
        (84, ','),
    ] {
        glyphs.insert(
            ch,
            Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
        );
    }
    glyphs
}

#[cfg(test)]
mod glyph_tests {
    use super::{CELL, retail_glyphs};

    /// A glyph's place on the sheet from its window-charset byte.
    fn at(byte: u8) -> (f32, f32) {
        let index = f32::from(byte - 1);
        ((index % 16.0) * CELL, (index / 16.0).floor() * CELL)
    }

    #[test]
    fn punctuation_sits_where_the_window_charset_puts_it() {
        let glyphs = retail_glyphs();
        for (ch, byte) in [
            ('-', 0x31),
            ('!', 0x32),
            ('?', 0x33),
            (':', 0x34),
            ('.', 0x53),
            ('\'', 0x54),
            (',', 0x55),
        ] {
            let glyph = glyphs[&ch];
            assert_eq!((glyph.x, glyph.y), at(byte), "{ch:?}");
        }
        assert_eq!((glyphs[&'a'].x, glyphs[&'a'].y), at(0x39));
        assert_eq!((glyphs[&'z'].x, glyphs[&'z'].y), at(0x52));
    }
}
