//! The port of `psiv_tools/battle_animation_receipts.py`: the checked-in
//! oracle receipts that close the residual enemy attack presentations. Those
//! module docs give the reasons. The receipt file is a tracked fixture, built
//! into the program so the pack still needs nothing but the ROM.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{int, string};
use crate::PackError;
use crate::json::Json;

const RECEIPTS: &str =
    include_str!("../../../../../oracle/fixtures/battle_animation_remainder.json");

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

/// `receipt_file` and `receipts_by_routine`, validated as Python validates
/// them.
pub struct Receipts {
    by_routine: BTreeMap<String, Value>,
    darkforce1: Value,
}

fn ints(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

impl Receipts {
    pub fn load() -> Result<Self, PackError> {
        Self::parse(RECEIPTS)
    }

    fn parse(text: &str) -> Result<Self, PackError> {
        let file: Value = serde_json::from_str(text)
            .map_err(|error| PackError(format!("battle-animation receipt: {error}")))?;
        if file["format_version"] != 1 {
            return refuse("unsupported battle-animation receipt version".into());
        }
        if file["source"]["grand_cross"] != 0 {
            return refuse("battle-animation receipt is not Grand Cross=0".into());
        }
        if file["fixture"]["enemy_position"] != "0x0E" {
            return refuse(
                "battle-animation fixture does not record Enemy_Positions[0] = $0E".into(),
            );
        }
        let mut by_routine = BTreeMap::new();
        for receipt in file["receipts"].as_array().into_iter().flatten() {
            let Some(routine) = receipt["routine_offset"].as_str() else {
                return refuse("duplicate or missing receipt routine offset".into());
            };
            if by_routine.contains_key(routine) {
                return refuse(format!(
                    "duplicate or missing receipt routine offset: {routine:?}"
                ));
            }
            let offsets = ints(&receipt["observed_offsets"]);
            let durations = ints(&receipt["durations"]);
            if offsets.is_empty() || offsets.len() != durations.len() {
                return refuse(format!(
                    "{routine}: observed offsets and durations do not line up"
                ));
            }
            if offsets[0] != 0 || offsets.iter().any(|&v| v < 0) {
                return refuse(format!("{routine}: invalid observation offsets"));
            }
            if durations.iter().any(|&v| v <= 0) {
                return refuse(format!("{routine}: non-positive observed duration"));
            }
            let capture = &receipt["capture"];
            if !capture.is_object() {
                return refuse(format!("{routine}: capture hash receipt is missing"));
            }
            if Some(durations.iter().sum::<i64>()) != capture["frame_count"].as_i64() {
                return refuse(format!(
                    "{routine}: observed durations do not cover the capture window"
                ));
            }
            let mut start = 0;
            for (&offset, &duration) in offsets.iter().zip(&durations) {
                if offset != start {
                    return refuse(format!(
                        "{routine}: offsets are not the starts of the observed dwells"
                    ));
                }
                start += duration;
            }
            let first = capture["frame_first"].as_i64().unwrap_or(-1);
            let count = capture["frame_count"].as_i64().unwrap_or(0);
            if capture["frame_last_exclusive"].as_i64() != Some(first + count) {
                return refuse(format!("{routine}: capture frame range is inconsistent"));
            }
            if ints(&receipt["enemy_ids"]).is_empty() {
                return refuse(format!("{routine}: receipt has no enemy ids"));
            }
            let origin = receipt["origin_pixels"].as_array();
            if !origin.is_some_and(|o| o.len() == 2 && o.iter().all(Value::is_i64)) {
                return refuse(format!(
                    "{routine}: formation-relative origin_pixels are missing"
                ));
            }
            by_routine.insert(routine.to_owned(), receipt.clone());
        }
        let darkforce1 = file["darkforce1"].clone();
        if !darkforce1.is_object() {
            return refuse("DarkForce1 palette receipt is missing".into());
        }
        let words = darkforce1["cram_words"].as_array().map_or(0, Vec::len);
        if words != 16 || !(darkforce1["cram_line"] == 1 || darkforce1["cram_line"] == 2) {
            return refuse("DarkForce1 CRAM receipt is malformed".into());
        }
        Ok(Receipts {
            by_routine,
            darkforce1,
        })
    }

    /// `apply_oracle_receipt`: close one extracted animation.
    pub fn apply(&self, animation: &mut Json) -> Result<(), PackError> {
        if int(animation, "enemy_id") == Some(130) {
            return self.apply_darkforce1(animation);
        }
        let routine = string(animation, "routine_offset").unwrap_or_default();
        if let Some(receipt) = self.by_routine.get(routine) {
            let id = int(animation, "enemy_id").unwrap_or(-1) as i64;
            if ints(&receipt["enemy_ids"]).contains(&id) {
                apply_observed(animation, receipt);
            }
        }
        Ok(())
    }

    fn apply_darkforce1(&self, animation: &mut Json) -> Result<(), PackError> {
        let receipt = &self.darkforce1;
        let hex = |key: &str| {
            receipt[key]
                .as_str()
                .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .unwrap_or(0)
        };
        let (mask, masked) = (hex("palette_mask"), hex("palette_word_masked"));
        let selector = Json::from(&receipt["palette_selector"]);
        let cram_line = Json::from(&receipt["cram_line"]);
        let Some(Json::Object(_)) = animation.get("frame_sequence") else {
            return refuse("DarkForce1 has no static mapping sequence to resolve".into());
        };
        let mut sequence = animation.get("frame_sequence").cloned().expect("checked");
        let mut records = match sequence.get("mapping_records") {
            Some(Json::Array(records)) => records.clone(),
            _ => Vec::new(),
        };
        for record in &mut records {
            let mut entries = super::mapping::entries_of(record);
            for entry in &mut entries {
                let word = string(entry, "tile_word")
                    .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0);
                if word & mask != 0 {
                    if word & mask != masked {
                        return refuse(format!(
                            "DarkForce1 palette word {word:#06x} disagrees with receipt"
                        ));
                    }
                    if entry.get("palette_bits") != Some(&selector) {
                        return refuse("DarkForce1 palette selector disagrees with receipt".into());
                    }
                    entry.insert("cram_line", cram_line.clone());
                }
                entry.insert("attributes_valid", true);
            }
            if record.get("entries").is_some() {
                record.insert("entries", entries);
            }
            record.insert("all_entries_valid", true);
            record.insert(
                "palette_receipt",
                Json::object()
                    .with("cram_line", cram_line.clone())
                    .with("palette_mask", Json::from(&receipt["palette_mask"]))
                    .with(
                        "palette_word_masked",
                        Json::from(&receipt["palette_word_masked"]),
                    ),
            );
        }
        sequence.insert("mapping_records", records);
        sequence.insert(
            "palette_receipt",
            Json::object()
                .with("cram_line", cram_line.clone())
                .with("cram_words", Json::from(&receipt["cram_words"]))
                .with("formation_patch", Json::from(&receipt["formation_patch"]))
                .with("receipt", Json::from(&receipt["receipt"])),
        );
        animation.insert("frame_sequence", sequence);
        animation.insert(
            "composition",
            Json::object().with("status", "exact").with(
                "reason",
                format!(
                    "DarkForce1 $6000 palette-bit mapping is resolved by the oracle formation \
                     CRAM receipt: selector {} selects CRAM line {} ({})",
                    python_str(&receipt["palette_selector"]),
                    python_str(&receipt["cram_line"]),
                    python_str(&receipt["receipt"]),
                ),
            ),
        );
        animation.insert("oracle_receipt", Json::from(receipt));
        animation.insert("sprite_sheet_proven", true);
        Ok(())
    }
}

/// `str(value)` for the scalars an f-string interpolates.
fn python_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `_oracle_mapping_record`: a record shell for one observed hardware frame.
fn oracle_record(receipt: &Value, index: usize) -> Json {
    let offset = receipt["observed_offsets"][index].as_i64().unwrap_or(0);
    let frame = receipt["action_frame"].as_i64().unwrap_or(0) + offset;
    let routine = python_str(&receipt["routine_offset"]);
    Json::object()
        .with("rom_offset", format!("oracle:{routine}:frame{index:02}"))
        .with("record_header_bytes", 0u8)
        .with("entry_bytes", 8u8)
        .with("sprite_count", Json::Null)
        .with("entries", Vec::<Json>::new())
        .with("all_entries_valid", true)
        .with("record_end", Json::Null)
        .with("observed_frame", frame)
        .with("observed_offset", offset)
        .with("observation", "oracle state_dump sprite_table + vdp_vram")
        .with("receipt", Json::from(&receipt["receipt"]))
}

/// `_apply_observed_receipt`.
fn apply_observed(animation: &mut Json, receipt: &Value) {
    let routine = python_str(&receipt["routine_offset"]);
    let durations = ints(&receipt["durations"]);
    let pointers: Vec<String> = (0..durations.len())
        .map(|index| format!("oracle:{routine}:frame{index:02}"))
        .collect();
    let sequence = Json::object()
        .with("object_id", Json::Null)
        .with(
            "assignment_offset",
            format!("oracle:{routine}:sprite-table"),
        )
        .with("mapping_offset", format!("oracle:{routine}:observed"))
        .with("frame_duration", durations[0])
        .with("frame_count", durations.len())
        .with("total_frames", durations.iter().sum::<i64>())
        .with("mapping_pointers", pointers)
        .with("frame_timer_helper", "oracle:observed_sprite_table_clock")
        .with("frame_durations", durations.clone())
        .with("timing_model", "observed")
        .with(
            "mapping_records",
            (0..durations.len())
                .map(|index| oracle_record(receipt, index))
                .collect::<Vec<_>>(),
        )
        .with(
            "receipt",
            Json::object()
                .with("routine_offset", routine.as_str())
                .with(
                    "representative_enemy_id",
                    Json::from(&receipt["representative_enemy_id"]),
                )
                .with("action_frame", Json::from(&receipt["action_frame"]))
                .with("observed_offsets", Json::from(&receipt["observed_offsets"]))
                .with("capture", Json::from(&receipt["capture"]))
                .with("source", Json::from(&receipt["receipt"])),
        );
    animation.insert("frame_sequence", sequence);
    animation.insert("frame_sequence_why_not", Json::Null);
    animation.insert(
        "composition",
        Json::object().with("status", "exact").with(
            "reason",
            format!(
                "oracle receipt resolves the retail presentation as the observed {} sprite-table \
                 state; {}",
                python_str(&receipt["presentation"]),
                python_str(&receipt["receipt"]),
            ),
        ),
    );
    let previous = animation.get("movement").cloned().unwrap_or(Json::Null);
    let keep = |key: &str| {
        previous
            .get(key)
            .cloned()
            .unwrap_or(Json::Array(Vec::new()))
    };
    animation.insert(
        "movement",
        Json::object()
            .with("status", "exact")
            .with(
                "reason",
                "observed SAT frames carry the retail effect motion; the local attack layer \
                 stays at the receipt-backed formation anchor",
            )
            .with("fields", super::movement::fields())
            .with("writes", keep("writes"))
            .with("tracks", keep("tracks"))
            .with(
                "track_completion",
                Json::object()
                    .with("status", "exact")
                    .with("fields", vec!["$2C(a4)", "$2E(a4)", "$30(a4)", "$34(a4)"])
                    .with(
                        "reason",
                        "the oracle frame carries sprite/effect motion; layer anchor is \
                         receipt-backed",
                    ),
            )
            .with(
                "runtime",
                Json::object()
                    .with("kind", "observed_receipt")
                    .with("initial_offset_pixels", vec![0u8, 0])
                    .with("step_pixels", vec![0u8, 0])
                    .with("branches", Vec::<Json>::new()),
            ),
    );
    animation.insert("movement_proven", true);
    animation.insert("flash_timing_proven", true);
    animation.insert("sprite_sheet_proven", true);
    animation.insert("oracle_receipt", Json::from(receipt));
    let reason = string(animation, "routine_classification_reason")
        .unwrap_or_default()
        .to_owned();
    animation.insert(
        "routine_classification_reason",
        reason + "; completed by oracle hardware receipt",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tracked_receipts_load() {
        let receipts = Receipts::load().unwrap();
        assert!(!receipts.by_routine.is_empty());
    }

    #[test]
    fn a_receipt_whose_offsets_skip_a_dwell_is_refused() {
        let mut file: Value = serde_json::from_str(RECEIPTS).unwrap();
        let receipt = &mut file["receipts"][0];
        let offsets = receipt["observed_offsets"].as_array_mut().unwrap();
        if offsets.len() > 1 {
            offsets[1] = Value::from(offsets[1].as_i64().unwrap() + 1);
        } else {
            offsets[0] = Value::from(1);
        }
        let error = Receipts::parse(&file.to_string()).err().unwrap();
        assert!(error.0.contains("offsets"), "{}", error.0);
    }

    #[test]
    fn a_receipt_off_grand_cross_zero_is_refused() {
        let mut file: Value = serde_json::from_str(RECEIPTS).unwrap();
        file["source"]["grand_cross"] = Value::from(1);
        assert!(Receipts::parse(&file.to_string()).is_err());
    }
}
