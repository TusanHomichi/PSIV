//! A scene's own plane-A picture, staged and shown by the scene ops,
//! independent of Godot (`picture.rs` draws it).
//!
//! `LoadSceneAsset` of a picture's cells stages it once the `LoadArt` its
//! cells index has run; `DmaPlanes` shows what is staged; a plane redraw
//! (`InitVramAndCram`, `ReloadMapChunks`: `RefreshPlaneFG`/`BG`) clears it.

use psiv_core::{PresentationOp, SceneOp};

/// Staged and visible picture, keyed by the ROM address of its cells.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PicturePlane {
    pub(crate) staged: Option<u32>,
    pub(crate) visible: Option<u32>,
}

/// What one scene op did to the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PictureChange {
    /// The op does not touch plane A's picture.
    None,
    /// `LoadSceneAsset` of a decoded picture's cells: staged for the next DMA.
    Staged(u32),
    /// A decoded picture's cells were loaded before the art they index.
    ArtMissing { source: u32, art: u32, tile: u16 },
    /// `DmaPlanes`: plane A shows the staged picture, if any.
    Dma,
    /// The planes were redrawn: nothing is staged or shown.
    Cleared,
}

impl PicturePlane {
    /// Applies `op`. `picture_art` names the `LoadArt` (source, tile) a
    /// decoded picture's cells need, `None` for an asset that is not a
    /// picture; `art_loaded` says whether the scene ran that `LoadArt`.
    pub(crate) fn consume(
        &mut self,
        op: &SceneOp,
        picture_art: impl Fn(u32) -> Option<(u32, u16)>,
        art_loaded: impl Fn(u32, u16) -> bool,
    ) -> PictureChange {
        match op {
            SceneOp::Presentation {
                op:
                    PresentationOp::LoadSceneAsset {
                        source_rom_addr, ..
                    },
            } => match picture_art(*source_rom_addr) {
                Some((art, tile)) if art_loaded(art, tile) => {
                    self.staged = Some(*source_rom_addr);
                    PictureChange::Staged(*source_rom_addr)
                }
                Some((art, tile)) => PictureChange::ArtMissing {
                    source: *source_rom_addr,
                    art,
                    tile,
                },
                None => PictureChange::None,
            },
            SceneOp::DmaPlanes => {
                self.visible = self.staged;
                PictureChange::Dma
            }
            SceneOp::InitVramAndCram
            | SceneOp::Presentation {
                op: PresentationOp::ReloadMapChunks,
            } => {
                self.clear();
                PictureChange::Cleared
            }
            _ => PictureChange::None,
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}
