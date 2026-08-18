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

/// `battle/art/enemy_attacks.json`: exact attack-object frame sheets. The
/// index also carries partial/deferred enemies with no frame paths, so a
/// missing asset cannot silently become a guessed animation.
#[derive(Clone, Deserialize)]
pub(super) struct EnemyAttackArtFile {
    pub(super) enemies: Vec<EnemyAttackArtFileEntry>,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyAttackArtFileEntry {
    pub(super) id: u16,
    pub(super) status: String,
    pub(super) movement: EnemyAttackMovementFile,
    #[serde(default)]
    pub(super) origin_pixels: Option<[i32; 2]>,
    #[serde(default)]
    pub(super) frames: Vec<EnemyAttackFrameFile>,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyAttackMovementFile {
    pub(super) status: String,
    pub(super) runtime: EnemyAttackRuntimeMotion,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyAttackRuntimeMotion {
    pub(super) kind: String,
    pub(super) initial_offset_pixels: [i32; 2],
    pub(super) step_pixels: [i32; 2],
    #[serde(default)]
    pub(super) limit_offset_pixels: Option<[i32; 2]>,
}

#[derive(Clone, Deserialize)]
pub(super) struct EnemyAttackFrameFile {
    pub(super) index: usize,
    pub(super) png: String,
    pub(super) duration: u8,
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

/// Godot-side attack layer. The PNGs are line-relative and are recoloured by
/// `BattleArt`, just like the enemy body; this object owns only the texture
/// sequence and normalized motion track.
#[derive(Clone)]
pub(super) struct AttackAnimation {
    pub(super) frames: Vec<Gd<ImageTexture>>,
    pub(super) origin_pixels: Vector2i,
    pub(super) initial_offset_pixels: Vector2i,
    pub(super) step_pixels: Vector2i,
    pub(super) limit_offset_pixels: Option<Vector2i>,
}

/// The live animation state for one enemy node.
pub(super) struct EnemyAnimation {
    base: Gd<Image>,
    pieces: Vec<RuntimePiece>,
    output: Gd<Image>,
    texture: Gd<ImageTexture>,
    attack: Option<AttackAnimation>,
}

impl EnemyAnimation {
    pub(super) fn new(
        base: Gd<Image>,
        pieces: Vec<AnimationPiece>,
        attack: Option<AttackAnimation>,
        initial_phase: usize,
    ) -> Option<Self> {
        if pieces.is_empty() && attack.is_none() {
            return None;
        }
        let runtime = pieces
            .into_iter()
            .map(|piece| {
                let frame = initial_phase % piece.frames.len().max(1);
                RuntimePiece {
                    initial: piece.initial,
                    frames: piece.frames,
                    durations: piece.durations,
                    placements: piece.placements,
                    frame,
                    timer: 0,
                }
            })
            .collect();
        let mut animation = Self {
            output: base.clone(),
            texture: ImageTexture::create_from_image(&base)?,
            base,
            pieces: runtime,
            attack,
        };
        animation.rebuild()?;
        Some(animation)
    }

    pub(super) fn texture(&self) -> Gd<ImageTexture> {
        self.texture.clone()
    }

    pub(super) fn attack_spec(&self) -> Option<AttackAnimation> {
        self.attack.clone()
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
