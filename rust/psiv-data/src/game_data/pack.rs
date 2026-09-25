//! The pack as a whole: the manifest against the record set, and every warp
//! against the map it leads to.
//!
//! Both directions of drift are defects. A manifest entry with no record, a record
//! the manifest does not list, an id packed and skipped at once, or a warp landing
//! on a map that is neither packed nor declared unpacked all stop the load.

use crate::error::DataError;
use crate::ids::MapId;
use crate::manifest::Manifest;
use crate::map::MapRecord;
use std::collections::BTreeMap;

pub(super) fn validate_manifest(
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

pub(super) fn validate_cross_references(
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
