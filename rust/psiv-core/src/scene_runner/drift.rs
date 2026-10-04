//! Live coordinate reads and per-frame object drift.
//!
//! Two vocabulary additions the Tyler's Grave opening (`Event_TylerGraveOpening`,
//! `docs/scenes/92_TylerGraveOpening.md`) needs and no earlier scene did:
//! a branch on an object's pixel coordinate against a literal, and
//! `DoMainUpdatesLoop` over objects that carry step constants.

use super::SceneRunner;
use crate::geom::CELL_PIXELS;
use crate::map::FieldMap;
use crate::scene::{ActorRef, Axis, SceneEffect, SceneOp};
use crate::scene_presentation::PresentationOp;
use crate::trigger::PixelPos;

/// The condition of the `Bcc` that follows a `cmpi.w #value, <coordinate>`.
///
/// `cmpi.w` computes `coordinate - value`; the unsigned branches read the
/// carry and zero flags of that subtraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordCmp {
    /// `beq`: the coordinate equals the literal.
    Equal,
    /// `bne`: it does not.
    NotEqual,
    /// `bcs`/`blo`: it is below the literal (unsigned).
    Below,
    /// `bls`: it is at most the literal (unsigned).
    AtMost,
    /// `bhi`: it is above the literal (unsigned).
    Above,
    /// `bcc`/`bhs`: it is at least the literal (unsigned).
    AtLeast,
}

impl CoordCmp {
    /// Whether `coordinate <cmp> value` holds, as unsigned words.
    #[must_use]
    pub const fn holds(self, coordinate: u16, value: u16) -> bool {
        match self {
            CoordCmp::Equal => coordinate == value,
            CoordCmp::NotEqual => coordinate != value,
            CoordCmp::Below => coordinate < value,
            CoordCmp::AtMost => coordinate <= value,
            CoordCmp::Above => coordinate > value,
            CoordCmp::AtLeast => coordinate >= value,
        }
    }
}

/// One object's step constants for [`SceneOp::DriftNpcs`]: the longwords the
/// scene writes to `x_step_constant` (`$20`) and `y_step_constant` (`$24`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcDrift {
    /// The map object's index in [`FieldMap::npcs`] (RAM `$FFFFC300 + 40 * npc`).
    pub npc: usize,
    /// `x_step_constant`: signed 16.16 pixels added to `curr_x_pos` per frame.
    pub step_x: i32,
    /// `y_step_constant`, likewise for `curr_y_pos`.
    pub step_y: i32,
}

/// One drifting object's running position, 16.16 pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Drifter {
    drift: NpcDrift,
    x: i64,
    y: i64,
}

/// A running [`SceneOp::DriftNpcs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DriftRun {
    drifters: Vec<Drifter>,
    remaining: u16,
}

/// The pixel position of an object standing in `cell` with `offset`:
/// `curr_x_pos`/`curr_y_pos`'s integer words. Object row zero is pixel `-16`
/// (`PixelPos::from_cell`).
pub(super) fn npc_pixel(npc: &crate::map::Npc) -> (i32, i32) {
    (
        i32::from(npc.cell.x) * CELL_PIXELS + i32::from(npc.offset.x),
        (i32::from(npc.cell.y) - 1) * CELL_PIXELS + i32::from(npc.offset.y),
    )
}

impl SceneRunner {
    /// Copies the live map objects' pixel positions, then lays the positions
    /// this scene has drifted over them. The runtime applies every drift step
    /// to the map as it is reported, so the two agree; the overlay is what a
    /// headless runner with no runtime behind it keeps reading.
    pub(super) fn refresh_npc_pixels(&mut self, map: &FieldMap) {
        self.npc_pixels = map.npcs().iter().map(npc_pixel).collect();
        for &(npc, at) in &self.drifted {
            if let Some(slot) = self.npc_pixels.get_mut(npc) {
                *slot = at;
            }
        }
    }

    /// The pixel position a coordinate branch reads for `actor`.
    pub(super) fn actor_pixel(&self, actor: ActorRef) -> Option<(i32, i32)> {
        let found = self.actor(actor)?;
        if let ActorRef::Npc(index) = actor
            && let Some(&at) = self.npc_pixels.get(index)
        {
            return Some(at);
        }
        let origin = PixelPos::from_cell(found.cell);
        let (dx, dy) = found.render_offset_16ths(self.step_frames);
        Some((origin.x + dx, origin.y + dy))
    }

    /// Starts a drift: every object's position is seeded from where it stands.
    pub(super) fn start_drift(
        &mut self,
        drifts: &'static [NpcDrift],
        frames: u16,
    ) -> Option<ActorRef> {
        let mut drifters = Vec::with_capacity(drifts.len());
        for &drift in drifts {
            let actor = ActorRef::Npc(drift.npc);
            let (Some(_), Some(&(x, y))) = (self.actor(actor), self.npc_pixels.get(drift.npc))
            else {
                return Some(actor);
            };
            drifters.push(Drifter {
                drift,
                x: i64::from(x) << 16,
                y: i64::from(y) << 16,
            });
        }
        self.drift = (frames > 0).then_some(DriftRun {
            drifters,
            remaining: frames,
        });
        None
    }

    /// One `FieldObj_UpdatePosition` per drifting object: add the step
    /// constants, report the whole-pixel result (the integer word the
    /// cartridge reads back at `$30`/`$34`).
    pub(super) fn tick_drift(&mut self, effects: &mut Vec<SceneEffect>) {
        let Some(run) = self.drift.as_mut() else {
            return;
        };
        for drifter in &mut run.drifters {
            drifter.x += i64::from(drifter.drift.step_x);
            drifter.y += i64::from(drifter.drift.step_y);
            let at = ((drifter.x >> 16) as i32, (drifter.y >> 16) as i32);
            let npc = drifter.drift.npc;
            match self.drifted.iter_mut().find(|(index, _)| *index == npc) {
                Some(entry) => entry.1 = at,
                None => self.drifted.push((npc, at)),
            }
            if let Some(slot) = self.npc_pixels.get_mut(npc) {
                *slot = at;
            }
            if let Some(actor) = self
                .actors
                .iter_mut()
                .find(|actor| actor.actor == ActorRef::Npc(npc))
            {
                actor.cell = super::pixel_cell(at.0, at.1);
            }
            effects.push(SceneEffect::Presentation {
                op: SceneOp::Presentation {
                    op: PresentationOp::NpcPixelPosition {
                        npc,
                        x: at.0,
                        y: at.1,
                    },
                },
            });
        }
        run.remaining -= 1;
        if run.remaining == 0 {
            self.drift = None;
        }
    }

    /// The coordinate `cmpi.w #value, $30(a4)` / `$34(a4)` reads: whole pixels
    /// as an unsigned word.
    pub(super) fn coordinate_word(&self, actor: ActorRef, axis: Axis) -> Option<u16> {
        let (x, y) = self.actor_pixel(actor)?;
        Some(match axis {
            Axis::X => x,
            Axis::Y => y,
        } as u16)
    }
}
