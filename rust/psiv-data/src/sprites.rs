//! Field sprite sheets and animation sequences, as pack format 1 emits them.
//!
//! `sprites/party.json` and `sprites/npcs.json` share one shape: a list of
//! sheets, each naming a frame-strip PNG plus the animation sequences the
//! cartridge's own mapping tables define (transcribed from `FieldObj_Animate`
//! and the per-entity `SprMapsPtrs_*` tables). Frame durations are in engine
//! ticks; mirrored frames are pre-rendered into the strip, so a renderer
//! never flips anything itself.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One sprite-sheet index file (`sprites/party.json` or `sprites/npcs.json`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SheetFile {
    /// Pack format; checked against [`crate::PACK_FORMAT_VERSION`].
    pub format_version: u32,
    /// `"field_party"` or the NPC counterpart.
    #[serde(default)]
    pub kind: Option<String>,
    /// Declared sheet count; validated against `sheets.len()`.
    pub sheet_count: u32,
    /// The sheets.
    pub sheets: Vec<Sheet>,
}

/// One frame-strip sheet.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Sheet {
    /// Sheet id: a party member's name, or an NPC art id like
    /// `NPCType2_a2bfdc9c` (symbol + content hash).
    pub id: String,
    /// The frame strip, pack-root-relative. Frames left to right.
    pub png: String,
    /// Frame width in pixels.
    pub frame_width: u32,
    /// Frame height in pixels.
    pub frame_height: u32,
    /// Number of frames in the strip.
    pub frame_count: u32,
    /// X of the cell anchor inside a frame, in pixels.
    pub origin_x: i32,
    /// Y of the cell anchor inside a frame, in pixels.
    pub origin_y: i32,
    /// Named animation sequences (`idle_down`, `walk_left`, ...).
    pub sequences: BTreeMap<String, Sequence>,
}

/// One animation sequence.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Sequence {
    /// Frames in play order.
    pub frames: Vec<SequenceFrame>,
    /// Whether the sequence loops (all retail field sequences do).
    #[serde(rename = "loop", default)]
    pub looping: bool,
}

/// One step of a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct SequenceFrame {
    /// Frame index into the strip.
    pub index: u32,
    /// How long the frame holds, in engine ticks (transcribed from the
    /// cartridge's own sequence tables, not invented).
    pub duration_ticks: u32,
}

impl Sheet {
    /// The named sequence, if the sheet defines it.
    #[must_use]
    pub fn sequence(&self, name: &str) -> Option<&Sequence> {
        self.sequences.get(name)
    }
}
