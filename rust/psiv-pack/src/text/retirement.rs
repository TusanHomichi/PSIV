//! Text contracts retained from test_text.py (#559). No cartridge strings or
//! content digests are pinned: local reference directives provide the oracle.
use super::*;
use crate::test_support::retail_rom;
use serde_json::{Value, json};
use std::collections::BTreeSet;

mod source;

fn decoded(bytes: &[u8], font: &str, slots: u8) -> Value {
    Value::from(&decode_string(bytes, font, slots).unwrap())
}
fn unhex_value(v: &Value) -> Vec<u8> {
    unhex(v.as_str().unwrap())
}

#[test]
fn both_fonts_control_table_and_retail_actions_keep_their_domains() {
    for c in 1..=26 {
        assert_eq!(window_charset().glyph(c), dialogue_charset().glyph(c));
    }
    for font in [window_charset(), dialogue_charset()] {
        assert_eq!(font.glyph(0), Some(' '));
    }
    assert_eq!(window_charset().glyph(27), Some('0'));
    assert_eq!(window_charset().glyph(57), Some('a'));
    assert_eq!(dialogue_charset().glyph(27), Some('a'));
    assert_eq!(dialogue_charset().glyph(64), Some('0'));
    assert_eq!(
        (0..=255)
            .filter(|&b| dialogue_charset().glyph(b).is_some())
            .collect::<Vec<_>>(),
        (0..78).collect::<Vec<_>>()
    );
    assert_eq!(
        (0..=255).rfind(|&b| window_charset().glyph(b).is_some()),
        Some(0x56)
    );
    for b in (0x25..=0x30).chain(0x35..=0x38) {
        assert_eq!(window_charset().glyph(b), None);
    }
    assert_eq!(CONTROL_CODES.len(), 16);
    assert_eq!(CONTROL_CODES[12].name, "newline");
    assert_eq!(CONTROL_CODES[15].name, "terminate");
    assert_eq!(
        CONTROL_CODES
            .iter()
            .enumerate()
            .filter(|(_, c)| c.operand_bytes.is_none())
            .map(|(i, _)| i + 0xF0)
            .collect::<Vec<_>>(),
        [0xF2, 0xF4]
    );
    assert_eq!(TEXT_ACTIONS.len(), 13);
}

#[test]
fn decoding_preserves_fonts_segments_operands_and_variable_lengths() {
    assert_eq!(decode_name(&[1, 27, 64], WINDOW).unwrap(), "A0h");
    assert_eq!(decode_name(&[1, 27, 64], DIALOGUE).unwrap(), "Aa0");
    let d = decoded(&[1, 0xFC, 2, 0xFD, 3], DIALOGUE, 1);
    assert_eq!(d["text"], "A\nB\nC");
    assert_eq!(
        d["segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.get("name").unwrap_or(&s["text"]).clone())
            .collect::<Vec<_>>(),
        json!(["A", "newline", "B", "wait_for_input", "C"])
            .as_array()
            .unwrap()
            .to_vec()
    );
    assert_eq!(
        decoded(&[0xFA, 0xDA, 3, 1], DIALOGUE, 1)["segments"][0],
        json!({"ctrl":"0xFA", "name":"event_flag_check", "operands":[218,3], "length":3})
    );
    let d = decoded(&[0xFB, 1, 2, 3, 1], DIALOGUE, 1);
    assert_eq!(d["segments"][0]["operands"], json!([1, 2, 3]));
    assert_eq!(d["text"], "A");
    assert_eq!(decoded(&[0xF4, 3, 2, 1], DIALOGUE, 1)["text"], "BA");
    let d = decoded(&[0xF4, 3, 2, 1], DIALOGUE, 2);
    assert_eq!(d["text"], "A");
    assert_eq!(d["segments"][0]["portrait_id"], 3);
    assert_eq!(d["segments"][0]["portrait_location"], 2);
    for (bytes, action, operands) in [
        (&[0xF2, 0, 0x12, 0x34, 1][..], "load_panel", json!([18, 52])),
        (&[0xF2, 8, 1][..], "pause_music", json!([])),
    ] {
        let d = decoded(bytes, DIALOGUE, 1);
        assert_eq!(d["segments"][0]["action"], action);
        assert_eq!(d["segments"][0]["operands"], operands);
        assert_eq!(d["text"], "A");
    }
    // Negative controls reject a shifted font, fork-only action, and short operand.
    assert!(decode_string(&[0x4E], DIALOGUE, 1).is_err());
    assert!(decode_name(&[0x25], WINDOW).is_err());
    assert!(decode_string(&[0xF2, 0x0D, 0, 0], DIALOGUE, 1).is_err());
    assert!(decode_string(&[0xFA, 1], DIALOGUE, 1).is_err());
    assert_ne!(
        decoded(&[1, 27, 64], WINDOW, 1)["text"],
        decoded(&[1, 27, 64], DIALOGUE, 1)["text"]
    );
}

#[test]
fn retail_names_keep_counts_ids_provenance_and_parallel_fonts() {
    let Some(rom) = retail_rom("text names") else {
        return;
    };
    let data = rom.bytes();
    let n = Value::from(&extract_names(data).unwrap());
    let counts = [11, 8, 153, 112, 14, 160, 40, 54, 54, 160];
    for (spec, count) in NAME_TABLES.iter().zip(counts) {
        let entries = n[spec.name].as_array().unwrap();
        assert_eq!(entries.len(), count);
        assert_eq!(n["tables"][spec.name]["entries"], n[spec.name]);
        assert_eq!(n["tables"][spec.name]["count"], count);
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry["id"], spec.id_base + i);
            let at = usize::from_str_radix(
                entry["rom_offset"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let raw = unhex_value(&entry["raw_hex"]);
            assert_eq!(&data[at..at + raw.len()], raw);
            assert_eq!(raw.last(), Some(&spec.terminator));
            assert_eq!(
                raw.len(),
                entry["name"].as_str().unwrap().chars().count() + 1
            );
        }
    }
    assert!(
        n["validation"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["ok"] == true)
    );
    assert_eq!(
        n["tables"]["enemy_names"]["rom_end_exclusive"],
        n["tables"]["enemy_skill_names"]["rom_offset"]
    );
    assert_eq!(
        n["tables"]["enemy_skill_names"]["rom_end_exclusive"],
        "0x2816BC"
    );
    let names = |table: &str| {
        n[table]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names("item_names"), names("item_names_dialogue"));
    assert_ne!(
        n["item_names"][0]["raw_hex"],
        n["item_names_dialogue"][0]["raw_hex"]
    );
    for (i, vehicle) in n["vehicle_names"].as_array().unwrap().iter().enumerate() {
        assert_eq!(vehicle["id"], i + 1);
        assert_eq!(vehicle["item_id"], 0x96 + i);
        assert_eq!(vehicle["name"], n["item_names"][0x95 + i]["name"]);
    }
    // Check every signature, rather than just the enemy table's first byte.
    for spec in NAME_TABLES {
        let mut bad = data.to_vec();
        bad[spec.start] ^= 1;
        assert!(extract_names(&bad).is_err());
    }
    source::names_against_reference(&n);
}

#[test]
fn retail_dialogue_regions_bytes_controls_actions_and_provenance() {
    let Some(rom) = retail_rom("text dialogue") else {
        return;
    };
    let data = rom.bytes();
    let decodes = crate::decode::Decodes::new(data);
    let specs = dialogue_tree_specs(data, &decodes).unwrap();
    assert_eq!(specs.len(), 43);
    let emitted = crate::dialogue::retirement::emission()
        .unwrap()
        .document("trees");
    assert_eq!(emitted["region"]["start"], "0x1DF600");
    assert_eq!(emitted["region"]["end_exclusive"], "0x1FE655");
    assert_eq!(emitted["region"]["tree_count"], 43);
    let mut at = DIALOGUE_REGION_START;
    let mut total = 0;
    let mut controls = BTreeSet::new();
    let mut actions = BTreeSet::new();
    for spec in &specs {
        assert_eq!(spec.start, at);
        assert_eq!(spec.padding, spec.end.next_multiple_of(16) - spec.end);
        assert!(
            data[spec.end..spec.end + spec.padding]
                .iter()
                .all(|&b| b == 0)
        );
        let stream = decodes
            .get(crate::decode::Call::Kosinski { offset: spec.start })
            .unwrap();
        let raw = stream.bytes();
        let (block, pad) = trim_tree_alignment(&raw, "retail").unwrap();
        let entries = split_terminated(block, 0xFF).unwrap();
        if spec.tree == 1 {
            assert_eq!((raw.len(), pad), (5854, 1));
            assert_eq!(entries[0].0, 0);
            let first = decoded(entries[0].1, DIALOGUE, 1);
            assert_eq!(
                first["segments"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|s| s["ctrl"].as_str())
                    .collect::<Vec<_>>(),
                ["0xFA", "0xFA", "0xFA", "0xFD", "0xFC"]
            );
        }
        if spec.tree == 31 {
            assert_eq!(entries.len(), 26);
        }
        let tree = &emitted["trees"][spec.tree - 1];
        assert_eq!(tree["label"], format!("DialogueTree{}", spec.tree));
        assert_eq!(tree["rom_offset"], format!("0x{:06X}", spec.start));
        assert_eq!(tree["rom_end_exclusive"], format!("0x{:06X}", spec.end));
        assert_eq!(tree["compressed_size"], spec.end - spec.start);
        assert_eq!(tree["decompressed_size"], raw.len());
        assert_eq!(tree["compression"], "kosinski");
        use sha2::Digest;
        assert_eq!(
            tree["compressed_sha256"],
            crate::png::hex(&sha2::Sha256::digest(&data[spec.start..spec.end]))
        );
        let talk = [31, 32].contains(&spec.tree);
        for (_, bytes) in &entries {
            let d = decoded(bytes, DIALOGUE, if talk { 2 } else { 1 });
            let mut length = 0;
            for s in d["segments"].as_array().unwrap() {
                length += if let Some(t) = s["text"].as_str() {
                    t.chars().count()
                } else {
                    s["length"].as_u64().unwrap() as usize
                };
                if let Some(c) = s["ctrl"].as_str() {
                    controls.insert(c.to_owned());
                }
                if s["ctrl"] == "0xF2" {
                    actions.insert(s["action_id"].as_u64().unwrap());
                }
                if talk && s["ctrl"] == "0xF4" {
                    assert!(s["portrait_location"].as_u64().unwrap() <= 2);
                }
            }
            assert_eq!(length, bytes.len());
        }
        total += entries.len();
        at = spec.end + spec.padding;
    }
    assert_eq!(specs.last().unwrap().end, 0x1FE655);
    assert_eq!(at, 0x1FE660);
    assert_eq!(total, 2736);
    assert_eq!(emitted["entry_count"], total);
    assert!(
        !decoded(&data[at + 2..at + 0x22], DIALOGUE, 1)["text"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        controls,
        [
            "0xF2", "0xF3", "0xF4", "0xF5", "0xF6", "0xF7", "0xF9", "0xFA", "0xFC", "0xFD"
        ]
        .map(str::to_owned)
        .into()
    );
    assert_eq!(actions, [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12].into());
    source::dialogue_against_reference(data, &specs, &decodes);
}
