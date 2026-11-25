//! Mod directory inventory inspection and `mod.json` enable/disable toggling (#785).
//!
//! Shares candidate directory scanning and manifest reading with [`super::discover_mods`]
//! so runtime discovery and in-game UI always agree on which folders exist under `mods/`.

use crate::mods::{
    DiscoveredMod, MOD_MANIFEST_FILE, ModError, ModManifest, reject_duplicate_mod_ids,
};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// One candidate mod directory discovered under `mods/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModInventoryEntry {
    /// A mod whose `mod.json` parsed and passed validation (enabled or disabled).
    Valid(DiscoveredMod),
    /// A folder carrying a `mod.json` that failed to read, parse, or validate.
    Invalid {
        /// Folder path under `mods/`.
        root: PathBuf,
        /// Structured reason the manifest could not be used.
        error: ModError,
    },
}

/// Lists direct subdirectories of `mods_dir` that contain a `mod.json`, ignoring
/// hidden (`.`-prefixed) and internal (`_`-prefixed) folders.
pub(crate) fn candidate_mod_dirs(mods_dir: &Path) -> Result<Vec<PathBuf>, ModError> {
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
    let mut dirs = Vec::new();
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
        dirs.push(path);
    }
    dirs.sort();
    Ok(dirs)
}

/// Modeled keys of [`ModManifest`], used when leniently parsing an invalid
/// `mod.json` strictly for disabling it during in-game error recovery.
const KNOWN_MANIFEST_KEYS: &[&str] = &[
    "format_version",
    "id",
    "name",
    "version",
    "author",
    "description",
    "pack_format_version",
    "order",
    "enabled",
];

fn read_manifest_json(mod_dir: &Path) -> Result<serde_json::Value, ModError> {
    let manifest_path = mod_dir.join(MOD_MANIFEST_FILE);
    let text = fs::read_to_string(&manifest_path).map_err(|error| {
        ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            format!("could not read manifest: {error}"),
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            format!("invalid JSON: {error}"),
        )
    })
}

fn parse_manifest_lenient(
    mod_dir: &Path,
    raw: &serde_json::Value,
) -> Result<ModManifest, ModError> {
    let Some(obj) = raw.as_object() else {
        return Err(ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            "manifest root must be a JSON object",
        ));
    };
    let filtered: serde_json::Map<String, serde_json::Value> = obj
        .iter()
        .filter(|(k, _)| KNOWN_MANIFEST_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    serde_json::from_value(serde_json::Value::Object(filtered)).map_err(|error| {
        ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            format!("invalid JSON: {error}"),
        )
    })
}

/// Reads and strictly parses `<mod_dir>/mod.json` (enforcing
/// `#[serde(deny_unknown_fields)]`) without running version-compatibility
/// validation.
pub(crate) fn read_manifest_unchecked(mod_dir: &Path) -> Result<ModManifest, ModError> {
    let raw = read_manifest_json(mod_dir)?;
    serde_json::from_value(raw).map_err(|error| {
        ModError::new(
            mod_dir.display().to_string(),
            MOD_MANIFEST_FILE,
            format!("invalid JSON: {error}"),
        )
    })
}

/// Inspects every mod folder under `mods_dir` — enabled, disabled, and invalid —
/// without failing on the first broken mod.
///
/// Valid entries come first in deterministic `(order, id, root)` order, followed
/// by invalid entries sorted by `root`.
pub fn inspect_mods_dir(mods_dir: &Path) -> Result<Vec<ModInventoryEntry>, ModError> {
    let dirs = candidate_mod_dirs(mods_dir)?;
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for path in dirs {
        match read_manifest_unchecked(&path) {
            Ok(manifest) => {
                let manifest_path = path.join(MOD_MANIFEST_FILE);
                match manifest.validate(&manifest_path) {
                    Ok(()) => valid.push(DiscoveredMod {
                        root: path,
                        manifest,
                    }),
                    Err(error) => {
                        invalid.push(ModInventoryEntry::Invalid { root: path, error });
                    }
                }
            }
            Err(error) => invalid.push(ModInventoryEntry::Invalid { root: path, error }),
        }
    }
    valid.sort_by(|a, b| {
        a.manifest
            .order
            .cmp(&b.manifest.order)
            .then_with(|| a.manifest.id.cmp(&b.manifest.id))
            .then_with(|| a.root.cmp(&b.root))
    });
    let mut out: Vec<ModInventoryEntry> = valid.into_iter().map(ModInventoryEntry::Valid).collect();
    out.extend(invalid);
    Ok(out)
}

/// Discovers enabled mod directories under `mods_dir`, sorted by `(order, id)`.
///
/// A mod folder whose `mod.json` parses as a JSON object with `"enabled": false`
/// is skipped before strict schema and compatibility checks, so disabling an
/// outdated or unknown-key mod unblocks startup cleanly.
pub(crate) fn discover_enabled_mods(mods_dir: &Path) -> Result<Vec<DiscoveredMod>, ModError> {
    let dirs = candidate_mod_dirs(mods_dir)?;
    let mut found = Vec::new();
    for path in dirs {
        let raw = read_manifest_json(&path)?;
        if raw
            .as_object()
            .and_then(|obj| obj.get("enabled"))
            .and_then(serde_json::Value::as_bool)
            == Some(false)
        {
            continue;
        }
        let manifest: ModManifest = serde_json::from_value(raw).map_err(|error| {
            ModError::new(
                path.display().to_string(),
                MOD_MANIFEST_FILE,
                format!("invalid JSON: {error}"),
            )
        })?;
        if !manifest.enabled {
            continue;
        }
        manifest.validate(&path.join(MOD_MANIFEST_FILE))?;
        found.push(DiscoveredMod {
            root: path,
            manifest,
        });
    }
    found.sort_by(|a, b| {
        a.manifest
            .order
            .cmp(&b.manifest.order)
            .then_with(|| a.manifest.id.cmp(&b.manifest.id))
            .then_with(|| a.root.cmp(&b.root))
    });
    reject_duplicate_mod_ids(&found)?;
    Ok(found)
}

/// Updates `"enabled"` in `<mod_dir>/mod.json` atomically.
///
/// Enabling (`enabled == true`) strictly parses and validates the manifest
/// (rejecting unknown keys, incompatible versions, and duplicate enabled IDs)
/// so an incompatible or malformed mod cannot be turned on. Disabling
/// (`enabled == false`) allows turning off any parseable `mod.json` (including
/// one targeting an older `pack_format_version` or carrying unknown keys) while
/// preserving its existing fields so players can recover in-game without losing
/// metadata.
pub fn set_mod_enabled(mod_dir: &Path, enabled: bool) -> Result<ModManifest, ModError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let manifest_path = mod_dir.join(MOD_MANIFEST_FILE);
    let mut raw = read_manifest_json(mod_dir)?;
    let mut manifest = if enabled {
        let strict: ModManifest = serde_json::from_value(raw.clone()).map_err(|error| {
            ModError::new(
                mod_dir.display().to_string(),
                MOD_MANIFEST_FILE,
                format!("invalid JSON: {error}"),
            )
        })?;
        strict.validate(&manifest_path)?;
        if let Some(parent) = mod_dir.parent() {
            reject_enabled_id_collision(parent, mod_dir, &strict.id)?;
        }
        strict
    } else {
        parse_manifest_lenient(mod_dir, &raw)?
    };
    manifest.enabled = enabled;
    let owner = if manifest.id.is_empty() {
        mod_dir.display().to_string()
    } else {
        manifest.id.clone()
    };
    let Some(obj) = raw.as_object_mut() else {
        return Err(ModError::new(
            &owner,
            MOD_MANIFEST_FILE,
            "manifest root must be a JSON object",
        ));
    };
    obj.insert("enabled".to_owned(), serde_json::Value::Bool(enabled));
    let mut json = serde_json::to_string_pretty(&raw).map_err(|error| {
        ModError::new(&owner, MOD_MANIFEST_FILE, format!("encode failed: {error}"))
    })?;
    json.push('\n');

    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = mod_dir.join(format!(
        "{MOD_MANIFEST_FILE}.tmp.{}.{seq}",
        std::process::id()
    ));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(ModError::new(
            &owner,
            MOD_MANIFEST_FILE,
            format!("write failed: {error}"),
        ));
    }
    if let Err(error) = fs::rename(&tmp, &manifest_path) {
        let _ = fs::remove_file(&tmp);
        return Err(ModError::new(
            &owner,
            MOD_MANIFEST_FILE,
            format!("rename failed: {error}"),
        ));
    }
    Ok(manifest)
}

/// Rejects enabling `target_dir` when a sibling folder under `mods_dir` already
/// carries an enabled manifest with the same `id`.
fn reject_enabled_id_collision(
    mods_dir: &Path,
    target_dir: &Path,
    id: &str,
) -> Result<(), ModError> {
    let target_key = target_dir
        .canonicalize()
        .unwrap_or_else(|_| target_dir.to_path_buf());
    for dir in candidate_mod_dirs(mods_dir)? {
        let dir_key = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if dir_key == target_key {
            continue;
        }
        let Ok(other) = read_manifest_unchecked(&dir) else {
            continue;
        };
        if other.enabled && other.id == id {
            return Err(ModError::new(
                id,
                MOD_MANIFEST_FILE,
                format!(
                    "cannot enable `{id}`: another enabled mod with the same id already exists at {}",
                    dir.display()
                ),
            ));
        }
    }
    Ok(())
}

/// Finds the unique enabled, parseable mod folder under `mods_dir` identified by
/// `mod_id` (either by manifest `id` or by exact folder path).
///
/// Returns `None` when no matching enabled folder exists, when `mod.json` is
/// unparseable JSON, or when multiple enabled folders share the same `id` (to
/// avoid guessing which duplicate copy to disable).
#[must_use]
pub fn find_mod_dir_by_id(mods_dir: &Path, mod_id: &str) -> Option<PathBuf> {
    let dirs = candidate_mod_dirs(mods_dir).ok()?;
    let target_path = Path::new(mod_id);
    let mut matches = Vec::new();
    for dir in dirs {
        let Ok(raw) = read_manifest_json(&dir) else {
            continue;
        };
        let Ok(manifest) = parse_manifest_lenient(&dir, &raw) else {
            continue;
        };
        if !manifest.enabled {
            continue;
        }
        if manifest.id == mod_id || dir == target_path || dir.join(MOD_MANIFEST_FILE) == target_path
        {
            matches.push(dir);
        }
    }
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
}
