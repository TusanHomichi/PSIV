//! Godot-side consumer for the scene presentation stream.
//!
//! `psiv-runtime` deliberately stops at typed, ordered `SceneOp`s.  This
//! module is the other half of that seam: it owns staged panel planes,
//! opening-image text, sprite visibility, temporary-object bookkeeping, and
//! the saved-music word.  It does not advance a scene or mutate scene flags.
//!
//! [`CutsceneLayer`] is the node the field shell drives. This file owns the
//! node, the assets it holds and the per-frame step; the submodules own one
//! concern each - [`state`] the renderer state, [`manifest`] the pack decode,
//! [`panels`] the panel-plane stack, [`opening`] the opening image and text,
//! [`window`] the generic window, and [`ops`] the shell that consumes the op
//! stream.

mod manifest;
mod opening;
mod ops;
mod panels;
mod state;
mod window;

#[path = "temporary_sprite_asset.rs"]
mod temporary_sprite_asset;
mod text_layer;

use std::collections::BTreeMap;

use godot::classes::{INode2D, ImageTexture, Node2D};
use godot::prelude::*;
use psiv_data::DialogueSet;

use manifest::SceneAssets;
use temporary_sprite_asset::TemporarySpriteAsset;
use text_layer::OpeningTextLayer;

pub(crate) use state::PresentationState;

const SCREEN: Vector2 = Vector2::new(320.0, 224.0);

/// The scene presentation node the field shell drives: staged panel planes,
/// the opening image and its narration layer, the generic cutscene window and
/// the sprite bookkeeping `psiv-runtime` deliberately does not hold.
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
        self.draw_opening();
        self.draw_planes();
        self.draw_window();
        self.draw_red_flash();
    }
}

impl CutsceneLayer {
    /// Loads the pack's scene art into the tables this layer draws from.
    pub(crate) fn configure(&mut self, pack_dir: &str, set: &DialogueSet) {
        let assets = SceneAssets::load(pack_dir, set);
        if let Some(screen_y) = assets.opening_screen_y {
            self.opening_screen_y = screen_y;
        }
        self.palettes.extend(assets.palettes);
        self.textures.extend(assets.textures);
        self.temporary_assets.extend(assets.temporary_assets);
        self.opening_background = assets.opening_background;
        if assets.generic_portrait_named {
            self.generic_portrait = assets.generic_portrait;
        }
        if let Some(rect) = assets.generic_window_rect {
            self.generic_window_rect = rect;
        }
        self.window_tiles.extend(assets.window_tiles);

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

    pub(crate) fn tick(&mut self) {
        self.red_flash_frames = self.red_flash_frames.saturating_sub(1);
        self.tick_opening_text();
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
}
