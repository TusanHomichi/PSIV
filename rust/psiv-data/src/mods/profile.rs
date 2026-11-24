//! Save-directory isolation and sidecar metadata for modded runs (#118).
//!
//! Modded runs never share the vanilla `saves/` folder by default: each active
//! mod combination gets its own deterministic subdirectory under
//! `saves/mods/<profile>/`, keyed by mod IDs in load order (never versions, so
//! updating a mod keeps the player's progress). A small `mod_profile.json`
//! sidecar lives next to `slot_1..3.sram` without touching any `.sram` bytes.

use crate::mods::{ModError, ModManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Filename of the sidecar written alongside modded save slots.
pub const MOD_PROFILE_FILE: &str = "mod_profile.json";

/// Current `mod_profile.json` schema version.
pub const MOD_PROFILE_FORMAT_VERSION: u32 = 1;

/// Maximum readable length of an unhashed multi-mod profile slug.
const MAX_PLAIN_PROFILE_SLUG_LEN: usize = 48;

/// One active mod recorded in `mod_profile.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModProfileEntry {
    /// Stable mod id.
    pub id: String,
    /// Human-readable mod name at the time the save was written.
    pub name: String,
    /// Mod version string at the time the save was written.
    pub version: String,
}

/// Sidecar metadata stored in `mod_profile.json` beside modded `.sram` files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModProfile {
    /// Profile sidecar format version (`MOD_PROFILE_FORMAT_VERSION`).
    pub format_version: u32,
    /// Active mods in load order when this save directory was written.
    pub mods: Vec<ModProfileEntry>,
}

/// Difference between a save directory's recorded `mod_profile.json` and the
/// currently loaded mod list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModProfileDiff {
    /// Mod IDs that are active now but were not recorded in the save profile.
    pub added: Vec<String>,
    /// Mod IDs that were recorded in the save profile but are not active now.
    pub removed: Vec<String>,
    /// `(id, previous_version, current_version)` for mods whose version changed.
    pub version_changes: Vec<(String, String, String)>,
    /// True when the same mod IDs are active in a different load order.
    pub order_changed: bool,
}

impl ModProfileDiff {
    /// Returns `true` when the saved profile matches the active mod set and
    /// versions exactly.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.version_changes.is_empty()
            && !self.order_changed
    }
}

impl ModProfile {
    /// Builds a profile snapshot from the active manifests in load order.
    #[must_use]
    pub fn from_manifests(active: &[ModManifest]) -> Self {
        Self {
            format_version: MOD_PROFILE_FORMAT_VERSION,
            mods: active
                .iter()
                .map(|m| ModProfileEntry {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    version: m.version.clone(),
                })
                .collect(),
        }
    }

    /// Compares `self` (the currently active mods) against `previous` (loaded
    /// from an existing save directory's `mod_profile.json`).
    #[must_use]
    pub fn diff_against(&self, previous: &Self) -> ModProfileDiff {
        let prev_map: BTreeMap<&str, &str> = previous
            .mods
            .iter()
            .map(|m| (m.id.as_str(), m.version.as_str()))
            .collect();
        let curr_map: BTreeMap<&str, &str> = self
            .mods
            .iter()
            .map(|m| (m.id.as_str(), m.version.as_str()))
            .collect();

        let added = self
            .mods
            .iter()
            .filter(|m| !prev_map.contains_key(m.id.as_str()))
            .map(|m| m.id.clone())
            .collect::<Vec<_>>();
        let removed = previous
            .mods
            .iter()
            .filter(|m| !curr_map.contains_key(m.id.as_str()))
            .map(|m| m.id.clone())
            .collect::<Vec<_>>();
        let version_changes = self
            .mods
            .iter()
            .filter_map(|m| {
                let prev = prev_map.get(m.id.as_str())?;
                (*prev != m.version.as_str())
                    .then(|| (m.id.clone(), (*prev).to_owned(), m.version.clone()))
            })
            .collect::<Vec<_>>();

        let order_changed = added.is_empty()
            && removed.is_empty()
            && self
                .mods
                .iter()
                .map(|m| m.id.as_str())
                .ne(previous.mods.iter().map(|m| m.id.as_str()));

        ModProfileDiff {
            added,
            removed,
            version_changes,
            order_changed,
        }
    }

    /// Reads `mod_profile.json` from `dir` if present.
    pub fn read_from_dir(dir: &Path) -> Result<Option<Self>, ModError> {
        let path = dir.join(MOD_PROFILE_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path).map_err(|error| {
            ModError::new(
                "save-profile",
                MOD_PROFILE_FILE,
                format!("could not read profile: {error}"),
            )
        })?;
        let profile: Self = serde_json::from_str(&text).map_err(|error| {
            ModError::new(
                "save-profile",
                MOD_PROFILE_FILE,
                format!("invalid JSON: {error}"),
            )
        })?;
        Ok(Some(profile))
    }

    /// Atomically writes `mod_profile.json` into `dir`.
    pub fn write_to_dir(&self, dir: &Path) -> Result<(), ModError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        fs::create_dir_all(dir).map_err(|error| {
            ModError::new(
                "save-profile",
                MOD_PROFILE_FILE,
                format!("could not create save dir: {error}"),
            )
        })?;
        let mut json = serde_json::to_string_pretty(self).map_err(|error| {
            ModError::new(
                "save-profile",
                MOD_PROFILE_FILE,
                format!("encode failed: {error}"),
            )
        })?;
        json.push('\n');

        let path = dir.join(MOD_PROFILE_FILE);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(
            "{MOD_PROFILE_FILE}.tmp.{}.{seq}",
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
                "save-profile",
                MOD_PROFILE_FILE,
                format!("write failed: {error}"),
            ));
        }
        if let Err(error) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(ModError::new(
                "save-profile",
                MOD_PROFILE_FILE,
                format!("rename failed: {error}"),
            ));
        }
        Ok(())
    }
}

/// Deterministic, filesystem-safe directory slug for an active mod list.
///
/// - Returns `None` for vanilla (`active.is_empty()`).
/// - Uses mod IDs in load order, **never** versions, so updating a mod keeps
///   the player's saves in the same isolated directory.
/// - Single mod: `<id>` (already validated `[a-z0-9_-]`, 1..=64 chars; encodes
///   reserved Windows device names defensively as `~<id>` if unvalidated).
/// - Multiple mods: `<id1>+<id2>+...` when `<= 48` bytes; otherwise
///   `<first_prefix>~<count>mods~<16-hex-fnv1a64>`. Because neither `+` nor `~`
///   is allowed in mod IDs, normal single-mod (`[a-z0-9_-]+`), reserved
///   single-mod (`~<id>`), joined multi-mod (contains `+`, no `~`), and hashed
///   multi-mod (contains `~`, does not start with `~`) live in four
///   mathematically disjoint namespaces and cannot collide.
#[must_use]
pub fn mod_save_profile_slug(active: &[ModManifest]) -> Option<String> {
    match active {
        [] => None,
        [single] => {
            if crate::mods::is_windows_reserved_name(&single.id) {
                Some(format!("~{}", single.id))
            } else {
                Some(single.id.clone())
            }
        }
        many => {
            let joined = many
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>()
                .join("+");
            if joined.len() <= MAX_PLAIN_PROFILE_SLUG_LEN {
                return Some(joined);
            }
            let prefix_len = many[0].id.len().min(16);
            let prefix = &many[0].id[..prefix_len];
            let hash = fnv1a_64_ordered_ids(many);
            Some(format!("{prefix}~{}mods~{hash:016x}", many.len()))
        }
    }
}

fn fnv1a_64_ordered_ids(active: &[ModManifest]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for manifest in active {
        for &byte in manifest.id.as_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
        // NUL separator prevents boundary collisions like ["a-b","c"] vs ["a","b-c"].
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
