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
    EndingContinue,
    Choice,
    Battle,
    Map,
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
            blocked: Blocked::No,
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
            .map(|entry| self.actor(entry.actor).copied().unwrap_or(entry))
            .collect();
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
            Blocked::Dialogue if input == SceneInput::DialogueClosed => Blocked::No,
            Blocked::EndingContinue if input == SceneInput::EndingContinue => Blocked::No,
            Blocked::Battle if matches!(input, SceneInput::BattleFinished { .. }) => Blocked::No,
            Blocked::Map if input == SceneInput::MapLoaded => Blocked::No,
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
            SceneOp::RunDialogueResume | SceneOp::RunDialogueResumeWithWindow { .. } => {
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
            SceneOp::SetVehicleIndex { index } => {
                state.set_vehicle_index(index);
                effects.push(SceneEffect::VehicleChanged { index });
                self.pc += 1;
            }
            SceneOp::AddItem { item } => {
                if state.inventory_mut().add(item).is_err() {
                    return Some(SceneFault::BadWrite);
                }
                effects.push(SceneEffect::InventoryChanged);
                self.pc += 1;
            }
            SceneOp::RestorePartyHp { amount } => {
                let members = state.party_members();
                for who in members {
                    if let Some(stats) = state.roster_mut().get_mut(who) {
                        stats.curr_hp = stats.curr_hp.saturating_add(amount).min(stats.max_hp);
                        stats.status = 0;
                    }
                    effects.push(SceneEffect::RosterChanged { who });
                }
                self.pc += 1;
            }
            SceneOp::ConfigureCharacter {
                who,
                equipment,
                restore_hp_tp,
            } => {
                if let Some(stats) = state.roster_mut().get_mut(who) {
                    stats.equipment = equipment;
                    if restore_hp_tp {
                        stats.curr_hp = stats.max_hp;
                        stats.curr_tp = stats.max_tp;
                    }
                }
                effects.push(SceneEffect::RosterChanged { who });
                self.pc += 1;
            }
            SceneOp::SetCharacterEquipment { who, slots } => {
                if let Some(stats) = state.roster_mut().get_mut(who) {
                    for (target, replacement) in stats.equipment.iter_mut().zip(slots) {
                        if let Some(item) = replacement {
                            *target = item;
                        }
                    }
                }
                effects.push(SceneEffect::RosterChanged { who });
                self.pc += 1;
            }
            SceneOp::ClearCharacterStatus { who } => {
                if let Some(stats) = state.roster_mut().get_mut(who) {
                    stats.status = 0;
                }
                effects.push(SceneEffect::RosterChanged { who });
                self.pc += 1;
            }
            SceneOp::ReviveIfDead { who } => {
                if let Some(stats) = state.roster_mut().get_mut(who) {
                    stats.status = 0;
                    if stats.curr_hp == 0 {
                        stats.curr_hp = stats.max_hp;
                    }
                }
                effects.push(SceneEffect::RosterChanged { who });
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
                // Retail builds the new character object at the NPC's field
                // position: the party object for `slot` is seated there.
                let seat = self.actor(ActorRef::Npc(npc)).map(|a| a.cell);
                if let Some(cell) = seat {
                    match self.actor_mut(ActorRef::PartyMember(slot)) {
                        Some(member) => member.park(cell, facing),
                        None => self.actors.push(ScriptedActor::new(
                            ActorRef::PartyMember(slot),
                            cell,
                            facing,
                        )),
                    }
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
                // The runtime must load the new map and recast the runner
                // before any following op can name a map-local NPC. Retail
                // returns to the event routine only after RefreshMap has
                // rebuilt those objects; keeping this edge explicit prevents
                // a scene from driving the old map's object list for one
                // accidental tick.
                self.blocked = Blocked::Map;
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
            SceneOp::MoveActorToActor {
                actor,
                target,
                wait,
            } => {
                let Some(to) = self.actor(target).map(|a| a.cell) else {
                    return Some(SceneFault::UnknownActor { actor: target });
                };
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.target = Some(to);
                let walking = walker.is_walking();
                effects.push(SceneEffect::ActorMoveStarted { actor, to });
                self.pc += 1;
                if wait && walking {
                    self.blocked = Blocked::Actor(actor);
                }
            }
            SceneOp::MoveActorToActorAxis {
                actor,
                target,
                axis,
                wait,
            } => {
                let Some(target_actor) = self.actor(target) else {
                    return Some(SceneFault::UnknownActor { actor: target });
                };
                let Some(walker) = self.actor(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                let mut to = walker.cell;
                match axis {
                    Axis::X => to.x = target_actor.cell.x,
                    Axis::Y => to.y = target_actor.cell.y,
                }
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                walker.target = Some(to);
                let walking = walker.is_walking();
                effects.push(SceneEffect::ActorMoveStarted { actor, to });
                self.pc += 1;
                if wait && walking {
                    self.blocked = Blocked::Actor(actor);
                }
            }
            SceneOp::MoveActorOffset {
                actor,
                dx,
                dy,
                wait,
            } => {
                let Some(walker) = self.actor(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                let x = (i32::from(walker.cell.x) + dx.div_euclid(crate::geom::CELL_PIXELS))
                    .clamp(0, i32::from(u16::MAX)) as u16;
                let y = (i32::from(walker.cell.y) + dy.div_euclid(crate::geom::CELL_PIXELS))
                    .clamp(0, i32::from(u16::MAX)) as u16;
                let to = crate::geom::Cell::new(x, y);
                let Some(walker) = self.actor_mut(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
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
            // The three-way `trap #1` exchanges the two character OBJECTS —
            // positions and walk state travel with the object contents, which
            // is how `Event_GameStart` puts Alys (and her doorstep position)
            // in front after the party-slot rewrite.
            SceneOp::SwapCharSlots { a, b } => {
                let ia = self
                    .actors
                    .iter()
                    .position(|x| x.actor == ActorRef::PartyMember(a));
                let ib = self
                    .actors
                    .iter()
                    .position(|x| x.actor == ActorRef::PartyMember(b));
                if let (Some(ia), Some(ib)) = (ia, ib) {
                    self.actors.swap(ia, ib);
                    let name = self.actors[ia].actor;
                    self.actors[ia].actor = self.actors[ib].actor;
                    self.actors[ib].actor = name;
                }
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::ReloadMapPalette
            | SceneOp::InitVramAndCram
            | SceneOp::LoadPalette { .. }
            | SceneOp::LoadArt { .. }
            | SceneOp::SetCameraPos { .. }
            | SceneOp::LoadTitleImage { .. }
            | SceneOp::SetTextColour { .. }
            | SceneOp::DrawTextToPlane { .. }
            | SceneOp::IntroTextFadeUp
            | SceneOp::IntroTextFadeDown
            | SceneOp::OverlapCharacters
            | SceneOp::SetStepOffset { .. }
            | SceneOp::PlaySound { .. }
            | SceneOp::SetSavedMusic { .. }
            | SceneOp::FadeIn
            | SceneOp::FadeOut
            | SceneOp::DmaPlanes
            | SceneOp::SetDialogueTree { .. }
            | SceneOp::SetRenderSpritesInCutscene { .. } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            // Bit 0 turns the party follow chain off, bit 1 flips walk
            // ordering to Y-first; the camera-lock bit stays a runtime
            // concern, so the op is consumed here AND forwarded.
            SceneOp::SetFollowMode { bits } => {
                self.follow_chain = bits & 0b1 == 0;
                self.y_first = bits & 0b10 != 0;
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::RecoverStats => {
                for who in state.party_members() {
                    if let Some(stats) = state.roster_mut().get_mut(who) {
                        stats.curr_hp = stats.max_hp;
                        stats.curr_tp = stats.max_tp;
                        stats.status = 0;
                    }
                    effects.push(SceneEffect::RosterChanged { who });
                }
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::MarkGameCleared => {
                effects.push(SceneEffect::GameCleared);
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
            SceneOp::RemovePartyMember { who } => {
                let slots = state.party();
                let Some(remove_at) = slots.iter().position(|member| *member == Some(who)) else {
                    return Some(SceneFault::BadWrite);
                };
                for slot in remove_at..slots.len().saturating_sub(1) {
                    if state.set_party_slot(slot, slots[slot + 1]).is_err() {
                        return Some(SceneFault::BadWrite);
                    }
                }
                if state.set_party_slot(slots.len() - 1, None).is_err() {
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
            SceneOp::SavePartySlots => {
                effects.push(SceneEffect::PartySlotsSaved {
                    slots: state.party(),
                });
                self.pc += 1;
            }
            SceneOp::RestorePartySlots => {
                effects.push(SceneEffect::PartySlotsRestored);
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
            SceneOp::WaitForStart => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
                self.blocked = Blocked::EndingContinue;
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
            SceneOp::BranchIfVehicle {
                if_mounted,
                if_on_foot,
            } => {
                let target = if state.vehicle_index() == 0 {
                    if_on_foot
                } else {
                    if_mounted
                };
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
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
                // Object-level, not party-level: no GameState write here. The
                // 32-word `trap #1` copy carries the object's position too.
                let source = self
                    .actor(ActorRef::PartyMember(from))
                    .map(|a| (a.cell, a.facing));
                if let Some((cell, facing)) = source {
                    match self.actor_mut(ActorRef::PartyMember(to)) {
                        Some(dest) => dest.park(cell, facing),
                        None => self.actors.push(ScriptedActor::new(
                            ActorRef::PartyMember(to),
                            cell,
                            facing,
                        )),
                    }
                }
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
            SceneOp::RemoveItem { item } => {
                if let Some(slot) = state
                    .inventory()
                    .slots()
                    .iter()
                    .position(|&held| held == item)
                {
                    let _ = state.inventory_mut().remove(slot);
                }
                effects.push(SceneEffect::InventoryChanged);
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::PanelCreate { .. }
            | SceneOp::PanelDestroy { .. }
            | SceneOp::PanelDestroyLast
            | SceneOp::PanelDestroyAll
            | SceneOp::ObjectAnimation { .. }
            | SceneOp::Presentation { .. }
            | SceneOp::SetMapLoadFlags { .. } => {
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
