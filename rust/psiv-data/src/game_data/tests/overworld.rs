//! Page hooks: a stale pack, a bad collision plane and a bad atlas tile.

use super::fixtures::MapJson;
use crate::MapId;
use crate::game_data::maps::validate_map;
use crate::map::MapRecord;

fn synthetic_page_hook_record() -> MapRecord {
    let mut value: serde_json::Value = serde_json::from_str(&MapJson::default().text()).unwrap();
    value["png_over"] = serde_json::json!("maps/synthetic_over.png");
    value["layout_patches"] = serde_json::json!([{
        "triggered_by_plane": "bg", "page": 0, "routine": "0x000100",
        "event_flag": {"id": 53, "id_hex": "0x35", "symbol": null},
        "writes": [
            {"plane": "fg", "chunk_x": 0, "chunk_y": 0,
             "cell_x": 0, "cell_y": 0, "chunk_ids": ["0x00"], "displacement": 0},
            {"plane": "bg", "chunk_x": 0, "chunk_y": 0,
             "cell_x": 0, "cell_y": 0, "chunk_ids": ["0x48"], "displacement": 0}
        ]
    }]);
    value["overworld_patches"] = serde_json::json!([{
        "event_flag": 53,
        "tiles": [{"chunk_x": 0, "chunk_y": 0, "fg_chunk_id": 0,
                   "bg_chunk_id": 72, "collision_chunk_id": 72,
                   "collision": [0,0,0,0], "patch_tile": 0}]
    }]);
    value["patch_tiles"] = serde_json::json!({
        "png": "maps/synthetic_patch.png",
        "png_over": "maps/synthetic_patch_over.png",
        "tile_pixels": 32, "count": 1,
        "tiles": [{"index": 0, "chunk_id": null, "fg_chunk_id": 0,
                   "bg_chunk_id": 72, "x": 0, "priority_tiles": 0,
                   "collision": [0,0,0,0]}]
    });
    serde_json::from_value(value).expect("synthetic page hook parses")
}

#[test]
fn page_hook_schema_rejects_stale_pack_and_bad_collision_or_atlas() {
    let valid = synthetic_page_hook_record();
    validate_map(&valid).expect("synthetic two-plane hook is internally coherent");

    let mut stale = valid.clone();
    stale.overworld_patches = None;
    assert!(
        validate_map(&stale)
            .unwrap_err()
            .to_string()
            .contains("rebuild")
    );

    let mut erased = valid.clone();
    erased.id = MapId(0);
    erased.layout_patches.clear();
    erased.overworld_patches = None;
    assert!(
        validate_map(&erased)
            .unwrap_err()
            .to_string()
            .contains("rebuild")
    );

    let mut wrong_plane = valid.clone();
    wrong_plane.overworld_patches.as_mut().unwrap()[0].tiles[0].collision_chunk_id = 0;
    assert!(
        validate_map(&wrong_plane)
            .unwrap_err()
            .to_string()
            .contains("collision plane")
    );

    let mut wrong_atlas = valid.clone();
    wrong_atlas.overworld_patches.as_mut().unwrap()[0].tiles[0].patch_tile = 1;
    assert!(
        validate_map(&wrong_atlas)
            .unwrap_err()
            .to_string()
            .contains("atlas index")
    );

    let mut overlap = valid;
    let mut second = overlap.layout_patches[0].clone();
    second.event_flag.id = 54;
    second.event_flag.id_hex = "0x36".into();
    overlap.layout_patches.push(second);
    assert!(
        validate_map(&overlap)
            .unwrap_err()
            .to_string()
            .contains("overlapping")
    );
}
