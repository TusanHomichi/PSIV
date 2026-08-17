//! Map-record camera setup copied from `loc_51AB2`.

use serde::{Deserialize, Serialize};

/// The two longwords that `loc_51AB2` can seed as camera step counters.
///
/// The pack keeps the ROM's eight-digit hexadecimal spelling because these
/// are 16.16 signed longwords, not pixel counts. Runtime code parses them at
/// the camera boundary and rejects no information by narrowing them to words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScrollCounters {
    /// `Camera_X_Step_Counter_*`, as an eight-digit hexadecimal string.
    pub x: String,
    /// `Camera_Y_Step_Counter_*`, as an eight-digit hexadecimal string.
    pub y: String,
}

/// The map record's six-to-twenty-two-byte camera setup section.
///
/// `loc_51AB2` consumes the first byte as `$EC24`, then the low byte of each
/// plane word as `$EC25`/`$EC26`; when a plane byte is zero it consumes the
/// following two longwords as that plane's initial step counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapScroll {
    /// Global sprite-plane selector, `$EC24` (`0` foreground, nonzero background).
    pub mode: u8,
    /// The skipped byte between `$EC24` and the foreground word.
    #[serde(default)]
    pub padding_byte: String,
    /// Foreground driver gate, `$EC25`.
    pub fg_scroll_mode: u8,
    /// Original foreground word, retained for extraction provenance.
    #[serde(default)]
    pub fg_scroll_word: String,
    /// Initial foreground step counters when `$EC25` is zero.
    #[serde(default)]
    pub fg_step_counters: Option<ScrollCounters>,
    /// Background driver gate, `$EC26`.
    pub bg_scroll_mode: u8,
    /// Original background word, retained for extraction provenance.
    #[serde(default)]
    pub bg_scroll_word: String,
    /// Initial background step counters when `$EC26` is zero.
    #[serde(default)]
    pub bg_step_counters: Option<ScrollCounters>,
}

impl Default for MapScroll {
    fn default() -> MapScroll {
        MapScroll {
            mode: 1,
            padding_byte: "0x00".to_owned(),
            fg_scroll_mode: 1,
            fg_scroll_word: "0x0001".to_owned(),
            fg_step_counters: None,
            bg_scroll_mode: 1,
            bg_scroll_word: "0x0001".to_owned(),
            bg_step_counters: None,
        }
    }
}
