//! The presentation pack: `presentation/panels.json` and the art it names.
//!
//! The manifest lists the panel planes, palettes, temporary objects and
//! shopkeeper portrait the pack decoded; this module turns that file and the
//! PNGs beside it into the texture tables the layer draws from. It decides no
//! scene semantics: what is on screen is the runtime's op order.

use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;
use psiv_data::{DialogueSet, Role};
use serde::Deserialize;

use super::temporary_sprite_asset::TemporarySpriteAsset;

#[derive(Debug, Deserialize)]
struct PanelManifest {
    panels: Vec<PanelRecord>,
    #[serde(default)]
    palettes: Vec<PaletteRecord>,
    #[serde(default)]
    opening_background: Option<OpeningBackgroundRecord>,
    #[serde(default)]
    temporary_objects: Vec<TemporaryObjectRecord>,
    #[serde(default)]
    portraits: Vec<PortraitRecord>,
}

#[derive(Debug, Deserialize)]
struct OpeningBackgroundRecord {
    /// Oracle-measured top of the narration image band in the 320x224 frame.
    screen_y: f32,
}

#[derive(Debug, Deserialize)]
struct PanelRecord {
    id: u16,
    png: String,
}

#[derive(Debug, Deserialize)]
struct PaletteRecord {
    rom_addr: String,
    words: u16,
    raw_hex: String,
}

#[derive(Debug, Deserialize)]
struct TemporaryObjectRecord {
    object_id: u16,
    art_tile: u16,
    png: String,
    frame_width: i32,
    frame_height: i32,
    origin_x: i32,
    origin_y: i32,
    sequences: BTreeMap<String, AnimationRecord>,
    #[serde(default)]
    playback_sequence: Option<String>,
    #[serde(default)]
    playback_once: bool,
    #[serde(default)]
    load_art: Option<LoadArtRecord>,
}

#[derive(Debug, Deserialize)]
struct AnimationRecord {
    frames: Vec<AnimationFrameRecord>,
}

#[derive(Debug, Deserialize)]
struct AnimationFrameRecord {
    index: i32,
    duration_ticks: u32,
}

#[derive(Debug, Deserialize)]
struct LoadArtRecord {
    source_rom_addr: String,
    destination_tile: String,
}

#[derive(Debug, Deserialize)]
struct PortraitRecord {
    id: String,
    png: String,
}

/// Every table and image `presentation/panels.json` decodes to, in the shape
/// the layer stores them. `None` and an empty table mean "the pack has no such
/// record"; the caller keeps whatever it already had.
pub(super) struct SceneAssets {
    /// Oracle-measured top of the narration image band, when the manifest
    /// carries an opening background at all.
    pub(super) opening_screen_y: Option<f32>,
    pub(super) palettes: BTreeMap<u32, Vec<u16>>,
    pub(super) textures: BTreeMap<u16, Gd<ImageTexture>>,
    pub(super) temporary_assets: BTreeMap<(u16, u16), TemporarySpriteAsset>,
    pub(super) opening_background: Option<Gd<ImageTexture>>,
    pub(super) generic_portrait: Option<Gd<ImageTexture>>,
    /// `true` when the manifest named the generic portrait at all: the layer
    /// then takes the load result as its portrait whether or not it decoded.
    pub(super) generic_portrait_named: bool,
    pub(super) window_tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    pub(super) generic_window_rect: Option<Rect2>,
}

impl SceneAssets {
    /// Loads the manifest and every pack image the layer draws.
    pub(super) fn load(pack_dir: &str, set: &DialogueSet) -> SceneAssets {
        let mut assets = SceneAssets {
            opening_screen_y: None,
            palettes: BTreeMap::new(),
            textures: BTreeMap::new(),
            temporary_assets: BTreeMap::new(),
            opening_background: None,
            generic_portrait: None,
            generic_portrait_named: false,
            window_tiles: BTreeMap::new(),
            generic_window_rect: None,
        };
        let manifest_path = Path::new(pack_dir).join("presentation/panels.json");
        match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PanelManifest>(&raw).ok())
        {
            Some(manifest) => {
                if let Some(opening) = &manifest.opening_background {
                    assets.opening_screen_y = Some(opening.screen_y);
                }
                for palette in &manifest.palettes {
                    let Ok(address) =
                        u32::from_str_radix(palette.rom_addr.trim_start_matches("0x"), 16)
                    else {
                        godot_error!("scene palette address is invalid: {}", palette.rom_addr);
                        continue;
                    };
                    let bytes = palette.raw_hex.as_bytes();
                    let words: Vec<u16> = bytes
                        .chunks_exact(4)
                        .filter_map(|chunk| {
                            let word = std::str::from_utf8(chunk).ok()?;
                            u16::from_str_radix(word, 16).ok()
                        })
                        .collect();
                    if words.len() != usize::from(palette.words) {
                        godot_error!(
                            "scene palette {address:#08x} decoded {} words, expected {}",
                            words.len(),
                            palette.words
                        );
                    }
                    assets.palettes.insert(address, words);
                }
                for panel in manifest.panels {
                    let path = format!("{pack_dir}/{}", panel.png);
                    if let Some(image) = Image::load_from_file(&GString::from(path.as_str()))
                        && let Some(texture) = ImageTexture::create_from_image(&image)
                    {
                        assets.textures.insert(panel.id, texture);
                    } else {
                        godot_error!("scene panel {} failed to load: {path}", panel.id);
                    }
                }
                for temporary in manifest.temporary_objects {
                    let path = format!("{pack_dir}/{}", temporary.png);
                    let Some(image) = Image::load_from_file(&GString::from(path.as_str())) else {
                        godot_error!(
                            "scene temporary object {:#06x}/{:#05x} failed to load: {path}",
                            temporary.object_id,
                            temporary.art_tile
                        );
                        continue;
                    };
                    let Some(texture) = ImageTexture::create_from_image(&image) else {
                        godot_error!("scene temporary object texture failed: {path}");
                        continue;
                    };
                    let sequences = temporary
                        .sequences
                        .into_iter()
                        .map(|(name, sequence)| {
                            let frames: Vec<(i32, u32)> = sequence
                                .frames
                                .into_iter()
                                .map(|frame| (frame.index, frame.duration_ticks))
                                .collect();
                            let total: u32 = frames.iter().map(|(_, duration)| *duration).sum();
                            (name, (frames, total.max(1)))
                        })
                        .collect();
                    let load_art = temporary.load_art.and_then(|load| {
                        let source = parse_hex(&load.source_rom_addr)?;
                        let tile = parse_hex(&load.destination_tile)? as u16;
                        Some((source, tile))
                    });
                    assets.temporary_assets.insert(
                        (temporary.object_id, temporary.art_tile),
                        TemporarySpriteAsset {
                            texture,
                            frame_width: temporary.frame_width,
                            frame_height: temporary.frame_height,
                            origin_x: temporary.origin_x,
                            origin_y: temporary.origin_y,
                            sequences,
                            load_art,
                            playback_sequence: temporary.playback_sequence,
                            playback_once: temporary.playback_once,
                        },
                    );
                }
                for portrait in manifest.portraits {
                    if portrait.id == "shopkeeper_2" {
                        let path = format!("{pack_dir}/{}", portrait.png);
                        assets.generic_portrait_named = true;
                        assets.generic_portrait =
                            Image::load_from_file(&GString::from(path.as_str()))
                                .and_then(|image| ImageTexture::create_from_image(&image));
                    }
                }
            }
            None => godot_warn!("scene panel manifest missing: {}", manifest_path.display()),
        }

        let background = format!("{pack_dir}/presentation/opening_background.png");
        assets.opening_background = Image::load_from_file(&GString::from(background.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        if assets.opening_background.is_none() {
            godot_error!("opening background failed to load: {background}");
        }

        let window_path = format!("{pack_dir}/{}", set.window.png);
        if let Some(window) = Image::load_from_file(&GString::from(window_path.as_str())) {
            for role in Role::ALL {
                let Some(tile) = set.window.role(role) else {
                    continue;
                };
                let Some(mut image) = window.get_region(Rect2i::new(
                    Vector2i::new(tile.x, tile.y),
                    Vector2i::new(tile.width as i32, tile.height as i32),
                )) else {
                    continue;
                };
                if tile.flip_h {
                    image.flip_x();
                }
                if tile.flip_v {
                    image.flip_y();
                }
                if let Some(texture) = ImageTexture::create_from_image(&image) {
                    assets.window_tiles.insert(role.as_str(), texture);
                }
            }
            let rect = set.window.text_window.rect;
            assets.generic_window_rect = Some(Rect2::new(
                Vector2::new(rect.x as f32, rect.y as f32),
                Vector2::new(rect.width as f32, rect.height as f32),
            ));
        } else {
            godot_error!("generic scene window failed to load: {window_path}");
        }

        assets
    }
}

fn parse_hex(value: &str) -> Option<u32> {
    u32::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}
