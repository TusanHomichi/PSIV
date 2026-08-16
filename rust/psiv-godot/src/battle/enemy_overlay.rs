//! Runtime composition for retail enemy dynamic-tile replacements.
//!
//! The pack stores one full body-sized image per descriptor frame.  The image
//! is not alpha-stacked over the body wholesale: each descriptor declares the
//! body rectangles whose pattern names point at its destination tile run, and
//! those rectangles are blitted into a copy of the body.  That preserves the
//! VDP's replacement semantics, including transparent colour zero clearing a
//! pixel from the old body.

use godot::builtin::{Rect2i, Vector2i};
use godot::classes::{Image, ImageTexture};
use godot::prelude::*;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub(super) struct EnemyOverlayFile {
    pub(super) enemies: Vec<EnemyOverlayFileEntry>,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyOverlayFileEntry {
    pub(super) id: u16,
    pub(super) pieces: Vec<EnemyOverlayPieceFile>,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyOverlayPieceFile {
    pub(super) enabled: bool,
    pub(super) initial_png: Option<String>,
    pub(super) frames: Vec<EnemyOverlayFrameFile>,
    pub(super) durations: Vec<u8>,
    pub(super) placements: Vec<EnemyOverlayPlacementFile>,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyOverlayFrameFile {
    pub(super) png: String,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyOverlayPlacementFile {
    pub(super) offset_pixels: [i32; 2],
    pub(super) size_pixels: [i32; 2],
}

struct RuntimePiece {
    initial: Gd<Image>,
    frames: Vec<Gd<Image>>,
    durations: Vec<u8>,
    placements: Vec<EnemyOverlayPlacementFile>,
    frame: usize,
    timer: u8,
}

pub(super) struct AnimationPiece {
    pub(super) initial: Gd<Image>,
    pub(super) frames: Vec<Gd<Image>>,
    pub(super) durations: Vec<u8>,
    pub(super) placements: Vec<EnemyOverlayPlacementFile>,
}

/// The live animation state for one enemy node.
pub(super) struct EnemyAnimation {
    base: Gd<Image>,
    pieces: Vec<RuntimePiece>,
    output: Gd<Image>,
    texture: Gd<ImageTexture>,
}

impl EnemyAnimation {
    pub(super) fn new(base: Gd<Image>, pieces: Vec<AnimationPiece>) -> Option<Self> {
        if pieces.is_empty() {
            return None;
        }
        let runtime = pieces
            .into_iter()
            .map(|piece| RuntimePiece {
                initial: piece.initial,
                frames: piece.frames,
                durations: piece.durations,
                placements: piece.placements,
                frame: 0,
                timer: 0,
            })
            .collect();
        let mut animation = Self {
            output: base.clone(),
            texture: ImageTexture::create_from_image(&base)?,
            base,
            pieces: runtime,
        };
        animation.rebuild()?;
        Some(animation)
    }

    pub(super) fn texture(&self) -> Gd<ImageTexture> {
        self.texture.clone()
    }

    /// Advance exactly one game update.  The Python decoder records the
    /// retail timer byte, whose state lasts `duration + 1` update calls.
    pub(super) fn advance(&mut self) -> Option<Gd<ImageTexture>> {
        let mut changed = false;
        for piece in &mut self.pieces {
            if piece.frames.len() <= 1 {
                continue;
            }
            if piece.timer > 0 {
                piece.timer -= 1;
                continue;
            }
            piece.frame = (piece.frame + 1) % piece.frames.len();
            piece.timer = piece
                .durations
                .get(piece.frame)
                .copied()
                .unwrap_or_default();
            changed = true;
        }
        if !changed {
            return None;
        }
        self.rebuild().map(|_| self.texture())
    }

    fn rebuild(&mut self) -> Option<()> {
        let mut image = self.base.duplicate()?.try_cast::<Image>().ok()?;
        for piece in &self.pieces {
            let frame = piece.frames.get(piece.frame).unwrap_or(&piece.initial);
            for placement in &piece.placements {
                let [x, y] = placement.offset_pixels;
                let [width, height] = placement.size_pixels;
                if width <= 0 || height <= 0 || x < 0 || y < 0 {
                    godot_error!(
                        "battle enemy overlay has invalid placement ({x},{y}) {width}x{height}"
                    );
                    return None;
                }
                image.blit_rect(
                    frame,
                    Rect2i::new(Vector2i::new(x, y), Vector2i::new(width, height)),
                    Vector2i::new(x, y),
                );
            }
        }
        self.output = image;
        self.texture = ImageTexture::create_from_image(&self.output)?;
        Some(())
    }
}
