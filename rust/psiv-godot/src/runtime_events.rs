//! Runtime-event dispatch for the Godot shell.
//!
//! Kept out of `lib.rs` because this is a real presentation subsystem, not
//! extension registration boilerplate. The event loop is deliberately
//! ordered: `ScenePresentation` is consumed exactly where the runtime put it.

use crate::input::debug_trace;
use crate::transitions::TransitionKind;
use crate::view::{NpcNode, sequence_name};
use crate::{Field, RETAIL_DISMISS_HOLD_FRAMES, event_battle_music, retail_pace_enabled};
use godot::prelude::*;
use psiv_core::WarpTrigger;
use psiv_runtime::{
    DialogueAction, DialogueSignal, Frame, NpcDialogueOpen, Routed, RuntimeEvent, SceneDialogueOpen,
};

impl Field {
    /// Applies a batch of runtime events to the presentation. Returns whether
    /// a step completed (the walk-animation bridge needs it).
    pub(super) fn process_events(&mut self, events: Vec<RuntimeEvent>) -> bool {
        let mut stepped = false;
        for event in events {
            match event {
                RuntimeEvent::FieldPoisonFlash => self.flash_field_poison(),
                RuntimeEvent::StepCompleted { .. } => stepped = true,
                RuntimeEvent::EncounterRolled { formation } => {
                    let music = self
                        .runtime()
                        .filter(|runtime| runtime.vehicle_active())
                        .map_or(0x8f, |_| 0x96);
                    self.play_sound(music);
                    self.start_random_battle(formation);
                    if self.battle_presentation_active() {
                        self.start_transition(TransitionKind::BattleEntry);
                    }
                }
                RuntimeEvent::MapChanged { map, trigger } => {
                    let kind = match trigger {
                        WarpTrigger::MapChange => "doorway",
                        WarpTrigger::NormalGround => "ground",
                    };
                    godot_print!("map change ({kind}) -> {:#05x}", map.0);
                    self.presentation.reload_field_objects();
                    // RefreshMap replaces the VDP planes; old dialogue
                    // panels cannot remain layered over the new field.
                    if let Some(layer) = self.cutscene_layer.as_mut() {
                        layer.bind_mut().panel_destroy_all();
                    }
                    self.load_map_visuals();
                    if self.runtime().is_some_and(|rt| rt.scene_active()) {
                        godot_print!("scene map reload retains scene music");
                    } else if !self.restore_saved_music() {
                        self.play_map_music();
                    }
                    if matches!(trigger, WarpTrigger::MapChange) {
                        self.start_transition(TransitionKind::Doorway);
                    }
                }
                RuntimeEvent::MapRefreshed => {
                    godot_print!("field map refreshed");
                    self.load_map_visuals();
                }
                RuntimeEvent::MapRefreshFailed { error } => {
                    godot_error!("field map refresh failed: {error}");
                }
                RuntimeEvent::UnpackedTarget { map } => {
                    godot_error!("transition target {:#05x} is not in the pack", map.0);
                }
                RuntimeEvent::WarpUnmapped { cell } => {
                    godot_error!("type-1 cell with no doorway record at {cell:?}");
                }
                RuntimeEvent::SceneStarted { trigger } => {
                    godot_print!("scene started (trigger {trigger})");
                    self.set_letterbox(true);
                }
                RuntimeEvent::SceneStartedFromInteraction { area, event } => {
                    godot_print!("scene started (interaction area {area}, event {event:#x})");
                    self.set_letterbox(true);
                    if event & 0x8000 != 0 {
                        self.scene_transition_active = true;
                        self.start_transition(TransitionKind::SceneStart);
                    }
                }
                RuntimeEvent::ScenePresentation { op } => self.consume_scene_op(op),
                RuntimeEvent::SceneEnded => {
                    godot_print!("scene ended");
                    self.finish_cutscene_presentation();
                    if self.scene_transition_active {
                        self.scene_transition_active = false;
                        self.start_transition(TransitionKind::SceneEnd);
                    } else {
                        self.set_letterbox(false);
                    }
                    self.load_map_visuals();
                }
                RuntimeEvent::SceneMissing { event } => {
                    godot_error!("trigger fired event {event:#x} with no transcribed scene");
                }
                RuntimeEvent::TriggerUnsupported { trigger } => {
                    godot_print!("trigger {trigger} is an unsupported custom check");
                }
                RuntimeEvent::SceneBattleStarted {
                    index,
                    events,
                    sounds,
                    animations,
                } => {
                    if let Some(id) = event_battle_music(index) {
                        self.play_sound(id);
                    }
                    self.start_scene_battle(index, events, sounds, animations);
                }
                RuntimeEvent::SceneFaulted { fault } => {
                    godot_error!("scene fault: {fault:?}");
                }
                RuntimeEvent::SceneBattleFailed { index, error } => {
                    godot_error!("scene battle {index} could not start: {error}");
                }
                RuntimeEvent::PartyChanged => self.refresh_party_sheets(),
                RuntimeEvent::InventoryChanged => {
                    godot_print!("inventory changed during scene/runtime tick");
                }
                RuntimeEvent::VehicleChanged { index } => {
                    godot_print!("vehicle changed to {index:#06x}");
                    if index == 0 && !self.battle_presentation_active() {
                        self.play_map_music();
                    }
                }
                RuntimeEvent::VehicleDismountBlocked => {
                    godot_print!("vehicle cannot dismount on this terrain");
                }
                RuntimeEvent::RosterChanged { who } => {
                    godot_print!("roster record changed for {who:?}");
                    self.refresh_party_sheets();
                }
                RuntimeEvent::NpcsDespawned { first, count } => {
                    self.presentation.despawn_objects(first, count);
                    for NpcNode { node, index, .. } in &mut self.npc_nodes {
                        if (first..first + count).contains(index) {
                            node.set_visible(false);
                        }
                    }
                }
                // The session opens these windows itself, between the field
                // node and the window node of the frame, and reports them as
                // `Frame::routed`. One reaching the shell is a session defect.
                RuntimeEvent::Interact { .. }
                | RuntimeEvent::InteractNothing { .. }
                | RuntimeEvent::SceneDialogue { .. }
                | RuntimeEvent::SceneDialogueResume
                | RuntimeEvent::SceneChoiceRequested => {
                    godot_error!("window-opening event reached the shell: {event:?}");
                }
            }
        }
        stepped
    }

    /// Presents what the dialogue did this frame.
    ///
    /// The runtime has already applied every game-state side: the `$F2`
    /// event-flag write, the scene's close acknowledgement, the choice. What
    /// is left is the presentation — the panel planes a scene dialogue leaves
    /// behind, the sounds, the palettes — plus the diagnostics.
    pub(super) fn present_dialogue_signals(&mut self, signals: Vec<DialogueSignal>) {
        for signal in signals {
            match signal {
                DialogueSignal::Closed { .. } => {
                    if self.runtime().is_some_and(|rt| rt.scene_active()) {
                        godot_print!("scene dialogue closed (t{})", self.anim_tick);
                    }
                    // F7 and FF both return through loc_69B00, which clears
                    // the panel rendering byte. Keep the cursor separately.
                    self.presentation.set_render_sprites(false);
                    if matches!(
                        self.runtime().and_then(|rt| rt.scene_dialogue_window()),
                        Some(
                            psiv_core::DialogueWindow::Standard
                                | psiv_core::DialogueWindow::Cutscene
                                | psiv_core::DialogueWindow::Cutscene5
                        )
                    ) && let Some(layer) = self.cutscene_layer.as_mut()
                    {
                        layer.bind_mut().panel_destroy_all();
                    }
                }
                DialogueSignal::ChoiceAnswered(yes) => {
                    godot_print!("dialogue choice: {}", if yes { "YES" } else { "NO" });
                }
                DialogueSignal::Action(action) => self.dispatch_dialogue_action(action),
                DialogueSignal::Log(line) => godot_print!("dialogue: {line}"),
                DialogueSignal::Fault(line) => godot_error!("dialogue: {line}"),
            }
        }
    }

    fn dispatch_dialogue_action(&mut self, action: DialogueAction) {
        match action {
            DialogueAction::LoadPanel(id) => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_create(id);
                    layer.bind_mut().dma_planes();
                }
                godot_print!("dialogue action: LoadPanel({id:#05x})");
            }
            DialogueAction::DestroyLastPanel => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_last();
                    layer.bind_mut().dma_planes();
                }
                godot_print!("dialogue action: DestroyLastPanel");
            }
            DialogueAction::DestroyAllPanels => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_all();
                    layer.bind_mut().dma_planes();
                }
                godot_print!("dialogue action: DestroyAllPanels");
            }
            DialogueAction::LoadSound(id) => {
                if id != 0 {
                    self.play_sound(id);
                }
                godot_print!("dialogue action: LoadSound({id:#04x})");
            }
            DialogueAction::LoadSound2(id) => {
                self.play_sound(id);
                godot_print!("dialogue action: LoadSound2({id:#04x})");
            }
            DialogueAction::UpdatePalette => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().refresh_palette();
                }
            }
            DialogueAction::ZioEyesRed => {
                self.play_sound(0xCB);
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().red_flash(6);
                }
                godot_print!("dialogue action: ZioEyesRed");
            }
            DialogueAction::PauseMusic => {
                if let Some(audio) = self.audio.as_mut() {
                    audio.pause_music();
                }
                godot_print!("dialogue action: PauseMusic");
            }
            DialogueAction::ResumeMusic => {
                if let Some(audio) = self.audio.as_mut() {
                    audio.resume_music();
                }
                godot_print!("dialogue action: ResumeMusic");
            }
            DialogueAction::SabotageAlarmRedPalette => {
                self.play_sound(0xDB);
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().red_flash(18);
                }
                godot_print!("dialogue action: SabotageAlarmRedPalette");
            }
            DialogueAction::SetEventFlag(flag) => {
                // The runtime already wrote the live flag bank when it
                // released the action, so this is the log line only.
                godot_print!("dialogue action: SetEventFlag({flag:#04x})");
            }
            DialogueAction::ElsydeonBroken => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().red_flash(12);
                }
                godot_print!("dialogue action: ElsydeonBroken");
            }
        }
    }

    /// Presents the windows the session opened this frame.
    ///
    /// The session already opened them, in the cartridge's order; what is left
    /// is the shell's share: the diagnostics, the retail-pace harness's hold
    /// counter, and the sprite sequence a talk turns an object to.
    pub(super) fn present_routed(&mut self, routed: &[Routed]) {
        for item in routed {
            match *item {
                Routed::Talk {
                    npc_index,
                    cell,
                    outcome,
                } => match outcome {
                    NpcDialogueOpen::Opened => {
                        let toward = self.runtime().map(|rt| rt.state().facing().opposite());
                        if let Some(toward) = toward {
                            let name = sequence_name("idle", toward);
                            for entry in &mut self.npc_nodes {
                                if entry.index == npc_index {
                                    entry.idle = name.clone();
                                }
                            }
                        }
                    }
                    NpcDialogueOpen::NoBinding => {
                        godot_print!("talk: npc {npc_index} at {cell:?} has no dialogue binding");
                    }
                    NpcDialogueOpen::Nothing => {}
                },
                Routed::CounterWithoutShop { cell } => {
                    godot_print!("counter reach at {cell:?} has no shop row; dialogue");
                }
                Routed::ShopOpened { .. } | Routed::NothingHere => {}
                Routed::SceneChoice => godot_print!("scene awaits a dialogue choice"),
                Routed::SceneDialogue { entry, outcome } => {
                    godot_print!("scene dialogue open: entry {entry:#04x}");
                    if retail_pace_enabled() {
                        if !self.retail_pace_logged {
                            godot_print!(
                                "debug: retail-paced scene dialogue enabled (3f/char, dismiss hold {RETAIL_DISMISS_HOLD_FRAMES}f)"
                            );
                            self.retail_pace_logged = true;
                        }
                        self.retail_dialogue_wait = 0;
                    }
                    if outcome == SceneDialogueOpen::UnknownTree {
                        godot_error!("scene dialogue tree is absent from the loaded pack");
                    }
                }
                Routed::SceneDialogueSkipped { entry } => {
                    godot_print!("scene dialogue open: entry {entry:#04x}");
                    godot_print!("debug: auto-closing scene dialogue entry {entry}");
                }
                Routed::SceneDialogueResume { .. } => {
                    godot_print!("scene dialogue resume (t{})", self.anim_tick);
                    self.retail_dialogue_wait = 0;
                }
                Routed::SceneDialogueResumeSkipped => {
                    godot_print!("scene dialogue resume (t{})", self.anim_tick);
                }
            }
        }
    }

    /// Presents one field frame of the session, in the frame's own order: the
    /// window's input-half signals, the `$F6` scene, the windows the session
    /// opened, the events, then the window's own half. The order is the
    /// cartridge's node order — an event can open the window, and the box takes
    /// its first open-animation step on the frame it opens — so this is the one
    /// place the frame's parts are consumed.
    pub(super) fn present_frame(&mut self, frame: Frame) {
        // `PSIV_DEBUG_INPUT=1` keeps its hook: the input the session gave the
        // field this frame. A window's frame has none — the field was starved.
        if let Some(input) = frame.field_input {
            debug_trace(input);
        }
        self.present_dialogue_signals(frame.signals);

        // A `$F6` the dialogue fired becomes a running scene. The high-bit
        // events are panel cutscenes and get a scene transition.
        if let Some(start) = frame.scene_started {
            if start.started {
                godot_print!("dialogue event {:#x} starts its scene", start.event);
                self.set_letterbox(true);
                if start.event & 0x8000 != 0 {
                    self.scene_transition_active = true;
                    self.start_transition(TransitionKind::SceneStart);
                }
            } else {
                godot_error!(
                    "dialogue fired event {:#x} with no transcribed scene",
                    start.event
                );
            }
        }

        self.present_routed(&frame.routed);
        let stepped = self.process_events(frame.events);
        self.present_shop();

        // The window's own half of the frame ran last in the session, after the
        // events that open the window: the box took its first open-animation
        // step on the frame it opened.
        self.present_dialogue_signals(frame.window_signals);
        self.sync_dialogue_view();

        // A landing tick with the key still held is mid-stride, not rest:
        // without this, the idle frame flashes for one tick every step (the
        // cartridge's animation free-runs and never sees such a gap). A frame
        // the window owned has no field input, and drew no walk.
        let walking = match frame.field_input {
            None => false,
            Some(input) => {
                self.runtime().is_some_and(|rt| rt.state().is_stepping())
                    || (stepped && input.direction().is_some())
            }
        };
        if !self.battle_presentation_active() {
            self.sync_visuals(walking);
        }
    }
}
