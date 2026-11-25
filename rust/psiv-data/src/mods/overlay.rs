//! In-memory pack overlay layer: resolves patched JSON tables and mod assets
//! without mirroring `runtime-pack/` into a staging folder (#791).
//!
//! [`PackOverlay`] owns the read-only view over a base runtime pack plus zero
//! or more active mods in load order:
//!
//! - Patched JSON tables (and reconciled live-palette map metadata) live in
//!   memory and are consumed directly by [`GameData::load_overlay`],
//!   [`BattleFiles::load_overlay`], and the rest of `psiv-data`'s loaders.
//! - Replacement assets resolve directly from active mods' `assets/` folders in
//!   reverse load order (latest mod wins) before falling back to `base_pack`.
//! - Neither `base_pack` nor any staging directory under `mods/` is ever
//!   written or mutated.

use super::{
    DiscoveredMod, MOD_ASSETS_DIR, MOD_PATCHES_DIR, ModError, ModManifest, collect_relative_files,
    map_art, normalize_rel,
    patch::{self, AddedRecordOwners, TouchedRecords},
    reject_duplicate_mod_ids,
};
use crate::error::DataError;
use crate::{BattleFiles, ContentsType, GameData, MapId};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// How one pack-relative file is supplied by an active mod layer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileOverride {
    /// JSON table patched in memory (or reconciled for live-palette map art).
    Memory(String),
    /// Replacement asset residing directly inside the winning mod's `assets/`.
    ModAsset(PathBuf),
}

/// Validated pack records loaded alongside a [`PackOverlay`].
///
/// Returned by [`PackOverlay::load_validated`] so callers such as the Godot
/// shell can reuse the already-validated records instead of parsing them twice.
#[derive(Debug, Clone)]
pub struct ValidatedPack {
    /// Validated [`GameData`] loaded from the overlay.
    pub game_data: GameData,
    /// Validated [`BattleFiles`] when `battle/enemies.json` is present.
    pub battle_files: Option<BattleFiles>,
    /// Validated [`crate::battle::CommandUiArt`] when present.
    pub command_ui_art: Option<crate::battle::CommandUiArt>,
    /// Validated [`crate::battle::DamageDigitArt`] when present.
    pub damage_digit_art: Option<crate::battle::DamageDigitArt>,
}

/// Read-only overlaid view of a base runtime pack plus zero or more active mods.
///
/// Patched JSON tables are merged in memory and replacement assets resolve
/// directly from active mods' `assets/` directories (latest mod in load order
/// wins) before falling back to `base_pack`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackOverlay {
    base_pack: PathBuf,
    active: Vec<ModManifest>,
    patched_files: Vec<String>,
    replaced_assets: Vec<String>,
    overrides: BTreeMap<String, FileOverride>,
    provenance: BTreeMap<String, String>,
    asset_layers: BTreeMap<String, usize>,
    added_record_owners: AddedRecordOwners,
    touched_records: TouchedRecords,
}

impl PackOverlay {
    /// Constructs an unmodified view backed directly by `base_pack`.
    #[must_use]
    pub fn base(base_pack: impl Into<PathBuf>) -> Self {
        Self {
            base_pack: base_pack.into(),
            active: Vec::new(),
            patched_files: Vec::new(),
            replaced_assets: Vec::new(),
            overrides: BTreeMap::new(),
            provenance: BTreeMap::new(),
            asset_layers: BTreeMap::new(),
            added_record_owners: AddedRecordOwners::new(),
            touched_records: TouchedRecords::new(),
        }
    }

    /// Layers `mods` in order over `base_pack` in memory and runs the full
    /// fail-closed schema validation (`GameData`, `BattleFiles`, command UI
    /// art, damage digit art).
    ///
    /// When `mods` is empty, returns [`PackOverlay::base`] without reading
    /// `base_pack`. Never mutates `base_pack` and never writes to disk.
    pub fn load(base_pack: &Path, mods: &[DiscoveredMod]) -> Result<Self, ModError> {
        if mods.is_empty() {
            return Ok(Self::base(base_pack));
        }
        Self::load_validated(base_pack, mods).map(|(overlay, _)| overlay)
    }

    /// Layers `mods` in order over `base_pack` in memory, runs full fail-closed
    /// schema validation, and returns both the overlay and the loaded records.
    pub fn load_validated(
        base_pack: &Path,
        mods: &[DiscoveredMod],
    ) -> Result<(Self, ValidatedPack), ModError> {
        let overlay = Self::build_in_memory(base_pack, mods)?;
        let validated = overlay.validate()?;
        Ok((overlay, validated))
    }

    /// Builds the in-memory overlay (patches, asset lookup map, and live-palette
    /// map art reconciliation) without running the full `GameData` loader.
    pub(crate) fn build_in_memory(
        base_pack: &Path,
        mods: &[DiscoveredMod],
    ) -> Result<Self, ModError> {
        if mods.is_empty() {
            return Ok(Self::base(base_pack));
        }
        reject_duplicate_mod_ids(mods)?;

        let mut overlay = Self {
            base_pack: base_pack.to_path_buf(),
            active: mods.iter().map(|m| m.manifest.clone()).collect(),
            patched_files: Vec::new(),
            replaced_assets: Vec::new(),
            overrides: BTreeMap::new(),
            provenance: BTreeMap::new(),
            asset_layers: BTreeMap::new(),
            added_record_owners: AddedRecordOwners::new(),
            touched_records: TouchedRecords::new(),
        };
        let mut patched_set: BTreeMap<String, ()> = BTreeMap::new();

        for (mod_idx, m) in mods.iter().enumerate() {
            let mod_id = &m.manifest.id;
            let patches_root = m.root.join(MOD_PATCHES_DIR);
            if patches_root.is_dir() {
                let files = collect_relative_files(&patches_root, mod_id, MOD_PATCHES_DIR)?;
                for rel in files {
                    let rel_str = normalize_rel(&rel, mod_id, MOD_PATCHES_DIR)?;
                    if rel_str == "manifest.json" {
                        return Err(ModError::new(
                            mod_id,
                            format!("{MOD_PATCHES_DIR}/{rel_str}"),
                            "pack manifest.json cannot be patched by a mod",
                        ));
                    }
                    if !rel_str.ends_with(".json") {
                        return Err(ModError::new(
                            mod_id,
                            format!("{MOD_PATCHES_DIR}/{rel_str}"),
                            "patch files must have a .json extension",
                        ));
                    }
                    if !overlay.is_file(&rel_str) {
                        return Err(ModError::new(
                            mod_id,
                            format!("{MOD_PATCHES_DIR}/{rel_str}"),
                            format!("base pack has no `{rel_str}` to patch"),
                        ));
                    }
                    let base_text = overlay.read_text(&rel_str).map_err(|error| {
                        ModError::new(
                            mod_id,
                            &rel_str,
                            format!("could not read base file: {error}"),
                        )
                    })?;
                    let mut base_json: serde_json::Value = serde_json::from_str(&base_text)
                        .map_err(|error| {
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
                    patch::apply_json_patch_tracked(
                        &mut base_json,
                        &patch_json,
                        &rel_str,
                        mod_id,
                        &mut overlay.added_record_owners,
                        &mut overlay.touched_records,
                    )
                    .map_err(|detail| {
                        ModError::new(mod_id, format!("{MOD_PATCHES_DIR}/{rel_str}"), detail)
                    })?;
                    let rendered = serde_json::to_string(&base_json).map_err(|error| {
                        ModError::new(
                            mod_id,
                            &rel_str,
                            format!("could not serialize JSON: {error}"),
                        )
                    })?;
                    overlay
                        .overrides
                        .insert(rel_str.clone(), FileOverride::Memory(rendered));
                    overlay.provenance.insert(rel_str.clone(), mod_id.clone());
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
                    let src = assets_root.join(&rel);
                    overlay
                        .overrides
                        .insert(rel_str.clone(), FileOverride::ModAsset(src));
                    overlay.provenance.insert(rel_str.clone(), mod_id.clone());
                    overlay.asset_layers.insert(rel_str, mod_idx);
                }
            }
        }

        overlay.patched_files = patched_set.into_keys().collect();
        overlay.replaced_assets = overlay.asset_layers.keys().cloned().collect();
        map_art::reconcile_replaced_map_update_images(&mut overlay)?;
        Ok(overlay)
    }

    /// Runs the full fail-closed schema loaders and cross-table reference
    /// checks against this overlay and attributes any defect to the offending
    /// mod, file, and location.
    pub fn validate(&self) -> Result<ValidatedPack, ModError> {
        let game_data =
            GameData::load_overlay(self).map_err(|error| self.attribute_data_error(&error))?;
        let battle_files = if self.is_file("battle/enemies.json") {
            Some(
                BattleFiles::load_overlay(self)
                    .map_err(|error| self.attribute_data_error(&error))?,
            )
        } else {
            None
        };
        self.validate_cross_pack_refs(&game_data, battle_files.as_ref())
            .map_err(|error| self.attribute_data_error_with_maps(&error, Some(&game_data)))?;
        let command_ui_art = if self.is_file(crate::battle::COMMAND_UI_ART_PATH) {
            Some(
                crate::battle::CommandUiArt::load_overlay(self)
                    .map_err(|error| self.attribute_data_error(&error))?,
            )
        } else {
            None
        };
        let damage_digit_art = if self.is_file(crate::battle::DAMAGE_DIGIT_ART_PATH) {
            Some(
                crate::battle::DamageDigitArt::load_overlay(self)
                    .map_err(|error| self.attribute_data_error(&error))?,
            )
        } else {
            None
        };
        Ok(ValidatedPack {
            game_data,
            battle_files,
            command_ui_art,
            damage_digit_art,
        })
    }

    /// Validates cross-pack references spanning `maps/*.json`,
    /// `battle/equipment.json`, and `dialogue/trees.json`.
    fn validate_cross_pack_refs(
        &self,
        game_data: &GameData,
        battle_files: Option<&BattleFiles>,
    ) -> Result<(), DataError> {
        if let Some(battle) = battle_files {
            let valid_items: BTreeSet<u16> = battle
                .equipment
                .items
                .iter()
                .map(|item| u16::from(item.id))
                .collect();
            for (map_id, record) in game_data.maps() {
                for (pos, chest) in record.treasure_chests.iter().enumerate() {
                    if chest.contents_type == ContentsType::Item
                        && let Some(item_id) = chest.item_id
                        && !valid_items.contains(&item_id)
                    {
                        return Err(DataError::validation(
                            map_id,
                            format!("treasure_chests[{pos}].item_id"),
                            format!(
                                "treasure chest index {} references unknown item_id {item_id} (not in battle/equipment.json)",
                                chest.index
                            ),
                        ));
                    }
                }
            }
        }

        // Strict check for any mod-added or mod-edited NPC dialogue reference
        // (even on retail MileHouse maps).
        if let Some(dialogue) = game_data.dialogue() {
            for (map_id, record) in game_data.maps() {
                let Some(rel_file) = game_data
                    .manifest()
                    .entry(map_id)
                    .map(|entry| entry.json.as_str())
                else {
                    continue;
                };
                for (pos, npc) in record.npcs.iter().enumerate() {
                    let key = (
                        rel_file.to_owned(),
                        "npcs".to_owned(),
                        npc.index.to_string(),
                    );
                    if !self.touched_records.contains(&key) {
                        continue;
                    }
                    if dialogue
                        .entry(record.dialogue_tree, npc.dialogue_id)
                        .is_none()
                    {
                        let count = dialogue
                            .tree(record.dialogue_tree)
                            .map_or(0, |t| t.entries.len());
                        return Err(DataError::validation(
                            map_id,
                            format!("npcs[{pos}].dialogue_id"),
                            format!(
                                "npc index {} references dialogue_id {} in tree {}, which has {count} entries",
                                npc.index, npc.dialogue_id, record.dialogue_tree
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// The unmodified base pack directory.
    #[must_use]
    pub fn base_pack(&self) -> &Path {
        &self.base_pack
    }

    /// Active mods in exact load order (empty for an unmodified pack).
    #[must_use]
    pub fn active_mods(&self) -> &[ModManifest] {
        &self.active
    }

    /// Whether one or more mods are active on this overlay.
    #[must_use]
    pub fn is_modded(&self) -> bool {
        !self.active.is_empty()
    }

    /// Pack-relative JSON paths modified by active mods' `patches/`.
    #[must_use]
    pub fn patched_files(&self) -> &[String] {
        &self.patched_files
    }

    /// Pack-relative asset paths supplied by active mods' `assets/`.
    #[must_use]
    pub fn replaced_assets(&self) -> &[String] {
        &self.replaced_assets
    }

    /// Winning mod load-order index (0-based) that supplied `rel_path` via
    /// `assets/`, or `None` when `rel_path` comes from the base pack.
    #[must_use]
    pub fn asset_layer(&self, rel_path: &str) -> Option<usize> {
        let norm = normalize_pack_rel_path(rel_path)?;
        self.asset_layers.get(&norm).copied()
    }

    /// Whether a live-palette `map_update_images` redirect `image_rel -> index_rel`
    /// is stale because a mod replaced `image_rel` at a strictly newer layer than
    /// `index_rel` (or without replacing `index_rel` at all).
    #[must_use]
    pub fn map_redirect_is_stale(&self, image_rel: &str, index_rel: &str) -> bool {
        map_art::redirect_is_stale(&self.asset_layers, image_rel, index_rel)
    }

    /// Resolves a pack-relative disk asset path, checking active mods' `assets/`
    /// in reverse load order (latest mod wins) before falling back to `base_pack`.
    ///
    /// Returns `None` if `rel_path` is empty, absolute, contains `..` traversal,
    /// or names a JSON file overridden purely in memory (read those via
    /// [`Self::read_text`] or [`Self::read_asset_bytes`]).
    #[must_use]
    pub fn resolve_asset(&self, rel_path: &str) -> Option<PathBuf> {
        let norm = normalize_pack_rel_path(rel_path)?;
        match self.overrides.get(&norm) {
            Some(FileOverride::ModAsset(path)) => Some(path.clone()),
            Some(FileOverride::Memory(_)) => None,
            None => Some(self.base_pack.join(&norm)),
        }
    }

    /// Whether `rel_path` exists in the overlaid pack (either as an in-memory
    /// patched file, a mod asset, or a base pack file).
    #[must_use]
    pub fn is_file(&self, rel_path: &str) -> bool {
        let Some(norm) = normalize_pack_rel_path(rel_path) else {
            return false;
        };
        match self.overrides.get(&norm) {
            Some(FileOverride::Memory(_)) => true,
            Some(FileOverride::ModAsset(path)) => path.is_file(),
            None => self.base_pack.join(&norm).is_file(),
        }
    }

    /// Reads raw bytes for `rel_path` from the overlay (in-memory patched JSON,
    /// winning mod asset, or base pack fallback).
    pub fn read_asset_bytes(&self, rel_path: &str) -> io::Result<Vec<u8>> {
        let norm = normalize_pack_rel_path(rel_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid pack-relative path: {rel_path:?}"),
            )
        })?;
        match self.overrides.get(&norm) {
            Some(FileOverride::Memory(text)) => Ok(text.as_bytes().to_vec()),
            Some(FileOverride::ModAsset(path)) => fs::read(path),
            None => fs::read(self.base_pack.join(&norm)),
        }
    }

    /// Reads UTF-8 text for `rel_path` from the overlay (in-memory patched JSON,
    /// winning mod asset, or base pack fallback).
    pub fn read_text(&self, rel_path: &str) -> io::Result<String> {
        let norm = normalize_pack_rel_path(rel_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid pack-relative path: {rel_path:?}"),
            )
        })?;
        match self.overrides.get(&norm) {
            Some(FileOverride::Memory(text)) => Ok(text.clone()),
            Some(FileOverride::ModAsset(path)) => fs::read_to_string(path),
            None => fs::read_to_string(self.base_pack.join(&norm)),
        }
    }

    pub(crate) fn asset_layers(&self) -> &BTreeMap<String, usize> {
        &self.asset_layers
    }

    pub(crate) fn override_keys(&self) -> impl Iterator<Item = &str> {
        self.overrides.keys().map(String::as_str)
    }

    pub(crate) fn insert_memory_override(&mut self, rel_path: String, text: String) {
        self.overrides.insert(rel_path, FileOverride::Memory(text));
    }

    /// Logical path reported in [`DataError`]s (`base_pack.join(rel_path)`).
    pub(crate) fn logical_path(&self, rel_path: &str) -> PathBuf {
        self.base_pack.join(rel_path)
    }

    pub(crate) fn read_text_data(&self, rel_path: &str) -> Result<String, DataError> {
        let path = self.logical_path(rel_path);
        self.read_text(rel_path)
            .map_err(|error| DataError::io(&path, error))
    }

    pub(crate) fn read_bytes_data(&self, rel_path: &str) -> Result<Vec<u8>, DataError> {
        let path = self.logical_path(rel_path);
        self.read_asset_bytes(rel_path)
            .map_err(|error| DataError::io(&path, error))
    }

    pub(crate) fn read_json_data<T: serde::de::DeserializeOwned>(
        &self,
        rel_path: &str,
    ) -> Result<T, DataError> {
        let path = self.logical_path(rel_path);
        let text = self
            .read_text(rel_path)
            .map_err(|error| DataError::io(&path, error))?;
        serde_json::from_str(&text).map_err(|error| DataError::json(&path, error))
    }

    fn attribute_data_error(&self, error: &DataError) -> ModError {
        self.attribute_data_error_with_maps(error, None)
    }

    fn attribute_data_error_with_maps(
        &self,
        error: &DataError,
        game_data: Option<&GameData>,
    ) -> ModError {
        let message = error.to_string();
        let rel_file = extract_error_file(error, &self.base_pack)
            .or_else(|| self.resolve_map_or_sprite_error_file(error, game_data))
            .unwrap_or_else(|| {
                self.provenance
                    .keys()
                    .find(|rel| message.contains(rel.as_str()))
                    .cloned()
                    .unwrap_or_else(|| "pack".to_owned())
            });
        let mod_id = self
            .provenance
            .get(&rel_file)
            .cloned()
            .or_else(|| {
                self.provenance
                    .iter()
                    .find(|(rel, _)| message.contains(rel.as_str()))
                    .map(|(_, id)| id.clone())
            })
            .unwrap_or_else(|| {
                self.active
                    .last()
                    .map(|m| m.id.clone())
                    .unwrap_or_else(|| "unknown".to_owned())
            });
        ModError::new(mod_id, rel_file, message)
    }

    fn resolve_map_or_sprite_error_file(
        &self,
        error: &DataError,
        game_data: Option<&GameData>,
    ) -> Option<String> {
        let map_id = match error {
            DataError::Validation { map, .. } => Some(*map),
            DataError::Sprite { who, .. } => parse_map_id_from_who(who),
            _ => None,
        };
        if let Some(map_id) = map_id {
            if let Some(gd) = game_data
                && let Some(entry) = gd.manifest().entry(map_id)
            {
                return Some(entry.json.clone());
            }
            if let Ok(manifest) = self.read_json_data::<crate::Manifest>("manifest.json")
                && let Some(entry) = manifest.entry(map_id)
            {
                return Some(entry.json.clone());
            }
        }
        None
    }
}

fn parse_map_id_from_who(who: &str) -> Option<MapId> {
    let rest = who.strip_prefix("map ")?;
    let token = rest.split_whitespace().next()?;
    let hex = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))?;
    u16::from_str_radix(hex, 16).ok().map(MapId)
}

/// Normalizes a pack-relative path to forward-slash form, rejecting empty,
/// absolute, drive-prefixed, and `..` traversal paths.
pub(crate) fn normalize_pack_rel_path(rel_path: &str) -> Option<String> {
    if rel_path.is_empty() || rel_path.starts_with('/') || rel_path.starts_with('\\') {
        return None;
    }
    if rel_path.as_bytes().get(1) == Some(&b':') && rel_path.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }
    let mut parts = Vec::new();
    for segment in rel_path.split(['/', '\\']) {
        match segment {
            "" | "." => continue,
            ".." => return None,
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

fn extract_error_file(error: &DataError, base_pack: &Path) -> Option<String> {
    let strip = |path: &Path| -> String {
        path.strip_prefix(base_pack)
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
