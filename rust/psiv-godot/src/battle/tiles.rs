//! Plane-A words drawn from the command art: the icons and the cursor sprite.
//!
//! The window and font art come from the pack's dialogue PNGs
//! ([`super::chrome`]). The command icons and the cursor sprite are the two
//! Nemesis streams `loc_7382` loads (`ps4.asm:10975`), which the extractor
//! writes to `battle/art/command_ui.json` and `psiv-data` parses
//! ([`psiv_data::CommandUiArt`]). Every plane word that names one of those tiles
//! draws through here: tile index from the low eleven bits, flips from bits 11
//! and 12, and the palette line from bits 13 and 14.
//!
//! Only CRAM line 3 exists for this art: `Battle_DrawCommandIcons` writes the
//! icon words verbatim (`$E1xx`, `ps4.asm:11149`), and the cursor object's
//! tile properties are `$2F80` (`ps4.asm:70478`), tile `$17C` with the
//! window palette; the sprite table of every command-strip dump reads
//! palette 3. The line's colors are the table at `loc_76A4`
//! (`ps4.asm:11229`), copied to indices 1-15 by `loc_7634`.

use std::collections::BTreeMap;

use godot::classes::{Image, ImageTexture, image::Format};
use godot::prelude::*;

use psiv_data::CommandUiArt;

use super::art::cram_color;
use super::chrome::Quad;

/// VRAM tile of the first cursor sprite tile (`ps4.asm:10985`).
pub(super) const CURSOR_FIRST_TILE: u16 = 0x17C;

const FLIP_H: u16 = 0x0800;
const FLIP_V: u16 = 0x1000;

/// The command art's textures, built on first use.
#[derive(Default)]
pub(super) struct CommandTiles {
    art: Option<CommandUiArt>,
    cache: BTreeMap<(u16, bool, bool), Gd<ImageTexture>>,
}

impl CommandTiles {
    /// The tiles of a pack's `battle/art/command_ui.json`.
    pub(super) fn new(art: CommandUiArt) -> CommandTiles {
        CommandTiles {
            art: Some(art),
            cache: BTreeMap::new(),
        }
    }

    /// The quad for one plane word that names a command-art tile, or `None`
    /// when the word is outside that art.
    pub(super) fn word(&mut self, word: u16, dest: Rect2) -> Option<Quad> {
        let tile = word & 0x07FF;
        let (flip_h, flip_v) = (word & FLIP_H != 0, word & FLIP_V != 0);
        let texture = self.texture(tile, flip_h, flip_v)?;
        Some(Quad {
            texture,
            dest,
            src: Rect2::new(Vector2::ZERO, Vector2::new(8.0, 8.0)),
        })
    }

    /// The cursor sprite's four quads. A sprite's tiles run down each column
    /// (`size $05` is two columns of two), so tile `n+1` is under tile `n`.
    pub(super) fn sprite_2x2(&mut self, first_tile: u16, x: i32, y: i32) -> Vec<Quad> {
        let mut quads = Vec::new();
        for (index, (dx, dy)) in [(0, 0), (0, 8), (8, 0), (8, 8)].into_iter().enumerate() {
            let dest = Rect2::new(
                Vector2::new((x + dx) as f32, (y + dy) as f32),
                Vector2::new(8.0, 8.0),
            );
            if let Some(quad) = self.word(first_tile + index as u16, dest) {
                quads.push(quad);
            }
        }
        quads
    }

    fn texture(&mut self, tile: u16, flip_h: bool, flip_v: bool) -> Option<Gd<ImageTexture>> {
        if let Some(texture) = self.cache.get(&(tile, flip_h, flip_v)) {
            return Some(texture.clone());
        }
        let art = self.art.as_ref()?;
        let rows = art.tile(tile)?;
        let mut bytes = Vec::with_capacity(8 * 8 * 4);
        for y in 0..8 {
            let row = rows[if flip_v { 7 - y } else { y }];
            for x in 0..8 {
                let index = usize::from(row[if flip_h { 7 - x } else { x }]);
                if index == 0 {
                    bytes.extend_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                let color = cram_color(*art.palette_line_3.get(index - 1)?);
                bytes.extend_from_slice(&[color.r8(), color.g8(), color.b8(), 255]);
            }
        }
        let image =
            Image::create_from_data(8, 8, false, Format::RGBA8, &PackedByteArray::from(bytes))?;
        let texture = ImageTexture::create_from_image(&image)?;
        self.cache.insert((tile, flip_h, flip_v), texture.clone());
        Some(texture)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psiv_data::CommandUiArt;

    /// The pack's command art is what the cartridge's VRAM holds: every tile
    /// of `battle/art/command_ui.json` equals the 32 bytes the oracle's VDP
    /// dump has at that tile (`oracle/states/battle_command_idle_vdp_25000.json`,
    /// tape 07 frame 25000, taken with the battle loaded). A missing local
    /// pack or oracle state skips; an existing but stale pack fails.
    #[test]
    fn command_art_matches_the_oracle_vram() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pack = std::env::var_os("PSIV_RUNTIME_PACK")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("runtime-pack"));
        if !pack.exists() {
            eprintln!("runtime pack not present at {}; skipping", pack.display());
            return;
        }
        let art = CommandUiArt::load(&pack).expect("existing command art pack must load");
        let state = root.join("oracle/states/battle_command_idle_vdp_25000.json");
        let Ok(text) = std::fs::read_to_string(&state) else {
            eprintln!("oracle state not present; skipping");
            return;
        };
        let json: serde_json::Value = serde_json::from_str(&text).expect("state parses");
        let hex = json["regions"]["vdp_vram"]["bytes_hex"]
            .as_str()
            .expect("a VRAM receipt");
        let vram: Vec<u8> = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex"))
            .collect();
        assert_eq!(art.tile_count, 38);
        let cram = json["regions"]["cram"]["bytes_hex"]
            .as_str()
            .expect("a CRAM receipt");
        let cram: Vec<u8> = (0..cram.len() / 2)
            .map(|i| u8::from_str_radix(&cram[i * 2..i * 2 + 2], 16).expect("hex"))
            .collect();
        assert_eq!(art.palette_line_3.len(), 15);
        for (index, word) in art.palette_line_3.iter().enumerate() {
            let at = 0x62 + index * 2;
            assert_eq!(*word, u16::from_be_bytes([cram[at], cram[at + 1]]));
        }
        for tile in &art.tiles {
            let pixels = tile.pixels().expect("well-formed tile");
            let at = usize::from(tile.vram_tile) * 32;
            for (y, row) in pixels.iter().enumerate() {
                for (pair, indices) in row.chunks(2).enumerate() {
                    assert_eq!(
                        vram[at + y * 4 + pair],
                        indices[0] << 4 | indices[1],
                        "tile ${:X} row {y}",
                        tile.vram_tile
                    );
                }
            }
        }
    }
}
