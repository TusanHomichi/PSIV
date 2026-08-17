//! Godot-side consumer for the scene presentation stream.
//!
//! `psiv-runtime` deliberately stops at typed, ordered `SceneOp`s.  This
//! module is the other half of that seam: it owns staged panel planes,
//! opening-image text, sprite visibility, temporary-object bookkeeping, and
//! the saved-music word.  It does not advance a scene or mutate scene flags.

use std::collections::BTreeMap;
use std::path::Path;

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;
use psiv_core::{PresentationOp, SceneOp};
use psiv_data::{DialogueEntry, DialogueSet};
use serde::Deserialize;

use crate::Field;
use crate::transitions::TransitionKind;

const SCREEN: Vector2 = Vector2::new(320.0, 224.0);

#[derive(Debug, Deserialize)]
struct PanelManifest {
    panels: Vec<PanelRecord>,
    #[serde(default)]
    palettes: Vec<PaletteRecord>,
    #[serde(default)]
    opening_background: Option<OpeningBackgroundRecord>,
}

#[derive(Debug, Deserialize)]
struct OpeningBackgroundRecord {
    /// Oracle-measured top of the narration image band in the 320x224 frame.
    screen_y: f32,
}

#[derive(Debug, Deserialize)]
struct PanelRecord {
    id: u8,
    png: String,
}

#[derive(Debug, Deserialize)]
struct PaletteRecord {
    rom_addr: String,
    words: u16,
    raw_hex: String,
}

/// Renderer state that has no place in `psiv-runtime`'s field semantics.
#[derive(Debug, Default)]
pub(crate) struct PresentationState {
    render_sprites: bool,
    saved_music: Option<u8>,
    temporary_objects: BTreeMap<usize, TemporaryObject>,
    loaded_palettes: BTreeMap<u32, u16>,
    current_dialogue_tree: Option<u32>,
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
        self.temporary_objects.clear();
        self.current_dialogue_tree = None;
    }

    pub(crate) fn sprites_visible(&self, scene_active: bool) -> bool {
        !scene_active || self.render_sprites
    }

    pub(crate) fn set_render_sprites(&mut self, enabled: bool) {
        self.render_sprites = enabled;
    }

    pub(crate) fn set_saved_music(&mut self, id: u8) {
        self.saved_music = (id != 0).then_some(id);
    }

    pub(crate) fn scene_dialogue_tree(&self, fallback: u8) -> u8 {
        match self.current_dialogue_tree {
            Some(0x001E_BA90) => 17,
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
    textures: BTreeMap<u8, Gd<ImageTexture>>,
    palettes: BTreeMap<u32, Vec<u16>>,
    staged: Vec<u8>,
    visible: Vec<u8>,
    opening_background: Option<Gd<ImageTexture>>,
    /// Oracle-measured top of the narration image band, from the pack
    /// manifest (`presentation/panels.json`, `opening_background.screen_y`).
    opening_screen_y: f32,
    opening_visible: bool,
    text_layer: Option<Gd<OpeningTextLayer>>,
}

#[godot_api]
impl INode2D for CutsceneLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            textures: BTreeMap::new(),
            palettes: BTreeMap::new(),
            staged: Vec::new(),
            visible: Vec::new(),
            opening_background: None,
            opening_screen_y: 40.0,
            opening_visible: false,
            text_layer: None,
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
                    Rect2::new(Vector2::ZERO, SCREEN),
                    true,
                );
            }
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
            }
            None => godot_warn!("scene panel manifest missing: {}", manifest_path.display()),
        }

        let background = format!("{pack_dir}/presentation/opening_background.png");
        self.opening_background = Image::load_from_file(&GString::from(background.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        if self.opening_background.is_none() {
            godot_error!("opening background failed to load: {background}");
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

    pub(crate) fn panel_create(&mut self, id: u8) {
        if self.textures.contains_key(&id) {
            self.staged.push(id);
            godot_print!("scene panel staged: {id:#04x}");
        } else {
            godot_error!("scene panel {id:#04x} is not decoded in the runtime pack");
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

    pub(crate) fn panel_destroy(&mut self, id: u8) {
        if let Some(actual) = self.staged.pop()
            && actual != id
        {
            godot_warn!(
                "Panel_Destroy({id:#04x}) popped staged panel {actual:#04x}; retail allocator is stack based"
            );
        }
        godot_print!("scene panel destroyed: {id:#04x}");
    }

    pub(crate) fn panel_destroy_all(&mut self) {
        self.staged.clear();
        self.visible.clear();
        let opening_visible = self.opening_visible;
        self.base_mut().set_visible(opening_visible);
        self.base_mut().queue_redraw();
        godot_print!("scene panels destroyed: all");
    }

    pub(crate) fn dma_planes(&mut self) {
        self.visible.clone_from(&self.staged);
        let visible = self.opening_visible || !self.visible.is_empty();
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
        let visible = !self.visible.is_empty();
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
        if let Some(text) = self.text_layer.as_mut() {
            text.bind_mut().tick();
        }
    }
}

/// The opening crawl is a separate drawable child so its CRAM text ramp does
/// not modulate the opaque black/title image behind it.
#[derive(GodotClass)]
#[class(base=Node2D)]
struct OpeningTextLayer {
    base: Base<Node2D>,
    font: Option<Gd<ImageTexture>>,
    set: Option<DialogueSet>,
    entries: [Option<DialogueEntry>; 4],
    next_slot: usize,
    colour: Color,
    fade_direction: i8,
    fade_tick: u8,
    fade_frames_left: u8,
}

#[godot_api]
impl INode2D for OpeningTextLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            font: None,
            set: None,
            entries: std::array::from_fn(|_| None),
            next_slot: 0,
            colour: Color::from_rgb(0.0, 0.0, 0.0),
            fade_direction: 0,
            fade_tick: 0,
            fade_frames_left: 0,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(560);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        let Some(font) = self.font.clone() else {
            return;
        };
        let mut quads = Vec::new();
        for (row, entry) in self.entries.iter().enumerate() {
            let Some(entry) = entry else { continue };
            let Some(page) = entry.pages.first() else {
                continue;
            };
            let line = page.lines.first().map(String::as_str).unwrap_or_default();
            // Plane offset $40A is tile column 5: every line starts at x=40
            // and any centring is baked into the entry text itself.
            let x = 40.0;
            // DrawTextToPlane targets $840A, then advances 0x180 bytes per
            // entry: plane offset $40A is tile row 8 (y=64) and 0x180 bytes
            // is three 0x80-byte plane rows, a 24-pixel line pitch — both
            // confirmed against oracle/frames/opening/frame_4000.png.
            let y = 64.0 + row as f32 * 24.0;
            for (column, ch) in line.chars().enumerate() {
                let Some(glyph) = self.set.as_ref().and_then(|set| set.glyph(ch)) else {
                    continue;
                };
                quads.push((
                    Rect2::new(
                        Vector2::new(x + column as f32 * 8.0, y),
                        Vector2::new(8.0, 16.0),
                    ),
                    Rect2::new(
                        Vector2::new(glyph.x as f32, glyph.y as f32),
                        Vector2::new(8.0, 16.0),
                    ),
                ));
            }
        }
        for (dest, src) in quads {
            self.base_mut().draw_texture_rect_region(&font, dest, src);
        }
    }
}

impl OpeningTextLayer {
    fn configure(&mut self, pack_dir: &str, set: DialogueSet) {
        let path = format!("{pack_dir}/{}", set.font.png);
        self.font = Image::load_from_file(&GString::from(path.as_str()))
            .and_then(|image| ImageTexture::create_from_image(&image));
        self.set = Some(set);
        self.base_mut().set_visible(false);
    }

    fn entry(&self, tree_rom_addr: Option<u32>, entry: u16) -> Option<&DialogueEntry> {
        // The opening's SetDialogueTree points at retail DialogueTree17. The
        // pack carries the same tree by its stable 1-based number; other
        // scene tree addresses belong to ordinary dialogue windows.
        if tree_rom_addr != Some(0x001E_BA90) {
            return None;
        }
        self.set.as_ref()?.entry(17, entry)
    }

    fn place(&mut self) {
        let viewport = self.base().get_viewport_rect();
        let canvas = self.base().get_canvas_transform().affine_inverse();
        let top_left = canvas * viewport.position;
        let bottom_right = canvas * (viewport.position + viewport.size);
        let visible = bottom_right - top_left;
        self.base_mut().set_position(Vector2::new(
            (top_left.x + (visible.x - SCREEN.x) / 2.0).floor(),
            (top_left.y + (visible.y - SCREEN.y) / 2.0).floor(),
        ));
    }

    fn draw_entry(&mut self, entry: &DialogueEntry) {
        self.entries[self.next_slot] = Some(entry.clone());
        self.next_slot = (self.next_slot + 1) % self.entries.len();
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    fn clear(&mut self) {
        self.entries = std::array::from_fn(|_| None);
        self.next_slot = 0;
        self.base_mut().set_visible(false);
        self.base_mut().queue_redraw();
    }

    fn set_colour(&mut self, raw: u16) {
        self.colour = cram_colour(raw);
        let colour = self.colour;
        self.base_mut().set_modulate(colour);
        self.base_mut().queue_redraw();
    }

    fn set_fade(&mut self, direction: i8) {
        self.fade_direction = direction;
        self.fade_tick = 0;
        self.fade_frames_left = 20;
    }

    fn tick(&mut self) {
        if self.fade_direction == 0 || self.fade_frames_left == 0 {
            return;
        }
        self.fade_frames_left = self.fade_frames_left.saturating_sub(1);
        self.fade_tick = self.fade_tick.wrapping_add(1);
        if !self.fade_tick.is_multiple_of(4) {
            return;
        }
        let channels = self.colour;
        let step = 2.0 / 7.0;
        let next = if self.fade_direction > 0 {
            Color::from_rgb(
                (channels.r + step).min(1.0),
                (channels.g + step).min(1.0),
                (channels.b + step).min(1.0),
            )
        } else {
            Color::from_rgb(
                (channels.r - step).max(0.0),
                (channels.g - step).max(0.0),
                (channels.b - step).max(0.0),
            )
        };
        self.colour = next;
        self.base_mut().set_modulate(next);
        self.base_mut().queue_redraw();
    }
}

fn cram_colour(raw: u16) -> Color {
    let channel = |shift: u16| f32::from((raw >> shift) & 0x7) / 7.0;
    Color::from_rgb(channel(1), channel(5), channel(9))
}

impl Field {
    /// Consume one presentation op in the exact event order the runtime
    /// emitted it. State-changing scene ops remain in the runtime; this only
    /// updates the shell's drawable/audio surfaces.
    pub(super) fn consume_scene_op(&mut self, op: SceneOp) {
        self.presentation.op_count = self.presentation.op_count.saturating_add(1);
        match op {
            SceneOp::FadeIn => self.start_transition(TransitionKind::SceneFadeIn),
            SceneOp::FadeOut => self.start_transition(TransitionKind::SceneFadeOut),
            SceneOp::InitVramAndCram => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_all();
                }
            }
            SceneOp::LoadPalette { rom_addr, words } => {
                self.presentation.loaded_palettes.insert(rom_addr, words);
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind().load_palette(rom_addr, words);
                }
                godot_print!("scene palette: {words} words from {rom_addr:#08x}");
            }
            SceneOp::LoadArt { rom_addr, tile } => {
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
            | PresentationOp::WindowDestroy { .. }
            | PresentationOp::WindowCreate { .. }
            | PresentationOp::LoadWindowTiles { .. }
            | PresentationOp::LoadPortrait { .. }
            | PresentationOp::DrawPortrait { .. }
            | PresentationOp::SetGameMode { .. }
            | PresentationOp::ClearHeldInput
            | PresentationOp::SavePartySpriteX { .. }
            | PresentationOp::RestorePartySpriteX
            | PresentationOp::SetDialoguePortrait { .. } => {
                godot_print!("scene presentation record consumed: {op:?}");
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
        }
    }

    pub(super) fn tick_cutscene_presentation(&mut self) {
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
