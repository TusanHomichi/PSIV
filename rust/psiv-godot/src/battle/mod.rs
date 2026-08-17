//! Battle presentation bridge.
//!
//! `Field` owns the runtime and this module owns the Godot battle node. The
//! only crossing is the documented Runtime battle API: setup party, start,
//! resolve a `RoundOrders`, absorb the result, and resume the field.

mod art;
mod attack;
mod chrome;
mod enemy_overlay;
mod layout;
mod sfx;
mod state;
mod status;
mod timeline;
mod ui;
mod vehicle;
mod vehicle_ui;

pub(crate) use ui::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH, BattleScreen};

use godot::prelude::*;

use psiv_core::Flag;
use psiv_core::battle::Outcome;
use psiv_data::BattleFiles;
use psiv_runtime::{BattleTimeline, Runtime};

use super::Field;

/// Data the renderer needs to compose one encounter. These are copied out of
/// pack/runtime records so the Godot node never borrows either lower layer.
pub(crate) struct BattleSetup {
    pub(crate) map_id: u16,
    /// `None` for random encounters; `Some` selects the event-battle
    /// background table for a scene-owned boss formation.
    pub(crate) event_battle: Option<u16>,
    /// Runtime does not yet expose the raw Motavia terrain byte. Keeping this
    /// `None` for non-Motavia battles; map 0 carries the raw chunk id used by
    /// `MotaBattleBGIndexes`.
    pub(crate) motavia_terrain: Option<u8>,
    /// The mounted selector, when this is the one-fighter vehicle surface.
    pub(crate) vehicle_index: Option<u16>,
    /// Pack-relative vehicle field sheet, used for the battle fighter body.
    pub(crate) vehicle_png: Option<String>,
    /// First-frame crop size from the vehicle field sheet.
    pub(crate) vehicle_frame: Option<(u32, u32)>,
    pub(crate) dark_force_2: bool,
    pub(crate) party: Vec<PartyPlacement>,
    pub(crate) enemies: Vec<EnemyPlacement>,
}

#[derive(Default)]
pub(crate) struct PartyPlacement {
    pub(crate) fighter_id: u8,
    pub(crate) character: u8,
    pub(crate) name: String,
    pub(crate) hp: u16,
    pub(crate) tp: u16,
    pub(crate) skills: [u8; 8],
    pub(crate) skill_uses: [u8; 8],
    pub(crate) max_skill_uses: [u8; 8],
}

pub(crate) struct EnemyPlacement {
    pub(crate) fighter_id: u8,
    pub(crate) enemy_id: u16,
    pub(crate) position: u8,
    pub(crate) name: String,
}

/// `oracle/layouts/battle_command_idle.json`: the two six-cell enemy body
/// runs start at plane columns 11 and 23. `enemy_sprite_origin` consumes the
/// retail position byte as the bottom-right column, so those decoded origins
/// are position bytes 17 and 29.
const ORACLE_ENEMY_POSITIONS: [u8; 2] = [17, 29];

pub(super) struct FieldVisibility {
    map: bool,
    overlay: bool,
    party: bool,
    vehicle: bool,
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

        let timeline = match self
            .runtime
            .as_mut()
            .expect("runtime was checked above")
            .start_battle_timeline(formation, party)
        {
            Ok(timeline) => timeline,
            Err(error) => {
                godot_error!("could not start battle {formation:#05x}: {error}");
                return;
            }
        };
        self.begin_battle_presentation(setup, timeline, &format!("formation {formation:#05x}"));
    }

    /// Presents the command-idle oracle fixture used by the visual loop.
    /// `PSIV_DEBUG_BATTLE=0x88` is intentionally a capture selector, not a
    /// raw formation id: the retail frame is tape 07's post-opening party
    /// (Chaz/Alys/Hahn) against a Zoran Bult and a Twin Arms on the Academy
    /// Basement art, so the debug timeline exercises both a static exact
    /// attack and a visible retail lunge.
    /// No runtime round is started, so this path cannot mutate game state.
    pub(crate) fn start_oracle_debug_battle(&mut self) {
        let setup = BattleSetup {
            map_id: 0x17,
            event_battle: Some(0),
            motavia_terrain: None,
            vehicle_index: None,
            vehicle_png: None,
            vehicle_frame: None,
            dark_force_2: false,
            party: vec![
                PartyPlacement {
                    fighter_id: 1,
                    character: 0,
                    name: "Chaz".into(),
                    hp: 25,
                    tp: 10,
                    ..Default::default()
                },
                PartyPlacement {
                    fighter_id: 2,
                    character: 1,
                    name: "Alys".into(),
                    hp: 53,
                    tp: 40,
                    ..Default::default()
                },
                PartyPlacement {
                    fighter_id: 3,
                    character: 2,
                    name: "Hahn".into(),
                    hp: 21,
                    tp: 25,
                    ..Default::default()
                },
            ],
            enemies: vec![
                EnemyPlacement {
                    fighter_id: 6,
                    enemy_id: 10,
                    position: ORACLE_ENEMY_POSITIONS[0],
                    name: "ZORAN BULT".into(),
                },
                EnemyPlacement {
                    fighter_id: 7,
                    enemy_id: 87,
                    position: ORACLE_ENEMY_POSITIONS[1],
                    name: "TWIN ARMS".into(),
                },
            ],
        };
        self.begin_battle_presentation(
            setup,
            BattleTimeline::debug_audio_probe(),
            "oracle tape-07 command idle with audio probe",
        );
    }

    /// Presents real formation `$0F7` with the newly exact Worker Pod attack
    /// injected as an ordered probe.  The formation and enemy placement come
    /// from the pack; only the command result is deterministic debug tape.
    pub(crate) fn start_newly_exact_debug_battle(&mut self) {
        const FORMATION: u16 = 0x00F7;
        let Some(files) = self.battle_files.as_ref() else {
            godot_error!("newly-exact debug battle needs battle files");
            return;
        };
        let Some(runtime) = self.runtime.as_ref() else {
            godot_error!("newly-exact debug battle needs a runtime");
            return;
        };
        let Some(mut setup) = build_setup(files, runtime, FORMATION) else {
            return;
        };
        // Keep the live proof on the already verified Academy battle
        // background. The enemy identities and positions still come from
        // formation $0F7; this only avoids making the screenshot depend on
        // the current field map's random-battle background binding.
        setup.map_id = 0x17;
        setup.event_battle = Some(0);
        let enemy_specs: Vec<_> = setup
            .enemies
            .iter()
            .map(|enemy| (enemy.fighter_id, enemy.enemy_id))
            .collect();
        let timeline = BattleTimeline::debug_newly_exact_probe(&enemy_specs);
        if timeline.animations.is_empty() {
            godot_error!("formation {FORMATION:#05x} has no newly-exact debug member");
            return;
        }
        self.begin_battle_presentation(
            setup,
            timeline,
            "formation 0x0f7 newly-exact Worker Pod attack probe",
        );
    }

    /// Starts a boss battle emitted by a running scene. Runtime owns the
    /// already-started battle; this method only selects the matching pack art
    /// and hands its initial events to the existing screen.
    pub(crate) fn start_scene_battle(
        &mut self,
        index: u16,
        events: Vec<psiv_core::battle::BattleEvent>,
        sounds: Vec<psiv_runtime::BattleSoundEvent>,
        animations: Vec<psiv_runtime::BattleAnimationEvent>,
    ) {
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
        self.begin_battle_presentation(
            setup,
            BattleTimeline {
                events,
                sounds,
                animations,
            },
            &format!("boss event {index}"),
        );
    }

    fn begin_battle_presentation(
        &mut self,
        setup: BattleSetup,
        timeline: BattleTimeline,
        label: &str,
    ) {
        let Some(screen) = self.battle_screen.as_mut() else {
            godot_error!("battle started without a BattleScreen node");
            if let Some(runtime) = self.runtime.as_mut() {
                let _ = runtime.finish_battle_for_outcome(Outcome::Escaped, 0);
            }
            return;
        };
        screen.bind_mut().begin(setup, timeline);
        self.service_battle_audio();
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
                .map(|runtime| runtime.battle_round_timeline(&order));
            match result {
                Some(Ok(timeline)) => {
                    if let Some(screen) = self.battle_screen.as_mut() {
                        screen.bind_mut().enqueue_timeline(timeline);
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
        self.service_battle_audio();
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

    fn service_battle_audio(&mut self) {
        let sounds = self
            .battle_screen
            .as_mut()
            .map(|screen| screen.bind_mut().take_sound_requests())
            .unwrap_or_default();
        for id in sounds {
            godot_print!("battle SFX dispatch: {id:#04x}");
            self.play_sound(id);
        }
    }

    fn service_battle_finish(&mut self) {
        let Some(request) = self
            .battle_screen
            .as_mut()
            .and_then(|screen| screen.bind_mut().take_finish_request())
        else {
            return;
        };
        if request.outcome == Outcome::Victory {
            self.play_sound(0x8b);
        }
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
            vehicle: self.vehicle.as_ref().is_some_and(|node| node.is_visible()),
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
        if let Some(node) = self.vehicle.as_mut() {
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
            if let Some(node) = self.vehicle.as_mut() {
                node.set_visible(visibility.vehicle);
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
            skills: member.stats.skills,
            skill_uses: member.stats.curr_skill_uses,
            max_skill_uses: member.stats.max_skill_uses,
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
        motavia_terrain: (runtime.map_id().0 == 0 || runtime.vehicle_index().is_some())
            .then(|| runtime.vehicle_battle_terrain())
            .flatten(),
        vehicle_index: runtime.vehicle_index(),
        vehicle_png: runtime
            .vehicle_index()
            .and_then(|index| {
                runtime
                    .data()
                    .vehicle_sheet_for_map(runtime.map_id().0, index)
            })
            .map(|sheet| sheet.png.clone()),
        vehicle_frame: runtime
            .vehicle_index()
            .and_then(|index| {
                runtime
                    .data()
                    .vehicle_sheet_for_map(runtime.map_id().0, index)
            })
            .map(|sheet| (sheet.frame_width, sheet.frame_height)),
        dark_force_2: runtime.game().is_set(Flag::event(0x9E)),
        party,
        enemies,
    })
}
