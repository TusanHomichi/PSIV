use super::*;
use serde_json::json;

#[test]
fn command_and_damage_streams_retain_vram_addresses_rows_and_palette() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    for (key, specs) in [
        (
            "command_ui",
            &[
                ("icons", 0x27D860, 30, 0x15E),
                ("cursor", 0x27DA70, 8, 0x17C),
            ][..],
        ),
        ("damage_digits", &[("digits", 0x27D778, 28, 0x5E4)][..]),
    ] {
        let file = &e.indexes[key];
        let tiles = array(&file["tiles"]);
        assert_eq!(file["tile_count"], specs.iter().map(|s| s.2).sum::<usize>());
        assert_eq!(array(&file["streams"]).len(), specs.len());
        let mut position = 0;
        for (stream, &(name, at, count, first)) in array(&file["streams"]).iter().zip(specs) {
            assert_eq!(stream["name"], name);
            assert_eq!(stream["rom_offset"], format!("0x{at:X}"));
            assert_eq!(stream["tile_count"], count);
            assert_eq!(stream["first_vram_tile"], first);
            let (raw, _) = crate::decode::nemesis::decompress(&r.rom, at).unwrap();
            for (index, tile) in tiles[position..position + count].iter().enumerate() {
                assert_eq!(tile["vram_tile"], first + index);
                let rows = array(&tile["rows"]);
                assert_eq!(rows.len(), 8);
                for (y, row) in rows.iter().enumerate() {
                    let text = row.as_str().unwrap();
                    assert_eq!(text.len(), 8);
                    for (x, pair) in text.as_bytes().chunks(2).enumerate() {
                        assert_eq!(
                            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap(),
                            raw[index * 32 + y * 4 + x]
                        );
                    }
                }
            }
            position += count;
        }
        assert_eq!(position, tiles.len());
    }
    let words: Vec<_> = (0..15)
        .map(|i| crate::battle::image_tests::word(&r.rom, 0x76A4 + i * 2))
        .collect();
    assert_eq!(e.indexes["command_ui"]["palette_line_3"], json!(words));
}

#[test]
fn local_oracle_vram_and_cram_match_emitted_command_and_digit_tiles() {
    let Some(e) = emission() else { return };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/states/battle_command_idle_vdp_25000.json");
    if !path.try_exists().unwrap() {
        eprintln!("local command VDP state absent: tile oracle comparison skipped");
        return;
    }
    let state: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let bytes = |value: &Value| {
        value
            .as_str()
            .unwrap()
            .as_bytes()
            .chunks(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let vram = bytes(&state["regions"]["vdp_vram"]["bytes_hex"]);
    let cram = bytes(&state["regions"]["cram"]["bytes_hex"]);
    for key in ["command_ui", "damage_digits"] {
        for tile in array(&e.indexes[key]["tiles"]) {
            let raw = bytes(&Value::String(
                array(&tile["rows"])
                    .iter()
                    .map(|r| r.as_str().unwrap())
                    .collect(),
            ));
            let at = number(&tile["vram_tile"]) * 32;
            assert!(raw == vram[at..at + 32], "tile differs from oracle VRAM");
        }
    }
    let words: Vec<_> = cram[0x62..0x80]
        .chunks_exact(2)
        .map(|p| u16::from_be_bytes([p[0], p[1]]))
        .collect();
    assert_eq!(e.indexes["command_ui"]["palette_line_3"], json!(words));
}
