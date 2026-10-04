//! The scene runner: executing a [`SceneOp`] list deterministically.
//!
//! Split from `scene.rs`, which owns the vocabulary; this file owns the
//! machine. See that module's docs for how blocking works and why scripted
//! actor motion reuses the ordinary movement model.

use crate::error::MapError;
use crate::field::StepFrames;
use crate::geom::Cell;
use crate::map::FieldMap;
use crate::scene::{
    ActorRef, OP_BUDGET_PER_TICK, SceneEffect, SceneFault, SceneInput, SceneOp, ScriptedActor,
};
#[cfg(test)]
mod map_update_tests;
mod ops;

use crate::state::{CharId, GameState, PARTY_SLOTS};

/// The cell a pixel position names, undoing the standing-cell shift.
fn pixel_cell(x: i32, y: i32) -> Cell {
    let cx = (x / crate::geom::CELL_PIXELS).clamp(0, i32::from(u16::MAX));
    let cy = ((y / crate::geom::CELL_PIXELS) + 1).clamp(0, i32::from(u16::MAX));
    Cell::new(cx as u16, cy as u16)
}

/// What the runner is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blocked {
    No,
    Ticks(u16),
    Actor(ActorRef),
    Dialogue,
    EndingContinue,
    Choice,
    Battle,
    Map,
    Camera,
    Done,
}

/// Runs one scene.
///
/// Construct it with the scene's op list and its cast, then [`SceneRunner::tick`]
/// once per frame until [`SceneRunner::is_finished`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneRunner {
    scene: &'static [SceneOp],
    pc: usize,
    actors: Vec<ScriptedActor>,
    party_slots: [Option<CharId>; PARTY_SLOTS],
    blocked: Blocked,
    dialogue_ended: bool,
    dialogue_window: crate::scene::DialogueWindow,
    step_frames: StepFrames,
    /// The retail follow chain: party followers trail the member ahead of
    /// them through scripted walks. `SetFollowMode` bit 0 turns it off for
    /// independently scripted party moves (the wake-up in the opening).
    follow_chain: bool,
    /// `SetFollowMode` bit 1 (`Char_Move_Flags`): close the Y gap before the
    /// X gap. Retail's default is X-first (`FieldObj_GetAutoInput`,
    /// `ps4.asm:93232`).
    y_first: bool,
}

impl SceneRunner {
    /// Starts a scene with its cast.
    ///
    /// The cast is explicit rather than discovered so a scene's actors have
    /// defined starting positions even before their first `MoveActor`.
    #[must_use]
    pub fn new(
        scene: &'static [SceneOp],
        cast: Vec<ScriptedActor>,
        step_frames: StepFrames,
    ) -> SceneRunner {
        SceneRunner {
            scene,
            pc: 0,
            actors: cast,
            party_slots: [None; PARTY_SLOTS],
            blocked: Blocked::No,
            dialogue_ended: false,
            dialogue_window: crate::scene::DialogueWindow::Standard,
            step_frames,
            follow_chain: true,
            y_first: false,
        }
    }

    /// Whether the scene has ended or faulted.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.blocked == Blocked::Done
    }

    /// Whether this boarding scene is waiting for the runtime's camera glide.
    #[must_use]
    pub fn is_waiting_for_camera(&self) -> bool {
        self.blocked == Blocked::Camera
    }

    /// Motion-loop frames update the map before testing arrival, including
    /// the final frame that unblocks into dialogue or ends the scene
    /// (ps4.asm:121117-121146,121484-121514).
    #[must_use]
    pub fn completes_map_update_loop(&self) -> bool {
        matches!(self.blocked, Blocked::Actor(_) | Blocked::Camera)
    }

    /// Whether this blocked scene frame runs the retail map-update loop.
    /// DoMapUpdateLoop/actor and camera loops call RunMapUpdates; dialogue,
    /// VInt_PrepareLoop, map loads and panel-only waits do not
    /// (ps4.asm:120957-121055,121117-121130,121484-121501).
    #[must_use]
    pub fn runs_map_updates(&self) -> bool {
        match self.blocked {
            Blocked::Ticks(_) => self
                .pc
                .checked_sub(1)
                .and_then(|pc| self.scene.get(pc))
                .is_some_and(|op| matches!(op, SceneOp::Wait { .. })),
            Blocked::Actor(_) | Blocked::Camera => true,
            _ => false,
        }
    }

    /// Window routine used by the last dialogue open or named resume.
    #[must_use]
    pub fn dialogue_window(&self) -> crate::scene::DialogueWindow {
        self.dialogue_window
    }

    /// The cast, for the renderer.
    #[must_use]
    pub fn actors(&self) -> &[ScriptedActor] {
        &self.actors
    }

    /// Replaces the cast mid-scene, keeping the program counter and any block.
    ///
    /// **Multi-map scenes need this.** [`SceneOp::LoadMap`] changes the map
    /// under a running scene — the opening event tours five maps — and an
    /// [`ActorRef::Npc`] index only means anything relative to one map's object
    /// list. So on a [`SceneEffect::MapRequested`] the runtime loads the map,
    /// then re-seats the cast for it before ticking again. Without that the
    /// scene would keep driving actors that no longer exist, and the runner
    /// would fault on the first op naming one.
    ///
    /// Actors not mentioned in the new cast are dropped; ones that persist keep
    /// whatever position the caller gives them, since a map change relocates
    /// everyone anyway.
    pub fn recast(&mut self, cast: Vec<ScriptedActor>) {
        self.actors = cast;
    }

    /// Reconciles the cast with a party change **without** touching live
    /// scripted state: refs that already exist keep their position, facing,
    /// and any walk in progress; new refs are added at the caller's seed;
    /// vanished refs drop.
    ///
    /// A full [`SceneRunner::recast`] here would re-seed every actor from
    /// field state the scene never moved — which teleported the opening's
    /// pair back into the bedroom mid-scene and sent the door walk through
    /// the wall. Party mutations move object contents (the runner's own
    /// `SwapCharSlots`/`CopyCharSlot`/`PromoteNpcToChar` arms); they never
    /// re-place the cast.
    pub fn sync_cast(&mut self, cast: Vec<ScriptedActor>) {
        self.actors = cast
            .into_iter()
            .map(|entry| {
                let mut current = self.actor(entry.actor).copied().unwrap_or(entry);
                current.actor = entry.actor;
                current
            })
            .collect();
    }

    /// One actor by reference.
    #[must_use]
    pub fn actor(&self, actor: ActorRef) -> Option<&ScriptedActor> {
        self.actor_index(actor).map(|index| &self.actors[index])
    }

    fn actor_index(&self, actor: ActorRef) -> Option<usize> {
        // Event_GetCharacter returns the same object as Character_1..5.
        // Resolve names through the current party order, never a second copy
        // of that character's position. Character-only test casts still work.
        if let ActorRef::Character(id) = actor
            && let Some(slot) = self.party_slots.iter().position(|&who| who == Some(id))
            && let Some(index) = self
                .actors
                .iter()
                .position(|a| a.actor == ActorRef::PartyMember(slot))
        {
            return Some(index);
        }
        self.actors.iter().position(|a| a.actor == actor)
    }

    fn actor_mut(&mut self, actor: ActorRef) -> Option<&mut ScriptedActor> {
        self.actor_index(actor).map(|index| &mut self.actors[index])
    }

    /// Advances the scene one tick.
    ///
    /// Actors move first, then the script advances as far as it can. Effects
    /// come out in that order, so an `ActorArrived` always precedes whatever
    /// the script does in response to it on the same tick.
    pub fn tick(
        &mut self,
        map: &FieldMap,
        state: &mut GameState,
        input: SceneInput,
    ) -> Vec<SceneEffect> {
        let mut effects = Vec::new();
        if self.blocked == Blocked::Done {
            return effects;
        }

        self.party_slots = state.party();
        for index in 0..self.actors.len() {
            if self.actor_index(self.actors[index].actor) != Some(index) {
                continue;
            }
            if let Some(at) = self.actors[index].tick(map, self.step_frames, self.y_first) {
                effects.push(SceneEffect::ActorArrived {
                    actor: self.actors[index].actor,
                    at,
                });
            }
        }
        self.tick_follow_chain();

        self.unblock(input);
        self.run(state, &mut effects);
        effects
    }

    /// The retail caterpillar through scripted walks: while the chain is on,
    /// each `PartyMember(n)` walks toward the cell `PartyMember(n-1)` is
    /// vacating (its step origin — the cell only commits on completion), so
    /// followers trail one cell behind at the shared step speed instead of
    /// standing where the walk began.
    fn tick_follow_chain(&mut self) {
        if !self.follow_chain {
            return;
        }
        let mut vacated: Option<Cell> = None;
        for slot in 0..crate::PARTY_SLOTS {
            let Some(index) = self
                .actors
                .iter()
                .position(|a| a.actor == ActorRef::PartyMember(slot))
            else {
                break;
            };
            let origin = self.actors[index]
                .is_stepping()
                .then_some(self.actors[index].cell);
            if slot > 0
                && let Some(cell) = vacated
                && self.actors[index].cell != cell
            {
                self.actors[index].target = Some(cell);
            }
            vacated = origin;
        }
    }

    /// Releases a block that this tick's input or actor state has satisfied.
    fn unblock(&mut self, input: SceneInput) {
        self.blocked = match self.blocked {
            Blocked::Ticks(0 | 1) => Blocked::No,
            Blocked::Ticks(n) => Blocked::Ticks(n - 1),
            Blocked::Actor(actor) => match self.actor(actor) {
                Some(a) if a.is_walking() => Blocked::Actor(actor),
                _ => Blocked::No,
            },
            Blocked::Dialogue
                if matches!(
                    input,
                    SceneInput::DialogueClosed | SceneInput::DialogueEnded
                ) =>
            {
                self.dialogue_ended = input == SceneInput::DialogueEnded;
                Blocked::No
            }
            Blocked::EndingContinue if input == SceneInput::EndingContinue => Blocked::No,
            Blocked::Battle if matches!(input, SceneInput::BattleFinished { .. }) => Blocked::No,
            Blocked::Map if input == SceneInput::MapLoaded => Blocked::No,
            Blocked::Camera if input == SceneInput::CameraArrived => Blocked::No,
            Blocked::Choice => match input {
                SceneInput::Choice(_) => Blocked::No,
                _ => Blocked::Choice,
            },
            other => other,
        };
        // A choice's answer decides the jump, so it is applied here where the
        // input is still in hand.
        if let (SceneInput::Choice(answer), Some(SceneOp::BranchChoice { if_yes, if_no })) =
            (input, self.scene.get(self.pc).copied())
            && self.blocked == Blocked::No
        {
            self.pc = if answer { if_yes } else { if_no };
        }
    }

    /// Executes ops until something blocks.
    fn run(&mut self, state: &mut GameState, effects: &mut Vec<SceneEffect>) {
        let mut budget = OP_BUDGET_PER_TICK;
        while self.blocked == Blocked::No {
            self.party_slots = state.party();
            if budget == 0 {
                effects.push(SceneEffect::Faulted(SceneFault::Runaway));
                self.blocked = Blocked::Done;
                return;
            }
            budget -= 1;

            let Some(op) = self.scene.get(self.pc).copied() else {
                // Running off the end is an implicit End rather than a fault:
                // a scene that simply stops has finished.
                effects.push(SceneEffect::Finished);
                self.blocked = Blocked::Done;
                return;
            };

            if let Some(fault) = self.step_op(op, state, effects) {
                effects.push(SceneEffect::Faulted(fault));
                self.blocked = Blocked::Done;
                return;
            }
        }
    }
}

/// A named scene, for the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scene {
    /// The `Event_*` label this was transcribed from.
    pub name: &'static str,
    /// The `Event_Index` value that selects it.
    pub event: crate::trigger::EventIndex,
    /// The op list.
    pub ops: &'static [SceneOp],
}

/// Builds a [`SceneRunner`] for a scene.
///
/// # Errors
///
/// [`MapError::ZeroStepFrames`] is impossible here; the signature returns a
/// `Result` so the registry lookup can fail once scenes exist.
pub fn runner_for(
    scene: &Scene,
    cast: Vec<ScriptedActor>,
    step_frames: StepFrames,
) -> Result<SceneRunner, MapError> {
    Ok(SceneRunner::new(scene.ops, cast, step_frames))
}
