//! Loading and validating a runtime pack into a [`GameData`].

use crate::collision::Collision;
use crate::error::DataError;
use crate::ids::MapId;
use crate::manifest::{Manifest, PACK_FORMAT_VERSION};
use crate::map::{
    COLLISION_CELL_PIXELS, CellPos, ContentsType, DIALOGUE_TREE_COUNT, Dimensions, MapRecord,
    Treasure, Warp,
};
use std::collections::BTreeMap;
use std::path::Path;

/// A loaded, validated runtime pack.
///
/// Construct it with [`GameData::load`]. Once you hold one, every invariant the
/// `validate_*` functions in this module check is true of it: grids match their
/// declared dimensions, positions are inside their maps, warps point somewhere
/// the manifest knows about. Later layers can read it without re-checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameData {
    manifest: Manifest,
    maps: BTreeMap<MapId, MapRecord>,
}

impl GameData {
    /// Read `manifest.json` from `pack_dir` and every map it lists, then
    /// validate the lot.
    ///
    /// Fails on a format-version mismatch, a missing or unreadable file,
    /// malformed JSON, or any validation failure. There is no partial success:
    /// either the whole pack is coherent, or you get an error naming what is
    /// not.
    pub fn load(pack_dir: &Path) -> Result<GameData, DataError> {
        let manifest_path = pack_dir.join("manifest.json");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| DataError::io(&manifest_path, e))?;
        let manifest: Manifest =
            serde_json::from_str(&text).map_err(|e| DataError::json(&manifest_path, e))?;

        // Checked before anything else is read: if the shape changed, the map
        // files are not ours to interpret, and a parse error would be a
        // misleading way to find that out.
        check_version(manifest.format_version)?;

        let mut records = Vec::with_capacity(manifest.maps.len());
        for entry in &manifest.maps {
            let path = pack_dir.join(&entry.json);
            let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
            let record: MapRecord =
                serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;

            check_version(record.format_version)?;
            if record.id != entry.id {
                return Err(DataError::ManifestMismatch {
                    path,
                    field: "id",
                    manifest: entry.id.to_string(),
                    record: record.id.to_string(),
                });
            }
            if entry.symbol.is_some() && record.symbol.is_some() && entry.symbol != record.symbol {
                return Err(DataError::ManifestMismatch {
                    path,
                    field: "symbol",
                    manifest: format!("{:?}", entry.symbol.as_deref().unwrap_or("")),
                    record: format!("{:?}", record.symbol.as_deref().unwrap_or("")),
                });
            }
            records.push(record);
        }

        GameData::from_parts(manifest, records)
    }

    /// Assemble and validate from parts already in memory.
    ///
    /// [`GameData::load`] is this plus the file reading. Useful for tests and
    /// for anything that gets a pack from somewhere other than a directory.
    pub fn from_parts(manifest: Manifest, records: Vec<MapRecord>) -> Result<GameData, DataError> {
        check_version(manifest.format_version)?;

        let mut maps: BTreeMap<MapId, MapRecord> = BTreeMap::new();
        for record in records {
            check_version(record.format_version)?;
            validate_map(&record)?;
            if let Some(previous) = maps.insert(record.id, record) {
                return Err(DataError::DuplicateMapId {
                    id: previous.id,
                    detail: format!("two map records claim to be {}", previous.label()),
                });
            }
        }

        validate_manifest(&manifest, &maps)?;
        validate_cross_references(&manifest, &maps)?;

        Ok(GameData { manifest, maps })
    }

    /// The pack index: the ROM hash, the inventory and the skipped maps.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The map with this id, if it is packed.
    pub fn map(&self, id: MapId) -> Option<&MapRecord> {
        self.maps.get(&id)
    }

    /// Every packed map, in ascending id order.
    pub fn maps(&self) -> impl Iterator<Item = (MapId, &MapRecord)> {
        self.maps.iter().map(|(id, record)| (*id, record))
    }

    /// Every packed map id, ascending.
    pub fn map_ids(&self) -> impl Iterator<Item = MapId> + '_ {
        self.maps.keys().copied()
    }

    /// How many maps are packed.
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Whether the pack contains no maps at all.
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }

    /// Is this id packed?
    pub fn contains(&self, id: MapId) -> bool {
        self.maps.contains_key(&id)
    }

    /// Why this id was skipped, when the manifest says so. `None` for a packed
    /// id, for an unpacked warp target, and for one nobody has heard of.
    pub fn skip_reason(&self, id: MapId) -> Option<&str> {
        self.manifest.skip_reason(id)
    }

    /// Does the pack account for this id at all, whether by packing it or by
    /// declaring it unpacked? A warp target that fails this is a defect.
    pub fn knows(&self, id: MapId) -> bool {
        self.contains(id) || self.manifest.declares_unpacked(id)
    }
}

fn check_version(found: u32) -> Result<(), DataError> {
    if found != PACK_FORMAT_VERSION {
        return Err(DataError::FormatVersion {
            found,
            expected: PACK_FORMAT_VERSION,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Validation
//
// Always on, no lenient mode. A runtime that tolerates a broken pack stops
// being a test oracle for the cartridge (docs/RUNTIME_DESIGN.md, "Goal").
// Every failure names the map and the field, down to the array index.
// ---------------------------------------------------------------------------

fn validate_map(record: &MapRecord) -> Result<(), DataError> {
    let id = record.id;
    let dims = &record.dimensions;

    validate_dimensions(id, dims)?;
    validate_collision(id, dims, &record.collision)?;

    // The trees are numbered from one, so zero is as wrong as forty-four.
    if record.dialogue_tree == 0 || record.dialogue_tree > DIALOGUE_TREE_COUNT {
        return Err(DataError::validation(
            id,
            "dialogue_tree",
            format!(
                "tree {} does not exist; the ROM holds {DIALOGUE_TREE_COUNT} Kosinski dialogue \
                 trees, numbered 1..={DIALOGUE_TREE_COUNT}",
                record.dialogue_tree
            ),
        ));
    }

    for (index, npc) in record.npcs.iter().enumerate() {
        check_in_bounds(id, dims, npc.pos(), &format!("npcs[{index}]"))?;
    }

    for (index, chest) in record.treasure_chests.iter().enumerate() {
        check_in_bounds(id, dims, chest.pos(), &format!("treasure_chests[{index}]"))?;
        validate_treasure_contents(id, index, chest)?;
    }

    for (index, warp) in record.warps.iter().enumerate() {
        validate_warp_geometry(id, dims, index, warp)?;
    }

    Ok(())
}

fn validate_dimensions(id: MapId, dims: &Dimensions) -> Result<(), DataError> {
    if dims.width_cells == 0 || dims.height_cells == 0 {
        return Err(DataError::validation(
            id,
            "dimensions",
            format!(
                "a map cannot be {}x{} cells",
                dims.width_cells, dims.height_cells
            ),
        ));
    }
    if dims.cell_pixels != COLLISION_CELL_PIXELS {
        return Err(DataError::validation(
            id,
            "dimensions.cell_pixels",
            format!(
                "collision cells are {COLLISION_CELL_PIXELS} pixels, not {}; the whole fidelity \
                 spine is built on the 16-pixel cell grid",
                dims.cell_pixels
            ),
        ));
    }
    // A chunk is 4x4 tiles and a cell is 2x2, so the three ways the pack states
    // a map's size are one fact. Any disagreement means the packer's layout and
    // its collision decode came apart.
    for (axis, cells, chunks, pixels) in [
        (
            "width",
            dims.width_cells,
            dims.width_chunks,
            dims.width_pixels,
        ),
        (
            "height",
            dims.height_cells,
            dims.height_chunks,
            dims.height_pixels,
        ),
    ] {
        let from_chunks = chunks.saturating_mul(2);
        if cells != from_chunks {
            return Err(DataError::validation(
                id,
                format!("dimensions.{axis}_cells"),
                format!("{chunks} chunks is {from_chunks} cells, but the pack says {cells}"),
            ));
        }
        let expected = cells.saturating_mul(dims.cell_pixels);
        if pixels != expected {
            return Err(DataError::validation(
                id,
                format!("dimensions.{axis}_pixels"),
                format!(
                    "{cells} cells at {} pixels is {expected} pixels, but the pack says {pixels}",
                    dims.cell_pixels
                ),
            ));
        }
    }
    Ok(())
}

fn validate_collision(id: MapId, dims: &Dimensions, section: &Collision) -> Result<(), DataError> {
    let grid = &section.grid;
    if grid.width() != dims.width_cells || grid.height() != dims.height_cells {
        return Err(DataError::validation(
            id,
            "collision",
            format!(
                "the grid is {}x{} cells but the map declares {}x{}",
                grid.width(),
                grid.height(),
                dims.width_cells,
                dims.height_cells
            ),
        ));
    }
    Ok(())
}

fn validate_treasure_contents(id: MapId, index: usize, chest: &Treasure) -> Result<(), DataError> {
    // Byte 1 of the chest record selects how byte 3 is read. If the pack says
    // "item" and carries a meseta amount, one of the two is a lie and we cannot
    // tell which.
    let (present, absent, expected) = match chest.contents_type {
        ContentsType::Item => (chest.item_id.is_some(), chest.meseta.is_some(), "item_id"),
        ContentsType::Meseta => (chest.meseta.is_some(), chest.item_id.is_some(), "meseta"),
    };
    if !present {
        return Err(DataError::validation(
            id,
            format!("treasure_chests[{index}].{expected}"),
            format!(
                "contents_type is {:?} but {expected} is null",
                chest.contents_type
            ),
        ));
    }
    if absent {
        return Err(DataError::validation(
            id,
            format!("treasure_chests[{index}]"),
            format!(
                "contents_type is {:?} but both item_id and meseta are set",
                chest.contents_type
            ),
        ));
    }
    Ok(())
}

fn validate_warp_geometry(
    id: MapId,
    dims: &Dimensions,
    index: usize,
    warp: &Warp,
) -> Result<(), DataError> {
    // `rect` is the trigger area and is the thing that has to be on the map;
    // the packer clips it. `source` is the raw record coordinate, which the
    // standing-cell shift can push one row past the bottom edge, so it is not
    // bounds-checked -- see the module note on `WarpSource`.
    let Some(rect) = warp.rect else {
        return Ok(());
    };

    if rect.width == 0 || rect.height == 0 {
        return Err(DataError::validation(
            id,
            format!("warps[{index}].rect"),
            format!(
                "a {}x{} trigger rectangle can never fire; the packer emits no rectangle at all \
                 for an empty area",
                rect.width, rect.height
            ),
        ));
    }
    let end = rect.end();
    if end.x > dims.width_cells || end.y > dims.height_cells {
        return Err(DataError::validation(
            id,
            format!("warps[{index}].rect"),
            format!(
                "cells {}..{} x {}..{} run outside the {}x{} map; the packer must clip \
                 open-ended XYRange entries such as {} to the map",
                rect.x, end.x, rect.y, end.y, dims.width_cells, dims.height_cells, warp.range.name
            ),
        ));
    }
    Ok(())
}

fn check_in_bounds(
    id: MapId,
    dims: &Dimensions,
    pos: CellPos,
    field: &str,
) -> Result<(), DataError> {
    if pos.x >= dims.width_cells || pos.y >= dims.height_cells {
        return Err(DataError::validation(
            id,
            field.to_string(),
            format!(
                "cell ({}, {}) is outside the {}x{} map",
                pos.x, pos.y, dims.width_cells, dims.height_cells
            ),
        ));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &Manifest,
    maps: &BTreeMap<MapId, MapRecord>,
) -> Result<(), DataError> {
    let mut seen: BTreeMap<MapId, &'static str> = BTreeMap::new();
    for entry in &manifest.maps {
        if seen.insert(entry.id, "maps").is_some() {
            return Err(DataError::DuplicateMapId {
                id: entry.id,
                detail: "listed twice in `maps`".to_string(),
            });
        }
    }
    for entry in &manifest.skipped {
        if let Some(previous) = seen.insert(entry.id, "skipped") {
            return Err(DataError::DuplicateMapId {
                id: entry.id,
                detail: format!("listed in `skipped` and already in `{previous}`"),
            });
        }
    }

    // The manifest and the record set have to describe the same pack. Either
    // direction of drift means something was packed or unpacked without the
    // index being told.
    for entry in &manifest.maps {
        if !maps.contains_key(&entry.id) {
            return Err(DataError::validation(
                entry.id,
                "manifest.maps",
                format!(
                    "listed in the manifest as {} but no record was loaded",
                    entry.json
                ),
            ));
        }
    }
    for id in maps.keys() {
        if !manifest.packs(*id) {
            return Err(DataError::validation(
                *id,
                "manifest.maps",
                "a record was loaded for a map the manifest does not list",
            ));
        }
    }
    Ok(())
}

fn validate_cross_references(
    manifest: &Manifest,
    maps: &BTreeMap<MapId, MapRecord>,
) -> Result<(), DataError> {
    for (id, record) in maps {
        for (index, warp) in record.warps.iter().enumerate() {
            let target_id = warp.target.id;
            match maps.get(&target_id) {
                Some(target) => {
                    // The destination has to exist on the map the player lands
                    // on, not merely on this one.
                    let landing = warp.destination.pos();
                    if !target.contains(landing) {
                        return Err(DataError::validation(
                            *id,
                            format!("warps[{index}].destination"),
                            format!(
                                "cell ({}, {}) is outside map {} ({}), which is {}x{} cells",
                                landing.x,
                                landing.y,
                                target_id,
                                target.label(),
                                target.dimensions.width_cells,
                                target.dimensions.height_cells
                            ),
                        ));
                    }
                }
                None if manifest.declares_unpacked(target_id) => {}
                None => {
                    return Err(DataError::validation(
                        *id,
                        format!("warps[{index}].target"),
                        format!(
                            "targets map {target_id} ({}), which is neither packed nor listed in \
                             the manifest's `skipped` or `unpacked_warp_targets`",
                            warp.target.label()
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "game_data_tests.rs"]
mod tests;
