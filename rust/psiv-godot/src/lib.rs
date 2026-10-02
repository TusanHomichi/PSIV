//! The Godot presentation layer: a thin gdext bridge over `psiv-runtime`.
//!
//! This crate owns pixels and input, zero game rules. Floats are legal here —
//! they exist only between the engine's integer state and the screen.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{
    Camera2D, ColorRect, INode2D, Node2D, ProjectSettings, Sprite2D, notify::CanvasItemNotification,
};
use godot::prelude::*;

mod audio;
mod battle;
mod boot;
mod camp;
mod cutscene;
#[path = "debug.rs"]
mod debug;
mod dialogue;
mod field_map;
mod field_status;
mod field_visuals;
mod input;
mod red_palette;
mod runtime_events;
mod save_dir;
mod shop;
#[path = "sound_hooks.rs"]
mod sound_hooks;
mod title;
mod transitions;
mod view;
use battle::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH, BattleScreen};
use boot::{
    FALLBACK_SPAWN_CELL, FALLBACK_SPAWN_MAP, debug_camp_runtime, debug_scene_runtime,
    title_bypassed,
};
use camp::CampMenu;
use cutscene::{CutsceneLayer, PresentationState};
use dialogue::DialogueWindow;
use input::{read_input, requested_save_slot};
use save_dir::{presented_save_slots, save_directory};
use shop::ShopWindow;
use transitions::TransitionKind;
use view::{NpcNode, SheetView};

use psiv_core::{Cell, Direction, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{FrameMode, Pad, Runtime, Session};
use psiv_sound::SAMPLE_RATE;

struct PsivExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PsivExtension {}

pub(crate) const CELL_PIXELS: f32 = 16.0;

/// A session over `runtime`, configured the way the shell runs every session:
/// the title's START and CONTINUE build theirs through here too.
pub(crate) fn new_session(runtime: Runtime) -> Session {
    let mut session = Session::new(runtime);
    session.set_scene_dialogue_autoclose(scene_dialogue_autoclose());
    session
}

/// Oracle tapes hold Speak for four frames for a dismissal edge.  Retail
/// pacing deliberately waits those frames after the typewriter reports a
/// complete page; it does not use the compressed debug autoclose path.
pub(crate) const RETAIL_DISMISS_HOLD_FRAMES: u16 = 4;

pub(crate) fn retail_pace_enabled() -> bool {
    std::env::var("PSIV_DEBUG_AUTOCLOSE_SCENE").is_ok_and(|value| value == "1")
        && std::env::var("PSIV_DEBUG_RETAIL_PACE").is_ok_and(|value| value == "1")
}

/// `PSIV_DEBUG_AUTOCLOSE_SCENE=1` without the retail pace: scene dialogue
/// lines are acknowledged unseen, for deterministic headless scene runs. Every
/// session the shell builds gets it once, at construction ([`new_session`]).
fn scene_dialogue_autoclose() -> bool {
    std::env::var("PSIV_DEBUG_AUTOCLOSE_SCENE").is_ok_and(|value| value == "1")
        && !retail_pace_enabled()
}

/// `EventBattleMusicData` (`ps4.asm:120820`): music id per event-battle
/// index, byte-for-byte. Index `$1A` (Abyss) also clears `Battle_Type`.
const EVENT_BATTLE_MUSIC: [u8; 27] = [
    0x95, 0x95, 0x8f, 0x95, 0xaa, 0x95, 0xaa, 0x8f, 0x95, 0xa0, 0x95, 0x8f, 0x95, 0x8f, 0x95, 0x8f,
    0xaa, 0xa0, 0xa0, 0x95, 0x8f, 0x95, 0x95, 0x95, 0xa4, 0x95, 0xa2,
];

fn event_battle_music(index: u16) -> Option<u8> {
    EVENT_BATTLE_MUSIC.get(usize::from(index)).copied()
}

/// Loads the slot named by `PSIV_LOAD_SLOT` for the field boot path.
///
/// A scripted run without `PSIV_SAVE_DIR` is refused before any filesystem
/// access; the caller starts a new game instead of reading a directory the
/// run did not name.
fn load_requested_slot(data: &GameData, slot: usize) -> Option<Runtime> {
    let directory = match save_directory() {
        Ok(directory) => directory,
        Err(error) => {
            godot_error!("save boot for slot {} refused: {error}", slot + 1);
            return None;
        }
    };
    match Runtime::load_slot(data.clone(), &directory, slot, StepFrames::default()) {
        Ok(runtime) => {
            godot_print!("save boot: loaded slot {}", slot + 1);
            Some(runtime)
        }
        Err(error) => {
            godot_error!(
                "save boot for slot {} failed: {error}; starting new game",
                slot + 1
            );
            None
        }
    }
}

/// The field scene: map picture, the party sprite, NPC sprites.
#[derive(GodotClass)]
#[class(base=Node2D)]
struct Field {
    base: Base<Node2D>,
    /// The game frame: the runtime, the dialogue window, the field's own input
    /// rules and the battle loop. The shell sends it one pad per frame and
    /// presents what comes back (`Session::frame`); the modes still listed
    /// below reach the runtime through `runtime_mut` until their graph nodes
    /// move them in.
    session: Option<Session>,
    pack_dir: String,
    map_sprite: Option<Gd<Sprite2D>>,
    /// Priority tiles — what the VDP draws above sprites (palm crowns,
    /// archways). Sits over the party and NPCs, under the dialogue window.
    overlay_sprite: Option<Gd<Sprite2D>>,
    party: Option<Gd<Sprite2D>>,
    party_view: Option<SheetView>,
    vehicle: Option<Gd<Sprite2D>>,
    /// One entry per visible NPC on the current map.
    npc_nodes: Vec<NpcNode>,
    /// Follower sprites (party members after the leader), created on demand.
    /// (node, active sequence, sequence start tick) per follower.
    follower_nodes: Vec<(Gd<Sprite2D>, String, u64)>,
    /// Scene-owned objects have no map NPC index; their nodes live by scene
    /// slot and are hidden as soon as the presentation state expires them.
    temporary_nodes: BTreeMap<usize, Gd<Sprite2D>>,
    sheet_views: HashMap<String, SheetView>,
    camera: Option<Gd<Camera2D>>,
    dialogue: Option<Gd<DialogueWindow>>,
    status_presentation: field_status::StatusPresentation,
    shop: Option<Gd<ShopWindow>>,
    camp_menu: Option<Gd<CampMenu>>,
    title: Option<title::TitleScreen>,
    battle_screen: Option<Gd<BattleScreen>>,
    battle_files: Option<psiv_data::BattleFiles>,
    battle_field_visibility: Option<battle::FieldVisibility>,
    /// Staged scene planes and the opening cinematic surface.
    cutscene_layer: Option<Gd<CutsceneLayer>>,
    /// Shell-only scene presentation state; scene control remains in the
    /// runtime and arrives here as ordered events.
    presentation: PresentationState,
    anim_tick: u64,
    /// Cinema-mode bars, shown while a scene or battle owns the authentic
    /// 320x224 frame. Four bars are needed in the wide field viewport: the
    /// extra horizontal margins are just as real as the top and bottom ones.
    letterbox: Vec<Gd<godot::classes::ColorRect>>,
    /// Active palette/cover transition, driven by the field visual seam.
    transition: Option<transitions::Transition>,
    /// ColorRect pieces used by the active transition cover.
    transition_nodes: Vec<Gd<ColorRect>>,
    red_palette: red_palette::RedPalette,
    /// True only for a high-bit cutscene whose retail scene has the full
    /// palette treatment; ordinary events retain their dialogue-window motion.
    scene_transition_active: bool,
    /// The character id whose sheet the leader sprite currently uses.
    leader_char: u8,
    /// Auto-paced scene dialogue hold counter.  This is only used when the
    /// debug autoclose harness is explicitly put back on retail cadence.
    retail_dialogue_wait: u16,
    retail_pace_logged: bool,
    /// Last frame's pad, for the `PSIV_DEBUG_INPUT` page trace, which logs the
    /// pages a Speak press dismissed and so has to read the view before the
    /// frame spends the press. The session's own presses are the runtime's.
    dialogue_pad: Pad,
    /// The party's active sequence and the tick it started, so animation
    /// phase restarts at frame 0 on a sequence change — matching
    /// `FieldObj_Move`'s reset-to-frame-0 rather than free-phase modulo.
    party_sequence: String,
    party_seq_start: u64,
    vehicle_sequence: String,
    vehicle_seq_start: u64,
    audio: Option<audio::AudioOutput>,
}

#[godot_api]
impl Field {
    #[func]
    fn debug_play_state(&self) -> GString {
        self.play_probe()
    }

    #[func]
    fn debug_walk_map(&self) -> GString {
        self.walk_probe()
    }
}

#[godot_api]
impl INode2D for Field {
    fn init(base: Base<Node2D>) -> Self {
        Field {
            base,
            session: None,
            pack_dir: String::new(),
            map_sprite: None,
            overlay_sprite: None,
            party: None,
            party_view: None,
            vehicle: None,
            npc_nodes: Vec::new(),
            follower_nodes: Vec::new(),
            temporary_nodes: BTreeMap::new(),
            sheet_views: HashMap::new(),
            camera: None,
            dialogue: None,
            status_presentation: field_status::StatusPresentation::default(),
            shop: None,
            camp_menu: None,
            title: None,
            battle_screen: None,
            battle_files: None,
            battle_field_visibility: None,
            cutscene_layer: None,
            presentation: PresentationState::default(),
            anim_tick: 0,
            retail_dialogue_wait: 0,
            retail_pace_logged: false,
            dialogue_pad: Pad::NEUTRAL,
            letterbox: Vec::new(),
            transition: None,
            transition_nodes: Vec::new(),
            red_palette: red_palette::RedPalette::default(),
            scene_transition_active: false,
            leader_char: 0,
            party_sequence: String::new(),
            party_seq_start: 0,
            vehicle_sequence: String::new(),
            vehicle_seq_start: 0,
            audio: None,
        }
    }

    fn on_notification(&mut self, what: CanvasItemNotification) {
        if what == CanvasItemNotification::EXIT_TREE {
            if let Some(audio) = self.audio.as_mut() {
                audio.shutdown();
            }
            self.audio = None;
        }
    }

    fn ready(&mut self) {
        // Characters sort by their feet line, like the hardware's sprite
        // ordering: standing north of an NPC puts you behind them.
        self.base_mut().set_y_sort_enabled(true);
        // Pack discovery: an exported build ships runtime-pack beside the
        // executable; the dev tree keeps it at the repo root. First hit wins.
        let exe_side = godot::classes::Os::singleton()
            .get_executable_path()
            .to_string();
        let exe_dir = std::path::Path::new(&exe_side)
            .parent()
            .map(|p| p.join("runtime-pack"));
        let dev = ProjectSettings::singleton()
            .globalize_path("res://../runtime-pack")
            .to_string();
        self.pack_dir = exe_dir
            .filter(|p| p.join("manifest.json").is_file())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(dev);
        let data = match GameData::load(std::path::Path::new(&self.pack_dir)) {
            Ok(data) => data,
            Err(e) => {
                godot_error!("runtime pack failed to load from {}: {e}", self.pack_dir);
                return;
            }
        };

        // Spawn where the cartridge's new game actually hands over control:
        // Chaz alone in PiataAcademy_F1, facing down (game_start.json). Alys
        // is the NPC he walks over to find, exactly as retail opens.
        let (spawn_map, spawn_cell, spawn_facing) = match data.manifest().game_start.as_ref() {
            Some(start) => (
                start.map.id,
                Cell::new(start.x_cell as u16, start.y_cell as u16),
                start
                    .facing
                    .name
                    .map(|d| match d {
                        psiv_data::Direction::Up => Direction::Up,
                        psiv_data::Direction::Down => Direction::Down,
                        psiv_data::Direction::Left => Direction::Left,
                        psiv_data::Direction::Right => Direction::Right,
                    })
                    .unwrap_or(Direction::Down),
            ),
            None => (
                FALLBACK_SPAWN_MAP,
                Cell::new(FALLBACK_SPAWN_CELL.0, FALLBACK_SPAWN_CELL.1),
                Direction::Up,
            ),
        };
        let requested_slot = requested_save_slot();
        let debug_event = std::env::var("PSIV_DEBUG_EVENT")
            .ok()
            .and_then(|value| u16::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok());
        let mut runtime =
            match debug_camp_runtime(data.clone(), StepFrames::default()).or_else(|| {
                debug_event.and_then(|event| {
                    debug_scene_runtime(data.clone(), event, StepFrames::default())
                })
            }) {
                Some(Ok(runtime)) => runtime,
                Some(Err(error)) => {
                    godot_error!("debug scene runtime failed: {error}");
                    return;
                }
                None => {
                    // The slot load is the fallback behind both debug
                    // runtimes, so it stays lazy: a debug run must not touch
                    // a save directory it never asked for.
                    let booted_slot =
                        requested_slot.and_then(|slot| load_requested_slot(&data, slot));
                    match booted_slot {
                        Some(runtime) => runtime,
                        None => match Runtime::new(
                            data,
                            spawn_map,
                            spawn_cell,
                            spawn_facing,
                            StepFrames::default(),
                        ) {
                            Ok(rt) => rt,
                            Err(e) => {
                                godot_error!("runtime failed to start: {e}");
                                return;
                            }
                        },
                    }
                }
            };
        let debug_vehicle = std::env::var("PSIV_DEBUG_VEHICLE_INDEX")
            .ok()
            .or_else(|| std::env::var("PSIV_DEBUG_VEHICLE").ok())
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|index| (1..=3).contains(index))
            .or_else(|| std::env::var_os("PSIV_DEBUG_VEHICLE_BATTLE").map(|_| 1));
        if let Some(index) = debug_vehicle {
            if let Err(error) = runtime.set_vehicle_index(index) {
                godot_error!("debug vehicle selector {index} failed: {error}");
                return;
            }
            let name = psiv_core::profile(index).map_or("UNKNOWN", |profile| profile.name);
            godot_print!("debug: {name} mounted (Vehicle_Index={index})");
        }
        self.configure_battles(&mut runtime);

        let mut map_sprite = Sprite2D::new_alloc();
        map_sprite.set_centered(false);
        self.base_mut().add_child(&map_sprite);
        self.map_sprite = Some(map_sprite);

        let mut overlay_sprite = Sprite2D::new_alloc();
        overlay_sprite.set_centered(false);
        overlay_sprite.set_z_index(20);
        self.base_mut().add_child(&overlay_sprite);
        self.overlay_sprite = Some(overlay_sprite);

        let mut party = Sprite2D::new_alloc();
        party.set_centered(false);
        party.set_z_index(5);
        self.base_mut().add_child(&party);
        self.party = Some(party);

        let mut vehicle = Sprite2D::new_alloc();
        vehicle.set_centered(false);
        vehicle.set_z_index(5);
        self.base_mut().add_child(&vehicle);
        self.vehicle = Some(vehicle);

        let mut camera = Camera2D::new_alloc();
        camera.set_zoom(Vector2::new(3.0, 3.0));
        self.base_mut().add_child(&camera);
        camera.make_current();
        self.camera = Some(camera);

        // The title is additive presentation over the existing runtime. All
        // explicit save/debug selectors keep their fast paths and never pay
        // the retail front-door delay.
        if !title_bypassed() {
            let slots = presented_save_slots(runtime.data());
            let pack_dir = self.pack_dir.clone();
            self.title = title::TitleScreen::build(&pack_dir, slots, self.base_mut());
        }

        let mut window = DialogueWindow::new_alloc();
        // The pack loads once, through `psiv-data`, and rides in the data the
        // runtime is built from: the dialogue rules (the trees, the window
        // geometry the open animation counts in) are the runtime's, the art is
        // the node's. Every runtime has the pack, so there is nothing to load
        // or check here.
        let set = runtime.dialogue_pack();
        self.presentation.configure_dialogue_trees(set);
        window.bind_mut().configure(&self.pack_dir, set);
        window.set_z_index(30);
        self.base_mut().add_child(&window);
        self.dialogue = Some(window);

        let mut shop = ShopWindow::new_alloc();
        shop.bind_mut().configure(&self.pack_dir);
        self.base_mut().add_child(&shop);
        self.shop = Some(shop);

        let mut camp = CampMenu::new_alloc();
        match psiv_data::DialogueSet::load(std::path::Path::new(&self.pack_dir)) {
            Ok(set) => camp.bind_mut().configure(&self.pack_dir, set),
            Err(e) => godot_error!("camp menu pack failed to load: {e}"),
        }
        self.base_mut().add_child(&camp);
        self.camp_menu = Some(camp);

        // Scene presentation has its own plane stack. It is intentionally
        // separate from DialogueWindow: Panel_Create writes the VDP planes
        // directly and only exposes a new stack on DmaPlanes.
        let mut cutscene_layer = CutsceneLayer::new_alloc();
        match psiv_data::DialogueSet::load(std::path::Path::new(&self.pack_dir)) {
            Ok(set) => cutscene_layer.bind_mut().configure(&self.pack_dir, &set),
            Err(e) => godot_error!("scene presentation pack failed to load: {e}"),
        }
        self.base_mut().add_child(&cutscene_layer);
        self.cutscene_layer = Some(cutscene_layer);

        let ready_map = runtime.map_id().0;
        let ready_cell = runtime.state().cell();
        let sound_bank = match audio::sound_bank_from_data(runtime.data().sound()) {
            Ok(bank) => bank,
            Err(error) => {
                godot_error!("sound records failed to resolve: {error}");
                psiv_sound::SoundBank::default()
            }
        };
        self.session = Some(new_session(runtime));
        let mut audio = audio::AudioOutput::new(sound_bank);
        let debug_audio = audio.has_debug_override();
        self.base_mut().add_child(audio.node());
        if debug_audio {
            audio.start();
        }
        self.audio = Some(audio);
        if debug_audio {
            godot_print!("debug: audio override started at {SAMPLE_RATE} Hz");
        }
        self.load_map_visuals();
        self.play_map_music();
        self.sync_visuals(false);
        self.start_transition(TransitionKind::GameStart);
        godot_print!(
            "PSIV field ready: map {:#05x}, party at ({}, {})",
            ready_map,
            ready_cell.x,
            ready_cell.y
        );
    }

    fn physics_process(&mut self, _delta: f64) {
        if let Some(audio) = self.audio.as_mut() {
            audio.fill();
        }
        self.anim_tick += 1;
        self.clear_poison_flash();
        self.tick_transition();
        self.tick_red_palette();
        self.tick_cutscene_presentation();
        // The modes that are still the shell's are checked in front of the
        // game frame, in the order the old dispatcher checked them. Each one
        // names the campaign-runner node that moves it into the session; the
        // battle is not among them any more (S3), so `drive_battle_if_active`
        // below is the stage, not the fight.
        if self.drive_title() {
            return; // S5
        }
        self.service_field_notices();
        if self.drive_game_over() {
            return; // S5
        }
        let battle_was_active = self.battle_presentation_active();
        self.debug_hooks_tick();
        self.service_ui_audio();
        if !battle_was_active && self.battle_presentation_active() {
            self.start_transition(TransitionKind::BattleEntry);
        }

        if self.drive_battle_if_active() {
            // The battle stage owns the frame: the session ran the fight
            // inside `drive_game_frame`, and this pass advanced the art clocks
            // and closed the stage if the runtime said so. The debug battle
            // receipt is sampled after it: the 19-tick seed plus the 170
            // updates before tick 200 become the 171 elapsed clock updates the
            // oracle receipt models.
            self.capture_debug_shot();
            // Battle close is the other retail restore edge. Scene battles
            // carry Saved_Sound_Index; ordinary battles fall back to the
            // current map's music request.
            if battle_was_active
                && !self.battle_presentation_active()
                && !self.runtime().is_some_and(|rt| rt.game_over())
                && !self.restore_saved_music()
            {
                self.play_map_music();
            }
            return;
        }

        self.drive_game_frame();
    }
}

impl Field {
    /// The runtime behind the session, for what the shell reads.
    pub(crate) fn runtime(&self) -> Option<&Runtime> {
        self.session.as_ref().map(Session::runtime)
    }

    /// The runtime behind the session, for the modes that are still the
    /// shell's — title and game over (S5). A gameplay
    /// frame goes through `Session::frame` instead, battle included.
    pub(crate) fn runtime_mut(&mut self) -> Option<&mut Runtime> {
        self.session.as_mut().map(Session::runtime_mut)
    }

    /// Blocks the field's confirm press until the pad releases it, for the
    /// shell modes that hand the field back: the press that closed a camp menu
    /// or left the game-over fade must not read as a talk.
    pub(crate) fn block_accept(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.block_accept();
        }
    }

    /// The game frame: everything the shell does not own.
    ///
    /// `Session::frame` owns the whole frame — the dialogue window, a pending
    /// `$F6`, the field or the scene, the shop and inn, the camp, a chest and
    /// the battle a frame starts —
    /// in the cartridge's own order. This method sends it the pad and presents
    /// what comes back.
    fn drive_game_frame(&mut self) {
        let window_open = self
            .session
            .as_ref()
            .is_some_and(|session| session.runtime().dialogue_open());
        let pad = self.frame_pad();
        if !window_open {
            self.retail_dialogue_wait = 0;
        }
        let Some(frame) = self.session.as_mut().map(|session| session.frame(pad)) else {
            return;
        };
        match frame.mode {
            // A battle-mode frame here is the field frame a battle began on;
            // the battle loop's own frames go through `drive_battle_if_active`.
            FrameMode::Field | FrameMode::Battle => self.present_frame(frame),
            FrameMode::Shop | FrameMode::Camp => self.present_menu_frame(frame),
        }
    }

    /// Hands the window this frame's runtime view. The runtime owns the
    /// window's state; the node owns its pixels.
    fn sync_dialogue_view(&mut self) {
        let view = self.runtime().and_then(|rt| rt.dialogue_view());
        if let Some(window) = self.dialogue.as_mut() {
            window.bind_mut().set_view(view);
        }
    }

    /// Cinema mode: letterbox bars over the world, under the dialogue box.
    fn set_letterbox(&mut self, on: bool) {
        if on && self.letterbox.is_empty() {
            for _ in 0..4 {
                let mut bar = godot::classes::ColorRect::new_alloc();
                bar.set_color(Color::from_rgb(0.0, 0.0, 0.0));
                bar.set_z_index(500);
                self.base_mut().add_child(&bar);
                self.letterbox.push(bar);
            }
        }
        for bar in &mut self.letterbox {
            bar.set_visible(on);
        }
    }

    /// Keeps the bars glued to the camera view and masks everything outside
    /// the authentic 320x224 frame. At the existing 3x camera zoom this is a
    /// lossless 960x672 presentation inside the 1280x800 wide viewport.
    fn place_letterbox(&mut self) {
        if self.letterbox.is_empty() || !self.letterbox[0].is_visible() {
            return;
        }
        let Some(camera) = self.camera.as_ref() else {
            return;
        };
        let viewport = self.base().get_viewport_rect().size;
        let zoom = camera.get_zoom().x.max(0.01);
        let view = viewport / zoom;
        let center = camera.get_position();
        let top_left = center - view / 2.0;
        let frame_top_left =
            center - Vector2::new(BATTLE_FRAME_WIDTH / 2.0, BATTLE_FRAME_HEIGHT / 2.0);
        let frame_bottom_right =
            center + Vector2::new(BATTLE_FRAME_WIDTH / 2.0, BATTLE_FRAME_HEIGHT / 2.0);
        // The camera centre is fractional, so a bar that starts exactly at
        // the view edge can rasterize a sub-pixel short and leak a one-pixel
        // strip of world along the screen border. Overdraw every outward
        // edge by a couple of world pixels; the frame-facing edges stay
        // exact.
        const BLEED: f32 = 2.0;
        let sizes = [
            (
                top_left - Vector2::new(BLEED, BLEED),
                Vector2::new(
                    view.x + 2.0 * BLEED,
                    (frame_top_left.y - top_left.y).max(0.0) + BLEED,
                ),
            ),
            (
                Vector2::new(top_left.x - BLEED, frame_bottom_right.y),
                Vector2::new(
                    view.x + 2.0 * BLEED,
                    (top_left.y + view.y - frame_bottom_right.y).max(0.0) + BLEED,
                ),
            ),
            (
                Vector2::new(top_left.x - BLEED, frame_top_left.y),
                Vector2::new(
                    (frame_top_left.x - top_left.x).max(0.0) + BLEED,
                    BATTLE_FRAME_HEIGHT,
                ),
            ),
            (
                Vector2::new(frame_bottom_right.x, frame_top_left.y),
                Vector2::new(
                    (top_left.x + view.x - frame_bottom_right.x).max(0.0) + BLEED,
                    BATTLE_FRAME_HEIGHT,
                ),
            ),
        ];
        for (bar, (pos, size)) in self.letterbox.iter_mut().zip(sizes) {
            bar.set_position(pos);
            bar.set_size(size);
        }
    }

    /// Reloads the leader sprite sheet from the game's party slot 0 and
    /// refreshes the cached view on party changes and native save/title loads.
    fn refresh_party_sheets(&mut self) {
        let leader = self
            .runtime()
            .and_then(|runtime| runtime.game().party_slot(0))
            .map(|c| c.0)
            .unwrap_or(0);
        if leader != self.leader_char || self.party_view.is_none() {
            self.leader_char = leader;
            let view = self
                .runtime()
                .and_then(|runtime| runtime.data().party_sheet(leader as usize))
                .and_then(|sheet| SheetView::build(&self.pack_dir, sheet));
            self.party_view = view;
            if self.party_view.is_some() {
                godot_print!("party leader is now sheet {leader}");
            } else {
                godot_error!("party sheet {leader} failed to load");
            }
        }
    }
}
