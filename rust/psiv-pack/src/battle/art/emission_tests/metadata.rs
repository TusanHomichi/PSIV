use super::*;
use serde_json::json;

#[test]
fn enemy_body_dimensions_palette_words_and_bank_metadata() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let file = &e.indexes["enemies"];
    let entries = array(&file["enemies"]);
    assert_eq!(entries.len(), 153);
    assert_eq!(file["art_bank_order"], json!([3, 1, 2]));
    let mut used = BTreeSet::new();
    let mut complete = 0;
    let mut holes = 0;
    let mut raw_ids = Vec::new();
    for (id, entry) in entries.iter().enumerate() {
        assert_eq!(entry["id"], id);
        assert_eq!(
            number(&entry["width_cells"]),
            2 * number(&entry["half_width_cells"])
        );
        let p = image(e, entry);
        assert_eq!(
            (p.width, p.height),
            (
                number(&entry["width_cells"]) * 8,
                number(&entry["height_cells"]) * 8
            )
        );
        assert_eq!(
            (p.width, p.height),
            (
                number(&entry["width_pixels"]),
                number(&entry["height_pixels"])
            )
        );
        assert!(
            entry["png"]
                .as_str()
                .unwrap()
                .starts_with("battle/art/enemies/")
        );
        assert_eq!(p.colour, 3);
        assert_eq!(p.plte.len(), 48);
        assert_eq!(&p.plte[45..48], &[0, 0, 0]);
        assert_eq!(p.trns, &[0]);
        used.extend(p.pixels);
        let words = array(&entry["palette_words"]);
        assert_eq!(words.len(), 11);
        for (index, word) in words.iter().enumerate() {
            let at = 0x27FFA2 + id * 22 + index * 2;
            assert_eq!(
                word,
                &format!("0x{:04X}", crate::battle::image_tests::word(&r.rom, at))
            );
        }
        let body_holes = number(&entry["body_holes"]);
        assert_eq!(entry["body_complete"], body_holes == 0);
        complete += usize::from(body_holes == 0);
        holes += body_holes;
        if entry["mapping"]["format"] == "raw_words" {
            raw_ids.push(id);
        }
        let art = array(&entry["art"]);
        assert_eq!(art.len(), 3);
        assert_eq!(entry["base_pattern"], 0);
        assert!(number(&entry["patterns_used"]) <= number(&entry["bank_patterns"]));
        assert_eq!(
            art.iter().map(|a| number(&a["first_pattern"])).min(),
            Some(0)
        );
    }
    assert_eq!((complete, entries.len() - complete, holes), (84, 69, 303));
    assert_eq!(raw_ids, [0x86, 0x87]);
    assert_eq!(file["raw_mapping_enemy_ids"], json!(raw_ids));
    assert_eq!(used, (0..14).collect());
    assert_eq!(
        file["palette_layout"]["body_color_indices_used"],
        json!(used)
    );
    assert_eq!(file["palette_layout"]["cram_lines"], json!([1, 2]));
    for (at, line) in [(0x7672, 0), (0x7678, 1), (0x767E, 2)] {
        let word = crate::battle::image_tests::word(&r.rom, at);
        assert_ne!(word, 0);
        assert_eq!(
            file["palette_layout"]["index_15_by_cram_line"][line.to_string()],
            format!("0x{word:04X}")
        );
    }
}

#[test]
fn poses_are_indexed_advisory_and_refer_to_their_own_art_and_palette() {
    let Some(e) = emission() else { return };
    let file = &e.indexes["characters"];
    let entries = array(&file["characters"]);
    assert_eq!(entries.len(), 11);
    assert_eq!(file["pose_count"], 43);
    assert_eq!(file["grid_cells"], json!([6, 6]));
    assert_eq!(file["pose_labels"]["advisory"], true);
    let mut poses = 0;
    let mut multiple = Vec::new();
    for (id, entry) in entries.iter().enumerate() {
        let art = array(&entry["art"]);
        if art.len() > 1 {
            multiple.push(id);
        }
        let list = array(&entry["poses"]);
        assert_eq!(entry["pose_count"], list.len());
        assert_eq!(list[0]["label"], "Idle");
        for (index, pose) in list.iter().enumerate() {
            assert_eq!(pose["pose"], index);
            assert_eq!(pose["label_source"], "ps4disasm");
            assert!(!pose["label"].as_str().unwrap().is_empty());
            assert!(
                art.iter()
                    .any(|a| a["rom_offset"] == pose["art_rom_offset"])
            );
            let p = image(e, pose);
            assert_eq!((p.width, p.height), (48, 48));
            assert!(
                pose["png"]
                    .as_str()
                    .unwrap()
                    .starts_with("battle/art/characters/")
            );
        }
        poses += list.len();
    }
    assert_eq!(poses, 43);
    assert_eq!(multiple, [6, 7]);
    let palette = &file["palette"];
    assert_eq!(palette["cram_line"], 3);
    assert_eq!(palette["png_baked_variant"], 0);
    assert_eq!(array(&palette["ui_colors"]).len(), 3);
    let variants = array(&palette["variants"]);
    assert_eq!(variants.len(), 4);
    assert_eq!(palette["table"]["entry_count"], variants.len());
    for (variant, name) in variants
        .iter()
        .zip(["on_foot", "land_rover", "ice_digger", "hydrofoil"])
    {
        assert_eq!(variant["name"], name);
        assert_eq!(array(&variant["words"]).len(), 15);
        assert_eq!(array(&variant["colors"]).len(), 16);
    }
}
