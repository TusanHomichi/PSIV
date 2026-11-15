use super::*;
use serde_json::json;

#[test]
fn attack_frames_retain_mapping_duration_art_and_disjoint_priority_surfaces() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let animations = &r.file("enemy_animations")["animations"];
    let attacks = &e.indexes["enemy_attacks"];
    let receipts: Value = serde_json::from_str(include_str!(
        "../../../../../../oracle/fixtures/battle_animation_remainder.json"
    ))
    .unwrap();
    let receipt_ids: BTreeSet<_> = array(&receipts["receipts"])
        .iter()
        .flat_map(|r| array(&r["enemy_ids"]).iter().map(number))
        .collect();
    assert_eq!(receipt_ids.len(), 32);
    let entries = array(&attacks["enemies"]);
    assert_eq!(entries.len(), 153);
    let census = &attacks["census"];
    for (key, n) in [
        ("exact", 121),
        ("deferred", 32),
        ("partial", 0),
        ("frames", 960),
        ("priority_pngs", 1920),
    ] {
        assert_eq!(census[key], n, "{key}");
    }
    let mut frame_total = 0;
    for (id, entry) in entries.iter().enumerate() {
        assert_eq!(entry["id"], id);
        assert_eq!(entry["source"]["grand_cross"], 0);
        let frames = array(&entry["frames"]);
        if receipt_ids.contains(&id) {
            assert_eq!(entry["status"], "deferred");
            assert!(frames.is_empty());
            assert!(entry.get("art").is_none());
            assert!(entry.get("origin_pixels").is_none());
            continue;
        }
        assert_eq!(entry["status"], "exact");
        let sequence = &animations[id]["frame_sequence"];
        assert_eq!(frames.len(), number(&sequence["frame_count"]));
        assert!(!frames.is_empty());
        assert_eq!(entry["total_frames"], sequence["total_frames"]);
        let runtime = &animations[id]["movement"]["runtime"];
        for key in ["kind", "initial_offset_pixels", "step_pixels"] {
            assert_eq!(
                entry["movement"]["runtime"][key], runtime[key],
                "enemy {id}: {key}"
            );
        }
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame["index"], index);
            assert_eq!(frame["duration"], sequence["frame_durations"][index]);
            assert_eq!(
                frame["mapping_rom_offset"],
                sequence["mapping_records"][index]["rom_offset"]
            );
            assert!(
                !frame["mapping_rom_offset"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("oracle:")
            );
            let images: Vec<_> = ["png", "low_priority_png", "high_priority_png"]
                .iter()
                .map(|key| {
                    let path = frame[key].as_str().unwrap();
                    assert!(path.starts_with("battle/art/enemy_attacks/"));
                    let bytes = &e.blobs[path];
                    assert_eq!(
                        frame[format!("{key}_sha256")],
                        png::image_sha256(bytes).unwrap()
                    );
                    let p = png::decode(bytes).unwrap();
                    assert_eq!(
                        (p.width, p.height),
                        (
                            number(&entry["width_pixels"]),
                            number(&entry["height_pixels"])
                        )
                    );
                    p
                })
                .collect();
            for p in &images[1..] {
                assert_eq!(p.plte, images[0].plte);
                assert_eq!(p.trns, images[0].trns);
            }
            for ((&combined, &low), &high) in images[0]
                .pixels
                .iter()
                .zip(&images[1].pixels)
                .zip(&images[2].pixels)
            {
                assert!(low == 0 || high == 0, "priority surfaces overlap");
                assert_eq!(combined, low | high);
            }
        }
        frame_total += frames.len();
    }
    assert_eq!(frame_total, 960);
    assert!(
        array(&entries[24]["art"])
            .iter()
            .any(|a| a["field"] == "attack_plc")
    );
    assert_eq!(entries[10]["palette_line"], 2);
    let expected: BTreeMap<_, _> = [(10, 0x10D72), (11, 0x10D78), (12, 0x10D7E)]
        .map(|(index, at)| {
            (
                index.to_string(),
                crate::battle::image_tests::word(&r.rom, at),
            )
        })
        .into();
    assert_eq!(entries[10]["palette_overrides"], json!(expected));
    // Attack bank begins at Art #1; body staging uses Art #3, #1, #2.
    let records = super::super::enemies::records(&r.rom).unwrap();
    let bounds = super::super::enemies::bounds(&records, r.rom.len());
    let (body, _) = super::super::enemies::bank(&r.rom, &records[10], &bounds).unwrap();
    let (attack, _) = super::super::enemies::attack_bank(&r.rom, &records[10], &bounds).unwrap();
    assert_eq!(attack.len(), 23);
    assert!(attack == body[4..27]);
    assert_ne!(attack[0], body[0]);
    assert_eq!(entries[10]["art"][0]["first_pattern"], 0);
    assert_eq!(entries[10]["art"][0]["field"], "+0x02");
}

/// Evidence-only check: never an input to the emitter. Missing local PNGs
/// skip explicitly; a present incomplete or unreadable corpus fails.
#[test]
fn local_receipt_frames_are_shared_between_routine_bodies() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/fixtures/battle_animation_art");
    if !root.try_exists().unwrap() {
        eprintln!("local receipt PNGs absent: shared-evidence check skipped");
        return;
    }
    let receipts: Value = serde_json::from_str(include_str!(
        "../../../../../../oracle/fixtures/battle_animation_remainder.json"
    ))
    .unwrap();
    let sequence = |routine: &str| {
        let receipt = array(&receipts["receipts"])
            .iter()
            .find(|r| r["routine_offset"] == routine)
            .unwrap();
        (0..array(&receipt["durations"]).len())
            .map(|i| {
                let bytes = std::fs::read(root.join(format!(
                    "{}_frame{i:02}.png",
                    routine.trim_start_matches("0x")
                )))
                .unwrap();
                let p = png::decode(&bytes).unwrap();
                (p.width, p.height, p.pixels)
            })
            .collect::<Vec<_>>()
    };
    let first = sequence("0x00D526");
    for routine in [
        "0x00D998", "0x00DCD2", "0x00EFB2", "0x00F088", "0x00FCCA", "0x010C7C", "0x010E7A",
    ] {
        assert!(
            sequence(routine) == first,
            "shared receipt sequence differs"
        );
    }
    let second = sequence("0x00E394");
    for routine in ["0x00F53C", "0x010AB4"] {
        assert!(sequence(routine) == second);
    }
    assert!(
        first != second,
        "different receipt groups are the negative control"
    );
    assert!(sequence("0x00E2BE") == [first[..6].to_vec(), first[5..8].to_vec()].concat());
}
