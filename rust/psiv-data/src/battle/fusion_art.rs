//! `battle/art/fusion.json`: the Zol slugs' Fusion frames
//! (`psiv_tools/battle_fusion_art.py`, #64).
//!
//! Required: a pack built before the Fusion was drawn has no such file, and
//! [`crate::GameData::load`] refuses it with a rebuild message, so the game
//! never starts a battle on a pack whose battle art would fail to load.

use std::path::Path;

use serde::Deserialize;

use crate::DataError;
use crate::manifest::PACK_FORMAT_VERSION;

/// The file, relative to the pack.
pub const FUSION_ART_PATH: &str = "battle/art/fusion.json";
/// `BattleObj_Fusion` selects five mapping records (`ps4.asm:35675-35844`).
pub const FUSION_FRAMES: usize = 5;

/// The Fusion frames and the palette they draw in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FusionArt {
    /// The pack's format version.
    pub format_version: u32,
    /// The enemy whose palette the frames use: the casting Zol slug.
    pub palette_enemy_id: u16,
    /// The frames, in the order the routine selects them.
    pub frames: Vec<FusionFrame>,
}

/// One frame: its PNG, and its top-left and size relative to the piece's
/// position.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FusionFrame {
    /// Its place in [`FusionArt::frames`].
    pub index: usize,
    /// The PNG, relative to the pack.
    pub png: String,
    /// Top-left relative to the piece's position, in pixels.
    pub origin: [i32; 2],
    /// Width and height in pixels.
    pub size: [i32; 2],
}

impl FusionArt {
    /// Reads and checks `battle/art/fusion.json` under `pack_dir`.
    ///
    /// # Errors
    /// [`DataError::ManifestMismatch`] when the file is absent (a pack older
    /// than the Fusion's frames: rebuild it), or when it does not hold the
    /// five frames in order with their PNGs present;
    /// [`DataError::FormatVersion`] for another pack format.
    pub fn load(pack_dir: &Path) -> Result<FusionArt, DataError> {
        let path = pack_dir.join(FUSION_ART_PATH);
        let refused = |record: String| DataError::ManifestMismatch {
            path: path.clone(),
            field: "fusion",
            manifest: format!("{FUSION_FRAMES} frames in order, each with its PNG"),
            record,
        };
        if !path.is_file() {
            return Err(DataError::ManifestMismatch {
                path: pack_dir.join("manifest.json"),
                field: "fusion",
                manifest: "absent; rebuild the pack".into(),
                record: FUSION_ART_PATH.into(),
            });
        }
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let art: FusionArt = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        if art.format_version != PACK_FORMAT_VERSION {
            return Err(DataError::FormatVersion {
                found: art.format_version,
                expected: PACK_FORMAT_VERSION,
            });
        }
        if art.frames.len() != FUSION_FRAMES {
            return Err(refused(format!("{} frames", art.frames.len())));
        }
        for (index, frame) in art.frames.iter().enumerate() {
            if frame.index != index {
                return Err(refused(format!("frame {index} says {}", frame.index)));
            }
            if frame.size[0] <= 0 || frame.size[1] <= 0 {
                return Err(refused(format!("frame {index} is empty")));
            }
            if !pack_dir.join(&frame.png).is_file() {
                return Err(refused(format!("frame {index}'s {} is missing", frame.png)));
            }
        }
        Ok(art)
    }
}
