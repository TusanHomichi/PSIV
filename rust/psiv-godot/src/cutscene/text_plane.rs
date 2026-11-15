//! RunText2's plane cells and character patterns, independent of Godot.
//! US `$06AAF2..$06AC98`: clear the preceding row, map 32 pairs of
//! tiles, upload monochrome glyphs at d3's tile index. Plane cells become
//! visible at DMA; patterns are VRAM, so reusing a tile replaces its art.

use psiv_core::{PresentationOp, SceneOp};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextCell {
    pub(crate) ch: char,
    pub(crate) half: u8,
    pub(crate) line: u8,
}

pub(crate) struct TextPlane {
    staged: [u16; 2048],
    visible: [u16; 2048],
    patterns: BTreeMap<u16, (char, u8)>,
}

impl Default for TextPlane {
    fn default() -> Self {
        Self {
            staged: [0; 2048],
            visible: [0; 2048],
            patterns: BTreeMap::new(),
        }
    }
}

impl TextPlane {
    pub(crate) fn draw(&mut self, plane: u32, tile: u32, text: &str) -> Result<(), &'static str> {
        if !(0xFFFF_8000..0xFFFF_9000).contains(&plane) || plane & 1 != 0 || tile > 0xFFFF {
            return Err("RunText2 needs an aligned plane-A buffer address and a tile word");
        }
        let origin = (plane as usize & 0xFFF) / 2;
        let row = origin / 64;
        let column = origin % 64;
        let cell = |dy: usize, dx: usize| (((row + dy) % 32) * 64) + (column + dx) % 64;
        // loc_6AAF2 clears 32 cells one row above, wrapping within plane A.
        for x in 0..32 {
            self.staged[cell(31, x)] = 0;
            for half in 0..2 {
                self.staged[cell(half, x)] = (tile as u16).wrapping_add((x * 2 + half) as u16);
            }
        }
        let base = tile as u16 & 0x7FF;
        // loc_6AA86 blanks the 64-pattern text allocation before its upload.
        for offset in 0..64 {
            self.patterns.remove(&((base + offset) & 0x7FF));
        }
        for (x, ch) in text.chars().take(32).enumerate() {
            for half in 0..2 {
                self.patterns
                    .insert((base + (x * 2 + half) as u16) & 0x7FF, (ch, half as u8));
            }
        }
        Ok(())
    }

    pub(crate) fn consume(&mut self, op: &SceneOp) {
        match op {
            SceneOp::DmaPlanes => self.visible = self.staged,
            SceneOp::Presentation {
                op: PresentationOp::ClearPlanes,
            } => self.staged.fill(0),
            SceneOp::InitVramAndCram
            | SceneOp::LoadMap { .. }
            | SceneOp::Presentation {
                op: PresentationOp::ReloadMapChunks,
            } => self.clear(),
            _ => {}
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn cell(&self, x: usize, y: usize) -> Option<TextCell> {
        let word = self.visible[(y % 32) * 64 + x % 64];
        let &(ch, half) = self.patterns.get(&(word & 0x7FF))?;
        Some(TextCell {
            ch,
            half,
            line: ((word >> 13) & 3) as u8,
        })
    }

    pub(crate) fn is_visible(&self) -> bool {
        (0..28).any(|y| (0..40).any(|x| self.cell(x, y).is_some_and(|c| c.ch != ' ')))
    }
}
