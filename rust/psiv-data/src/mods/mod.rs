//! Shareable mod folders layered over a ROM-built runtime pack (#118).
//!
//! A mod folder carries only the modder's edits and original assets, never
//! base pack files:
//!
//! - `mod.json`: [`ModManifest`] (id, name, version, pack format, load order, enabled).
//! - `patches/<pack-rel-path>.json`: sparse ID-keyed JSON patches merged over
//!   base pack JSON tables.
//! - `assets/<pack-rel-path>`: replacement files (images, audio, etc.) layered
//!   over the base pack at the same relative path.
//!
//! [`apply_mods`] materializes an overlaid effective pack directory without
//! touching the base pack (linking untouched files and writing only patched
//! JSON and mod assets), then validates the result with [`crate::GameData::load`]
//! and [`crate::BattleFiles::load`]. Any failure returns a [`ModError`] naming
//! the offending mod, file, and place.

mod diff;
mod patch;
#[cfg(test)]
mod tests;

pub use diff::{DiffSummary, diff_packs};
pub use patch::{apply_json_patch, diff_json};

use crate::error::DataError;
use crate::manifest::PACK_FORMAT_VERSION;
use crate::{BattleFiles, GameData};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Current `mod.json` schema version.
pub const MOD_FORMAT_VERSION: u32 = 1;

/// Filename of a mod folder's manifest.
pub const MOD_MANIFEST_FILE: &str = "mod.json";

/// Subdirectory holding sparse ID-keyed JSON patches.
pub const MOD_PATCHES_DIR: &str = "patches";

/// Subdirectory holding mod-authored replacement assets.
pub const MOD_ASSETS_DIR: &str = "assets";

const fn default_pack_format_version() -> u32 {
    PACK_FORMAT_VERSION
}

const fn default_order() -> i32 {
    100
}

const fn default_enabled() -> bool {
    true
}

/// Metadata stored in `<mod-dir>/mod.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModManifest {
    /// Mod manifest format version; must equal [`MOD_FORMAT_VERSION`].
    pub format_version: u32,
    /// Stable slug (`[a-z0-9_-]`), used for load ordering and error reporting.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Mod version string (for display and sharing).
    pub version: String,
    /// Optional author attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Optional short summary of what the mod changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Target runtime pack format version; checked against [`PACK_FORMAT_VERSION`].
    #[serde(default = "default_pack_format_version")]
    pub pack_format_version: u32,
    /// Sort key relative to other mods (lower loads earlier; ties break by `id`).
    #[serde(default = "default_order")]
    pub order: i32,
    /// Whether directory discovery loads this mod automatically.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl ModManifest {
    /// Constructs a valid manifest with default format versions, order 100, and
    /// `enabled: true`.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            format_version: MOD_FORMAT_VERSION,
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: None,
            description: None,
            pack_format_version: PACK_FORMAT_VERSION,
            order: default_order(),
            enabled: true,
        }
    }

    /// Validates manifest fields and version compatibility.
    pub fn validate(&self, manifest_path: &Path) -> Result<(), ModError> {
        let who = if self.id.is_empty() {
            manifest_path.display().to_string()
        } else {
            self.id.clone()
        };
        if self.format_version != MOD_FORMAT_VERSION {
            return Err(ModError::new(
                who,
                MOD_MANIFEST_FILE,
                format!(
                    "unsupported mod format_version {} (expected {MOD_FORMAT_VERSION})",
                    self.format_version
                ),
            ));
        }
        if !is_valid_mod_id(&self.id) {
            return Err(ModError::new(
                who,
                MOD_MANIFEST_FILE,
                format!(
                    "invalid mod id {:?}: use 1..=64 ASCII lowercase letters, digits, '-' or '_'",
                    self.id
                ),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(ModError::new(
                self.id.clone(),
                MOD_MANIFEST_FILE,
                "name must not be empty",
            ));
        }
        if self.version.trim().is_empty() {
            return Err(ModError::new(
                self.id.clone(),
                MOD_MANIFEST_FILE,
                "version must not be empty",
            ));
        }
        if self.pack_format_version != PACK_FORMAT_VERSION {
            return Err(ModError::new(
                self.id.clone(),
                MOD_MANIFEST_FILE,
                format!(
                    "targets pack format_version {} (this build uses {PACK_FORMAT_VERSION})",
                    self.pack_format_version
                ),
            ));
        }
        Ok(())
    }
}

/// A validated mod directory discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredMod {
    /// Root folder holding `mod.json`, `patches/`, and/or `assets/`.
    pub root: PathBuf,
    /// Parsed and validated manifest.
    pub manifest: ModManifest,
}

/// Outcome of layering mods over a base pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMods {
    /// Path the game should load from (`base_pack` when no mods are active,
    /// otherwise the materialized overlay directory).
    pub effective_pack: PathBuf,
    /// Active mods in exact load order.
    pub active: Vec<ModManifest>,
    /// Pack-relative JSON paths modified by `patches/`.
    pub patched_files: Vec<String>,
    /// Pack-relative asset paths supplied by `assets/`.
    pub replaced_assets: Vec<String>,
}

/// Error produced while reading, layering, diffing, or validating a mod.
///
/// Always identifies the mod, the relative file, and the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModError {
    /// Mod id (or folder path before `mod.json` is parsed).
    pub mod_id: String,
    /// Relative file path inside the mod or pack.
    pub file: String,
    /// Actionable failure detail (including schema/line location when present).
    pub detail: String,
}

impl ModError {
    /// Creates a structured mod error.
    #[must_use]
    pub fn new(
        mod_id: impl Into<String>,
        file: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            mod_id: mod_id.into(),
            file: file.into(),
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mod `{}` ({}): {}", self.mod_id, self.file, self.detail)
    }
}

impl std::error::Error for ModError {}

/// Checks whether `id` is a safe, portable mod slug.
#[must_use]
pub fn is_valid_mod_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Loads and validates a single mod directory's `mod.json`.
pub fn load_mod(mod_dir: &Path) -> Result<DiscoveredMod, ModError> {
    let manifest_path = mod_dir.join(MOD_MANIFEST_FILE);
    let text = fs::read_to_string(&manifest_path).map_err(|error| {
        ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            format!("could not read manifest: {error}"),
        )
    })?;
    let manifest: ModManifest = serde_json::from_str(&text).map_err(|error| {
        ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            format!("invalid JSON: {error}"),
        )
    })?;
    manifest.validate(&manifest_path)?;
    Ok(DiscoveredMod {
        root: mod_dir.to_path_buf(),
        manifest,
    })
}

/// Discovers enabled mod directories directly under `mods_dir`, sorted by
/// `(order, id)`.
///
/// Returns an empty list if `mods_dir` does not exist. Hidden entries (such as
/// the active-pack staging directory) and subdirectories without `mod.json` are
/// ignored.
pub fn discover_mods(mods_dir: &Path) -> Result<Vec<DiscoveredMod>, ModError> {
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(mods_dir).map_err(|error| {
        ModError::new(
            mods_dir.display().to_string(),
            ".",
            format!("could not read mods directory: {error}"),
        )
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            ModError::new(
                mods_dir.display().to_string(),
                ".",
                format!("could not read directory entry: {error}"),
            )
        })?;
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy();
        if name_lossy.starts_with('.') || name_lossy.starts_with('_') {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() || !path.join(MOD_MANIFEST_FILE).is_file() {
            continue;
        }
        let loaded = load_mod(&path)?;
        if loaded.manifest.enabled {
            found.push(loaded);
        }
    }
    found.sort_by(|a, b| {
        a.manifest
            .order
            .cmp(&b.manifest.order)
            .then_with(|| a.manifest.id.cmp(&b.manifest.id))
    });
    Ok(found)
}

/// Scaffolds a new mod folder with `mod.json`, `patches/`, and `assets/`.
pub fn init_mod_dir(out_dir: &Path, manifest: &ModManifest) -> Result<(), ModError> {
    manifest.validate(&out_dir.join(MOD_MANIFEST_FILE))?;
    fs::create_dir_all(out_dir.join(MOD_PATCHES_DIR)).map_err(|error| {
        ModError::new(
            &manifest.id,
            MOD_PATCHES_DIR,
            format!("mkdir failed: {error}"),
        )
    })?;
    fs::create_dir_all(out_dir.join(MOD_ASSETS_DIR)).map_err(|error| {
        ModError::new(
            &manifest.id,
            MOD_ASSETS_DIR,
            format!("mkdir failed: {error}"),
        )
    })?;
    let mut json = serde_json::to_string_pretty(manifest).map_err(|error| {
        ModError::new(
            &manifest.id,
            MOD_MANIFEST_FILE,
            format!("encode failed: {error}"),
        )
    })?;
    json.push('\n');
    fs::write(out_dir.join(MOD_MANIFEST_FILE), json).map_err(|error| {
        ModError::new(
            &manifest.id,
            MOD_MANIFEST_FILE,
            format!("write failed: {error}"),
        )
    })?;
    Ok(())
}

/// Applies `mods` in order over `base_pack` into `out_dir`, then validates the
/// resulting pack with [`GameData::load`] and [`BattleFiles::load`].
///
/// When `mods` is empty, `out_dir` is untouched and `effective_pack` points
/// directly at `base_pack`. The base pack is never modified.
pub fn apply_mods(
    base_pack: &Path,
    mods: &[DiscoveredMod],
    out_dir: &Path,
) -> Result<AppliedMods, ModError> {
    if mods.is_empty() {
        return Ok(AppliedMods {
            effective_pack: base_pack.to_path_buf(),
            active: Vec::new(),
            patched_files: Vec::new(),
            replaced_assets: Vec::new(),
        });
    }

    let base_canonical = base_pack
        .canonicalize()
        .unwrap_or_else(|_| base_pack.to_path_buf());
    reset_directory(out_dir)?;
    mirror_base_pack(&base_canonical, &base_canonical, out_dir)?;

    let mut provenance: BTreeMap<String, String> = BTreeMap::new();
    let mut patched_set: BTreeMap<String, ()> = BTreeMap::new();
    let mut asset_set: BTreeMap<String, ()> = BTreeMap::new();

    for m in mods {
        let mod_id = &m.manifest.id;
        let patches_root = m.root.join(MOD_PATCHES_DIR);
        if patches_root.is_dir() {
            let files = collect_relative_files(&patches_root, mod_id, MOD_PATCHES_DIR)?;
            for rel in files {
                let rel_str = normalize_rel(&rel, mod_id, MOD_PATCHES_DIR)?;
                if !rel_str.ends_with(".json") {
                    return Err(ModError::new(
                        mod_id,
                        format!("{MOD_PATCHES_DIR}/{rel_str}"),
                        "patch files must have a .json extension",
                    ));
                }
                let target = out_dir.join(&rel);
                if !target.is_file() {
                    return Err(ModError::new(
                        mod_id,
                        format!("{MOD_PATCHES_DIR}/{rel_str}"),
                        format!("base pack has no `{rel_str}` to patch"),
                    ));
                }
                let base_text = fs::read_to_string(&target).map_err(|error| {
                    ModError::new(
                        mod_id,
                        &rel_str,
                        format!("could not read base file: {error}"),
                    )
                })?;
                let mut base_json: serde_json::Value =
                    serde_json::from_str(&base_text).map_err(|error| {
                        ModError::new(mod_id, &rel_str, format!("base JSON invalid: {error}"))
                    })?;
                let patch_path = patches_root.join(&rel);
                let patch_text = fs::read_to_string(&patch_path).map_err(|error| {
                    ModError::new(
                        mod_id,
                        format!("{MOD_PATCHES_DIR}/{rel_str}"),
                        format!("could not read patch: {error}"),
                    )
                })?;
                let patch_json: serde_json::Value =
                    serde_json::from_str(&patch_text).map_err(|error| {
                        ModError::new(
                            mod_id,
                            format!("{MOD_PATCHES_DIR}/{rel_str}"),
                            format!("invalid patch JSON: {error}"),
                        )
                    })?;
                patch::apply_json_patch(&mut base_json, &patch_json, &rel_str).map_err(
                    |detail| ModError::new(mod_id, format!("{MOD_PATCHES_DIR}/{rel_str}"), detail),
                )?;
                // Replace the symlink with a fresh file so base_pack is never written through.
                if target.symlink_metadata().is_ok() {
                    fs::remove_file(&target).map_err(|error| {
                        ModError::new(mod_id, &rel_str, format!("could not replace link: {error}"))
                    })?;
                }
                let rendered = serde_json::to_string(&base_json).map_err(|error| {
                    ModError::new(
                        mod_id,
                        &rel_str,
                        format!("could not serialize JSON: {error}"),
                    )
                })?;
                fs::write(&target, rendered).map_err(|error| {
                    ModError::new(
                        mod_id,
                        &rel_str,
                        format!("could not write patched file: {error}"),
                    )
                })?;
                provenance.insert(rel_str.clone(), mod_id.clone());
                patched_set.insert(rel_str, ());
            }
        }

        let assets_root = m.root.join(MOD_ASSETS_DIR);
        if assets_root.is_dir() {
            let files = collect_relative_files(&assets_root, mod_id, MOD_ASSETS_DIR)?;
            for rel in files {
                let rel_str = normalize_rel(&rel, mod_id, MOD_ASSETS_DIR)?;
                if rel_str == "manifest.json" {
                    return Err(ModError::new(
                        mod_id,
                        format!("{MOD_ASSETS_DIR}/{rel_str}"),
                        "manifest.json cannot be replaced as a raw asset; patch individual tables instead",
                    ));
                }
                let target = out_dir.join(&rel);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        ModError::new(mod_id, &rel_str, format!("mkdir failed: {error}"))
                    })?;
                }
                if target.symlink_metadata().is_ok() {
                    fs::remove_file(&target).map_err(|error| {
                        ModError::new(mod_id, &rel_str, format!("could not replace link: {error}"))
                    })?;
                }
                let src = assets_root.join(&rel);
                let src_canonical = src.canonicalize().unwrap_or(src);
                link_or_copy(&src_canonical, &target).map_err(|error| {
                    ModError::new(
                        mod_id,
                        format!("{MOD_ASSETS_DIR}/{rel_str}"),
                        format!("could not stage asset: {error}"),
                    )
                })?;
                provenance.insert(rel_str.clone(), mod_id.clone());
                asset_set.insert(rel_str, ());
            }
        }
    }

    // Validate the resulting pack through the exact same schema loaders as retail.
    if let Err(error) = GameData::load(out_dir) {
        return Err(attribute_data_error(&error, out_dir, &provenance, mods));
    }
    if out_dir.join("battle").join("enemies.json").is_file()
        && let Err(error) = BattleFiles::load(out_dir)
    {
        return Err(attribute_data_error(&error, out_dir, &provenance, mods));
    }

    Ok(AppliedMods {
        effective_pack: out_dir.to_path_buf(),
        active: mods.iter().map(|m| m.manifest.clone()).collect(),
        patched_files: patched_set.into_keys().collect(),
        replaced_assets: asset_set.into_keys().collect(),
    })
}

fn attribute_data_error(
    error: &DataError,
    out_dir: &Path,
    provenance: &BTreeMap<String, String>,
    mods: &[DiscoveredMod],
) -> ModError {
    let message = error.to_string();
    let rel_file = extract_error_file(error, out_dir).unwrap_or_else(|| {
        provenance
            .keys()
            .find(|rel| message.contains(rel.as_str()))
            .cloned()
            .unwrap_or_else(|| "pack".to_owned())
    });
    let mod_id = provenance
        .get(&rel_file)
        .cloned()
        .or_else(|| {
            provenance
                .iter()
                .find(|(rel, _)| message.contains(rel.as_str()))
                .map(|(_, id)| id.clone())
        })
        .unwrap_or_else(|| {
            mods.last()
                .map(|m| m.manifest.id.clone())
                .unwrap_or_else(|| "unknown".to_owned())
        });
    ModError::new(mod_id, rel_file, message)
}

fn extract_error_file(error: &DataError, out_dir: &Path) -> Option<String> {
    let strip = |path: &Path| -> String {
        path.strip_prefix(out_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    };
    match error {
        DataError::Io { path, .. }
        | DataError::Json { path, .. }
        | DataError::ManifestMismatch { path, .. }
        | DataError::Dialogue { path, .. }
        | DataError::Presentation { path, .. }
        | DataError::Sound { path, .. } => Some(strip(path)),
        _ => None,
    }
}

fn reset_directory(dir: &Path) -> Result<(), ModError> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|error| {
            ModError::new(
                "overlay",
                dir.display().to_string(),
                format!("could not clear overlay directory: {error}"),
            )
        })?;
    }
    fs::create_dir_all(dir).map_err(|error| {
        ModError::new(
            "overlay",
            dir.display().to_string(),
            format!("could not create overlay directory: {error}"),
        )
    })
}

fn mirror_base_pack(base_root: &Path, current: &Path, out_root: &Path) -> Result<(), ModError> {
    let entries = fs::read_dir(current).map_err(|error| {
        ModError::new(
            "base-pack",
            current.display().to_string(),
            format!("could not read base pack directory: {error}"),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ModError::new(
                "base-pack",
                current.display().to_string(),
                format!("could not read entry: {error}"),
            )
        })?;
        let src = entry.path();
        let rel = src.strip_prefix(base_root).unwrap_or(&src);
        let dst = out_root.join(rel);
        let meta = fs::metadata(&src).map_err(|error| {
            ModError::new(
                "base-pack",
                rel.display().to_string(),
                format!("could not stat base file: {error}"),
            )
        })?;
        if meta.is_dir() {
            fs::create_dir_all(&dst).map_err(|error| {
                ModError::new(
                    "base-pack",
                    rel.display().to_string(),
                    format!("mkdir failed: {error}"),
                )
            })?;
            mirror_base_pack(base_root, &src, out_root)?;
        } else if meta.is_file() {
            link_or_copy(&src, &dst).map_err(|error| {
                ModError::new(
                    "base-pack",
                    rel.display().to_string(),
                    format!("could not link base file: {error}"),
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn link_or_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(not(unix))]
fn link_or_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::hard_link(src, dst).or_else(|_| fs::copy(src, dst).map(|_| ()))
}

pub(crate) fn collect_relative_files(
    root: &Path,
    mod_id: &str,
    label: &str,
) -> Result<Vec<PathBuf>, ModError> {
    let mut out = Vec::new();
    collect_recursive(root, root, mod_id, label, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_recursive(
    root: &Path,
    current: &Path,
    mod_id: &str,
    label: &str,
    out: &mut Vec<PathBuf>,
) -> Result<(), ModError> {
    let entries = fs::read_dir(current)
        .map_err(|error| ModError::new(mod_id, label, format!("could not read dir: {error}")))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ModError::new(mod_id, label, format!("could not read dir entry: {error}"))
        })?;
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy();
        if name_lossy.starts_with('.') || name_lossy.starts_with('_') {
            continue;
        }
        let path = entry.path();
        let meta = fs::metadata(&path).map_err(|error| {
            ModError::new(mod_id, label, format!("could not stat entry: {error}"))
        })?;
        if meta.is_dir() {
            collect_recursive(root, &path, mod_id, label, out)?;
        } else if meta.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

pub(crate) fn normalize_rel(rel: &Path, mod_id: &str, label: &str) -> Result<String, ModError> {
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(segment) => parts.push(segment.to_string_lossy().into_owned()),
            _ => {
                return Err(ModError::new(
                    mod_id,
                    format!("{label}/{}", rel.display()),
                    "path traversal or absolute path is not allowed in a mod",
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(ModError::new(mod_id, label, "empty relative path"));
    }
    Ok(parts.join("/"))
}
