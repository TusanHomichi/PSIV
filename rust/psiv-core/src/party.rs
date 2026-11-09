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
    /// The cell it walks to: `dest_x_pos`/`dest_y_pos`, written only when the
    /// member ahead *begins* a step, with the cell that member is leaving.
    dest: Cell,
    /// Whether it walks to a destination more than a step away
    /// ([`Party::walk_in`]) rather than holding the one-cell trail.
    walk: Walk,
}

/// How a follower reaches its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    /// On the trail: the destination is a step away, or it is not followed.
    Trail,
    /// Off the trail and standing until the member ahead first steps.
    Armed,
    /// Off the trail and walking to its destination, X first.
    Walking,
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
            dest: leader.cell(),
            walk: Walk::Trail,
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
    /// A warp's load then lines the followers up behind the leader
    /// ([`Party::line_up`]).
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
            follower.dest = follower.cell;
            follower.walk = Walk::Trail;
        }
        Ok(())
    }

    /// Lines the followers up behind the leader, one cell apart in `toward`,
    /// each at rest and facing the leader's way: a warp's load
    /// (`loc_535D4`, `ps4.asm:110848-110885`). Every member is placed on the
    /// start cell, and each gives the member behind it a destination one cell
    /// along `Map_Start_Char_Align` (`loc_53642`'s table: 0 none, 4 down,
    /// 8 up, 12 left, 16 right). They walk it during the load, so on tape 06
    /// (members patched into slots three and four before the door into map
    /// `$14`, whose warp aligns down) the four stand at pixel rows 304, 320,
    /// 336 and 352 in one column by the load's end, before control. A cell off the map keeps its member
    /// on the one ahead.
    pub fn line_up(&mut self, map: &FieldMap, toward: Direction) {
        let mut ahead = self.leader.cell();
        let facing = self.leader.facing();
        for follower in &mut self.followers {
            let cell = ahead
                .neighbor(toward)
                .filter(|cell| cell.x < map.width() && cell.y < map.height())
                .unwrap_or(ahead);
            follower.cell = cell;
            follower.facing = facing;
            follower.step = None;
            follower.dest = cell;
            follower.walk = Walk::Trail;
            ahead = cell;
        }
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
            self.start_steps(map, Some(leader_from));
        }
        self.advance_followers(map);

        effects
    }

    /// Starts every step that is due, in slot order, the first follower
    /// handed `handed` as its destination. As `FieldObj_Move`'s tail write,
    /// a follower hands the cell it leaves to the one behind only on the tick
    /// it actually *begins* a step: one that stands still, stacked or waiting,
    /// hands on nothing.
    fn start_steps(&mut self, map: &FieldMap, mut handed: Option<Cell>) {
        let mut ahead_off_trail = false;
        for follower in &mut self.followers {
            follower.take(handed);
            handed = follower.start_due(map, ahead_off_trail);
            ahead_off_trail = follower.walk != Walk::Trail;
        }
    }

    /// Advances every follower's step in progress by one tick. One that lands
    /// short of its destination (a walk-in) starts its next step at once and
    /// hands the cell it leaves to the one behind, so the chain follows a
    /// member who walks while the leader stands.
    fn advance_followers(&mut self, map: &FieldMap) {
        let frames = self.leader.step_frames().get();
        let mut handed = None;
        let mut ahead_off_trail = false;
        for follower in &mut self.followers {
            follower.take(handed);
            if let Some(step) = follower.step.as_mut() {
                step.progress = step.progress.saturating_add(1);
                if step.progress >= frames {
                    follower.cell = step.to;
                    follower.step = None;
                }
            }
            handed = follower.start_due(map, ahead_off_trail);
            ahead_off_trail = follower.walk != Walk::Trail;
        }
    }

    /// Seeds a scene's cast with each follower's live motion: its step in
    /// progress, and, for one walking in, its destination as the actor's
    /// target, so the scene sees the party moving
    /// (`Event_PickingFifthCharacter`'s wait), the scene's own follow chain
    /// carries the walk on, and [`Party::resume_scripted`] hands it back.
    pub fn seed_scripted(&self, actors: &mut [crate::scene::ScriptedActor]) {
        for actor in actors {
            let crate::scene::ActorRef::PartyMember(slot) = actor.actor else {
                continue;
            };
            let Some(follower) = slot.checked_sub(1).and_then(|i| self.followers.get(i)) else {
                continue;
            };
            actor.step = follower.step.map(|s| (s.dir, s.to, s.progress));
            actor.target = (follower.walk != Walk::Trail && follower.dest != follower.cell)
                .then_some(follower.dest);
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
            if follower.walk == Walk::Trail {
                follower.walk = Walk::Armed;
                if follower.step.is_none() {
                    follower.dest = follower.cell;
                }
            }
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
            let landing = follower.step.map_or(cell, |step| step.to);
            follower.dest = landing;
            // A walk-in keeps going from where the scene left it: to the
            // target the scene still had (the scene's follow chain hands it
            // on, as the field's does), or ending where it stands. One the
            // scene moved has been handed a cell, so it is armed no longer.
            if follower.walk != Walk::Trail {
                follower.dest = actor.target.unwrap_or(landing);
                if follower.dest != cell {
                    follower.walk = Walk::Walking;
                }
            }
        }
        // Steps due now start now, each handing its cell to the one behind.
        resumed.start_steps(map, None);
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
    /// Takes the cell the member ahead began a step from, if it began one.
    /// A follower mid-step keeps its step and goes on once it lands.
    fn take(&mut self, handed: Option<Cell>) {
        if let Some(cell) = handed {
            self.dest = cell;
            if self.walk == Walk::Armed {
                self.walk = Walk::Walking;
            }
        }
    }

    /// Starts the step toward the destination, if one is due, and returns the
    /// cell it leaves for the follower behind; `None` when it does not move.
    ///
    /// A destination a step away is stepped onto, on a wrapping map too. A
    /// walk-in further away closes the X gap first (`FieldObj_GetAutoInput`).
    /// An ordinary follower further away snaps, which happens only when a
    /// caller moved the party without the trail. At its destination a walk-in
    /// is back on the trail, unless the member ahead is still off it: then it
    /// waits there for its next destination.
    fn start_due(&mut self, map: &FieldMap, ahead_off_trail: bool) -> Option<Cell> {
        if self.step.is_some() {
            return None;
        }
        let from = self.cell;
        if self.dest == from {
            if self.walk == Walk::Walking && !ahead_off_trail {
                self.walk = Walk::Trail;
            }
            return None;
        }
        let adjacent = Direction::ALL
            .into_iter()
            .find(|&dir| map.neighbor(from, dir) == Some(self.dest));
        let dir = match adjacent {
            Some(dir) => dir,
            None if self.walk != Walk::Trail => {
                let dir = match (self.dest.x.cmp(&from.x), self.dest.y.cmp(&from.y)) {
                    (Ordering::Greater, _) => Direction::Right,
                    (Ordering::Less, _) => Direction::Left,
                    (_, Ordering::Greater) => Direction::Down,
                    _ => Direction::Up,
                };
                if map.neighbor(from, dir).is_none() {
                    // Off the edge of a bounded map: the walk ends here.
                    self.dest = from;
                    self.walk = Walk::Trail;
                    return None;
                }
                dir
            }
            None => {
                // Unreachable while the trail's invariant holds. If a caller
                // breaks it (moving the leader without the party, say), snap
                // rather than desync the trail or invent a multi-step path.
                self.cell = self.dest;
                return Some(from);
            }
        };
        let to = map.neighbor(from, dir)?;
        self.facing = dir;
        self.step = Some(FollowerStep {
            dir,
            to,
            progress: 0,
        });
        Some(from)
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
        assert_eq!(party.followers[0].walk, Walk::Trail, "on the trail");

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
            party.followers.iter().all(|f| f.walk == Walk::Trail),
            "all on the trail"
        );
        // A trail: each member a step behind the one ahead.
        for m in 1..4 {
            let (a, b) = (
                party.member(m - 1).unwrap().cell,
                party.member(m).unwrap().cell,
            );
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
        // Slot 1 leaves the stack on the leader's second step, and hands slot
        // 2 the cell it left.
        for _ in 0..2 {
            party.tick(&map, Input::Direction(Direction::Down));
            party.tick(&map, Input::Neutral);
        }
        party.tick(&map, Input::Neutral);
        let mut cast: Vec<ScriptedActor> = (0..3)
            .map(|m| {
                let view = party.member(m).unwrap();
                ScriptedActor::new(ActorRef::PartyMember(m), view.cell, view.facing)
            })
            .collect();
        party.seed_scripted(&mut cast);
        assert!(
            cast[2].target.is_some() || cast[2].step.is_some(),
            "{:?}",
            cast[2]
        );
        assert!(
            cast[1].step.is_none() && cast[1].target.is_none(),
            "on the trail"
        );

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
        assert_eq!(ended.followers[1].walk, Walk::Trail);
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
        assert_ne!(party.followers[0].walk, Walk::Trail);
        party
            .enter_map(&map, Cell::new(3, 3), Direction::Up)
            .unwrap();
        assert_eq!(party.followers[0].walk, Walk::Trail);
        assert_eq!(party.member(1).unwrap().cell, Cell::new(3, 3));
    }

    fn cells(party: &Party) -> Vec<Cell> {
        party.members().iter().map(|m| m.cell).collect()
    }

    fn distance(a: Cell, b: Cell) -> u16 {
        a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
    }

    /// Ticks the party with `input`, checking that no member moves more than
    /// a cell in a tick (no snap).
    fn tick_checked(party: &mut Party, map: &FieldMap, input: Input) {
        let before = cells(party);
        party.tick(map, input);
        for (a, b) in before.iter().zip(&cells(party)) {
            assert!(distance(*a, *b) <= 1, "no snap: {a:?} -> {b:?}");
        }
    }

    /// Ticks the standing party until every member is at rest and on the
    /// trail, then checks the trail: each member a step behind the one ahead.
    fn settle_contiguous(party: &mut Party, map: &FieldMap, label: &str) {
        for _ in 0..200 {
            tick_checked(party, map, Input::Neutral);
        }
        assert!(
            party
                .followers
                .iter()
                .all(|f| f.walk == Walk::Trail && f.step.is_none()),
            "{label}: all at rest on the trail: {:?}",
            party.followers
        );
        let now = cells(party);
        for pair in now.windows(2) {
            assert_eq!(
                distance(pair[0], pair[1]),
                1,
                "{label}: contiguous: {now:?}"
            );
        }
    }

    /// Runs a scene of `ops` over the party for `ticks` scene ticks, then
    /// hands the party back, as the runtime does.
    fn through_scene(
        party: &Party,
        map: &FieldMap,
        ops: &'static [crate::scene::SceneOp],
        ticks: usize,
    ) -> Party {
        use crate::scene::{ActorRef, ScriptedActor};
        let mut cast: Vec<ScriptedActor> = (0..party.len())
            .map(|m| {
                let view = party.member(m).unwrap();
                ScriptedActor::new(ActorRef::PartyMember(m), view.cell, view.facing)
            })
            .collect();
        party.seed_scripted(&mut cast);
        let mut runner =
            crate::scene_runner::SceneRunner::new(ops, cast, party.leader.step_frames());
        let mut state = crate::GameState::new();
        for _ in 0..ticks {
            runner.tick(map, &mut state, crate::scene::SceneInput::None);
        }
        let mut resumed = party.clone();
        resumed.resume_scripted(map, runner.actors()).unwrap();
        resumed
    }

    static WAIT_FIVE: [crate::scene::SceneOp; 1] =
        [crate::scene::SceneOp::WaitFrames { frames: 5 }];

    /// A scene entered and left while several members walk in, at every tick
    /// of the walk: between steps, mid-step, after the first arrives, and
    /// before the leader has moved at all. The members behind keep their
    /// destinations across the handback and every step started on resuming
    /// hands its cell on, so the chain finishes contiguous, without a stall
    /// or a snap.
    #[test]
    fn a_walking_chain_crosses_a_scene_at_every_tick() {
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let chain = || {
            let mut party = Party::new(&map, Cell::new(1, 1), Direction::Down, frames, 3).unwrap();
            party.followers[0].cell = Cell::new(6, 1);
            party.followers[1].cell = Cell::new(6, 2);
            party.followers[2].cell = Cell::new(6, 3);
            party.walk_in(1);
            party
        };
        // Before the leader's first step: the walk stays armed.
        let armed = through_scene(&chain(), &map, &WAIT_FIVE, 6);
        assert!(armed.followers.iter().all(|f| f.walk == Walk::Armed));
        let mut armed = armed;
        tick_checked(&mut armed, &map, Input::Direction(Direction::Down));
        settle_contiguous(&mut armed, &map, "armed");

        // From the leader's landing on (a scene starts with the leader at rest).
        for field_ticks in 2..30 {
            for scene_ticks in [1, 2, 3, 6, 11] {
                let mut party = chain();
                tick_checked(&mut party, &map, Input::Direction(Direction::Down));
                for _ in 1..field_ticks {
                    tick_checked(&mut party, &map, Input::Neutral);
                }
                let mut resumed = through_scene(&party, &map, &WAIT_FIVE, scene_ticks);
                settle_contiguous(
                    &mut resumed,
                    &map,
                    &format!("{field_ticks} field ticks, {scene_ticks} scene ticks"),
                );
            }
        }
    }

    /// Codex's case on the PR: two field ticks into the chain's walk, a
    /// five-frame scene of six ticks, and back. Every member ends a step
    /// behind the one ahead.
    #[test]
    fn steps_started_on_a_handback_hand_their_cells_on() {
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let mut party = Party::new(&map, Cell::new(1, 1), Direction::Down, frames, 3).unwrap();
        party.followers[0].cell = Cell::new(6, 1);
        party.followers[1].cell = Cell::new(6, 2);
        party.followers[2].cell = Cell::new(6, 3);
        party.walk_in(1);
        party.tick(&map, Input::Direction(Direction::Down));
        party.tick(&map, Input::Neutral);
        let mut resumed = through_scene(&party, &map, &WAIT_FIVE, 6);
        settle_contiguous(&mut resumed, &map, "two ticks, six");
        assert_eq!(
            cells(&resumed),
            vec![
                Cell::new(1, 2),
                Cell::new(1, 1),
                Cell::new(2, 1),
                Cell::new(3, 1)
            ]
        );
    }

    /// A member that does not begin a step hands nothing on: with the members
    /// ahead stacked on the leader, a friend joining in any slot stands until
    /// the member directly ahead first steps, and then walks to the cell that
    /// member left. Checked for every joining slot, with a leader that walks,
    /// turns back on itself, and stops.
    #[test]
    fn a_stationary_member_hands_nothing_on() {
        let map = open_map();
        let frames = StepFrames::new(2).unwrap();
        let path = [
            Direction::Down,
            Direction::Down,
            Direction::Right,
            Direction::Left,
            Direction::Up,
            Direction::Right,
            Direction::Right,
            Direction::Down,
        ];
        for slot in 1..MAX_PARTY_MEMBERS {
            let start = Cell::new(1, 1);
            let stand = Cell::new(6, 6);
            let mut party = Party::new(&map, start, Direction::Down, frames, 4).unwrap();
            party.followers[slot - 1].cell = stand;
            party.walk_in(slot);
            for (n, &dir) in path.iter().enumerate() {
                let ahead_before = party.member(slot - 1).unwrap().cell;
                let ahead_was_stepping = party.member(slot - 1).unwrap().is_stepping;
                tick_checked(&mut party, &map, Input::Direction(dir));
                let ahead_began =
                    !ahead_was_stepping && party.member(slot - 1).unwrap().is_stepping;
                let joiner = party.followers[slot - 1];
                if n + 1 < slot {
                    // The member ahead is still in the stack: it has not
                    // stepped, so the joiner stands, armed.
                    assert!(!ahead_began, "slot {slot}, step {n}");
                    assert_eq!(joiner.cell, stand, "slot {slot}, step {n}");
                    assert!(joiner.step.is_none(), "slot {slot}, step {n}");
                    assert_eq!(joiner.walk, Walk::Armed, "slot {slot}, step {n}");
                    assert_eq!(joiner.dest, stand, "slot {slot}, step {n}");
                } else if n + 1 == slot {
                    // The member ahead's first step: the joiner is handed the
                    // cell it left, the stack's cell, and sets out.
                    assert!(ahead_began, "slot {slot}, step {n}");
                    assert_eq!(joiner.dest, ahead_before, "slot {slot}, step {n}");
                    assert!(joiner.step.is_some(), "slot {slot}, step {n}");
                }
                // The members behind the joiner move only once it has.
                if n + 1 < slot {
                    for behind in &party.followers[slot..] {
                        assert_eq!(behind.cell, start, "slot {slot}, step {n}");
                    }
                }
                tick_checked(&mut party, &map, Input::Neutral);
            }
            // The leader stops; the joiner finishes its walk alone.
            for _ in 0..200 {
                tick_checked(&mut party, &map, Input::Neutral);
            }
            assert!(
                party
                    .followers
                    .iter()
                    .all(|f| f.walk == Walk::Trail && f.step.is_none())
            );
            let now = cells(&party);
            for pair in now.windows(2) {
                assert!(distance(pair[0], pair[1]) <= 1, "slot {slot}: {now:?}");
            }
        }
    }

    /// An ordinary follower handed a destination while mid-step (a scene
    /// handed it back stepping) finishes that step, then takes the new one,
    /// rather than restarting; the one behind is handed a cell only when it
    /// does.
    #[test]
    fn a_step_in_progress_is_not_restarted() {
        use crate::scene::{ActorRef, ScriptedActor};
        let map = open_map();
        let frames = StepFrames::new(4).unwrap();
        let mut party = Party::new(&map, Cell::new(3, 3), Direction::Down, frames, 2).unwrap();
        let mut cast: Vec<ScriptedActor> = vec![
            ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(3, 3), Direction::Down),
            ScriptedActor::new(ActorRef::PartyMember(1), Cell::new(3, 1), Direction::Down),
            ScriptedActor::new(ActorRef::PartyMember(2), Cell::new(3, 1), Direction::Down),
        ];
        cast[1].step = Some((Direction::Down, Cell::new(3, 2), 1));
        party.resume_scripted(&map, &cast).unwrap();
        party.tick(&map, Input::Direction(Direction::Down));
        let first = party.followers[0];
        assert_eq!(first.step.unwrap().to, Cell::new(3, 2), "the same step");
        assert_eq!(first.step.unwrap().progress, 2, "not restarted");
        assert_eq!(first.dest, Cell::new(3, 3));
        assert!(party.followers[1].step.is_none(), "nothing handed on yet");
        party.tick(&map, Input::Neutral);
        party.tick(&map, Input::Neutral);
        // Landed on (3,2) and set out for (3,3), handing (3,2) on.
        assert_eq!(party.followers[0].cell, Cell::new(3, 2));
        assert_eq!(party.followers[0].step.unwrap().to, Cell::new(3, 3));
        assert_eq!(party.followers[1].dest, Cell::new(3, 2));
        assert_eq!(party.followers[1].step.unwrap().to, Cell::new(3, 2));
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
