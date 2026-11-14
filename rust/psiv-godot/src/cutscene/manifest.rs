//! The presentation pack: `presentation/panels.json` and the art it names.
//!
//! The manifest lists the panel planes, temporary objects and
//! shopkeeper portrait the pack decoded; this module turns that file and the
//! PNGs beside it into the texture tables the layer draws from. It decides no
//! scene semantics: what is on screen is the runtime's op order.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;
use psiv_data::{DialogueSet, Role};
use serde::Deserialize;

use super::temporary_sprite_asset::TemporarySpriteAsset;
use crate::cram_art::IndexedArt;
use psiv_data::color::IndexLayout;

#[derive(Debug, Deserialize)]
struct PanelManifest {
    panels: Vec<PanelRecord>,
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
    /// Layer A's art tile; its palette select is the line A draws from.
    #[serde(default = "no_layer")]
    art_tile_a: u16,
    /// Layer B's art tile, `$FFFF` when the panel has one layer.
    #[serde(default = "no_layer")]
    art_tile_b: u16,
}

const fn no_layer() -> u16 {
    0xFFFF
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
    #[serde(default)]
    palette: Option<SheetPaletteRecord>,
}

/// A sheet's `palette` block (`psiv_tools/sprites/emit.py`, `sheet_json`).
#[derive(Debug, Deserialize)]
struct SheetPaletteRecord {
    /// The CRAM line the object's pixels draw from: the line its sprite
    /// pieces select when the scene builds it.
    #[serde(default)]
    cram_line: Option<u8>,
}

impl TemporaryObjectRecord {
    /// How the sheet's palette indices reach the CRAM: its pack record's
    /// line, or `None` (baked colours only) for a record that names none.
    fn layout(&self) -> Option<IndexLayout> {
        let line = self.palette.as_ref()?.cram_line?;
        (line < 4).then_some(IndexLayout::Line(line))
    }
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
    pub(super) textures: BTreeMap<u16, Gd<ImageTexture>>,
    /// Each panel's PNG and the lines its layers draw from, for the live
    /// palette (`cram_art`); loaded on the panel's first draw.
    pub(super) indexed_panels: BTreeMap<u16, (String, IndexLayout)>,
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
            textures: BTreeMap::new(),
            indexed_panels: BTreeMap::new(),
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
                for panel in manifest.panels {
                    let path = format!("{pack_dir}/{}", panel.png);
                    let layout = IndexLayout::Panel {
                        a: IndexLayout::art_tile_line(panel.art_tile_a),
                        b: IndexLayout::art_tile_line(panel.art_tile_b),
                    };
                    assets
                        .indexed_panels
                        .insert(panel.id, (path.clone(), layout));
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
                    // The sheet's indices on its line, for the live palette.
                    let indexed = temporary.layout().and_then(|layout| {
                        let art = IndexedArt::load(&path, layout)?;
                        Some(Rc::new(RefCell::new(art)))
                    });
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
                            indexed,
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
            None => {
                godot_warn!("scene panel manifest missing: {}", manifest_path.display());
            }
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

#[cfg(test)]
mod tests {
    use super::{PanelManifest, TemporaryObjectRecord};
    use crate::cram_art::{CramState, IndexedPixels};
    use psiv_data::color::IndexLayout;

    fn record(palette: &str) -> TemporaryObjectRecord {
        serde_json::from_str(&format!(
            r#"{{"object_id": 392, "art_tile": 933, "png": "t.png",
                "frame_width": 32, "frame_height": 32, "origin_x": 16,
                "origin_y": 32, "sequences": {{}} {palette}}}"#
        ))
        .expect("record parses")
    }

    #[test]
    fn a_temporary_object_draws_on_its_record_line() {
        let line = |n| record(&format!(r#", "palette": {{"cram_line": {n}}}"#)).layout();
        assert_eq!(line(3), Some(IndexLayout::Line(3)));
        assert_eq!(line(0), Some(IndexLayout::Line(0)));
        // No line, or one the VDP has no line for: baked colours only.
        assert_eq!(line(4), None);
        assert_eq!(record("").layout(), None);
        assert_eq!(record(r#", "palette": {}"#).layout(), None);
    }

    /// What goes wrong when `record` draws a two-colour object whose colours
    /// are words 1 and 2 of line 3: its first draw, a CRAM revision changing
    /// word 1 of line 3, then one changing only line 2.
    fn temporary_recolor_failures(record: &TemporaryObjectRecord) -> Vec<&'static str> {
        let Some(layout) = record.layout() else {
            return vec!["no line"];
        };
        // Pixels: index 1, transparent index 0, index 2, and index 1 whose
        // alpha is zero (a hole in a partial sheet).
        let mut pixels = IndexedPixels::new(layout, vec![1, 0, 2, 1], vec![255, 255, 255, 0]);
        let mut cram = CramState {
            words: [0x0222; 64],
            revision: 1,
            live: true,
        };
        cram.words[49] = 0x000E;
        cram.words[50] = 0x00E0;
        let mut failures = Vec::new();
        let first = pixels.recolor(&cram).unwrap_or_default();
        if first[..12] != [238, 0, 0, 255, 0, 0, 0, 0, 0, 238, 0, 255] {
            failures.push("pixels must read line 3");
        }
        if first.get(12..16) != Some(&[0; 4][..]) {
            failures.push("a transparent pixel drew");
        }
        if pixels.recolor(&cram).is_some() {
            failures.push("redrew an unchanged revision");
        }
        cram.words[49] = 0x0E00;
        cram.revision = 2;
        let changed = pixels.recolor(&cram).unwrap_or_default();
        if changed.get(..4) != Some(&[0, 0, 238, 255][..]) {
            failures.push("a CRAM revision did not recolour its line");
        }
        if changed.get(4..8) != Some(&[0; 4][..]) || changed.get(12..16) != Some(&[0; 4][..]) {
            failures.push("a transparent pixel drew");
        }
        cram.words[33] = 0x0EEE;
        cram.words[34] = 0x0EEE;
        cram.revision = 3;
        if pixels.recolor(&cram) != Some(changed) {
            failures.push("another line changed the object");
        }
        failures
    }

    #[test]
    fn a_temporary_object_recolours_only_its_line_and_keeps_transparency() {
        let igglanova = record(r#", "palette": {"cram_line": 3}"#);
        assert_eq!(temporary_recolor_failures(&igglanova), Vec::<&str>::new());
    }

    /// Negative control: the same object on the party's line 2 is caught.
    #[test]
    fn the_temporary_recolour_check_rejects_a_wrong_line() {
        let wrong = record(r#", "palette": {"cram_line": 2}"#);
        let failures = temporary_recolor_failures(&wrong);
        assert!(
            failures.contains(&"pixels must read line 3"),
            "{failures:?}"
        );
        assert!(
            failures.contains(&"a CRAM revision did not recolour its line"),
            "{failures:?}"
        );
    }

    /// The pack's records, when a local pack is present: every temporary
    /// object names a line, and Igglanova's is the line `Cutscene_Alshline`
    /// loads before building it (`LoadPalette` at word 48).
    #[test]
    fn the_packed_temporary_objects_name_their_lines() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../runtime-pack/presentation/panels.json"
        );
        let Ok(raw) = std::fs::read_to_string(path) else {
            eprintln!("runtime pack not present; skipping");
            return;
        };
        let manifest: PanelManifest = serde_json::from_str(&raw).expect("manifest parses");
        assert!(!manifest.temporary_objects.is_empty());
        for record in &manifest.temporary_objects {
            assert!(
                record.layout().is_some(),
                "{:#06x}/{:#05x}",
                record.object_id,
                record.art_tile
            );
        }
        let load = psiv_core::SCENES
            .iter()
            .find(|scene| scene.name == "Cutscene_Alshline")
            .and_then(|scene| {
                scene.ops.iter().find_map(|op| match *op {
                    psiv_core::SceneOp::LoadPalette { start, words, .. } => Some((start, words)),
                    _ => None,
                })
            })
            .expect("Alshline loads Igglanova's palette");
        let igglanova = manifest
            .temporary_objects
            .iter()
            .find(|record| record.object_id == 0x188)
            .expect("Igglanova's record");
        assert_eq!(
            igglanova.layout(),
            Some(IndexLayout::Line(load.0 / 16)),
            "Igglanova draws the line the scene writes"
        );
        assert_eq!(load, (48, 14));
    }
}
