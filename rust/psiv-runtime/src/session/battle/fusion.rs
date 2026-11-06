//! The Zol slugs' Fusion, frame by frame: `BattleObj_Fusion` and
//! `BattleObj_Fusion2` (`ps4.asm:35675-35891`).
//!
//! `EnemyAttack_Blob` sends a Fusion turn to `loc_10796` (`ps4.asm:23062`),
//! which spawns the two objects over the two slugs. Their own frame counter
//! (`$11`) drives everything, with `t` counting frames from the objects' first
//! one:
//!
//! - **Windup** (`loc_1A164`, `ps4.asm:35731-35752`): the standing slug for
//!   six frames, the flat one for six; at `t = 12` the slide begins.
//! - **Slide** (`loc_1A1A8`, `ps4.asm:35753-35796`): each piece moves one pixel
//!   a frame toward x `$120` (screen 160) and counts itself arrived there; the
//!   counter restarts every twelve frames and sets `$2(a4)` bit 7, a horizontal
//!   flip, for its second half. `BattleObj_Fusion` takes the right-hand slug
//!   and runs before `BattleObj_Fusion2`; it moves on once it sees both
//!   arrived, and `BattleObj_Fusion2` deletes itself (`loc_1A382`).
//! - **Growth** (`loc_1A244`, `ps4.asm:35797-35844`): the remaining piece shows
//!   the standing slug, then three growing frames six frames apart, and on its
//!   twentieth frame `loc_14D46` seats the MetaSlug.
//!
//! Oracle tape 33 (formation `$D2`) shows every one of these frames at
//! `t = 0` f25168 through the seat at f25248.

/// The enemy ability a Zol slug's Fusion turn rolls (`EnemyAI_ZolSlugs`,
/// `ps4.asm:23017-23038`).
pub const FUSION_ABILITY: u8 = 0x12;
/// The Zol slug, the only enemy the objects take over: `cmpi.w #$22, $12(a2)`
/// in `BattleObj_Fusion`'s setup (`ps4.asm:35689`).
pub const ZOL_SLUG: u16 = 0x22;
/// Screen x both pieces slide to: `$120` less the sprite plane's `$80`.
pub const TARGET_X: i16 = 0x120 - 0x80;
/// Screen y of both pieces: `move.w #$F8, $2E(a4)` (`ps4.asm:35703`) less `$80`.
pub const PIECE_Y: i16 = 0xF8 - 0x80;
/// The frames the MetaSlug's window holds after the seat (tape 33: seated at
/// f25248, the window starts to close at f25264).
pub const SEATED_HOLD_FRAMES: u32 = 16;

/// The pack's Fusion frames, in the order `BattleObj_Fusion` selects them.
pub mod frame {
    /// `loc_1AA5A`: a standing slug.
    pub const STANDING: u8 = 0;
    /// `loc_1AA74`: a flat slug, sliding.
    pub const FLAT: u8 = 1;
    /// `loc_1AA82`, `loc_1AAA8`, `loc_1AACE`: the MetaSlug growing.
    pub const GROWING: [u8; 3] = [2, 3, 4];
}

/// One piece as the frame draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FusionPiece {
    /// Screen x of the object's position, the frame's origin.
    pub x: i16,
    /// Screen y of the object's position.
    pub y: i16,
    /// Which of the pack's Fusion frames ([`frame`]).
    pub frame: u8,
    /// Whether the sprite is mirrored (`$2(a4)` bit 7).
    pub flipped: bool,
}

/// The Fusion of two slugs standing at screen x `left` and `right`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fusion {
    left: i16,
    right: i16,
}

impl Fusion {
    /// The two slugs' screen x, in either order.
    #[must_use]
    pub fn new(a: i16, b: i16) -> Self {
        Fusion {
            left: a.min(b),
            right: a.max(b),
        }
    }

    /// The frame a piece starting at `x` arrives at [`TARGET_X`]: the slide's
    /// first move is at `t = 12`, and a piece already there counts itself
    /// arrived on that frame.
    fn arrival(x: i16) -> u32 {
        11 + u32::from((x - TARGET_X).unsigned_abs().max(1))
    }

    /// The frame growth begins. `BattleObj_Fusion` (the right-hand piece)
    /// tests for both arrivals after its own move: if it arrives last it sees
    /// both that frame and grows from the next; otherwise it sees the left
    /// piece's arrival one frame later.
    #[must_use]
    pub fn growth_start(self) -> u32 {
        let (left, right) = (Self::arrival(self.left), Self::arrival(self.right));
        if right > left { right + 1 } else { left + 2 }
    }

    /// The frame `loc_14D46` seats the MetaSlug: growth's twentieth.
    #[must_use]
    pub fn seat(self) -> u32 {
        self.growth_start() + 19
    }

    /// The pieces drawn on frame `t`; none from the seat on.
    #[must_use]
    pub fn pieces(self, t: u32) -> Vec<FusionPiece> {
        let growth = self.growth_start();
        if t >= self.seat() {
            return Vec::new();
        }
        let slide_flip = |t: u32| t >= 12 && (t - 12) % 12 >= 6;
        if t >= growth {
            let into = t - growth;
            return vec![FusionPiece {
                x: TARGET_X,
                y: PIECE_Y,
                frame: if into < 6 {
                    frame::STANDING
                } else {
                    frame::GROWING[((into - 6) / 6).min(2) as usize]
                },
                // Growth leaves bit 7 as the slide's last frame set it.
                flipped: slide_flip(growth - 1),
            }];
        }
        let place = |start: i16| {
            let moved = i16::try_from(t.saturating_sub(11)).unwrap_or(i16::MAX);
            let distance = (TARGET_X - start).abs();
            start + (TARGET_X - start).signum() * moved.min(distance)
        };
        let frame = if t < 6 { frame::STANDING } else { frame::FLAT };
        // `BattleObj_Fusion2` (the left piece) deletes itself the frame after
        // it sees both arrived.
        let left_gone = t > Self::arrival(self.left).max(Self::arrival(self.right));
        [(self.left, left_gone), (self.right, false)]
            .into_iter()
            .filter(|(_, gone)| !gone)
            .map(|(start, _)| FusionPiece {
                x: place(start),
                y: PIECE_Y,
                frame,
                flipped: slide_flip(t),
            })
            .collect()
    }
}

/// The screen x of an enemy body at formation `position`: its column's
/// pixel, the body's horizontal centre (`docs/battle/BATTLE_GEOMETRY.md`).
#[must_use]
pub fn body_x(position: u8) -> i16 {
    i16::from(position & 0x7F) * 8
}
