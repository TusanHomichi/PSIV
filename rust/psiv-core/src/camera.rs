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
//! The camera does **not** re-centre on the party. The FG routines
//! `FieldObj_CameraYPos_FG` (`ps4.asm:89590`) and `FieldObj_CameraXPos_FG`
//! (`ps4.asm:89548`) have parallel BG routines at `89670` and `89628`; all
//! four implement the same one-sided latch, with `$EC25`/`$EC26` as separate
//! driver gates:
//!
//! ```text
//! clr.l   Camera_Y_Step_Counter_FG        ; default: do not scroll
//! if y_step_constant <  0 and sprite_y <= $D8: step = y_step_constant
//! if y_step_constant >= 0 and sprite_y >= $D8: step = y_step_constant
//! ```
//!
//! Each enabled plane moves at exactly the driver's own velocity, and only while the
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
//! longwords whose high word is the integer pixel. BG has the parallel
//! `Camera_X_Pos_BG` (`$FFFFEF9C`). Keeping the low word matters
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

/// The cartridge's two camera planes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CameraPlane {
    /// `Camera_*_Pos_FG` and `$EC25`.
    Foreground,
    /// `Camera_*_Pos_BG` and `$EC26`.
    Background,
}

/// The three camera-control bytes used by the cartridge.
///
/// `$EC24` selects which plane `FieldObj_CalcSpritePos` subtracts globally;
/// `$EC25` and `$EC26` independently gate the FG and BG driver latches. The
/// map-load routine fills the bytes before the first field frame. A normal
/// field map selects BG for sprite subtraction and enables both latches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CameraGates {
    /// Global sprite-plane selector: zero is FG, nonzero is BG.
    pub ec24: u8,
    /// FG driver gate.
    pub ec25: u8,
    /// BG driver gate.
    pub ec26: u8,
}

impl CameraGates {
    /// The ordinary map-load state used by field maps in the pack.
    #[must_use]
    pub const fn field_default() -> CameraGates {
        CameraGates {
            ec24: 1,
            ec25: 1,
            ec26: 1,
        }
    }

    /// The plane selected by `$EC24` for sprite subtraction.
    #[must_use]
    pub const fn plane(self) -> CameraPlane {
        if self.ec24 == 0 {
            CameraPlane::Foreground
        } else {
            CameraPlane::Background
        }
    }
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
/// Holds both `Camera_*_Pos_FG`/`BG` and their step counters because the
/// cartridge does. The two latches consume the same driver's sprite position,
/// while `$EC24` chooses which plane object sprite calculation subtracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// `Camera_X_Pos_BG`, 16.16.
    pos_x_bg: i32,
    /// `Camera_Y_Pos_BG`, 16.16.
    pos_y_bg: i32,
    /// `Camera_X_Step_Counter_BG`, 16.16.
    step_x_bg: i32,
    /// `Camera_Y_Step_Counter_BG`, 16.16.
    step_y_bg: i32,
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
    /// BG's map extent. The cartridge carries separate row/column sizes for
    /// the two planes even when the pack's ordinary maps make them equal.
    bounds_bg: CameraBounds,
    /// `$EC24`, `$EC25`, `$EC26`.
    gates: CameraGates,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            bounds: CameraBounds::default(),
            bounds_bg: CameraBounds::default(),
            gates: CameraGates::field_default(),
            ..Camera::zeroed()
        }
    }
}

impl Camera {
    const fn zeroed() -> Camera {
        Camera {
            pos_x: 0,
            pos_y: 0,
            step_x: 0,
            step_y: 0,
            pos_x_bg: 0,
            pos_y_bg: 0,
            step_x_bg: 0,
            step_y_bg: 0,
            driver_sprite_x: 0,
            driver_sprite_y: 0,
            driver_x: 0,
            driver_y: 0,
            bounds: CameraBounds {
                width: 0,
                height: 0,
                edges: CameraEdges::Clamped,
            },
            bounds_bg: CameraBounds {
                width: 0,
                height: 0,
                edges: CameraEdges::Clamped,
            },
            gates: CameraGates {
                ec24: 0,
                ec25: 0,
                ec26: 0,
            },
        }
    }

    /// The camera as a map places it: the driver at its home offset, then the
    /// map's edge rule applied.
    ///
    /// Map entry has no scroll-in — the view is simply positioned — so this is
    /// also the state a replay must start from at its alignment frame.
    #[must_use]
    pub fn placed_on(driver: Driver, bounds: CameraBounds) -> Camera {
        Self::placed_on_planes(driver, bounds, bounds, CameraGates::field_default())
    }

    /// Places both camera planes with independent map extents and gates.
    #[must_use]
    pub fn placed_on_planes(
        driver: Driver,
        bounds_fg: CameraBounds,
        bounds_bg: CameraBounds,
        gates: CameraGates,
    ) -> Camera {
        Self::placed_on_planes_with_steps(driver, bounds_fg, bounds_bg, gates, (0, 0), (0, 0))
    }

    /// Places both planes with the map record's initial 16.16 step counters.
    ///
    /// `loc_51AB2` can seed these before `loc_53854` positions the camera. The
    /// ordinary pack has zeroes today, but retaining the values prevents a
    /// map-record exception from being silently turned into a clean start.
    #[must_use]
    pub fn placed_on_planes_with_steps(
        driver: Driver,
        bounds_fg: CameraBounds,
        bounds_bg: CameraBounds,
        gates: CameraGates,
        step_fg: (i32, i32),
        step_bg: (i32, i32),
    ) -> Camera {
        let mut camera = Camera {
            pos_x: driver.x - HOME_X * ONE_PIXEL,
            pos_y: driver.y - HOME_Y * ONE_PIXEL,
            step_x: step_fg.0,
            step_y: step_fg.1,
            pos_x_bg: driver.x - HOME_X * ONE_PIXEL,
            pos_y_bg: driver.y - HOME_Y * ONE_PIXEL,
            step_x_bg: step_bg.0,
            step_y_bg: step_bg.1,
            bounds: bounds_fg,
            bounds_bg,
            gates,
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
        Self::new_planes(bounds, bounds, CameraGates::field_default())
    }

    /// Creates an origin camera with independent plane extents and gates.
    #[must_use]
    pub fn new_planes(
        bounds_fg: CameraBounds,
        bounds_bg: CameraBounds,
        gates: CameraGates,
    ) -> Camera {
        Camera {
            bounds: bounds_fg,
            bounds_bg,
            gates,
            ..Camera::default()
        }
    }

    /// The map this camera lives in.
    #[must_use]
    pub const fn bounds(&self) -> CameraBounds {
        self.bounds
    }

    /// The bounds for one camera plane.
    #[must_use]
    pub const fn bounds_on(&self, plane: CameraPlane) -> CameraBounds {
        match plane {
            CameraPlane::Foreground => self.bounds,
            CameraPlane::Background => self.bounds_bg,
        }
    }

    /// The three live camera-control bytes.
    #[must_use]
    pub const fn gates(&self) -> CameraGates {
        self.gates
    }

    /// The plane selected globally by `$EC24`.
    #[must_use]
    pub const fn active_plane(&self) -> CameraPlane {
        self.gates.plane()
    }

    /// Updates `$EC24`, `$EC25` and `$EC26` as a map-load routine would.
    pub fn set_gates(&mut self, gates: CameraGates) {
        self.gates = gates;
        self.refresh_driver_sprite(Driver {
            x: self.driver_x,
            y: self.driver_y,
        });
    }

    /// Applies the packed `loc_51AB2` gate write used by both ordinary map
    /// entry and `RefreshMap`.
    ///
    /// The gate bytes are always written. A zero plane gate is followed by
    /// two longwords which seed that plane's step counters; a nonzero gate
    /// leaves its existing counters alone, exactly as the 68000 routine does.
    pub fn apply_gate_write(
        &mut self,
        gates: CameraGates,
        step_fg: (i32, i32),
        step_bg: (i32, i32),
    ) {
        self.gates = gates;
        if gates.ec25 == 0 {
            self.step_x = step_fg.0;
            self.step_y = step_fg.1;
        }
        if gates.ec26 == 0 {
            self.step_x_bg = step_bg.0;
            self.step_y_bg = step_bg.1;
        }
        self.refresh_driver_sprite(Driver {
            x: self.driver_x,
            y: self.driver_y,
        });
    }

    /// The driver position the latch last saw, 16.16. A caller that may hand
    /// the latch a different driver (a scripted actor) uses this to tell a
    /// walk from a teleport.
    #[must_use]
    pub const fn driver_position(&self) -> (i32, i32) {
        (self.driver_x, self.driver_y)
    }

    /// The camera position in whole pixels, as the renderer wants it.
    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        self.position_on(self.active_plane())
    }

    /// The camera position for one plane, in whole pixels.
    #[must_use]
    pub const fn position_on(&self, plane: CameraPlane) -> (i32, i32) {
        let (x, y) = self.raw_on(plane);
        (x >> 16, y >> 16)
    }

    /// The camera sprite positions are actually computed against: pos + step.
    ///
    /// `FieldObj_CalcSpritePos` subtracts both, so this is the value that
    /// decides visibility, and it leads [`Camera::position`] by one commit.
    #[must_use]
    pub const fn effective(&self) -> (i32, i32) {
        self.effective_on(self.active_plane())
    }

    /// The position plus the uncommitted step for one plane.
    #[must_use]
    pub const fn effective_on(&self, plane: CameraPlane) -> (i32, i32) {
        let (x, y) = match plane {
            CameraPlane::Foreground => (self.pos_x + self.step_x, self.pos_y + self.step_y),
            CameraPlane::Background => (
                self.pos_x_bg + self.step_x_bg,
                self.pos_y_bg + self.step_y_bg,
            ),
        };
        (x >> 16, y >> 16)
    }

    /// The raw 16.16 position, for tests and for diffing against oracle RAM.
    #[must_use]
    pub const fn raw(&self) -> (i32, i32) {
        self.raw_on(self.active_plane())
    }

    /// The raw 16.16 position for one plane.
    #[must_use]
    pub const fn raw_on(&self, plane: CameraPlane) -> (i32, i32) {
        match plane {
            CameraPlane::Foreground => (self.pos_x, self.pos_y),
            CameraPlane::Background => (self.pos_x_bg, self.pos_y_bg),
        }
    }

    /// The raw 16.16 step counters.
    #[must_use]
    pub const fn raw_step(&self) -> (i32, i32) {
        self.raw_step_on(self.active_plane())
    }

    /// The raw 16.16 step counters for one plane.
    #[must_use]
    pub const fn raw_step_on(&self, plane: CameraPlane) -> (i32, i32) {
        match plane {
            CameraPlane::Foreground => (self.step_x, self.step_y),
            CameraPlane::Background => (self.step_x_bg, self.step_y_bg),
        }
    }

    /// Places the camera without the home offset, for scenes that park it.
    ///
    /// `SceneOp::SetCameraPos` hands an absolute camera position rather than a
    /// subject to frame.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.pos_x = x * ONE_PIXEL;
        self.pos_y = y * ONE_PIXEL;
        self.pos_x_bg = self.pos_x;
        self.pos_y_bg = self.pos_y;
        self.step_x = 0;
        self.step_y = 0;
        self.step_x_bg = 0;
        self.step_y_bg = 0;
        self.settle();
    }

    /// Parks one camera plane at an absolute position.
    pub fn set_position_on(&mut self, plane: CameraPlane, x: i32, y: i32) {
        match plane {
            CameraPlane::Foreground => {
                self.pos_x = x * ONE_PIXEL;
                self.pos_y = y * ONE_PIXEL;
                self.step_x = 0;
                self.step_y = 0;
            }
            CameraPlane::Background => {
                self.pos_x_bg = x * ONE_PIXEL;
                self.pos_y_bg = y * ONE_PIXEL;
                self.step_x_bg = 0;
                self.step_y_bg = 0;
            }
        }
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
    /// `Event_MoveCamera` shows the field loop's sequence plainly: the commits
    /// (`UpdateCameraXPosFG/BG`) run *before* `Field_UpdateObjects`, so a frame
    /// folds last frame's scroll into the position first, then decides this
    /// frame's scroll from the driver's sprite position — which is itself a
    /// frame old, having been written by the previous frame's
    /// `FieldObj_CalcSpritePos`.
    pub fn tick(&mut self, driver: Driver) {
        // 1. FieldObj_Camera*Pos_FG/BG: latch this frame's scroll off the
        //    driver's velocity, gated on its sprite position — a frame old,
        //    written by the previous CalcSpritePos — being past the threshold
        //    in the direction of travel. Each plane has its own gate/counter.
        let vx = driver.x - self.driver_x;
        let vy = driver.y - self.driver_y;
        self.driver_x = driver.x;
        self.driver_y = driver.y;
        self.step_x = if self.gates.ec25 == 0 {
            0
        } else {
            latch(vx, self.driver_sprite_x, THRESHOLD_X)
        };
        self.step_y = if self.gates.ec25 == 0 {
            0
        } else {
            latch(vy, self.driver_sprite_y, THRESHOLD_Y)
        };
        self.step_x_bg = if self.gates.ec26 == 0 {
            0
        } else {
            latch(vx, self.driver_sprite_x, THRESHOLD_X)
        };
        self.step_y_bg = if self.gates.ec26 == 0 {
            0
        } else {
            latch(vy, self.driver_sprite_y, THRESHOLD_Y)
        };

        // 2. UpdateCamera*PosFG/BG: fold this frame's scroll in, then apply the
        //    plane's edge rule. The commit lands *after* the latch, not before:
        //    the oracle's camera columns hold `leader - camera` at exactly
        //    (152, 88) on all 1563 field-control frames of tape 02, with no
        //    frame of lag anywhere, which only the latch-then-commit order
        //    produces. (`Event_MoveCamera` commits before its object pass, but
        //    that is a scene helper driving its own loop, not the field one.)
        self.pos_x += self.step_x;
        self.pos_y += self.step_y;
        self.pos_x_bg += self.step_x_bg;
        self.pos_y_bg += self.step_y_bg;
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
        self.sprite_pos_on(self.active_plane(), x, y)
    }

    /// The sprite-space position against one camera plane.
    #[must_use]
    pub const fn sprite_pos_on(&self, plane: CameraPlane, x: i32, y: i32) -> (i32, i32) {
        let (cam_x, cam_y) = self.position_on(plane);
        match self.bounds_on(plane).edges {
            CameraEdges::Clamped => (
                (x >> 16) - cam_x + SPRITE_ORIGIN,
                (y >> 16) - cam_y + SPRITE_ORIGIN,
            ),
            CameraEdges::Wrapping => (
                across_seam(x >> 16, cam_x, self.bounds_on(plane).width),
                across_seam(y >> 16, cam_y, self.bounds_on(plane).height),
            ),
        }
    }

    /// The sprite position with render-flags bit 0 applied.
    ///
    /// Retail skips both plane subtractions for that bit. The normal path
    /// remains the legacy integer view used by the established field tape;
    /// the bypass path is explicit so a caller cannot accidentally make every
    /// object screen-relative.
    #[must_use]
    pub const fn sprite_pos_on_with_camera_bypass(
        &self,
        plane: CameraPlane,
        x: i32,
        y: i32,
        camera_bypass: bool,
    ) -> (i32, i32) {
        if camera_bypass {
            return ((x >> 16) + SPRITE_ORIGIN, (y >> 16) + SPRITE_ORIGIN);
        }
        self.sprite_pos_on(plane, x, y)
    }

    /// Whether an object at 16.16 map position `(x, y)` is on screen.
    #[must_use]
    pub const fn sees(&self, x: i32, y: i32) -> bool {
        let (sx, sy) = self.sprite_pos(x, y);
        on_screen(sx, sy)
    }

    /// Whether an object is in the retail visibility box, honoring bit 0.
    #[must_use]
    pub const fn sees_with_camera_bypass(&self, x: i32, y: i32, camera_bypass: bool) -> bool {
        let (sx, sy) =
            self.sprite_pos_on_with_camera_bypass(self.active_plane(), x, y, camera_bypass);
        on_screen(sx, sy)
    }

    /// Whether a position is visible against one camera plane.
    #[must_use]
    pub const fn sees_on(&self, plane: CameraPlane, x: i32, y: i32) -> bool {
        let (sx, sy) = self.sprite_pos_on(plane, x, y);
        on_screen(sx, sy)
    }

    /// Applies the map's edge rule to the position, zeroing the scroll on a
    /// clamp exactly as `UpdateCameraYPosFG` does.
    fn settle(&mut self) {
        settle_plane(
            &mut self.pos_x,
            &mut self.pos_y,
            &mut self.step_x,
            &mut self.step_y,
            self.bounds,
        );
        settle_plane(
            &mut self.pos_x_bg,
            &mut self.pos_y_bg,
            &mut self.step_x_bg,
            &mut self.step_y_bg,
            self.bounds_bg,
        );
    }

    fn refresh_driver_sprite(&mut self, driver: Driver) {
        let (sx, sy) = self.sprite_pos(driver.x, driver.y);
        self.driver_sprite_x = sx;
        self.driver_sprite_y = sy;
    }
}

const fn settle_plane(
    pos_x: &mut i32,
    pos_y: &mut i32,
    step_x: &mut i32,
    step_y: &mut i32,
    bounds: CameraBounds,
) {
    match bounds.edges {
        CameraEdges::Clamped => {
            if *pos_x < 0 {
                *pos_x = 0;
                *step_x = 0;
            } else if *pos_x > bounds.max_x() * ONE_PIXEL {
                *pos_x = bounds.max_x() * ONE_PIXEL;
                *step_x = 0;
            }
            if *pos_y < 0 {
                *pos_y = 0;
                *step_y = 0;
            } else if *pos_y > bounds.max_y() * ONE_PIXEL {
                *pos_y = bounds.max_y() * ONE_PIXEL;
                *step_y = 0;
            }
        }
        CameraEdges::Wrapping => {
            *pos_x = pos_x.rem_euclid(bounds.width * ONE_PIXEL);
            *pos_y = pos_y.rem_euclid(bounds.height * ONE_PIXEL);
        }
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
#[path = "camera_tests.rs"]
mod tests;
