//! Field mechanics a scene runs as loops over the party: the moving
//! platforms' ride, the conveyor belts' carry, and the small party-state ops
//! the Vahal Fort and Weapon Plant events share.
//!
//! Split from `ops.rs` (the 1,000-line rule). The cartridge runs these as
//! event routines whose bodies are synchronous frame loops
//! (`Event_VahFortMovingPlatform1`, `$06C478`; `Event_ConveyorBeltDown`,
//! `$06CE5C`), so they are scenes here too; what makes them mechanics is that
//! the loop moves the *party objects*, not scripted cast.

use super::{Blocked, SceneRunner};
use crate::field::StepFrames;
use crate::geom::{CELL_PIXELS, Cell, Direction};
use crate::scene::{ActorRef, SceneEffect, SceneFault, SceneOp};
use crate::state::{GameState, PARTY_SLOTS};
use crate::trigger::PixelPos;

/// Frames per cell at `FieldObj_Step_Offset` 0: the movement table's first
/// block steps `$0100` (one pixel) per frame (`FieldObj_MovementsTbl`,
/// `$047AA8`), and the belts clear the offset (`clr.b $ECE0`, `$06CE5C`).
const BELT_STEP_FRAMES: u8 = 16;

/// How far a belt scan looks before it gives up: past the largest chunk
/// plane, so only a belt with no end can reach it.
const BELT_SCAN_LIMIT: i32 = 4096;

/// A running [`SceneOp::RidePlatform`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RideRun {
    /// Signed 16.16 pixels per frame.
    step: i64,
    /// Frames still to run.
    remaining: u16,
    /// Whole pixels already applied to the party.
    applied: i64,
    /// 16.16 position accumulated so far.
    position: i64,
}

/// A [`SceneOp::ConveyorRide`] waiting for its scan, or carrying the party.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BeltRun {
    direction: Direction,
    first_chunk: u8,
    last_chunk: u8,
    /// Whether the scan has run and the leader is walking its target.
    resolved: bool,
}

impl SceneRunner {
    /// Runs the ops of this module. Returns a fault instead of advancing when
    /// the op cannot run.
    pub(super) fn step_mechanic_op(
        &mut self,
        op: SceneOp,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        match op {
            SceneOp::ToggleFlag { flag } => {
                let value = !state.is_set(flag);
                if state.write(flag, value).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::FlagChanged { flag, value });
                self.pc += 1;
            }
            SceneOp::BranchIfPartyMember {
                who,
                if_present,
                if_absent,
            } => {
                let present = state.party().contains(&Some(who));
                let target = if present { if_present } else { if_absent };
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            SceneOp::FaceParty { facing } => {
                for slot in 0..PARTY_SLOTS {
                    let actor = ActorRef::PartyMember(slot);
                    if let Some(member) = self.actor_mut(actor) {
                        member.facing = facing;
                        effects.push(SceneEffect::ActorFaced { actor, facing });
                    }
                }
                self.pc += 1;
            }
            SceneOp::SetCharacterSkill { who, slot, skill } => {
                let Some(stats) = state.roster_mut().get_mut(who) else {
                    return Some(SceneFault::BadWrite);
                };
                let Some(byte) = stats.skills.get_mut(usize::from(slot)) else {
                    return Some(SceneFault::BadWrite);
                };
                *byte = skill;
                effects.push(SceneEffect::RosterChanged { who });
                self.pc += 1;
            }
            SceneOp::RidePlatform {
                slot,
                step_y,
                frames,
            } => {
                // The party moves rigidly; a ride that ends between cells
                // would strand it off the cell grid.
                let distance = i64::from(step_y) * i64::from(frames);
                if distance % (i64::from(CELL_PIXELS) << 16) != 0 {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::Presentation {
                    op: SceneOp::StepFieldObject {
                        slot,
                        step_x: 0,
                        step_y,
                        frames,
                    },
                });
                self.ride = (frames > 0).then_some(RideRun {
                    step: i64::from(step_y),
                    remaining: frames,
                    applied: 0,
                    position: 0,
                });
                self.pc += 1;
                if frames > 0 {
                    self.blocked = Blocked::Ticks(frames);
                }
            }
            SceneOp::ConveyorRide {
                direction,
                first_chunk,
                last_chunk,
            } => {
                self.belt = Some(BeltRun {
                    direction,
                    first_chunk,
                    last_chunk,
                    resolved: false,
                });
                self.pc += 1;
                self.blocked = Blocked::Belt;
            }
            _ => unreachable!("not a mechanic op: {op:?}"),
        }
        None
    }

    /// One frame of a [`SceneOp::RidePlatform`]: the step is added to every
    /// party object's position, which the actors carry as a whole-pixel slide
    /// that rolls over into the cell.
    pub(super) fn tick_ride(&mut self) {
        let Some(run) = self.ride.as_mut() else {
            return;
        };
        run.position += run.step;
        let whole = run.position >> 16;
        let delta = (whole - run.applied) as i32;
        run.applied = whole;
        run.remaining -= 1;
        if run.remaining == 0 {
            self.ride = None;
        }
        for slot in 0..PARTY_SLOTS {
            let Some(member) = self.actor_mut(ActorRef::PartyMember(slot)) else {
                continue;
            };
            let slid = member.slide.1 + delta;
            member.slide.1 = slid.rem_euclid(CELL_PIXELS);
            let row = i32::from(member.cell.y) + slid.div_euclid(CELL_PIXELS);
            member.cell = Cell::new(member.cell.x, row.clamp(0, i32::from(u16::MAX)) as u16);
        }
    }

    /// After the ops of a tick: a belt op that just started reads the live
    /// layout once, finds where the carry ends and sets the leader walking
    /// there.
    ///
    /// The cartridge tests the chunk under the leader every frame
    /// (`GetChunkAndCollision`, `$45A52`) and stops at the first frame it is
    /// outside the belt's chunk ids, letting the step in flight finish. That
    /// frame is the first pixel offset `t >= 1` along the belt whose chunk is
    /// outside, and the leader ends `ceil(t / 16)` cells along.
    pub(super) fn resolve_belt(
        &mut self,
        map: &crate::map::FieldMap,
        chunks: Option<&dyn Fn(PixelPos) -> Option<u16>>,
        effects: &mut Vec<SceneEffect>,
    ) {
        let Some(run) = self.belt.as_mut() else {
            return;
        };
        if run.resolved || self.blocked != Blocked::Belt {
            return;
        }
        let run = *run;
        let Some(chunks) = chunks else {
            self.belt_fault(SceneFault::NoLayout, effects);
            return;
        };
        let leader = ActorRef::PartyMember(0);
        let Some(origin) = self.actor(leader).map(|a| a.cell) else {
            self.belt_fault(SceneFault::UnknownActor { actor: leader }, effects);
            return;
        };
        let at = PixelPos::from_cell(origin);
        let (dx, dy) = run.direction.delta();
        let covered = |id: u16| id >= u16::from(run.first_chunk) && id <= u16::from(run.last_chunk);
        let ends_at = (1..=BELT_SCAN_LIMIT).find(|t| {
            let probe = PixelPos {
                x: at.x + dx * t,
                // `GetChunkAndCollision` adds `$10` to `curr_y_pos`.
                y: at.y + CELL_PIXELS + dy * t,
            };
            !chunks(probe).is_some_and(covered)
        });
        let Some(ends_at) = ends_at else {
            self.belt_fault(SceneFault::NoLayout, effects);
            return;
        };
        let mut target = origin;
        for _ in 0..(ends_at + CELL_PIXELS - 1) / CELL_PIXELS {
            let Some(next) = map.neighbor(target, run.direction) else {
                self.belt_fault(SceneFault::NoLayout, effects);
                return;
            };
            target = next;
        }
        let pace = StepFrames::new(BELT_STEP_FRAMES).expect("belt pace is non-zero");
        for slot in 0..PARTY_SLOTS {
            if let Some(member) = self.actor_mut(ActorRef::PartyMember(slot)) {
                member.pace = Some(pace);
            }
        }
        if let Some(member) = self.actor_mut(leader) {
            member.target = Some(target);
        }
        effects.push(SceneEffect::ActorMoveStarted {
            actor: leader,
            to: target,
        });
        if let Some(run) = self.belt.as_mut() {
            run.resolved = true;
        }
        self.blocked = Blocked::Actor(leader);
    }

    /// Ends a carry the leader has finished: the belt's pace goes with it.
    pub(super) fn end_belt_if_arrived(&mut self) {
        let Some(run) = self.belt else {
            return;
        };
        let walking = self
            .actor(ActorRef::PartyMember(0))
            .is_some_and(crate::scene::ScriptedActor::is_walking);
        if run.resolved && !walking {
            self.belt = None;
            for slot in 0..PARTY_SLOTS {
                if let Some(member) = self.actor_mut(ActorRef::PartyMember(slot)) {
                    member.pace = None;
                }
            }
        }
    }

    fn belt_fault(&mut self, fault: SceneFault, effects: &mut Vec<SceneEffect>) {
        self.belt = None;
        effects.push(SceneEffect::Faulted(fault));
        self.blocked = Blocked::Done;
    }
}
