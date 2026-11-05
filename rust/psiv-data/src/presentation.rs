//! The palette words scenes read from the ROM, from the pack's
//! `presentation/panels.json`: its `palettes` records (address, word count and
//! the big-endian words as hex) and, for each panel record, the line and ROM
//! palette `Panel_Draw` copies for each of its two layers (`ps4.asm:144392`:
//! `move.b (Panel_Art_Tile_1).w, d0` / `andi.w #$70, d0` / sixteen words of
//! `Panel_Palette_Addr_1` to `Palette_Table_Buffer + d0`).
//!
//! The words are cartridge data; they live in the pack, never in source. The
//! runtime's CRAM model reads them through [`GameData::scene_palettes`].
//!
//! [`GameData::scene_palettes`]: crate::GameData::scene_palettes

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::DataError;

/// The pack file the scene palettes come from.
pub const PRESENTATION_FILE: &str = "presentation/panels.json";

/// One layer's palette upload in `Panel_Draw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelPalette {
    /// The layer's `Panel_Art_Tile` word. Its high byte `& $70` is the byte
    /// offset into `Palette_Table_Buffer`.
    pub art_tile: u16,
    /// The ROM palette copied: sixteen words (`move.w #$F, d7` / `trap #1`).
    pub rom_addr: u32,
}

impl PanelPalette {
    /// The first `Palette_Table_Buffer` word the layer's copy writes:
    /// `(art_tile >> 8) & $70` bytes, halved.
    #[must_use]
    pub const fn start(&self) -> usize {
        (((self.art_tile >> 8) & 0x70) / 2) as usize
    }
}

/// The ROM palettes and panel palette uploads a scene can make.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScenePalettes {
    words: BTreeMap<u32, Vec<u16>>,
    panels: BTreeMap<u16, Vec<PanelPalette>>,
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    palettes: Vec<PaletteRecord>,
    #[serde(default)]
    panels: Vec<PanelRecord>,
}

#[derive(Deserialize)]
struct PaletteRecord {
    rom_addr: String,
    words: usize,
    raw_hex: String,
}

#[derive(Deserialize)]
struct PanelRecord {
    id: u16,
    art_tile_a: u16,
    palette_a: Option<String>,
    art_tile_b: u16,
    palette_b: Option<String>,
}

fn address(text: &str) -> Option<u32> {
    u32::from_str_radix(text.strip_prefix("0x")?, 16).ok()
}

fn decode(hex: &str) -> Option<Vec<u16>> {
    if !hex.len().is_multiple_of(4) {
        return None;
    }
    (0..hex.len() / 4)
        .map(|i| u16::from_str_radix(hex.get(i * 4..i * 4 + 4)?, 16).ok())
        .collect()
}

impl ScenePalettes {
    /// Reads [`PRESENTATION_FILE`] under `pack_dir`. Every palette record must
    /// decode to its declared word count, and every panel layer the cartridge
    /// draws (art tile not `$FFFF`) must name a palette the file carries.
    pub(crate) fn load(pack_dir: &Path) -> Result<ScenePalettes, DataError> {
        let path = pack_dir.join(PRESENTATION_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let file: File = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        let fail = |message: String| DataError::Presentation {
            path: path.clone(),
            message,
        };
        let mut words = BTreeMap::new();
        for record in file.palettes {
            let at = address(&record.rom_addr)
                .ok_or_else(|| fail(format!("palette address {:?}", record.rom_addr)))?;
            let decoded = decode(&record.raw_hex)
                .filter(|decoded| decoded.len() == record.words)
                .ok_or_else(|| {
                    fail(format!(
                        "palette {at:#08x} does not decode to {} words",
                        record.words
                    ))
                })?;
            words.insert(at, decoded);
        }
        let mut panels = BTreeMap::new();
        for panel in file.panels {
            let mut layers = Vec::new();
            for (art_tile, palette) in [
                (panel.art_tile_a, &panel.palette_a),
                (panel.art_tile_b, &panel.palette_b),
            ] {
                if art_tile == 0xFFFF {
                    continue;
                }
                let rom_addr = palette.as_deref().and_then(address).ok_or_else(|| {
                    fail(format!("panel {:#05x} names no palette address", panel.id))
                })?;
                if words.get(&rom_addr).is_none_or(|w: &Vec<u16>| w.len() < 16) {
                    return Err(fail(format!(
                        "panel {:#05x} palette {rom_addr:#08x} has no sixteen-word record",
                        panel.id
                    )));
                }
                layers.push(PanelPalette { art_tile, rom_addr });
            }
            panels.insert(panel.id, layers);
        }
        Ok(ScenePalettes { words, panels })
    }

    /// Builds the table from parts, for tests and synthetic packs.
    #[must_use]
    pub fn from_parts(
        words: BTreeMap<u32, Vec<u16>>,
        panels: BTreeMap<u16, Vec<PanelPalette>>,
    ) -> ScenePalettes {
        ScenePalettes { words, panels }
    }

    /// The words the pack carries for the ROM palette at `rom_addr`.
    #[must_use]
    pub fn words(&self, rom_addr: u32) -> Option<&[u16]> {
        self.words.get(&rom_addr).map(Vec::as_slice)
    }

    /// Every ROM palette the pack carries, by address.
    pub fn all(&self) -> impl Iterator<Item = (u32, &[u16])> {
        self.words.iter().map(|(&at, words)| (at, words.as_slice()))
    }

    /// The palette uploads `Panel_Create` makes for panel `id`, layer A
    /// first; `None` for a panel the pack does not carry.
    #[must_use]
    pub fn panel(&self, id: u16) -> Option<&[PanelPalette]> {
        self.panels.get(&id).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_line_is_the_art_tiles_high_byte_masked_with_70() {
        // $8100 -> high byte $81 & $70 = 0; $A100 -> $20 bytes, word 16;
        // $E000 -> $60 bytes, word 48; $3000 -> $30 bytes, word 24 (the
        // vertical-flip bit is in the mask, as the cartridge has it).
        let at = |art_tile| PanelPalette {
            art_tile,
            rom_addr: 0,
        }
        .start();
        assert_eq!(at(0x8100), 0);
        assert_eq!(at(0xA100), 16);
        assert_eq!(at(0xE000), 48);
        assert_eq!(at(0x3000), 24);
    }

    #[test]
    fn hex_decodes_big_endian_words() {
        assert_eq!(decode("0eee0000"), Some(vec![0x0EEE, 0]));
        assert_eq!(decode("0eee0"), None);
        assert_eq!(address("0x296300"), Some(0x29_6300));
        assert_eq!(address("296300"), None);
    }
}
