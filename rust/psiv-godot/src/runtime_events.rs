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
    DialogueAction, DialogueSignal, Frame, NpcDialogueOpen, RuntimeEvent, SceneDialogueOpen,
    Session,
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
                    // The session has already started this battle; the shell's
                    // share is the theme. The stage arrives with the frame's
                    // battle view, and the transition with it.
                    let music = self
                        .runtime()
                        .filter(|runtime| runtime.vehicle_active())
                        .map_or(0x8f, |_| 0x96);
                    self.play_sound(music);
                    godot_print!("encounter rolled: formation {formation:#05x}");
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
                RuntimeEvent::Interact {
                    npc_index,
                    cell,
                    reach,
                } => {
                    if matches!(reach, psiv_core::InteractReach::AcrossCounter) {
                        let counter = self.shop.as_ref().and_then(|shop| {
                            let runtime = self.runtime()?;
                            let object_cell =
                                runtime.map().npcs().get(npc_index).map(|npc| npc.cell);
                            object_cell
                                .into_iter()
                                .chain(std::iter::once(cell))
                                .find_map(|at| {
                                    shop.bind().counter_at(runtime.map_id().0, at.x, at.y)
                                })
                        });
                        if let Some(counter) = counter {
                            let opened = match (self.shop.as_mut(), self.session.as_ref()) {
                                (Some(shop), Some(session)) => {
                                    shop.bind_mut().open(counter, session.runtime())
                                }
                                _ => false,
                            };
                            if opened {
                                self.place_shop_window();
                                if let Some(runtime) = self.runtime_mut() {
                                    let facing = runtime.state().facing().opposite();
                                    runtime.face_npc(npc_index, facing);
                                }
                                continue;
                            }
                        }
                        godot_print!("counter reach at {cell:?} has no shop row; dialogue");
                    }
                    // The runtime resolves the map's tree and the object's
                    // dialogue id, opens the window and turns the object to
                    // face the party (the cartridge's default for a talk).
                    // The shell's share is the sprite sequence and the
                    // diagnostic for an object the map never bound.
                    let opened = self.runtime_mut().map(|rt| rt.open_npc_dialogue(npc_index));
                    match opened {
                        Some(NpcDialogueOpen::Opened) => {
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
                        Some(NpcDialogueOpen::NoBinding) => godot_print!(
                            "talk: npc {npc_index} at {cell:?} has no dialogue binding"
                        ),
                        _ => {}
                    }
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
                RuntimeEvent::SceneDialogue { entry } => {
                    godot_print!("scene dialogue open: entry {entry:#04x}");
                    if std::env::var("PSIV_DEBUG_AUTOCLOSE_SCENE").is_ok_and(|value| value == "1")
                        && !retail_pace_enabled()
                    {
                        godot_print!("debug: auto-closing scene dialogue entry {entry}");
                        if let Some(rt) = self.runtime_mut() {
                            // The runtime opened the window when the scene
                            // asked for it; the harness shuts it again
                            // without presenting anything.
                            rt.close_dialogue();
                            rt.dialogue_closed();
                        }
                        continue;
                    }
                    if retail_pace_enabled() {
                        if !self.retail_pace_logged {
                            godot_print!(
                                "debug: retail-paced scene dialogue enabled (3f/char, dismiss hold {RETAIL_DISMISS_HOLD_FRAMES}f)"
                            );
                            self.retail_pace_logged = true;
                        }
                        self.retail_dialogue_wait = 0;
                    }
                    let panel_layout = self
                        .presentation
                        .panel_dialogue_mode(self.runtime().and_then(|rt| rt.scene_event()));
                    // The runtime resolves the entry against the tree the
                    // scene selected (`SetDialogueTree`, or the map's own
                    // binding); a tree the pack does not have is the one case
                    // that must leave the scene's dialogue barrier pending.
                    let opened = self
                        .runtime_mut()
                        .map(|rt| rt.open_scene_dialogue(entry, panel_layout));
                    match opened {
                        Some(SceneDialogueOpen::UnknownTree) => {
                            godot_error!("scene dialogue tree is absent from the loaded pack");
                        }
                        Some(SceneDialogueOpen::Empty) => {
                            if let Some(rt) = self.runtime_mut() {
                                rt.dialogue_closed();
                            }
                        }
                        _ => {}
                    }
                }
                RuntimeEvent::SceneDialogueResume => {
                    godot_print!("scene dialogue resume (t{})", self.anim_tick);
                    if std::env::var("PSIV_DEBUG_AUTOCLOSE_SCENE").is_ok_and(|value| value == "1")
                        && !retail_pace_enabled()
                    {
                        if let Some(rt) = self.runtime_mut() {
                            rt.close_dialogue();
                            rt.dialogue_closed();
                        }
                        continue;
                    }
                    self.retail_dialogue_wait = 0;
                    let panel_layout = self
                        .presentation
                        .panel_dialogue_mode(self.runtime().and_then(|rt| rt.scene_event()));
                    let opened = self
                        .runtime_mut()
                        .is_some_and(|rt| rt.resume_scene_dialogue(panel_layout));
                    if !opened && let Some(rt) = self.runtime_mut() {
                        rt.dialogue_closed();
                    }
                }
                RuntimeEvent::SceneChoiceRequested => {
                    godot_print!("scene awaits a dialogue choice");
                    if let Some(rt) = self.runtime_mut() {
                        rt.open_scene_choice();
                    }
                }
                RuntimeEvent::SceneBattleStarted { index, .. } => {
                    // The runtime started this battle inside the scene and the
                    // frame's battle view carries the stage; the timeline the
                    // event names is the runtime's own copy.
                    if let Some(id) = event_battle_music(index) {
                        self.play_sound(id);
                    }
                    godot_print!("scene battle {index} started");
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
                RuntimeEvent::InteractNothing { .. } => {
                    if let Some(rt) = self.runtime_mut() {
                        rt.open_nothing_here(0);
                    }
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

    /// Presents one frame of the session, in the frame's own order.
    /// Presents one frame of the session, in the frame's own order: the
    /// window's input-half signals, the `$F6` scene, the events, then the
    /// window's own half. The split is the cartridge's node order — an event
    /// can open the window, and the box takes its first open-animation step on
    /// the frame it opens — so this is the one place the frame's parts are
    /// consumed.
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

        let stepped = self.process_events(frame.events);

        // A battle this frame's events started: the runtime has already begun
        // it, so the shell builds the art, shows the stage and — for an
        // encounter — runs the battle transition its entry always ran.
        if let Some(battle) = frame.battle {
            let encounter = matches!(
                battle.started,
                Some(psiv_runtime::BattleStart::Encounter(_))
            );
            self.present_battle_frame(battle);
            if encounter {
                self.start_transition(TransitionKind::BattleEntry);
            }
        }

        // The window's own half of the frame, after the events above: an
        // `Interact` the shell routed to a talk or a scene's own
        // `SceneDialogue` opens the box in `process_events`, and the box takes
        // its first open-animation step on the frame it opens — the cartridge's
        // window node ran after that decision, not before it.
        let signals = self
            .session
            .as_mut()
            .map(Session::window_tick)
            .unwrap_or_default();
        self.present_dialogue_signals(signals);
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
