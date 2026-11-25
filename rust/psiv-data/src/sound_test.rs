//! `title/sound_test.json`: the post-clear Sound Test track table
//! (`Title_SoundTestLoop`, `SoundTestStrings`, `SoundTestData`,
//! `ps4.asm:88525-88645`).

use std::path::Path;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::DataError;

/// Pack-relative path of the Sound Test table.
pub const SOUND_TEST_FILE: &str = "title/sound_test.json";

/// Schema version of `title/sound_test.json`.
pub const SOUND_TEST_VERSION: u32 = 1;

/// One entry in `SoundTestStrings` / `SoundTestData`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct SoundTestTrack {
    /// Zero-based track index (`0..track_count`).
    pub index: usize,
    /// Cartridge display title (`SoundTestStrings`).
    pub title: String,
    /// Retail `Sound_Index` byte (`SoundTestData` byte 0).
    pub sound_id: u8,
    /// Composer byte (`0 = IPPO`, `1 = GAKI CHAN`).
    pub composer_id: u8,
    /// Composer display label (`IPPO` or `GAKI CHAN`).
    pub composer: String,
}

/// Retail's repeat-play override (`ps4.asm:88554-88561`): playing
/// `MusicID_DungeonArrange2` twice in a row plays `MusicID_DungeonArrange2Cont`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct SoundTestRepeatOverride {
    /// Track `sound_id` that triggers the alternate track when repeated.
    pub when_sound_id: u8,
    /// Alternate `Sound_Index` byte played on repeat.
    pub play_sound_id: u8,
}

/// Validated `title/sound_test.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct SoundTestFile {
    /// Pack format version (1).
    pub format_version: u32,
    /// Declared track count, checked against `tracks.len()`.
    pub track_count: usize,
    /// Optional retail repeat-play override (`DungeonArrange2` -> `Cont`).
    #[serde(default)]
    pub repeat_override: Option<SoundTestRepeatOverride>,
    /// Ordered tracks (`index == position`).
    pub tracks: Vec<SoundTestTrack>,
}

impl SoundTestFile {
    /// Reads and validates `title/sound_test.json` under `pack_dir`.
    pub fn load(pack_dir: &Path) -> Result<SoundTestFile, DataError> {
        Self::load_overlay(&crate::mods::PackOverlay::base(pack_dir))
    }

    /// Reads and validates `title/sound_test.json` through a [`crate::PackOverlay`].
    pub fn load_overlay(overlay: &crate::mods::PackOverlay) -> Result<SoundTestFile, DataError> {
        let path = overlay.logical_path(SOUND_TEST_FILE);
        let file: SoundTestFile = overlay.read_json_data(SOUND_TEST_FILE)?;
        if file.format_version != SOUND_TEST_VERSION {
            return Err(DataError::FormatVersion {
                found: file.format_version,
                expected: SOUND_TEST_VERSION,
            });
        }
        if file.tracks.is_empty() || file.tracks.len() != file.track_count {
            return Err(DataError::ManifestMismatch {
                path,
                field: "track_count",
                manifest: file.track_count.to_string(),
                record: file.tracks.len().to_string(),
            });
        }
        for (i, track) in file.tracks.iter().enumerate() {
            if track.index != i {
                return Err(DataError::ManifestMismatch {
                    path,
                    field: "index",
                    manifest: i.to_string(),
                    record: track.index.to_string(),
                });
            }
        }
        Ok(file)
    }

    /// Resolves the `Sound_Index` byte to play for `index`, honoring the
    /// retail consecutive-repeat override (`ps4.asm:88554-88561`), and returns
    /// `(sound_to_play, base_sound_id)` where `base_sound_id` is stored as the
    /// next `last_played` value (`$FFFFE201`).
    #[must_use]
    pub fn resolve_play(&self, index: usize, last_played: Option<u8>) -> Option<(u8, u8)> {
        let track = self.tracks.get(index)?;
        let base = track.sound_id;
        let play = match self.repeat_override {
            Some(ov) if base == ov.when_sound_id && last_played == Some(base) => ov.play_sound_id,
            _ => base,
        };
        Some((play, base))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_play_applies_repeat_override_only_on_consecutive_same_track() {
        let file = SoundTestFile {
            format_version: 1,
            track_count: 2,
            repeat_override: Some(SoundTestRepeatOverride {
                when_sound_id: 0xB1,
                play_sound_id: 0xA7,
            }),
            tracks: vec![
                SoundTestTrack {
                    index: 0,
                    title: "TRACK A".into(),
                    sound_id: 0x84,
                    composer_id: 0,
                    composer: "IPPO".into(),
                },
                SoundTestTrack {
                    index: 1,
                    title: "TRACK B".into(),
                    sound_id: 0xB1,
                    composer_id: 0,
                    composer: "IPPO".into(),
                },
            ],
        };
        // First play of 0xB1 plays 0xB1 and records 0xB1 as last_played.
        assert_eq!(file.resolve_play(1, None), Some((0xB1, 0xB1)));
        // Consecutive replay of 0xB1 triggers override 0xA7, still recording 0xB1.
        assert_eq!(file.resolve_play(1, Some(0xB1)), Some((0xA7, 0xB1)));
        // Switching to another track plays its base id and clears the repeat match.
        assert_eq!(file.resolve_play(0, Some(0xB1)), Some((0x84, 0x84)));
        assert_eq!(file.resolve_play(1, Some(0x84)), Some((0xB1, 0xB1)));
    }
}
