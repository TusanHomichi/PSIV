//! Extracting a shareable mod folder by diffing an edited pack against a base
//! pack (`psiv-pack mod diff`), plus shareability auditing for `mod check`.

use super::{
    DiscoveredMod, MOD_ASSETS_DIR, MOD_MANIFEST_FILE, MOD_PATCHES_DIR, ModError, ModManifest,
    collect_relative_files,
    map_art::strip_reconciled_map_update_removals,
    normalize_rel,
    patch::{collect_whole_base_array_patches, diff_json_tracked},
    reject_overlapping_output,
};
use std::fmt;
use std::fs;
use std::path::Path;

/// Policy controlling whether [`diff_packs_with_options`] and
/// [`super::diff_json_with_options`] may export non-delta base-pack copies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffOptions {
    /// When `false` (the default), exports fail closed if they would copy a
    /// whole non-empty unkeyed base JSON array or copy a modified binary asset
    /// at a path that already exists in `base_pack`.
    ///
    /// Set `true` (`--allow-derived`) only for private local edits or when
    /// explicitly attesting that a replaced base-path asset is original work
    /// rather than edited cartridge data.
    pub allow_derived: bool,
}

/// Report returned by [`diff_packs`] and [`diff_packs_with_options`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummary {
    /// Relative paths written under `patches/`.
    pub patches: Vec<String>,
    /// Relative paths copied under `assets/`.
    pub assets: Vec<String>,
    /// Non-delta base-pack items exported because `allow_derived` was enabled
    /// (always empty for default strict shareable exports).
    pub derived_items: Vec<String>,
}

/// Advisory or warning produced when auditing a mod folder's shareability
/// against a base pack ([`audit_mod_shareability`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareabilityNotice {
    /// `assets/<path>` is byte-for-byte identical to the base pack file.
    IdenticalBaseAsset {
        /// Pack-relative asset path.
        path: String,
    },
    /// `assets/<path>` replaces an existing base pack file with modified bytes.
    ReplacedBaseAsset {
        /// Pack-relative asset path.
        path: String,
    },
    /// `patches/<file>` replaces a whole non-empty unkeyed base JSON array or
    /// raw full 2D grid array.
    WholeBaseArray {
        /// Pack-relative JSON path.
        file: String,
        /// Dot-separated field path inside the JSON file.
        field: String,
    },
}

impl ShareabilityNotice {
    /// Whether this notice indicates an unambiguously leaked unmodified base
    /// file (warning) vs. an item requiring author attestation (note).
    #[must_use]
    pub fn is_warning(&self) -> bool {
        matches!(self, Self::IdenticalBaseAsset { .. })
    }
}

impl fmt::Display for ShareabilityNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdenticalBaseAsset { path } => write!(
                f,
                "assets/{path} is byte-for-byte identical to the base pack; remove untouched base files before sharing"
            ),
            Self::ReplacedBaseAsset { path } => write!(
                f,
                "assets/{path} replaces a base pack asset; only share if it is original art/audio (not edited cartridge data)"
            ),
            Self::WholeBaseArray { file, field } => write!(
                f,
                "patches/{file} replaces whole base array `{field}`; use a sparse patch or keep private unless entirely original"
            ),
        }
    }
}

/// Diffs `edited_pack` against `base_pack` and writes a shareable mod folder
/// to `out_mod` with strict shareability enabled by default
/// (`DiffOptions::default()`).
///
/// Unchanged files are never copied. Changed JSON tables become sparse ID-keyed
/// or cell-level patches under `patches/`, and brand-new assets are copied
/// under `assets/`. Whole unkeyed base JSON array copies and modified base-path
/// binary assets are refused unless [`diff_packs_with_options`] is called with
/// `allow_derived: true`.
pub fn diff_packs(
    base_pack: &Path,
    edited_pack: &Path,
    out_mod: &Path,
    manifest: &ModManifest,
) -> Result<DiffSummary, ModError> {
    diff_packs_with_options(
        base_pack,
        edited_pack,
        out_mod,
        manifest,
        DiffOptions::default(),
    )
}

/// Like [`diff_packs`], with explicit [`DiffOptions`] controlling whether
/// whole unkeyed base arrays and modified base-path binary assets are allowed.
pub fn diff_packs_with_options(
    base_pack: &Path,
    edited_pack: &Path,
    out_mod: &Path,
    manifest: &ModManifest,
    options: DiffOptions,
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
    reject_overlapping_output(out_mod, base_pack, "base pack", &manifest.id)?;
    reject_overlapping_output(out_mod, edited_pack, "edited pack", &manifest.id)?;

    let base_files = collect_relative_files(base_pack, &manifest.id, "base-pack")?;
    let edited_files = collect_relative_files(edited_pack, &manifest.id, "edited-pack")?;
    let mut edited_set = std::collections::BTreeSet::new();
    let mut changed_assets = std::collections::BTreeSet::new();
    let mut raw_patches: Vec<(
        std::path::PathBuf,
        String,
        serde_json::Value,
        serde_json::Value,
    )> = Vec::new();
    let mut staged_assets: Vec<(std::path::PathBuf, String, Vec<u8>)> = Vec::new();
    let mut derived_items: Vec<String> = Vec::new();

    for rel in edited_files {
        let rel_str = normalize_rel(&rel, &manifest.id, "edited-pack")?;
        edited_set.insert(rel_str.clone());
        if rel_str == "manifest.json" {
            continue;
        }
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
                let mut copied_arrays = Vec::new();
                let patch_opt = diff_json_tracked(
                    &base_json,
                    &edited_json,
                    &rel_str,
                    options,
                    &mut copied_arrays,
                )
                .map_err(|detail| ModError::new(&manifest.id, &rel_str, detail))?;
                for field in copied_arrays {
                    derived_items.push(format!("{MOD_PATCHES_DIR}/{rel_str}#{field}"));
                }
                if let Some(patch_val) = patch_opt {
                    raw_patches.push((rel, rel_str, base_json, patch_val));
                }
                continue;
            }
            if !options.allow_derived {
                return Err(ModError::new(
                    &manifest.id,
                    &rel_str,
                    format!(
                        "refusing to copy modified base binary asset `{rel_str}` into `{MOD_ASSETS_DIR}/`: `mod diff` cannot verify that a modified base-pack file contains no cartridge-derived data (place original art directly in `{MOD_ASSETS_DIR}/` or re-run with `--allow-derived` for private local edits / explicit original-art attestation)"
                    ),
                ));
            }
            derived_items.push(format!("{MOD_ASSETS_DIR}/{rel_str}"));
        }

        changed_assets.insert(rel_str.clone());
        staged_assets.push((rel, rel_str, edited_bytes));
    }

    let mut staged_patches: Vec<(std::path::PathBuf, String, String)> =
        Vec::with_capacity(raw_patches.len());
    for (rel, rel_str, base_json, mut patch_val) in raw_patches {
        if !strip_reconciled_map_update_removals(
            &rel_str,
            &base_json,
            &mut patch_val,
            &changed_assets,
        ) {
            continue;
        }
        let mut pretty = serde_json::to_string_pretty(&patch_val).map_err(|error| {
            ModError::new(&manifest.id, &rel_str, format!("encode failed: {error}"))
        })?;
        pretty.push('\n');
        staged_patches.push((rel, rel_str, pretty));
    }

    for rel in base_files {
        let rel_str = normalize_rel(&rel, &manifest.id, "base-pack")?;
        if !edited_set.contains(&rel_str) {
            return Err(ModError::new(
                &manifest.id,
                &rel_str,
                "deleting base pack files is not supported in shareable mod folders",
            ));
        }
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

    Ok(DiffSummary {
        patches,
        assets,
        derived_items,
    })
}

/// Audits a mod folder against `base_pack` for shareability notices:
/// - unmodified base-pack files copied verbatim under `assets/`
/// - replacement files under `assets/` at existing base-pack paths
/// - whole non-empty base JSON array replacements under `patches/`
///
/// Never fails a valid mod; returns structured notices so callers such as
/// `psiv-pack mod check` can inform the author without rejecting original
/// hand-drawn replacement art or private local mods.
pub fn audit_mod_shareability(
    base_pack: &Path,
    discovered: &DiscoveredMod,
) -> Result<Vec<ShareabilityNotice>, ModError> {
    let mod_id = &discovered.manifest.id;
    let mut notices = Vec::new();

    let patches_root = discovered.root.join(MOD_PATCHES_DIR);
    if patches_root.is_dir() {
        let files = collect_relative_files(&patches_root, mod_id, MOD_PATCHES_DIR)?;
        for rel in files {
            let rel_str = normalize_rel(&rel, mod_id, MOD_PATCHES_DIR)?;
            if !rel_str.ends_with(".json") {
                continue;
            }
            let base_path = base_pack.join(&rel);
            if !base_path.is_file() {
                continue;
            }
            let Ok(base_text) = fs::read_to_string(&base_path) else {
                continue;
            };
            let Ok(patch_text) = fs::read_to_string(patches_root.join(&rel)) else {
                continue;
            };
            let (Ok(base_json), Ok(patch_json)) = (
                serde_json::from_str::<serde_json::Value>(&base_text),
                serde_json::from_str::<serde_json::Value>(&patch_text),
            ) else {
                continue;
            };
            for field in collect_whole_base_array_patches(&base_json, &patch_json, &rel_str) {
                notices.push(ShareabilityNotice::WholeBaseArray {
                    file: rel_str.clone(),
                    field,
                });
            }
        }
    }

    let assets_root = discovered.root.join(MOD_ASSETS_DIR);
    if assets_root.is_dir() {
        let files = collect_relative_files(&assets_root, mod_id, MOD_ASSETS_DIR)?;
        for rel in files {
            let rel_str = normalize_rel(&rel, mod_id, MOD_ASSETS_DIR)?;
            let base_path = base_pack.join(&rel);
            if !base_path.is_file() {
                continue;
            }
            let mod_bytes = fs::read(assets_root.join(&rel)).map_err(|error| {
                ModError::new(
                    mod_id,
                    format!("{MOD_ASSETS_DIR}/{rel_str}"),
                    format!("could not read asset: {error}"),
                )
            })?;
            let base_bytes = fs::read(&base_path).map_err(|error| {
                ModError::new(
                    mod_id,
                    &rel_str,
                    format!("could not read base asset: {error}"),
                )
            })?;
            if mod_bytes == base_bytes {
                notices.push(ShareabilityNotice::IdenticalBaseAsset { path: rel_str });
            } else {
                notices.push(ShareabilityNotice::ReplacedBaseAsset { path: rel_str });
            }
        }
    }

    Ok(notices)
}
