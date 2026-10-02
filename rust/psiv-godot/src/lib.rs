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
use boot::title_bypassed;
use camp::CampMenu;
use cutscene::{CutsceneLayer, PresentationState};
use dialogue::DialogueWindow;
use input::requested_save_slot;
use save_dir::save_directory;
use shop::ShopWindow;
use transitions::TransitionKind;
use view::{NpcNode, SheetView};

use psiv_core::StepFrames;
use psiv_data::GameData;
use psiv_runtime::{FrameMode, GameOverFrame, Pad, SaveStore, Session, TitleEntry, TitleFrame};
use psiv_sound::SAMPLE_RATE;

struct PsivExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PsivExtension {}

pub(crate) const CELL_PIXELS: f32 = 16.0;

/// The run's save directory as a store, or the logged refusal.
///
/// The policy is the shell's (`save_dir.rs`): a scripted run that named no
/// directory gets no store, and the session refuses every slot operation
/// instead of falling back to a default path. Resolved once per boot, so the
/// fixtures and the constructors share one answer.
pub(crate) fn session_store() -> Option<SaveStore> {
    match save_directory() {
        Ok(directory) => Some(SaveStore::new(directory)),
        Err(error) => {
            godot_error!("save slot scan refused: {error}");
            None
        }
    }
}

/// The shell's configuration of a session it did not build itself — a runtime
/// fixture (`psiv_runtime::camp_fixture`, `scene_fixture`): the run's store and
/// the harness switches.
///
/// A session the shell builds through `psiv_runtime::Session::start` gets the
/// same configuration from the constructor
/// (`with_saves`, `with_title_autostart`, `with_scene_dialogue_autoclose`),
/// because the title reads its switch and its slot rows as it installs.
pub(crate) fn configure_session(mut session: Session, store: Option<SaveStore>) -> Session {
    if let Some(store) = store {
        session.set_save_store(store);
    }
    session.set_scene_dialogue_autoclose(scene_dialogue_autoclose());
    session.set_title_autostart(title_autostart());
    session
}

/// `PSIV_DEBUG_TITLE_AUTOSTART=1`: the title walks its own phases to START, so
/// the new-game handoff is testable without an input device.
fn title_autostart() -> bool {
    std::env::var("PSIV_DEBUG_TITLE_AUTOSTART").is_ok_and(|value| value == "1")
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
/// session the shell builds gets it once, at construction
/// ([`configure_session`], or the constructor's own setter).
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

/// Opens the slot named by `PSIV_LOAD_SLOT` for the field boot path.
///
/// The load is the session's own CONTINUE over the same [`SaveStore`]
/// (`Session::start(...).continue_slot`): a scripted run without
/// `PSIV_SAVE_DIR` is refused before any filesystem access, and the caller
/// starts a new game instead of reading a directory the run did not name.
fn load_requested_slot(data: &GameData, slot: usize, store: Option<SaveStore>) -> Option<Session> {
    let Some(store) = store else {
        godot_error!("save boot for slot {} refused: no save directory", slot + 1);
        return None;
    };
    match Session::start(data.clone())
        .with_saves(store)
        .continue_slot(slot)
    {
        Ok(session) => {
            godot_print!("save boot: loaded slot {}", slot + 1);
            Some(session)
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
    /// rules, the battle loop, the title and a defeat's fade. The shell sends
    /// it one pad per frame and presents what comes back (`Session::frame`);
    /// nothing here can reach the runtime's mutators — the session's read-only
    /// view is the whole surface a shell gets.
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

        // Where the pack's own first control is — Chaz alone in
        // PiataAcademy_F1, facing down (game_start.json), the cell the
        // cartridge hands over on — is the runtime's own business now:
        // `Session::start(...).power_on()`/`field()` derive it, with the
        // runtime's fallback for a pack that predates game-start extraction.
        let requested_slot = requested_save_slot();
        let debug_event = std::env::var("PSIV_DEBUG_EVENT")
            .ok()
            .and_then(|value| u16::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok());
        let debug_camp = std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1");
        // The fixtures are the runtime's (`psiv_runtime::camp_fixture`,
        // `scene_fixture`): they build their own session from the retail
        // initializer, so no shell ever needs a mutable runtime for one. The
        // slot load stays the fallback behind them, and it stays lazy: a debug
        // run must not touch a save directory it never asked for.
        // The run's directory policy, resolved once: every session below gets
        // the store the policy answered with.
        let store = session_store();
        let starts_at_title = !title_bypassed();
        let mut session = if debug_camp {
            match psiv_runtime::camp_fixture(data.clone(), StepFrames::default()) {
                Ok(session) => configure_session(session, store.clone()),
                Err(error) => {
                    godot_error!("debug scene runtime failed: {error}");
                    return;
                }
            }
        } else if let Some(Ok(session)) = debug_event.and_then(|event| {
            psiv_runtime::scene_fixture(data.clone(), event, StepFrames::default())
        }) {
            configure_session(session, store.clone())
        } else if let Some(session) =
            requested_slot.and_then(|slot| load_requested_slot(&data, slot, store.clone()))
        {
            configure_session(session, store.clone())
        } else {
            // The shell's own boot, through the runtime's constructors: retail
            // powers on into the title (`MainGameProgram`, ps4.asm:86190), and
            // a selector that must reach its surface now takes the same boot
            // with no front door. The run's configuration rides on the
            // constructor, because the title reads its slot rows and its
            // autostart switch as it installs.
            let mut start = Session::start(data)
                .with_title_autostart(title_autostart())
                .with_scene_dialogue_autoclose(scene_dialogue_autoclose());
            if let Some(store) = store.clone() {
                start = start.with_saves(store);
            }
            let built = if starts_at_title {
                start.power_on()
            } else {
                start.field()
            };
            match built {
                Ok(session) => session,
                Err(error) => {
                    godot_error!("runtime failed to start: {error}");
                    return;
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
            if let Err(error) = session.debug_mount_vehicle(index) {
                godot_error!("debug vehicle selector {index} failed: {error}");
                return;
            }
            let name = psiv_core::profile(index).map_or("UNKNOWN", |profile| profile.name);
            godot_print!("debug: {name} mounted (Vehicle_Index={index})");
        }
        self.configure_battles(&mut session);

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

        // The title is additive presentation over the existing runtime, and
        // the session owns its flow (`Session::power_on` put it at the front
        // door above); this builds the nodes. All explicit save/debug
        // selectors keep their fast paths and never pay the front-door delay.
        if starts_at_title {
            let pack_dir = self.pack_dir.clone();
            self.title = title::TitleScreen::build(&pack_dir, self.base_mut());
        }

        let mut window = DialogueWindow::new_alloc();
        // The pack loads once, through `psiv-data`, and rides in the data the
        // runtime is built from: the dialogue rules (the trees, the window
        // geometry the open animation counts in) are the runtime's, the art is
        // the node's. Every runtime has the pack, so there is nothing to load
        // or check here.
        let set = session.runtime().dialogue_pack();
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

        let ready_map = session.runtime().map_id().0;
        let ready_cell = session.runtime().state().cell();
        let sound_bank = match audio::sound_bank_from_data(session.runtime().data().sound()) {
            Ok(bank) => bank,
            Err(error) => {
                godot_error!("sound records failed to resolve: {error}");
                psiv_sound::SoundBank::default()
            }
        };
        self.session = Some(session);
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
        // The modes the session owns are presented in the order the shell's own
        // dispatcher ran them: the title first (its driver ran before the field
        // notices, the battle stage and the field), then the battle stage, then
        // the field frame — which may itself be the defeat fade or the frame a
        // defeated game's title comes back on.
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.title().is_some())
        {
            self.drive_title_frame();
            return;
        }
        let battle_was_active = self.battle_presentation_active();
        self.debug_hooks_tick();
        self.service_ui_audio();
        if !battle_was_active && self.battle_presentation_active() {
            self.start_transition(TransitionKind::BattleEntry);
        }

        // The stage is driven unless a defeat's fade owns the frames: the
        // screen stays visible through it, and the session's game-over mode is
        // what says so — not the screen's own visibility. The static debug
        // fixtures (no session battle behind the screen) keep their stage.
        let fade_owns_frame = self.session.as_ref().is_some_and(Session::game_over_active);
        if self.battle_presentation_active() && !fade_owns_frame {
            self.drive_battle_if_active();
            // The battle stage owns the frame: the session ran the fight
            // inside `drive_battle_frame`, and this pass advanced the art
            // clocks and closed the stage if the runtime said so. The debug
            // battle receipt is sampled after it: the 19-tick seed plus the
            // 170 updates before tick 200 become the 171 elapsed clock updates
            // the oracle receipt models.
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
    /// One frame of the front door: the session's title mode, presented.
    ///
    /// The session owns the phases, the rows and the slot rules; this rebuilds
    /// the nodes when the title appears (a power-on or a defeat) and applies
    /// what came back: the view, the erase's log line, the failure's, and the
    /// handoff when START or CONTINUE entered the game.
    fn drive_title_frame(&mut self) {
        let pad = self.frame_pad();
        let Some(frame) = self.session.as_mut().map(|session| session.frame(pad)) else {
            return;
        };
        let Some(title) = frame.title else {
            return;
        };
        self.present_title(title);
    }

    /// Presents one title frame.
    fn present_title(&mut self, frame: TitleFrame) {
        if self.title.is_none() {
            let pack_dir = self.pack_dir.clone();
            self.title = title::TitleScreen::build(&pack_dir, self.base_mut());
        }
        if let Some(view) = frame.view {
            let camera = self.camera.as_ref().map(|node| node.get_position());
            if let Some(screen) = self.title.as_mut() {
                if let Some(center) = camera {
                    screen.reposition(center);
                }
                screen.present(&view);
            }
            let ticks = view.ticks;
            let anim_tick = self.anim_tick;
            title::title_debug_shot(self, anim_tick, ticks);
        }
        if let Some(erased) = frame.erased {
            match erased {
                psiv_runtime::TitleErase::Erased { slot, path } => {
                    godot_print!("title: erased save slot {} ({path})", slot + 1);
                }
                psiv_runtime::TitleErase::Failed { slot, error } => {
                    godot_error!("title: ERASE DATA slot {} failed: {error}", slot + 1);
                }
            }
        }
        if let Some(failure) = frame.failure {
            match failure {
                psiv_runtime::TitleFailure::NewGame(error) => {
                    godot_error!("title: new game failed to build: {error}");
                }
                psiv_runtime::TitleFailure::Continue { slot, error } => {
                    godot_error!(
                        "title: CONTINUE slot {} failed validation: {error}",
                        slot + 1
                    );
                }
            }
        }
        if let Some(entry) = frame.entered {
            self.finish_title_entry(entry);
        }
    }

    /// START or CONTINUE entered the game: the session already built the
    /// runtime and armed the battle pack, so this only takes the front door
    /// down and shows the field, in the order the shell's own title driver had.
    fn finish_title_entry(&mut self, entry: TitleEntry) {
        match entry {
            TitleEntry::Started { event_started } => {
                godot_print!("title: START — new game, firing Event_GameStart");
                self.load_map_visuals();
                self.sync_visuals(false);
                if event_started {
                    self.presentation.reset_scene();
                    self.set_letterbox(true);
                } else {
                    godot_error!("title: Event_GameStart did not start");
                }
            }
            TitleEntry::Continued { slot } => {
                godot_print!("title: CONTINUE loaded slot {}", slot + 1);
                self.load_map_visuals();
                self.play_map_music();
                self.sync_visuals(false);
            }
        }
        self.start_transition(TransitionKind::GameStart);
        // A later game over builds a new title. Release the old nodes instead
        // of accumulating hidden copies.
        if let Some(screen) = self.title.take() {
            screen.dispose();
        }
    }

    /// Presents the defeat fade and, on its last frame, the title it restores.
    fn present_game_over(&mut self, game_over: Option<GameOverFrame>) {
        let Some(fade) = game_over else {
            return;
        };
        if fade.frames == 0 {
            self.start_transition(TransitionKind::SceneFadeOut);
            return;
        }
        if !fade.title_restored {
            return;
        }
        if let Some(screen) = self.battle_screen.as_mut() {
            screen.hide();
        }
        self.battle_field_visibility = None;
        self.presentation.reset_scene();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            layer.bind_mut().end_opening();
            layer.bind_mut().panel_destroy_all();
        }
        self.set_letterbox(false);
        self.play_sound(0xFB);
        godot_print!("game over: title restored; saved slots unchanged");
    }
}

impl Field {
    /// The runtime behind the session, for what the shell reads.
    pub(crate) fn runtime(&self) -> Option<&psiv_runtime::Runtime> {
        self.session.as_ref().map(Session::runtime)
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
        // A field-status window the session opened this frame: its own retail
        // lines, logged where the shell's notice service logged them.
        if let Some(notice) = &frame.notice {
            godot_print!("field status: {}", notice.lines.join(" "));
        }
        match frame.mode {
            // A battle-mode frame here is the field frame a battle began on;
            // the battle loop's own frames go through `drive_battle_if_active`.
            FrameMode::Field | FrameMode::Battle => self.present_frame(frame),
            FrameMode::Shop | FrameMode::Camp => self.present_menu_frame(frame),
            // The defeat fade, and the frame its last count hands the title
            // back on: the picture comes down and the front door's nodes go up
            // in the same frame the shell's own game-over driver did it.
            FrameMode::GameOver => {
                self.present_game_over(frame.game_over);
                if let Some(title) = frame.title {
                    self.present_title(title);
                }
            }
            // Unreachable while the title owns its frames — the dispatcher
            // drives those above — but the frame still carries the view.
            FrameMode::Title => {
                if let Some(title) = frame.title {
                    self.present_title(title);
                }
            }
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
