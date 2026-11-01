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
use serde_json::{Map, Value};

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
    /// Optional per-map vehicle palette variants. The base `sheets` remain the
    /// selector-addressed fallback; each variant names the same three vehicle
    /// slots and carries only the additional sheets required by its CRAM line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette_variants: Vec<VehiclePaletteVariant>,
    /// What the pack carries and the game doesn't read (`field_objects`,
    /// `palette_source`): it has no typed home yet.
    #[serde(flatten)]
    pub research: Map<String, Value>,
}

/// One CRAM-line-3 vehicle palette used by one or more maps.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct VehiclePaletteVariant {
    /// Map ids whose vehicle sprites use this palette.
    pub map_ids: Vec<u16>,
    /// Three sheet ids in `Vehicle_Index` order.
    pub sheet_ids: Vec<String>,
    /// Additional sheets. The base variant may leave this empty and refer to
    /// the ordinary `SheetFile::sheets` entries.
    #[serde(default)]
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
    /// What the pack carries and the game doesn't read (the palette, mirrors,
    /// art provenance, placements): it has no typed home yet.
    #[serde(flatten)]
    pub research: Map<String, Value>,
}

/// One animation sequence.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Sequence {
    /// Frames in play order.
    pub frames: Vec<SequenceFrame>,
    /// Whether the sequence loops (all retail field sequences do).
    #[serde(rename = "loop", default)]
    pub looping: bool,
    /// Where the cartridge's sequence table is, as `0x` hex; research only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence_offset: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_game_does_not_read_survives_a_round_trip() {
        let text = r#"{"format_version":2,"kind":"field_party","sheet_count":1,
            "field_objects":{"count":1},
            "sheets":[{"id":"A","png":"a.png","frame_width":1,"frame_height":1,"frame_count":1,
            "origin_x":0,"origin_y":0,"palette":{"cram_line":2},"mirrors":{},
            "sequences":{"idle_down":{"frames":[{"index":0,"duration_ticks":1}],"loop":true,
            "sequence_offset":"0x000001"}}}]}"#;
        let file: SheetFile = serde_json::from_str(text).unwrap();
        assert!(file.research.contains_key("field_objects"));
        assert!(file.sheets[0].research.contains_key("palette"));
        let back = serde_json::to_value(&file).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(text).unwrap());
        // A file with no variants writes none: only vehicles.json has the key.
        assert!(back.get("palette_variants").is_none());
    }
}
