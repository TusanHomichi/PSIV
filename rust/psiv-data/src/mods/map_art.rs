//! Reconciling `map_update_images` redirects when a mod replaces a map PNG.
//!
//! Live-palette maps record `"maps/<name>.png" -> "maps/<name>_indices.png"` in
//! `map_update_images` so the renderer can load the pack's pre-indexed copy.
//! When a mod supplies `assets/maps/<name>.png` without also supplying the
//! corresponding `_indices.png` (or supplies `.png` in a later mod than an
//! earlier mod's `_indices.png`), keeping the base redirect would hide the
//! mod's art behind the unmodded `_indices.png`.
//!
//! Removing that stale redirect in the overlay lets `field_map.rs`'s existing
//! `MapImageSource::Rewrite` path inspect the mod's PNG: indexed PNGs use live
//! CRAM colours, and ordinary RGB/RGBA PNGs fall back cleanly to baked colours.

use super::ModError;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Removes `map_update_images` entries in `out_dir/maps/*.json` whose source
/// image was replaced by a mod at a strictly newer layer than its mapped index
/// image (including when no mod replaced the index image at all).
pub(super) fn reconcile_replaced_map_update_images(
    out_dir: &Path,
    asset_layer: &BTreeMap<String, usize>,
) -> Result<(), ModError> {
    if asset_layer.is_empty() {
        return Ok(());
    }
    let maps_dir = out_dir.join("maps");
    if !maps_dir.is_dir() {
        return Ok(());
    }

    let mut map_files = Vec::new();
    let entries = fs::read_dir(&maps_dir).map_err(|error| {
        ModError::new(
            "overlay",
            "maps",
            format!("could not read overlay maps directory: {error}"),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ModError::new(
                "overlay",
                "maps",
                format!("could not read overlay maps entry: {error}"),
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") && path.is_file() {
            map_files.push(path);
        }
    }
    map_files.sort();

    for path in map_files {
        let rel = path
            .strip_prefix(out_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&path).map_err(|error| {
            ModError::new("overlay", &rel, format!("could not read map JSON: {error}"))
        })?;
        if !text.contains("\"map_update_images\"") {
            continue;
        }
        // Leave malformed JSON untouched so `GameData::load` attributes the
        // parse failure to the mod that introduced it.
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
                let png_layer = asset_layer.get(image_key)?;
                let index_layer = index_val
                    .as_str()
                    .and_then(|idx_path| asset_layer.get(idx_path));
                index_layer
                    .is_none_or(|idx_layer| idx_layer < png_layer)
                    .then(|| image_key.clone())
            })
            .collect();
        if to_remove.is_empty() {
            continue;
        }
        for key in to_remove {
            images.remove(&key);
        }
        // Unlink first so the base pack or a mod's raw asset is never mutated.
        if path.symlink_metadata().is_ok() {
            fs::remove_file(&path).map_err(|error| {
                ModError::new(
                    "overlay",
                    &rel,
                    format!("could not replace map JSON link: {error}"),
                )
            })?;
        }
        let rendered = serde_json::to_string(&json).map_err(|error| {
            ModError::new(
                "overlay",
                &rel,
                format!("could not serialize map JSON: {error}"),
            )
        })?;
        fs::write(&path, rendered).map_err(|error| {
            ModError::new(
                "overlay",
                &rel,
                format!("could not write reconciled map JSON: {error}"),
            )
        })?;
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
            // replaced (in which case `apply_mods` would not drop the mapping).
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
