//! Map effects, the cartridge's flag-gated patches to a loaded map: the port of
//! `psiv_tools/map_effects/`, which owns the retail citations (#216, slice 3).
//!
//! The last section of a map record lists indexes into `MapDataManagerJmpTbl`.
//! Those routines run at map load and only then. [`decoder`] walks each into
//! the paths it can take, and [`extract_map_effects`] walks every real map's
//! list, resolves its layout writes against that map's geometry and counts what
//! it found. No claimed section writes these yet, so the gate holds them to
//! Python as the `map_effects` probe.

pub mod anchors;
pub mod decoder;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::layouts::Plane;
use super::records::{MapRecord, hex6};
use crate::PackError;
use crate::json::Json;
use crate::png::hex;
use anchors::{
    MAP_DATA_MANAGER_ROUTINES, MAP_LAYOUT_BG, MAP_LAYOUT_BYTES, MAP_LAYOUT_FG, bank_address,
    check_map_layout_bases, dispatch_table, flag_clears, flag_tests, kos_decomp, routine_address,
};
use decoder::{Decoder, LayoutTarget, Path, Write};

/// What a `layout_write` needs to become a cell on a particular map, by plane
/// (FG, then BG).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapGeometry {
    pub widths: [i64; 2],
    pub heights: [i64; 2],
}

impl MapGeometry {
    pub fn of(record: &MapRecord) -> Self {
        let dimensions = &record.dimensions;
        MapGeometry {
            widths: [
                i64::from(dimensions.fg_row_size) + 1,
                i64::from(dimensions.bg_row_size) + 1,
            ],
            heights: [
                i64::from(dimensions.fg_column_size) + 1,
                i64::from(dimensions.bg_column_size) + 1,
            ],
        }
    }
}

fn plane_index(plane: Plane) -> usize {
    match plane {
        Plane::Fg => 0,
        Plane::Bg => 1,
    }
}

fn plane_base(plane: Plane) -> i64 {
    match plane {
        Plane::Fg => MAP_LAYOUT_FG,
        Plane::Bg => MAP_LAYOUT_BG,
    }
}

/// `resolve_layout_write`: the displacement applies to an address, not a
/// coordinate, so it can leave the plane asked for. Which buffer the address
/// lands in names the plane written, and the offset is read with that plane's
/// row size. An address in neither buffer is emitted as a refusal.
pub fn resolve_layout_write(
    at: i64,
    (asked, displacement, chunk_id): (LayoutTarget, i16, Option<u16>),
    geometry: &MapGeometry,
) -> Json {
    let plane = asked.plane;
    let address = plane_base(plane)
        + i64::from(asked.chunk_y) * geometry.widths[plane_index(plane)]
        + i64::from(asked.chunk_x)
        + i64::from(displacement);
    let out = Json::object()
        .with("kind", "layout_write")
        .with("at", hex6(at as usize))
        .with("chunk_id", chunk_id)
        .with("requested_plane", plane.name())
        .with("displacement", displacement);
    let target = [Plane::Fg, Plane::Bg].into_iter().find(|&candidate| {
        (plane_base(candidate)..plane_base(candidate) + MAP_LAYOUT_BYTES).contains(&address)
    });
    let Some(target) = target else {
        return out
            .with("plane", plane.name())
            .with("out_of_bounds", true)
            .with("address", format!("0xFFFF{:04X}", address & 0xFFFF));
    };
    let offset = address - plane_base(target);
    let width = geometry.widths[plane_index(target)];
    let (chunk_y, chunk_x) = (offset / width, offset % width);
    let resolved = out
        .with("plane", target.name())
        .with("crosses_plane", target != plane)
        .with("chunk_x", chunk_x)
        .with("chunk_y", chunk_y)
        .with("cell_x", chunk_x * 2)
        .with("cell_y", chunk_y * 2)
        .with("out_of_bounds", false);
    if chunk_y >= geometry.heights[plane_index(target)] {
        resolved.with("past_map_layout", true)
    } else {
        resolved
    }
}

/// One jump-table entry, decoded or refused with its reason.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub index: usize,
    pub rom_offset: usize,
    pub paths: Vec<Path>,
    pub undecoded: Option<String>,
    pub raw_hex: String,
}

impl Entry {
    fn write_json(write: &Write, geometry: &MapGeometry) -> Json {
        if let Some(layout) = write.layout {
            return resolve_layout_write(write.at, layout, geometry);
        }
        let mut out = write.detail.clone();
        out.insert("kind", write.kind);
        out.insert("at", hex6(write.at as usize));
        out
    }

    /// `Entry.to_json`: this entry as the pack emits it on one map.
    pub fn to_json(&self, geometry: &MapGeometry) -> Json {
        let out = Json::object()
            .with("entry", self.index)
            .with("entry_hex", format!("0x{:02X}", self.index))
            .with("routine", hex6(self.rom_offset))
            .with("raw_hex", self.raw_hex.as_str());
        if let Some(reason) = &self.undecoded {
            return out.with("decoded", false).with("reason", reason.as_str());
        }
        let kinds: BTreeSet<&str> = self
            .paths
            .iter()
            .flat_map(|path| &path.writes)
            .map(|write| write.kind)
            .collect();
        let paths: Vec<Json> = self
            .paths
            .iter()
            // A path that does nothing is the routine's own "not set, return".
            .filter(|path| !path.writes.is_empty() || path.aborts)
            .map(|path| {
                Json::object()
                    .with(
                        "gates",
                        path.gates.iter().map(|g| g.to_json()).collect::<Vec<_>>(),
                    )
                    .with(
                        "unconditional",
                        path.gates.is_empty() && path.conditions.is_empty(),
                    )
                    .with("unmodelled_conditions", path.conditions.clone())
                    .with("aborts_remaining_entries", path.aborts)
                    .with("deferred", path.deferred.clone())
                    .with(
                        "writes",
                        path.writes
                            .iter()
                            .map(|write| Self::write_json(write, geometry))
                            .collect::<Vec<_>>(),
                    )
            })
            .collect();
        out.with("decoded", true)
            .with("kinds", kinds.into_iter().collect::<Vec<_>>())
            .with("paths", paths)
    }
}

/// The anchors every entry decodes against.
pub struct Anchors {
    pub table: usize,
    pub tests: HashMap<i64, &'static str>,
    pub clears: HashMap<i64, &'static str>,
    pub kos: i64,
}

impl Anchors {
    pub fn read(rom: &[u8]) -> Result<Self, PackError> {
        Ok(Anchors {
            table: dispatch_table(rom)?,
            tests: flag_tests(rom)?,
            clears: flag_clears(rom)?,
            kos: kos_decomp(rom)?,
        })
    }
}

/// `decode_entry`: an entry the reader refuses comes back with its reason.
pub fn decode_entry(rom: &[u8], index: usize, anchors: &Anchors) -> Result<Entry, PackError> {
    let address = routine_address(rom, anchors.table, index)?;
    let mut decoder = Decoder {
        rom,
        start: address as i64,
        tests: &anchors.tests,
        clears: &anchors.clears,
        kos: anchors.kos,
        paths: Vec::new(),
    };
    let raw = |end: usize| hex(&rom[address.min(rom.len())..end.min(rom.len())]);
    Ok(match decoder.run() {
        Err(reason) => Entry {
            index,
            rom_offset: address,
            paths: Vec::new(),
            undecoded: Some(reason),
            raw_hex: raw(address + 0x40),
        },
        Ok(()) => {
            let end = decoder
                .paths
                .iter()
                .flat_map(|path| &path.writes)
                .map(|write| write.at as usize)
                .max()
                .unwrap_or(address)
                + 8;
            Entry {
                index,
                rom_offset: address,
                paths: decoder.paths,
                undecoded: None,
                raw_hex: raw(end),
            }
        }
    })
}

/// What [`extract_map_effects`] returns.
pub struct MapEffects {
    /// By map id, each map's entries in list order.
    pub per_map: BTreeMap<usize, Vec<Json>>,
    pub replacements: Vec<Json>,
    pub dispatch_table: usize,
    pub census: Json,
}

pub(super) fn field<'a>(value: &'a Json, key: &str) -> Option<&'a Json> {
    let Json::Object(pairs) = value else {
        return None;
    };
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub(super) fn field_mut<'a>(value: &'a mut Json, key: &str) -> Option<&'a mut Json> {
    let Json::Object(pairs) = value else {
        return None;
    };
    pairs.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub(super) fn items<'a>(value: &'a Json, key: &str) -> &'a [Json] {
    match field(value, key) {
        Some(Json::Array(items)) => items,
        _ => &[],
    }
}

pub(super) fn text<'a>(value: &'a Json, key: &str) -> &'a str {
    match field(value, key) {
        Some(Json::Str(text)) => text,
        _ => "",
    }
}

pub(super) fn int(value: &Json, key: &str) -> Option<i128> {
    match field(value, key) {
        Some(Json::Int(number)) => Some(*number),
        _ => None,
    }
}

fn counts_json(counts: &BTreeMap<String, usize>) -> Json {
    let mut out = Json::object();
    for (key, count) in counts {
        out.insert(key.as_str(), *count);
    }
    out
}

fn ints(values: &BTreeSet<i128>) -> Json {
    Json::Array(values.iter().map(|v| Json::Int(*v)).collect())
}

/// `extract_map_effects`: every map's list, decoded, with the census.
pub fn extract_map_effects<'a>(
    rom: &[u8],
    maps: impl Iterator<Item = &'a MapRecord>,
) -> Result<MapEffects, PackError> {
    let anchors = Anchors::read(rom)?;
    check_map_layout_bases(rom)?;
    let real: Vec<&MapRecord> = maps.collect();
    let referenced: BTreeSet<usize> = real
        .iter()
        .flat_map(|record| &record.map_data_manager.ids)
        .map(|&id| usize::from(id))
        .collect();
    let mut entries = BTreeMap::new();
    for &index in &referenced {
        entries.insert(index, decode_entry(rom, index, &anchors)?);
    }

    let mut per_map = BTreeMap::new();
    let mut past_end = Vec::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut banks: BTreeMap<String, usize> = BTreeMap::new();
    let mut flags: BTreeMap<String, BTreeSet<i128>> = BTreeMap::new();
    let mut maps_per_kind: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut clears: BTreeMap<String, BTreeMap<i128, BTreeSet<i128>>> = BTreeMap::new();
    let mut replacements = Vec::new();
    let (mut unconditional, mut gated, mut position_gated) = (0usize, 0usize, 0usize);

    for record in &real {
        let ids = &record.map_data_manager.ids;
        if ids.is_empty() {
            continue;
        }
        let geometry = MapGeometry::of(record);
        let emitted: Vec<Json> = ids
            .iter()
            .map(|&id| entries[&usize::from(id)].to_json(&geometry))
            .collect();
        for entry in &emitted {
            for path in items(entry, "paths") {
                let gates = items(path, "gates");
                if !gates.is_empty() {
                    gated += 1;
                } else if field(path, "unconditional") == Some(&Json::Bool(true)) {
                    unconditional += 1;
                } else {
                    position_gated += 1;
                }
                for gate in gates {
                    let bank = text(gate, "bank").to_owned();
                    *banks.entry(bank.clone()).or_default() += 1;
                    flags.entry(bank).or_default().extend(int(gate, "flag"));
                }
                for write in items(path, "writes") {
                    let kind = text(write, "kind");
                    *kinds.entry(kind.to_owned()).or_default() += 1;
                    maps_per_kind
                        .entry(kind.to_owned())
                        .or_default()
                        .insert(record.id);
                    let count = record.objects.entries.len();
                    if let Some(index) = int(write, "object_index")
                        && index >= count as i128
                    {
                        past_end.push(
                            Json::object()
                                .with("map", record.id)
                                .with("entry", text(entry, "entry_hex"))
                                .with("at", text(write, "at"))
                                .with("object_index", Json::Int(index))
                                .with("map_objects", count),
                        );
                    }
                    if kind == "flag_clear" {
                        clears
                            .entry(text(write, "bank").to_owned())
                            .or_default()
                            .entry(int(entry, "entry").unwrap_or(0))
                            .or_default()
                            .extend(int(write, "flag"));
                    }
                    if kind == "layout_replace" {
                        replacements.push(
                            Json::object()
                                .with("map", record.id)
                                .with("plane", text(write, "plane"))
                                .with("source", text(write, "source")),
                        );
                    }
                }
            }
        }
        per_map.insert(record.id, emitted);
    }

    let mut undecoded = Vec::new();
    for (&index, entry) in &entries {
        if let Some(reason) = &entry.undecoded {
            let on: Vec<usize> = real
                .iter()
                .filter(|record| {
                    record
                        .map_data_manager
                        .ids
                        .iter()
                        .any(|&id| usize::from(id) == index)
                })
                .map(|record| record.id)
                .collect();
            undecoded.push(
                Json::object()
                    .with("entry", index)
                    .with("entry_hex", format!("0x{index:02X}"))
                    .with("routine", hex6(entry.rom_offset))
                    .with("reason", reason.as_str())
                    .with("maps", on),
            );
        }
    }

    let mut flag_clears_json = Json::object();
    for (bank, by_entry) in &clears {
        let all: BTreeSet<i128> = by_entry.values().flatten().copied().collect();
        let mut listed = Json::object();
        for (entry, ids) in by_entry {
            listed.insert(format!("0x{entry:02X}"), ints(ids));
        }
        flag_clears_json.insert(
            bank.as_str(),
            Json::object()
                .with("bank_address", format!("0xFFFF{:04X}", bank_address(bank)))
                .with("entries", by_entry.len())
                .with("flag_ids", ints(&all))
                .with("by_entry", listed),
        );
    }
    let mut flags_per_bank = Json::object();
    for (bank, ids) in &flags {
        flags_per_bank.insert(bank.as_str(), ints(ids));
    }
    let map_counts: BTreeMap<String, usize> = maps_per_kind
        .iter()
        .map(|(kind, on)| (kind.clone(), on.len()))
        .collect();

    let census = Json::object()
        .with("jump_table", hex6(anchors.table))
        .with("table_entries", MAP_DATA_MANAGER_ROUTINES)
        .with("referenced_entries", referenced.len())
        .with(
            "unreferenced_entries",
            MAP_DATA_MANAGER_ROUTINES - referenced.len(),
        )
        .with("maps_with_entries", per_map.len())
        .with(
            "map_entry_pairs",
            per_map.values().map(Vec::len).sum::<usize>(),
        )
        .with("decoded_entries", referenced.len() - undecoded.len())
        .with("undecoded_entries", undecoded)
        .with("kinds", counts_json(&kinds))
        .with("maps_per_kind", counts_json(&map_counts))
        .with("gate_banks", counts_json(&banks))
        .with("flag_clears", flag_clears_json)
        .with("flags_per_bank", flags_per_bank)
        .with("object_writes_past_map_object_count", past_end)
        .with("paths_gated", gated)
        .with("paths_position_gated", position_gated)
        .with("paths_unconditional", unconditional)
        .with("evaluated", "map load only")
        .with(
            "note",
            "MapDataManager is reached from two jsr sites, both in a map loader, and \
             Map_Data_Manager_Addr is written at both and read nowhere: these apply when a map \
             is built and are never re-evaluated while it is loaded. A routine returning \
             non-zero aborts the rest of that map's list, which is what \
             aborts_remaining_entries carries; no retail routine does.",
        );
    Ok(MapEffects {
        per_map,
        replacements,
        dispatch_table: anchors.table,
        census,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(plane: Plane, chunk_x: u16, chunk_y: u16) -> LayoutTarget {
        LayoutTarget {
            plane,
            chunk_x,
            chunk_y,
        }
    }

    const GEOMETRY: MapGeometry = MapGeometry {
        widths: [32, 64],
        heights: [4, 4],
    };

    fn number(value: &Json, key: &str) -> i128 {
        int(value, key).unwrap()
    }

    #[test]
    fn a_displacement_can_leave_the_plane_and_uses_the_landing_planes_rows() {
        // BG row 2 column 3 is $B083; $1000 below is FG offset $83, and FG's
        // rows are 32 wide: row 4, column 3, past FG's height of 4.
        let out =
            resolve_layout_write(0x10, (target(Plane::Bg, 3, 2), -0x1000, Some(7)), &GEOMETRY);
        assert_eq!(text(&out, "plane"), "fg");
        assert_eq!(text(&out, "requested_plane"), "bg");
        assert_eq!(field(&out, "crosses_plane"), Some(&Json::Bool(true)));
        assert_eq!((number(&out, "chunk_x"), number(&out, "chunk_y")), (3, 4));
        assert_eq!((number(&out, "cell_x"), number(&out, "cell_y")), (6, 8));
        assert_eq!(field(&out, "past_map_layout"), Some(&Json::Bool(true)));
    }

    #[test]
    fn a_write_inside_its_plane_is_not_marked() {
        let out = resolve_layout_write(0x10, (target(Plane::Fg, 3, 1), 1, None), &GEOMETRY);
        assert_eq!(field(&out, "crosses_plane"), Some(&Json::Bool(false)));
        assert_eq!((number(&out, "chunk_x"), number(&out, "chunk_y")), (4, 1));
        assert_eq!(field(&out, "past_map_layout"), None);
        assert_eq!(field(&out, "chunk_id"), Some(&Json::Null));
    }

    #[test]
    fn an_address_in_neither_buffer_is_a_refusal_and_not_a_coordinate() {
        let out = resolve_layout_write(0x10, (target(Plane::Fg, 0, 0), -2, Some(1)), &GEOMETRY);
        assert_eq!(field(&out, "out_of_bounds"), Some(&Json::Bool(true)));
        assert_eq!(text(&out, "address"), "0xFFFF9FFE");
        assert_eq!(field(&out, "chunk_x"), None);
    }
}
