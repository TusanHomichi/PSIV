//! The port of `psiv_tools/battle_animation_mapping.py`: one retail six-byte
//! VDP sprite mapping record (a count-minus-one byte, then the entries), and
//! `_frame_sequence_evidence` from `battle_animations.py`, which resolves each
//! selected record's patterns against the enemy bank and the attack PLC art.

use super::{hex6, int, string};
use crate::json::Json;

pub const HEADER_BYTES: usize = 1;
pub const ENTRY_BYTES: usize = 6;

fn signed_byte(value: u8) -> i64 {
    i64::from(value as i8)
}

/// `decode_mapping_record` with no bank bound: `Err` carries the message the
/// Python decoder raises, which the record keeps as `decode_error`.
pub fn decode_record(rom: &[u8], offset: usize) -> Result<Json, String> {
    if offset + HEADER_BYTES > rom.len() {
        return Err(format!(
            "mapping record at {} is outside the ROM",
            hex6(offset)
        ));
    }
    let sprite_count = usize::from(rom[offset]) + 1;
    let record_end = offset + HEADER_BYTES + sprite_count * ENTRY_BYTES;
    if sprite_count > 64 || record_end > rom.len() {
        return Err(format!(
            "mapping record at {} has {sprite_count} sprites or runs past ROM",
            hex6(offset)
        ));
    }
    let mut entries = Vec::new();
    for index in 0..sprite_count {
        let at = offset + HEADER_BYTES + index * ENTRY_BYTES;
        let size = rom[at + 1];
        let tile_word = u16::from_be_bytes([rom[at + 2], rom[at + 3]]);
        let (width_tiles, height_tiles) = crate::sprites::records::vdp_sprite_size(size);
        let tile_index = usize::from(tile_word & 0x07FF);
        let tile_span_end = tile_index + width_tiles * height_tiles;
        entries.push(
            Json::object()
                .with("rom_offset", hex6(at))
                .with("y", signed_byte(rom[at]))
                .with("size", size)
                .with("tile_word", format!("0x{tile_word:04X}"))
                .with("tile_index", tile_index)
                .with("h_flip", tile_word & 0x0800 != 0)
                .with("v_flip", tile_word & 0x1000 != 0)
                .with("priority", tile_word & 0x8000 != 0)
                .with("palette_bits", (tile_word >> 13) & 0x03)
                .with("x", signed_byte(rom[at + 4]))
                .with("x_mirror", signed_byte(rom[at + 5]))
                .with("width_tiles", width_tiles)
                .with("height_tiles", height_tiles)
                .with("tile_span_end", tile_span_end)
                .with("tile_bank_valid", true)
                // Palette bits select a different CRAM line; the
                // line-relative PNG surface cannot claim those pixels.
                .with("attributes_valid", tile_word & 0x6000 == 0),
        );
    }
    Ok(Json::object()
        .with("rom_offset", hex6(offset))
        .with("record_header_bytes", HEADER_BYTES)
        .with("entry_bytes", ENTRY_BYTES)
        .with("sprite_count", sprite_count)
        .with("entries", entries)
        .with("all_entries_valid", true)
        .with("record_end", hex6(record_end)))
}

/// `pattern_sources`: where each pattern of `[first, last)` comes from, the
/// enemy's own bank first and then the attack PLC ranges; empty when any
/// pattern has no source.
fn pattern_sources(first: i128, last: i128, bank: Option<i128>, plc: &[Json]) -> Vec<Json> {
    let mut sources = Vec::new();
    let mut cursor = first;
    while cursor < last {
        if let Some(bank) = bank
            && cursor < bank
        {
            let stop = last.min(bank);
            sources.push(
                Json::object()
                    .with("kind", "enemy_upload")
                    .with("first_pattern", Json::Int(cursor))
                    .with("pattern_count", Json::Int(stop - cursor))
                    .with("last_pattern_exclusive", Json::Int(stop))
                    .with("source", "loc_27F3AE bank order Art #3, Art #1, Art #2"),
            );
            cursor = stop;
            continue;
        }
        let Some(range) = plc.iter().find(|range| {
            int(range, "first_pattern") <= Some(cursor)
                && Some(cursor) < int(range, "last_pattern_exclusive")
        }) else {
            return Vec::new();
        };
        let stop = last.min(int(range, "last_pattern_exclusive").expect("a range end"));
        let mut source = range.clone();
        if let Json::Object(pairs) = &mut source {
            pairs.retain(|(key, _)| key != "compressed_bytes_read");
        }
        source.insert("first_pattern", Json::Int(cursor));
        source.insert("pattern_count", Json::Int(stop - cursor));
        source.insert("last_pattern_exclusive", Json::Int(stop));
        sources.push(source);
        cursor = stop;
    }
    sources
}

fn valid(record: &Json) -> bool {
    matches!(record.get("all_entries_valid"), Some(Json::Bool(true)))
}

/// `_frame_sequence_evidence`: the sequence with its records decoded, and the
/// composition verdict.
pub fn sequence_evidence(
    rom: &[u8],
    sequence: &Json,
    bank: Option<i128>,
    plc: &[Json],
) -> (Json, Json) {
    let pointers = match sequence.get("mapping_pointers") {
        Some(Json::Array(pointers)) => pointers.clone(),
        _ => Vec::new(),
    };
    let mut records = Vec::new();
    for pointer in &pointers {
        let Json::Str(pointer) = pointer else {
            records.push(
                Json::object()
                    .with("rom_offset", Json::Null)
                    .with("record_header_bytes", HEADER_BYTES)
                    .with("entry_bytes", ENTRY_BYTES)
                    .with("sprite_count", 0u8)
                    .with("entries", Vec::<Json>::new())
                    .with("all_entries_valid", true)
                    .with("record_end", Json::Null)
                    .with("hidden", true)
                    .with(
                        "reason",
                        "selector table stores a negative pointer: retail hides this frame",
                    ),
            );
            continue;
        };
        let offset = usize::from_str_radix(pointer.trim_start_matches("0x"), 16)
            .expect("a hex pointer this module wrote");
        match decode_record(rom, offset) {
            Ok(mut record) => {
                let mut all = true;
                if let Some(Json::Array(entries)) = record_entries(&mut record) {
                    for entry in entries.iter_mut() {
                        let sources = pattern_sources(
                            int(entry, "tile_index").expect("tile_index"),
                            int(entry, "tile_span_end").expect("tile_span_end"),
                            bank,
                            plc,
                        );
                        let bank_valid = !sources.is_empty();
                        let attributes =
                            matches!(entry.get("attributes_valid"), Some(Json::Bool(true)));
                        all &= bank_valid && attributes;
                        entry.insert("tile_sources", sources);
                        entry.insert("tile_bank_valid", bank_valid);
                    }
                }
                record.insert("all_entries_valid", all);
                records.push(record);
            }
            Err(message) => records.push(
                Json::object()
                    .with("rom_offset", hex6(offset))
                    .with("record_header_bytes", HEADER_BYTES)
                    .with("entry_bytes", ENTRY_BYTES)
                    .with("sprite_count", Json::Null)
                    .with("entries", Vec::<Json>::new())
                    .with("all_entries_valid", false)
                    .with("record_end", Json::Null)
                    .with("decode_error", message),
            ),
        }
    }
    let exact = records.iter().filter(|r| valid(r)).count();
    let (status, reason) = if exact == records.len() {
        let external = records
            .iter()
            .flat_map(entries_of)
            .flat_map(|entry| match entry.get("tile_sources") {
                Some(Json::Array(sources)) => sources.clone(),
                _ => Vec::new(),
            })
            .filter(|source| string(source, "kind") == Some("attack_plc"))
            .count();
        let hidden = records
            .iter()
            .filter(|r| matches!(r.get("hidden"), Some(Json::Bool(true))))
            .count();
        let mut reason = "every selected six-byte VDP mapping resolves against proven enemy or \
                          attack PLC art"
            .to_owned();
        if external != 0 {
            reason += &format!("; {external} sprite span(s) use routine LoadPLC1 art");
        }
        if hidden != 0 {
            reason += &format!("; {hidden} selector frame(s) are retail hide records");
        }
        ("exact", reason)
    } else if exact != 0 {
        (
            "partial",
            format!(
                "{} selected mapping record(s) lack proven enemy/PLC coverage or set palette bits",
                records.len() - exact
            ),
        )
    } else {
        (
            "deferred",
            "none of the selected mapping records resolves within the enemy art bank".to_owned(),
        )
    };
    let mut sequence = sequence.clone();
    sequence.insert("mapping_records", records);
    (
        sequence,
        Json::object().with("status", status).with("reason", reason),
    )
}

fn record_entries(record: &mut Json) -> Option<&mut Json> {
    match record {
        Json::Object(pairs) => pairs
            .iter_mut()
            .find(|(key, _)| key == "entries")
            .map(|(_, value)| value),
        _ => None,
    }
}

/// A record's entries, or none.
pub fn entries_of(record: &Json) -> Vec<Json> {
    match record.get("entries") {
        Some(Json::Array(entries)) => entries.clone(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_decodes_signed_offsets_sizes_and_attribute_bits() {
        let mut rom = vec![0u8; 16];
        rom[2..9].copy_from_slice(&[0, 0xF8, 0x05, 0x08, 0x10, 0xFC, 0x04]);
        let record = decode_record(&rom, 2).unwrap();
        let entry = &entries_of(&record)[0];
        let value = serde_json::Value::from(entry);
        assert_eq!(value["y"], -8);
        assert_eq!(
            (
                value["width_tiles"].as_u64(),
                value["height_tiles"].as_u64()
            ),
            (Some(2), Some(2))
        );
        assert_eq!(value["tile_index"], 0x010);
        assert_eq!(value["tile_span_end"], 0x014);
        assert_eq!(value["h_flip"], true);
        assert_eq!(value["attributes_valid"], true);
        assert_eq!(value["x"], -4);
        assert_eq!(serde_json::Value::from(&record)["record_end"], "0x000009");
    }

    #[test]
    fn a_non_square_entry_is_columns_in_bits_3_2_and_rows_in_bits_1_0() {
        // #364: the VDP's reading; size $09 is three columns of two rows.
        for (size, expected) in [(0x09, (3, 2)), (0x06, (2, 3)), (0x0C, (4, 1))] {
            let rom = [0, 0, size, 0x00, 0x10, 0, 0];
            let record = decode_record(&rom, 0).unwrap();
            let value = serde_json::Value::from(&entries_of(&record)[0]);
            assert_eq!(
                (
                    value["width_tiles"].as_u64(),
                    value["height_tiles"].as_u64()
                ),
                (Some(expected.0), Some(expected.1)),
                "size {size:#04x}"
            );
        }
    }

    #[test]
    fn a_record_past_the_rom_or_too_long_is_refused_with_its_message() {
        let rom = [0x02u8, 0, 0];
        assert_eq!(
            decode_record(&rom, 0).unwrap_err(),
            "mapping record at 0x000000 has 3 sprites or runs past ROM"
        );
        assert_eq!(
            decode_record(&rom, 3).unwrap_err(),
            "mapping record at 0x000003 is outside the ROM"
        );
    }

    #[test]
    fn patterns_resolve_from_the_bank_then_the_plc_ranges() {
        let plc = [Json::object()
            .with("kind", "attack_plc")
            .with("first_pattern", 10u8)
            .with("last_pattern_exclusive", 20u8)
            .with("compressed_bytes_read", 99u8)];
        let sources = pattern_sources(8, 12, Some(10), &plc);
        assert_eq!(sources.len(), 2);
        let second = serde_json::Value::from(&sources[1]);
        assert_eq!(second["pattern_count"], 2);
        assert!(second.get("compressed_bytes_read").is_none());
        assert!(pattern_sources(18, 22, Some(10), &plc).is_empty());
    }

    #[test]
    fn a_tile_index_keeps_all_eleven_bits() {
        let rom = [0u8, 0, 0, 0x04, 0x10, 0, 0];
        let record = decode_record(&rom, 0).unwrap();
        let entry = serde_json::Value::from(&entries_of(&record)[0]);
        assert_eq!(entry["tile_index"], 0x410);
        assert_eq!(entry["tile_span_end"], 0x411);
    }
}
