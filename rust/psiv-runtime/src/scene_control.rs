//! Scene lifecycle from the shell's side: installing a transcribed scene with
//! its cast, the input seams the renderer reports, and the ending and
//! cleared-game latch.

use psiv_core::{
    ActorRef, EventIndex, Party, SceneInput, SceneRunner, ScriptedActor, StepFrames, runner_for,
    scene_for,
};

use crate::Runtime;

impl Runtime {
    /// Installs a transcribed scene and parks the field until its runner
    /// produces a completion input. The renderer is notified separately by
    /// the caller because the source of a scene matters to its diagnostics.
    pub(crate) fn install_scene(&mut self, event: EventIndex) -> bool {
        if self.scene.is_some() {
            return false;
        }
        let Some(scene) = scene_for(event) else {
            return false;
        };
        let cast = self.build_cast();
        // The runner walks with the party's own step timing, so scripted-walk
        // interpolation (renderer, camera driver) shares one clock with field
        // walking.
        let Ok(runner) = runner_for(scene, cast, self.party.leader().step_frames()) else {
            return false;
        };
        self.scene = Some(runner);
        self.scene_event = event;
        self.scene_input = SceneInput::None;
        self.scene_choice_pending = false;
        self.scene_camera_locked = false;
        self.scene_warmup = true;
        true
    }

    /// The cast a scene may address: every party member (by slot and by
    /// character through the runner alias) plus every map object by index.
    pub(crate) fn build_cast(&self) -> Vec<ScriptedActor> {
        let mut cast = Vec::new();
        for (slot, member) in self.party.members().iter().enumerate() {
            cast.push(ScriptedActor::new(
                ActorRef::PartyMember(slot),
                member.cell,
                member.facing,
            ));
        }
        for (i, npc) in self.map.npcs().iter().enumerate() {
            cast.push(ScriptedActor::new(ActorRef::Npc(i), npc.cell, npc.facing));
        }
        cast
    }

    /// Whether a scene is running (cinema mode, input ownership).
    #[must_use]
    pub fn scene_active(&self) -> bool {
        self.scene.is_some()
    }

    /// The running scene's actors, for the renderer to draw at their scripted
    /// positions — with live step state, so walks render as walks. Empty when
    /// no scene runs.
    #[must_use]
    pub fn scene_actors(&self) -> &[ScriptedActor] {
        self.scene.as_ref().map(|r| r.actors()).unwrap_or(&[])
    }

    /// The step timing scene walks interpolate with.
    #[must_use]
    pub fn step_frames(&self) -> StepFrames {
        self.party.leader().step_frames()
    }

    /// The live object behind a party slot, also used by Character(id) ops.
    #[must_use]
    pub fn scene_party_actor(&self, slot: usize) -> Option<&ScriptedActor> {
        self.scene.as_ref()?.actor(ActorRef::PartyMember(slot))
    }

    /// Rebuilds the walking party to match the game state's composition,
    /// stacked at the leader's cell exactly as retail stacks on entry.
    pub(crate) fn resize_party(&mut self) {
        let followers = self.game.party_len().saturating_sub(1);
        if followers + 1 == self.party.len() {
            return;
        }
        let cell = self.party.leader().cell();
        let facing = self.party.leader().facing();
        let frames = self.party.leader().step_frames();
        if let Ok(party) = Party::new(&self.map, cell, facing, frames, followers) {
            self.party = party;
        }
    }

    /// Starts an event's scene directly — the `$F6` dialogue path (the
    /// principal's briefing). Returns whether a transcribed scene began.
    pub fn start_event(&mut self, event: u16) -> bool {
        self.install_scene(EventIndex(event))
    }

    /// Original event index while the scene is active; bit 15 distinguishes
    /// panel cutscenes from ordinary field events in the text renderer.
    #[must_use]
    pub fn scene_event(&self) -> Option<EventIndex> {
        self.scene.as_ref().map(|_| self.scene_event)
    }

    /// The active scene's selected text-window routine, including resumes.
    #[must_use]
    pub fn scene_dialogue_window(&self) -> Option<psiv_core::DialogueWindow> {
        self.scene.as_ref().map(SceneRunner::dialogue_window)
    }

    /// The renderer reports the scene-requested dialogue window has closed.
    pub fn dialogue_closed(&mut self) {
        if self.scene.is_some() && !matches!(self.scene_input, SceneInput::Choice(_)) {
            self.scene_input = SceneInput::DialogueClosed;
        }
    }

    /// The renderer reached FF, with no suspended F7 cursor left to resume.
    pub fn dialogue_ended(&mut self) {
        if self.scene.is_some() && !matches!(self.scene_input, SceneInput::Choice(_)) {
            self.scene_input = SceneInput::DialogueEnded;
        }
    }

    /// Releases the retail ending's final Start gate.
    pub fn ending_continue(&mut self) {
        if self.scene.is_some() {
            self.scene_input = SceneInput::EndingContinue;
        }
    }

    /// Returns whether the retail ending has latched the cleared-game state.
    #[must_use]
    pub fn game_cleared(&self) -> bool {
        self.game_cleared
    }
}
