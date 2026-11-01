//! `battle/art/damage_digits.json`: the tiles the battle draws damage numbers from.
//!
//! The battle setup decompresses one Nemesis stream, ROM `$27D778`, to VRAM
//! `$BC80` (tile `$5E4`, `ps4.asm:10007-10009`): 28 patterns. The pack carries
//! each as eight rows of eight CRAM indices, the shape
//! [`super::CommandUiArt`]'s tiles have. The plane word that draws a tile picks
//! its palette line.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::CommandUiTile;
use super::command_art::validate_tiles;
use crate::error::DataError;
use crate::manifest::PACK_FORMAT_VERSION;

/// Where the file sits inside a pack.
pub const DAMAGE_DIGIT_ART_PATH: &str = "battle/art/damage_digits.json";
const FIRST_TILE: u16 = 0x5E4;
const LAST_TILE: u16 = 0x5FF;
const REQUIRED_TILES: usize = (LAST_TILE - FIRST_TILE + 1) as usize;

/// The parsed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageDigitArt {
    /// The pack format that wrote it.
    pub format_version: u32,
    /// The declared tile count.
    pub tile_count: usize,
    /// The tiles, in VRAM order.
    pub tiles: Vec<CommandUiTile>,
}

impl DamageDigitArt {
    /// Reads and validates `battle/art/damage_digits.json` under `pack_dir`.
    ///
    /// # Errors
    /// [`DataError`] for an unreadable or malformed file, a format-version
    /// mismatch, a missing or repeated VRAM tile, or a row that is not eight
    /// hex digits.
    pub fn load(pack_dir: &Path) -> Result<DamageDigitArt, DataError> {
        let path = pack_dir.join(DAMAGE_DIGIT_ART_PATH);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let art: DamageDigitArt =
            serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        if art.format_version != PACK_FORMAT_VERSION {
            return Err(DataError::FormatVersion {
                found: art.format_version,
                expected: PACK_FORMAT_VERSION,
            });
        }
        if art.tile_count != REQUIRED_TILES || art.tiles.len() != REQUIRED_TILES {
            return Err(DataError::ManifestMismatch {
                path,
                field: "tile_count",
                manifest: REQUIRED_TILES.to_string(),
                record: format!("declared {}, actual {}", art.tile_count, art.tiles.len()),
            });
        }
        validate_tiles(&path, &art.tiles, FIRST_TILE, LAST_TILE)?;
        Ok(art)
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
                "psiv-damage-digits-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(dir.join("battle/art")).expect("temporary art directory");
            Self { dir }
        }

        fn load(&self, art: &DamageDigitArt) -> Result<DamageDigitArt, DataError> {
            std::fs::write(
                self.dir.join(DAMAGE_DIGIT_ART_PATH),
                serde_json::to_vec(art).expect("serialize damage art"),
            )
            .expect("write damage art");
            DamageDigitArt::load(&self.dir)
        }
    }

    impl Drop for TempArt {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Synthetic tiles: every pixel index 1.
    fn complete_art() -> DamageDigitArt {
        DamageDigitArt {
            format_version: PACK_FORMAT_VERSION,
            tile_count: REQUIRED_TILES,
            tiles: (FIRST_TILE..=LAST_TILE)
                .map(|vram_tile| CommandUiTile {
                    vram_tile,
                    rows: vec!["11111111".into(); 8],
                })
                .collect(),
        }
    }

    fn assert_rejected(pack: &TempArt, art: &DamageDigitArt, field: &str) {
        let error = pack.load(art).expect_err("corrupt art must not load");
        assert!(error.to_string().contains(field), "{error}");
    }

    #[test]
    fn public_load_requires_every_vram_tile() {
        let pack = TempArt::new();
        let valid = complete_art();
        let loaded = pack.load(&valid).unwrap();
        assert_eq!(loaded, valid);
        assert_eq!(loaded.tile(0x5E4), Some([[1; 8]; 8]));
        assert_eq!(loaded.tile(0x5E3), None);

        let mut art = valid.clone();
        art.tiles.remove(3);
        art.tile_count = art.tiles.len();
        assert_rejected(&pack, &art, "tile_count");

        let mut art = valid.clone();
        art.tiles[3].vram_tile = FIRST_TILE;
        assert_rejected(&pack, &art, "tiles");

        let mut art = valid.clone();
        art.tiles[3].vram_tile = LAST_TILE + 1;
        assert_rejected(&pack, &art, "tiles");

        let mut art = valid.clone();
        art.format_version += 1;
        assert!(matches!(
            pack.load(&art),
            Err(DataError::FormatVersion { .. })
        ));

        let mut art = valid;
        art.tiles[3].rows[0] = "1111111".into();
        assert_rejected(&pack, &art, "tiles");
    }

    #[test]
    fn public_load_names_a_missing_file() {
        let pack = TempArt::new();
        let error = DamageDigitArt::load(&pack.dir).expect_err("missing art must not load");
        assert!(error.to_string().contains("damage_digits.json"), "{error}");
    }
}
