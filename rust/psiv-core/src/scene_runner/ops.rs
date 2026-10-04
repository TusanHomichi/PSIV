//! The op interpreter: one [`SceneOp`] per call.
//!
//! Split from `scene_runner.rs` (the 1,000-line rule), which keeps the
//! runner's state, blocking and tick loop. Every op's retail meaning is cited
//! on the [`SceneOp`] variant in `scene.rs`.

use super::{Blocked, SceneRunner, pixel_cell};
use crate::scene::{
    ActorRef, Axis, DialogueId, DialogueSource, SceneEffect, SceneFault, SceneOp, ScriptedActor,
};
use crate::state::{GameState, PARTY_SLOTS};

impl SceneRunner {
    /// Runs one op. Returns a fault instead of advancing when it cannot.
    pub(super) fn step_op(
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
            SceneOp::RunDialogue { source, window } => {
                self.dialogue_window = window;
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
                if let SceneOp::RunDialogueResumeWithWindow { window } = op {
                    self.dialogue_window = window;
                }
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
            SceneOp::LoadMap { .. }
            | SceneOp::LoadFlightMap { .. }
            | SceneOp::TakeMapTransition => {
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
            SceneOp::RestoreMapChunks { chunks } => {
                effects.push(SceneEffect::MapChunksRestored { chunks });
                self.pc += 1;
            }
            SceneOp::WriteActorMapChunks { actor, chunks } => {
                let Some(walker) = self.actor(actor) else {
                    return Some(SceneFault::UnknownActor { actor });
                };
                let position = crate::PixelPos::from_cell(walker.cell);
                let mut writes = Vec::with_capacity(chunks.len());
                for &(dx, dy, id) in chunks {
                    let x = position.x + dx;
                    let y = position.y + dy;
                    if x < 0 || y < 0 {
                        return Some(SceneFault::BadWrite);
                    }
                    writes.push(((x / 32) as u32, (y / 32) as u32, id));
                }
                effects.push(SceneEffect::MapChunksWritten { chunks: writes });
                self.pc += 1;
            }
            SceneOp::WriteMapChunks { chunks } => {
                effects.push(SceneEffect::MapChunksWritten {
                    chunks: chunks.to_vec(),
                });
                self.pc += 1;
            }
            SceneOp::Return { value } => {
                effects.push(SceneEffect::Returned { value });
                effects.push(SceneEffect::Finished);
                self.blocked = Blocked::Done;
            }
            SceneOp::BranchDialogueEnd { if_ended } => {
                if if_ended > self.scene.len() {
                    return Some(SceneFault::BadJump { target: if_ended });
                }
                self.pc = if self.dialogue_ended {
                    if_ended
                } else {
                    self.pc + 1
                };
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
            SceneOp::OverlapCharacters => {
                let Some(leader) = self.actor(ActorRef::PartyMember(0)).copied() else {
                    return Some(SceneFault::UnknownActor {
                        actor: ActorRef::PartyMember(0),
                    });
                };
                for slot in 1..PARTY_SLOTS {
                    if let Some(actor) = self.actor_mut(ActorRef::PartyMember(slot)) {
                        *actor = ScriptedActor::new(
                            ActorRef::PartyMember(slot),
                            leader.cell,
                            leader.facing,
                        );
                    }
                }
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::AlignVehicleBoarding { index } => {
                let Some(leader) = self.actor(ActorRef::PartyMember(0)).copied() else {
                    return Some(SceneFault::UnknownActor {
                        actor: ActorRef::PartyMember(0),
                    });
                };
                let snap = crate::vehicle::boarding_snap(leader.cell);
                for slot in 0..PARTY_SLOTS {
                    if let Some(actor) = self.actor_mut(ActorRef::PartyMember(slot)) {
                        actor.park(snap.cell, actor.facing);
                    }
                }
                effects.push(SceneEffect::VehicleBoardingAligned {
                    index,
                    cell: snap.cell,
                    pan_camera: snap.pan_camera,
                });
                self.pc += 1;
                if snap.pan_camera {
                    self.blocked = Blocked::Camera;
                }
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
            | SceneOp::SetStepOffset { .. }
            | SceneOp::PlaySound { .. }
            | SceneOp::PlayMusicIfSavedDifferent { .. }
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
                state.recover_stats();
                for who in state.party_members() {
                    effects.push(SceneEffect::RosterChanged { who });
                }
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::MarkGameCleared => {
                effects.push(SceneEffect::GameCleared);
                self.pc += 1;
            }
            SceneOp::WaitFrames { frames } | SceneOp::StepFieldObject { frames, .. } => {
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
            SceneOp::Presentation {
                op:
                    crate::PresentationOp::FadeToRed { lines: delay }
                    | crate::PresentationOp::FadeFromRed { lines: delay },
            } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
                self.blocked = Blocked::Ticks(8 * (u16::from(delay) + 1));
            }
            // The barrier ramp's own `dbra` waits are inside the loop, so the
            // op blocks for the whole ramp exactly as the cartridge does.
            SceneOp::Presentation {
                op:
                    crate::PresentationOp::PaletteRampFromTable {
                        steps, frame_delay, ..
                    },
            } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
                self.blocked = Blocked::Ticks(u16::from(steps) * frame_delay);
            }
            SceneOp::PanelCreate { .. }
            | SceneOp::PanelDestroy { .. }
            | SceneOp::PanelDestroyLast
            | SceneOp::PanelDestroyAll
            | SceneOp::CreateFieldObject { .. }
            | SceneOp::ObjectAnimation { .. }
            | SceneOp::Presentation { .. }
            | SceneOp::SetMapLoadFlags { .. } => {
                effects.push(SceneEffect::Presentation { op });
                self.pc += 1;
            }
            SceneOp::DestinationMenu { .. } => {
                // The pc stays on this op: the answer decides where the scene
                // resumes (`unblock`), the way `BranchChoice`'s does. The
                // request travels as the op itself; the session opens the menu.
                effects.push(SceneEffect::Presentation { op });
                self.blocked = Blocked::Destination;
            }
            SceneOp::SetWorldIndex { world } => {
                effects.push(SceneEffect::WorldIndexSet { world });
                self.pc += 1;
            }
            SceneOp::SkipUnlessMap { maps, skip } => {
                self.pc += 1;
                if !maps.contains(&self.map_id) {
                    self.pc += usize::from(skip);
                }
            }
            SceneOp::SkipOps { count } => {
                self.pc += 1 + usize::from(count);
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
