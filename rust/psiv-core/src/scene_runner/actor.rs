//! The scene cast's walking actor.
//!
//! Split from `scene.rs` (the 1,000-line rule) when the vocabulary grew; the
//! type is re-exported from there, so `psiv_core::ScriptedActor` is unchanged.

use crate::field::StepFrames;
use crate::geom::{Cell, Direction};
use crate::map::FieldMap;
use crate::scene::ActorRef;

/// A scene actor: a cell position, a facing, and a walk in progress.
///
/// Movement is the auto-input model: close the X gap first, then the Y gap,
/// one whole cell-step at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptedActor {
    /// Who this is.
    pub actor: ActorRef,
    /// Current cell.
    pub cell: Cell,
    /// Current facing.
    pub facing: Direction,
    /// Where it is walking, if anywhere.
    pub target: Option<Cell>,
    pub(crate) step: Option<(Direction, Cell, u8)>,
    /// Frames per cell this actor walks at, when a scene op fixes its own
    /// speed (the conveyor belts run at `FieldObj_Step_Offset` 0, not the
    /// party's walk). `None` is the runner's ordinary step timing.
    pub(crate) pace: Option<StepFrames>,
    /// Whole-pixel displacement from [`ScriptedActor::cell`], in the sixteenths
    /// [`ScriptedActor::render_offset_16ths`] reports: what a platform ride
    /// adds to the object's position between cell boundaries.
    pub(crate) slide: (i32, i32),
}

impl ScriptedActor {
    /// A stationary actor.
    #[must_use]
    pub const fn new(actor: ActorRef, cell: Cell, facing: Direction) -> ScriptedActor {
        ScriptedActor {
            actor,
            cell,
            facing,
            target: None,
            step: None,
            pace: None,
            slide: (0, 0),
        }
    }

    /// Whether it is mid-step.
    #[must_use]
    pub const fn is_stepping(&self) -> bool {
        self.step.is_some()
    }

    /// Parks the actor at a cell, cancelling any walk in progress.
    pub(crate) fn park(&mut self, cell: Cell, facing: Direction) {
        self.cell = cell;
        self.facing = facing;
        self.target = None;
        self.step = None;
        self.slide = (0, 0);
    }

    /// Whether it still has walking to do.
    #[must_use]
    pub const fn is_walking(&self) -> bool {
        self.target.is_some() || self.step.is_some()
    }

    /// Sub-cell displacement in sixteenths, as [`crate::FieldState`] reports it.
    #[must_use]
    pub fn render_offset_16ths(&self, frames: StepFrames) -> (i32, i32) {
        let Some((dir, _, progress)) = self.step else {
            return self.slide;
        };
        let frames = self.pace.unwrap_or(frames);
        let travelled = i32::from(progress) * crate::field::SUBCELL_UNITS / i32::from(frames.get());
        let (dx, dy) = dir.delta();
        (self.slide.0 + dx * travelled, self.slide.1 + dy * travelled)
    }

    /// The direction that closes the gap to `target`.
    ///
    /// `FieldObj_GetAutoInput` (`ps4.asm:93232`) closes the X gap first
    /// unless `Char_Move_Flags` bit 1 is set — the oracle's house-exit walk
    /// (tape 27, frames 1820..1900) confirms it: left along the wall-top
    /// walkway, then down through the arch.
    fn direction_toward(&self, target: Cell, y_first: bool) -> Option<Direction> {
        let dx = (target.x != self.cell.x).then_some({
            if target.x > self.cell.x {
                Direction::Right
            } else {
                Direction::Left
            }
        });
        let dy = (target.y != self.cell.y).then_some({
            if target.y > self.cell.y {
                Direction::Down
            } else {
                Direction::Up
            }
        });
        if y_first { dy.or(dx) } else { dx.or(dy) }
    }

    /// Advances one tick. Returns the cell arrived at when a walk finishes.
    ///
    /// Crate-internal: the runner drives this, callers read the actor instead.
    pub(crate) fn tick(
        &mut self,
        map: &FieldMap,
        frames: StepFrames,
        y_first: bool,
    ) -> Option<Cell> {
        let frames = self.pace.unwrap_or(frames);
        if self.step.is_none() {
            let target = self.target?;
            match self.direction_toward(target, y_first) {
                Some(dir) => {
                    self.facing = dir;
                    // Scene actors walk where the script says. Collision is not
                    // consulted: the cartridge's scripted moves write dest and
                    // drive the object there, and several scenes deliberately
                    // walk actors across cells the player could not.
                    //
                    // A target off the edge of a bounded map is the one thing
                    // that cannot be walked to. Abandoning the walk there is
                    // what keeps a `WaitForActor` from blocking forever; the
                    // arrival is still reported, at the cell actually reached.
                    let Some(to) = map.neighbor(self.cell, dir) else {
                        self.target = None;
                        return Some(self.cell);
                    };
                    self.step = Some((dir, to, 0));
                }
                None => {
                    self.target = None;
                    return Some(self.cell);
                }
            }
        }

        let (dir, to, progress) = self.step?;
        let progress = progress.saturating_add(1);
        if progress < frames.get() {
            self.step = Some((dir, to, progress));
            return None;
        }
        self.step = None;
        self.cell = to;
        if self.target == Some(self.cell) {
            self.target = None;
            return Some(self.cell);
        }
        None
    }
}
