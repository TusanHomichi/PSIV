//! Unit tests for loading and validation.
//!
//! Fixtures are hand-written synthetic JSON -- 4x4 maps, invented symbols -- so
//! nothing Sega-derived is committed. They are shaped exactly like
//! `psiv_tools.pack` output, extra provenance fields included, so a field-name
//! drift between the two lanes fails here rather than at integration. The real
//! pack is exercised by `tests/runtime_pack.rs`, which is gated on it existing.

use super::*;
use crate::collision::{CollisionType, Plane};
use crate::map::{CellRect, Direction, TransitionTable};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A map record as JSON, with every section a test might want to break exposed
/// as a field. Defaults describe a valid 4x4 map.
struct MapJson {
    version: u32,
    id: u16,
    symbol: &'static str,
    dimensions: String,
    collision: String,
    warps: String,
    npcs: String,
    treasure_chests: String,
    music: String,
    flags: String,
    dialogue_tree: String,
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
fn warp_json(rect: &str, target: &str, destination: &str) -> String {
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
    fn academy() -> Self {
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

    fn text(&self) -> String {
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

    fn parse(&self) -> MapRecord {
        serde_json::from_str(&self.text())
            .unwrap_or_else(|e| panic!("fixture for map {:#05X} should parse: {e}", self.id))
    }

    fn try_parse(&self) -> Result<MapRecord, serde_json::Error> {
        serde_json::from_str(&self.text())
    }

    fn stem(&self) -> String {
        format!("{:03X}_{}", self.id, self.symbol)
    }

    fn json_name(&self) -> String {
        format!("maps/{}.json", self.stem())
    }

    fn png_name(&self) -> String {
        format!("maps/{}.png", self.stem())
    }
}

fn manifest_text(version: u32, maps: &[&MapJson], skipped: &str, unpacked: &str) -> String {
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

const SKIPPED_MOTAVIA: &str = r#"{"id": 0, "id_hex": "0x000", "symbol": "Motavia",
                                  "reason": "paged layout format not decoded"}"#;

fn manifest_of(maps: &[&MapJson]) -> Manifest {
    serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        maps,
        SKIPPED_MOTAVIA,
        "",
    ))
    .expect("fixture manifest parses")
}

/// Assemble a pack from fixtures, running the whole validation pass.
fn assemble(maps: &[&MapJson]) -> Result<GameData, DataError> {
    let manifest = manifest_of(maps);
    let records = maps.iter().map(|m| m.parse()).collect();
    GameData::from_parts(manifest, records)
}

/// Assemble the default two-map pack with the first map replaced.
fn assemble_with(map: &MapJson) -> Result<GameData, DataError> {
    let academy = MapJson::academy();
    assemble(&[map, &academy])
}

fn expect_validation(result: Result<GameData, DataError>, field: &str, needle: &str) -> String {
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

// ---------------------------------------------------------------------------
// Happy path and accessors
// ---------------------------------------------------------------------------

#[test]
fn a_well_formed_pack_loads() {
    let data = assemble_with(&MapJson::default()).expect("the default fixture pack is valid");

    assert_eq!(data.len(), 2);
    assert!(!data.is_empty());
    assert!(data.contains(MapId(0x010)));
    assert!(!data.contains(MapId(0x000)));
    assert_eq!(
        data.skip_reason(MapId(0x000)),
        Some("paged layout format not decoded")
    );
    assert_eq!(data.skip_reason(MapId(0x010)), None);
    assert!(data.knows(MapId(0x000)), "skipped maps are still known");
    assert!(!data.knows(MapId(0x123)));
    assert_eq!(data.manifest().rom.sha256.as_str(), DIGEST);

    let piata = data.map(MapId(0x010)).expect("Piata is packed");
    assert_eq!(piata.label(), "Piata");
    assert_eq!(piata.music.id, 132);
    assert!(piata.flags.allows_town_teleport());
    assert!(!piata.flags.rolls_random_battles());
    assert_eq!(piata.dialogue_tree, 2);
    assert_eq!(piata.collision.plane, Plane::Bg);
    assert_eq!(piata.npcs[0].dialogue_id, 64);
    assert_eq!(piata.npcs[0].facing.direction(), Some(Direction::Down));
    assert_eq!(piata.treasure_chests[0].item_id, Some(125));
    assert_eq!(piata.warps[0].table, TransitionTable::MapChange);
    assert_eq!(piata.warps[0].facing.direction(), Some(Direction::Up));
    assert_eq!(piata.warps[0].trigger, "map_change_tile");
}

#[test]
fn maps_iterate_in_ascending_id_order() {
    let academy = MapJson::academy();
    let piata = MapJson::default();
    // Manifest order is deliberately the reverse of id order.
    let data = assemble(&[&academy, &piata]).expect("valid");
    let ids: Vec<MapId> = data.map_ids().collect();
    assert_eq!(ids, vec![MapId(0x010), MapId(0x011)]);
    let labels: Vec<String> = data.maps().map(|(_, m)| m.label()).collect();
    assert_eq!(labels, vec!["Piata", "PiataAcademy"]);
}

#[test]
fn collision_and_warp_lookups_agree_with_the_grid() {
    let data = assemble_with(&MapJson::default()).unwrap();
    let piata = data.map(MapId(0x010)).unwrap();

    let door = CellPos::new(1, 1);
    assert_eq!(piata.collision_at(door), Some(CollisionType::MapChange));
    assert!(
        !piata.blocks_at(door),
        "a map_change cell is walked onto, not blocked"
    );
    assert!(piata.blocks_at(CellPos::new(1, 2)), "0x8 is solid");
    assert!(
        piata.blocks_at(CellPos::new(9, 9)),
        "outside the map blocks"
    );

    let hits: Vec<MapId> = piata.warps_at(door).map(|w| w.target.id).collect();
    assert_eq!(hits, vec![MapId(0x011)]);
    assert_eq!(piata.warps_at(CellPos::new(0, 0)).count(), 0);
    assert!(piata.contains(door));
    assert!(!piata.contains(CellPos::new(4, 0)));
}

#[test]
fn a_warp_may_target_a_map_the_manifest_declares_unpacked() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 0, "y": 0, "width": 4, "height": 1}"#,
            r#""target": {"id": 0, "id_hex": "0x000", "symbol": "Motavia"}"#,
            // Nowhere near this map's 4x4 bounds, which is the point: an
            // unpacked target's size is unknown and must not be guessed at.
            r#""destination": {"x_cell": 46, "y_cell": 142}"#,
        ),
        ..Default::default()
    };
    assemble_with(&map).expect("a skipped map is a known target");

    // The other list of known-but-unpacked maps works the same way.
    let piata = MapJson {
        warps: warp_json(
            r#""rect": {"x": 0, "y": 0, "width": 4, "height": 1}"#,
            r#""target": {"id": 24, "id_hex": "0x018", "symbol": "PiataDorm"}"#,
            r#""destination": {"x_cell": 45, "y_cell": 30}"#,
        ),
        ..Default::default()
    };
    let academy = MapJson::academy();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        &[&piata, &academy],
        SKIPPED_MOTAVIA,
        r#"{"id": 24, "id_hex": "0x018", "symbol": "PiataDorm"}"#,
    ))
    .unwrap();
    GameData::from_parts(manifest, vec![piata.parse(), academy.parse()])
        .expect("an unpacked warp target is a known target");
}

#[test]
fn a_warp_with_no_trigger_rectangle_is_accepted() {
    // XYRange 0 is `Null` and never fires; a clipped half-plane can also fall
    // entirely off the map. Dead data, not a defect.
    let map = MapJson {
        warps: warp_json(
            r#""rect": null"#,
            r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        ),
        ..Default::default()
    };
    let data = assemble_with(&map).expect("a rectangle-less warp is not an error");
    let piata = data.map(MapId(0x010)).unwrap();
    assert!(piata.warps[0].rect.is_none());
    assert_eq!(piata.warps_at(CellPos::new(1, 1)).count(), 0);
}

// ---------------------------------------------------------------------------
// Manifest-level failures
// ---------------------------------------------------------------------------

#[test]
fn a_format_version_mismatch_is_refused() {
    let piata = MapJson::default();
    let academy = MapJson::academy();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION + 1,
        &[&piata, &academy],
        SKIPPED_MOTAVIA,
        "",
    ))
    .unwrap();
    match GameData::from_parts(manifest, vec![piata.parse(), academy.parse()]) {
        Err(DataError::FormatVersion { found, expected }) => {
            assert_eq!(
                (found, expected),
                (PACK_FORMAT_VERSION + 1, PACK_FORMAT_VERSION)
            );
        }
        other => panic!("expected a FormatVersion error, got {other:?}"),
    }
}

#[test]
fn a_map_record_from_a_different_pack_version_is_refused() {
    // Each map file carries its own `format_version`, so a stale file left in a
    // rebuilt pack directory is caught even when the manifest looks current.
    let stale = MapJson {
        version: PACK_FORMAT_VERSION + 1,
        ..Default::default()
    };
    match assemble_with(&stale) {
        Err(DataError::FormatVersion { found, .. }) => {
            assert_eq!(found, PACK_FORMAT_VERSION + 1);
        }
        other => panic!("expected a FormatVersion error, got {other:?}"),
    }
}

#[test]
fn a_map_listed_twice_in_the_manifest_is_refused() {
    let piata = MapJson::default();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        &[&piata, &piata],
        SKIPPED_MOTAVIA,
        "",
    ))
    .unwrap();
    match GameData::from_parts(manifest, vec![piata.parse()]) {
        Err(DataError::DuplicateMapId { id, detail }) => {
            assert_eq!(id, MapId(0x010));
            assert!(detail.contains("listed twice"), "{detail}");
        }
        other => panic!("expected a DuplicateMapId error, got {other:?}"),
    }
}

#[test]
fn a_map_both_packed_and_skipped_is_refused() {
    let piata = MapJson::default();
    let academy = MapJson::academy();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        &[&piata, &academy],
        r#"{"id": 16, "symbol": "Piata", "reason": "contradiction"}"#,
        "",
    ))
    .unwrap();
    match GameData::from_parts(manifest, vec![piata.parse(), academy.parse()]) {
        Err(DataError::DuplicateMapId { id, detail }) => {
            assert_eq!(id, MapId(0x010));
            assert!(detail.contains("skipped"), "{detail}");
        }
        other => panic!("expected a DuplicateMapId error, got {other:?}"),
    }
}

#[test]
fn two_records_claiming_the_same_id_are_refused() {
    let piata = MapJson::default();
    let manifest = manifest_of(&[&piata]);
    match GameData::from_parts(manifest, vec![piata.parse(), piata.parse()]) {
        Err(DataError::DuplicateMapId { id, detail }) => {
            assert_eq!(id, MapId(0x010));
            assert!(detail.contains("two map records"), "{detail}");
        }
        other => panic!("expected a DuplicateMapId error, got {other:?}"),
    }
}

#[test]
fn the_manifest_and_the_record_set_must_describe_the_same_pack() {
    let piata = MapJson::default();
    let academy = MapJson::academy();

    // Listed but not loaded.
    let manifest = manifest_of(&[&piata, &academy]);
    expect_validation(
        GameData::from_parts(manifest, vec![piata.parse()]),
        "manifest.maps",
        "no record was loaded",
    );

    // Loaded but not listed.
    let manifest = manifest_of(&[&piata]);
    expect_validation(
        GameData::from_parts(manifest, vec![piata.parse(), academy.parse()]),
        "manifest.maps",
        "does not list",
    );
}

// ---------------------------------------------------------------------------
// Dimension and collision failures
// ---------------------------------------------------------------------------

#[test]
fn a_grid_that_does_not_match_the_declared_dimensions_is_refused() {
    let map = MapJson {
        collision: r#"{"plane": "fg", "plane_byte": 0,
                       "width_cells": 3, "height_cells": 3,
                       "rows": [[0,0,0],[0,1,0],[0,8,8]]}"#
            .to_string(),
        ..Default::default()
    };
    let text = expect_validation(assemble_with(&map), "collision", "3x3");
    assert!(text.contains("map 0x010"), "{text}");
    assert!(text.contains("declares 4x4"), "{text}");
}

#[test]
fn a_cell_size_other_than_sixteen_pixels_is_refused() {
    let map = MapJson {
        dimensions: r#"{"width_cells": 4, "height_cells": 4,
                        "width_chunks": 2, "height_chunks": 2,
                        "width_pixels": 32, "height_pixels": 32, "cell_pixels": 8}"#
            .to_string(),
        ..Default::default()
    };
    expect_validation(
        assemble_with(&map),
        "dimensions.cell_pixels",
        "16-pixel cell grid",
    );
}

#[test]
fn pixel_dimensions_must_agree_with_cell_dimensions() {
    let map = MapJson {
        dimensions: r#"{"width_cells": 4, "height_cells": 4,
                        "width_chunks": 2, "height_chunks": 2,
                        "width_pixels": 64, "height_pixels": 99, "cell_pixels": 16}"#
            .to_string(),
        ..Default::default()
    };
    expect_validation(assemble_with(&map), "dimensions.height_pixels", "99");
}

#[test]
fn chunk_dimensions_must_agree_with_cell_dimensions() {
    let map = MapJson {
        dimensions: r#"{"width_cells": 4, "height_cells": 4,
                        "width_chunks": 3, "height_chunks": 2,
                        "width_pixels": 64, "height_pixels": 64, "cell_pixels": 16}"#
            .to_string(),
        ..Default::default()
    };
    expect_validation(
        assemble_with(&map),
        "dimensions.width_cells",
        "3 chunks is 6 cells",
    );
}

#[test]
fn a_zero_sized_map_is_refused() {
    let map = MapJson {
        dimensions: r#"{"width_cells": 0, "height_cells": 0,
                        "width_chunks": 0, "height_chunks": 0,
                        "width_pixels": 0, "height_pixels": 0, "cell_pixels": 16}"#
            .to_string(),
        collision: r#"{"plane": "fg", "plane_byte": 0,
                       "width_cells": 0, "height_cells": 0, "rows": []}"#
            .to_string(),
        warps: "[]".to_string(),
        npcs: "[]".to_string(),
        treasure_chests: "[]".to_string(),
        ..Default::default()
    };
    expect_validation(assemble_with(&map), "dimensions", "cannot be 0x0");
}

#[test]
fn a_collision_value_too_big_for_a_nibble_fails_the_parse() {
    let map = MapJson {
        collision: r#"{"plane": "fg", "plane_byte": 0,
                       "width_cells": 4, "height_cells": 4,
                       "rows": [[0,0,0,0],[0,0,99,0],[0,0,0,0],[0,0,0,0]]}"#
            .to_string(),
        ..Default::default()
    };
    let err = map.try_parse().unwrap_err().to_string();
    assert!(err.contains("row 1, cell 2"), "{err}");
    assert!(err.contains("4-bit code"), "{err}");
}

#[test]
fn an_unnamed_collision_code_loads_and_does_not_block() {
    // Code $D is one of the eight the disassembly does not name. Retail uses
    // $7 the same way; both route to `TileColl_Empty`.
    let map = MapJson {
        collision: r#"{"plane": "fg", "plane_byte": 0,
                       "width_cells": 4, "height_cells": 4,
                       "rows": [[0,0,0,0],[0,1,13,0],[0,8,8,0],[0,0,0,0]]}"#
            .to_string(),
        ..Default::default()
    };
    let data = assemble_with(&map).expect("an unnamed code is legal cartridge data");
    let piata = data.map(MapId(0x010)).unwrap();
    assert_eq!(
        piata.collision_at(CellPos::new(2, 1)),
        Some(CollisionType::Unnamed(0xD))
    );
    assert!(!piata.blocks_at(CellPos::new(2, 1)));
}

// ---------------------------------------------------------------------------
// Per-map content failures
// ---------------------------------------------------------------------------

#[test]
fn a_dialogue_tree_outside_the_forty_three_is_refused() {
    // The trees are numbered from one, so both ends are wrong.
    for tree in [0, DIALOGUE_TREE_COUNT + 1, 200] {
        let map = MapJson {
            dialogue_tree: tree.to_string(),
            ..Default::default()
        };
        expect_validation(assemble_with(&map), "dialogue_tree", "does not exist");
    }
    // Both real ends are fine; ten retail maps bind the last tree.
    for tree in [1, DIALOGUE_TREE_COUNT] {
        let map = MapJson {
            dialogue_tree: tree.to_string(),
            ..Default::default()
        };
        assert!(assemble_with(&map).is_ok(), "tree {tree} should be valid");
    }
}

#[test]
fn an_npc_outside_the_map_is_refused() {
    let map = MapJson {
        npcs: r#"[{"index": 0, "record_offset": "0x11C734", "object_id": 60,
                   "symbol": "NPCType2", "x_pixels": 32, "y_pixels": 16,
                   "x_cell": 1, "y_cell": 1, "facing": {"id": 0, "name": "down"},
                   "dialogue_id": 64, "art_tile": 792, "sprite_reason": "synthetic fixture"},
                  {"index": 1, "record_offset": "0x11C73E", "object_id": 76,
                   "symbol": "NPCType6", "x_pixels": 32, "y_pixels": 144,
                   "x_cell": 2, "y_cell": 9, "facing": {"id": 4, "name": "up"},
                   "dialogue_id": 83, "art_tile": 720, "sprite_reason": "synthetic fixture"}]"#
            .to_string(),
        ..Default::default()
    };
    expect_validation(
        assemble_with(&map),
        "npcs[1]",
        "(2, 9) is outside the 4x4 map",
    );
}

#[test]
fn a_chest_outside_the_map_is_refused() {
    let map = MapJson {
        treasure_chests: r#"[{"index": 0, "record_offset": "0x129004",
                              "x_cell": 4, "y_cell": 0, "x_pixels": 64, "y_pixels": 0,
                              "white_chest": true, "object_symbol": "TreasureChest",
                              "contents_type": "meseta", "item_id": null,
                              "item_symbol": null, "meseta": 100, "chest_flag": 25}]"#
            .to_string(),
        ..Default::default()
    };
    expect_validation(
        assemble_with(&map),
        "treasure_chests[0]",
        "(4, 0) is outside",
    );
}

#[test]
fn chest_contents_must_match_the_declared_type() {
    let chest = |contents: &str, item: &str, meseta: &str| {
        format!(
            r#"[{{"index": 0, "record_offset": "0x129004",
                  "x_cell": 0, "y_cell": 0, "x_pixels": 0, "y_pixels": 0,
                  "white_chest": false, "object_symbol": "TreasureChest",
                  "contents_type": "{contents}", "item_id": {item},
                  "item_symbol": null, "meseta": {meseta}, "chest_flag": 1}}]"#
        )
    };

    let missing = MapJson {
        treasure_chests: chest("item", "null", "null"),
        ..Default::default()
    };
    expect_validation(
        assemble_with(&missing),
        "treasure_chests[0].item_id",
        "is null",
    );

    let both = MapJson {
        treasure_chests: chest("meseta", "12", "100"),
        ..Default::default()
    };
    expect_validation(
        assemble_with(&both),
        "treasure_chests[0]",
        "both item_id and meseta",
    );
}

// ---------------------------------------------------------------------------
// Warp failures
// ---------------------------------------------------------------------------

#[test]
fn a_warp_rectangle_running_off_the_map_is_refused() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 3, "y": 3, "width": 4, "height": 1}"#,
            r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        ),
        ..Default::default()
    };
    let text = expect_validation(assemble_with(&map), "warps[0].rect", "outside the 4x4 map");
    assert!(
        text.contains("XYExact") && text.contains("clip"),
        "the message should point at the unclipped range: {text}"
    );
}

#[test]
fn an_empty_warp_rectangle_is_refused() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 1, "y": 1, "width": 0, "height": 1}"#,
            r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        ),
        ..Default::default()
    };
    expect_validation(assemble_with(&map), "warps[0].rect", "can never fire");
}

#[test]
fn a_warp_to_a_map_nobody_has_heard_of_is_refused() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 1, "y": 1, "width": 1, "height": 1}"#,
            r#""target": {"id": 291, "id_hex": "0x123", "symbol": "SomewhereElse"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        ),
        ..Default::default()
    };
    let text = expect_validation(
        assemble_with(&map),
        "warps[0].target",
        "neither packed nor listed",
    );
    assert!(
        text.contains("0x123"),
        "the target id should be hex: {text}"
    );
}

#[test]
fn a_destination_outside_the_target_map_is_refused() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 1, "y": 1, "width": 1, "height": 1}"#,
            r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 8}"#,
        ),
        ..Default::default()
    };
    let text = expect_validation(
        assemble_with(&map),
        "warps[0].destination",
        "outside map 0x011",
    );
    assert!(text.contains("PiataAcademy"), "{text}");
}

#[test]
fn a_transition_table_the_rom_does_not_have_fails_the_parse() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": null"#,
            r#""target": {"id": 17, "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        )
        .replace(r#""table": 2"#, r#""table": 5"#),
        ..Default::default()
    };
    let err = map.try_parse().unwrap_err().to_string();
    assert!(err.contains("tables 1 and 2"), "{err}");
}

// ---------------------------------------------------------------------------
// GameData::load, against a real directory
// ---------------------------------------------------------------------------

/// A throwaway pack directory that deletes itself.
struct TempPack {
    dir: std::path::PathBuf,
}

impl TempPack {
    fn new(name: &str) -> TempPack {
        let dir =
            std::env::temp_dir().join(format!("psiv-data-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("maps")).expect("create the temp pack directory");
        TempPack { dir }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create temp pack dirs");
        }
        std::fs::write(path, contents).expect("write a temp pack file");
    }

    /// Write a complete, valid pack.
    fn populate(&self, version: u32) {
        let piata = MapJson::default();
        let academy = MapJson::academy();
        self.write(
            "manifest.json",
            &manifest_text(version, &[&piata, &academy], SKIPPED_MOTAVIA, ""),
        );
        self.write(&piata.json_name(), &piata.text());
        self.write(&academy.json_name(), &academy.text());
        // Pack format 1 always carries the sprite index files; a minimal pair
        // keeps the synthetic pack loadable without dragging art into tests.
        let empty = format!(
            r#"{{"format_version": {version}, "kind": "field_party", "sheet_count": 0, "sheets": []}}"#
        );
        self.write("sprites/party.json", &empty);
        self.write("sprites/npcs.json", &empty.replace("field_party", "field_npcs"));
    }

    fn load(&self) -> Result<GameData, DataError> {
        GameData::load(&self.dir)
    }
}

impl Drop for TempPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn load_reads_a_directory_of_files() {
    let pack = TempPack::new("happy");
    pack.populate(PACK_FORMAT_VERSION);
    let data = pack.load().expect("a valid pack directory loads");
    assert_eq!(data.len(), 2);
    assert_eq!(data.map(MapId(0x011)).unwrap().label(), "PiataAcademy");
}

#[test]
fn load_fails_when_the_manifest_is_missing() {
    let pack = TempPack::new("no-manifest");
    match pack.load() {
        Err(err @ DataError::Io { .. }) => {
            assert!(err.to_string().contains("manifest.json"), "{err}");
        }
        other => panic!("expected an Io error, got {other:?}"),
    }
}

#[test]
fn load_fails_when_a_listed_map_file_is_missing() {
    let pack = TempPack::new("missing-map");
    pack.populate(PACK_FORMAT_VERSION);
    std::fs::remove_file(pack.dir.join(MapJson::academy().json_name())).unwrap();
    match pack.load() {
        Err(err @ DataError::Io { .. }) => {
            assert!(err.to_string().contains("011_PiataAcademy.json"), "{err}");
        }
        other => panic!("expected an Io error, got {other:?}"),
    }
}

#[test]
fn load_checks_the_format_version_before_reading_any_map() {
    let pack = TempPack::new("bad-version");
    pack.populate(PACK_FORMAT_VERSION + 7);
    // Every map file is deleted, so reaching a map read at all would be an Io
    // error and this test would fail.
    std::fs::remove_dir_all(pack.dir.join("maps")).unwrap();
    match pack.load() {
        Err(DataError::FormatVersion { found, .. }) => {
            assert_eq!(found, PACK_FORMAT_VERSION + 7);
        }
        other => panic!("expected a FormatVersion error, got {other:?}"),
    }
}

#[test]
fn load_fails_on_malformed_json() {
    let pack = TempPack::new("malformed");
    pack.populate(PACK_FORMAT_VERSION);
    pack.write(&MapJson::default().json_name(), "{ this is not json");
    match pack.load() {
        Err(err @ DataError::Json { .. }) => {
            assert!(err.to_string().contains("010_Piata.json"), "{err}");
        }
        other => panic!("expected a Json error, got {other:?}"),
    }
}

#[test]
fn load_catches_a_map_file_that_disagrees_with_its_manifest_entry() {
    let pack = TempPack::new("id-drift");
    pack.populate(PACK_FORMAT_VERSION);
    let impostor = MapJson {
        id: 0x099,
        ..Default::default()
    };
    pack.write(&MapJson::default().json_name(), &impostor.text());
    match pack.load() {
        Err(DataError::ManifestMismatch { field, .. }) => assert_eq!(field, "id"),
        other => panic!("expected a ManifestMismatch error, got {other:?}"),
    }

    let renamed = MapJson {
        symbol: "NotPiata",
        ..Default::default()
    };
    pack.write(&MapJson::default().json_name(), &renamed.text());
    match pack.load() {
        Err(DataError::ManifestMismatch { field, .. }) => assert_eq!(field, "symbol"),
        other => panic!("expected a ManifestMismatch error, got {other:?}"),
    }
}

#[test]
fn a_missing_section_is_an_error_but_a_surplus_one_is_not() {
    let bare = r#"{"format_version": 1, "id": 16, "symbol": "Piata"}"#;
    assert!(serde_json::from_str::<MapRecord>(bare).is_err());

    // The pack already carries fields this crate ignores (`id_hex`,
    // `record_offset`, `item_symbol`, ...); one more must not break a load.
    let with_extra =
        MapJson::default()
            .text()
            .replacen('{', r#"{"encounters": {"mode": "none"}, "#, 1);
    let record: MapRecord =
        serde_json::from_str(&with_extra).expect("unknown fields are ignored, not denied");
    assert_eq!(record.id, MapId(0x010));
}

#[test]
fn a_record_round_trips_through_json() {
    let original = MapJson::default().parse();
    let text = serde_json::to_string(&original).unwrap();
    let back: MapRecord = serde_json::from_str(&text).unwrap();
    assert_eq!(original, back);
    assert_eq!(
        back.collision.grid.rows().next().unwrap(),
        [CollisionType::Normal; 4]
    );
    assert_eq!(
        back.warps[0].rect,
        Some(CellRect {
            x: 1,
            y: 1,
            width: 1,
            height: 1
        })
    );
}

#[test]
fn a_null_dungeon_teleport_index_is_accepted() {
    // Bit 7 of the stored byte means "no index"; nine retail maps set it.
    let map = MapJson {
        flags: r#"{"poison": 1, "random_battles": 1, "town_teleport": 1,
                   "dungeon_teleport_index": null}"#
            .to_string(),
        ..Default::default()
    };
    let data = assemble_with(&map).expect("a null dungeon teleport index is normal");
    assert_eq!(
        data.map(MapId(0x010)).unwrap().flags.dungeon_teleport_index,
        None
    );
}
