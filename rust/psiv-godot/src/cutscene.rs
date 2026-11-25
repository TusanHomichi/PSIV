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
//! [`panels`] the panel-plane stack, [`picture_plane`] and [`picture`] a
//! scene's own plane-A picture, [`opening`] the opening image and text,
//! [`window`] the generic window, and [`ops`] the shell that consumes the op
//! stream.

mod credits;
mod manifest;
mod opening;
mod ops;
mod panel_stack;
mod panels;
mod picture;
mod picture_plane;
mod state;
mod window;

#[path = "temporary_sprite_asset.rs"]
mod temporary_sprite_asset;
mod text_layer;
mod text_plane;

use std::collections::BTreeMap;

use godot::classes::{INode2D, ImageTexture, Node2D, Sprite2D};
use godot::prelude::*;
use psiv_data::DialogueSet;
use psiv_data::color::IndexLayout;

use crate::cram_art::{IndexedArt, SharedCram};
use crate::pack_assets::PackAssets;

use manifest::SceneAssets;
use temporary_sprite_asset::TemporarySpriteAsset;
use text_layer::TextPlaneLayer;

pub(crate) use state::{MusicRestore, PresentationState};

const SCREEN: Vector2 = crate::retail_surface::SURFACE;

/// The scene presentation node the field shell drives: staged panel planes,
/// the opening image and its narration layer, the generic cutscene window and
/// the sprite bookkeeping `psiv-runtime` deliberately does not hold.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct CutsceneLayer {
    base: Base<Node2D>,
    pack_assets: PackAssets,
    textures: BTreeMap<u16, Gd<ImageTexture>>,
    /// Each panel's PNG and layer lines, and its indices once first drawn
    /// (`None`: the load failed, so the baked texture draws).
    indexed_panels: BTreeMap<u16, (String, IndexLayout)>,
    panel_art: BTreeMap<u16, Option<IndexedArt>>,
    /// The field's live palette; `None` until the field hands it over.
    cram: Option<SharedCram>,
    temporary_assets: BTreeMap<(u16, u16), TemporarySpriteAsset>,
    panels: panel_stack::PanelStack,
    opening_background: Option<Gd<ImageTexture>>,
    /// The background's indexed PNG, and its indices once first drawn in the
    /// live palette (`None` inside: the load failed, so the baked one draws).
    opening_background_path: Option<String>,
    opening_art: Option<Option<IndexedArt>>,
    /// Oracle-measured top of the narration image band, from the pack
    /// manifest (`presentation/panels.json`, `opening_background.screen_y`).
    opening_screen_y: f32,
    opening_visible: bool,
    text_layer: Option<Gd<TextPlaneLayer>>,
    window_tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    generic_window_rect: Rect2,
    generic_window_visible: bool,
    generic_portrait: Option<Gd<ImageTexture>>,
    generic_portrait_rect: Option<Rect2>,
    red_flash_frames: u8,
    /// The plane-A pictures the pack decoded, by the ROM address of their
    /// cells, and each one's indices once first drawn.
    pictures: BTreeMap<u32, picture::PictureAsset>,
    picture_art: BTreeMap<u32, Option<IndexedArt>>,
    picture: picture_plane::PicturePlane,
    /// Drawn under the sprites; created on first use.
    picture_sprite: Option<Gd<Sprite2D>>,
    /// `Cutscene_Ending`'s 320×224 indexed credits plane surface.
    credits: Option<credits::CreditsSurface>,
}

#[godot_api]
impl INode2D for CutsceneLayer {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            pack_assets: PackAssets::default(),
            textures: BTreeMap::new(),
            indexed_panels: BTreeMap::new(),
            panel_art: BTreeMap::new(),
            cram: None,
            temporary_assets: BTreeMap::new(),
            panels: panel_stack::PanelStack::default(),
            opening_background: None,
            opening_background_path: None,
            opening_art: None,
            opening_screen_y: 40.0,
            opening_visible: false,
            text_layer: None,
            window_tiles: BTreeMap::new(),
            generic_window_rect: Rect2::new(Vector2::new(24.0, 160.0), Vector2::new(272.0, 48.0)),
            generic_window_visible: false,
            generic_portrait: None,
            generic_portrait_rect: None,
            red_flash_frames: 0,
            pictures: BTreeMap::new(),
            picture_art: BTreeMap::new(),
            picture: picture_plane::PicturePlane::default(),
            picture_sprite: None,
            credits: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(550);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        self.draw_opening();
        self.draw_credits();
        self.draw_planes();
        self.draw_window();
        self.draw_red_flash();
    }
}

impl CutsceneLayer {
    /// Loads the pack's scene art into the tables this layer draws from.
    pub(crate) fn configure(&mut self, pack_assets: &PackAssets, set: &DialogueSet) {
        self.pack_assets = pack_assets.clone();
        let assets = SceneAssets::load(pack_assets, set);
        if let Some(screen_y) = assets.opening_screen_y {
            self.opening_screen_y = screen_y;
        }
        self.textures.extend(assets.textures);
        self.indexed_panels.extend(assets.indexed_panels);
        self.temporary_assets.extend(assets.temporary_assets);
        self.opening_background = assets.opening_background;
        self.opening_background_path = assets.opening_background_path;
        self.opening_art = None;
        if assets.generic_portrait_named {
            self.generic_portrait = assets.generic_portrait;
        }
        if let Some(rect) = assets.generic_window_rect {
            self.generic_window_rect = rect;
        }
        self.window_tiles.extend(assets.window_tiles);
        self.pictures.extend(assets.pictures);

        let mut text = TextPlaneLayer::new_alloc();
        text.bind_mut().configure(pack_assets, set.clone());
        self.base_mut().add_child(&text);
        self.text_layer = Some(text);
    }

    pub(crate) fn place(&mut self) {
        let position = crate::retail_surface::surface_origin(
            self.base().get_canvas_transform().affine_inverse(),
            self.base().get_viewport_rect(),
        );
        self.base_mut().set_position(position);
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
