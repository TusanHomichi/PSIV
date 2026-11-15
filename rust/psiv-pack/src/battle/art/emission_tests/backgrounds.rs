use super::*;
use serde_json::json;

#[test]
fn background_planes_palettes_selection_tables_and_reachability() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let file = &e.indexes["backgrounds"];
    let entries = array(&file["backgrounds"]);
    assert_eq!(entries.len(), 32);
    assert_eq!(file["distinct_art_blobs"], 20);
    assert_eq!(
        entries
            .iter()
            .map(|e| hex(&e["art"]["rom_offset"]))
            .collect::<BTreeSet<_>>()
            .len(),
        20
    );
    assert_eq!(file["plane_size_cells"], json!([64, 24]));
    let selection = &file["selection"];
    assert_eq!(
        selection["order"],
        json!(["event_battle", "field_map", "motavia_terrain"])
    );
    assert_eq!(
        selection["vehicle_mounted"]["selector"],
        "Vehicle_Index != 0"
    );
    for (key, at, count) in [
        ("event_battle", 0x6C8C, 27),
        ("field_map", 0x6CA8, 416),
        ("motavia_terrain", 0x586EC, 214),
    ] {
        let block = &selection[key];
        assert_eq!(block["rom_offset"], format!("0x{at:06X}"));
        assert_eq!(block["count"], count);
        assert_eq!(block["indexes"], json!(&r.rom[at..at + count]));
    }
    assert_eq!((0x6C8C + 27 + 1) & !1, 0x6CA8);
    assert_eq!(0x6CA8 + 416, 0x6E48);
    assert_eq!(0x586EC + 214, 0x587C2);
    assert_eq!(&r.rom[0x587C2..0x587C4], &[0x60, 0]);
    assert_eq!(selection["field_map"]["none"], 255);
    let field = array(&selection["field_map"]["indexes"]);
    assert_eq!(field.iter().filter(|&v| v == 255).count(), 263);
    assert_eq!(field[0], 255);
    assert_eq!(field[1], 4);
    assert_eq!(field.len(), 416);
    assert!(0x1A0 > field.len() - 1);
    let mota = array(&selection["motavia_terrain"]["indexes"]);
    assert_eq!(selection["motavia_terrain"]["stored_bias"], 1);
    assert!(mota.iter().all(|v| number(v) <= 4));
    let resolved: BTreeSet<_> = mota.iter().map(|v| number(v).saturating_sub(1)).collect();
    assert_eq!(resolved, (0..4).collect());
    // Public disassembly symbols, independent of the emitter's label table.
    assert_eq!(
        entries[..4]
            .iter()
            .map(|entry| entry["symbol"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["MotaDesert", "MotaBeach", "MotaGrass", "MotaSea"]
    );
    assert!(
        mota[42..]
            .iter()
            .map(|v| number(v).saturating_sub(1))
            .collect::<BTreeSet<_>>()
            .len()
            > 1
    );
    assert_eq!(selection["dark_force_2_swap"]["from"], 4);
    assert_eq!(selection["dark_force_2_swap"]["to"], 5);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(entry["index"], index);
        let p = image(e, entry);
        assert_eq!((p.width, p.height), (512, 192));
        assert!(
            entry["png"]
                .as_str()
                .unwrap()
                .starts_with("battle/art/backgrounds/")
        );
        let palette = &entry["palette"];
        assert_eq!(palette["cram_line"], 0);
        assert_eq!(palette["first_index"], 1);
        assert_eq!(palette["color_count"], 13);
        assert_eq!(palette["index_0_forced_black"], true);
        assert_eq!(array(&palette["colors"]).len(), 13);
        let at = hex(&palette["rom_offset"]);
        let colors = crate::battle::image_tests::palette(&r.rom, at, 13);
        assert_eq!(palette["colors"], json!(colors));
        assert_eq!(&p.plte[..3], &[0, 0, 0]);
        assert_eq!(&p.plte[3..42], colors.as_flattened());
        assert_eq!(entry["selectable"], true);
        let counts = &entry["selected_by"];
        let mut total = 0;
        for (key, table) in [
            ("event_battles", "event_battle"),
            ("maps", "field_map"),
            ("motavia_terrain", "motavia_terrain"),
        ] {
            let count = array(&selection[table]["indexes"])
                .iter()
                .filter(|v| {
                    let stored = number(v);
                    (if table == "motavia_terrain" {
                        stored.saturating_sub(1)
                    } else {
                        stored
                    }) == index
                })
                .count();
            assert_eq!(counts[key], count);
            total += count;
        }
        if index == 5 {
            assert_eq!(total, 0);
        } else {
            assert!(total > 0);
        }
    }
}

#[test]
fn local_disassembly_lists_the_full_selection_tables_including_motavia_tail() {
    let Some(r) = retail() else { return };
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm/ps4.asm");
    if !path.try_exists().unwrap() {
        eprintln!("local disassembly absent: background table comparison skipped");
        return;
    }
    let text = std::fs::read_to_string(path).unwrap();
    for (label, at, count, end) in [
        (
            "EventBattleBGIndexes",
            0x6C8C,
            27,
            "Battle_BackgroundIndexes",
        ),
        ("Battle_BackgroundIndexes", 0x6CA8, 416, "loc_6E48"),
        (
            "MotaBattleBGIndexes",
            0x586EC,
            214,
            "FieldRoutine_Interaction",
        ),
    ] {
        let start = text.find(&format!("\n{label}:")).unwrap();
        let tail = &text[start..];
        let stop = tail.find(&format!("\n{end}:")).unwrap();
        let mut bytes = Vec::new();
        for line in tail[..stop].lines() {
            let line = line.split(';').next().unwrap().trim();
            let (width, values) = if let Some(v) = line.strip_prefix("dc.b") {
                (1, v)
            } else if let Some(v) = line.strip_prefix("dc.w") {
                (2, v)
            } else {
                continue;
            };
            for value in values.split(',') {
                let value = value.trim();
                let word = if let Some(v) = value.strip_prefix('$') {
                    u16::from_str_radix(v, 16).unwrap()
                } else {
                    value.parse::<u16>().unwrap()
                };
                if width == 2 {
                    bytes.extend(word.to_be_bytes());
                } else {
                    bytes.push(word as u8);
                }
            }
        }
        // Event table's alignment byte precedes the next table.
        assert_eq!(bytes.len(), count);
        assert!(
            bytes == r.rom[at..at + count],
            "{label} differs from retail"
        );
    }
}
