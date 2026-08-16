//! Battle presentation bridge.
//!
//! `Field` owns the runtime and this module owns the Godot battle node. The
//! only crossing is the documented Runtime battle API: setup party, start,
//! resolve a `RoundOrders`, absorb the result, and resume the field.

mod art;
mod chrome;
mod timeline;
mod ui;

pub(crate) use ui::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH, BattleScreen};

use godot::prelude::*;

use psiv_core::Flag;
use psiv_core::battle::{BattleEvent, Outcome};
use psiv_data::BattleFiles;
use psiv_runtime::Runtime;

use super::Field;

/// Data the renderer needs to compose one encounter. These are copied out of
/// pack/runtime records so the Godot node never borrows either lower layer.
pub(crate) struct BattleSetup {
    pub(crate) map_id: u16,
    /// `None` for random encounters; `Some` selects the event-battle
    /// background table for a scene-owned boss formation.
    pub(crate) event_battle: Option<u16>,
    /// Runtime does not yet expose the raw Motavia terrain byte. Keeping this
    /// `None` makes the missing boundary explicit; a map-0 encounter reports
    /// and uses the art-0 fallback instead of silently inventing terrain.
    pub(crate) motavia_terrain: Option<u8>,
    pub(crate) dark_force_2: bool,
    pub(crate) party: Vec<PartyPlacement>,
    pub(crate) enemies: Vec<EnemyPlacement>,
}

pub(crate) struct PartyPlacement {
    pub(crate) fighter_id: u8,
    pub(crate) character: u8,
    pub(crate) name: String,
    pub(crate) hp: u16,
    pub(crate) tp: u16,
}

pub(crate) struct EnemyPlacement {
    pub(crate) fighter_id: u8,
    pub(crate) enemy_id: u16,
    pub(crate) position: u8,
    pub(crate) name: String,
}

pub(super) struct FieldVisibility {
    map: bool,
    overlay: bool,
    party: bool,
    dialogue: bool,
    npcs: Vec<bool>,
    followers: Vec<bool>,
}

impl Field {
    /// Enables the runtime battle seam and creates the presentation node.
    /// Battle art is loaded by the node; battle data is loaded once here and
    /// retained for formation names/positions when an encounter starts.
    pub(crate) fn configure_battles(&mut self, runtime: &mut Runtime) {
        match BattleFiles::load(std::path::Path::new(&self.pack_dir)) {
            Ok(files) => match runtime.enable_battles(&files) {
                Ok(()) => self.battle_files = Some(files),
                Err(error) => godot_error!("battle pack failed to enable: {error}"),
            },
            Err(error) => {
                godot_error!("battle pack failed to load from {}: {error}", self.pack_dir)
            }
        }

        let mut screen = BattleScreen::new_alloc();
        screen.set_z_index(100);
        screen.bind_mut().configure(&self.pack_dir);
        self.base_mut().add_child(&screen);
        self.battle_screen = Some(screen);
    }

    /// Starts a random encounter through the shared battle presentation path.
    pub(crate) fn start_random_battle(&mut self, formation: u16) {
        let Some(files) = self.battle_files.as_ref() else {
            godot_error!(
                "encounter rolled formation {formation:#05x}, but battle files are not enabled"
            );
            return;
        };
        let Some(runtime) = self.runtime.as_ref() else {
            godot_error!("encounter rolled formation {formation:#05x} without a runtime");
            return;
        };
        let Some(setup) = build_setup(files, runtime, formation) else {
            return;
        };
        let party = runtime.battle_party();
        if party.is_empty() {
            godot_error!("encounter rolled formation {formation:#05x} with an empty party");
            return;
        }

        let events = match self
            .runtime
            .as_mut()
            .expect("runtime was checked above")
            .start_battle(formation, party)
        {
            Ok(events) => events,
            Err(error) => {
                godot_error!("could not start battle {formation:#05x}: {error}");
                return;
            }
        };
        self.begin_battle_presentation(setup, events, &format!("formation {formation:#05x}"));
    }

    /// Starts a boss battle emitted by a running scene. Runtime owns the
    /// already-started battle; this method only selects the matching pack art
    /// and hands its initial events to the existing screen.
    pub(crate) fn start_scene_battle(&mut self, index: u16, events: Vec<BattleEvent>) {
        let Some(files) = self.battle_files.as_ref() else {
            godot_error!(
                "scene requested boss event battle {index}, but battle files are not enabled"
            );
            if let Some(runtime) = self.runtime.as_mut() {
                let _ = runtime.finish_battle_for_outcome(Outcome::Escaped, 0);
            }
            return;
        };
        let Some(runtime) = self.runtime.as_ref() else {
            godot_error!("scene boss event battle {index} started without a runtime");
            return;
        };
        let Some(setup) = build_boss_setup(files, runtime, index) else {
            if let Some(runtime) = self.runtime.as_mut() {
                let _ = runtime.finish_battle_for_outcome(Outcome::Escaped, 0);
            }
            return;
        };
        self.begin_battle_presentation(setup, events, &format!("boss event {index}"));
    }

    fn begin_battle_presentation(
        &mut self,
        setup: BattleSetup,
        events: Vec<BattleEvent>,
        label: &str,
    ) {
        let Some(screen) = self.battle_screen.as_mut() else {
            godot_error!("battle started without a BattleScreen node");
            if let Some(runtime) = self.runtime.as_mut() {
                let _ = runtime.finish_battle_for_outcome(Outcome::Escaped, 0);
            }
            return;
        };
        screen.bind_mut().begin(setup, events);
        self.hide_field_for_battle();
        // HOTFIX (live QA: the framed takeover blacked out all battle art
        // while screen-space windows survived): camera repositioning and the
        // cinema letterbox are DISABLED for battles until the framing is
        // rebuilt with live visual verification. The camera stays where the
        // field left it; battle art parents to the BattleScreen at the
        // field camera's current view. Ugly float, but visible - the
        // authentic 320x224 framing returns as its own verified task.
        if let Some(camera) = self.camera.as_mut() {
            camera.set_position(Vector2::new(
                BATTLE_FRAME_WIDTH / 2.0,
                BATTLE_FRAME_HEIGHT / 2.0,
            ));
        }
        godot_print!("battle started: {label}");
    }

    /// Drives one presentation frame while a battle owns the field.
    /// Returns `true` when the normal field tick must be skipped.
    pub(crate) fn drive_battle_if_active(&mut self) -> bool {
        let visible = self
            .battle_screen
            .as_ref()
            .is_some_and(|screen| screen.is_visible());
        if !visible {
            return false;
        }

        if self.runtime.as_ref().is_some_and(Runtime::battle_active)
            && let Some(runtime) = self.runtime.as_mut()
        {
            // Battles still consume the shared vblank/RNG stream. No field
            // input reaches Runtime while GameMode_Battle owns the frame.
            let _ = runtime.tick(psiv_core::Input::Neutral);
        }

        let order = self
            .battle_screen
            .as_mut()
            .and_then(|screen| screen.bind_mut().take_command());
        if let Some(order) = order {
            let result = self
                .runtime
                .as_mut()
                .map(|runtime| runtime.battle_round(&order));
            match result {
                Some(Ok(events)) => {
                    if let Some(screen) = self.battle_screen.as_mut() {
                        screen.bind_mut().enqueue_events(events);
                    }
                }
                Some(Err(error)) => {
                    if let Some(screen) = self.battle_screen.as_mut() {
                        screen.bind_mut().fail_round(&error.to_string());
                    }
                }
                None => godot_error!("battle command selected without a runtime"),
            }
        }

        if let Some(screen) = self.battle_screen.as_mut() {
            screen.bind_mut().advance_frame();
        }
        self.service_battle_finish();
        self.place_letterbox();

        let close = self
            .battle_screen
            .as_mut()
            .is_some_and(|screen| screen.bind_mut().take_close_ready());
        if close {
            self.end_battle_presentation();
        }
        true
    }

    pub(crate) fn battle_presentation_active(&self) -> bool {
        self.battle_screen
            .as_ref()
            .is_some_and(|screen| screen.is_visible())
    }

    fn service_battle_finish(&mut self) {
        let Some(request) = self
            .battle_screen
            .as_mut()
            .and_then(|screen| screen.bind_mut().take_finish_request())
        else {
            return;
        };
        let reward = match request.outcome {
            Outcome::Victory => request.reward_each,
            Outcome::Escaped | Outcome::Defeat => 0,
        };
        let levels = self.runtime.as_mut().map_or_else(Vec::new, |runtime| {
            runtime.finish_battle_for_outcome(request.outcome, reward)
        });
        if let Some(screen) = self.battle_screen.as_mut() {
            // Queue level-up narration before marking idle-close, otherwise
            // an empty level-up vector could close the node one frame early.
            let mut screen = screen.bind_mut();
            screen.enqueue_events(levels);
            screen.set_close_when_idle();
        }
    }

    fn hide_field_for_battle(&mut self) {
        if self.battle_field_visibility.is_some() {
            return;
        }
        let visibility = FieldVisibility {
            map: self
                .map_sprite
                .as_ref()
                .is_some_and(|node| node.is_visible()),
            overlay: self
                .overlay_sprite
                .as_ref()
                .is_some_and(|node| node.is_visible()),
            party: self.party.as_ref().is_some_and(|node| node.is_visible()),
            dialogue: self.dialogue.as_ref().is_some_and(|node| node.is_visible()),
            npcs: self
                .npc_nodes
                .iter()
                .map(|node| node.node.is_visible())
                .collect(),
            followers: self
                .follower_nodes
                .iter()
                .map(|(node, _, _)| node.is_visible())
                .collect(),
        };
        if let Some(node) = self.map_sprite.as_mut() {
            node.set_visible(false);
        }
        if let Some(node) = self.overlay_sprite.as_mut() {
            node.set_visible(false);
        }
        if let Some(node) = self.party.as_mut() {
            node.set_visible(false);
        }
        if let Some(node) = self.dialogue.as_mut() {
            node.set_visible(false);
        }
        for node in &mut self.npc_nodes {
            node.node.set_visible(false);
        }
        for (node, _, _) in &mut self.follower_nodes {
            node.set_visible(false);
        }
        self.battle_field_visibility = Some(visibility);
    }

    fn end_battle_presentation(&mut self) {
        if let Some(screen) = self.battle_screen.as_mut() {
            screen.set_visible(false);
        }
        if let Some(visibility) = self.battle_field_visibility.take() {
            if let Some(node) = self.map_sprite.as_mut() {
                node.set_visible(visibility.map);
            }
            if let Some(node) = self.overlay_sprite.as_mut() {
                node.set_visible(visibility.overlay);
            }
            if let Some(node) = self.party.as_mut() {
                node.set_visible(visibility.party);
            }
            if let Some(node) = self.dialogue.as_mut() {
                node.set_visible(visibility.dialogue);
            }
            for (node, was_visible) in self.npc_nodes.iter_mut().zip(visibility.npcs) {
                node.node.set_visible(was_visible);
            }
            for ((node, _, _), was_visible) in
                self.follower_nodes.iter_mut().zip(visibility.followers)
            {
                node.set_visible(was_visible);
            }
        }
        self.set_letterbox(false);
        self.sync_visuals(false);
        godot_print!("battle presentation ended");
    }
}

fn build_setup(files: &BattleFiles, runtime: &Runtime, formation_id: u16) -> Option<BattleSetup> {
    let Some(formation) = files
        .formations
        .formations
        .iter()
        .find(|formation| formation.id == Some(formation_id))
    else {
        godot_error!("encounter rolled unknown formation {formation_id:#05x}");
        return None;
    };
    build_setup_for_formation(files, runtime, formation, None)
}

fn build_boss_setup(
    files: &BattleFiles,
    runtime: &Runtime,
    event_battle_index: u16,
) -> Option<BattleSetup> {
    let Some(formation) = files
        .formations
        .boss_formations
        .iter()
        .find(|formation| formation.event_battle_index == Some(event_battle_index))
    else {
        godot_error!("scene requested unknown boss event battle {event_battle_index}");
        return None;
    };
    build_setup_for_formation(files, runtime, formation, Some(event_battle_index))
}

fn build_setup_for_formation(
    files: &BattleFiles,
    runtime: &Runtime,
    formation: &psiv_data::Formation,
    event_battle: Option<u16>,
) -> Option<BattleSetup> {
    let party = runtime
        .battle_party()
        .into_iter()
        .enumerate()
        .map(|(index, member)| PartyPlacement {
            fighter_id: index as u8 + 1,
            character: member.character,
            name: member.name,
            hp: member.stats.curr_hp,
            tp: member.stats.curr_tp,
        })
        .collect();
    let enemies = formation
        .enemies
        .iter()
        .map(|enemy| {
            let name = files
                .enemies
                .enemies
                .iter()
                .find(|record| record.id == enemy.enemy_id)
                .map(|record| {
                    record
                        .display_name
                        .clone()
                        .unwrap_or_else(|| record.symbol.clone())
                })
                .unwrap_or_else(|| format!("Enemy {}", enemy.enemy_id));
            EnemyPlacement {
                fighter_id: 5 + enemy.slot,
                enemy_id: enemy.enemy_id,
                position: enemy.position,
                name,
            }
        })
        .collect();
    Some(BattleSetup {
        map_id: runtime.map_id().0,
        event_battle,
        // Runtime currently exposes no raw Motavia terrain byte. Do not
        // derive one from map art or collision; report the boundary instead.
        motavia_terrain: None,
        dark_force_2: runtime.game().is_set(Flag::event(0x9E)),
        party,
        enemies,
    })
}
