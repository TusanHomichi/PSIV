//! Extracting a legal-to-share mod folder by diffing an edited pack against a
//! base pack (`psiv-pack mod diff`).

use super::{
    MOD_ASSETS_DIR, MOD_MANIFEST_FILE, MOD_PATCHES_DIR, ModError, ModManifest,
    collect_relative_files, normalize_rel, patch::diff_json,
};
use std::fs;
use std::path::Path;

/// Report returned by [`diff_packs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummary {
    /// Relative paths written under `patches/`.
    pub patches: Vec<String>,
    /// Relative paths copied under `assets/`.
    pub assets: Vec<String>,
}

/// Diffs `edited_pack` against `base_pack` and writes a shareable mod folder
/// to `out_mod`.
///
/// Unchanged files are never copied. Changed JSON tables become sparse ID-keyed
/// patches under `patches/`; changed or added binary assets (PNGs, sound, etc.)
/// are copied under `assets/`.
pub fn diff_packs(
    base_pack: &Path,
    edited_pack: &Path,
    out_mod: &Path,
    manifest: &ModManifest,
) -> Result<DiffSummary, ModError> {
    manifest.validate(&out_mod.join(MOD_MANIFEST_FILE))?;
    if !base_pack.join("manifest.json").is_file() {
        return Err(ModError::new(
            &manifest.id,
            "manifest.json",
            format!("base pack `{}` has no manifest.json", base_pack.display()),
        ));
    }
    if !edited_pack.join("manifest.json").is_file() {
        return Err(ModError::new(
            &manifest.id,
            "manifest.json",
            format!(
                "edited pack `{}` has no manifest.json",
                edited_pack.display()
            ),
        ));
    }

    let edited_files = collect_relative_files(edited_pack, &manifest.id, "edited-pack")?;
    let mut staged_patches: Vec<(std::path::PathBuf, String, String)> = Vec::new();
    let mut staged_assets: Vec<(std::path::PathBuf, String, Vec<u8>)> = Vec::new();

    for rel in edited_files {
        let rel_str = normalize_rel(&rel, &manifest.id, "edited-pack")?;
        let edited_path = edited_pack.join(&rel);
        let base_path = base_pack.join(&rel);
        let edited_bytes = fs::read(&edited_path).map_err(|error| {
            ModError::new(&manifest.id, &rel_str, format!("read failed: {error}"))
        })?;
        if base_path.is_file() {
            let base_bytes = fs::read(&base_path).map_err(|error| {
                ModError::new(&manifest.id, &rel_str, format!("read base failed: {error}"))
            })?;
            if base_bytes == edited_bytes {
                continue;
            }
            if rel_str.ends_with(".json") {
                let base_json: serde_json::Value =
                    serde_json::from_slice(&base_bytes).map_err(|error| {
                        ModError::new(
                            &manifest.id,
                            &rel_str,
                            format!("base JSON invalid: {error}"),
                        )
                    })?;
                let edited_json: serde_json::Value = serde_json::from_slice(&edited_bytes)
                    .map_err(|error| {
                        ModError::new(
                            &manifest.id,
                            &rel_str,
                            format!("edited JSON invalid: {error}"),
                        )
                    })?;
                let patch_opt = diff_json(&base_json, &edited_json, &rel_str)
                    .map_err(|detail| ModError::new(&manifest.id, &rel_str, detail))?;
                if let Some(patch_val) = patch_opt {
                    let mut pretty = serde_json::to_string_pretty(&patch_val).map_err(|error| {
                        ModError::new(&manifest.id, &rel_str, format!("encode failed: {error}"))
                    })?;
                    pretty.push('\n');
                    staged_patches.push((rel, rel_str, pretty));
                }
                continue;
            }
        }

        if rel_str == "manifest.json" {
            continue;
        }
        staged_assets.push((rel, rel_str, edited_bytes));
    }

    // Clear previous exported patches/assets only after all diffs validate, so
    // a failed diff never wipes an earlier export and a clean re-export leaves
    // no stale files behind from reverted edits.
    fs::create_dir_all(out_mod).map_err(|error| {
        ModError::new(
            &manifest.id,
            out_mod.display().to_string(),
            format!("mkdir failed: {error}"),
        )
    })?;
    for subdir in [MOD_PATCHES_DIR, MOD_ASSETS_DIR] {
        let dir = out_mod.join(subdir);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|error| {
                ModError::new(
                    &manifest.id,
                    subdir,
                    format!("could not clear previous export: {error}"),
                )
            })?;
        }
        fs::create_dir_all(&dir).map_err(|error| {
            ModError::new(&manifest.id, subdir, format!("mkdir failed: {error}"))
        })?;
    }

    let mut patches = Vec::with_capacity(staged_patches.len());
    for (rel, rel_str, pretty) in staged_patches {
        let dst = out_mod.join(MOD_PATCHES_DIR).join(&rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ModError::new(&manifest.id, &rel_str, format!("mkdir failed: {error}"))
            })?;
        }
        fs::write(&dst, pretty).map_err(|error| {
            ModError::new(&manifest.id, &rel_str, format!("write failed: {error}"))
        })?;
        patches.push(rel_str);
    }

    let mut assets = Vec::with_capacity(staged_assets.len());
    for (rel, rel_str, edited_bytes) in staged_assets {
        let dst = out_mod.join(MOD_ASSETS_DIR).join(&rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ModError::new(&manifest.id, &rel_str, format!("mkdir failed: {error}"))
            })?;
        }
        fs::write(&dst, edited_bytes).map_err(|error| {
            ModError::new(
                &manifest.id,
                &rel_str,
                format!("write asset failed: {error}"),
            )
        })?;
        assets.push(rel_str);
    }

    let mut manifest_text = serde_json::to_string_pretty(manifest).map_err(|error| {
        ModError::new(
            &manifest.id,
            MOD_MANIFEST_FILE,
            format!("encode failed: {error}"),
        )
    })?;
    manifest_text.push('\n');
    fs::write(out_mod.join(MOD_MANIFEST_FILE), manifest_text).map_err(|error| {
        ModError::new(
            &manifest.id,
            MOD_MANIFEST_FILE,
            format!("write failed: {error}"),
        )
    })?;

    Ok(DiffSummary { patches, assets })
}
