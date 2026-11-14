//! Drawing the plane-A picture a scene draws itself, outside `Panel_Create`.
//!
//! `Event_MeetingLeRoof` and `Cutscene_LeRoofAgain` decompress the galaxy
//! (`LoadArt` of its patterns, `LoadSceneAsset` of its Enigma cells), copy the
//! cells to plane A at (0,0), clear plane B and DMA both: the party then
//! walks on the picture, not the room. The pack decodes each such picture
//! (`presentation/panels.json`, `scene_pictures`) with its pixels as CRAM
//! slots, so it draws from the live palette like the map does, and a colour
//! cycle reaches it on the frame the runtime's CRAM model applies it.
//!
//! Which picture shows is [`PicturePlane`]'s, from the ordered ops. It sits
//! where the field map does, under the sprites: a child of the scene layer
//! drawn at an absolute z between the map and the party.

use godot::classes::{ImageTexture, Sprite2D};
use godot::prelude::*;
use psiv_core::SceneOp;

use crate::cram_art::IndexedArt;

use super::CutsceneLayer;
use super::picture_plane::{PictureChange, PicturePlane};

/// Above the field map (z 0), below the party, vehicle and NPCs (z 5) and
/// the map's high-priority overlay (z 20), which the picture hides anyway.
const PICTURE_Z: i32 = 1;

/// One decoded picture: its PNG, the `LoadArt` its cells index into and its
/// texture in the file's baked colours.
pub(super) struct PictureAsset {
    pub(super) path: String,
    pub(super) art: (u32, u16),
    pub(super) baked: Option<Gd<ImageTexture>>,
}

impl CutsceneLayer {
    /// Applies one scene op to the picture plane, before the op's other
    /// presentation. `art_loaded` says whether the scene ran a `LoadArt`.
    pub(crate) fn consume_picture_op(
        &mut self,
        op: &SceneOp,
        art_loaded: impl Fn(u32, u16) -> bool,
    ) -> PictureChange {
        let before = self.picture;
        let pictures = &self.pictures;
        let change = self.picture.consume(
            op,
            |source| pictures.get(&source).map(|picture| picture.art),
            art_loaded,
        );
        match change {
            PictureChange::Staged(source) => {
                godot_print!("scene picture staged: {source:#08x}");
            }
            PictureChange::ArtMissing { source, art, tile } => {
                godot_error!(
                    "scene picture {source:#08x} needs LoadArt {art:#08x} at tile {tile:#05x} first"
                );
            }
            PictureChange::Cleared if before != PicturePlane::default() => {
                godot_print!("scene picture cleared");
            }
            _ => {}
        }
        if self.picture.visible != before.visible {
            self.sync_picture();
        }
        change
    }

    /// Whether plane A shows a picture.
    pub(crate) fn picture_visible(&self) -> bool {
        self.picture.visible.is_some()
    }

    /// The planes are replaced outside the scene's own ops (the scene's
    /// end, a field reload, a map change, the return to title): no picture
    /// is staged or shown, and no panel is. A later DMA revives neither.
    /// Every such teardown takes this one call, so none can drop the panels
    /// and keep the picture.
    pub(crate) fn clear_planes(&mut self) {
        if self.picture != PicturePlane::default() {
            godot_print!("scene picture cleared");
        }
        self.picture.clear();
        self.sync_picture();
        self.panel_destroy_all();
    }

    /// Puts the visible picture, in the live palette, on the picture sprite.
    pub(super) fn sync_picture(&mut self) {
        let texture = self.picture.visible.and_then(|source| {
            self.live_picture(source)
                .or_else(|| self.pictures.get(&source)?.baked.clone())
        });
        let sprite = self.picture_sprite();
        match texture {
            Some(texture) => {
                sprite.set_texture(&texture);
                sprite.set_visible(true);
            }
            None => sprite.set_visible(false),
        }
    }

    fn picture_sprite(&mut self) -> &mut Gd<Sprite2D> {
        if self.picture_sprite.is_none() {
            let mut sprite = Sprite2D::new_alloc();
            sprite.set_centered(false);
            sprite.set_z_as_relative(false);
            sprite.set_z_index(PICTURE_Z);
            sprite.set_visible(false);
            self.base_mut().add_child(&sprite);
            self.picture_sprite = Some(sprite);
        }
        self.picture_sprite.as_mut().expect("created above")
    }

    /// Picture `source` in the live palette, its indices read on its first
    /// draw; `None` while no live palette is known or the file did not load.
    fn live_picture(&mut self, source: u32) -> Option<Gd<ImageTexture>> {
        let cram = self.cram.clone()?;
        let cram = cram.borrow();
        if !cram.live {
            return None;
        }
        if !self.picture_art.contains_key(&source) {
            let art = self.pictures.get(&source).and_then(|picture| {
                IndexedArt::load(&picture.path, psiv_data::color::IndexLayout::Plane)
            });
            self.picture_art.insert(source, art);
        }
        self.picture_art.get_mut(&source)?.as_mut()?.texture(&cram)
    }
}
