//! Dimension and collision failures, per-map content failures and warp
//! failures: one record's own numbers and geometry, each refused with the field
//! that is wrong.

use super::fixtures::{MapJson, assemble_with, expect_validation, warp_json};
use crate::MapId;
use crate::collision::CollisionType;
use crate::map::{CellPos, DIALOGUE_TREE_COUNT};

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
