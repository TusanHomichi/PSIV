//! `manifest.json`: what ROM the pack came from and what is in it.
//!
//! Field names are `psiv_tools.pack`'s. The manifest carries a good deal of
//! self-description the runtime does not need -- the collision vocabulary, the
//! `XYRangeJmpTbl` names, anomaly reports -- which is deliberate: the pack is
//! meant to be readable without the extractor beside it. This module models the
//! parts the runtime consumes and lets the rest pass by, so a new report added
//! to the manifest never breaks a load.

use crate::ids::{MapId, RomHash};
use crate::map::MapRef;
use serde::{Deserialize, Serialize};

/// The pack format this build reads. A pack that declares anything else is
/// refused outright rather than parsed hopefully.
///
/// Kept in step with `psiv_tools.pack.PACK_FORMAT_VERSION`, which moves
/// whenever an emitted field changes meaning or disappears.
pub const PACK_FORMAT_VERSION: u32 = 1;

/// The pack index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Bumped whenever the packer changes the on-disk shape.
    pub format_version: u32,
    /// What wrote this pack, for a human staring at a stale directory.
    #[serde(default)]
    pub generator: Option<String>,
    /// The ROM the pack was extracted from.
    pub rom: RomInfo,
    /// The cartridge's new-game first-controllable state, when the pack
    /// carries it (extracted from the title-screen handoff and the opening
    /// event's epilogue; full provenance lives in `game_start.json`).
    #[serde(default)]
    pub game_start: Option<GameStartSummary>,
    /// Every packed map, in id order.
    pub maps: Vec<MapEntry>,
    /// Maps deliberately not packed, and why: the `PtrMap_Null` placeholders
    /// and the two world maps, whose paged layout format is not decoded.
    pub skipped: Vec<SkippedMap>,
    /// Maps that a packed map warps to but that this pack does not contain.
    ///
    /// A warp out of the packed set is normal -- packing a subset for a test is
    /// a supported thing to do -- so these are declared rather than treated as
    /// dangling. A warp to a map in neither list is a real defect.
    pub unpacked_warp_targets: Vec<MapRef>,
}

impl Manifest {
    /// Is this id packed?
    pub fn packs(&self, id: MapId) -> bool {
        self.maps.iter().any(|entry| entry.id == id)
    }

    /// Is this id explicitly declared unpacked, either as skipped or as a known
    /// warp target outside the pack?
    pub fn declares_unpacked(&self, id: MapId) -> bool {
        self.skipped.iter().any(|entry| entry.id == id)
            || self
                .unpacked_warp_targets
                .iter()
                .any(|target| target.id == id)
    }

    /// Why a map was skipped, if it was.
    pub fn skip_reason(&self, id: MapId) -> Option<&str> {
        self.skipped
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.reason.as_str())
    }

    /// The entry for a packed id.
    pub fn entry(&self, id: MapId) -> Option<&MapEntry> {
        self.maps.iter().find(|entry| entry.id == id)
    }
}

/// The ROM a pack was built from.
///
/// The pack records its hash and `psiv-data` refuses a mismatched pack set --
/// the same fail-closed hash discipline as everything else in the project
/// (docs/RUNTIME_DESIGN.md, "Data path").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomInfo {
    /// SHA-256 of the ROM image.
    pub sha256: RomHash,
    /// Its size in bytes.
    pub size_bytes: u64,
}

/// One packed map: what it is, where its files are, and how big it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapEntry {
    /// The map's id.
    pub id: MapId,
    /// Its `MapID_*` symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Path to the map record, relative to the pack directory.
    pub json: String,
    /// Path to the composed render, relative to the pack directory.
    pub png: String,
    /// SHA-256 of the JSON file as written. Carried for provenance; this crate
    /// does not verify it, because doing so would mean a hashing dependency for
    /// a check the filesystem is not lying about.
    #[serde(default)]
    pub json_sha256: Option<String>,
    /// SHA-256 of the PNG as written.
    #[serde(default)]
    pub png_sha256: Option<String>,
    /// Width in collision cells, repeated from the record so the inventory can
    /// be read on its own.
    pub width_cells: u32,
    /// Height in collision cells.
    pub height_cells: u32,
    /// Width in pixels.
    pub width_pixels: u32,
    /// Height in pixels.
    pub height_pixels: u32,
}

/// One map the packer left out, and the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedMap {
    /// The map's id.
    pub id: MapId,
    /// Its `MapID_*` symbol, when it has one.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Free text, for a human reading a failed load.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest_text(extra_maps: &str) -> String {
        format!(
            r#"{{"format_version": 1,
                 "generator": "psiv_tools.pack",
                 "rom": {{"sha256": "{DIGEST}", "size_bytes": 3145728}},
                 "collision": {{"cell_pixels": 16, "blocking_types": [8, 9, 10, 11, 12]}},
                 "warps": {{"count": 7, "rect_units": "collision cells"}},
                 "map_count": 1,
                 "maps": [{{"id": 16, "symbol": "Piata", "json": "maps/010_Piata.json",
                            "png": "maps/010_Piata.png", "json_sha256": "{DIGEST}",
                            "png_sha256": "{DIGEST}", "width_cells": 62, "height_cells": 62,
                            "width_pixels": 992, "height_pixels": 992}}{extra_maps}],
                 "skipped": [{{"id": 0, "id_hex": "0x000", "symbol": "Motavia",
                               "reason": "paged layout format not decoded"}}],
                 "unpacked_warp_targets": [{{"id": 24, "id_hex": "0x018",
                                             "symbol": "PiataDorm"}}],
                 "unloaded_patterns": [],
                 "layout_blob_size_mismatches": []}}"#
        )
    }

    #[test]
    fn parses_the_packers_manifest_and_ignores_its_extra_reports() {
        let manifest: Manifest = serde_json::from_str(&manifest_text("")).unwrap();
        assert_eq!(manifest.format_version, PACK_FORMAT_VERSION);
        assert_eq!(manifest.rom.sha256.as_str(), DIGEST);
        assert_eq!(manifest.rom.size_bytes, 3_145_728);
        assert_eq!(manifest.generator.as_deref(), Some("psiv_tools.pack"));

        assert!(manifest.packs(MapId(0x010)));
        assert!(!manifest.packs(MapId(0x000)));
        assert_eq!(
            manifest.entry(MapId(0x010)).unwrap().json,
            "maps/010_Piata.json"
        );
        assert_eq!(manifest.entry(MapId(0x010)).unwrap().width_cells, 62);
    }

    #[test]
    fn unpacked_targets_come_from_both_lists() {
        let manifest: Manifest = serde_json::from_str(&manifest_text("")).unwrap();
        assert!(manifest.declares_unpacked(MapId(0x000)), "skipped");
        assert!(
            manifest.declares_unpacked(MapId(0x018)),
            "unpacked warp target"
        );
        assert!(!manifest.declares_unpacked(MapId(0x123)));
        assert_eq!(
            manifest.skip_reason(MapId(0x000)),
            Some("paged layout format not decoded")
        );
        assert_eq!(manifest.skip_reason(MapId(0x018)), None);
    }

    #[test]
    fn a_map_entry_missing_its_json_path_is_refused() {
        let err = serde_json::from_str::<Manifest>(&manifest_text(
            r#", {"id": 17, "symbol": "PiataAcademy", "png": "p.png",
                  "width_cells": 4, "height_cells": 4,
                  "width_pixels": 64, "height_pixels": 64}"#,
        ))
        .unwrap_err();
        assert!(err.to_string().contains("json"), "{err}");
    }

    #[test]
    fn a_bad_rom_hash_fails_the_parse() {
        let json = manifest_text("").replace(DIGEST, "not-a-digest");
        assert!(serde_json::from_str::<Manifest>(&json).is_err());
    }
}

/// The manifest's new-game headline: enough to spawn without opening
/// `game_start.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStartSummary {
    /// The first controllable map.
    pub map: GameStartMap,
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells (standing shift applied, like everything in the pack).
    pub y_cell: u32,
    /// Initial facing.
    pub facing: GameStartFacing,
    /// Occupied party slots at first control, as character symbols (the
    /// cartridge starts Chaz alone). Full slot detail is in game_start.json.
    pub party: Vec<String>,
    /// Event flags already set when control arrives.
    pub event_flags_set: Vec<u16>,
    /// Extended event flags already set when control arrives, as ids in the
    /// combined `$100..=$1FF` event-flag space (the pack emits them
    /// pre-offset). Absent in packs predating the field.
    #[serde(default)]
    pub extended_event_flags_set: Vec<u16>,
    /// Chest flags already set when control arrives (retail: none). Absent
    /// in packs predating the field.
    #[serde(default)]
    pub chest_flags_set: Vec<u16>,
    /// Town flags already set when control arrives. Absent in packs
    /// predating the field.
    #[serde(default)]
    pub town_flags_set: Vec<u16>,
}

/// The starting map reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStartMap {
    /// Map id.
    pub id: u16,
    /// The `MapID_*` symbol.
    #[serde(default)]
    pub symbol: Option<String>,
}

/// The starting facing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStartFacing {
    /// Raw byte.
    pub id: u8,
    /// Decoded name, when the byte is one of the four.
    #[serde(default)]
    pub name: Option<crate::map::Direction>,
}
