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

use core::cmp::Ordering;

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
    /// Where a follower off the trail is walking ([`Party::walk_in`]).
    walking_in: Option<Cell>,
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
            walking_in: None,
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
            follower.walking_in = None;
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
    ///
    /// A follower walking in ([`Party::walk_in`]) takes it as its destination
    /// only from a member ahead that is stepping now; the cell it vacates is
    /// handed on only if it starts a step itself.
    fn propagate(&mut self, map: &FieldMap, leader_from: Cell) {
        let mut vacated = leader_from;
        let mut ahead_stepped = true;
        let mut ahead_walking_in = false;
        for follower in &mut self.followers {
            let leaving = follower.cell;
            if let Some(destination) = follower.walking_in.as_mut() {
                if ahead_stepped {
                    *destination = vacated;
                }
                ahead_stepped = follower.continue_walk_in(ahead_walking_in);
                ahead_walking_in = follower.walking_in.is_some();
                vacated = leaving;
                continue;
            }
            if vacated != follower.cell {
                follower.begin_step(map, vacated);
            }
            ahead_stepped = true;
            ahead_walking_in = false;
            vacated = leaving;
        }
    }

    /// Advances every follower's step in progress by one tick. A follower
    /// walking in that starts its next step hands the cell it leaves to the
    /// one behind, so the chain follows a member who walks while the leader
    /// stands.
    fn advance_followers(&mut self) {
        let frames = self.leader.step_frames().get();
        let mut ahead_left: Option<Cell> = None;
        let mut ahead_walking_in = false;
        for follower in &mut self.followers {
            if let (Some(destination), Some(cell)) = (follower.walking_in.as_mut(), ahead_left) {
                *destination = cell;
            }
            let mut left = None;
            if let Some(step) = follower.step.as_mut() {
                step.progress = step.progress.saturating_add(1);
                if step.progress >= frames {
                    follower.cell = step.to;
                    follower.step = None;
                    let from = follower.cell;
                    if follower.continue_walk_in(ahead_walking_in) {
                        left = Some(from);
                    }
                }
            } else if ahead_left.is_some() {
                let from = follower.cell;
                if follower.continue_walk_in(ahead_walking_in) {
                    left = Some(from);
                }
            }
            ahead_left = left;
            ahead_walking_in = follower.walking_in.is_some();
        }
    }

    /// Starts whatever walk-in steps are due, in slot order: after a scene
    /// hands the party back, a follower whose scene walk ended short of its
    /// destination carries on, and one that is there rejoins the trail.
    fn settle_walk_ins(&mut self) {
        let mut ahead_walking_in = false;
        for follower in &mut self.followers {
            follower.continue_walk_in(ahead_walking_in);
            ahead_walking_in = follower.walking_in.is_some();
        }
    }

    /// Seeds a scene's cast with the live motion of each follower walking in:
    /// its step in progress and its destination as the actor's target, so the
    /// scene sees the party moving (`Event_PickingFifthCharacter`'s wait) and
    /// [`Party::resume_scripted`] hands the walk back. A follower on the trail
    /// enters a scene on its cell, as it always has.
    pub fn seed_scripted(&self, actors: &mut [crate::scene::ScriptedActor]) {
        for actor in actors {
            let crate::scene::ActorRef::PartyMember(slot) = actor.actor else {
                continue;
            };
            let Some(follower) = slot
                .checked_sub(1)
                .and_then(|i| self.followers.get(i))
                .filter(|follower| follower.walking_in.is_some())
            else {
                continue;
            };
            actor.step = follower.step.map(|s| (s.dir, s.to, s.progress));
            actor.target = follower
                .walking_in
                .filter(|&destination| destination != follower.cell);
        }
    }

    /// The follower in party slot `slot`, and those behind it, walk to the
    /// member ahead of them from wherever they stand, rather than holding the
    /// one-cell trail: the move a party object makes whenever its destination
    /// is further than a step (`FieldObj_GetAutoInput`, `ps4.asm:93232`, X first
    /// by default). A friend who joins from their own stand does this
    /// (`docs/scenes/181_PickingFifthCharacter.md`). They stand still until the
    /// leader first steps, and rejoin the trail on reaching their destination.
    pub fn walk_in(&mut self, slot: usize) {
        for follower in self.followers.iter_mut().skip(slot.saturating_sub(1)) {
            follower.walking_in = Some(follower.cell);
        }
    }

    /// Adopt a scene's live party objects without restacking them. Scripted
    /// walks use the same cell and progress clock as ordinary field walking.
    pub fn resume_scripted(
        &mut self,
        map: &FieldMap,
        actors: &[crate::scene::ScriptedActor],
    ) -> Result<(), MapError> {
        let Some(leader) = actors.first() else {
            return Ok(());
        };
        let mut resumed = self.clone();
        resumed.leader.resume_scripted(map, leader)?;
        for (follower, actor) in resumed.followers.iter_mut().zip(&actors[1..]) {
            let cell = map
                .normalize(actor.cell)
                .ok_or(MapError::PartyOutOfBounds {
                    map: map.id(),
                    cell: actor.cell,
                    width: map.width(),
                    height: map.height(),
                })?;
            follower.cell = cell;
            follower.facing = actor.facing;
            follower.step =
                actor
                    .step
                    .map(|(dir, to, progress)| FollowerStep { dir, to, progress });
            // A walk-in keeps going from where the scene left it: to the
            // target the scene still had, or ending where it stands.
            if follower.walking_in.is_some() {
                follower.walking_in = Some(actor.target.unwrap_or(cell));
            }
        }
        resumed.settle_walk_ins();
        *self = resumed;
        Ok(())
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
    /// One more step toward a walk-in's destination, X first; returns whether
    /// one started. A step in progress runs out first. At the destination the
    /// walk is over, unless the member ahead is still walking in: then this
    /// one waits there for its next step.
    fn continue_walk_in(&mut self, ahead_walking_in: bool) -> bool {
        let Some(destination) = self.walking_in else {
            return false;
        };
        if self.step.is_some() {
            return false;
        }
        let dir = match (
            destination.x.cmp(&self.cell.x),
            destination.y.cmp(&self.cell.y),
        ) {
            (Ordering::Greater, _) => Direction::Right,
            (Ordering::Less, _) => Direction::Left,
            (_, Ordering::Greater) => Direction::Down,
            (_, Ordering::Less) => Direction::Up,
            (Ordering::Equal, Ordering::Equal) => {
                if !ahead_walking_in {
                    self.walking_in = None;
                }
                return false;
            }
        };
        let Some(to) = self.cell.neighbor(dir) else {
            self.walking_in = None;
            return false;
        };
        self.facing = dir;
        self.step = Some(FollowerStep {
            dir,
            to,
            progress: 0,
        });
        true
    }

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

    /// `Party::walk_in`: the fifth stands at their own stand until the leader
    /// steps, walks there X first, and is on the trail once they arrive.
    #[test]
    fn a_member_off_the_trail_walks_in_and_rejoins_it() {
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let mut party = Party::new(&map, Cell::new(5, 5), Direction::Down, frames, 1).unwrap();
        let far = Cell::new(7, 7);
        party.followers[0].cell = far;
        party.walk_in(1);
        // Not until the leader moves.
        for _ in 0..4 {
            party.tick(&map, Input::Neutral);
        }
        assert_eq!(party.member(1).unwrap().cell, far);

        // One step of the leader's, from (5,5): the destination is (5,5).
        party.tick(&map, Input::Direction(Direction::Up));
        let mut seen = vec![party.member(1).unwrap().cell];
        for _ in 0..40 {
            party.tick(&map, Input::Neutral);
            let cell = party.member(1).unwrap().cell;
            if seen.last() != Some(&cell) {
                seen.push(cell);
            }
        }
        assert_eq!(
            seen,
            vec![
                Cell::new(7, 7),
                Cell::new(6, 7),
                Cell::new(5, 7),
                Cell::new(5, 6),
                Cell::new(5, 5),
            ],
            "X first, a step at a time"
        );
        assert_eq!(party.followers[0].walking_in, None, "on the trail");

        // On the trail: the next step of the leader's is the ordinary one.
        let leader = party.member(0).unwrap().cell;
        party.tick(&map, Input::Direction(Direction::Up));
        assert_eq!(party.followers[0].step.unwrap().to, leader);
    }

    /// A member walking in from an earlier slot leads the ones behind it:
    /// each takes the cell it vacates, step by step, while the leader stands,
    /// and none snaps across a gap.
    #[test]
    fn the_chain_follows_a_member_walking_in_from_an_earlier_slot() {
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let mut party = Party::new(&map, Cell::new(1, 1), Direction::Down, frames, 3).unwrap();
        // The leader at (1,1); slot 1 at its stand (6,1), slots 2-3 behind it.
        party.followers[0].cell = Cell::new(6, 1);
        party.followers[1].cell = Cell::new(6, 2);
        party.followers[2].cell = Cell::new(6, 3);
        party.walk_in(1);
        party.tick(&map, Input::Direction(Direction::Down));
        let mut last: Vec<Cell> = (1..4).map(|m| party.member(m).unwrap().cell).collect();
        for _ in 0..60 {
            party.tick(&map, Input::Neutral);
            let now: Vec<Cell> = (1..4).map(|m| party.member(m).unwrap().cell).collect();
            for (before, after) in last.iter().zip(&now) {
                let moved = before.x.abs_diff(after.x) + before.y.abs_diff(after.y);
                assert!(moved <= 1, "no snap: {before:?} -> {after:?}");
            }
            last = now;
        }
        assert!(
            party.followers.iter().all(|f| f.walking_in.is_none()),
            "all on the trail"
        );
        // A trail: each member a step behind the one ahead.
        for m in 1..4 {
            let (a, b) = (party.member(m - 1).unwrap().cell, party.member(m).unwrap().cell);
            assert_eq!(a.x.abs_diff(b.x) + a.y.abs_diff(b.y), 1, "{a:?} {b:?}");
        }
    }

    /// The walk survives a scene: the cast carries the follower's step and
    /// destination, and the handback continues or ends the walk where the
    /// scene left it. A follower on the trail is seeded as before, on its cell.
    #[test]
    fn a_walk_in_crosses_a_scene_handoff() {
        use crate::scene::{ActorRef, ScriptedActor};
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let mut party = Party::new(&map, Cell::new(1, 1), Direction::Down, frames, 2).unwrap();
        party.followers[1].cell = Cell::new(6, 1);
        party.walk_in(2);
        party.tick(&map, Input::Direction(Direction::Down));
        party.tick(&map, Input::Neutral);
        let mut cast: Vec<ScriptedActor> = (0..3)
            .map(|m| {
                let view = party.member(m).unwrap();
                ScriptedActor::new(ActorRef::PartyMember(m), view.cell, view.facing)
            })
            .collect();
        party.seed_scripted(&mut cast);
        assert!(cast[2].target.is_some() || cast[2].step.is_some(), "{:?}", cast[2]);
        assert!(cast[1].step.is_none() && cast[1].target.is_none(), "on the trail");

        // Handed back mid-walk: the follower keeps walking without the leader.
        let mut resumed = party.clone();
        resumed.resume_scripted(&map, &cast).unwrap();
        let before = resumed.member(2).unwrap().cell;
        for _ in 0..8 {
            resumed.tick(&map, Input::Neutral);
        }
        assert_ne!(resumed.member(2).unwrap().cell, before, "the walk went on");

        // Handed back at the destination: the walk is over.
        let mut there = cast.clone();
        let destination = cast[2].target.unwrap_or(cast[2].cell);
        there[2].cell = destination;
        there[2].target = None;
        there[2].step = None;
        let mut ended = party.clone();
        ended.resume_scripted(&map, &there).unwrap();
        assert_eq!(ended.followers[1].walking_in, None);
    }

    /// A destination that moves is followed: the walker never catches a leader
    /// who keeps walking, and a map load ends the walk.
    #[test]
    fn a_walk_in_follows_the_trail_and_ends_with_the_map() {
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let mut party = Party::new(&map, Cell::new(5, 6), Direction::Up, frames, 1).unwrap();
        party.followers[0].cell = Cell::new(7, 6);
        party.walk_in(1);
        for _ in 0..8 {
            party.tick(&map, Input::Direction(Direction::Up));
        }
        assert!(party.followers[0].walking_in.is_some());
        party
            .enter_map(&map, Cell::new(3, 3), Direction::Up)
            .unwrap();
        assert_eq!(party.followers[0].walking_in, None);
        assert_eq!(party.member(1).unwrap().cell, Cell::new(3, 3));
    }
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
