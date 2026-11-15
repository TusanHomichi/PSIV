use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use godot::classes::{ImageTexture, Sprite2D};
use godot::prelude::{Gd, Rect2, Vector2};

use crate::cram_art::{CramState, IndexedArt};

use super::PresentationState;

/// A scene-owned sprite sheet, kept separate from map `SheetView`s because it
/// is keyed by the literal ObjectAnimation pair rather than a placed NPC.
///
/// Like a map sheet, it draws its palette indices from the live CRAM
/// ([`crate::cram_art`]) on the line its pack record names; `texture`, the
/// file's baked colours, draws while no live palette is known. Clones share
/// one [`IndexedArt`], so a CRAM revision recolours the sheet once.
#[derive(Clone)]
pub(crate) struct TemporarySpriteAsset {
    pub(super) texture: Gd<ImageTexture>,
    pub(super) indexed: Option<Rc<RefCell<IndexedArt>>>,
    pub(crate) frame_width: i32,
    pub(crate) frame_height: i32,
    pub(crate) origin_x: i32,
    pub(crate) origin_y: i32,
    pub(super) sequences: BTreeMap<String, (Vec<(i32, u32)>, u32)>,
    pub(crate) playback_sequence: Option<String>,
    pub(super) playback_once: bool,
    pub(super) load_art: Option<(u32, u16)>,
}

impl TemporarySpriteAsset {
    pub(crate) fn frame_at(&self, sequence: &str, tick: u64) -> i32 {
        let Some((frames, total)) = self.sequences.get(sequence) else {
            return 0;
        };
        let duration = u64::from((*total).max(1));
        let mut remaining = if self.playback_once {
            tick.min(duration - 1)
        } else {
            tick % duration
        } as u32;
        for (index, duration) in frames {
            if remaining < *duration {
                return *index;
            }
            remaining = remaining.saturating_sub(*duration);
        }
        frames.last().map_or(0, |(index, _)| *index)
    }

    /// Shows `frame` on `node`, in `cram`'s colours when it is live.
    pub(crate) fn apply(&self, node: &mut Gd<Sprite2D>, frame: i32, cram: &CramState) {
        let live = self
            .indexed
            .as_ref()
            .and_then(|art| art.borrow_mut().texture(cram));
        node.set_texture(live.as_ref().unwrap_or(&self.texture));
        node.set_offset(Vector2::new(0.0, -(self.frame_height as f32)));
        node.set_region_enabled(true);
        node.set_region_rect(Rect2::new(
            Vector2::new((frame * self.frame_width) as f32, 0.0),
            Vector2::new(self.frame_width as f32, self.frame_height as f32),
        ));
    }

    pub(crate) fn art_loaded(&self, state: &PresentationState) -> bool {
        self.load_art
            .is_none_or(|key| state.art_loaded(key.0, key.1))
    }
}
