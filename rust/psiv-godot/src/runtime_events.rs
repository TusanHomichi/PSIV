//! Runtime-event dispatch for the Godot shell.
//!
//! Kept out of `lib.rs` because this is a real presentation subsystem, not
//! extension registration boilerplate. The event loop is deliberately
//! ordered: `ScenePresentation` is consumed exactly where the runtime put it.

use crate::boot::collect_event_flags;
use crate::dialogue::DialogueAction;
use crate::transitions::TransitionKind;
use crate::view::{NpcNode, sequence_name};
use crate::{Field, RETAIL_DISMISS_HOLD_FRAMES, event_battle_music, retail_pace_enabled};
use godot::prelude::*;
use psiv_core::WarpTrigger;
use psiv_runtime::RuntimeEvent;

impl Field {
    /// Applies a batch of runtime events to the presentation. Returns whether
    /// a step completed (the walk-animation bridge needs it).
    pub(super) fn process_events(&mut self, events: Vec<RuntimeEvent>) -> bool {
        let mut stepped = false;
        for event in events {
            match event {
                RuntimeEvent::StepCompleted { .. } => stepped = true,
                RuntimeEvent::EncounterRolled { formation } => {
                    let music = self
                        .runtime
                        .as_ref()
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
                    self.load_map_visuals();
                    if self.runtime.as_ref().is_some_and(|rt| rt.scene_active()) {
                        godot_print!("scene map reload retains scene music");
                    } else if !self.restore_saved_music() {
                        self.play_map_music();
                    }
                    if matches!(trigger, WarpTrigger::MapChange) {
                        self.start_transition(TransitionKind::Doorway);
                    }
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
                            let runtime = self.runtime.as_ref()?;
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
                            let opened = match (self.shop.as_mut(), self.runtime.as_ref()) {
                                (Some(shop), Some(runtime)) => {
                                    shop.bind_mut().open(counter, runtime)
                                }
                                _ => false,
                            };
                            if opened {
                                self.place_shop_window();
                                if let Some(runtime) = self.runtime.as_mut() {
                                    let facing = runtime.state().facing().opposite();
                                    runtime.face_npc(npc_index, facing);
                                }
                                continue;
                            }
                        }
                        godot_print!("counter reach at {cell:?} has no shop row; dialogue");
                    }
                    let binding = self.runtime.as_ref().and_then(|rt| {
                        let record = rt.map_record()?;
                        let tree = match record.dialogue_tree {
                            0 => return None,
                            tree => tree,
                        };
                        let id = rt.npc_dialogue_id(npc_index)?;
                        Some((tree, id))
                    });
                    match binding {
                        Some((tree, id)) => {
                            let flags = self.runtime.as_ref().map(collect_event_flags);
                            let opened = self.dialogue.as_mut().is_some_and(|w| {
                                let mut w = w.bind_mut();
                                if let Some(flags) = flags {
                                    w.set_event_flags(flags);
                                }
                                w.open_dialogue(tree, id)
                            });
                            if opened {
                                let toward = self
                                    .runtime
                                    .as_ref()
                                    .map(|rt| rt.state().facing().opposite());
                                if let Some(toward) = toward {
                                    let name = sequence_name("idle", toward);
                                    for entry in &mut self.npc_nodes {
                                        if entry.index == npc_index {
                                            entry.idle = name.clone();
                                        }
                                    }
                                    if let Some(rt) = self.runtime.as_mut() {
                                        rt.face_npc(npc_index, toward);
                                    }
                                }
                            }
                        }
                        None => godot_print!(
                            "talk: npc {npc_index} at {cell:?} has no dialogue binding"
                        ),
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
                    if std::env::var("PSIV_DEBUG_AUTOCLOSE_SCENE").is_ok_and(|value| value == "1")
                        && !retail_pace_enabled()
                    {
                        godot_print!("debug: auto-closing scene dialogue entry {entry}");
                        if let Some(rt) = self.runtime.as_mut() {
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
                    let tree = self
                        .runtime
                        .as_ref()
                        .and_then(|rt| rt.map_record())
                        .map(|r| r.dialogue_tree)
                        .unwrap_or(0);
                    let tree = self.presentation.scene_dialogue_tree(tree);
                    let flags = self.runtime.as_ref().map(collect_event_flags);
                    let opened = self.dialogue.as_mut().is_some_and(|w| {
                        let mut w = w.bind_mut();
                        if let Some(flags) = flags {
                            w.set_event_flags(flags);
                        }
                        w.open_scene_dialogue(tree, entry)
                    });
                    if !opened && let Some(rt) = self.runtime.as_mut() {
                        rt.dialogue_closed();
                    }
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
                    for NpcNode { node, index, .. } in &mut self.npc_nodes {
                        if (first..first + count).contains(index) {
                            node.set_visible(false);
                        }
                    }
                }
                RuntimeEvent::InteractNothing { .. } => {
                    if let Some(window) = self.dialogue.as_mut() {
                        window.bind_mut().open_nothing_here(0);
                    }
                }
            }
        }
        stepped
    }

    /// Drain actions only when the typewriter has reached their byte
    /// position. Consecutive actions are intentionally looped here: retail
    /// runs them back-to-back before the next character iteration.
    pub(super) fn service_dialogue_actions(&mut self) {
        loop {
            let action = self
                .dialogue
                .as_mut()
                .and_then(|window| window.bind_mut().take_ready_action());
            let Some(action) = action else {
                break;
            };
            self.dispatch_dialogue_action(action);
            if let Some(window) = self.dialogue.as_mut() {
                window.bind_mut().resume_after_action();
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
                let result = self
                    .runtime
                    .as_mut()
                    .map_or(Ok(()), |runtime| runtime.set_event_flag(flag));
                match result {
                    Ok(()) => {
                        if let Some(window) = self.dialogue.as_mut() {
                            window.bind_mut().set_event_flag(flag);
                        }
                        godot_print!("dialogue action: SetEventFlag({flag:#04x})");
                    }
                    Err(error) => {
                        godot_error!("dialogue action SetEventFlag({flag:#04x}) failed: {error}")
                    }
                }
            }
            DialogueAction::ElsydeonBroken => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().red_flash(12);
                }
                godot_print!("dialogue action: ElsydeonBroken");
            }
        }
    }
}
