//! Record checks from tests/test_battle_animations.py (#551). The battle
//! fixture writes and reads the actual index; no cartridge tables or digests
//! are copied here. Movement opcode coverage stays in tests/movement.rs.

use serde_json::{Value, json};

use crate::battle::tests::retail;

fn array(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}

/// Shared with the corruption controls: relationships must hold even when
/// individual fields are otherwise well-formed JSON.
fn assert_sequence(animation: &Value) {
    let sequence = &animation["frame_sequence"];
    assert!(animation["flash_timing_proven"].as_bool().unwrap());
    assert!(matches!(
        sequence["frame_timer_helper"].as_str().unwrap(),
        "loc_256AE" | "loc_256F4" | "loc_25270" | "oracle:observed_sprite_table_clock"
    ));
    let durations = array(&sequence["frame_durations"]);
    assert_eq!(
        durations.len() as u64,
        sequence["frame_count"].as_u64().unwrap()
    );
    assert!(!durations.is_empty());
    assert!(durations.iter().all(|d| d.as_u64().unwrap() > 0));
    assert!(sequence["frame_duration"].as_u64().unwrap() > 0);
    assert_eq!(
        sequence["total_frames"].as_u64().unwrap(),
        durations.iter().map(|d| d.as_u64().unwrap()).sum::<u64>()
    );
    assert_eq!(array(&sequence["mapping_records"]).len(), durations.len());
    assert_eq!(array(&sequence["mapping_pointers"]).len(), durations.len());
    for (record, pointer) in array(&sequence["mapping_records"])
        .iter()
        .zip(array(&sequence["mapping_pointers"]))
    {
        assert_eq!(&record["rom_offset"], pointer);
        if sequence["timing_model"] == "observed" {
            assert_eq!(record["entry_bytes"], 8);
            assert_eq!(record["record_header_bytes"], 0);
            assert!(array(&record["entries"]).is_empty());
        } else {
            assert_eq!(record["entry_bytes"], 6);
            assert_eq!(record["record_header_bytes"], 1);
        }
        assert_eq!(record["all_entries_valid"], true);
        for entry in array(&record["entries"]) {
            for key in ["y", "size", "tile_word", "x", "x_mirror"] {
                assert!(!entry[key].is_null(), "missing {key}");
            }
            assert_eq!(entry["tile_bank_valid"], true);
            assert_eq!(entry["attributes_valid"], true);
        }
    }
}

#[test]
fn retail_provenance_sfx_and_complete_census() {
    let Some(r) = retail() else { return };
    let file = r.file("enemy_animations");
    let source = &file["source"];
    assert_eq!(source["rom_sha256"], crate::rom::EXPECTED_SHA256);
    assert_eq!(source["grand_cross"], 0);
    assert_eq!(source["reference_grand_cross"], 1);
    assert_eq!(source["record_count"], 153);
    assert!(!array(&source["tables"]).is_empty());
    assert!(
        array(&source["tables"])
            .iter()
            .all(|t| t["grand_cross"] == 0)
    );
    assert_eq!(file["count"], 153);
    let census = &file["census"];
    for key in [
        "exact_sfx",
        "frame_sequence_records",
        "timed_frame_sequences",
        "sprite_sheet_exact",
        "movement_exact",
    ] {
        assert_eq!(census[key], 153, "{key}");
    }
    for key in [
        "generic_sfx",
        "frame_sequence_deferred",
        "sprite_sheet_partial",
        "sprite_sheet_status_deferred",
        "movement_partial",
        "movement_status_deferred",
        "deferred_routine_bodies",
    ] {
        assert_eq!(census[key], 0, "{key}");
    }
    assert_eq!(census["attack_plc_loads"], 383);
    assert_eq!(census["attack_plc_distinct_records"], 122);
    assert_eq!(
        census["routine_classifications"],
        json!({
            "fixed_mapping": 62, "palette_or_plane_effect": 1,
            "projectile_effect_graph": 1, "selector_mapping": 10,
            "state_machine_without_sprite_mapping": 9, "tile_upload_only": 21,
            "variable_mapping": 49
        })
    );
    let records = array(&file["animations"]);
    assert_eq!(records.len(), 153);
    for (id, animation) in records.iter().enumerate() {
        assert_eq!(animation["enemy_id"], id);
        assert!(
            array(&animation["sfx_writes"])
                .iter()
                .any(|w| w["dispatch"] == "Sound_Index" && w["sound_id"] == animation["sfx_id"])
        );
        assert_eq!(animation["composition"]["status"], "exact");
        assert_eq!(animation["sprite_sheet_proven"], true);
        assert!(animation["frame_sequence_why_not"].is_null());
        // Names resolve through the shared SFX index; pin distinct retail IDs
        // without copying cartridge-derived names into this test.
        assert_eq!(
            animation["sfx_name"],
            crate::sound_defs::SFX_NAMES[animation["sfx_id"].as_u64().unwrap() as usize - 0xB5]
        );
        assert_sequence(animation);
    }
    assert_eq!(records[2]["sfx_id"], 0xD6);
    assert_eq!(records[10]["sfx_id"], 0xD8);
    assert_ne!(records[2]["sfx_name"], records[10]["sfx_name"]);
}

#[test]
fn distinct_playback_selector_and_external_plc_paths() {
    let Some(r) = retail() else { return };
    let records = &r.file("enemy_animations")["animations"];
    assert_eq!(
        records[10]["movement"]["runtime"]["initial_offset_pixels"],
        json!([0, 16])
    );
    assert_eq!(records[87]["movement"]["runtime"]["kind"], "linear");
    assert_eq!(
        records[87]["movement"]["runtime"]["step_pixels"],
        json!([0, 64])
    );
    assert_eq!(records[10]["frame_sequence"]["frame_count"], 8);
    assert_eq!(records[87]["frame_sequence"]["frame_count"], 4);
    assert_eq!(
        records[2]["frame_sequence"]["frame_timer_helper"],
        "loc_256F4"
    );
    assert!(!array(&records[24]["attack_plc"]["ranges"]).is_empty());
    assert!(
        array(&records[24]["frame_sequence"]["mapping_records"])
            .iter()
            .flat_map(|r| array(&r["entries"]))
            .flat_map(|e| array(&e["tile_sources"]))
            .any(|s| s["kind"] == "attack_plc")
    );
    assert_eq!(
        records[39]["frame_sequence"]["frame_timer_helper"],
        "loc_25270"
    );
    assert!(
        records[39]["frame_sequence"]["hidden_frames"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(records[149]["routine_classification"], "variable_mapping");
}

#[test]
fn oracle_records_keep_timing_offsets_origins_and_palette_evidence() {
    let Some(r) = retail() else { return };
    let records = &r.file("enemy_animations")["animations"];
    for id in [
        4, 7, 14, 27, 56, 78, 91, 96, 99, 114, 117, 121, 127, 128, 132, 151,
    ] {
        let record = &records[id];
        let receipt = &record["oracle_receipt"];
        assert_eq!(
            record["frame_sequence"]["frame_timer_helper"],
            "oracle:observed_sprite_table_clock"
        );
        assert!(array(&receipt["enemy_ids"]).contains(&json!(id)));
        let mut total = 0;
        for (duration, offset) in array(&receipt["durations"])
            .iter()
            .zip(array(&receipt["observed_offsets"]))
        {
            assert_eq!(offset.as_u64().unwrap(), total);
            total += duration.as_u64().unwrap();
        }
        assert_eq!(
            array(&receipt["durations"]).len(),
            array(&receipt["observed_offsets"]).len()
        );
        assert_eq!(record["frame_sequence"]["total_frames"], total);
        assert_eq!(receipt["capture"]["frame_count"], total);
        assert_eq!(array(&receipt["origin_pixels"]).len(), 2);
    }
    assert_eq!(
        records[27]["oracle_receipt"]["origin_pixels"],
        json!([-32, -56])
    );
    assert_eq!(
        records[132]["oracle_receipt"]["origin_pixels"],
        json!([-176, 0])
    );
    let dark = &records[130];
    assert_eq!(dark["oracle_receipt"]["cram_line"], 1);
    let palette = &dark["frame_sequence"]["palette_receipt"];
    assert_eq!(palette["cram_line"], 1);
    assert_eq!(palette["cram_words"][1], "0x0EEE");
}

#[test]
fn sequence_checks_reject_corrupt_durations_and_mapping_relationships() {
    let Some(r) = retail() else { return };
    let good = &r.file("enemy_animations")["animations"][10];
    assert_sequence(good);
    for path in ["duration", "pointer", "valid"] {
        let mut bad = good.clone();
        match path {
            "duration" => bad["frame_sequence"]["total_frames"] = json!(0),
            "pointer" => bad["frame_sequence"]["mapping_pointers"][0] = json!("0x000000"),
            _ => {
                bad["frame_sequence"]["mapping_records"][0]["entries"][0]["tile_bank_valid"] =
                    json!(false)
            }
        }
        assert!(
            std::panic::catch_unwind(|| assert_sequence(&bad)).is_err(),
            "{path}"
        );
    }
}
