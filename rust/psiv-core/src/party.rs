//! The caterpillar: the party walking single file behind its leader.
//!
//! # The cartridge's mechanism
//!
//! There is no path buffer and no history array. Each field object carries a
//! destination (`dest_x_pos`/`dest_y_pos`), and the whole effect falls out of
//! one write at the tail of `FieldObj_Move` (`ps4.asm:93415`), executed on the
//! single frame a character *begins* a step:
//!
//! ```text
//!     btst    #0, (Char_Move_Flags).w
//!     bne.s   +                       ; bit 0 set disables the follow chain
//!     lea     $40(a4), a1             ; the next character slot
//!     cmpa.w  #$C180, a1
//!     bcc.s   +                       ; five slots, $C000..$C100
//!     move.w  curr_x_pos(a4), dest_x_pos(a1)
//!     move.w  curr_y_pos(a4), dest_y_pos(a1)
//! ```
//!
//! `curr_x_pos` here is still the character's **pre-step** position, because
//! `FieldObj_UpdatePosition` runs later in the same handler. So a member's
//! destination becomes *the cell the member ahead of it is leaving* — which is
//! the whole caterpillar, in two instructions.
//!
//! Followers turn that destination into movement through
//! `FieldObj_GetAutoInput` (`ps4.asm:93232`), which synthesises the same d-pad
//! bit pattern the leader gets from the joypad by comparing current against
//! destination. That means followers run the identical movement code, and take
//! their facing from the same `FieldObj_MovementsTbl` entry — a follower's
//! facing is derived from where it is walking, never copied from the leader.
//!
//! `Field_RunObjects` (`ps4.asm:89477`) walks the object slots in ascending
//! address order, leader first, so a follower sees its new destination and
//! starts moving on the *same* frame as the member ahead of it. The party
//! therefore steps in lockstep: everyone starts together and lands together.
//!
//! # What that means here
//!
//! Because steps are whole cells and everyone moves in phase, the cell-level
//! model is exact and much simpler than the pixel-level one: on the tick the
//! leader begins a step, every follower whose target differs from its own cell
//! begins a step onto the cell the member ahead of it just left. A party that
//! starts stacked unspools one member per step, which is what the cartridge
//! does after a map load.

use crate::error::MapError;
use crate::field::{Effect, FieldState, Input, StepFrames};
use crate::geom::{Cell, Direction};
use crate::map::FieldMap;

/// The cartridge's field-party cap.
///
/// `Current_Party_Slot_1..5` (`$FFFFF40A..$FFFFF40E`) holds five `CharID`
/// bytes terminated by `$FF`, `Event_RemoveCharacter` compacts the five slots
/// `$C000..$C100`, and the follow-chain write above stops before `$C180`.
pub const MAX_PARTY_MEMBERS: usize = 5;

/// A follower's state. The leader is a [`FieldState`]; this is everyone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Follower {
    cell: Cell,
    facing: Direction,
    step: Option<FollowerStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FollowerStep {
    dir: Direction,
    to: Cell,
    progress: u8,
}

/// One party member as the renderer needs it.
///
/// Index 0 is the leader. Everything a sprite needs and nothing more; the
/// fields mean exactly what the same-named [`FieldState`] accessors mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberView {
    /// The cell it occupies, or is stepping away from mid-step.
    pub cell: Cell,
    /// Which way it faces.
    pub facing: Direction,
    /// Sub-cell displacement in sixteenths of a cell — pixels at 1x.
    pub render_offset_16ths: (i32, i32),
    /// Whether it is mid-step, for picking a walk versus stand animation.
    pub is_stepping: bool,
}

/// The party: a leader that reads input and followers that trace its path.
///
/// [`Party`] wraps rather than replaces [`FieldState`]: the leader *is* a
/// `FieldState` and every movement, collision, warp and talk rule lives there
/// unchanged. This type adds only the trail.
///
/// Followers are deliberately **not** collision-checked. They step onto cells
/// the leader has already walked and just vacated, so the ground is walkable by
/// construction — and the cartridge agrees, gating its collision calls on being
/// the leading character (`ps4.asm:93359`, "branch if this is not the leading
/// character").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    leader: FieldState,
    followers: Vec<Follower>,
}

impl Party {
    /// Places a party of `followers + 1` members on `map`, all stacked on
    /// `cell`.
    ///
    /// # Errors
    ///
    /// [`MapError::PartyOutOfBounds`] as [`FieldState::new`], or
    /// [`MapError::TooManyPartyMembers`] beyond [`MAX_PARTY_MEMBERS`].
    pub fn new(
        map: &FieldMap,
        cell: Cell,
        facing: Direction,
        step_frames: StepFrames,
        followers: usize,
    ) -> Result<Party, MapError> {
        if followers + 1 > MAX_PARTY_MEMBERS {
            return Err(MapError::TooManyPartyMembers {
                requested: followers + 1,
                max: MAX_PARTY_MEMBERS,
            });
        }
        let leader = FieldState::new(map, cell, facing, step_frames)?;
        let follower = Follower {
            cell: leader.cell(),
            facing,
            step: None,
        };
        Ok(Party {
            leader,
            followers: vec![follower; followers],
        })
    }

    /// Moves the whole party onto `map` at `cell`, stacked on the leader and
    /// facing the same way, cancelling every step in progress.
    ///
    /// This is what the cartridge does: the post-battle reload path copies the
    /// leader's position and facing into each follower (`ps4.asm:110825-110829`,
    /// `curr`, `dest` and `facing_dir` all copied), and the fresh map-load path
    /// spawns every member on the same `Map_Start_X_Pos`/`Map_Start_Y_Pos`
    /// tile with `Map_Start_Facing_Dir` (`ps4.asm:110862-110865`). The party
    /// unspools again as the leader walks.
    ///
    /// One detail is deliberately not modelled: the fresh-load path also queues
    /// each follower a one-cell nudge chosen by `Map_Start_Char_Align`
    /// (`ps4.asm:110871`, offsets `±$10` from `loc_53656`), the little
    /// spread-out-from-the-doorway shuffle. It needs a per-map alignment byte
    /// the engine is not given, it is cosmetic, and it resolves within one
    /// step of walking. Filed rather than guessed.
    ///
    /// # Errors
    ///
    /// [`MapError::PartyOutOfBounds`], as [`FieldState::enter_map`].
    pub fn enter_map(
        &mut self,
        map: &FieldMap,
        cell: Cell,
        facing: Direction,
    ) -> Result<(), MapError> {
        self.leader.enter_map(map, cell, facing)?;
        for follower in &mut self.followers {
            follower.cell = self.leader.cell();
            follower.facing = facing;
            follower.step = None;
        }
        Ok(())
    }

    /// Advances the whole party one tick.
    ///
    /// The leader ticks exactly as it would alone, and the returned effects are
    /// the leader's — followers generate none, because nothing they do is an
    /// event the runtime must act on. Their motion is read through
    /// [`Party::member`].
    pub fn tick(&mut self, map: &FieldMap, input: Input) -> Vec<Effect> {
        let was_stepping = self.leader.is_stepping();
        let leader_from = self.leader.cell();

        let effects = self.leader.tick(map, input);

        // Did the leader *begin* a step this tick? It cannot begin one while
        // already stepping, and with a one-frame duration a step begins and
        // completes inside the same tick — hence both halves of the test.
        let began_step = !was_stepping
            && (self.leader.is_stepping()
                || effects
                    .iter()
                    .any(|e| matches!(e, Effect::StepCompleted { .. })));

        if began_step {
            self.propagate(map, leader_from);
        }
        self.advance_followers();

        effects
    }

    /// Hands each follower the cell the member ahead of it is leaving.
    fn propagate(&mut self, map: &FieldMap, leader_from: Cell) {
        let mut vacated = leader_from;
        for follower in &mut self.followers {
            let leaving = follower.cell;
            if vacated != follower.cell {
                follower.begin_step(map, vacated);
            }
            vacated = leaving;
        }
    }

    /// Advances every follower's step in progress by one tick.
    fn advance_followers(&mut self) {
        let frames = self.leader.step_frames().get();
        for follower in &mut self.followers {
            let Some(step) = follower.step.as_mut() else {
                continue;
            };
            step.progress = step.progress.saturating_add(1);
            if step.progress >= frames {
                follower.cell = step.to;
                follower.step = None;
            }
        }
    }

    /// The leader, for every rule that is not the trail.
    #[must_use]
    pub const fn leader(&self) -> &FieldState {
        &self.leader
    }

    /// The leader, mutably — for [`FieldState::enter_map`] on a warp when the
    /// followers should *not* be restacked. Prefer [`Party::enter_map`].
    pub const fn leader_mut(&mut self) -> &mut FieldState {
        &mut self.leader
    }

    /// How many members the party has, leader included. Always at least 1.
    #[must_use]
    pub fn len(&self) -> usize {
        self.followers.len() + 1
    }

    /// Always false — a party always has a leader. Present because clippy asks
    /// for it wherever `len` exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// One member's drawable state. Index 0 is the leader; `None` past the end.
    #[must_use]
    pub fn member(&self, index: usize) -> Option<MemberView> {
        if index == 0 {
            return Some(MemberView {
                cell: self.leader.cell(),
                facing: self.leader.facing(),
                render_offset_16ths: self.leader.render_offset_16ths(),
                is_stepping: self.leader.is_stepping(),
            });
        }
        let follower = self.followers.get(index - 1)?;
        Some(MemberView {
            cell: follower.cell,
            facing: follower.facing,
            render_offset_16ths: follower.render_offset_16ths(self.leader.step_frames()),
            is_stepping: follower.step.is_some(),
        })
    }

    /// Every member, leader first — the draw order the cartridge uses.
    #[must_use]
    pub fn members(&self) -> Vec<MemberView> {
        (0..self.len())
            .filter_map(|index| self.member(index))
            .collect()
    }
}

impl Follower {
    /// Starts a step onto `target`, which the caller guarantees is adjacent.
    fn begin_step(&mut self, map: &FieldMap, target: Cell) {
        // In lockstep a follower is always exactly one cell behind on the path,
        // so one of the four directions leads to the target — on a wrapping map
        // too, since `neighbor` is topology-aware.
        let dir = Direction::ALL
            .into_iter()
            .find(|&dir| map.neighbor(self.cell, dir) == Some(target));
        match dir {
            Some(dir) => {
                self.facing = dir;
                self.step = Some(FollowerStep {
                    dir,
                    to: target,
                    progress: 0,
                });
            }
            None => {
                // Unreachable while the invariant holds. If a caller ever
                // breaks it — moving the leader without the party, say — snap
                // rather than desync the trail or invent a multi-step path.
                self.cell = target;
                self.step = None;
            }
        }
    }

    fn render_offset_16ths(&self, frames: StepFrames) -> (i32, i32) {
        let Some(step) = self.step else {
            return (0, 0);
        };
        let travelled =
            i32::from(step.progress) * crate::field::SUBCELL_UNITS / i32::from(frames.get());
        let (dx, dy) = step.dir.delta();
        (dx * travelled, dy * travelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::CollisionGrid;
    use crate::map::MapId;

    fn open_map() -> FieldMap {
        FieldMap::new(
            MapId(0),
            CollisionGrid::filled(8, 8, 0).unwrap(),
            vec![],
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn a_party_is_capped_at_the_cartridges_five_slots() {
        let map = open_map();
        assert!(matches!(
            Party::new(
                &map,
                Cell::new(0, 0),
                Direction::Down,
                StepFrames::default(),
                MAX_PARTY_MEMBERS,
            ),
            Err(MapError::TooManyPartyMembers { .. })
        ));
        assert!(
            Party::new(
                &map,
                Cell::new(0, 0),
                Direction::Down,
                StepFrames::default(),
                MAX_PARTY_MEMBERS - 1,
            )
            .is_ok()
        );
    }

    #[test]
    fn a_lone_leader_is_a_valid_party() {
        let map = open_map();
        let party = Party::new(
            &map,
            Cell::new(1, 1),
            Direction::Down,
            StepFrames::default(),
            0,
        )
        .unwrap();
        assert_eq!(party.len(), 1);
        assert!(party.member(1).is_none());
    }
}
