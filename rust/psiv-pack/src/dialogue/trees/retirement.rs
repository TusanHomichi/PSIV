use super::*;
use crate::dialogue::retirement::emission;
use serde_json::{Value, json};

fn text(s: &str) -> Json {
    Json::object().with("text", s)
}
fn code(c: &str) -> Json {
    Json::object().with("ctrl", c)
}
fn expect(segments: Vec<Json>, lines: Value, endings: Value) {
    let pages = Value::from(&Json::from(paginate(&segments)));
    let pages = pages.as_array().unwrap();
    assert_eq!(
        pages.iter().map(|p| p["lines"].clone()).collect::<Vec<_>>(),
        lines.as_array().unwrap().to_vec()
    );
    assert_eq!(
        pages.iter().map(|p| p["end"].clone()).collect::<Vec<_>>(),
        endings.as_array().unwrap().to_vec()
    );
}

#[test]
fn pagination_preserves_wrap_wait_close_choice_null_and_preamble_rules() {
    expect(
        vec![text("hi"), code("0xFC"), text("there")],
        json!([["hi", "there"]]),
        json!(["end"]),
    );
    expect(
        vec![text(&"a".repeat(40))],
        json!([["a".repeat(32), "a".repeat(8)]]),
        json!(["end"]),
    );
    expect(
        vec![text(&"a".repeat(64)), code("0xFD"), text("b")],
        json!([["a".repeat(32), "a".repeat(32)], ["b"]]),
        json!(["full", "end"]),
    );
    expect(
        vec![text("a"), code("0xFD")],
        json!([["a"]]),
        json!(["end"]),
    );
    expect(
        vec![text("a"), code("0xFD"), code("0xFD"), text("b")],
        json!([["a"], ["b"]]),
        json!(["wait", "end"]),
    );
    for middle in [vec![code("0xF7")], vec![code("0xFD"), code("0xF7")]] {
        let mut segments = vec![text("a")];
        segments.extend(middle);
        segments.extend([text("b"), code("0xFD")]);
        expect(segments, json!([["a"], ["b"]]), json!(["close", "end"]));
    }
    for n in [1, 32] {
        expect(
            vec![text(&"a".repeat(n)), code("0xF5"), text("unreachable")],
            json!([["a".repeat(n)]]),
            json!(["choice"]),
        );
    }
    expect(
        vec![text("a"), code("0xFC"), text("b"), code("0xFC"), text("c")],
        json!([["a", "b", "c"]]),
        json!(["end"]),
    );
    expect(
        vec![code("0xFA"), code("0xF3"), text("a")],
        json!([["a"]]),
        json!(["end"]),
    );
    for c in ["0xF0", "0xF1", "0xF3", "0xF8", "0xFB"] {
        expect(
            vec![text("a"), code(c), text("b")],
            json!([["a"]]),
            json!(["end"]),
        );
    }
    expect(
        vec![text(&"a".repeat(31)), code("0xF4"), text("bc")],
        json!([["a".repeat(31) + "b", "c"]]),
        json!(["end"]),
    );
    assert_eq!(
        split_preamble(&[code("0xFA"), code("0xFA"), text("a")]),
        (2, false)
    );
    assert_eq!(split_preamble(&[code("0xFA"), code("0xF6")]), (2, true));
    assert_eq!(split_preamble(&[code("0xF3"), text("a")]), (1, false));
    assert_eq!(split_preamble(&[text("a")]), (0, false));
}

#[test]
fn decoded_controls_keep_typed_operands_and_talk_slots() {
    let cases: &[(Vec<u8>, u8, Value)] = &[
        (vec![0xF4, 6, 2], 2, json!({"id":6,"position":2})),
        (vec![0xF4, 6], 1, json!({"id":6,"position":null})),
        (vec![0xF5, 7, 8], 1, json!({"yes_entry":7,"no_entry":8})),
        (vec![0xF6, 1, 0x23], 1, json!({"id":0x123})),
        (
            vec![0xFA, 0xDA, 3],
            1,
            json!({"scope":"event_flag","flag":0xDA,"then_entry":3}),
        ),
        (vec![0xF9, 0], 1, json!({"frames":256})),
        (vec![0xF9, 59], 1, json!({"frames":59})),
        (
            vec![0xF2, 0, 1, 35],
            1,
            json!({"action_id":0,"action":"load_panel","panel":291}),
        ),
        (vec![0xF2, 3, 0xFE], 1, json!({"action_id":3,"sound":254})),
        (vec![0xF2, 4, 2], 1, json!({"action_id":4,"sound":2})),
        (vec![0xF2, 11, 0xDA], 1, json!({"flag":218})),
    ];
    for (bytes, slots, expected) in cases {
        let decoded = decode_string(bytes, DIALOGUE, *slots).unwrap();
        let typed = Value::from(&entry(&decoded).unwrap().0);
        let segment = &typed["segments"][0];
        assert_eq!(segment["code"], format!("0x{:02X}", bytes[0]));
        assert_eq!(
            segment["operands"],
            Value::from(&decoded)["segments"][0]["operands"]
        );
        let name = match bytes[0] {
            0xF2 => "action",
            0xF4 => "portrait",
            0xF5 => "yes_no",
            0xF6 => "event",
            0xF9 => "delay",
            0xFA => "flag_check",
            _ => unreachable!(),
        };
        assert_eq!(segment["ctrl"], name);
        for (key, value) in expected.as_object().unwrap() {
            assert_eq!(&segment[key], value, "{key}");
        }
    }
    assert_eq!(control_codes(), (0xF0..=0xFF).collect::<Vec<_>>());
    assert!(control_segment(&code("0xFB")).is_err());
    for bytes in [vec![0xF4], vec![0xF5, 1], vec![0xF6, 1], vec![0xF2, 0, 1]] {
        assert!(decode_string(&bytes, DIALOGUE, 1).is_err());
    }
}

#[test]
fn world_selection_reads_tree_identities_and_refuses_branch_pointer_and_length_corruption() {
    let Some(e) = emission() else { return };
    let rom = e.rom.bytes();
    let specs = dialogue_tree_specs(rom, &Decodes::new(rom)).unwrap();
    let s = Value::from(&selection(rom, &Decodes::new(rom)).unwrap());
    let table = usize::from_str_radix(
        s["table_offset"].as_str().unwrap().trim_start_matches("0x"),
        16,
    )
    .unwrap();
    for i in 0..7 {
        let pointer =
            u32::from_be_bytes(rom[table + i * 4..table + i * 4 + 4].try_into().unwrap()) as usize;
        let id = specs.iter().find(|p| p.start == pointer).unwrap().tree;
        assert_eq!(
            if i < 6 {
                &s["world_trees"][i]
            } else {
                &s["first_world_override"]["tree"]
            },
            &json!(id)
        );
    }
    assert_eq!(
        s["first_world_override"]["entry_from"],
        u16::from_be_bytes(rom[SELECTION + 18..SELECTION + 20].try_into().unwrap())
    );
    let mut corrupt = rom.to_vec();
    corrupt[SELECTION + 20] ^= 1;
    assert!(
        selection(&corrupt, &Decodes::new(&corrupt))
            .unwrap_err()
            .0
            .contains("opcode mismatch")
    );
    corrupt = rom.to_vec();
    corrupt[table + 4..table + 8].fill(0);
    assert!(
        selection(&corrupt, &Decodes::new(&corrupt))
            .unwrap_err()
            .0
            .contains("is not a tree")
    );
    let short = &rom[..table + 4];
    assert!(
        selection(short, &Decodes::new(short))
            .unwrap_err()
            .0
            .contains("past the end")
    );
}

#[test]
fn emitted_trees_dense_entries_raw_bytes_talk_slots_and_pinned_structure_match_source() {
    let Some(e) = emission() else { return };
    let d = e.document("trees");
    assert_eq!(d["tree_count"], 43);
    assert_eq!(d["entry_count"], 2736);
    let specs = dialogue_tree_specs(e.rom.bytes(), &Decodes::new(e.rom.bytes())).unwrap();
    let decodes = Decodes::new(e.rom.bytes());
    for (tree, spec) in d["trees"].as_array().unwrap().iter().zip(specs) {
        let data = decodes.get(Call::Kosinski { offset: spec.start }).unwrap();
        let bytes = data.bytes();
        let (block, _) = trim_tree_alignment(&bytes, "test").unwrap();
        let source = split_terminated(block, 0xFF).unwrap();
        let talk = [31, 32].contains(&spec.tree);
        assert_eq!(tree["tree"], spec.tree);
        assert_eq!(tree["is_talk_tree"], talk);
        assert_eq!(tree["portrait_operand_bytes"], if talk { 2 } else { 1 });
        assert_eq!(tree["entry_count"], source.len());
        assert_eq!(tree["entries"].as_array().unwrap().len(), source.len());
        for (id, (_, raw)) in source.iter().enumerate() {
            let entry = &tree["entries"][id];
            assert_eq!(entry["id"], id);
            assert_eq!(entry["raw_hex"], png::hex(raw));
            let decoded = decode_string(raw, DIALOGUE, if talk { 2 } else { 1 }).unwrap();
            assert_eq!(entry["text"], Value::from(&decoded)["text"]);
            for s in entry["segments"].as_array().unwrap() {
                if s["ctrl"] == "portrait" {
                    assert_eq!(!s["position"].is_null(), talk);
                }
            }
            for p in entry["pages"].as_array().unwrap() {
                assert!(p["lines"].as_array().unwrap().len() <= 2);
                for l in p["lines"].as_array().unwrap() {
                    assert!(l.as_str().unwrap().chars().count() <= 32);
                }
            }
        }
    }
    let tree = |id| {
        d["trees"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["tree"] == id)
            .unwrap()
    };
    let pinned = &tree(1)["entries"][4];
    assert_eq!(pinned["segments"][0]["flag"], 218);
    assert_eq!(pinned["segments"][0]["then_entry"], 3);
    assert_eq!(pinned["pages"][0]["end"], "full");
    assert_eq!(pinned["pages"][1]["end"], "end");
    let conversation = &tree(5)["entries"][14];
    let ids: Vec<_> = conversation["segments"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["ctrl"] == "portrait")
        .map(|s| s["id"].clone())
        .take(6)
        .collect();
    assert_eq!(
        ids,
        json!([3, 12, 3, 3, 2, 12]).as_array().unwrap().to_vec()
    );
    assert_eq!(conversation["pages"][0]["end"], "close");
    assert_eq!(conversation["pages"][1]["end"], "wait");
    assert_eq!(d["window"], e.fragment["window"]);
    for (key, value) in [
        ("chars_per_line", 32),
        ("lines_per_window", 2),
        ("glyph_width", 8),
        ("glyph_height", 16),
        ("cram_line", 2),
    ] {
        assert_eq!(d["window"][key], value);
    }
    assert_eq!(
        d["window"]["rect"],
        json!({"x":32,"y":168,"width":256,"height":32})
    );
    assert_eq!(d["window"]["vram_tile"], "0x580");
}

#[test]
fn system_messages_follow_rom_terminators_leader_portraits_and_vehicle_lea() {
    let Some(e) = emission() else { return };
    let rom = e.rom.bytes();
    let d = e.document("trees");
    let b = &d["system_messages"];
    assert_eq!(b["count"], 11);
    assert_eq!(b["messages"].as_array().unwrap().len(), 11);
    assert_eq!(b["label"], SYSTEM_MESSAGES_LABEL);
    assert_eq!(b["selector"]["ram"], "Current_Party_Slots ($FFFFF40A)");
    assert_eq!(e.fragment["trees"]["system_message_count"], 11);
    let mut at = SYSTEM_MESSAGES;
    for (i, m) in b["messages"].as_array().unwrap().iter().enumerate() {
        let end = terminator(rom, at).unwrap();
        assert_eq!(m["raw_hex"], png::hex(&rom[at..end]));
        assert_eq!(
            m["text"],
            Value::from(&decode_string(&rom[at..end], DIALOGUE, 1).unwrap())["text"]
        );
        assert_eq!(m["id"], i);
        assert_eq!(m["portrait"], i + 1);
        assert_eq!(m["portrait_symbol"], SPEAKERS[i]);
        assert_eq!(m["segments"][0]["id"], i + 1);
        assert_eq!(m["pages"].as_array().unwrap().len(), 1);
        let lines = m["pages"][0]["lines"].as_array().unwrap();
        assert!(lines.len() <= 2);
        assert!(
            lines
                .iter()
                .all(|l| l.as_str().unwrap().chars().count() <= 32)
        );
        assert_eq!(
            lines
                .iter()
                .map(|l| l.as_str().unwrap())
                .collect::<Vec<_>>()
                .join("\n"),
            m["text"].as_str().unwrap()
        );
        at = end + 1;
    }
    assert_eq!(b["rom_end_exclusive"], hex6(at));
    assert_eq!(b["alignment_pad_bytes"], 1);
    assert_eq!(rom[at - 1], 255);
    assert_eq!(rom[at], 0);
    assert_eq!(&rom[at + 1..at + 3], &[0xF4, 1]);
    let line = &b["cannot_get_off"];
    let lea = &rom[CANNOT_GET_OFF_LEA..CANNOT_GET_OFF_LEA + 6];
    assert_eq!(lea[..2], LEA_ABS_A0);
    let at = u32::from_be_bytes(lea[2..].try_into().unwrap()) as usize;
    assert_eq!(line["rom_offset"], hex6(at));
    assert_eq!(
        line["raw_hex"],
        png::hex(&rom[at..terminator(rom, at).unwrap()])
    );
    assert_eq!(line["charset"], "window");
    assert_eq!(line["pages"].as_array().unwrap().len(), 1);
    assert!(
        line["segments"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s.get("text").is_some())
    );
    let mut corrupt = rom.to_vec();
    corrupt[CANNOT_GET_OFF_LEA] ^= 1;
    assert!(cannot_get_off(&corrupt).is_err());
}

#[test]
fn action_payloads_and_entry_locations_survive_emission_and_census() {
    let decoded = decode_string(&[0xF2, 0, 0, 0x30, 0xF2, 3, 0xFE, 1], DIALOGUE, 1).unwrap();
    let mut record = entry(&decoded).unwrap().0;
    record.insert("id", 3);
    let typed = Value::from(&record);
    assert_eq!(typed["segments"][0]["panel"], 48);
    assert_eq!(typed["segments"][1]["sound"], 254);
    let mut census = crate::dialogue::census::Census::default();
    census.add_entry(7, &record).unwrap();
    let c = Value::from(&census.to_json(
        &std::collections::BTreeSet::new(),
        &std::collections::BTreeSet::new(),
        &control_codes(),
    ));
    assert_eq!(c["entries"], 1);
    assert_eq!(c["control_codes_by_tree"]["7"]["0xF2"], 2);
    assert_eq!(c["text_actions"]["load_panel"], 1);
    assert_eq!(c["text_actions"]["load_sound"], 1);
    assert_eq!(c["longest_line_at"], json!({"tree":7,"entry":3}));
}
