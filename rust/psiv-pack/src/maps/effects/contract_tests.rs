//! Extraction contracts from test_map_effects.py (#555). No retail fixtures.
use super::*;
use crate::{decode::Decodes, maps::extract_maps};
use serde_json::{Value, json};

fn array(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn paths(e: &Value) -> &[Value] {
    e.get("paths").map_or(&[], array)
}
fn writes(e: &Value) -> Vec<&Value> {
    paths(e).iter().flat_map(|p| array(&p["writes"])).collect()
}
fn resolve(plane: Plane, x: u16, y: u16, disp: i16, widths: [i64; 2]) -> Value {
    Value::from(&resolve_layout_write(
        0x1234,
        (
            LayoutTarget {
                plane,
                chunk_x: x,
                chunk_y: y,
            },
            disp,
            Some(0x59),
        ),
        &MapGeometry {
            widths,
            heights: [128, 128],
        },
    ))
}

#[test]
fn layout_addresses_wrap_use_the_target_plane_and_refuse_outside_both_buffers() {
    let bg = |x, y, d, widths| resolve(Plane::Bg, x, y, d, widths);
    let xy = |v: Value| (v["chunk_x"].clone(), v["chunk_y"].clone());
    assert_eq!(xy(bg(31, 3, 1, [32, 32])), (json!(0), json!(4)));
    assert_eq!(xy(bg(4, 3, 32, [32, 32])), (json!(4), json!(4)));
    assert_eq!(xy(bg(4, 3, 1, [32, 32])), (json!(5), json!(3)));
    assert_eq!(xy(bg(31, 3, 1, [48, 48])), (json!(32), json!(3)));
    assert_eq!(resolve(Plane::Fg, 31, 3, 1, [32, 48])["chunk_y"], 4);
    assert_eq!(bg(31, 3, 1, [32, 48])["chunk_y"], 3);
    let cells = bg(5, 6, 0, [32, 32]);
    assert_eq!(
        (&cells["cell_x"], &cells["cell_y"]),
        (&json!(10), &json!(12))
    );
    let crossed = bg(5, 6, -0x1000, [48, 32]);
    assert_eq!(crossed["plane"], "fg");
    assert_eq!(crossed["crosses_plane"], true);
    assert_eq!(xy(crossed), (json!(5), json!(4)));
    let outside = resolve(Plane::Fg, 0, 0, -0x4000, [32, 32]);
    assert_eq!(outside["out_of_bounds"], true);
    assert!(outside.get("cell_x").is_none() && outside.get("cell_y").is_none());
    assert_eq!(outside["displacement"], -0x4000);
}

#[test]
fn retail_dispatcher_banks_and_layout_buffers_are_the_decoders_anchors() {
    let Some(rom) = crate::test_support::retail_rom("effect anchors") else {
        return;
    };
    let data = rom.bytes();
    let a = Anchors::read(data).unwrap();
    assert_eq!(a.table, 0x51b64);
    for index in [0, 1, MAP_DATA_MANAGER_ROUTINES - 1] {
        assert_eq!(
            &data[a.table + index * 4..a.table + index * 4 + 2],
            &0x6000u16.to_be_bytes()
        );
    }
    assert_eq!(a.tests.len(), 4);
    assert_eq!(a.tests[&0x57624], "event_flags");
    assert_eq!(
        a.tests.values().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "event_flags",
            "extended_event_flags",
            "chest_flags",
            "town_flags"
        ])
    );
    // No LEA of the fork's alleged temporary bank occurs in the US image.
    assert!(!data.windows(4).any(|w| w == [0x41, 0xf8, 0xf1, 0x56]));
    assert_eq!(a.clears.len(), 3);
    assert_eq!(a.clears[&0x576a8], "event_flags");
    assert!(!a.clears.values().any(|&v| v == "town_flags"));
    let at = anchors::GET_MAP_LAYOUT_OFFSET as usize;
    // tst.w d3 / bne / move.b Map_Row_Size_FG,d3 / lea Map_Layout_FG,a1
    assert_eq!(crate::m68k::w(data, at).unwrap(), 0x4a43);
    assert_eq!(crate::m68k::w(data, at + 2).unwrap(), 0x660a);
    assert_eq!(crate::m68k::w(data, at + 4).unwrap(), 0x1638);
    assert_eq!(crate::m68k::w(data, at + 6).unwrap(), 0xec64);
    assert_eq!(crate::m68k::w(data, at + 8).unwrap(), 0x43f8);
    assert_eq!(crate::m68k::w(data, at + 10).unwrap() as i64, MAP_LAYOUT_FG);
    check_map_layout_bases(data).unwrap();
    assert_eq!(MAP_LAYOUT_BG - MAP_LAYOUT_FG, MAP_LAYOUT_BYTES);
    assert!(routine_address(data, a.table, MAP_DATA_MANAGER_ROUTINES).is_err());
}

#[test]
fn retail_paths_keep_gate_polarity_loops_absolute_writes_and_spaceport_choices() {
    let Some(rom) = crate::test_support::retail_rom("effect paths") else {
        return;
    };
    let data = rom.bytes();
    let a = Anchors::read(data).unwrap();
    let entry = |index| {
        let e = decode_entry(data, index, &a).unwrap();
        assert!(e.undecoded.is_none(), "entry {index}: {:?}", e.undecoded);
        e
    };
    let zema = entry(5);
    let live: Vec<_> = zema.paths.iter().filter(|p| !p.writes.is_empty()).collect();
    assert_eq!(live.len(), 1);
    let p = live[0];
    assert_eq!(
        p.gates
            .iter()
            .map(|g| (g.bank, g.flag, g.required))
            .collect::<Vec<_>>(),
        [("event_flags", 0x33, true)]
    );
    assert_eq!(
        p.writes
            .iter()
            .map(|w| {
                assert_eq!(w.kind, "layout_write");
                let (t, d, c) = w.layout.unwrap();
                (t.chunk_x, t.chunk_y, d, c.unwrap())
            })
            .collect::<Vec<_>>(),
        [
            (21, 11, 0, 0x59),
            (21, 11, 1, 0x5a),
            (20, 21, 0, 0x59),
            (20, 21, 1, 0x5a),
            (12, 15, 0, 0x59),
            (12, 15, 1, 0x5a),
            (17, 15, 0, 0x59),
            (17, 15, 1, 0x5a),
        ]
    );
    let alys = entry(1);
    let live: Vec<_> = alys.paths.iter().filter(|p| !p.writes.is_empty()).collect();
    assert_eq!(live.len(), 1);
    assert_eq!(
        live[0]
            .gates
            .iter()
            .map(|g| (g.bank, g.flag, g.required))
            .collect::<Vec<_>>(),
        [("event_flags", 8, true)]
    );
    assert_eq!(live[0].writes.len(), 1);
    assert_eq!(live[0].writes[0].kind, "object_despawn");
    assert_eq!(Value::from(&live[0].writes[0].detail)["object_index"], 7);
    let maps = extract_maps(data, &Decodes::new(data)).unwrap();
    let academy = maps.real().find(|r| r.id == 0x13).unwrap();
    assert_eq!(
        academy.objects.entries[7].record.symbol(),
        Some("NPCAlysPiata")
    );
    let aiedo = entry(0x83);
    assert_eq!(aiedo.paths.len(), 1);
    assert!(aiedo.paths[0].gates.is_empty() && aiedo.paths[0].deferred.is_empty());
    assert_eq!(
        aiedo.paths[0]
            .writes
            .iter()
            .map(|w| {
                let (t, d, c) = w.layout.unwrap();
                assert_eq!(w.kind, "layout_write");
                (t.plane, d, c)
            })
            .collect::<Vec<_>>(),
        [(Plane::Bg, 0x706, Some(6)), (Plane::Bg, 0x709, Some(6))]
    );
    let port = entry(0x67);
    let despawned = |flags: &[u16]| {
        let matches: Vec<_> = port
            .paths
            .iter()
            .filter(|p| {
                p.gates
                    .iter()
                    .all(|g| flags.contains(&g.flag) == g.required)
            })
            .collect();
        assert_eq!(matches.len(), 1, "{flags:?}");
        let mut ids: Vec<_> = matches[0]
            .writes
            .iter()
            .map(|w| {
                assert_eq!(w.kind, "object_despawn");
                Value::from(&w.detail)["object_index"].as_u64().unwrap()
            })
            .collect();
        ids.sort_unstable();
        ids
    };
    assert_eq!(despawned(&[]), (0..10).collect::<Vec<_>>());
    assert_eq!(despawned(&[0xd9]), (5..10).collect::<Vec<_>>());
    assert_eq!(despawned(&[0xd9, 0xda]), (0..5).collect::<Vec<_>>());
    for (flag, picked) in [(0xdc, 5), (0xdb, 6), (0xdd, 7), (0xde, 8), (0xdf, 9)] {
        assert_eq!(despawned(&[0xd9, 0xda, flag]), vec![0, 1, 2, 3, 4, picked]);
    }
}

#[test]
fn retail_census_accounts_for_every_path_object_slot_refusal_and_flag_clear() {
    let Some(rom) = crate::test_support::retail_rom("effect census") else {
        return;
    };
    let data = rom.bytes();
    let maps = extract_maps(data, &Decodes::new(data)).unwrap();
    let effects = extract_map_effects(data, maps.real()).unwrap();
    let c = Value::from(&effects.census);
    for (key, n) in [
        ("table_entries", 160),
        ("referenced_entries", 127),
        ("unreferenced_entries", 33),
        ("maps_with_entries", 139),
        ("map_entry_pairs", 188),
        ("decoded_entries", 117),
    ] {
        assert_eq!(c[key], n, "{key}");
    }
    assert_eq!(c["evaluated"], "map load only");
    assert_eq!(
        c["kinds"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "flag_clear",
            "layout_replace",
            "layout_write",
            "object_despawn",
            "object_dialogue",
            "object_rewrite",
            "object_x",
            "object_y"
        ]
    );
    assert_eq!(c["kinds"]["object_despawn"], 1455);
    assert_eq!(c["kinds"]["layout_write"], 193);
    assert_eq!(c["kinds"]["layout_replace"], 4);
    assert_eq!(c["maps_per_kind"]["layout_replace"], 2);
    for key in ["gate_banks", "flags_per_bank"] {
        assert_eq!(
            c[key]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["chest_flags", "event_flags"]
        );
    }
    let past = array(&c["object_writes_past_map_object_count"]);
    assert_eq!(
        past.iter()
            .map(|e| e["map"].as_u64().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0xf6, 0xfb])
    );
    assert_eq!(
        past.iter()
            .map(|e| e["entry"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["0x64", "0x65"])
    );
    assert!(
        past.iter()
            .all(|e| e["object_index"].as_u64() >= e["map_objects"].as_u64())
    );
    let refused = array(&c["undecoded_entries"]);
    assert_eq!(refused.len(), 10);
    assert_eq!(
        refused
            .iter()
            .map(|e| e["entry"].as_u64().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        10
    );
    for e in refused {
        assert!(!e["reason"].as_str().unwrap().is_empty());
        assert!(!array(&e["maps"]).is_empty());
    }
    let a = Anchors::read(data).unwrap();
    let mut checked = 0;
    let mut polarities = BTreeSet::new();
    let mut notes = BTreeSet::new();
    for record in maps.real() {
        let Some(entries) = effects.per_map.get(&record.id) else {
            assert!(record.map_data_manager.ids.is_empty());
            continue;
        };
        assert_eq!(entries.len(), record.map_data_manager.ids.len());
        for (e, &id) in entries.iter().zip(&record.map_data_manager.ids) {
            let e = Value::from(e);
            assert_eq!(e["entry"], id);
            if e["decoded"] == false {
                assert!(
                    refused.iter().any(|r| r["entry"] == e["entry"]
                        && array(&r["maps"]).contains(&json!(record.id)))
                );
                continue;
            }
            let raw = decode_entry(data, id as usize, &a).unwrap();
            assert_eq!(
                paths(&e).len(),
                raw.paths
                    .iter()
                    .filter(|p| !p.writes.is_empty() || p.aborts || !p.deferred.is_empty())
                    .count()
            );
            for p in paths(&e) {
                assert_eq!(p["aborts_remaining_entries"], false);
                let mut seen = BTreeMap::new();
                for g in array(&p["gates"]) {
                    let key = (g["bank"].as_str().unwrap(), g["flag"].as_u64().unwrap());
                    let required = g["required"].as_str().unwrap();
                    if let Some(before) = seen.insert(key, required) {
                        assert_eq!(before, required);
                    }
                    polarities.insert(required.to_owned());
                }
                notes.extend(
                    array(&p["deferred"])
                        .iter()
                        .map(|n| n.as_str().unwrap().to_owned()),
                );
                if !array(&p["unmodelled_conditions"]).is_empty() {
                    assert_eq!(p["unconditional"], false);
                }
                for w in array(&p["writes"]) {
                    if let Some(slot) = w["object_index"].as_u64() {
                        checked += 1;
                        if slot as usize >= record.objects.entries.len() {
                            assert!(past.iter().any(|r| r["map"] == record.id
                                && r["at"] == w["at"]
                                && r["object_index"] == slot));
                        }
                    }
                    if w["kind"] == "layout_write" && w["out_of_bounds"] == false {
                        for key in ["cell_x", "cell_y", "chunk_x", "chunk_y"] {
                            assert!(w[key].as_i64().unwrap() >= 0);
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 300);
    assert_eq!(polarities, BTreeSet::from(["set".into(), "clear".into()]));
    assert!(notes.contains("write to 0xFFFFECF4"));
    assert!(!notes.contains("write to 0xFFFFB706"));
    assert!(c["paths_position_gated"].as_u64().unwrap() > 0);
    let chest = &c["flag_clears"]["chest_flags"];
    assert_eq!(
        chest["by_entry"],
        json!({"0x14":[9,10,13,14,15,16],"0x17":[11,12,17,18],"0x18":[19],"0x3D":[21],"0x3E":[23],"0x47":[25],"0x84":[8]})
    );
    assert_eq!(chest["entries"], 7);
    assert_eq!(array(&chest["flag_ids"]).len(), 15);
    assert_eq!(chest["bank_address"], "0xFFFFF140");
    assert_eq!(
        c["flag_clears"]["event_flags"]["by_entry"],
        json!({"0x1A":[20]})
    );
    assert_eq!(
        c["flag_clears"]["event_flags"]["bank_address"],
        "0xFFFFF100"
    );

    let emitted = |map, index| {
        Value::from(
            effects.per_map[&map]
                .iter()
                .find(|e| int(e, "entry") == Some(index))
                .unwrap(),
        )
    };
    let aiedo = emitted(0x54, 0x83);
    assert_eq!(
        writes(&aiedo)
            .iter()
            .map(|w| (&w["plane"], &w["chunk_x"], &w["chunk_y"], &w["chunk_id"]))
            .collect::<Vec<_>>(),
        vec![
            (&json!("bg"), &json!(22), &json!(37), &json!(6)),
            (&json!("bg"), &json!(25), &json!(37), &json!(6))
        ]
    );
    let krup = emitted(0x3f, 32);
    assert_eq!(
        paths(&krup)
            .iter()
            .map(|p| array(&p["gates"])
                .iter()
                .map(|g| (g["flag"].as_u64().unwrap(), g["required"].as_str().unwrap()))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [
            vec![(0x67, "clear"), (0x42, "clear")],
            vec![(0x67, "set"), (0x42, "clear")],
            vec![(0x67, "set"), (0x42, "set")]
        ]
    );
    assert_eq!(
        paths(&krup)
            .iter()
            .map(|p| array(&p["writes"]).len())
            .collect::<Vec<_>>(),
        [4, 4, 7]
    );
    let layout: Vec<_> = writes(&krup)
        .into_iter()
        .filter(|w| w["kind"] == "layout_write")
        .collect();
    assert_eq!(layout.len(), 3);
    for (w, (plane, x, y, cross, d)) in layout.iter().zip([
        ("bg", 34, 32, false, 0),
        ("fg", 34, 32, true, -4096),
        ("fg", 34, 30, true, -4128),
    ]) {
        assert_eq!(
            (&w["plane"], &w["cell_x"], &w["cell_y"], &w["crosses_plane"]),
            (&json!(plane), &json!(x), &json!(y), &json!(cross))
        );
        if cross {
            assert_eq!(w["requested_plane"], "bg");
        }
        assert_eq!(w["displacement"], d);
    }
    let sanctuary = emitted(0x16f, 85);
    let crossed: Vec<_> = writes(&sanctuary)
        .into_iter()
        .filter(|w| w["crosses_plane"] == true)
        .collect();
    assert_eq!(crossed.len(), 1);
    assert_eq!(
        (
            &crossed[0]["plane"],
            &crossed[0]["cell_x"],
            &crossed[0]["cell_y"]
        ),
        (&json!("fg"), &json!(30), &json!(22))
    );
}
