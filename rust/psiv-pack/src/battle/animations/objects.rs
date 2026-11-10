//! The port of `psiv_tools/battle_animation_objects.py`: the byte-level
//! decoders for one battle object's helper calls, `$08`/`$28` pointer
//! assignments, timed mapping sequences (fixed, variable and selector), and
//! `LoadPLC1` art loads. That module's docs give the four presentation
//! contracts these read.

use std::collections::BTreeSet;

use super::{hex6, int, long, string, word};
use crate::decode::nemesis;
use crate::json::Json;

const FRAME_HELPERS: [(i64, &str); 5] = [
    (0x025270, "loc_25270"),
    (0x0256AE, "loc_256AE"),
    (0x0256F4, "loc_256F4"),
    (0x0258AC, "loc_258AC"),
    (0x025978, "loc_25978"),
];
const LOAD_PLC1: i64 = 0x00B790;
const PLC_OFFSET: usize = 0x27E88A;
const TILE_SIZE: usize = 32;

fn s16(value: u16) -> i64 {
    i64::from(value as i16)
}

/// `_call_target`: jsr/jmp abs.l, or jsr/jmp (d16,pc) as the Python reads it.
fn call_target(rom: &[u8], at: usize) -> Option<i64> {
    match word(rom, at)? {
        0x4EB9 | 0x4EF9 => long(rom, at + 2).map(i64::from),
        0x4EBA | 0x4EFA => Some(at as i64 + 4 + s16(word(rom, at + 2)?)),
        _ => None,
    }
}

/// `helper_calls`: absolute and PC-relative calls to the known helpers.
pub fn helper_calls(rom: &[u8], start: usize, end: usize) -> Vec<Json> {
    let stop = start.max(end.saturating_sub(3));
    let mut calls = Vec::new();
    for at in (start..stop).step_by(2) {
        let Some(target) = call_target(rom, at) else {
            continue;
        };
        let helper = if target == LOAD_PLC1 {
            "LoadPLC1"
        } else if let Some((_, name)) = FRAME_HELPERS.iter().find(|(t, _)| *t == target) {
            name
        } else {
            continue;
        };
        calls.push(
            Json::object()
                .with("rom_offset", hex6(at))
                .with("target", hex6(target as usize))
                .with("helper", helper),
        );
    }
    calls
}

/// `_pointer_assignments`: `move.l #imm,$08(a4)` and `$28(a4)`.
fn pointer_assignments(rom: &[u8], start: usize, end: usize) -> Vec<(Json, usize)> {
    let mut out = Vec::new();
    for at in (start..start.max(end.saturating_sub(9))).step_by(2) {
        if word(rom, at) != Some(0x297C) {
            continue;
        }
        let Some(field) = word(rom, at + 6) else {
            continue;
        };
        if field != 0x08 && field != 0x28 {
            continue;
        }
        let pointer = long(rom, at + 2).expect("a pointer inside the span") as usize;
        out.push((
            Json::object()
                .with("rom_offset", hex6(at))
                .with("field", format!("${field:02X}(a4)"))
                .with("pointer", hex6(pointer)),
            pointer,
        ));
    }
    out
}

fn valid_pointer(rom: &[u8], pointer: usize) -> bool {
    pointer < rom.len() && pointer & 1 == 0
}

/// `_mapping_pointers`.
fn mapping_pointers(rom: &[u8], count: usize, base: usize) -> Option<Vec<usize>> {
    if !(1..=64).contains(&count) || base + count * 4 > rom.len() {
        return None;
    }
    let values: Vec<usize> = (0..count)
        .map(|i| long(rom, base + i * 4).map(|v| v as usize))
        .collect::<Option<_>>()?;
    values
        .iter()
        .all(|&v| valid_pointer(rom, v))
        .then_some(values)
}

fn hexes(pointers: &[usize]) -> Vec<String> {
    pointers.iter().map(|&p| hex6(p)).collect()
}

/// The keys every sequence shares.
fn sequence(
    object_id: &Json,
    assignment: &Json,
    data: usize,
    durations: &[u8],
    helper: &str,
    model: &str,
) -> Json {
    Json::object()
        .with("object_id", object_id.clone())
        .with(
            "assignment_offset",
            assignment.get("rom_offset").cloned().unwrap_or(Json::Null),
        )
        .with("mapping_offset", hex6(data))
        .with("frame_duration", durations[0])
        .with("frame_durations", durations.to_vec())
        .with("frame_count", durations.len())
        .with(
            "total_frames",
            durations.iter().map(|&d| u64::from(d)).sum::<u64>(),
        )
        .with("frame_timer_helper", helper)
        .with("timing_model", model)
}

/// `_fixed_sequence`: a duration byte, a count byte, then the pointers.
fn fixed(rom: &[u8], object_id: &Json, assignment: &(Json, usize), helper: &str) -> Option<Json> {
    let data = assignment.1;
    if data + 2 > rom.len() {
        return None;
    }
    let (duration, count) = (rom[data], usize::from(rom[data + 1]));
    let pointers = mapping_pointers(rom, count, data + 2)?;
    if duration == 0 {
        return None;
    }
    Some(
        sequence(
            object_id,
            &assignment.0,
            data,
            &vec![duration; count],
            helper,
            "fixed",
        )
        .with("mapping_pointers", hexes(&pointers)),
    )
}

/// `_variable_sequence`: a count, the durations, word alignment, the pointers.
fn variable(
    rom: &[u8],
    object_id: &Json,
    assignment: &(Json, usize),
    helper: &str,
) -> Option<Json> {
    let data = assignment.1;
    if data + 1 > rom.len() {
        return None;
    }
    let count = usize::from(rom[data]);
    if !(1..=64).contains(&count) {
        return None;
    }
    let durations_start = data + 1;
    let pointers_start = durations_start + count + if count & 1 == 1 { 0 } else { 1 };
    if pointers_start + count * 4 > rom.len() {
        return None;
    }
    let durations = &rom[durations_start..durations_start + count];
    let pointers: Vec<usize> = (0..count)
        .map(|i| long(rom, pointers_start + i * 4).expect("in range") as usize)
        .collect();
    if durations.contains(&0) || pointers.iter().any(|&p| !valid_pointer(rom, p)) {
        return None;
    }
    Some(
        sequence(
            object_id,
            &assignment.0,
            data,
            durations,
            helper,
            "variable",
        )
        .with("mapping_pointers", hexes(&pointers))
        .with("duration_table_end", hex6(pointers_start)),
    )
}

/// `_selector_sequence`: durations and selector bytes into a pointer table
/// whose negative entries hide the sprite for that tick.
fn selector(
    rom: &[u8],
    object_id: &Json,
    data_assignment: &(Json, usize),
    table_assignment: &(Json, usize),
    helper: &str,
) -> Option<Json> {
    let (data, table) = (data_assignment.1, table_assignment.1);
    if data + 1 > rom.len() {
        return None;
    }
    let count = usize::from(rom[data]);
    if !(1..=64).contains(&count) || data + 1 + count * 2 > rom.len() {
        return None;
    }
    let durations = &rom[data + 1..data + 1 + count];
    let selectors = &rom[data + 1 + count..data + 1 + count * 2];
    let mut pointers = Vec::new();
    for &selector in selectors {
        let at = table + usize::from(selector);
        if at + 4 > rom.len() {
            return None;
        }
        let pointer = long(rom, at)?;
        pointers.push((pointer & 0x8000_0000 == 0).then_some(pointer as usize));
    }
    if durations.contains(&0) || pointers.iter().flatten().any(|&p| !valid_pointer(rom, p)) {
        return None;
    }
    Some(
        sequence(
            object_id,
            &data_assignment.0,
            data,
            durations,
            helper,
            "selector",
        )
        .with(
            "table_assignment_offset",
            table_assignment
                .0
                .get("rom_offset")
                .cloned()
                .unwrap_or(Json::Null),
        )
        .with("selector_table_offset", hex6(table))
        .with(
            "mapping_pointers",
            pointers.iter().map(|p| p.map(hex6)).collect::<Vec<_>>(),
        )
        .with("selectors", selectors.to_vec())
        .with(
            "hidden_frames",
            pointers.iter().filter(|p| p.is_none()).count(),
        ),
    )
}

/// `frame_sequences`: every candidate sequence the object's helper calls
/// select, deduplicated by helper, data and pointers.
pub fn frame_sequences(rom: &[u8], start: usize, end: usize, object_id: &Json) -> Vec<Json> {
    let assignments = pointer_assignments(rom, start, end);
    let field = |a: &&(Json, usize), name: &str| string(&a.0, "field") == Some(name);
    let data_assignments: Vec<&(Json, usize)> =
        assignments.iter().filter(|a| field(a, "$08(a4)")).collect();
    let table_assignments: Vec<&(Json, usize)> =
        assignments.iter().filter(|a| field(a, "$28(a4)")).collect();
    let mut candidates: Vec<Json> = Vec::new();
    let mut keys: Vec<(String, String, Json)> = Vec::new();
    for call in helper_calls(rom, start, end) {
        let helper = string(&call, "helper").expect("a helper").to_owned();
        for data_assignment in &data_assignments {
            let found = match helper.as_str() {
                "loc_256AE" => fixed(rom, object_id, data_assignment, &helper),
                "loc_256F4" => variable(rom, object_id, data_assignment, &helper),
                "loc_25270" => table_assignments
                    .iter()
                    .find_map(|table| selector(rom, object_id, data_assignment, table, &helper)),
                _ => None,
            };
            let Some(mut found) = found else { continue };
            found.insert(
                "helper_call_offset",
                call.get("rom_offset").cloned().unwrap_or(Json::Null),
            );
            let key = (
                helper.clone(),
                string(&found, "mapping_offset")
                    .unwrap_or_default()
                    .to_owned(),
                found.get("mapping_pointers").cloned().unwrap_or(Json::Null),
            );
            if !keys.contains(&key) {
                keys.push(key);
                candidates.push(found);
            }
        }
    }
    candidates
}

/// `_plc_id_before`: the `moveq` or `move.w #imm,d0` that names the PLC.
fn plc_id_before(rom: &[u8], start: usize, call: usize) -> Option<u16> {
    let floor = (start as i64 - 1).max(call as i64 - 40);
    let mut at = call as i64 - 2;
    while at > floor {
        let opcode = word(rom, at as usize)?;
        if (0x7000..=0x70FF).contains(&opcode) {
            return Some(opcode & 0xFF);
        }
        if opcode == 0x303C {
            return word(rom, at as usize + 2);
        }
        at -= 2;
    }
    None
}

/// Python's `{value:#x}`, sign first.
fn signed_hex(value: i64) -> String {
    if value < 0 {
        format!("-{:#x}", -value)
    } else {
        format!("{value:#x}")
    }
}

/// `_plc_base`: the VRAM pattern a load lands at, from `$10(a4)` plus the
/// `addi.w` adjustments before the call.
fn plc_base(rom: &[u8], start: usize, call: usize) -> (Option<i64>, String) {
    let mut base = None;
    let mut adjustment = 0i64;
    let mut source = "unresolved".to_owned();
    for at in (start.max(call.saturating_sub(48))..call).step_by(2) {
        let Some(opcode) = word(rom, at) else { break };
        if opcode == 0x322C && word(rom, at + 2) == Some(0x10) {
            base = Some(0);
            source = "$10(a4)".to_owned();
        } else if opcode == 0x0641 {
            adjustment += s16(word(rom, at + 2).unwrap_or(0));
        }
    }
    if base.is_none() {
        return (None, source);
    }
    if adjustment % 0x20 != 0 {
        return (
            None,
            format!("{source}+{} (not pattern aligned)", signed_hex(adjustment)),
        );
    }
    (
        Some(adjustment.div_euclid(0x20)),
        format!("{source}+{}", signed_hex(adjustment)),
    )
}

/// `plc_loads`: the `LoadPLC1` calls and the pattern base each proves.
pub fn plc_loads(rom: &[u8], start: usize, end: usize) -> Vec<Json> {
    helper_calls(rom, start, end)
        .into_iter()
        .filter(|call| string(call, "helper") == Some("LoadPLC1"))
        .map(|call| {
            let at = usize::from_str_radix(
                string(&call, "rom_offset")
                    .expect("an offset")
                    .trim_start_matches("0x"),
                16,
            )
            .expect("hex");
            let (base, source) = plc_base(rom, start, at);
            Json::object()
                .with("call_rom_offset", hex6(at))
                .with("loader", "LoadPLC1")
                .with("loader_rom_offset", hex6(LOAD_PLC1 as usize))
                .with("plc_id", plc_id_before(rom, start, at))
                .with("base_pattern", base)
                .with("base_source", source)
        })
        .collect()
}

/// `plc_record`: one retail PLC record and its streams' headers.
fn plc_record(rom: &[u8], plc_id: usize) -> Option<Json> {
    let entry = PLC_OFFSET + plc_id * 2;
    if entry + 2 > rom.len() {
        return None;
    }
    let record = usize::try_from(PLC_OFFSET as i64 + s16(word(rom, entry)?)).ok()?;
    if record + 2 > rom.len() {
        return None;
    }
    let count = usize::from(word(rom, record)?) + 1;
    if !(1..=128).contains(&count) || record + 2 + count * 4 > rom.len() {
        return None;
    }
    let mut streams = Vec::new();
    for index in 0..count {
        let stream = long(rom, record + 2 + index * 4)? as usize;
        if stream + 2 > rom.len() {
            return None;
        }
        let raw = word(rom, stream)?;
        streams.push(
            Json::object()
                .with("index", index)
                .with("stream_rom_offset", hex6(stream))
                .with("tile_count", raw & 0x7FFF)
                .with("stream_header", format!("0x{raw:04X}")),
        );
    }
    Some(
        Json::object()
            .with("plc_id", plc_id)
            .with("table_rom_offset", hex6(entry))
            .with("record_rom_offset", hex6(record))
            .with("stream_count", count)
            .with("streams", streams),
    )
}

fn merged(base: &Json, over: &Json) -> Json {
    let mut out = base.clone();
    if let Json::Object(pairs) = over {
        for (key, value) in pairs {
            out.insert(key.as_str(), value.clone());
        }
    }
    out
}

/// What `plc_art` decodes: the pattern ranges the loads fill, the evidence
/// for each load, and the patterns themselves, by VRAM pattern.
pub struct PlcArt {
    pub ranges: Vec<Json>,
    pub evidence: Vec<Json>,
    pub patterns: std::collections::BTreeMap<i128, [u8; 64]>,
}

impl PlcArt {
    /// Python's `tiles` list: every pattern up to the highest, blank where
    /// no load put one.
    pub fn tiles(&self) -> Vec<[u8; 64]> {
        let Some((&last, _)) = self.patterns.last_key_value() else {
            return Vec::new();
        };
        let mut tiles = vec![[0u8; 64]; usize::try_from(last + 1).unwrap_or(0)];
        for (&pattern, tile) in &self.patterns {
            if let Ok(index) = usize::try_from(pattern) {
                tiles[index] = *tile;
            }
        }
        tiles
    }
}

/// `plc_art`: decode the loads' PLC streams into their patterns.
pub fn plc_art(rom: &[u8], loads: &[Json]) -> PlcArt {
    let mut patterns = std::collections::BTreeMap::new();
    let mut ranges = Vec::new();
    let mut evidence = Vec::new();
    let mut seen = BTreeSet::new();
    for load in loads {
        let (Some(plc_id), Some(base)) = (int(load, "plc_id"), int(load, "base_pattern")) else {
            evidence.push(merged(
                load,
                &Json::object().with("status", "partial").with(
                    "reason",
                    "PLC id or VRAM base is not statically recoverable",
                ),
            ));
            continue;
        };
        let call = string(load, "call_rom_offset")
            .unwrap_or_default()
            .to_owned();
        if !seen.insert((plc_id, base, call.clone())) {
            continue;
        }
        let Some(record) = usize::try_from(plc_id)
            .ok()
            .and_then(|id| plc_record(rom, id))
        else {
            evidence.push(merged(
                load,
                &Json::object()
                    .with("status", "deferred")
                    .with("reason", "PLC table record is not a valid retail record"),
            ));
            continue;
        };
        let mut load_evidence = merged(
            &merged(load, &record),
            &Json::object().with("status", "exact"),
        );
        let mut load_ranges = Vec::new();
        let mut pattern = base;
        let streams = match record.get("streams") {
            Some(Json::Array(streams)) => streams.clone(),
            _ => Vec::new(),
        };
        for stream in &streams {
            let offset = string(stream, "stream_rom_offset").unwrap_or_default();
            let at = usize::from_str_radix(offset.trim_start_matches("0x"), 16).expect("hex");
            let (data, consumed) = match nemesis::decompress(rom, at) {
                Ok(decoded) => decoded,
                Err(error) => {
                    load_evidence.insert("status", "partial");
                    load_evidence.insert("reason", error.0);
                    continue;
                }
            };
            let tiles = data.len().div_ceil(TILE_SIZE) as i128;
            let expected = int(stream, "tile_count").unwrap_or(-1);
            if tiles != expected {
                load_evidence.insert("status", "partial");
                load_evidence.insert(
                    "reason",
                    format!("Nemesis header says {expected} tiles, decoder produced {tiles}"),
                );
                continue;
            }
            let first = pattern;
            for chunk in data.chunks(TILE_SIZE) {
                let mut tile = [0u8; 64];
                for (i, byte) in chunk.iter().enumerate() {
                    tile[i * 2] = byte >> 4;
                    tile[i * 2 + 1] = byte & 0x0F;
                }
                patterns.insert(pattern, tile);
                pattern += 1;
            }
            load_ranges.push(
                Json::object()
                    .with("kind", "attack_plc")
                    .with("plc_id", Json::Int(plc_id))
                    .with("call_rom_offset", call.as_str())
                    .with(
                        "record_rom_offset",
                        record
                            .get("record_rom_offset")
                            .cloned()
                            .unwrap_or(Json::Null),
                    )
                    .with("stream_rom_offset", offset)
                    .with("first_pattern", Json::Int(first))
                    .with("pattern_count", Json::Int(tiles))
                    .with("last_pattern_exclusive", Json::Int(pattern))
                    .with("compressed_bytes_read", consumed),
            );
        }
        load_evidence.insert("ranges", load_ranges.clone());
        ranges.extend(load_ranges);
        evidence.push(load_evidence);
    }
    PlcArt {
        ranges,
        evidence,
        patterns,
    }
}

/// `object_evidence`: helpers, sequences, PLC loads and assignments for one
/// object, and what kind of presentation they add up to.
pub fn object_evidence(rom: &[u8], start: usize, end: usize, object_id: &Json) -> Json {
    let assignments: Vec<Json> = pointer_assignments(rom, start, end)
        .into_iter()
        .map(|(assignment, _)| assignment.with("object_id", object_id.clone()))
        .collect();
    let calls = helper_calls(rom, start, end);
    let sequences = frame_sequences(rom, start, end, object_id);
    let loads = plc_loads(rom, start, end);
    let names: BTreeSet<String> = calls
        .iter()
        .filter_map(|call| string(call, "helper").map(str::to_owned))
        .collect();
    let (classification, reason) = if !sequences.is_empty() {
        (
            "mapping_sequence",
            "a retail frame helper consumes a proven timed mapping record",
        )
    } else if names.contains("loc_25978") {
        (
            "projectile_effect_graph",
            "the object reads a tile/DMA stream but does not call a mapping timer",
        )
    } else if names.contains("loc_258AC") {
        (
            "palette_or_plane_effect",
            "the object consumes palette/effect state rather than VDP mapping frames",
        )
    } else if !loads.is_empty() {
        (
            "tile_upload_only",
            "the object constructs or uploads attack art without a mapping loop",
        )
    } else {
        (
            "state_machine_without_sprite_mapping",
            "no direct mapping timer or proven sprite-art consumer is present",
        )
    };
    Json::object()
        .with("object_id", object_id.clone())
        .with("rom_span", vec![hex6(start), hex6(end)])
        .with("helpers", calls)
        .with("helper_names", names.into_iter().collect::<Vec<_>>())
        .with("assignments", assignments)
        .with("frame_sequences", sequences)
        .with("plc_loads", loads)
        .with("classification", classification)
        .with("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(rom: &mut [u8], at: usize, words: &[u16]) {
        for (i, w) in words.iter().enumerate() {
            rom[at + i * 2..at + i * 2 + 2].copy_from_slice(&w.to_be_bytes());
        }
    }

    #[test]
    fn a_fixed_sequence_reads_its_duration_count_and_pointers() {
        let mut rom = vec![0u8; 0x400];
        // move.l #$100,$08(a4); jsr loc_256AE.
        put(
            &mut rom,
            0,
            &[
                0x297C, 0x0000, 0x0100, 0x0008, 0x4EB9, 0x0002, 0x56AE, 0, 0, 0,
            ],
        );
        rom[0x100] = 3;
        rom[0x101] = 2;
        put(&mut rom, 0x102, &[0x0000, 0x0200, 0x0000, 0x0210]);
        let found = frame_sequences(&rom, 0, 0x20, &Json::Int(0x40));
        assert_eq!(found.len(), 1);
        let value = serde_json::Value::from(&found[0]);
        assert_eq!(value["timing_model"], "fixed");
        assert_eq!(value["total_frames"], 6);
        assert_eq!(
            value["mapping_pointers"],
            serde_json::json!(["0x000200", "0x000210"])
        );
        assert_eq!(value["helper_call_offset"], "0x000008");
    }

    #[test]
    fn an_odd_pointer_or_zero_duration_is_no_sequence() {
        let mut rom = vec![0u8; 0x400];
        put(
            &mut rom,
            0,
            &[
                0x297C, 0x0000, 0x0100, 0x0008, 0x4EB9, 0x0002, 0x56AE, 0, 0, 0,
            ],
        );
        rom[0x100] = 3;
        rom[0x101] = 1;
        put(&mut rom, 0x102, &[0x0000, 0x0201]);
        assert!(frame_sequences(&rom, 0, 0x20, &Json::Null).is_empty());
        put(&mut rom, 0x102, &[0x0000, 0x0200]);
        rom[0x100] = 0;
        assert!(frame_sequences(&rom, 0, 0x20, &Json::Null).is_empty());
    }

    #[test]
    fn a_plc_base_is_a10_plus_aligned_adjustments() {
        let mut rom = vec![0u8; 0x40];
        put(
            &mut rom,
            0,
            &[0x322C, 0x0010, 0x0641, 0x0040, 0x0641, 0xFFE0],
        );
        assert_eq!(plc_base(&rom, 0, 12), (Some(1), "$10(a4)+0x20".to_owned()));
        put(&mut rom, 4, &[0x0641, 0xFFC0]);
        assert_eq!(plc_base(&rom, 0, 12).0, Some(-3));
        assert_eq!(signed_hex(-0x60), "-0x60");
        put(&mut rom, 4, &[0x0641, 0x0001]);
        assert!(plc_base(&rom, 0, 12).1.ends_with("(not pattern aligned)"));
        assert_eq!(
            plc_base(&[0u8; 0x10], 0, 8),
            (None, "unresolved".to_owned())
        );
    }

    #[test]
    fn a_selector_pointer_with_its_sign_bit_set_hides_the_frame() {
        let mut rom = vec![0u8; 0x400];
        // move.l #$100,$08(a4); move.l #$180,$28(a4); jsr loc_25270.
        put(
            &mut rom,
            0,
            &[
                0x297C, 0x0000, 0x0100, 0x0008, 0x297C, 0x0000, 0x0180, 0x0028, 0x4EB9, 0x0002,
                0x5270,
            ],
        );
        rom[0x100..0x105].copy_from_slice(&[2, 4, 4, 0, 4]);
        put(&mut rom, 0x180, &[0x0000, 0x0200, 0x8000, 0x0000]);
        let found = frame_sequences(&rom, 0, 0x30, &Json::Null);
        assert_eq!(found.len(), 1);
        let value = serde_json::Value::from(&found[0]);
        assert_eq!(value["timing_model"], "selector");
        assert_eq!(
            value["mapping_pointers"],
            serde_json::json!(["0x000200", null])
        );
        assert_eq!(value["hidden_frames"], 1);
    }
}
