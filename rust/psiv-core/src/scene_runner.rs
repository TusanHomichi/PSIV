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
    ActorRef, Axis, DialogueId, DialogueSource, OP_BUDGET_PER_TICK, SceneEffect, SceneFault,
    SceneInput, SceneOp, ScriptedActor,
};
use crate::state::GameState;

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
    Choice,
    Battle,
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
    blocked: Blocked,
    step_frames: StepFrames,
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
            blocked: Blocked::No,
            step_frames,
        }
    }

    /// Whether the scene has ended or faulted.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.blocked == Blocked::Done
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

    /// One actor by reference.
    #[must_use]
    pub fn actor(&self, actor: ActorRef) -> Option<&ScriptedActor> {
        self.actors.iter().find(|a| a.actor == actor)
    }

    fn actor_mut(&mut self, actor: ActorRef) -> Option<&mut ScriptedActor> {
        self.actors.iter_mut().find(|a| a.actor == actor)
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

        for index in 0..self.actors.len() {
            if let Some(at) = self.actors[index].tick(map, self.step_frames) {
                effects.push(SceneEffect::ActorArrived {
                    actor: self.actors[index].actor,
                    at,
                });
            }
        }

        self.unblock(input);
        self.run(state, &mut effects);
        effects
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
            Blocked::Dialogue if input == SceneInput::DialogueClosed => Blocked::No,
            Blocked::Battle if matches!(input, SceneInput::BattleFinished { .. }) => Blocked::No,
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

    /// Runs one op. Returns a fault instead of advancing when it cannot.
    fn step_op(
        &mut self,
        op: SceneOp,
        state: &mut GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        match op {
            SceneOp::MoveActor { actor, to } => {
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.target = Some(to);
                effects.push(SceneEffect::ActorMoveStarted { actor, to });
                self.pc += 1;
            }
            SceneOp::Face { actor, facing } => {
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.facing = facing;
                effects.push(SceneEffect::ActorFaced { actor, facing });
                self.pc += 1;
            }
            SceneOp::RunDialogue { source, .. } => {
                let dialogue = match source {
                    DialogueSource::Entry(id) => id,
                    DialogueSource::NpcDialogueId(actor) => {
                        // The entry index lives on the field object. The engine
                        // does not carry object dialogue ids, so it reports the
                        // source and lets the runtime read the map record.
                        match self.actor(actor) {
                            Some(_) => DialogueId(u16::MAX),
                            None => return Some(SceneFault::UnknownActor { actor }),
                        }
                    }
                };
                effects.push(match source {
                    DialogueSource::Entry(_) => SceneEffect::DialogueOpen(dialogue),
                    DialogueSource::NpcDialogueId(actor) => {
                        SceneEffect::DialogueOpenFromNpc { actor }
                    }
                });
                self.blocked = Blocked::Dialogue;
                self.pc += 1;
            }
            SceneOp::RunDialogueResume => {
                effects.push(SceneEffect::DialogueResume);
                self.blocked = Blocked::Dialogue;
                self.pc += 1;
            }
            SceneOp::AddMoney { amount } => {
                state.add_money(amount);
                effects.push(SceneEffect::MoneyChanged {
                    total: state.money(),
                });
                self.pc += 1;
            }
            SceneOp::PromoteNpcToChar {
                npc,
                char_id,
                slot,
                art_tile,
                facing,
            } => {
                if state.join_party(slot, char_id).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::NpcPromoted {
                    npc,
                    char_id,
                    slot,
                    art_tile,
                    facing,
                });
                effects.push(SceneEffect::PartyChanged);
                self.pc += 1;
            }
            SceneOp::StartBattle { index } => {
                effects.push(SceneEffect::BattleRequested { index });
                self.blocked = Blocked::Battle;
                self.pc += 1;
            }
            SceneOp::LoadMap { .. } => {
                effects.push(SceneEffect::MapRequested { op });
                self.pc += 1;
            }
            SceneOp::MoveActorTo { actor, x, y, wait } => {
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                let to = pixel_cell(x, y);
                walker.target = Some(to);
                let walking = walker.is_walking();
                effects.push(SceneEffect::ActorMoveStarted { actor, to });
                self.pc += 1;
                if wait && walking {
                    self.blocked = Blocked::Actor(actor);
                }
            }
            SceneOp::MoveActorCommand { actor, .. } => {
                if self.actor(actor).is_none() {
                    return Some(SceneFault::UnknownActor { actor });
                }
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::FaceOppositeOf { actor, of } => {
                let Some(other) = self.actor(of) else {
                    return Some(SceneFault::UnknownActor { actor: of });
                };
                let facing = other.facing.opposite();
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.facing = facing;
                effects.push(SceneEffect::ActorFaced { actor, facing });
                self.pc += 1;
            }
            SceneOp::PlaceActor { actor, x, y } => {
                let cell = pixel_cell(x, y);
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.cell = cell;
                walker.target = None;
                effects.push(SceneEffect::ActorPlaced { actor, at: cell });
                self.pc += 1;
            }
            SceneOp::SetActorDest { actor, x, y } => {
                let cell = pixel_cell(x, y);
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.target = Some(cell);
                effects.push(SceneEffect::ActorMoveStarted { actor, to: cell });
                self.pc += 1;
            }
            SceneOp::Return { value } => {
                effects.push(SceneEffect::Returned { value });
                effects.push(SceneEffect::Finished);
                self.blocked = Blocked::Done;
            }
            SceneOp::BranchIfActorGreater {
                a,
                b,
                axis,
                if_greater,
                if_not,
            } => {
                let (Some(first), Some(second)) = (self.actor(a), self.actor(b)) else {
                    let missing = if self.actor(a).is_none() { a } else { b };
                    return Some(SceneFault::UnknownActor { actor: missing });
                };
                let greater = match axis {
                    Axis::X => first.cell.x > second.cell.x,
                    Axis::Y => first.cell.y > second.cell.y,
                };
                let target = if greater { if_greater } else { if_not };
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            SceneOp::SwapCharSlots { .. }
            | SceneOp::ReloadMapPalette
            | SceneOp::InitVramAndCram
            | SceneOp::LoadPalette { .. }
            | SceneOp::SetCameraPos { .. }
            | SceneOp::LoadTitleImage { .. }
            | SceneOp::SetTextColour { .. }
            | SceneOp::DrawTextToPlane { .. }
            | SceneOp::IntroTextFadeUp
            | SceneOp::IntroTextFadeDown
            | SceneOp::OverlapCharacters
            | SceneOp::SetFollowMode { .. }
            | SceneOp::SetStepOffset { .. }
            | SceneOp::PlaySound { .. }
            | SceneOp::SetSavedMusic { .. }
            | SceneOp::FadeIn
            | SceneOp::FadeOut
            | SceneOp::SetDialogueTree { .. }
            | SceneOp::SetRenderSpritesInCutscene { .. } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::WaitFrames { frames } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
                if frames > 0 {
                    self.blocked = Blocked::Ticks(frames);
                }
            }
            SceneOp::SetFlag { flag, value } => {
                if state.write(flag, value).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::FlagChanged { flag, value });
                self.pc += 1;
            }
            SceneOp::JoinParty { slot, who } => {
                if state.join_party(slot, who).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::PartyChanged);
                self.pc += 1;
            }
            SceneOp::SetParty { slots } => {
                state.set_party(slots);
                effects.push(SceneEffect::PartyChanged);
                self.pc += 1;
            }
            SceneOp::DespawnNpc { npc_index, count } => {
                effects.push(SceneEffect::NpcDespawned { npc_index, count });
                self.pc += 1;
            }
            SceneOp::MoveCamera { .. } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::Wait { ticks } => {
                self.pc += 1;
                if ticks > 0 {
                    self.blocked = Blocked::Ticks(ticks);
                }
            }
            SceneOp::WaitForActor { actor } => {
                let Some(walker) = self.actor(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                let walking = walker.is_walking();
                self.pc += 1;
                if walking {
                    self.blocked = Blocked::Actor(actor);
                }
            }
            SceneOp::BranchFlag {
                flag,
                if_set,
                if_clear,
            } => {
                let target = if state.is_set(flag) { if_set } else { if_clear };
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            SceneOp::BranchChoice { if_yes, if_no } => {
                if if_yes > self.scene.len() || if_no > self.scene.len() {
                    return Some(SceneFault::BadJump {
                        target: if_yes.max(if_no),
                    });
                }
                effects.push(SceneEffect::ChoiceRequested);
                self.blocked = Blocked::Choice;
            }
            SceneOp::BranchIfAligned {
                a,
                b,
                axis,
                if_aligned,
                if_not,
            } => {
                let (Some(first), Some(second)) = (self.actor(a), self.actor(b)) else {
                    let missing = if self.actor(a).is_none() { a } else { b };
                    return Some(SceneFault::UnknownActor { actor: missing });
                };
                let aligned = match axis {
                    Axis::X => first.cell.x == second.cell.x,
                    Axis::Y => first.cell.y == second.cell.y,
                };
                let target = if aligned { if_aligned } else { if_not };
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            SceneOp::CopyCharSlot { from, to } => {
                // Object-level, not party-level: no GameState write here.
                effects.push(SceneEffect::CharSlotCopied { from, to });
                self.pc += 1;
            }
            SceneOp::SetArtTile { actor, .. } => {
                if self.actor(actor).is_none() {
                    return Some(SceneFault::UnknownActor { actor });
                }
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::Jump { to } => {
                if to > self.scene.len() {
                    return Some(SceneFault::BadJump { target: to });
                }
                self.pc = to;
            }
            SceneOp::End => {
                effects.push(SceneEffect::Finished);
                self.blocked = Blocked::Done;
            }
        }
        None
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
