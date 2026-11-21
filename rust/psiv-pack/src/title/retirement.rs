//! Emitted title assets, preserving TestTitlePack and numeric layout contracts.
use super::*;
use crate::test_support::{retail_rom, scratch};
use serde_json::{Value, json};

#[test]
fn title_emits_all_assets_layout_placements_and_last_frame_replay() {
    let Some(rom) = retail_rom("title assets") else {
        return;
    };
    let root = scratch("title");
    // build() has only ROM bytes and an empty output root: no oracle input.
    let fragment = Value::from(&build(&rom, &root).unwrap());
    let title = &fragment["title"];
    assert_eq!(title["layout"]["path"], "title/layout.json");
    assert_eq!(title["background"]["png"], "title/background.png");
    assert_eq!(
        title["background_transfer"]["png"],
        "title/titlebarbgtoppart.png"
    );
    let layout: Value =
        serde_json::from_slice(&std::fs::read(root.join("title/layout.json")).unwrap()).unwrap();
    assert_eq!(
        layout["screen"],
        json!({"width_pixels":320,"height_pixels":224})
    );
    assert_eq!(layout["background"]["placements"][0]["x_pixels"], 0);
    assert_eq!(
        layout["background"]["placements"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["x_pixels"],
        112
    );
    assert_eq!(
        layout["background_transfer"]["size_pixels"],
        json!([64, 224])
    );
    assert_eq!(layout["palette_cycle"]["frames"], json!(boot::FRAMES));
    for (name, (x, y, w, h)) in [
        ("sega_logo", (12, 11, 17, 5)),
        ("title_logo", (11, 3, 17, 13)),
        ("subtitle", (5, 17, 29, 3)),
        ("press_start", (11, 22, 18, 1)),
        ("copyright", (12, 25, 17, 1)),
    ] {
        assert_eq!(
            layout["placement_contract"][name],
            json!({"x_cell":x,"y_cell":y,"width_cells":w,"height_cells":h})
        );
        let bytes =
            std::fs::read(root.join(title["assets"][name]["png"].as_str().unwrap())).unwrap();
        let image = png::decode(&bytes).unwrap();
        assert_eq!(
            (image.width, image.height),
            ((w * 8) as usize, (h * 8) as usize)
        );
    }
    for path in [
        "title/replay/frame_650/background.png",
        "title/replay/frame_650/title_logo.png",
        "title/titlebarbgtoppart.png",
        "title/sound_test.json",
    ] {
        assert!(root.join(path).is_file());
    }
    let st: Value =
        serde_json::from_slice(&std::fs::read(root.join("title/sound_test.json")).unwrap())
            .unwrap();
    assert_eq!(st["track_count"], 48);
    let tracks = st["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 48);
    assert_eq!(
        crate::test_support::text_pin(tracks[0]["title"].as_str().unwrap()),
        (
            25,
            "acb4a5a1446b1b7f8457b61a7ab4c479f77ff3c3f8ee61d77fba8a520196083c".into()
        )
    );
    assert_eq!(tracks[0]["sound_id"], 0xAC);
    assert_eq!(
        crate::test_support::text_pin(tracks[0]["composer"].as_str().unwrap()),
        (
            4,
            "f315e35819d70f38f1ec406cef7e2e8df4323409a711023af6086735566f7d8f".into()
        )
    );
    assert_eq!(
        crate::test_support::text_pin(tracks[2]["composer"].as_str().unwrap()),
        (
            9,
            "87b28a072f555836bfe8f4b52ffc1b8e1447a60d4bfed8a64f2f21f915533db8".into()
        )
    );
    assert_eq!(
        crate::test_support::text_pin(tracks[47]["title"].as_str().unwrap()),
        (
            8,
            "3152c0b03720c9b8c2e1651a2f33f736ba3255e1dff4f7ae6189afe0c77ef945".into()
        )
    );
    assert_eq!(st["repeat_override"]["when_sound_id"], 0xB1);
    assert_eq!(st["repeat_override"]["play_sound_id"], 0xA7);

    // Negative controls: opcode corruption in Title_SoundTestLoop is refused.
    for offset in [
        sound_test::TRACK_MAX_CMPI,
        sound_test::PLAY_TABLE_LEA,
        sound_test::STRINGS_LEA,
        sound_test::REPEAT_CHECK_CMPI,
    ] {
        let mut corrupt = rom.bytes().to_vec();
        corrupt[offset] ^= 1;
        assert!(
            sound_test::extract(&corrupt)
                .unwrap_err()
                .0
                .contains("opcode mismatch"),
            "corrupting 0x{offset:06X} must fail"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
