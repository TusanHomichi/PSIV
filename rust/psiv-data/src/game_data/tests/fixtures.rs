//! The synthetic pack fixtures every case builds on.
//!
//! `MapJson` is the two-map pack the loader is pointed at, one section per field
//! a case might want to break; `manifest_text` writes the index that lists it.
//! Everything here is visible to the sibling test modules and to nothing else.

use crate::error::DataError;
use crate::manifest::PACK_FORMAT_VERSION;
use crate::map::MapRecord;
use crate::{GameData, Manifest};

pub(super) const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A map record as JSON, with every section a test might want to break exposed
/// as a field. Defaults describe a valid 4x4 map.
pub(super) struct MapJson {
    pub(super) version: u32,
    pub(super) id: u16,
    pub(super) symbol: &'static str,
    pub(super) dimensions: String,
    pub(super) collision: String,
    pub(super) warps: String,
    pub(super) npcs: String,
    pub(super) treasure_chests: String,
    pub(super) music: String,
    pub(super) flags: String,
    pub(super) dialogue_tree: String,
}

impl Default for MapJson {
    fn default() -> Self {
        MapJson {
            version: PACK_FORMAT_VERSION,
            id: 0x010,
            symbol: "Piata",
            dimensions: r#"{"width_cells": 4, "height_cells": 4,
                            "width_chunks": 2, "height_chunks": 2,
                            "width_pixels": 64, "height_pixels": 64, "cell_pixels": 16}"#
                .to_string(),
            // (1,1) is a doorway; two solid cells sit below it.
            collision: r#"{"plane": "bg", "plane_byte": 1,
                           "width_cells": 4, "height_cells": 4,
                           "rows": [[0,0,0,0],[0,1,0,0],[0,8,8,0],[0,0,0,0]]}"#
                .to_string(),
            warps: warp_json(
                r#""rect": {"x": 1, "y": 1, "width": 1, "height": 1}"#,
                r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
                r#""destination": {"x_cell": 2, "y_cell": 3}"#,
            ),
            npcs: r#"[{"index": 0, "record_offset": "0x11C734", "object_id": 60,
                       "symbol": "NPCType2", "x_pixels": 32, "y_pixels": 16,
                       "x_cell": 2, "y_cell": 2,
                       "facing": {"id": 0, "name": "down"},
                       "dialogue_id": 64, "art_tile": 792, "sprite_reason": "synthetic fixture"}]"#
                .to_string(),
            treasure_chests: r#"[{"index": 0, "record_offset": "0x129004",
                                  "x_cell": 3, "y_cell": 3, "x_pixels": 48, "y_pixels": 32,
                                  "white_chest": false, "object_symbol": "TreasureChest",
                                  "contents_type": "item", "item_id": 125,
                                  "item_symbol": "Dagger", "meseta": null,
                                  "chest_flag": 24}]"#
                .to_string(),
            music: r#"{"id": 132, "symbol": "MotabiaTown", "changes_music": true}"#.to_string(),
            flags: r#"{"poison": 0, "random_battles": 0, "town_teleport": 1,
                       "dungeon_teleport_index": 0}"#
                .to_string(),
            dialogue_tree: "2".to_string(),
        }
    }
}

/// One warp, with the three parts tests vary spliced in.
pub(super) fn warp_json(rect: &str, target: &str, destination: &str) -> String {
    format!(
        r#"[{{"index": 0, "table": 2, "trigger": "map_change_tile",
               "record_offset": "0x11C6F6",
               "range": {{"id": 3, "name": "XYExact"}},
               "source": {{"x_byte": 1, "y_byte": 0, "x_cell": 1, "y_cell": 1}},
               {rect}, {target}, {destination},
               "facing": {{"id": 4, "name": "up"}},
               "character_alignment": 4}}]"#
    )
}

impl MapJson {
    /// The second map of the default pack: warps back to the first.
    pub(super) fn academy() -> Self {
        MapJson {
            id: 0x011,
            symbol: "PiataAcademy",
            warps: warp_json(
                r#""rect": {"x": 2, "y": 3, "width": 1, "height": 1}"#,
                r#""target": {"id": 16, "id_hex": "0x010", "symbol": "Piata"}"#,
                r#""destination": {"x_cell": 1, "y_cell": 2}"#,
            ),
            npcs: "[]".to_string(),
            treasure_chests: "[]".to_string(),
            ..Default::default()
        }
    }

    pub(super) fn text(&self) -> String {
        format!(
            r#"{{"format_version": {}, "id": {}, "id_hex": "0x{:03X}", "symbol": "{}",
                 "record_offset": "0x11C690", "png": "{}",
                 "dimensions": {}, "collision": {}, "music": {}, "flags": {},
                 "dialogue_tree": {}, "warps": {}, "npcs": {}, "treasure_chests": {}}}"#,
            self.version,
            self.id,
            self.id,
            self.symbol,
            self.png_name(),
            self.dimensions,
            self.collision,
            self.music,
            self.flags,
            self.dialogue_tree,
            self.warps,
            self.npcs,
            self.treasure_chests,
        )
    }

    pub(super) fn parse(&self) -> MapRecord {
        serde_json::from_str(&self.text())
            .unwrap_or_else(|e| panic!("fixture for map {:#05X} should parse: {e}", self.id))
    }

    pub(super) fn try_parse(&self) -> Result<MapRecord, serde_json::Error> {
        serde_json::from_str(&self.text())
    }

    fn stem(&self) -> String {
        format!("{:03X}_{}", self.id, self.symbol)
    }

    pub(super) fn json_name(&self) -> String {
        format!("maps/{}.json", self.stem())
    }

    fn png_name(&self) -> String {
        format!("maps/{}.png", self.stem())
    }
}

pub(super) fn manifest_text(
    version: u32,
    maps: &[&MapJson],
    skipped: &str,
    unpacked: &str,
) -> String {
    let entries: Vec<String> = maps
        .iter()
        .map(|m| {
            format!(
                r#"{{"id": {}, "id_hex": "0x{:03X}", "symbol": "{}", "json": "{}", "png": "{}",
                     "json_sha256": "{DIGEST}", "png_sha256": "{DIGEST}",
                     "width_cells": 4, "height_cells": 4,
                     "width_pixels": 64, "height_pixels": 64}}"#,
                m.id,
                m.id,
                m.symbol,
                m.json_name(),
                m.png_name()
            )
        })
        .collect();
    format!(
        r#"{{"format_version": {version}, "generator": "psiv_tools.pack",
             "rom": {{"sha256": "{DIGEST}", "size_bytes": 3145728}},
             "collision": {{"cell_pixels": 16, "blocking_types": [8, 9, 10, 11, 12]}},
             "warps": {{"count": {}, "rect_units": "collision cells",
                        "standing_cell_y_offset": 1, "without_map_change_cell": []}},
             "map_count": {}, "maps": [{}],
             "skipped": [{skipped}],
             "unpacked_warp_targets": [{unpacked}],
             "unloaded_patterns": [], "layout_blob_size_mismatches": []}}"#,
        maps.len(),
        maps.len(),
        entries.join(", ")
    )
}

pub(super) const SKIPPED_MOTAVIA: &str = r#"{"id": 0, "id_hex": "0x000", "symbol": "Motavia",
                                  "reason": "paged layout format not decoded"}"#;

pub(super) fn manifest_of(maps: &[&MapJson]) -> Manifest {
    serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        maps,
        SKIPPED_MOTAVIA,
        "",
    ))
    .expect("fixture manifest parses")
}

/// Assemble a pack from fixtures, running the whole validation pass.
pub(super) fn assemble(maps: &[&MapJson]) -> Result<GameData, DataError> {
    let manifest = manifest_of(maps);
    let records = maps.iter().map(|m| m.parse()).collect();
    GameData::from_parts(manifest, records)
}

/// Assemble the default two-map pack with the first map replaced.
pub(super) fn assemble_with(map: &MapJson) -> Result<GameData, DataError> {
    let academy = MapJson::academy();
    assemble(&[map, &academy])
}

pub(super) fn expect_validation(
    result: Result<GameData, DataError>,
    field: &str,
    needle: &str,
) -> String {
    let err = match result {
        Ok(_) => panic!("expected a validation failure for `{field}`, but the pack loaded"),
        Err(err) => err,
    };
    let text = err.to_string();
    assert!(
        matches!(err, DataError::Validation { .. }),
        "expected a Validation error, got {err:?}"
    );
    assert!(
        text.contains(field),
        "message should name `{field}`: {text}"
    );
    assert!(
        text.contains(needle),
        "message should explain `{needle}`: {text}"
    );
    text
}
