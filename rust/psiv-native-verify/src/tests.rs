use super::*;

fn fixture() -> (Value, Value) {
    let mut rows: Vec<_> = (1..=50).map(|n| json!({"label":format!("zema-page-{n:02}"),"frame":n,
        "capture":"page.png","state":{"map":36.0,"dialogue_page":{"ready":true,"tree":105,"lines":[n.to_string()]}}})).collect();
    for (label, y) in [("igglanova-halfway", 176), ("igglanova-arrived", 192)] {
        rows.push(
            json!({"label":label,"capture":"page.png","state":{"map":36.0,
            "temporary_objects":[{"id":392.0,"position":[480.0,y]}]}}),
        );
    }
    let mut words = vec![json!(0); 48];
    words.extend((0..16).map(|n| json!(n)));
    rows.push(json!({"label":"zema-restored","capture":"page.png","state":{"map":36.0,"flags":[55.0],
        "active_npcs":(0..7).map(|n|json!(f64::from(n))).collect::<Vec<_>>(),
        "npc_sheets":(0..7).map(|n|json!({"index":f64::from(n),"sheet":format!("normal-{n}")})).collect::<Vec<_>>(),
        "cram":{"live":true,"words":words}}}));
    let map = json!({"map_effects":[{"paths":[{"deferred_effects":[{"at":"0x051E56",
        "words_hex":(0..16).map(|n|format!("0x{n:x}")).collect::<Vec<_>>(),
        "affects":{"npc_sheets":(0..7).map(|n|json!({"npc_index":n,"to":format!("normal-{n}")})).collect::<Vec<_>>()}}]}]}]});
    (json!({"observations":rows}), map)
}

#[test]
fn schema_and_pinned_identity_reject_ambiguous_addresses() {
    let good = json!([{"frame":1,"label":"state"},{"frame":1,"label":"shot","capture":true}]);
    assert_eq!(requests(&good, None).unwrap(), good);
    for bad in [
        json!([]),
        json!([{"frame":true,"label":"a"}]),
        json!([{"frame":1.5,"label":"a"}]),
        json!([{"frame":0,"label":"a"}]),
        json!([{"frame":2,"label":"a"},{"frame":1,"label":"b"}]),
        json!([{"frame":1,"label":".."}]),
        json!([{"frame":1,"label":""}]),
        json!([{"frame":1,"label":"a","capture":"yes"}]),
        json!([{"frame":1,"label":"a","extra":true}]),
        json!([{"frame":1,"label":"a"},{"frame":1,"label":"a"}]),
    ] {
        assert!(requests(&bad, None).is_err(), "{bad}");
    }
    let manifest = json!({"tape_sha256":"a".repeat(64),"requests":good});
    assert!(requests(&manifest, Some(&"a".repeat(64))).is_ok());
    assert!(requests(&manifest, Some(&"b".repeat(64))).is_err());
    assert!(requests(&manifest, None).is_err());
    assert!(
        requests(
            &json!({"tape_sha256":"A".repeat(64),"requests":good}),
            Some(&"A".repeat(64))
        )
        .is_err()
    );
}

#[test]
fn receipts_require_exact_frames_complete_state_and_unchanged_capture_bytes() {
    let root = std::env::temp_dir().join(format!("psiv-native-verify-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("shot.png");
    std::fs::write(&path, b"png").unwrap();
    let wants = json!([{"label":"shot","frame":2,"capture":true}]);
    let row = json!({"label":"shot","frame":2,"state":{"map":0,"cell":[0,0]},"capture":path,"capture_file_sha256":sha256(b"png")});
    let report = json!({"observations":[row]});
    assert!(receipt(&wants, &report, &root).is_ok());
    for bad in [Value::Null, json!({}), json!({"observations":[]})] {
        assert!(receipt(&wants, &bad, &root).is_err());
    }
    for (key, value) in [
        ("frame", json!(3)),
        ("frame", json!(2.0)),
        ("frame", json!(true)),
        ("state", Value::Null),
        ("state", json!({})),
        ("label", json!("other")),
    ] {
        let mut bad = report.clone();
        bad["observations"][0][key] = value;
        assert!(receipt(&wants, &bad, &root).is_err());
    }
    std::fs::write(&path, b"changed").unwrap();
    assert!(receipt(&wants, &report, &root).is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(receipt(&wants, &report, &root).is_err());
    std::fs::remove_dir(&root).unwrap();
}

#[test]
fn zema_pages_entrance_live_cram_and_every_restored_sheet_are_required() {
    let (native, map) = fixture();
    assert_eq!(zema(&native, &map).unwrap()["resident_sheets"], 7);
    for (pointer, value) in [
        ("/observations/0/state/dialogue_page/ready", json!(false)),
        ("/observations/1/state/dialogue_page/lines", json!(["1"])),
        ("/observations/1/frame", json!(1)),
        (
            "/observations/50/state/temporary_objects/0/position",
            json!([480, 175]),
        ),
        ("/observations/52/state/cram/live", json!(false)),
        ("/observations/52/state/cram/words/49", json!(0)),
        (
            "/observations/52/state/npc_sheets/0/sheet",
            json!("petrified"),
        ),
        ("/observations/52/state/active_npcs/0", json!(1)),
        ("/observations/52/state/active_npcs/0", json!(0.5)),
        ("/observations/52/state/npc_sheets/0/index", json!(1)),
    ] {
        let mut bad = native.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(zema(&bad, &map).is_err(), "{pointer}");
    }
    let mut extra = native.clone();
    extra["observations"][52]["state"]["npc_sheets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"index":7,"sheet":"extra"}));
    assert!(zema(&extra, &map).is_err());
    let mut wrong = native.clone();
    wrong["observations"][52]["state"]["npc_sheets"][0]["index"] = json!(7);
    assert!(zema(&wrong, &map).is_err());
    let mut bad = native.clone();
    bad["observations"].as_array_mut().unwrap().remove(0);
    assert!(zema(&bad, &map).is_err());
    let mut bad = native.clone();
    bad["observations"]
        .as_array_mut()
        .unwrap()
        .push(native["observations"][0].clone());
    assert!(zema(&bad, &map).is_err());
}

#[test]
fn replacement_mapping_must_cover_seven_distinct_active_residents() {
    let (native, map) = fixture();
    let pointer = "/map_effects/0/paths/0/deferred_effects/0/affects/npc_sheets";
    // Reproduce the review bug: no/partial mappings formerly let all seven
    // captured residents stay petrified and still declared success.
    for count in [0, 1, 6] {
        let mut bad_map = map.clone();
        bad_map
            .pointer_mut(pointer)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .truncate(count);
        let mut petrified = native.clone();
        for sheet in petrified["observations"][52]["state"]["npc_sheets"]
            .as_array_mut()
            .unwrap()
        {
            sheet["sheet"] = json!("petrified");
        }
        assert!(zema(&petrified, &bad_map).is_err());
        assert!(zema(&native, &bad_map).is_err());
    }
    for replacement in [json!(1), json!(7)] {
        let mut bad = map.clone();
        bad.pointer_mut(pointer).unwrap()[0]["npc_index"] = replacement;
        assert!(zema(&native, &bad).is_err());
    }
    let mut bad = map.clone();
    bad.pointer_mut(pointer).unwrap()[0]["to"] = json!("wrong-target");
    assert!(zema(&native, &bad).is_err());
    let mut bad = map.clone();
    bad["map_effects"][0]["paths"][0]["deferred_effects"][0]["words_hex"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(zema(&native, &bad).is_err());
}
