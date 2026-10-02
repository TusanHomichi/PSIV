//! Battle presentation bridge.
//!
//! The runtime owns the battle: its menu, its beats, its epilogue and its end
//! (`psiv-runtime/src/session/battle/`). This module builds the art for the
//! battle the session started, hands the node the session's view every frame,
//! and advances the two clocks that are pure presentation — the enemy overlay
//! animation and the decoded attack layers.
//!
//! The only crossing is [`Frame::battle`](psiv_runtime::Frame::battle): a view
//! to draw, the retail sound cues the frame raised, the battle that began and
//! the frame the presentation closes on.

mod art;
mod attack;
mod chrome;
mod enemy_overlay;
mod fixtures;
mod layout;
mod menu_draw;
mod status;
mod ui;
mod vehicle;
mod vehicle_ui;

pub(crate) use ui::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH, BattleScreen};

use godot::prelude::*;

use psiv_core::Flag;
use psiv_data::BattleFiles;
use psiv_runtime::{BattleStart, BattleView, Runtime, RuntimeEvent, Session};

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
    /// Initial elapsed idle-overlay ticks. The tape-07 command-idle receipt at
    /// frame 25000 needs a non-zero fixture offset because its three live tile
    /// pieces have different cadences; normal encounters retain phase zero.
    pub(crate) enemy_animation_phase_ticks: usize,
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
    vehicle: bool,
    dialogue: bool,
    npcs: Vec<bool>,
    followers: Vec<bool>,
}

impl Field {
    /// Enables the runtime battle seam and creates the presentation node.
    /// Battle art is loaded by the node; battle data is loaded once here and
    /// retained for formation names and positions when a battle starts.
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

    /// Presents the battle the session just started.
    ///
    /// The session decided *that* a battle runs and what it rolls; this method
    /// only selects the pack's art for it — the formation table for an
    /// encounter, the boss table for a scene-owned event battle.
    pub(crate) fn present_battle_start(&mut self, start: BattleStart) -> bool {
        let (setup, label) = match start {
            BattleStart::Encounter(formation) => {
                let label = format!("formation {formation:#05x}");
                let (Some(files), Some(runtime)) = (self.battle_files.as_ref(), self.runtime())
                else {
                    godot_error!("encounter {formation:#05x} started with no battle pack");
                    return false;
                };
                (build_setup(files, runtime, formation), label)
            }
            BattleStart::EventBattle(index) => {
                let label = format!("boss event {index}");
                let (Some(files), Some(runtime)) = (self.battle_files.as_ref(), self.runtime())
                else {
                    godot_error!("boss event battle {index} started with no battle pack");
                    return false;
                };
                (build_boss_setup(files, runtime, index), label)
            }
        };
        let Some(setup) = setup else {
            return false;
        };
        self.begin_battle_presentation(setup, &label);
        true
    }

    /// Presents what a battle frame produced: its faults, the retail sound
    /// cues it raised, the battle it began, and the view to draw.
    ///
    /// The transition an encounter's screen entry runs is *not* here: the
    /// field frame that starts one runs it where the shell's own encounter
    /// handler did (`present_frame`), so the debug selectors' tick-30 entry
    /// keeps its single transition from the frame top.
    pub(crate) fn present_battle_frame(&mut self, battle: psiv_runtime::BattleFrame) {
        if let Some(fault) = &battle.fault {
            godot_error!("{fault}");
        }
        if let Some(view) = battle.view.as_ref() {
            for id in &view.sounds {
                godot_print!("battle SFX dispatch: {id:#04x}");
                self.play_sound(*id);
            }
        }
        if let Some(start) = battle.started
            && !self.present_battle_start(start)
        {
            // The shell cannot draw this battle at all, so it must not leave
            // the fight running unseen: the session ends it the way the
            // shell's own error paths did, and the field keeps the frame.
            if let Some(session) = self.session.as_mut() {
                session.abort_battle();
            }
        }
        if let Some(view) = battle.view {
            self.set_battle_view(view);
        }
    }

    /// The debug selectors' entry: starts `formation` as an ordinary encounter
    /// battle through the session (`PSIV_DEBUG_BATTLE=<formation>`,
    /// `PSIV_DEBUG_VEHICLE_BATTLE`). A real battle starts, and the runtime owns
    /// everything about it.
    pub(crate) fn start_random_battle(&mut self, formation: u16) {
        let Some(frame) = self
            .session
            .as_mut()
            .map(|session| session.debug_battle(formation))
        else {
            return;
        };
        self.present_battle_frame(frame);
    }

    fn begin_battle_presentation(&mut self, setup: BattleSetup, label: &str) {
        let Some(screen) = self.battle_screen.as_mut() else {
            godot_error!("battle started without a BattleScreen node");
            return;
        };
        screen.bind_mut().begin(setup);
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

    /// Hands the node this frame's view.
    pub(crate) fn set_battle_view(&mut self, view: BattleView) {
        if let Some(screen) = self.battle_screen.as_mut() {
            screen.bind_mut().set_view(view);
        }
    }

    /// Drives one presentation frame while a battle owns the field.
    /// Returns `true` when the normal field frame must be skipped.
    ///
    /// The battle's own frame — the RNG tick, the menu, the round, the beats
    /// and the epilogue — runs inside [`Session::frame`], which the field's
    /// frame path calls while the session is in its battle mode. What is left
    /// here is the stage: the art clock, the sound cues the frame raised, and
    /// the frame the presentation closes on.
    pub(crate) fn drive_battle_if_active(&mut self) -> bool {
        if !self.battle_presentation_active() {
            return false;
        }
        let mut events = Vec::new();
        if self.session.as_ref().is_some_and(Session::battle_active) {
            events = self.drive_battle_frame();
            // The shell's own fixtures never reach this point: they draw a
            // static view with no session battle behind it.
        }
        if let Some(screen) = self.battle_screen.as_mut() {
            // The art clocks this frame: the idle overlay animation and any
            // live attack layer. Both carry presentation only.
            screen.bind_mut().advance_animation();
        }
        self.place_letterbox();
        if self.battle_close_ready() {
            self.end_battle_presentation(events);
        }
        true
    }

    /// One battle frame from the session, presented in the frame's own order.
    ///
    /// Returns the field events the frame carried — a finished battle returns
    /// its map refresh with the frame that closes it.
    fn drive_battle_frame(&mut self) -> Vec<RuntimeEvent> {
        let pad = self.frame_pad();
        let Some(frame) = self.session.as_mut().map(|session| session.frame(pad)) else {
            return Vec::new();
        };
        if let Some(battle) = frame.battle {
            self.present_battle_frame(battle);
        }
        frame.events
    }

    fn battle_close_ready(&self) -> bool {
        self.battle_screen
            .as_ref()
            .is_some_and(|screen| screen.bind().view().is_some_and(|view| view.close_ready))
    }

    pub(crate) fn battle_presentation_active(&self) -> bool {
        self.battle_screen
            .as_ref()
            .is_some_and(|screen| screen.is_visible())
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

    /// The frame the presentation closes on: the stage goes away and the field
    /// takes the frame back, with the events the session returned
    /// `MapRefreshed` through.
    fn end_battle_presentation(&mut self, events: Vec<RuntimeEvent>) {
        if self.runtime().is_some_and(|rt| rt.game_over()) {
            self.begin_game_over();
            return;
        }
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
        self.process_events(events);
        self.sync_visuals(false);
        godot_print!("battle presentation ended");
    }
}

pub(super) fn build_setup(
    files: &BattleFiles,
    runtime: &Runtime,
    formation_id: u16,
) -> Option<BattleSetup> {
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
        enemy_animation_phase_ticks: 0,
        party,
        enemies,
    })
}
