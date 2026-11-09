//! `battle/enemy_animations.json`: the port of
//! `psiv_tools/battle_animations.py`, whose module docs give the reasons. Each
//! enemy's `EnemyAttackOffs` routine is followed through the battle objects it
//! creates to the exact `Sound_Index` write, the timed mapping sequence a
//! retail frame helper consumes, the attack PLC art, and the coordinate
//! writes; the checked-in oracle receipts close the presentations the static
//! decode cannot.
//!
//! The Python works on loose records, so this port builds the same JSON
//! directly and then reads it back as `psiv_data::EnemyAnimationsFile`, the
//! type the game loads, before it is written.

pub(super) mod mapping;
mod movement;
pub(super) mod objects;
mod receipts;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::names::{Display, display, enemy_symbols};
use crate::json::Json;
use crate::sound_defs::SFX_NAMES;
use crate::{PackError, rom::EXPECTED_SHA256};

const ENEMY_ATTACK_OFFS: usize = 0x00D0A4;
const ENEMY_ATTACK_COUNT: usize = 153;
const ENEMY_ATTACK_END: usize = ENEMY_ATTACK_OFFS + ENEMY_ATTACK_COUNT * 2;
const ROM_SIZE: usize = 3_145_728;

/// `OBJECT_TABLES`: `Battle_RunObjects`' pointer groups, by id range (ids
/// step by four: each range is a longword table).
const OBJECT_TABLES: [(&str, usize, usize, usize); 7] = [
    ("battle_objects_group_2", 0x01252C, 0x040, 0x128),
    ("battle_objects_group_3", 0x015E2C, 0x12C, 0x23C),
    ("battle_objects_group_4", 0x01B6F6, 0x240, 0x31C),
    ("battle_objects_group_5", 0x020590, 0x320, 0x3FC),
    ("battle_objects_group_8", 0x026EC0, 0x700, 0x814),
    ("battle_objects_group_9", 0x02D88E, 0x818, 0x8BC),
    ("battle_objects_group_10", 0x033456, 0x8C0, 0x934),
];
const SOUND_INDEX_ADDRESS: [u8; 4] = [0x00, 0xFF, 0x50, 0x0A];

/// `ENEMY_ART_TABLE`'s record size, which the source names.
const ENEMY_ART_STRIDE: usize = 20;

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

pub(crate) fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

pub(crate) fn word(rom: &[u8], at: usize) -> Option<u16> {
    rom.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
}

pub(crate) fn long(rom: &[u8], at: usize) -> Option<u32> {
    rom.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

pub(crate) fn int(value: &Json, key: &str) -> Option<i128> {
    match value.get(key) {
        Some(Json::Int(n)) => Some(*n),
        _ => None,
    }
}

pub(crate) fn string<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    match value.get(key) {
        Some(Json::Str(s)) => Some(s),
        _ => None,
    }
}

fn list(value: &Json, key: &str) -> Vec<Json> {
    match value.get(key) {
        Some(Json::Array(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn truthy(value: &Json, key: &str) -> bool {
    matches!(value.get(key), Some(Json::Bool(true)))
}

fn checked_word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    word(rom, at).ok_or_else(|| PackError(format!("word at {} runs past the ROM", hex6(at))))
}

fn checked_long(rom: &[u8], at: usize) -> Result<u32, PackError> {
    long(rom, at).ok_or_else(|| PackError(format!("longword at {} runs past the ROM", hex6(at))))
}

fn sfx_name(sound: u8) -> Option<&'static str> {
    usize::from(sound)
        .checked_sub(0xB5)
        .and_then(|i| SFX_NAMES.get(i))
        .copied()
}

/// `_check_retail_rom`'s table guards (the size and hash are the ROM check's).
fn check_guards(rom: &[u8]) -> Result<(), PackError> {
    let before = [0x4E, 0xBA, 0xE6, 0x02, 0x4E, 0x75];
    let after = [0x00, 0x28, 0x3B, 0x40, 0x3A, 0x52];
    if rom.get(ENEMY_ATTACK_OFFS - before.len()..ENEMY_ATTACK_OFFS) != Some(&before[..]) {
        return refuse("EnemyAttackOffs retail pre-table guard changed".into());
    }
    if rom.get(ENEMY_ATTACK_END..ENEMY_ATTACK_END + after.len()) != Some(&after[..]) {
        return refuse("EnemyAttackOffs retail post-table guard changed".into());
    }
    Ok(())
}

/// `_attack_routines`.
fn attack_routines(rom: &[u8]) -> Result<Vec<usize>, PackError> {
    let routines = (0..ENEMY_ATTACK_COUNT)
        .map(|i| Ok(ENEMY_ATTACK_OFFS + usize::from(checked_word(rom, ENEMY_ATTACK_OFFS + i * 2)?)))
        .collect::<Result<Vec<_>, PackError>>()?;
    let distinct: BTreeSet<usize> = routines.iter().copied().collect();
    if distinct.len() != 74 {
        return refuse(format!(
            "EnemyAttackOffs should resolve to 74 routines, got {}",
            distinct.len()
        ));
    }
    if distinct.first() != Some(&0x00D218) || distinct.last() != Some(&0x010ECC) {
        return refuse("EnemyAttackOffs routine range is not retail".into());
    }
    Ok(routines)
}

/// `_object_pointers`: object id to routine, and the tables' records.
fn object_pointers(rom: &[u8]) -> Result<(BTreeMap<usize, usize>, Vec<Json>), PackError> {
    let mut pointers = BTreeMap::new();
    let mut tables = Vec::new();
    for (name, offset, first, last) in OBJECT_TABLES {
        let count = (last - first) / 4 + 1;
        for index in 0..count {
            let id = first + index * 4;
            let pointer = checked_long(rom, offset + index * 4)? as usize;
            if pointer & 1 != 0 || pointer >= rom.len() {
                return refuse(format!(
                    "{name} entry {} points outside retail ROM",
                    hex6(id)
                ));
            }
            if pointers.insert(id, pointer).is_some() {
                return refuse(format!("duplicate battle object id {}", hex6(id)));
            }
        }
        tables.push(
            Json::object()
                .with("name", name)
                .with("rom_offset", hex6(offset))
                .with("entry_count", count)
                .with("entry_bytes", 4u8)
                .with("grand_cross", 0u8),
        );
    }
    if pointers.len() != 382 {
        return refuse(format!(
            "retail object pointer census changed: {}",
            pointers.len()
        ));
    }
    Ok((pointers, tables))
}

/// `_spans`: each routine runs to the next one, the last for `$400` bytes.
fn spans(
    targets: impl Iterator<Item = usize>,
    last_end: impl Fn(usize) -> usize,
) -> HashMap<usize, (usize, usize)> {
    let targets: Vec<usize> = targets.collect::<BTreeSet<_>>().into_iter().collect();
    targets
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = targets
                .get(i + 1)
                .copied()
                .unwrap_or_else(|| last_end(start));
            (start, (start, end))
        })
        .collect()
}

/// `_object_refs`: `move.w #id,(a1)` (and one legacy raw spelling) naming a
/// known object.
fn object_refs(
    rom: &[u8],
    start: usize,
    end: usize,
    pointers: &BTreeMap<usize, usize>,
) -> Vec<usize> {
    let mut refs = Vec::new();
    for at in (start..start.max(end.saturating_sub(5))).step_by(2) {
        let id = match word(rom, at) {
            Some(0x32BC) => word(rom, at + 2),
            Some(0x337C) if word(rom, at + 4) == Some(0) => word(rom, at + 2),
            _ => None,
        };
        if let Some(id) = id.map(usize::from)
            && pointers.contains_key(&id)
        {
            refs.push(id);
        }
    }
    refs
}

/// `_sound_writes`: `move.b #id,(Sound_Index).l` and the object-local form.
fn sound_writes(rom: &[u8], start: usize, end: usize, object: &Json) -> Vec<Json> {
    let mut writes = Vec::new();
    for at in (start..start.max(end.saturating_sub(7))).step_by(2) {
        let dispatch = match (word(rom, at), rom.get(at + 4..at + 8)) {
            (Some(0x13FC), Some(target)) if target == SOUND_INDEX_ADDRESS => "Sound_Index",
            (Some(0x137C), _) if word(rom, at + 4) == Some(0x0010) => "object_local_sound",
            _ => continue,
        };
        let sound = rom[at + 3];
        writes.push(
            Json::object()
                .with("object_id", object.clone())
                .with("rom_offset", hex6(at))
                .with("dispatch", dispatch)
                .with("sound_id", sound)
                .with("sound_name", sfx_name(sound)),
        );
    }
    writes
}

/// What `_walk_animation` gathers from the objects a routine reaches.
#[derive(Default)]
struct Walk {
    objects: Vec<usize>,
    writes: Vec<Json>,
    mappings: Vec<Json>,
    records: Vec<Json>,
    loads: Vec<Json>,
}

fn walk(
    rom: &[u8],
    roots: &[usize],
    pointers: &BTreeMap<usize, usize>,
    spans: &HashMap<usize, (usize, usize)>,
) -> Walk {
    fn visit(
        rom: &[u8],
        id: usize,
        pointers: &BTreeMap<usize, usize>,
        spans: &HashMap<usize, (usize, usize)>,
        out: &mut Walk,
    ) {
        if out.objects.contains(&id) {
            return;
        }
        out.objects.push(id);
        let (start, end) = spans[&pointers[&id]];
        let object = Json::from(id);
        out.writes.extend(sound_writes(rom, start, end, &object));
        let evidence = objects::object_evidence(rom, start, end, &object);
        out.mappings.extend(list(&evidence, "frame_sequences"));
        out.loads.extend(list(&evidence, "plc_loads"));
        out.records.push(evidence);
        for child in object_refs(rom, start, end, pointers) {
            visit(rom, child, pointers, spans, out);
        }
    }
    let mut out = Walk::default();
    for &root in roots {
        visit(rom, root, pointers, spans, &mut out);
    }
    out
}

/// The attack sprite bank length, matching `build_attack_tile_bank`.
/// Art #1 is uploaded at fighter+$10 (`loc_7BFA`), then copied to an
/// attack object by `loc_12618`. The other blobs belong to the body bank.
fn art_banks(rom: &[u8]) -> Result<Vec<i128>, PackError> {
    let records = crate::battle::art::enemies::records(rom)?;
    let bounds = crate::battle::art::enemies::bounds(&records, rom.len());
    records
        .iter()
        .map(|record| {
            let offset = record.art[0];
            let label = format!("ArtNem_{}Battle1", record.symbol);
            let header = crate::decode::nemesis::read_header(rom, offset)?;
            let (data, consumed) = crate::decode::nemesis::decompress(rom, offset)?;
            if data.len() != usize::from(header.row_count) * 4 {
                return refuse(format!(
                    "{label} at 0x{offset:06X}: decoded {} bytes but the header declares {} \
                     patterns",
                    data.len(),
                    header.tile_count
                ));
            }
            if consumed > bounds[&offset] + 1 {
                return refuse(format!(
                    "{label} at 0x{offset:06X}: consumed {consumed} bytes and ran into the \
                     next blob, which starts {} bytes in",
                    bounds[&offset]
                ));
            }
            Ok(data.len().div_ceil(32) as i128)
        })
        .collect()
}

/// One frame-sequence candidate and what ranks it.
struct Choice {
    sequence: Json,
    composition: Json,
    rank: (i128, usize, i64, i64),
}

fn choose(rom: &[u8], mappings: &[Json], bank: i128, plc: &[Json]) -> Option<Choice> {
    let mut best: Option<Choice> = None;
    for mapping in mappings {
        let (sequence, composition) = mapping::sequence_evidence(rom, mapping, Some(bank), plc);
        let records = list(&sequence, "mapping_records");
        let valid = records
            .iter()
            .filter(|r| truthy(r, "all_entries_valid"))
            .count();
        let external = records
            .iter()
            .flat_map(mapping::entries_of)
            .flat_map(|e| list(&e, "tile_sources"))
            .filter(|s| string(s, "kind") == Some("attack_plc"))
            .count();
        let status = match string(&composition, "status") {
            Some("exact") => 3,
            Some("partial") => 2,
            _ => 1,
        };
        let hidden = int(&sequence, "hidden_frames").unwrap_or(0);
        let rank = (status, valid, -(external as i64), -(hidden as i64));
        if best.as_ref().is_none_or(|b| rank > b.rank) {
            best = Some(Choice {
                sequence,
                composition,
                rank,
            });
        }
    }
    best
}

/// Why no sequence was chosen, from the helpers and assignments reached.
fn why_not(records: &[Json], loads: &[Json]) -> String {
    let helpers: BTreeSet<String> = records
        .iter()
        .flat_map(|r| list(r, "helper_names"))
        .filter_map(|n| match n {
            Json::Str(s) => Some(s),
            _ => None,
        })
        .collect();
    let assignments: usize = records.iter().map(|r| list(r, "assignments").len()).sum();
    if helpers.contains("loc_25978") {
        "routine constructs projectile/tile-stream state through loc_25978; no mapping timer \
         loop consumes it"
            .into()
    } else if helpers.contains("loc_258AC") {
        "routine consumes palette/effect state through loc_258AC; no enemy VDP mapping \
         sequence is selected"
            .into()
    } else if !loads.is_empty() {
        "routine uploads attack PLC art but constructs no timed enemy mapping record".into()
    } else if !helpers.is_empty() {
        format!(
            "routine calls {} but no structurally valid fixed, variable, or selector record \
             follows its {assignments} immediate $08/$28 pointer assignment(s)",
            helpers.into_iter().collect::<Vec<_>>().join(", ")
        )
    } else if assignments != 0 {
        format!(
            "routine has {assignments} immediate $08/$28 pointer assignment(s) but no retail \
             frame-helper call to consume a timed mapping record"
        )
    } else {
        "no fixed, variable, or selector mapping record passed structural decode".into()
    }
}

fn classification(
    sequence: Option<&Json>,
    records: &[Json],
    loads: &[Json],
) -> (&'static str, &'static str) {
    let any = |kind: &str| {
        records
            .iter()
            .any(|r| string(r, "classification") == Some(kind))
    };
    if let Some(sequence) = sequence {
        let kind = match string(sequence, "timing_model") {
            Some("fixed") => "fixed_mapping",
            Some("variable") => "variable_mapping",
            Some("selector") => "selector_mapping",
            _ => "mapping_sequence",
        };
        (
            kind,
            "selected sequence is consumed by a retail mapping timer helper",
        )
    } else if any("projectile_effect_graph") {
        (
            "projectile_effect_graph",
            "reachable objects construct a separate projectile/effect stream instead of an \
             enemy frame loop",
        )
    } else if any("palette_or_plane_effect") {
        (
            "palette_or_plane_effect",
            "reachable objects only consume palette or plane-effect state",
        )
    } else if !loads.is_empty() {
        (
            "tile_upload_only",
            "the routine has proven LoadPLC1 art but no timed mapping consumer",
        )
    } else {
        (
            "state_machine_without_sprite_mapping",
            "the object graph has no proven timed enemy mapping consumer",
        )
    }
}

/// One enemy's record, before its receipt.
fn animation(
    rom: &[u8],
    id: usize,
    symbol: &str,
    routine: usize,
    context: &Context,
) -> Result<Json, PackError> {
    let (start, end) = context.attack_spans[&routine];
    let mut roots = Vec::new();
    for object in object_refs(rom, start, end, &context.pointers) {
        if !roots.contains(&object) {
            roots.push(object);
        }
    }
    let reached = walk(rom, &roots, &context.pointers, &context.object_spans);
    let mut writes = sound_writes(rom, start, end, &Json::Null);
    writes.extend(reached.writes);
    let direct = objects::object_evidence(rom, start, end, &Json::Null);
    let mut records = vec![direct.clone()];
    records.extend(reached.records);
    let mut mappings = list(&direct, "frame_sequences");
    mappings.extend(reached.mappings);
    let mut loads = list(&direct, "plc_loads");
    loads.extend(reached.loads);
    let mut unique = Vec::new();
    let mut seen = Vec::new();
    for load in loads {
        let key = (
            load.get("call_rom_offset").cloned(),
            load.get("plc_id").cloned(),
            load.get("base_pattern").cloned(),
        );
        if !seen.contains(&key) {
            seen.push(key);
            unique.push(load);
        }
    }
    let objects::PlcArt {
        ranges: plc_ranges,
        evidence: plc_evidence,
        ..
    } = objects::plc_art(rom, &unique);
    let Some(exact) = writes
        .iter()
        .find(|w| string(w, "dispatch") == Some("Sound_Index"))
        .cloned()
    else {
        return refuse(format!(
            "enemy {id} ({symbol}) has no direct Sound_Index write"
        ));
    };

    let chosen = choose(rom, &mappings, context.banks[id], &plc_ranges);
    let (mut sequence, composition) = match chosen {
        Some(choice) => (Some(choice.sequence), choice.composition),
        None => (
            None,
            Json::object()
                .with("status", "deferred")
                .with("reason", why_not(&records, &unique)),
        ),
    };
    if let Some(sequence) = &mut sequence
        && int(sequence, "object_id") == Some(0xFFFF)
    {
        sequence.insert("object_id", Json::Null);
    }
    let by_object: Vec<(i128, Vec<Json>)> = reached
        .objects
        .iter()
        .map(|&object| {
            let (s, e) = context.object_spans[&context.pointers[&object]];
            (
                object as i128,
                movement::coordinate_sites(rom, s, e, object as i128),
            )
        })
        .collect();
    let movement = movement::evidence(&by_object, sequence.as_ref());
    let (kind, kind_reason) = classification(sequence.as_ref(), &records, &unique);
    let why = match &sequence {
        Some(_) => Json::Null,
        None => composition.get("reason").cloned().unwrap_or(Json::Null),
    };
    let movement_exact = string(&movement, "status") == Some("exact");
    let composition_exact = string(&composition, "status") == Some("exact");
    Ok(Json::object()
        .with("enemy_id", id)
        .with("symbol", symbol)
        .with("display_name", Json::Null)
        .with("routine_offset", hex6(routine))
        .with("root_object_ids", roots)
        .with("object_ids", reached.objects)
        .with(
            "sfx_id",
            exact.get("sound_id").cloned().unwrap_or(Json::Null),
        )
        .with(
            "sfx_name",
            exact.get("sound_name").cloned().unwrap_or(Json::Null),
        )
        .with(
            "dispatch",
            Json::object()
                .with("kind", exact.get("dispatch").cloned().unwrap_or(Json::Null))
                .with(
                    "rom_offset",
                    exact.get("rom_offset").cloned().unwrap_or(Json::Null),
                )
                .with(
                    "object_id",
                    exact.get("object_id").cloned().unwrap_or(Json::Null),
                ),
        )
        .with("sfx_writes", writes)
        .with("flash_timing_proven", sequence.is_some())
        .with("frame_sequence", sequence)
        .with("frame_sequence_why_not", why)
        .with("routine_classification", kind)
        .with("routine_classification_reason", kind_reason)
        .with("object_records", records)
        .with(
            "attack_plc",
            Json::object()
                .with("loads", unique)
                .with("ranges", plc_ranges)
                .with("records", plc_evidence),
        )
        .with("movement", movement)
        .with("composition", composition)
        .with("movement_proven", movement_exact)
        .with("sprite_sheet_proven", composition_exact))
}

/// What every enemy's walk reads.
struct Context {
    pointers: BTreeMap<usize, usize>,
    object_spans: HashMap<usize, (usize, usize)>,
    attack_spans: HashMap<usize, (usize, usize)>,
    banks: Vec<i128>,
}

fn counts<'a>(values: impl Iterator<Item = &'a str>) -> Json {
    let mut counted: BTreeMap<&str, usize> = BTreeMap::new();
    for value in values {
        *counted.entry(value).or_default() += 1;
    }
    let mut out = Json::object();
    for (key, count) in counted {
        out.insert(key, count);
    }
    out
}

fn census(animations: &[Json], routines: &[usize]) -> Json {
    let has_sequence = |a: &&Json| !matches!(a.get("frame_sequence"), Some(Json::Null) | None);
    let timed = animations.iter().filter(has_sequence).count();
    let status = |key: &str, value: &str| {
        animations
            .iter()
            .filter(|a| a.get(key).and_then(|m| string(m, "status")) == Some(value))
            .count()
    };
    let (m_exact, m_partial, m_deferred) = (
        status("movement", "exact"),
        status("movement", "partial"),
        status("movement", "deferred"),
    );
    let (c_exact, c_partial, c_deferred) = (
        status("composition", "exact"),
        status("composition", "partial"),
        status("composition", "deferred"),
    );
    let deferred: Vec<&Json> = animations.iter().filter(|a| !has_sequence(a)).collect();
    let deferred_bodies: BTreeSet<&str> = deferred
        .iter()
        .filter_map(|a| string(a, "routine_offset"))
        .collect();
    let records: Vec<Json> = animations
        .iter()
        .flat_map(|a| list(a, "object_records"))
        .collect();
    let plc = |key: &str| -> Vec<Json> {
        animations
            .iter()
            .flat_map(|a| {
                a.get("attack_plc")
                    .map(|p| list(p, key))
                    .unwrap_or_default()
            })
            .collect()
    };
    let distinct_records: BTreeSet<(Option<i128>, Option<String>)> = plc("records")
        .iter()
        .filter(|r| string(r, "status") == Some("exact"))
        .map(|r| {
            (
                int(r, "plc_id"),
                string(r, "record_rom_offset").map(str::to_owned),
            )
        })
        .collect();
    Json::object()
        .with("enemy_count", animations.len())
        .with(
            "distinct_attack_routines",
            routines.iter().collect::<BTreeSet<_>>().len(),
        )
        .with("exact_sfx", animations.len())
        .with("generic_sfx", 0u8)
        .with("frame_sequence_records", timed)
        .with("timed_frame_sequences", timed)
        .with("frame_sequence_deferred", animations.len() - timed)
        .with("movement_deferred", m_partial + m_deferred)
        .with("movement_exact", m_exact)
        .with("movement_partial", m_partial)
        .with("movement_status_deferred", m_deferred)
        .with("flash_timing", timed)
        .with("sprite_sheet_deferred", c_partial + c_deferred)
        .with("sprite_sheet_exact", c_exact)
        .with("sprite_sheet_partial", c_partial)
        .with("sprite_sheet_status_deferred", c_deferred)
        .with(
            "routine_classifications",
            counts(
                animations
                    .iter()
                    .filter_map(|a| string(a, "routine_classification")),
            ),
        )
        .with(
            "deferred_routine_classifications",
            counts(
                deferred
                    .iter()
                    .filter_map(|a| string(a, "routine_classification")),
            ),
        )
        .with("deferred_routine_bodies", deferred_bodies.len())
        .with(
            "object_classifications",
            counts(records.iter().filter_map(|r| string(r, "classification"))),
        )
        .with("attack_plc_loads", plc("loads").len())
        .with("attack_plc_distinct_records", distinct_records.len())
}

fn source(tables: Vec<Json>) -> Json {
    let mut all = vec![
        Json::object()
            .with("name", "EnemyAttackOffs")
            .with("rom_offset", hex6(ENEMY_ATTACK_OFFS))
            .with("entry_count", ENEMY_ATTACK_COUNT)
            .with("entry_bytes", 2u8)
            .with("grand_cross", 0u8),
    ];
    all.extend(tables);
    let art_fields = Json::object()
        .with("art_1", "+0x02")
        .with("art_2", "+0x06")
        .with("art_3", "+0x0A")
        .with("body_mapping", "+0x0E")
        .with("half_width", "+0x12")
        .with("height", "+0x13");
    Json::object()
        .with("label", "EnemyAttackOffs")
        .with("rom_offset", hex6(ENEMY_ATTACK_OFFS))
        .with("table_end", hex6(ENEMY_ATTACK_END))
        .with("record_count", ENEMY_ATTACK_COUNT)
        .with("record_bytes", 2u8)
        .with("rom_size", ROM_SIZE)
        .with("rom_sha256", EXPECTED_SHA256)
        .with("grand_cross", 0u8)
        .with("reference_clone", "reference/ps4disasm")
        .with("reference_grand_cross", 1u8)
        .with(
            "reference_role",
            "labels and semantic guide only; not byte authority",
        )
        .with(
            "retail_guards",
            Json::object()
                .with("before_table", "4EBAE6024E75")
                .with("after_table", "00283B403A52"),
        )
        .with(
            "enemy_art_table",
            Json::object()
                .with("label", "loc_27F3AE")
                .with("rom_offset", "0x27F3AE")
                .with("entry_count", ENEMY_ATTACK_COUNT)
                .with("entry_bytes", ENEMY_ART_STRIDE)
                .with("fields", art_fields)
                .with("bank_order", vec![3u8, 1, 2])
                .with("grand_cross", 0u8),
        )
        .with(
            "mapping_record",
            Json::object()
                .with("consumer", "Battle_FillSpriteAttributes")
                .with("grand_cross", 0u8)
                .with("header_bytes", mapping::HEADER_BYTES)
                .with("entry_bytes", mapping::ENTRY_BYTES)
                .with(
                    "entry_fields",
                    vec![
                        "y_offset",
                        "size",
                        "tile_word",
                        "x_offset",
                        "x_offset_mirrored",
                    ],
                )
                .with("tile_index_mask", "0x07FF")
                .with("h_flip_bit", "0x0800")
                .with("v_flip_bit", "0x1000")
                .with("palette_mask", "0x6000"),
        )
        .with("movement_fields", movement::fields())
        .with("tables", all)
}

/// The whole file, named by `names`.
pub fn build(rom: &[u8], names: &Display) -> Result<Json, PackError> {
    if rom.len() != ROM_SIZE {
        return refuse(format!(
            "enemy animation extraction needs the {ROM_SIZE}-byte retail ROM; got {}",
            rom.len()
        ));
    }
    check_guards(rom)?;
    let receipts = receipts::Receipts::load()?;
    let symbols = enemy_symbols();
    let routines = attack_routines(rom)?;
    let (pointers, tables) = object_pointers(rom)?;
    let object_spans = spans(pointers.values().copied(), |start| {
        rom.len().min(start + 0x400)
    });
    let mut attack_spans = spans(routines.iter().copied(), |_| 0x010F00);
    // ForcedFly branches into MonsterFly's shared body after its first test;
    // its entry starts six bytes earlier than the next unique pointer.
    attack_spans.insert(0x010D86, (0x010D86, 0x010DB8));
    let context = Context {
        pointers,
        object_spans,
        attack_spans,
        banks: art_banks(rom)?,
    };
    let mut animations = Vec::new();
    for (id, &routine) in routines.iter().enumerate() {
        let mut record = animation(rom, id, symbols[id], routine, &context)?;
        receipts.apply(&mut record)?;
        record.insert("display_name", display(names, id as u16));
        animations.push(record);
    }
    let census = census(&animations, &routines);
    let out = Json::object()
        .with("format_version", super::FORMAT_VERSION)
        .with("kind", "battle_enemy_animations")
        .with("count", animations.len())
        .with("source", source(tables))
        .with("animations", animations)
        .with("census", census);
    serde_json::from_value::<psiv_data::EnemyAnimationsFile>(serde_json::Value::from(&out))
        .map_err(|error| PackError(format!("enemy_animations.json does not read back: {error}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_patterns_start_at_art_one_instead_of_the_concatenated_body_bank() {
        let mut rom = vec![0u8; 0x27F3AE + ENEMY_ATTACK_COUNT * ENEMY_ART_STRIDE];
        // Synthetic Nemesis streams: one-bit code zero repeats palette index
        // five across each row. Different lengths distinguish all three banks.
        for (offset, tiles) in [(0x40, 2u8), (0x60, 3), (0x80, 4)] {
            let mut stream = vec![0, tiles, 0x85, 0x71, 0, 0xFF];
            stream.resize(usize::from(tiles) + 8, 0);
            rom[offset..offset + stream.len()].copy_from_slice(&stream);
        }
        for id in 0..ENEMY_ATTACK_COUNT {
            let at = 0x27F3AE + id * ENEMY_ART_STRIDE;
            for (field, offset) in [(2, 0x40u32), (6, 0x60), (10, 0x80), (14, 0xA0)] {
                rom[at + field..at + field + 4].copy_from_slice(&offset.to_be_bytes());
            }
        }
        assert_eq!(
            art_banks(&rom).unwrap(),
            vec![2; ENEMY_ATTACK_COUNT],
            "loc_7BFA uploads only Art #1 at the fighter base copied by loc_12618"
        );
    }

    #[test]
    fn a_sound_write_is_absolute_or_object_local() {
        let mut rom = vec![0u8; 0x20];
        rom[0..8].copy_from_slice(&[0x13, 0xFC, 0x00, 0xB5, 0x00, 0xFF, 0x50, 0x0A]);
        rom[8..14].copy_from_slice(&[0x13, 0x7C, 0x00, 0x01, 0x00, 0x10]);
        let writes = sound_writes(&rom, 0, 0x18, &Json::Null);
        let writes: Vec<serde_json::Value> = writes.iter().map(serde_json::Value::from).collect();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0]["dispatch"], "Sound_Index");
        assert_eq!(writes[0]["sound_name"], SFX_NAMES[0]);
        assert_eq!(writes[1]["dispatch"], "object_local_sound");
        assert!(writes[1]["sound_name"].is_null());
    }

    #[test]
    fn object_refs_name_only_known_objects() {
        let mut rom = vec![0u8; 0x20];
        rom[0..4].copy_from_slice(&[0x32, 0xBC, 0x00, 0x40]);
        rom[4..8].copy_from_slice(&[0x32, 0xBC, 0x00, 0x44]);
        rom[8..14].copy_from_slice(&[0x33, 0x7C, 0x00, 0x48, 0x00, 0x00]);
        let pointers: BTreeMap<usize, usize> = [(0x40, 0), (0x48, 0)].into();
        assert_eq!(object_refs(&rom, 0, 0x20, &pointers), [0x40, 0x48]);
    }

    #[test]
    fn the_best_sequence_is_the_first_of_the_highest_rank() {
        let rom = vec![0u8; 0x10];
        let sequence = |offset: &str| {
            Json::object()
                .with("mapping_offset", offset)
                .with("mapping_pointers", vec![Json::Null])
        };
        let chosen = choose(&rom, &[sequence("a"), sequence("b")], 0, &[]).unwrap();
        assert_eq!(string(&chosen.sequence, "mapping_offset"), Some("a"));
    }

    #[test]
    fn of_two_equal_sequences_the_one_hiding_fewer_frames_wins() {
        let rom = vec![0u8; 0x10];
        let sequence = |offset: &str, hidden: u8| {
            Json::object()
                .with("mapping_offset", offset)
                .with("mapping_pointers", vec![Json::Null])
                .with("hidden_frames", hidden)
        };
        let chosen = choose(&rom, &[sequence("a", 1), sequence("b", 0)], 0, &[]).unwrap();
        assert_eq!(string(&chosen.sequence, "mapping_offset"), Some("b"));
    }
}
