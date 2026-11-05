//! The bytes a fresh-process CONTINUE, one harmless Down step and a SAVE may
//! change: the check behind `psiv-campaign verify-continue`, which reads what
//! `tools/native/native_continue.gd` leaves (the source slot, the resaved
//! slot and the driver's receipt).
//!
//! Every gameplay byte but the `$F408` standing-Y word must stay; the header
//! may change only its slot word and checksum table, gain the `SY` marker
//! when the source lacks it, and, when the source is a cartridge image (no
//! native metadata), gain the metadata the slot writer adds: the roster mask
//! of exactly the records the codec seats, and a valid `TR` exit. Which
//! convention a slot's Y word is in, and which records it seats, are the
//! codec's own rules ([`RetailSlot::decode`]), so this check cannot disagree
//! with the loader. Use on checkpoint fixtures where the step has no
//! gameplay effect (no poison tick, encounter, warp or story trigger).

use std::collections::BTreeMap;

use psiv_core::{RETAIL_SLOT_COUNT, RETAIL_SLOT_FILE_BYTES, RetailSlot};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

/// The header's slot word and its three-word checksum table, `$F012..$F01A`.
const SLOT_AND_CHECKSUMS: core::ops::Range<usize> = 0x12..0x1A;
/// The native roster mask, `$F020..$F022`.
const ROSTER_MASK: core::ops::Range<usize> = 0x20..0x22;
/// The native `TR` dungeon exit, `$F022..$F026`.
const DUNGEON_EXIT: core::ops::Range<usize> = 0x22..0x26;
/// The `SY` standing-Y marker, `$F02E..$F030`.
const STANDING_Y_MARKER: core::ops::Range<usize> = 0x2E..0x30;
/// The `$F408` word, the only payload bytes the step may change.
const STANDING_Y_WORD: [usize; 2] = [0x308, 0x309];
/// The receipt state keys a harmless step leaves alone.
const UNCHANGED_KEYS: [&str; 8] = [
    "map",
    "world",
    "flags",
    "money",
    "leader",
    "inventory",
    "party_status",
    "party_resources",
];

/// What changed between a passing source and its resave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinueReport {
    /// The source file's SHA-256, lower-case hex.
    pub source_sha256: String,
    /// The resaved file's SHA-256, lower-case hex.
    pub resaved_sha256: String,
    /// Logical header offsets that changed.
    pub header_changes: Vec<usize>,
    /// Logical payload offsets that changed, with their old and new bytes.
    pub payload_changes: Vec<(usize, u8, u8)>,
}

impl ContinueReport {
    /// The JSON the command prints and writes to `--out`.
    #[must_use]
    pub fn to_json(&self, source: &str) -> Value {
        json!({
            "complete": true,
            "source": source,
            "source_sha256": self.source_sha256,
            "resaved_sha256": self.resaved_sha256,
            "header_slot_and_checksum_offsets": self
                .header_changes
                .iter()
                .map(|offset| format!("0x{offset:02x}"))
                .collect::<Vec<_>>(),
            "logical_payload_changes": self
                .payload_changes
                .iter()
                .map(|(offset, before, after)| json!({
                    "offset": format!("0x{offset:03x}"),
                    "before": before,
                    "after": after,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

/// The SHA-256 of `bytes`, lower-case hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Checks a source slot, its resave and the driver's receipt.
///
/// # Errors
/// The first rule the files break, in words.
pub fn verify(
    source: &[u8],
    source_sha256: &str,
    resaved: &[u8],
    receipt: &Value,
) -> Result<ContinueReport, String> {
    let digest = sha256_hex(source);
    if digest != source_sha256.to_ascii_lowercase() {
        return Err("Continue modified the source save".into());
    }
    if source.len() != RETAIL_SLOT_FILE_BYTES || resaved.len() != RETAIL_SLOT_FILE_BYTES {
        return Err(format!(
            "a slot file is not {RETAIL_SLOT_FILE_BYTES:#x} bytes"
        ));
    }
    check_receipt(receipt)?;
    if source.iter().step_by(2).ne(resaved.iter().step_by(2)) {
        return Err("unused interleaved bytes changed".into());
    }
    let old = slot("source", source)?;
    let new = slot("resave", resaved)?;

    let (old_header, new_header) = (old.logical_header(), new.logical_header());
    let header_changes: Vec<usize> = (0..old_header.len())
        .filter(|&i| old_header[i] != new_header[i])
        .collect();
    let mut allowed: Vec<usize> = SLOT_AND_CHECKSUMS.collect();
    if !old.has_standing_y_marker() {
        allowed.extend(STANDING_Y_MARKER);
    }
    if !old.has_native_metadata() {
        // A cartridge image's first native SAVE adds what the writer always
        // adds: the occupancy of the records it seated, and `TR`.
        allowed.extend(ROSTER_MASK);
        allowed.extend(DUNGEON_EXIT);
        if new.dungeon_exit().is_none() {
            return Err("the resave's dungeon exit is not the writer's".into());
        }
        let seats = decode("source", &old)?.snapshot.occupancy_mask();
        if new.roster_mask() != seats {
            return Err(format!(
                "the resave's roster mask {:#06x} is not the source's seats {seats:#06x}",
                new.roster_mask()
            ));
        }
    }
    if !new.has_standing_y_marker() {
        return Err("the resave lacks the SY marker".into());
    }
    if let Some(offset) = header_changes.iter().find(|i| !allowed.contains(i)) {
        return Err(format!(
            "non-checksum header state changed at 0x{offset:02x}"
        ));
    }

    let (old_payload, new_payload) = (old.logical_payload(), new.logical_payload());
    let payload_changes: Vec<(usize, u8, u8)> = (0..old_payload.len())
        .filter(|&i| old_payload[i] != new_payload[i])
        .map(|i| (i, old_payload[i], new_payload[i]))
        .collect();
    let unexpected: Vec<String> = payload_changes
        .iter()
        .map(|(offset, _, _)| *offset)
        .filter(|offset| !STANDING_Y_WORD.contains(offset))
        .map(|offset| format!("0x{offset:03x}"))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "unexpected gameplay changes: {}",
            unexpected.join(", ")
        ));
    }
    let before = decode("source", &old)?.location.char_y;
    let after = decode("resave", &new)?.location.char_y;
    if before.checked_add(16) != Some(after) {
        return Err("the step is not one row down".into());
    }
    Ok(ContinueReport {
        source_sha256: digest,
        resaved_sha256: sha256_hex(resaved),
        header_changes,
        payload_changes,
    })
}

/// The receipt finished, and its `continued`, `moved` and `resaved` states
/// are one cell down and otherwise the same.
fn check_receipt(receipt: &Value) -> Result<(), String> {
    if receipt.get("complete") != Some(&Value::Bool(true)) {
        return Err("native Continue flow did not finish".into());
    }
    let observations = receipt
        .get("observations")
        .and_then(Value::as_array)
        .ok_or("the receipt has no observations")?;
    let states: BTreeMap<&str, &Value> = observations
        .iter()
        .filter_map(|o| Some((o.get("label")?.as_str()?, o.get("state")?)))
        .collect();
    let [first, moved, last] = ["continued", "moved", "resaved"].map(|label| states.get(label));
    let (Some(first), Some(moved), Some(last)) = (first, moved, last) else {
        return Err("the receipt lacks a continued, moved or resaved state".into());
    };
    let cell = |state: &Value| -> Option<(u16, u16)> {
        match state.get("cell")?.as_array()?.as_slice() {
            [x, y] => Some((coordinate(x)?, coordinate(y)?)),
            _ => None,
        }
    };
    let (x, y) = cell(first).ok_or("the continued state has no cell")?;
    let below = y.checked_add(1).map(|y| (x, y));
    if below.is_none() || cell(moved) != below || cell(last) != below {
        return Err("the step is not one cell down".into());
    }
    for key in UNCHANGED_KEYS {
        let value = first.get(key);
        if value.is_none() || value != moved.get(key) || value != last.get(key) {
            return Err(format!("one-step Continue changed {key}"));
        }
    }
    Ok(())
}

/// A cell coordinate, a [`psiv_core::Cell`] `u16`, as the receipt holds it.
///
/// `native_continue.gd` re-serializes the extension's integers through
/// Godot's JSON, which parses every number as a float and writes `38` back
/// as `38.0`, so an integral float is the same coordinate. A fractional,
/// negative or out-of-range number is no cell.
fn coordinate(value: &Value) -> Option<u16> {
    if let Some(integer) = value.as_u64() {
        return u16::try_from(integer).ok();
    }
    let float = value.as_f64()?;
    let in_range =
        float.is_finite() && float.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(&float);
    // Exact: an integral float in `0..=u16::MAX` converts without loss.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    in_range.then_some(float as u16)
}

/// A validated slot at the slot number its own header names.
fn slot(name: &str, bytes: &[u8]) -> Result<RetailSlot, String> {
    let index = usize::from(u16::from_be_bytes([bytes[0x25], bytes[0x27]]));
    if index >= RETAIL_SLOT_COUNT {
        return Err(format!("the {name}'s slot word {index} is not a slot"));
    }
    RetailSlot::from_bytes(bytes, index).map_err(|e| format!("the {name} is not a valid slot: {e}"))
}

fn decode(name: &str, slot: &RetailSlot) -> Result<psiv_core::RetailSave, String> {
    slot.decode()
        .map_err(|e| format!("the {name} does not decode: {e}"))
}
