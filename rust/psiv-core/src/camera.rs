//! The field camera, as the cartridge computes it.
//!
//! This is not a rendering convenience. The camera decides which field objects
//! are on screen, `FieldObj_OnScreenTest` freezes every off-screen object
//! completely, and a frozen object draws no `UpdateRNGSeed` roll — so the
//! camera position feeds back into the shared random stream and through it into
//! everything else that reads the seed. A camera that is off by sixteen pixels
//! desynchronises the whole simulation, which is exactly what a leader-centred
//! placeholder did before this module existed.
//!
//! # The mechanism
//!
//! The camera does **not** re-centre on the party. `FieldObj_CameraYPos_FG`
//! (`ps4.asm:89590`) and `FieldObj_CameraXPos_FG` (`ps4.asm:89548`) implement a
//! one-sided latch:
//!
//! ```text
//! clr.l   Camera_Y_Step_Counter_FG        ; default: do not scroll
//! if y_step_constant <  0 and sprite_y <= $D8: step = y_step_constant
//! if y_step_constant >= 0 and sprite_y >= $D8: step = y_step_constant
//! ```
//!
//! The camera moves at exactly the driver's own velocity, and only while the
//! driver is past the threshold in its direction of travel. There is no
//! restoring force. While the camera follows, the driver's *screen* position is
//! frozen wherever it happens to be — so the leader is not pinned to the centre
//! of the view, and its screen offset is path-dependent. No constant offset
//! reproduces this.
//!
//! The thresholds are `$118` (x) and `$D8` (y) in sprite space, where `$80` is
//! screen pixel 0, so the driver's home offset is (152, 88) from the top-left
//! of the view. `Event_MoveCamera` (`ps4.asm:121468`) independently confirms
//! those two numbers: it converts a target position to a camera position by
//! subtracting `$98` = 152 and `$58` = 88.
//!
//! # Fixed point
//!
//! Positions and velocities are 16.16 throughout, exactly as the cartridge
//! stores them: `curr_x_pos` (`$30`) and `Camera_X_Pos_FG` (`$FFFFEF94`) are
//! longwords whose high word is the integer pixel. Keeping the low word matters
//! because wanderers move half a pixel per frame.

use crate::geom::Cell;

/// One pixel, in the 16.16 fixed point the cartridge uses for positions.
pub const ONE_PIXEL: i32 = 0x0001_0000;

/// The visible display: 320x224, the Mega Drive's H40 mode.
pub const SCREEN_WIDTH: i32 = 0x140;
/// The visible display height.
pub const SCREEN_HEIGHT: i32 = 0xE0;

/// Sprite coordinate of screen pixel 0. Sega's sprite plane origin.
pub const SPRITE_ORIGIN: i32 = 0x80;

/// The x threshold the camera latches on, in sprite space (`$118`).
pub const THRESHOLD_X: i32 = 0x118;
/// The y threshold the camera latches on, in sprite space (`$D8`).
pub const THRESHOLD_Y: i32 = 0xD8;

/// The driver's resting offset from the top-left of the view: `$118 - $80`.
///
/// `Event_MoveCamera` spells the same number as `$98`.
pub const HOME_X: i32 = THRESHOLD_X - SPRITE_ORIGIN;
/// The driver's resting vertical offset: `$D8 - $80`, and `Event_MoveCamera`'s
/// `$58`.
pub const HOME_Y: i32 = THRESHOLD_Y - SPRITE_ORIGIN;

/// How the camera treats the map's edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraEdges {
    /// Clamp to `[0, map - screen]`, and zero the scroll on contact.
    ///
    /// `UpdateCameraYPosFG` (`ps4.asm:90317`); the maximum comes from
    /// `loc_45780` as `(Map_Column_Size_FG + 1) * 32 - $E0`, which is the map's
    /// pixel size less the screen.
    Clamped,
    /// Wrap modulo the map's pixel size, never clamping.
    ///
    /// The overworlds. `loc_45640` / `loc_45686` add the screen size back onto
    /// the clamp maximum to recover the full map extent, then wrap into it.
    Wrapping,
}

/// The map extent the camera lives in, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraBounds {
    /// Map width in pixels.
    pub width: i32,
    /// Map height in pixels.
    pub height: i32,
    /// Edge behaviour.
    pub edges: CameraEdges,
}

impl Default for CameraBounds {
    /// A degenerate clamped map, so [`Camera`] can derive [`Default`]. Every
    /// real camera is constructed against a map.
    fn default() -> CameraBounds {
        CameraBounds {
            width: 0,
            height: 0,
            edges: CameraEdges::Clamped,
        }
    }
}

impl CameraBounds {
    /// Bounds for a map of `width` x `height` cells of 16 pixels.
    #[must_use]
    pub const fn from_cells(width: u16, height: u16, edges: CameraEdges) -> CameraBounds {
        CameraBounds {
            width: width as i32 * 16,
            height: height as i32 * 16,
            edges,
        }
    }

    /// The largest camera x a clamped map allows: `map - screen`, floored at 0
    /// for maps narrower than the view.
    #[must_use]
    pub const fn max_x(&self) -> i32 {
        let max = self.width - SCREEN_WIDTH;
        if max < 0 { 0 } else { max }
    }

    /// The largest camera y a clamped map allows.
    #[must_use]
    pub const fn max_y(&self) -> i32 {
        let max = self.height - SCREEN_HEIGHT;
        if max < 0 { 0 } else { max }
    }
}

/// The object the camera follows, at its 16.16 position.
///
/// Only the position is supplied. The cartridge latches on `x_step_constant`
/// (`$20`), but `FieldObj_UpdatePosition` is exactly `curr_pos += step_constant`
/// and it runs before the camera routine, so this frame's position delta *is*
/// the step constant. Deriving it here rather than accepting it removes a way
/// for a caller to be subtly wrong — an early version read the party's
/// "is stepping" flag, which goes false on the frame a step lands even though
/// the party still moved that frame, and the camera lost a frame per cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Driver {
    /// `curr_x_pos` (`$30`).
    pub x: i32,
    /// `curr_y_pos` (`$34`).
    pub y: i32,
}

impl Driver {
    /// A driver standing still on `cell`.
    ///
    /// Uses the same standing-cell convention as the rest of the engine: the
    /// occupied cell is one row below `curr_y_pos / 16`, so the inverse
    /// subtracts it back.
    #[must_use]
    pub const fn at_cell(cell: Cell) -> Driver {
        Driver {
            x: cell.x as i32 * 16 * ONE_PIXEL,
            y: (cell.y as i32 - 1) * 16 * ONE_PIXEL,
        }
    }
}

/// The field camera.
///
/// Holds `Camera_*_Pos_FG` and `Camera_*_Step_Counter_FG` separately because
/// the cartridge does, and because the difference is observable: sprite
/// positions are computed from **pos + step**, while the commit that folds step
/// into pos happens at a different point in the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Camera {
    /// `Camera_X_Pos_FG`, 16.16.
    pos_x: i32,
    /// `Camera_Y_Pos_FG`, 16.16.
    pos_y: i32,
    /// `Camera_X_Step_Counter_FG`, 16.16: this frame's scroll, not yet folded
    /// into the position.
    step_x: i32,
    /// `Camera_Y_Step_Counter_FG`, 16.16.
    step_y: i32,
    /// The driver's `sprite_x_pos` (`$2C`) as written by the last
    /// `FieldObj_CalcSpritePos`. The latch tests this, and it is a frame old by
    /// construction.
    driver_sprite_x: i32,
    /// The driver's `sprite_y_pos` (`$2E`).
    driver_sprite_y: i32,
    /// The driver's position last frame, which is how the velocity the latch
    /// wants is recovered.
    driver_x: i32,
    /// The driver's vertical position last frame.
    driver_y: i32,
    /// The map this camera lives in. A camera without its map cannot compute a
    /// sprite position at all on the overworlds, where the seam is part of the
    /// arithmetic.
    bounds: CameraBounds,
}

impl Camera {
    /// The camera as a map places it: the driver at its home offset, then the
    /// map's edge rule applied.
    ///
    /// Map entry has no scroll-in — the view is simply positioned — so this is
    /// also the state a replay must start from at its alignment frame.
    #[must_use]
    pub fn placed_on(driver: Driver, bounds: CameraBounds) -> Camera {
        let mut camera = Camera {
            pos_x: driver.x - HOME_X * ONE_PIXEL,
            pos_y: driver.y - HOME_Y * ONE_PIXEL,
            bounds,
            driver_x: driver.x,
            driver_y: driver.y,
            ..Camera::default()
        };
        camera.settle();
        camera.refresh_driver_sprite(driver);
        camera
    }

    /// An unplaced camera on `bounds`, at the map's origin.
    #[must_use]
    pub fn new(bounds: CameraBounds) -> Camera {
        Camera {
            bounds,
            ..Camera::default()
        }
    }

    /// The map this camera lives in.
    #[must_use]
    pub const fn bounds(&self) -> CameraBounds {
        self.bounds
    }

    /// The camera position in whole pixels, as the renderer wants it.
    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        (self.pos_x >> 16, self.pos_y >> 16)
    }

    /// The camera sprite positions are actually computed against: pos + step.
    ///
    /// `FieldObj_CalcSpritePos` subtracts both, so this is the value that
    /// decides visibility, and it leads [`Camera::position`] by one commit.
    #[must_use]
    pub const fn effective(&self) -> (i32, i32) {
        (
            (self.pos_x + self.step_x) >> 16,
            (self.pos_y + self.step_y) >> 16,
        )
    }

    /// The raw 16.16 position, for tests and for diffing against oracle RAM.
    #[must_use]
    pub const fn raw(&self) -> (i32, i32) {
        (self.pos_x, self.pos_y)
    }

    /// The raw 16.16 step counters.
    #[must_use]
    pub const fn raw_step(&self) -> (i32, i32) {
        (self.step_x, self.step_y)
    }

    /// Places the camera without the home offset, for scenes that park it.
    ///
    /// `SceneOp::SetCameraPos` hands an absolute camera position rather than a
    /// subject to frame.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.pos_x = x * ONE_PIXEL;
        self.pos_y = y * ONE_PIXEL;
        self.step_x = 0;
        self.step_y = 0;
        self.settle();
    }

    /// Re-baselines the driver without scrolling, so the next tick reads a
    /// velocity of zero rather than a teleport.
    ///
    /// A warp or a scene that moves the party without walking it there must
    /// call this, or the camera will chase a one-frame velocity of the whole
    /// jump.
    pub fn reseat(&mut self, driver: Driver) {
        self.driver_x = driver.x;
        self.driver_y = driver.y;
        self.refresh_driver_sprite(driver);
    }

    /// One frame, in the cartridge's own order.
    ///
    /// `Event_MoveCamera` shows the field loop's sequence plainly: the commit
    /// (`UpdateCameraXPosFG`) runs *before* `Field_UpdateObjects`, so a frame
    /// folds last frame's scroll into the position first, then decides this
    /// frame's scroll from the driver's sprite position — which is itself a
    /// frame old, having been written by the previous frame's
    /// `FieldObj_CalcSpritePos`.
    pub fn tick(&mut self, driver: Driver) {
        // 1. FieldObj_Camera*Pos_FG: latch this frame's scroll off the driver's
        //    velocity, gated on its sprite position — a frame old, written by
        //    the previous CalcSpritePos — being past the threshold in the
        //    direction of travel.
        let vx = driver.x - self.driver_x;
        let vy = driver.y - self.driver_y;
        self.driver_x = driver.x;
        self.driver_y = driver.y;
        self.step_x = latch(vx, self.driver_sprite_x, THRESHOLD_X);
        self.step_y = latch(vy, self.driver_sprite_y, THRESHOLD_Y);

        // 2. UpdateCamera*PosFG: fold this frame's scroll in, then apply the
        //    map's edge rule. The commit lands *after* the latch, not before:
        //    the oracle's camera columns hold `leader - camera` at exactly
        //    (152, 88) on all 1563 field-control frames of tape 02, with no
        //    frame of lag anywhere, which only the latch-then-commit order
        //    produces. (`Event_MoveCamera` commits before its object pass, but
        //    that is a scene helper driving its own loop, not the field one.)
        self.pos_x += self.step_x;
        self.pos_y += self.step_y;
        self.settle();

        // 3. FieldObj_CalcSpritePos: every object, on screen or not, recomputes
        //    its sprite position. The driver's is what the latch reads next
        //    frame.
        self.refresh_driver_sprite(driver);
    }

    /// The sprite-space position of an object at 16.16 map position `(x, y)`.
    ///
    /// `FieldObj_CalcSpritePos` subtracts the camera position and the step
    /// counter, but it runs before the commit that folds the step into the
    /// position — so `pos_before + step` is the same number as the committed
    /// position this type already holds, and subtracting it again would double
    /// the scroll.
    #[must_use]
    pub const fn sprite_pos(&self, x: i32, y: i32) -> (i32, i32) {
        let cam_x = self.pos_x >> 16;
        let cam_y = self.pos_y >> 16;
        match self.bounds.edges {
            CameraEdges::Clamped => (
                (x >> 16) - cam_x + SPRITE_ORIGIN,
                (y >> 16) - cam_y + SPRITE_ORIGIN,
            ),
            CameraEdges::Wrapping => (
                across_seam(x >> 16, cam_x, self.bounds.width),
                across_seam(y >> 16, cam_y, self.bounds.height),
            ),
        }
    }

    /// Whether an object at 16.16 map position `(x, y)` is on screen.
    #[must_use]
    pub const fn sees(&self, x: i32, y: i32) -> bool {
        let (sx, sy) = self.sprite_pos(x, y);
        on_screen(sx, sy)
    }

    /// Applies the map's edge rule to the position, zeroing the scroll on a
    /// clamp exactly as `UpdateCameraYPosFG` does.
    fn settle(&mut self) {
        let bounds = self.bounds;
        match bounds.edges {
            CameraEdges::Clamped => {
                if self.pos_x < 0 {
                    self.pos_x = 0;
                    self.step_x = 0;
                } else if self.pos_x > bounds.max_x() * ONE_PIXEL {
                    self.pos_x = bounds.max_x() * ONE_PIXEL;
                    self.step_x = 0;
                }
                if self.pos_y < 0 {
                    self.pos_y = 0;
                    self.step_y = 0;
                } else if self.pos_y > bounds.max_y() * ONE_PIXEL {
                    self.pos_y = bounds.max_y() * ONE_PIXEL;
                    self.step_y = 0;
                }
            }
            CameraEdges::Wrapping => {
                self.pos_x = self.pos_x.rem_euclid(bounds.width * ONE_PIXEL);
                self.pos_y = self.pos_y.rem_euclid(bounds.height * ONE_PIXEL);
            }
        }
    }

    fn refresh_driver_sprite(&mut self, driver: Driver) {
        let (sx, sy) = self.sprite_pos(driver.x, driver.y);
        self.driver_sprite_x = sx;
        self.driver_sprite_y = sy;
    }
}

/// The one-sided latch, per axis.
///
/// Returns the scroll for this frame: the driver's own velocity when it is past
/// `threshold` in its direction of travel, and zero otherwise. A stationary
/// driver takes the non-negative branch with a velocity of zero, which scrolls
/// nothing either way.
const fn latch(velocity: i32, driver_sprite: i32, threshold: i32) -> i32 {
    if velocity < 0 {
        // Moving up or left: `bgt` skips the scroll, so the threshold itself
        // still scrolls.
        if driver_sprite <= threshold {
            velocity
        } else {
            0
        }
    } else {
        // `bcs` skips below the threshold, so again the threshold scrolls.
        if driver_sprite >= threshold {
            velocity
        } else {
            0
        }
    }
}

/// One axis of the wrapping `FieldObj_CalcSpritePos` (`loc_4509C`,
/// `ps4.asm:89860`).
///
/// On a toroidal map an object near the origin and a camera near the far edge
/// are neighbours, so a plain subtraction would put the object a whole map
/// away. The cartridge decides with one comparison — `lsr.w #1` on the camera,
/// then `cmp`/`bgt` — which reads as: if the object sits in the near half of
/// the space behind the camera, it is really ahead of it across the seam, so
/// shift the camera back by a full map before subtracting.
const fn across_seam(obj: i32, camera: i32, extent: i32) -> i32 {
    let obj = obj.rem_euclid(extent);
    let camera = if obj < camera / 2 {
        camera - extent
    } else {
        camera
    };
    obj - camera + SPRITE_ORIGIN
}

/// The `FieldObjectsJmpTbl` offsets whose routine calls `FieldObj_OnScreenTest`.
///
/// Visibility is **not** universal. Whether an object is frozen off screen is a
/// property of its object type, because the test is a call each routine makes
/// or does not make. The generic townsfolk types do call it — `NPCType1`
/// (`$38`) through `NPCType12` (`$64`), which includes the two wanderers
/// `NPCType2` (`$3C`) and `NPCType3` (`$40`). The party members and the named
/// story NPCs do not: `FieldObj_NPCAlysPiata` (`$68`, `ps4.asm:92120`) runs
/// straight into `FieldObj_NPCMove`, `UpdateStepDuration`, `UpdatePosition` and
/// `CalcSpritePos` with no test at all, so its `offscreen_flag` keeps the value
/// its slot was initialised with, forever, however far off screen it drifts.
///
/// Tape 02 shows exactly that: Alys's sprite x crosses below `$60` at frame
/// 7303 and her flag stays 0.
///
/// Derived mechanically from the disassembly — each jump table entry's body,
/// from its label to the next `FieldObj_` label, scanned for the call. 87 of
/// the 222 entries qualify.
const VISIBILITY_GATED_TYPES: [u16; 87] = [
    0x0038, 0x003C, 0x0040, 0x0044, 0x0048, 0x004C, 0x0050, 0x0054, 0x0058, 0x005C, 0x0060, 0x0064,
    0x0074, 0x0088, 0x008C, 0x0090, 0x0094, 0x0098, 0x009C, 0x00A0, 0x00A4, 0x00A8, 0x00AC, 0x00BC,
    0x00C0, 0x00C4, 0x00C8, 0x00CC, 0x00F4, 0x00F8, 0x0100, 0x0104, 0x010C, 0x0110, 0x0114, 0x0118,
    0x011C, 0x0128, 0x012C, 0x0130, 0x0134, 0x0138, 0x013C, 0x0144, 0x0148, 0x0154, 0x0158, 0x015C,
    0x0160, 0x0164, 0x0168, 0x016C, 0x0170, 0x0174, 0x0178, 0x017C, 0x0180, 0x0184, 0x01B4, 0x01D4,
    0x01E8, 0x0228, 0x022C, 0x0230, 0x0234, 0x0238, 0x023C, 0x0240, 0x0244, 0x0248, 0x024C, 0x0250,
    0x0258, 0x0260, 0x0268, 0x026C, 0x02F4, 0x0304, 0x0308, 0x030C, 0x0310, 0x0314, 0x0318, 0x031C,
    0x0320, 0x0324, 0x0328,
];

/// Whether an object of this type is frozen when it leaves the screen.
///
/// `id` is the object's type id as the pack stores it; the dispatcher masks it
/// with `$7FFC` to index `FieldObjectsJmpTbl`, so the same mask applies here.
#[must_use]
pub fn type_tests_visibility(id: u16) -> bool {
    VISIBILITY_GATED_TYPES.contains(&(id & 0x7FFC))
}

/// `FieldObj_OnScreenTest` (`ps4.asm:96661`), in sprite coordinates.
///
/// The box is the 320x224 view plus a 32-pixel margin on all four sides. A
/// coordinate of exactly zero short-circuits to on-screen on either axis
/// independently — the cartridge tests `beq` before the range at all, so an
/// object whose sprite x lands on 0 is on screen no matter where its y is.
#[must_use]
pub const fn on_screen(sprite_x: i32, sprite_y: i32) -> bool {
    if sprite_x == 0 {
        return true;
    }
    if sprite_x < 0x60 || sprite_x > 0x1E0 {
        return false;
    }
    if sprite_y == 0 {
        return true;
    }
    sprite_y >= 0x60 && sprite_y <= 0x180
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACADEMY: CameraBounds = CameraBounds {
        width: 1024,
        height: 512,
        edges: CameraEdges::Clamped,
    };

    fn px(v: i32) -> i32 {
        v * ONE_PIXEL
    }

    #[test]
    fn the_home_offset_is_the_two_numbers_the_rom_states_twice() {
        // FieldObj_Camera*Pos_FG thresholds, less the sprite origin.
        assert_eq!(HOME_X, 152);
        assert_eq!(HOME_Y, 88);
        // Event_MoveCamera subtracts exactly these to turn a subject position
        // into a camera position.
        assert_eq!(HOME_X, 0x98);
        assert_eq!(HOME_Y, 0x58);
    }

    #[test]
    fn a_placed_camera_puts_the_driver_at_its_home_offset() {
        let driver = Driver {
            x: px(800),
            y: px(240),
        };
        let camera = Camera::placed_on(driver, ACADEMY);
        assert_eq!(camera.position(), (800 - HOME_X, 240 - HOME_Y));
        assert_eq!(
            camera.sprite_pos(driver.x, driver.y),
            (THRESHOLD_X, THRESHOLD_Y)
        );
    }

    #[test]
    fn a_placed_camera_clamps_into_a_small_map_instead_of_showing_the_void() {
        let driver = Driver::at_cell(Cell::new(1, 1));
        let camera = Camera::placed_on(driver, ACADEMY);
        assert_eq!(camera.position(), (0, 0), "clamped at the top-left corner");
    }

    #[test]
    fn following_holds_the_driver_at_the_threshold_rather_than_centring_it() {
        // y 300 keeps the camera off both clamps, so this is the latch alone.
        let mut driver = Driver {
            x: px(800),
            y: px(300),
        };
        let mut camera = Camera::placed_on(driver, ACADEMY);
        assert_eq!(camera.sprite_pos(driver.x, driver.y).1, THRESHOLD_Y);
        // Move first, then tick, so the driver the assertions read is the one
        // the last tick actually saw.
        for _ in 0..20 {
            driver.y -= px(2);
            camera.tick(driver);
        }
        assert_eq!(
            camera.sprite_pos(driver.x, driver.y).1,
            THRESHOLD_Y,
            "held at the threshold, not re-centred"
        );
        // It really did scroll rather than sit still.
        assert_eq!(camera.position().1, 300 - HOME_Y - 2 * 20);
    }

    #[test]
    fn the_camera_tracks_the_driver_with_no_frame_of_lag() {
        // The oracle's camera columns hold `leader - camera` at exactly
        // (152, 88) on all 1563 field-control frames of tape 02 — one distinct
        // offset, no lag anywhere. That is only true if the commit lands after
        // the latch inside a frame, and it is the sharpest available check on
        // the ordering.
        let mut driver = Driver {
            x: px(800),
            y: px(300),
        };
        let mut camera = Camera::placed_on(driver, ACADEMY);
        for _ in 0..24 {
            driver.y -= px(2);
            camera.tick(driver);
            let (cx, cy) = camera.position();
            assert_eq!(
                ((driver.x >> 16) - cx, (driver.y >> 16) - cy),
                (HOME_X, HOME_Y),
                "the offset never varies while the camera is unclamped"
            );
        }
    }

    #[test]
    fn a_driver_short_of_the_threshold_walks_across_a_still_camera() {
        // Placed at the top-left clamp, the driver is above its home offset, so
        // walking down does not move the camera until it reaches the threshold.
        let mut driver = Driver {
            x: px(160),
            y: px(16),
        };
        let mut camera = Camera::placed_on(driver, ACADEMY);
        let before = camera.position();
        for _ in 0..8 {
            driver.y += px(2);
            camera.tick(driver);
        }
        assert_eq!(camera.position(), before, "camera has not moved");
        assert!(
            camera.sprite_pos(driver.x, driver.y).1 > SPRITE_ORIGIN,
            "but the driver has walked down the screen"
        );
    }

    #[test]
    fn a_clamped_edge_stops_the_camera_dead_however_long_the_driver_walks() {
        let mut driver = Driver {
            x: px(800),
            y: px(120),
        };
        let mut camera = Camera::placed_on(driver, ACADEMY);
        for _ in 0..64 {
            camera.tick(driver);
            driver.y -= px(2);
        }
        assert_eq!(camera.position().1, 0, "pinned at the top of the map");
        // The clamp is re-applied every frame rather than latched once, so a
        // driver that keeps walking never drags the view off the map.
        for _ in 0..64 {
            camera.tick(driver);
            driver.y -= px(2);
            assert_eq!(camera.position().1, 0);
        }
    }

    #[test]
    fn a_wrapping_camera_comes_back_around_the_overworld() {
        let bounds = CameraBounds {
            width: 2048,
            height: 2048,
            edges: CameraEdges::Wrapping,
        };
        let mut camera = Camera::new(bounds);
        camera.set_position(16, 16);
        let mut driver = Driver {
            x: px(16 + HOME_X),
            y: px(16 + HOME_Y),
        };
        // A camera parked rather than walked to needs its driver baseline set,
        // or the first tick reads the whole jump as one frame of velocity.
        camera.reseat(driver);
        for _ in 0..20 {
            driver.x -= px(2);
            camera.tick(driver);
        }
        // Walked left off x=0 and came back around the far side.
        assert_eq!(camera.position().0, 2024, "wrapped, never clamped");
        assert_eq!(
            camera.sprite_pos(driver.x, driver.y).0,
            THRESHOLD_X,
            "and the driver is still held at the threshold across the seam"
        );
    }

    #[test]
    fn the_on_screen_box_is_the_view_plus_thirty_two_pixels() {
        // Dead centre.
        assert!(on_screen(0x140, 0x120));
        // The margin's four edges, inclusive.
        assert!(on_screen(0x60, 0x100));
        assert!(on_screen(0x1E0, 0x100));
        assert!(on_screen(0x100, 0x60));
        assert!(on_screen(0x100, 0x180));
        // One step outside each.
        assert!(!on_screen(0x5F, 0x100));
        assert!(!on_screen(0x1E1, 0x100));
        assert!(!on_screen(0x100, 0x5F));
        assert!(!on_screen(0x100, 0x181));
    }

    #[test]
    fn only_the_object_types_whose_routine_calls_the_test_are_frozen() {
        // The generic townsfolk, including both wanderers.
        assert!(type_tests_visibility(0x8038), "NPCType1");
        assert!(type_tests_visibility(0x803C), "NPCType2, a wanderer");
        assert!(
            type_tests_visibility(0x8040),
            "NPCType3, the other wanderer"
        );
        // The party and the named story NPCs never run the test, so they are
        // updated wherever they are.
        assert!(!type_tests_visibility(0x8004), "Chaz");
        assert!(!type_tests_visibility(0x8008), "Alys");
        assert!(!type_tests_visibility(0x8068), "NPCAlysPiata");
        assert!(!type_tests_visibility(0x8070), "NPCRune");
    }

    #[test]
    fn a_sprite_coordinate_of_exactly_zero_short_circuits_to_on_screen() {
        // `move.w sprite_x_pos(a4), d0; beq.w .onscreen` runs before any range
        // check, so a zero on either axis wins outright — even paired with a
        // coordinate that is otherwise far off screen.
        assert!(on_screen(0, 0x9999));
        assert!(on_screen(0x100, 0));
        assert!(on_screen(0, 0));
        // And it really is zero that is special, not "small".
        assert!(!on_screen(1, 0x100));
        assert!(!on_screen(0x100, 1));
    }
}
