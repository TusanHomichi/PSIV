//! Godot-side consumer for the scene presentation stream.
//!
//! `psiv-runtime` deliberately stops at typed, ordered `SceneOp`s.  This
//! module is the other half of that seam: it owns staged panel planes,
//! opening-image text, sprite visibility, temporary-object bookkeeping, and
//! the saved-music word.  It does not advance a scene or mutate scene flags.

use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{INode2D, Image, ImageTexture, Input, Node2D};
use godot::prelude::*;
use psiv_core::{PresentationAsset, PresentationOp, SceneOp};
use psiv_data::{DialogueSet, Role};
use serde::Deserialize;

use crate::Field;
use crate::transitions::TransitionKind;

#[path = "temporary_sprite_asset.rs"]
mod temporary_sprite_asset;
mod text_layer;
use temporary_sprite_asset::TemporarySpriteAsset;
use text_layer::OpeningTextLayer;

const SCREEN: Vector2 = Vector2::new(320.0, 224.0);

/// Retail `grand_cross=0` keeps the plane origin one pixel right/down of the
/// window coordinate origin in the settled MeetingRika receipt.  Camera and
/// VDP scroll values are decoded in `oracle/scroll_state.py`; this is the
/// remaining hardware-origin term, not a camera adjustment.
const RETAIL_PLANE_SCROLL_RESIDUE: Vector2 = Vector2::new(1.0, 1.0);

fn retail_plane_scroll_offset() -> Vector2 {
    RETAIL_PLANE_SCROLL_RESIDUE
}

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

/// Renderer state that has no place in `psiv-runtime`'s field semantics.
#[derive(Debug, Default)]
pub(crate) struct PresentationState {
    render_sprites: bool,
    /// `InitVramAndCram` wiped the stage: no map, no actors, until a map
    /// redraw reloads the art. Distinct from `render_sprites`, which mirrors
    /// retail's explicit cutscene sprite toggle and survives map loads.
    vram_blanked: bool,
    saved_music: Option<u8>,
    temporary_objects: BTreeMap<usize, TemporaryObject>,
    loaded_palettes: BTreeMap<u32, u16>,
    loaded_art: BTreeMap<(u32, u16), u64>,
    current_dialogue_tree: Option<u32>,
    ending_waiting_for_start: bool,
    op_count: u64,
}

/// The literal fields of a retail temporary object.  A matching map sprite is
/// used when available; no synthetic art is invented when the pack has not
/// decoded the object's Nemesis blob yet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TemporaryObject {
    pub(crate) object_id: u16,
    pub(crate) art_tile: u16,
    pub(crate) frames_left: u16,
    pub(crate) destination: Option<(i32, i32)>,
}

impl PresentationState {
    pub(crate) fn reset_scene(&mut self) {
        self.render_sprites = true;
        self.vram_blanked = false;
        self.temporary_objects.clear();
        self.loaded_art.clear();
        self.current_dialogue_tree = None;
        self.ending_waiting_for_start = false;
    }

    pub(crate) fn sprites_visible(&self, scene_active: bool) -> bool {
        !scene_active || (self.render_sprites && !self.vram_blanked)
    }

    pub(crate) fn set_vram_blanked(&mut self, blanked: bool) {
        self.vram_blanked = blanked;
    }

    pub(crate) fn set_render_sprites(&mut self, enabled: bool) {
        self.render_sprites = enabled;
    }

    pub(crate) fn set_saved_music(&mut self, id: u8) {
        self.saved_music = (id != 0).then_some(id);
    }

    pub(crate) fn load_art(&mut self, rom_addr: u32, tile: u16) {
        self.loaded_art.insert((rom_addr, tile), self.op_count);
    }

    pub(crate) fn art_loaded(&self, rom_addr: u32, tile: u16) -> bool {
        self.loaded_art.contains_key(&(rom_addr, tile))
    }

    pub(crate) fn scene_dialogue_tree(&self, fallback: u8) -> u8 {
        match self.current_dialogue_tree {
            Some(0x001E_BA90) => 17,
            Some(0x001F_AAC0) => 39,
            Some(0x001F_C920) => 42,
            _ => fallback,
        }
    }

    pub(crate) fn take_saved_music(&mut self) -> Option<u8> {
        self.saved_music.take()
    }

    pub(crate) fn temporary_draws(&self) -> Vec<(usize, TemporaryObject)> {
        self.temporary_objects
            .iter()
            .map(|(&slot, &object)| (slot, object))
            .collect()
    }

    fn object_animation(&mut self, slot: usize, object_id: u16, art_tile: u16, frames: u16) {
        self.temporary_objects.insert(
            slot,
            TemporaryObject {
                object_id,
                art_tile,
                frames_left: frames,
                destination: None,
            },
        );
    }

    fn object_destination(&mut self, slot: usize, x: i32, y: i32) {
        if let Some(object) = self.temporary_objects.get_mut(&slot) {
            object.destination = Some((x, y));
        }
    }

    fn advance_objects(&mut self) {
        for object in self.temporary_objects.values_mut() {
            object.frames_left = object.frames_left.saturating_sub(1);
        }
    }
}

/// One panel-plane transaction. `staged` mirrors the cartridge's panel stack;
/// `visible` changes only on `DmaPlanes`, so a scene can build the next frame
/// without exposing half of it for one Godot draw.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct CutsceneLayer {
    base: Base<Node2D>,
    textures: BTreeMap<u16, Gd<ImageTexture>>,
    temporary_assets: BTreeMap<(u16, u16), TemporarySpriteAsset>,
    palettes: BTreeMap<u32, Vec<u16>>,
    staged: Vec<u16>,
    visible: Vec<u16>,
    opening_background: Option<Gd<ImageTexture>>,
    /// Oracle-measured top of the narration image band, from the pack
    /// manifest (`presentation/panels.json`, `opening_background.screen_y`).
    opening_screen_y: f32,
    opening_visible: bool,
    text_layer: Option<Gd<OpeningTextLayer>>,
    window_tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    generic_window_rect: Rect2,
    generic_window_visible: bool,
    generic_portrait: Option<Gd<ImageTexture>>,
    generic_portrait_rect: Option<Rect2>,
    red_flash_frames: u8,
}

#[godot_api]
impl INode2D for CutsceneLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            textures: BTreeMap::new(),
            temporary_assets: BTreeMap::new(),
            palettes: BTreeMap::new(),
            staged: Vec::new(),
            visible: Vec::new(),
            opening_background: None,
            opening_screen_y: 40.0,
            opening_visible: false,
            text_layer: None,
            window_tiles: BTreeMap::new(),
            generic_window_rect: Rect2::new(Vector2::new(24.0, 160.0), Vector2::new(272.0, 48.0)),
            generic_window_visible: false,
            generic_portrait: None,
            generic_portrait_rect: None,
            red_flash_frames: 0,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(550);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        if self.opening_visible {
            self.base_mut().draw_rect(
                Rect2::new(Vector2::ZERO, SCREEN),
                Color::from_rgb(0.0, 0.0, 0.0),
            );
            if let Some(texture) = self.opening_background.clone() {
                let screen_y = self.opening_screen_y;
                self.base_mut().draw_texture_rect(
                    &texture,
                    Rect2::new(Vector2::new(0.0, screen_y), Vector2::new(320.0, 128.0)),
                    true,
                );
            }
        }
        let visible = self.visible.clone();
        for id in visible {
            if let Some(texture) = self.textures.get(&id).cloned() {
                self.base_mut().draw_texture_rect(
                    &texture,
                    Rect2::new(retail_plane_scroll_offset(), SCREEN),
                    true,
                );
            }
        }
        if self.generic_window_visible {
            self.draw_generic_window();
        }
        if let (Some(texture), Some(rect)) =
            (self.generic_portrait.clone(), self.generic_portrait_rect)
        {
            self.base_mut().draw_texture_rect(&texture, rect, true);
        }
        if self.red_flash_frames > 0 {
            self.base_mut().draw_rect(
                Rect2::new(Vector2::ZERO, SCREEN),
                Color::from_rgba(0.8, 0.0, 0.0, 0.55),
            );
        }
    }
}

impl CutsceneLayer {
    pub(crate) fn configure(&mut self, pack_dir: &str, set: &DialogueSet) {
        let manifest_path = Path::new(pack_dir).join("presentation/panels.json");
        match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PanelManifest>(&raw).ok())
        {
            Some(manifest) => {
                if let Some(opening) = &manifest.opening_background {
                    self.opening_screen_y = opening.screen_y;
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
                    self.palettes.insert(address, words);
                }
                for panel in manifest.panels {
                    let path = format!("{pack_dir}/{}", panel.png);
                    if let Some(image) = Image::load_from_file(&GString::from(path.as_str()))
                        && let Some(texture) = ImageTexture::create_from_image(&image)
                    {
                        self.textures.insert(panel.id, texture);
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
                    self.temporary_assets.insert(
                        (temporary.object_id, temporary.art_tile),
                        TemporarySpriteAsset {
                            texture,
                            frame_width: temporary.frame_width,
                            frame_height: temporary.frame_height,
                            origin_x: temporary.origin_x,
                            origin_y: temporary.origin_y,
                            sequences,
                            load_art,
                        },
                    );
                }
                for portrait in manifest.portraits {
                    if portrait.id == "shopkeeper_2" {
                        let path = format!("{pack_dir}/{}", portrait.png);
                        self.generic_portrait =
                            Image::load_from_file(&GString::from(path.as_str()))
                                .and_then(|image| ImageTexture::create_from_image(&image));
                    }
                }
            }
            None => godot_warn!("scene panel manifest missing: {}", manifest_path.display()),
        }

        let background = format!("{pack_dir}/presentation/opening_background.png");
        self.opening_background = Image::load_from_file(&GString::from(background.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        if self.opening_background.is_none() {
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
                    self.window_tiles.insert(role.as_str(), texture);
                }
            }
            let rect = set.window.text_window.rect;
            self.generic_window_rect = Rect2::new(
                Vector2::new(rect.x as f32, rect.y as f32),
                Vector2::new(rect.width as f32, rect.height as f32),
            );
        } else {
            godot_error!("generic scene window failed to load: {window_path}");
        }

        let mut text = OpeningTextLayer::new_alloc();
        text.bind_mut().configure(pack_dir, set.clone());
        self.base_mut().add_child(&text);
        self.text_layer = Some(text);
    }

    pub(crate) fn place(&mut self) {
        let viewport = self.base().get_viewport_rect();
        let canvas = self.base().get_canvas_transform().affine_inverse();
        let top_left = canvas * viewport.position;
        let bottom_right = canvas * (viewport.position + viewport.size);
        let visible = bottom_right - top_left;
        let position = Vector2::new(
            (top_left.x + (visible.x - SCREEN.x) / 2.0).floor(),
            (top_left.y + (visible.y - SCREEN.y) / 2.0).floor(),
        );
        self.base_mut().set_position(position);
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().place();
        }
    }

    pub(crate) fn panel_create(&mut self, id: u16) {
        if self.textures.contains_key(&id) {
            self.staged.push(id);
            godot_print!("scene panel staged: {id:#05x}");
        } else {
            godot_error!("scene panel {id:#05x} is not decoded in the runtime pack");
        }
    }

    pub(crate) fn load_palette(&self, rom_addr: u32, words: u16) {
        match self.palettes.get(&rom_addr) {
            Some(decoded) if decoded.len() >= usize::from(words) => {
                godot_print!("scene palette loaded: {words} words from {rom_addr:#08x}");
            }
            Some(decoded) => godot_error!(
                "scene palette {rom_addr:#08x} has {} words, requested {words}",
                decoded.len()
            ),
            None => godot_error!("scene palette {rom_addr:#08x} is not in the runtime pack"),
        }
    }

    pub(crate) fn panel_destroy(&mut self, id: u16) {
        if let Some(actual) = self.staged.pop()
            && actual != id
        {
            godot_warn!(
                "Panel_Destroy({id:#05x}) popped staged panel {actual:#05x}; retail allocator is stack based"
            );
        }
        godot_print!("scene panel destroyed: {id:#05x}");
    }

    pub(crate) fn panel_destroy_last(&mut self) {
        if let Some(id) = self.staged.pop() {
            godot_print!("scene panel destroyed: last {id:#05x}");
        }
    }

    /// `$F2` palette actions target CRAM, while this pack stores panels as
    /// palette-baked PNGs. Redrawing is the faithful operation available at
    /// this presentation boundary; the raw palette records remain auditable
    /// in `presentation/panels.json`.
    pub(crate) fn refresh_palette(&mut self) {
        self.base_mut().queue_redraw();
        godot_print!("dialogue action: palette refresh");
    }

    /// Small red-palette overlay used by the three retail dialogue effects
    /// whose source writes CRAM synchronously instead of loading a panel.
    pub(crate) fn red_flash(&mut self, frames: u8) {
        self.red_flash_frames = self.red_flash_frames.max(frames);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn panel_destroy_all(&mut self) {
        self.staged.clear();
        self.visible.clear();
        let opening_visible = self.opening_visible;
        let generic_window_visible = self.generic_window_visible;
        self.base_mut()
            .set_visible(opening_visible || generic_window_visible);
        self.base_mut().queue_redraw();
        godot_print!("scene panels destroyed: all");
    }

    pub(crate) fn dma_planes(&mut self) {
        self.visible.clone_from(&self.staged);
        let visible =
            self.opening_visible || self.generic_window_visible || !self.visible.is_empty();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
        godot_print!("scene DMA planes: {} visible panel(s)", self.visible.len());
    }

    pub(crate) fn begin_opening(&mut self) {
        self.opening_visible = true;
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().clear();
        }
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn end_opening(&mut self) {
        self.opening_visible = false;
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().clear();
        }
        let visible = self.generic_window_visible || !self.visible.is_empty();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn draw_text(&mut self, tree_rom_addr: Option<u32>, entry: u16) {
        if let Some(text) = self.text_layer.as_mut() {
            let decoded = { text.bind().entry(tree_rom_addr, entry).cloned() };
            if let Some(entry) = decoded {
                text.bind_mut().draw_entry(&entry);
            } else {
                godot_warn!("opening text entry {entry} has no decoded tree record");
            }
        }
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn set_text_colour(&mut self, colour: u16) {
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().set_colour(colour);
        }
    }

    pub(crate) fn fade_text(&mut self, direction: i8) {
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().set_fade(direction);
        }
    }

    pub(crate) fn tick(&mut self) {
        self.red_flash_frames = self.red_flash_frames.saturating_sub(1);
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().tick();
        }
        if self.red_flash_frames > 0 {
            self.base_mut().queue_redraw();
        }
    }

    pub(crate) fn temporary_asset(
        &self,
        object_id: u16,
        art_tile: u16,
    ) -> Option<TemporarySpriteAsset> {
        self.temporary_assets.get(&(object_id, art_tile)).cloned()
    }

    pub(crate) fn window_destroy(&mut self) {
        self.generic_window_visible = false;
        self.generic_portrait_rect = None;
        let visible = self.opening_visible || !self.visible.is_empty();
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn window_create(&mut self) {
        self.generic_window_visible = true;
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    pub(crate) fn load_window_tiles(&self, asset: PresentationAsset) {
        godot_print!("scene generic window tiles loaded: {asset:?}");
    }

    pub(crate) fn load_portrait(&mut self, asset: PresentationAsset) {
        if asset == PresentationAsset::ShopkeeperDialPortrait2 && self.generic_portrait.is_none() {
            godot_error!("scene shopkeeper portrait was not emitted in the presentation pack");
        }
    }

    pub(crate) fn draw_portrait(&mut self, x: u8, y: u8, width: u8, height: u8) {
        self.generic_portrait_rect = Some(Rect2::new(
            Vector2::new(f32::from(x) * 8.0, f32::from(y) * 8.0),
            Vector2::new(f32::from(width) * 8.0, f32::from(height) * 8.0),
        ));
        self.base_mut().queue_redraw();
    }

    fn draw_generic_window(&mut self) {
        let rect = self.generic_window_rect;
        let columns = (rect.size.x / 8.0).round() as i32;
        let rows = (rect.size.y / 8.0).round() as i32;
        let cell = |role: &str, x: i32, y: i32, this: &mut CutsceneLayer| {
            let Some(texture) = this.window_tiles.get(role).cloned() else {
                return;
            };
            this.base_mut().draw_texture_rect(
                &texture,
                Rect2::new(
                    rect.position + Vector2::new(x as f32 * 8.0, y as f32 * 8.0),
                    Vector2::new(8.0, 8.0),
                ),
                false,
            );
        };
        for y in 1..rows - 1 {
            for x in 1..columns - 1 {
                cell("fill", x, y, self);
            }
        }
        for x in 1..columns - 1 {
            cell("edge_top", x, 0, self);
            cell("edge_bottom", x, rows - 1, self);
        }
        for y in 1..rows - 1 {
            cell("edge_left", 0, y, self);
            cell("edge_right", columns - 1, y, self);
        }
        cell("corner_top_left", 0, 0, self);
        cell("corner_top_right", columns - 1, 0, self);
        cell("corner_bottom_left", 0, rows - 1, self);
        cell("corner_bottom_right", columns - 1, rows - 1, self);
    }
}

fn parse_hex(value: &str) -> Option<u32> {
    u32::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}

impl Field {
    /// Consume one presentation op in the exact event order the runtime
    /// emitted it. State-changing scene ops remain in the runtime; this only
    /// updates the shell's drawable/audio surfaces.
    pub(super) fn consume_scene_op(&mut self, op: SceneOp) {
        self.presentation.op_count = self.presentation.op_count.saturating_add(1);
        // The tick prefix is what lets a debug capture be paired with an
        // oracle tape frame: the op log becomes a timeline, not just an order.
        if std::env::var_os("PSIV_DEBUG_SCENE_TICKS").is_some() {
            godot_print!("scene op t{}: {op:?}", self.anim_tick);
        }
        match op {
            SceneOp::FadeIn => self.start_transition(TransitionKind::SceneFadeIn),
            SceneOp::FadeOut => self.start_transition(TransitionKind::SceneFadeOut),
            SceneOp::InitVramAndCram => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_all();
                }
                // On hardware this wipes the tile planes AND the sprite art:
                // map and actors are gone until the scene's own
                // LoadMap/RefreshMap reloads them (the oracle shows the
                // opening's first dialogue over pure black — no sprites —
                // before its LoadMap).
                self.set_field_map_visible(false);
                self.presentation.set_vram_blanked(true);
            }
            SceneOp::LoadPalette { rom_addr, words } => {
                self.presentation.loaded_palettes.insert(rom_addr, words);
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind().load_palette(rom_addr, words);
                }
                godot_print!("scene palette: {words} words from {rom_addr:#08x}");
            }
            SceneOp::LoadArt { rom_addr, tile } => {
                self.presentation.load_art(rom_addr, tile);
                godot_print!("scene art: {rom_addr:#08x} -> VRAM tile {tile:#05x}");
            }
            SceneOp::SetCameraPos { x, y } | SceneOp::MoveCamera { x, y, .. } => {
                if let Some(runtime) = self.runtime.as_mut() {
                    runtime.set_camera(x, y);
                }
            }
            SceneOp::LoadTitleImage { .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().begin_opening();
                }
                godot_print!("scene title image loaded");
            }
            SceneOp::SetTextColour { colour } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().set_text_colour(colour);
                }
            }
            SceneOp::DrawTextToPlane { entry, .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer
                        .bind_mut()
                        .draw_text(self.presentation.current_dialogue_tree, entry);
                }
            }
            SceneOp::IntroTextFadeUp => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().fade_text(1);
                }
            }
            SceneOp::IntroTextFadeDown => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().fade_text(-1);
                }
            }
            SceneOp::PlaySound { id } => self.play_scene_sound(id),
            SceneOp::SetSavedMusic { id } => self.save_scene_music(id),
            SceneOp::SetRenderSpritesInCutscene { enabled } => {
                self.presentation.set_render_sprites(enabled);
                self.apply_scene_sprite_visibility();
            }
            SceneOp::WaitFrames { frames } => {
                godot_print!("scene wait-frames: {frames}");
            }
            SceneOp::WaitForStart => {
                self.presentation.ending_waiting_for_start = true;
                godot_print!("scene waiting for ending Start input");
            }
            SceneOp::MarkGameCleared => {
                godot_print!("scene marked game cleared");
            }
            SceneOp::PanelCreate { id } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_create(id);
                }
            }
            SceneOp::PanelDestroy { id } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy(id);
                }
            }
            SceneOp::PanelDestroyLast => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_last();
                }
            }
            SceneOp::PanelDestroyAll => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_all();
                }
            }
            SceneOp::DmaPlanes => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().dma_planes();
                }
            }
            SceneOp::ObjectAnimation {
                slot,
                object_id,
                art_tile,
                frames,
            } => {
                godot_print!(
                    "scene temporary object: slot {slot}, id {object_id:#06x}, art {art_tile:#05x}, frames {frames}"
                );
                self.presentation
                    .object_animation(slot, object_id, art_tile, frames);
            }
            SceneOp::Presentation { op } => self.consume_presentation_op(op),
            SceneOp::SetArtTile { actor, tile } => {
                godot_print!("scene art tile: {actor:?} -> {tile:#05x}");
            }
            SceneOp::SetDialogueTree { rom_addr } => {
                self.presentation.current_dialogue_tree = Some(rom_addr);
            }
            SceneOp::ReloadMapPalette
            | SceneOp::OverlapCharacters
            | SceneOp::SetFollowMode { .. }
            | SceneOp::SetStepOffset { .. }
            | SceneOp::RecoverStats
            | SceneOp::SwapCharSlots { .. }
            | SceneOp::RemoveItem { .. } => {
                godot_print!("scene presentation op consumed: {op:?}");
            }
            _ => {}
        }
    }

    fn consume_presentation_op(&mut self, op: PresentationOp) {
        match op {
            PresentationOp::ReloadMapChunks | PresentationOp::RebuildSprites => {
                self.load_map_visuals();
            }
            PresentationOp::LoadSceneAsset { .. }
            | PresentationOp::SetGameMode { .. }
            | PresentationOp::ClearHeldInput
            | PresentationOp::SavePartySpriteX { .. }
            | PresentationOp::RestorePartySpriteX
            | PresentationOp::SetDialoguePortrait { .. } => {
                godot_print!("scene presentation record consumed: {op:?}");
            }
            PresentationOp::WindowDestroy { .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().window_destroy();
                }
            }
            PresentationOp::WindowCreate { .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().window_create();
                }
            }
            PresentationOp::LoadWindowTiles { asset, .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind().load_window_tiles(asset);
                }
            }
            PresentationOp::LoadPortrait { asset, .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().load_portrait(asset);
                }
            }
            PresentationOp::DrawPortrait {
                x,
                y,
                width,
                height,
                ..
            } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().draw_portrait(x, y, width, height);
                }
            }
            PresentationOp::SetPaletteWords {
                offset,
                first,
                second,
            } => {
                godot_print!(
                    "scene palette words: offset {offset:#06x}, {first:#06x}, {second:#06x}"
                );
            }
            PresentationOp::AddMacro { slot } => {
                godot_print!("scene macro added for party slot {slot}");
            }
            PresentationOp::SetObjectDestination { slot, x, y } => {
                self.presentation.object_destination(slot, x, y);
            }
            PresentationOp::FadeToRed { lines } | PresentationOp::FadeFromRed { lines } => {
                godot_print!("scene red palette fade across {lines} line(s)");
            }
            op @ (PresentationOp::CopyRamWords { .. }
            | PresentationOp::ClearRamWords { .. }
            | PresentationOp::ClearRamLongs { .. }
            | PresentationOp::FillRamWords { .. }
            | PresentationOp::ClearPlanes
            | PresentationOp::EndingFinaleFieldPrep { .. }
            | PresentationOp::DmaPlanesLoop { .. }
            | PresentationOp::PaletteIncreaseTone { .. }
            | PresentationOp::ClearPaletteLine { .. }
            | PresentationOp::VariablePaletteFade { .. }
            | PresentationOp::RykrosPaletteCycle { .. }
            | PresentationOp::CameraToActor { .. }
            | PresentationOp::RajaSickTemporaryObject { .. }
            | PresentationOp::RajaSickResetRaja { .. }
            | PresentationOp::RajaSickArrangeParty { .. }
            | PresentationOp::EndingCreditsAssets { .. }
            | PresentationOp::EndingCreditsStage { .. }
            | PresentationOp::EndingCreditsPaletteRamp { .. }
            | PresentationOp::EndingStaffRollTransition { .. }
            | PresentationOp::EndingFinale { .. }) => {
                godot_print!("scene presentation record consumed: {op:?}");
            }
        }
    }

    pub(super) fn tick_cutscene_presentation(&mut self) {
        if self.presentation.ending_waiting_for_start
            && Input::singleton().is_action_just_pressed("ui_accept")
        {
            self.presentation.ending_waiting_for_start = false;
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.ending_continue();
            }
        }
        self.presentation.advance_objects();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            layer.bind_mut().tick();
            layer.bind_mut().place();
        }
    }

    pub(super) fn finish_cutscene_presentation(&mut self) {
        let restored = self.restore_saved_music();
        self.presentation.reset_scene();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            layer.bind_mut().end_opening();
            layer.bind_mut().panel_destroy_all();
        }
        if !restored {
            self.play_map_music();
        }
        self.apply_scene_sprite_visibility();
    }

    pub(super) fn apply_scene_sprite_visibility(&mut self) {
        let active = self.runtime.as_ref().is_some_and(|rt| rt.scene_active());
        let visible = self.presentation.sprites_visible(active);
        if let Some(party) = self.party.as_mut() {
            party.set_visible(visible);
        }
        for follower in &mut self.follower_nodes {
            follower.0.set_visible(visible);
        }
        for npc in &mut self.npc_nodes {
            npc.node.set_visible(visible);
        }
        let has_temporary = !self.presentation.temporary_objects.is_empty();
        for node in self.temporary_nodes.values_mut() {
            node.set_visible(visible && has_temporary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PresentationState;

    #[test]
    fn saved_music_is_a_one_shot_restore_word_and_zero_clears_it() {
        let mut state = PresentationState::default();
        assert_eq!(state.take_saved_music(), None);

        state.set_saved_music(0x91);
        assert_eq!(state.take_saved_music(), Some(0x91));
        assert_eq!(state.take_saved_music(), None);

        state.set_saved_music(0x91);
        state.set_saved_music(0);
        assert_eq!(state.take_saved_music(), None);
    }

    #[test]
    fn sprite_gate_only_suppresses_sprites_while_a_scene_is_active() {
        let mut state = PresentationState::default();
        state.set_render_sprites(false);
        assert!(!state.sprites_visible(true));
        assert!(state.sprites_visible(false));
    }
}
