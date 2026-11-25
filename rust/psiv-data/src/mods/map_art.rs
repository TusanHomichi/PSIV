//! Reconciling `map_update_images` redirects when a mod replaces a map PNG.
//!
//! Live-palette maps record `"maps/<name>.png" -> "maps/<name>_indices.png"` in
//! `map_update_images` so the renderer can load the pack's pre-indexed copy.
//! When a mod supplies `assets/maps/<name>.png` without also supplying the
//! corresponding `_indices.png` (or supplies `.png` in a later mod than an
//! earlier mod's `_indices.png`), keeping the base redirect would hide the
//! mod's art behind the unmodded `_indices.png`.
//!
//! Removing that stale redirect in memory inside [`PackOverlay`] lets
//! `field_map.rs`'s existing `MapImageSource::Rewrite` path inspect the mod's
//! PNG: indexed PNGs use live CRAM colours, and ordinary RGB/RGBA PNGs fall
//! back cleanly to baked colours.

use super::{ModError, PackOverlay};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// Whether a `map_update_images` redirect `image_key -> index_path` is stale
/// under `asset_layers` (the `.png` was replaced by a mod at a strictly newer
/// layer than its mapped `_indices.png`, including when `_indices.png` was not
/// replaced by any mod).
pub(super) fn redirect_is_stale(
    asset_layers: &BTreeMap<String, usize>,
    image_key: &str,
    index_path: &str,
) -> bool {
    let Some(png_layer) = asset_layers.get(image_key) else {
        return false;
    };
    let index_layer = asset_layers.get(index_path);
    index_layer.is_none_or(|idx_layer| idx_layer < png_layer)
}

/// Removes stale `map_update_images` entries in memory for `maps/*.json` inside
/// `overlay` whenever the source image was replaced by a mod at a strictly
/// newer layer than its mapped index image (or without replacing the index
/// image at all). Never writes to disk.
pub(super) fn reconcile_replaced_map_update_images(
    overlay: &mut PackOverlay,
) -> Result<(), ModError> {
    if overlay.asset_layers().is_empty() {
        return Ok(());
    }

    let mut map_rels = BTreeSet::new();
    let maps_dir = overlay.base_pack().join("maps");
    if maps_dir.is_dir() {
        let entries = fs::read_dir(&maps_dir).map_err(|error| {
            ModError::new(
                "overlay",
                "maps",
                format!("could not read base maps directory: {error}"),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                ModError::new(
                    "overlay",
                    "maps",
                    format!("could not read base maps entry: {error}"),
                )
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                map_rels.insert(format!("maps/{name}"));
            }
        }
    }
    for key in overlay.override_keys() {
        if key.starts_with("maps/") && key.ends_with(".json") {
            map_rels.insert(key.to_owned());
        }
    }

    let asset_layers = overlay.asset_layers().clone();
    for rel in map_rels {
        let text = overlay.read_text(&rel).map_err(|error| {
            ModError::new("overlay", &rel, format!("could not read map JSON: {error}"))
        })?;
        if !text.contains("\"map_update_images\"") {
            continue;
        }
        // Leave malformed JSON untouched so `GameData::load_overlay` attributes
        // the parse failure to the mod that introduced it.
        let Ok(mut json) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(images) = json
            .get_mut("map_update_images")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let to_remove: Vec<String> = images
            .iter()
            .filter_map(|(image_key, index_val)| {
                let stale = match index_val.as_str() {
                    Some(idx_path) => redirect_is_stale(&asset_layers, image_key, idx_path),
                    None => asset_layers.contains_key(image_key),
                };
                stale.then(|| image_key.clone())
            })
            .collect();
        if to_remove.is_empty() {
            continue;
        }
        for key in to_remove {
            images.remove(&key);
        }
        let rendered = serde_json::to_string(&json).map_err(|error| {
            ModError::new(
                "overlay",
                &rel,
                format!("could not serialize map JSON: {error}"),
            )
        })?;
        overlay.insert_memory_override(rel, rendered);
    }

    Ok(())
}

/// Drops synthetic `map_update_images: {<key>: null}` entries produced when
/// diffing an overlaid pack back against its base pack (`diff_packs`), whenever
/// `<key>` is itself an exported asset and its base index image is not.
///
/// Returns `true` if `patch_val` still contains user edits after stripping.
pub(super) fn strip_reconciled_map_update_removals(
    rel_str: &str,
    base_json: &Value,
    patch_val: &mut Value,
    changed_assets: &BTreeSet<String>,
) -> bool {
    if !rel_str.starts_with("maps/") {
        return true;
    }
    let Some(patch_obj) = patch_val.as_object_mut() else {
        return true;
    };
    let Some(base_images) = base_json
        .get("map_update_images")
        .and_then(Value::as_object)
    else {
        return !patch_obj.is_empty();
    };
    let remove_field = if let Some(patch_images) = patch_obj
        .get_mut("map_update_images")
        .and_then(Value::as_object_mut)
    {
        patch_images.retain(|image_key, val| {
            if !val.is_null() || !changed_assets.contains(image_key) {
                return true;
            }
            // Keep the explicit deletion only if the index counterpart was also
            // replaced (in which case `PackOverlay` would not drop the mapping).
            base_images
                .get(image_key)
                .and_then(Value::as_str)
                .is_some_and(|idx_path| changed_assets.contains(idx_path))
        });
        patch_images.is_empty()
    } else {
        false
    };
    if remove_field {
        patch_obj.remove("map_update_images");
    }
    !patch_obj.is_empty()
}
