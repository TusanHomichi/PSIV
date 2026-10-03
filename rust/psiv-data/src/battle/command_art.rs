//! `battle/art/command_ui.json`: the battle's command icon and cursor tiles.
//!
//! `loc_7382` (`ps4.asm:10975`) decompresses two Nemesis streams into VRAM as a
//! battle loads: 30 icon tiles at ROM `$27D860` to tile `$15E` and 8 cursor
//! sprite tiles at ROM `$27DA70` to tile `$17C`. The pack carries each tile as
//! eight rows of eight CRAM indices. `loc_7634` copies `loc_76A4` to CRAM line
//! 3, indices 1-15; that palette lives in the same pack file. The plane word
//! or sprite attribute that draws a tile picks the palette line.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::DataError;
use crate::manifest::PACK_FORMAT_VERSION;

/// Where the file sits inside a pack.
pub const COMMAND_UI_ART_PATH: &str = "battle/art/command_ui.json";
const FIRST_TILE: u16 = 0x15E;
const LAST_TILE: u16 = 0x183;
const REQUIRED_TILES: usize = (LAST_TILE - FIRST_TILE + 1) as usize;
const PALETTE_WORDS: usize = 15;

/// One 8x8 pattern, keyed by the VRAM tile the cartridge loads it to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandUiTile {
    /// The VRAM tile index (`$15E..$17B` icons, `$17C..$183` cursor).
    pub vram_tile: u16,
    /// Eight rows of eight hex digits, one CRAM index per pixel.
    pub rows: Vec<String>,
}

/// The parsed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandUiArt {
    /// The pack format that wrote it.
    pub format_version: u32,
    /// The declared tile count.
    pub tile_count: usize,
    /// CRAM line 3, indices 1-15, copied from `loc_76A4` by `loc_7634`.
    pub palette_line_3: Vec<u16>,
    /// The tiles, in VRAM order.
    pub tiles: Vec<CommandUiTile>,
}

impl CommandUiArt {
    /// Reads and validates `battle/art/command_ui.json` under `pack_dir`.
    ///
    /// # Errors
    /// [`DataError`] for an unreadable or malformed file, a format-version
    /// mismatch, a missing required VRAM tile, an invalid palette word, or a
    /// row that is not eight hex digits.
    pub fn load(pack_dir: &Path) -> Result<CommandUiArt, DataError> {
        let path = pack_dir.join(COMMAND_UI_ART_PATH);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let art: CommandUiArt =
            serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        if art.format_version != PACK_FORMAT_VERSION {
            return Err(DataError::FormatVersion {
                found: art.format_version,
                expected: PACK_FORMAT_VERSION,
            });
        }
        art.validate(&path)?;
        Ok(art)
    }

    fn validate(&self, path: &Path) -> Result<(), DataError> {
        let invalid =
            |field: &'static str, manifest: String, record: String| DataError::ManifestMismatch {
                path: path.to_path_buf(),
                field,
                manifest,
                record,
            };
        if self.tile_count != REQUIRED_TILES || self.tiles.len() != REQUIRED_TILES {
            return Err(invalid(
                "tile_count",
                REQUIRED_TILES.to_string(),
                format!("declared {}, actual {}", self.tile_count, self.tiles.len()),
            ));
        }
        if self.palette_line_3.len() != PALETTE_WORDS {
            return Err(invalid(
                "palette_line_3",
                format!("{PALETTE_WORDS} CRAM words for indices 1-15"),
                self.palette_line_3.len().to_string(),
            ));
        }
        for (index, word) in self.palette_line_3.iter().enumerate() {
            if word & !0x0EEE != 0 {
                return Err(invalid(
                    "palette_line_3",
                    "a 12-bit Mega Drive CRAM word".into(),
                    format!("index {} has ${word:04X}", index + 1),
                ));
            }
        }
        // `loc_76A4`'s last word is the visible window ink used by the icon
        // art. A zeroed palette is a partial pack even if its length is right.
        if self.palette_line_3[14] == 0 {
            return Err(invalid(
                "palette_line_3",
                "nonzero CRAM index 15 ink".into(),
                "index 15 is zero".into(),
            ));
        }
        let mut seen = BTreeSet::new();
        for tile in &self.tiles {
            if !(FIRST_TILE..=LAST_TILE).contains(&tile.vram_tile) {
                return Err(invalid(
                    "tiles",
                    format!("every VRAM tile ${FIRST_TILE:X}..${LAST_TILE:X}"),
                    format!("out-of-range ${:X}", tile.vram_tile),
                ));
            }
            if !seen.insert(tile.vram_tile) {
                return Err(invalid(
                    "tiles",
                    "unique vram_tile".into(),
                    format!("${:X} appears twice", tile.vram_tile),
                ));
            }
            if tile.pixels().is_none() {
                return Err(invalid(
                    "tiles",
                    "eight rows of eight hex digits".into(),
                    format!("${:X}", tile.vram_tile),
                ));
            }
        }
        if let Some(missing) = (FIRST_TILE..=LAST_TILE).find(|tile| !seen.contains(tile)) {
            return Err(invalid(
                "tiles",
                format!("every VRAM tile ${FIRST_TILE:X}..${LAST_TILE:X}"),
                format!("missing ${missing:X}"),
            ));
        }
        Ok(())
    }

    /// The pattern the cartridge loads to `vram_tile`, as CRAM indices.
    #[must_use]
    pub fn tile(&self, vram_tile: u16) -> Option<[[u8; 8]; 8]> {
        self.tiles
            .iter()
            .find(|tile| tile.vram_tile == vram_tile)
            .and_then(CommandUiTile::pixels)
    }
}

impl CommandUiTile {
    /// The rows as CRAM indices, or `None` for a malformed row.
    #[must_use]
    pub fn pixels(&self) -> Option<[[u8; 8]; 8]> {
        if self.rows.len() != 8 {
            return None;
        }
        let mut out = [[0u8; 8]; 8];
        for (y, row) in self.rows.iter().enumerate() {
            let digits: Vec<u8> = row
                .chars()
                .map(|c| c.to_digit(16).map(|d| d as u8))
                .collect::<Option<_>>()?;
            out[y] = digits.try_into().ok()?;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempArt {
        dir: std::path::PathBuf,
    }

    impl TempArt {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "psiv-command-ui-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(dir.join("battle/art")).expect("temporary art directory");
            Self { dir }
        }

        fn load(&self, art: &CommandUiArt) -> Result<CommandUiArt, DataError> {
            std::fs::write(
                self.dir.join(COMMAND_UI_ART_PATH),
                serde_json::to_vec(art).expect("serialize command art"),
            )
            .expect("write command art");
            CommandUiArt::load(&self.dir)
        }
    }

    impl Drop for TempArt {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn complete_art() -> CommandUiArt {
        CommandUiArt {
            format_version: PACK_FORMAT_VERSION,
            tile_count: REQUIRED_TILES,
            palette_line_3: {
                let mut words = vec![0; PALETTE_WORDS];
                words[14] = 0x0002;
                words
            },
            tiles: (FIRST_TILE..=LAST_TILE)
                .map(|vram_tile| CommandUiTile {
                    vram_tile,
                    rows: vec!["00000000".into(); 8],
                })
                .collect(),
        }
    }

    fn assert_rejected(pack: &TempArt, art: &CommandUiArt, field: &str) {
        let error = pack.load(art).expect_err("corrupt art must not load");
        assert!(error.to_string().contains(field), "{error}");
    }

    fn tile(rows: &[&str]) -> CommandUiTile {
        CommandUiTile {
            vram_tile: 0x15E,
            rows: rows.iter().map(|row| (*row).to_string()).collect(),
        }
    }

    #[test]
    fn a_tile_reads_as_cram_indices() {
        let rows = ["01234567"; 8];
        assert_eq!(tile(&rows).pixels().unwrap()[3], [0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn malformed_rows_are_refused() {
        assert!(tile(&["01234567"; 7]).pixels().is_none(), "seven rows");
        let mut rows = ["01234567"; 8];
        rows[2] = "0123456";
        assert!(tile(&rows).pixels().is_none(), "a short row");
        rows[2] = "0123456G";
        assert!(tile(&rows).pixels().is_none(), "a non-hex digit");
    }

    #[test]
    fn public_load_requires_every_vram_tile_and_the_palette() {
        let pack = TempArt::new();
        let valid = complete_art();
        assert_eq!(pack.load(&valid).unwrap(), valid);

        let mut art = valid.clone();
        art.tile_count = 0;
        art.tiles.clear();
        assert_rejected(&pack, &art, "tile_count");

        let mut art = valid.clone();
        art.tiles.remove(12);
        art.tile_count = art.tiles.len();
        assert_rejected(&pack, &art, "tile_count");

        let mut art = valid.clone();
        art.tiles[12].vram_tile = FIRST_TILE;
        assert_rejected(&pack, &art, "tiles");

        let mut art = valid.clone();
        art.tiles[12].vram_tile = LAST_TILE + 1;
        assert_rejected(&pack, &art, "tiles");

        let mut art = valid.clone();
        art.palette_line_3.pop();
        assert_rejected(&pack, &art, "palette_line_3");

        let mut art = valid.clone();
        art.palette_line_3[0] = 0x0EEF;
        assert_rejected(&pack, &art, "palette_line_3");

        let mut art = valid.clone();
        art.palette_line_3[14] = 0;
        assert_rejected(&pack, &art, "palette_line_3");

        let mut art = valid;
        art.tiles[12].rows.pop();
        assert_rejected(&pack, &art, "tiles");
    }

    #[test]
    fn public_load_names_a_missing_file() {
        let pack = TempArt::new();
        let error = CommandUiArt::load(&pack.dir).expect_err("missing art must not load");
        assert!(error.to_string().contains(COMMAND_UI_ART_PATH), "{error}");
    }
}
