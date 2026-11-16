use super::*;
use crate::dialogue::retirement::emission;
use serde_json::json;

#[test]
fn font_pixels_indices_blanks_and_palette_match_local_source() {
    let Some(e) = emission() else { return };
    let f = e.document("font");
    let image = png::decode(&e.files["dialogue/font.png"]).unwrap();
    assert_eq!((image.width, image.height, image.colour), (640, 16, 3));
    assert_eq!(image.trns[14], 0);
    assert_eq!(f["glyphs"].as_array().unwrap().len(), 80);
    assert_eq!(f["palette"]["cram_line"], 2);
    assert_eq!(f["palette"]["colors"][14], json!([0, 32, 98]));
    let raw = &e.rom.bytes()[FONT..FONT + FONT_SIZE];
    for (i, g) in f["glyphs"].as_array().unwrap().iter().enumerate() {
        assert_eq!(g["byte"], i);
        assert_eq!(g["x"], i * 8);
        assert_eq!(
            g["char"],
            json!(dialogue_charset().glyph(i as u8).map(|c| c.to_string()))
        );
        for y in 0..16 {
            for x in 0..8 {
                assert_eq!(
                    image.pixels[y * 640 + i * 8 + x],
                    if raw[i * 16 + y] & (0x80 >> x) != 0 {
                        15
                    } else {
                        14
                    }
                );
            }
        }
    }
    assert_eq!(
        image
            .pixels
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([14, 15])
    );
    for (c, i) in [("A", 1), ("a", 27), ("0", 64), (" ", 0), (".", 0x35)] {
        assert_eq!(f["by_char"][c], i);
    }
    assert_eq!(f["unmapped_glyphs"], json!([78, 79]));
    let blank: Vec<_> = f["glyphs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["blank"] == true)
        .map(|g| g["byte"].clone())
        .collect();
    assert_eq!(
        blank,
        json!([0, 74, 75, 76, 77, 78, 79])
            .as_array()
            .unwrap()
            .to_vec()
    );
    for y in [0, 1, 2, 13, 14, 15] {
        assert!(
            image.pixels[y * 640 + 8..y * 640 + 16]
                .iter()
                .all(|p| *p == 14)
        );
    }
}

#[test]
fn portraits_cover_used_ids_deduplicate_art_and_decode_to_source_pixels() {
    let Some(e) = emission() else { return };
    let p = e.document("portraits");
    let portraits = p["portraits"].as_array().unwrap();
    assert_eq!(p["count"], 39);
    assert_eq!(p["distinct_art"], 36);
    assert_eq!(p["palette"]["cram_line"], 2);
    assert_eq!(p["geometry"]["talk_slot_stride_tiles"], 12);
    for (i, p) in portraits.iter().enumerate() {
        assert_eq!(p["id"], i + 1);
        let path = p["png"].as_str().unwrap();
        assert!(path.starts_with("dialogue/portraits/"));
        let bytes = &e.files[path];
        let image = png::decode(bytes).unwrap();
        assert_eq!((image.width, image.height), (48, 48));
        assert_eq!(image.trns[0], 0);
        assert_eq!(p["png_sha256"], png::image_sha256(bytes).unwrap());
        assert_eq!(p["art"]["tile_count"], 36);
        let at = usize::from_str_radix(
            p["art"]["rom_offset"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();
        let decoded = crate::decode::Decodes::new(e.rom.bytes())
            .get(crate::decode::Call::Nemesis { offset: at })
            .unwrap();
        let raw = decoded.bytes();
        // Six row-major tiles across; independently expand each nibble.
        for y in 0..48 {
            for x in 0..48 {
                let byte = raw[((y / 8) * 6 + x / 8) * 32 + (y % 8) * 4 + (x % 8) / 2];
                assert_eq!(
                    image.pixels[y * 48 + x],
                    if x % 2 == 0 { byte >> 4 } else { byte & 15 }
                );
            }
        }
    }
    assert_eq!(portraits[0]["symbol"], "Chaz");
    assert_eq!(portraits[0]["png"], "dialogue/portraits/01_Chaz.png");
    assert_eq!(portraits[0]["art"]["rom_offset"], "0x29BE5C");
    assert!(portraits[0]["duplicate_of"].is_null());
    for (a, b) in [(22, 23), (24, 25), (30, 31)] {
        assert_eq!(portraits[b - 1]["duplicate_of"], a);
        assert_eq!(
            portraits[a - 1]["png_sha256"],
            portraits[b - 1]["png_sha256"]
        );
    }
    assert_eq!(
        portraits
            .iter()
            .filter(|p| p["duplicate_of"].is_null())
            .count(),
        36
    );
}

#[test]
fn arrow_geometry_mapping_transparency_and_source_tiles_match() {
    let Some(e) = emission() else { return };
    let d = e.document("trees");
    let w = e.document("window");
    let a = &d["window"]["scroll_arrow"];
    assert_eq!(w["scroll_arrow"], *a);
    assert_eq!(e.fragment["chrome"]["scroll_arrow"], *a);
    for (k, v) in [
        ("screen_x", 264),
        ("screen_y", 202),
        ("width", 16),
        ("height", 8),
    ] {
        assert_eq!(a[k], v);
    }
    let image = png::decode(&e.files["dialogue/scroll_arrow.png"]).unwrap();
    assert_eq!((image.width, image.height), (16, 8));
    assert!(!image.trns.is_empty());
    assert!(image.pixels.contains(&1));
    assert!(image.pixels.contains(&15));
    let s = &a["source"];
    assert_eq!(s["label"], "ArtNem_Font");
    assert_eq!(s["rom_offset"], "0x2A303A");
    assert_eq!(s["sprite_vram_tile"], "0x7F6");
    assert_eq!(s["tile_indices"], json!(["0x36", "0x37"]));
    // Mapping bytes come from the local ROM, never a committed derived fixture.
    assert_eq!(
        s["mapping_hex"],
        png::hex(&e.rom.bytes()[ARROW_MAPPING..ARROW_MAPPING + ARROW_MAPPING_PIN.0])
    );
    let art = menu_font_art(e.rom.bytes()).unwrap();
    for y in 0..8 {
        for x in 0..16 {
            let tile = 0x36 + x / 8;
            let b = art.data[tile * 32 + y * 4 + (x % 8) / 2];
            assert_eq!(
                image.pixels[y * 16 + x],
                if x % 2 == 0 { b >> 4 } else { b & 15 }
            );
        }
    }
}
