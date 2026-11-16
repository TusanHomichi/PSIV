use super::*;
use crate::dialogue::retirement::emission;
use serde_json::{Value, json};

#[test]
fn frame_words_flips_and_font_overwrite_keep_the_border_tiles() {
    let words = [0xC6E9, 0xCEE9, 0xD6E9, 0xDEE9];
    for (i, w) in words.into_iter().enumerate() {
        let p = Value::from(&pattern_word(w));
        assert_eq!(p["vram_tile"], 0x6E9);
        assert_eq!(p["priority"], true);
        assert_eq!(p["cram_line"], 2);
        assert_eq!(p["flip_h"], i % 2 == 1);
        assert_eq!(p["flip_v"], i >= 2);
    }
    let roles = Value::from(&frame_roles().unwrap());
    let roles = roles.as_object().unwrap();
    assert_eq!(roles.len(), 9);
    for role in roles.values() {
        let tile = role["tile"].as_u64().unwrap();
        assert!([0, 105, 106, 115].contains(&tile));
        assert!(!(1..88).contains(&tile));
        assert_eq!(role["x"], tile * 8);
        assert_eq!(role["priority"], true);
        assert_eq!(role["cram_line"], 2);
    }
    assert_eq!(roles["fill"]["tile"], 0);
    assert_eq!(roles["edge_left"]["tile"], roles["edge_right"]["tile"]);
    assert_eq!(roles["edge_right"]["flip_h"], true);
}

#[test]
fn chrome_pixels_records_geometry_and_menu_boundaries_match_retail() {
    let Some(e) = emission() else { return };
    let rom = e.rom.bytes();
    let w = e.document("window");
    let image = png::decode(&e.files["dialogue/window.png"]).unwrap();
    assert_eq!((image.width, image.height), (1024, 8));
    assert!(image.trns.is_empty());
    assert!(!image.pixels.contains(&0));
    assert_eq!(w["source"]["vram_tile"], "0x680");
    assert_eq!(w["source"]["tile_count"], 128);
    let tile = |name: &str| {
        let x = w["roles"][name]["x"].as_u64().unwrap() as usize;
        (0..8)
            .map(|y| image.pixels[y * image.width + x..y * image.width + x + 8].to_vec())
            .collect::<Vec<_>>()
    };
    assert!(tile("fill").iter().flatten().all(|p| *p == 14));
    assert_eq!(&image.plte[42..45], &[0, 32, 98]);
    let corner = tile("corner_top_left");
    assert!(corner[0].iter().all(|p| *p == 14));
    assert!(corner[1][1..].iter().all(|p| *p != 14));
    assert!(corner.iter().all(|r| r[0] == 14));
    let edge = tile("edge_top");
    assert!(edge[0].iter().chain(&edge[4]).all(|p| *p == 14));
    assert!(edge[1].iter().all(|p| *p != 14));
    assert!(tile("edge_left").iter().all(|r| {
        r.iter()
            .enumerate()
            .all(|(x, p)| (*p != 14) == (1..5).contains(&x))
    }));
    assert_eq!(w["palette"]["fill_index"], 14);
    assert_eq!(w["palette"]["cram_line"], 2);
    let records = window_records(rom).unwrap();
    assert_eq!(
        w["windows"],
        Value::from(&Json::from(
            records.iter().map(|r| r.0.clone()).collect::<Vec<_>>()
        ))
    );
    assert_eq!(
        records
            .iter()
            .map(|r| r.0.get("name").unwrap().clone())
            .collect::<Vec<_>>(),
        ["controls", "dialogue", "portrait", "yes_no"].map(Json::from)
    );
    assert_eq!(records[1].1, [3, 20, 34, 6]);
    assert_eq!(
        records[1].0.get("record_offset"),
        Some(&Json::from("0x069388"))
    );
    assert_eq!(
        w["windows"][1]["rect"],
        json!({"x":24,"y":160,"width":272,"height":48})
    );
    assert_eq!(
        w["windows"][2]["rect"],
        json!({"x":40,"y":104,"width":48,"height":48})
    );
    let menus = Value::from(&Json::from(menu_records(rom).unwrap()));
    assert_eq!(w["menu_windows"], menus);
    assert_eq!(menus.as_array().unwrap().len(), 312);
    let cells = |r: &Value| {
        json!([
            r["x_cell"],
            r["y_cell"],
            r["width_cells"],
            r["height_cells"]
        ])
    };
    assert_eq!(cells(&menus[0]), json!([4, 2, 9, 15]));
    assert_eq!(cells(&menus[1]), json!([3, 23, 13, 3]));
    for (i, r) in menus.as_array().unwrap().iter().enumerate() {
        assert_eq!(r["index"], i);
    }
    for i in [0x45, 0x65, 0x75] {
        assert_eq!(cells(&menus[i]), cells(&w["windows"][0]));
    }
    assert_eq!(
        w["windows"][1]["interior"],
        json!({"x_cell":4,"y_cell":21,"width_cells":32,"height_cells":4})
    );
    assert_eq!(w["geometry"]["border_cells"], 1);
    assert_eq!(w["geometry"]["shadow"], false);
    assert_eq!(w["geometry"]["open_animation"]["step_cells"], 2);
    assert_eq!(w["geometry"]["open_animation"]["from"], "center");
    let mut corrupt = rom.to_vec();
    corrupt[WIN_GROUP_OFFSETS + 2 * DIALOGUE_ENTRY + 1] ^= 8;
    assert!(menu_records(&corrupt).is_err());
    corrupt = rom.to_vec();
    corrupt[WIN_GROUP_DIALOGUE + RECORD_SIZE] ^= 2;
    let root = crate::test_support::scratch("bad-window");
    assert!(
        write(
            &corrupt,
            &root,
            &crate::dialogue::palette(rom).unwrap(),
            &crate::json::Json::Null
        )
        .unwrap_err()
        .0
        .contains("interior")
    );
    std::fs::remove_dir_all(root).unwrap();
}
