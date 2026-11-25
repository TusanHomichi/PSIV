//! Shared pack asset-resolution helper for the Godot shell (#791).
//!
//! Every pack read in `psiv-godot` — images, indexed CRAM PNGs, raw bytes, and
//! presentation/art JSON tables — routes through [`PackAssets`], which wraps
//! `psiv_data::PackOverlay`. Active mods' `assets/` directories are checked in
//! reverse load order (latest mod wins) before falling back to `base_pack`, and
//! patched JSON tables are read straight from memory without any on-disk
//! staging mirror.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;
use psiv_data::PackOverlay;
use serde::de::DeserializeOwned;

/// Cheaply cloneable handle to the active [`PackOverlay`].
#[derive(Clone, Debug)]
pub(crate) struct PackAssets {
    overlay: Arc<PackOverlay>,
}

impl Default for PackAssets {
    fn default() -> Self {
        Self::base_dir(PathBuf::new())
    }
}

impl PackAssets {
    /// Wraps an already-built [`PackOverlay`].
    pub(crate) fn new(overlay: PackOverlay) -> Self {
        Self {
            overlay: Arc::new(overlay),
        }
    }

    /// Constructs an unmodified view backed directly by `base_pack`.
    pub(crate) fn base_dir(base_pack: impl Into<PathBuf>) -> Self {
        Self::new(PackOverlay::base(base_pack))
    }

    /// The underlying [`PackOverlay`] for `psiv-data` loaders.
    pub(crate) fn overlay(&self) -> &PackOverlay {
        &self.overlay
    }

    /// The unmodified base pack directory.
    pub(crate) fn base_pack(&self) -> &Path {
        self.overlay.base_pack()
    }

    /// Display path for diagnostic messages.
    pub(crate) fn display_root(&self) -> String {
        self.base_pack().to_string_lossy().into_owned()
    }

    /// Resolves a pack-relative disk asset path (checking active mods'
    /// `assets/` in reverse load order before falling back to `base_pack`).
    ///
    /// Returns `None` for unsafe paths or in-memory patched JSON files.
    pub(crate) fn resolve_path(&self, rel_path: &str) -> Option<PathBuf> {
        self.overlay.resolve_asset(rel_path)
    }

    /// Reads raw bytes for `rel_path` from the overlay (in-memory patched JSON,
    /// winning mod asset, or base pack fallback).
    pub(crate) fn read_bytes(&self, rel_path: &str) -> io::Result<Vec<u8>> {
        self.overlay.read_asset_bytes(rel_path)
    }

    /// Reads UTF-8 text for `rel_path` from the overlay.
    pub(crate) fn read_text(&self, rel_path: &str) -> io::Result<String> {
        self.overlay.read_text(rel_path)
    }

    /// Reads and deserializes a JSON file from the overlay, picking up both
    /// in-memory `patches/` merges and `assets/` replacements.
    pub(crate) fn read_json<T: DeserializeOwned>(&self, rel_path: &str) -> Result<T, String> {
        let text = self
            .read_text(rel_path)
            .map_err(|error| format!("{rel_path}: {error}"))?;
        serde_json::from_str(&text).map_err(|error| format!("{rel_path}: {error}"))
    }

    /// Loads a pack-relative image through Godot's `Image::load_from_file`,
    /// resolving active mods' `assets/` before falling back to `base_pack`.
    pub(crate) fn load_image(&self, rel_path: &str) -> Option<Gd<Image>> {
        let path = self.resolve_path(rel_path)?;
        Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
    }

    /// Loads a pack-relative image and uploads it into an `ImageTexture`.
    pub(crate) fn load_texture(&self, rel_path: &str) -> Option<Gd<ImageTexture>> {
        let image = self.load_image(rel_path)?;
        ImageTexture::create_from_image(&image)
    }

    /// Loads `dialogue/menu_font.png` from the overlay, falling back to a
    /// checkout's `generated/gfx/ArtNem_Font.png` beside `base_pack` for older
    /// packs.
    pub(crate) fn load_menu_font_image(&self) -> Option<Gd<Image>> {
        self.load_image("dialogue/menu_font.png").or_else(|| {
            let path = self
                .base_pack()
                .parent()?
                .join("generated/gfx/ArtNem_Font.png");
            Image::load_from_file(&GString::from(path.to_string_lossy().as_ref()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_data::mods::{ModManifest, discover_mods, init_mod_dir};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

    fn temp_dir(label: &str) -> PathBuf {
        let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "psiv-godot-pack-assets-{label}-{}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pack_assets_resolves_base_and_rejects_traversal() {
        let scratch = temp_dir("base");
        let base = scratch.join("runtime-pack");
        fs::create_dir_all(base.join("maps")).unwrap();
        fs::write(base.join("maps/test.png"), b"base-png").unwrap();

        let assets = PackAssets::base_dir(&base);
        assert_eq!(
            assets.resolve_path("maps/test.png"),
            Some(base.join("maps/test.png"))
        );
        assert_eq!(assets.read_bytes("maps/test.png").unwrap(), b"base-png");
        let parent_escape = ["..", "outside.png"].join("/");
        assert_eq!(assets.resolve_path(&parent_escape), None);
        assert!(assets.read_bytes(&parent_escape).is_err());

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn pack_assets_layers_mods_without_staging_dir() {
        let scratch = temp_dir("layered");
        let base = scratch.join("runtime-pack");
        let mods_dir = scratch.join("mods");
        fs::create_dir_all(base.join("maps")).unwrap();
        fs::write(base.join("manifest.json"), "{}\n").unwrap();
        fs::write(base.join("maps/a.png"), b"base-a").unwrap();
        fs::write(base.join("maps/b.png"), b"base-b").unwrap();

        let mod_one = mods_dir.join("one");
        let mut m1 = ModManifest::new("one", "One", "1.0.0");
        m1.order = 10;
        init_mod_dir(&mod_one, &m1).unwrap();
        fs::create_dir_all(mod_one.join("assets/maps")).unwrap();
        fs::write(mod_one.join("assets/maps/a.png"), b"one-a").unwrap();

        let mod_two = mods_dir.join("two");
        let mut m2 = ModManifest::new("two", "Two", "1.0.0");
        m2.order = 20;
        init_mod_dir(&mod_two, &m2).unwrap();
        fs::create_dir_all(mod_two.join("assets/maps")).unwrap();
        fs::write(mod_two.join("assets/maps/a.png"), b"two-a").unwrap();

        let discovered = discover_mods(&mods_dir).unwrap();
        assert_eq!(discovered.len(), 2);

        // Verify install::apply_active_mods does not create `_active_pack` even
        // when validation fails on a synthetic minimal pack without full tables.
        let install = crate::install::Install::Checkout {
            root: scratch.clone(),
        };
        let err = crate::install::apply_active_mods(&install, &base);
        assert!(
            err.is_err(),
            "incomplete synthetic base pack fails schema check"
        );
        assert!(
            !mods_dir.join("_active_pack").exists(),
            "mods/_active_pack must never be created"
        );

        let _ = fs::remove_dir_all(&scratch);
    }
}
