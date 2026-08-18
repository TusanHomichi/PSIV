//! The Godot presentation layer: a thin gdext bridge over `psiv-runtime`.
//!
//! This crate owns pixels and input, zero game rules. Floats are legal here —
//! they exist only between the engine's integer state and the screen.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{
    Camera2D, ColorRect, INode2D, Image, ImageTexture, Input, Node2D, ProjectSettings, Sprite2D,
    notify::CanvasItemNotification,
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
mod field_visuals;
mod input;
mod runtime_events;
mod shop;
#[path = "sound_hooks.rs"]
mod sound_hooks;
mod title;
mod transitions;
mod view;
use battle::{BATTLE_FRAME_HEIGHT, BATTLE_FRAME_WIDTH, BattleScreen};
use boot::{
    FALLBACK_SPAWN_CELL, FALLBACK_SPAWN_MAP, available_save_slots, debug_camp_runtime,
    debug_scene_runtime, title_bypassed,
};
use camp::CampMenu;
use cutscene::{CutsceneLayer, PresentationState};
use dialogue::DialogueWindow;
use input::{read_input, requested_save_slot, save_directory};
use shop::ShopWindow;
use transitions::TransitionKind;
use view::{NpcNode, SheetView, camp_receipt_frame, npc_pixel_position};

use psiv_core::{Cell, Direction, StepFrames};
use psiv_data::GameData;
use psiv_runtime::Runtime;
use psiv_sound::SAMPLE_RATE;

struct PsivExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PsivExtension {}

pub(crate) const CELL_PIXELS: f32 = 16.0;

/// Oracle tapes hold Speak for four frames for a dismissal edge.  Retail
/// pacing deliberately waits those frames after the typewriter reports a
/// complete page; it does not use the compressed debug autoclose path.
pub(crate) const RETAIL_DISMISS_HOLD_FRAMES: u16 = 4;

pub(crate) fn retail_pace_enabled() -> bool {
    std::env::var("PSIV_DEBUG_AUTOCLOSE_SCENE").is_ok_and(|value| value == "1")
        && std::env::var("PSIV_DEBUG_RETAIL_PACE").is_ok_and(|value| value == "1")
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

/// The field scene: map picture, the party sprite, NPC sprites.
#[derive(GodotClass)]
#[class(base=Node2D)]
struct Field {
    base: Base<Node2D>,
    runtime: Option<Runtime>,
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
    /// True only for a high-bit cutscene whose retail scene has the full
    /// palette treatment; ordinary events retain their dialogue-window motion.
    scene_transition_active: bool,
    /// The character id whose sheet the leader sprite currently uses.
    leader_char: u8,
    /// Set while a dialogue is open and until accept is released after it
    /// closes — the press that dismisses a window must not immediately
    /// re-open it (the engine's own press latch resets while we starve it
    /// with Neutral, so the still-held key would read as a fresh press).
    accept_blocked: bool,
    /// Auto-paced scene dialogue hold counter.  This is only used when the
    /// debug autoclose harness is explicitly put back on retail cadence.
    retail_dialogue_wait: u16,
    retail_pace_logged: bool,
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
impl INode2D for Field {
    fn init(base: Base<Node2D>) -> Self {
        Field {
            base,
            runtime: None,
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
            shop: None,
            camp_menu: None,
            title: None,
            battle_screen: None,
            battle_files: None,
            battle_field_visibility: None,
            cutscene_layer: None,
            presentation: PresentationState::default(),
            anim_tick: 0,
            accept_blocked: false,
            retail_dialogue_wait: 0,
            retail_pace_logged: false,
            letterbox: Vec::new(),
            transition: None,
            transition_nodes: Vec::new(),
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

        // Chaz is party slot 0.
        self.party_view = data
            .party_sheet(0)
            .and_then(|sheet| SheetView::build(&self.pack_dir, sheet));
        if self.party_view.is_none() {
            godot_error!("party sheet 0 (Chaz) failed to load; falling back to nothing visible");
        }

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
                None => match requested_slot {
                    Some(slot) => match Runtime::load_slot(
                        data.clone(),
                        &save_directory(),
                        slot,
                        StepFrames::default(),
                    ) {
                        Ok(rt) => {
                            godot_print!("save boot: loaded slot {}", slot + 1);
                            rt
                        }
                        Err(error) => {
                            godot_error!(
                                "save boot for slot {} failed: {error}; starting new game",
                                slot + 1
                            );
                            match Runtime::new(
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
                            }
                        }
                    },
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
                },
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
            let slots = available_save_slots(runtime.data(), &save_directory());
            let pack_dir = self.pack_dir.clone();
            self.title = title::TitleScreen::build(&pack_dir, slots, self.base_mut());
        }

        let mut window = DialogueWindow::new_alloc();
        match psiv_data::DialogueSet::load(std::path::Path::new(&self.pack_dir)) {
            Ok(set) => window.bind_mut().configure(&self.pack_dir, set),
            Err(e) => godot_error!("dialogue pack failed to load: {e}"),
        }
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
        self.runtime = Some(runtime);
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
        self.tick_transition();
        self.tick_cutscene_presentation();
        if self.drive_title() {
            return;
        }
        let battle_was_active = self.battle_presentation_active();
        self.debug_hooks_tick();
        self.service_ui_audio();
        if !battle_was_active && self.battle_presentation_active() {
            self.start_transition(TransitionKind::BattleEntry);
        }

        if self.drive_battle_if_active() {
            // Battle close is the other retail restore edge. Scene battles
            // carry Saved_Sound_Index; ordinary battles fall back to the
            // current map's music request.
            if battle_was_active
                && !self.battle_presentation_active()
                && !self.restore_saved_music()
            {
                self.play_map_music();
            }
            return;
        }

        // A `$F6` the dialogue fired becomes a running scene.
        let pending = self
            .dialogue
            .as_mut()
            .and_then(|w| w.bind_mut().take_pending_event());
        if let Some(event) = pending {
            let started = self
                .runtime
                .as_mut()
                .is_some_and(|rt| rt.start_event(event));
            if started {
                godot_print!("dialogue event {event:#x} starts its scene");
                self.set_letterbox(true);
                if event & 0x8000 != 0 {
                    self.scene_transition_active = true;
                    self.start_transition(TransitionKind::SceneStart);
                }
            } else {
                godot_error!("dialogue fired event {event:#x} with no transcribed scene");
            }
        }

        // An open dialogue owns the input: accept advances the window, the
        // engine gets Neutral (the cartridge swaps Game_Mode_Routine to
        // FieldRoutine_Interaction; we model it by starving the field of
        // input, per the engine's documented non-modal contract).
        if self.dialogue.as_ref().is_some_and(|w| w.bind().is_open()) {
            self.accept_blocked = true;
            let dismissable = self
                .dialogue
                .as_ref()
                .is_some_and(|window| window.bind().is_dismissable());
            let retail_auto_advance = retail_pace_enabled()
                && dismissable
                && self.retail_dialogue_wait >= RETAIL_DISMISS_HOLD_FRAMES;
            if retail_pace_enabled() && dismissable {
                self.retail_dialogue_wait = self.retail_dialogue_wait.saturating_add(1);
            } else if !dismissable {
                self.retail_dialogue_wait = 0;
            }
            if (Input::singleton().is_action_just_pressed("ui_accept") || retail_auto_advance)
                && let Some(window) = self.dialogue.as_mut()
            {
                window.bind_mut().advance();
                self.retail_dialogue_wait = 0;
                let closed = !window.bind().is_open();
                if closed && let Some(rt) = self.runtime.as_mut() {
                    rt.dialogue_closed();
                }
            }
            let events = self
                .runtime
                .as_mut()
                .map(|rt| {
                    // A window is up: the cartridge suspends field-object
                    // updates (the wanderers freeze mid-town).
                    rt.set_field_suspended(true);
                    rt.tick(psiv_core::Input::Neutral)
                })
                .unwrap_or_default();
            // The scene's post-dialogue ops run on exactly this tick, so
            // their events (party changes, despawns, SceneEnded) must be
            // processed here too — dropping them was a live bug: Alys stayed
            // standing and the leader never swapped.
            self.process_events(events);
            self.service_dialogue_actions();
            self.sync_visuals(false);
            return;
        }
        self.retail_dialogue_wait = 0;

        if self.drive_shop_if_active() {
            return;
        }

        if self.drive_camp_if_active() {
            return;
        }

        // While a scene runs the field gets Neutral: the story owns the
        // party. (Dialogue windows opened by scenes are handled above.)
        let scene_active = self.runtime.as_ref().is_some_and(|rt| rt.scene_active());
        let mut input = if scene_active {
            psiv_core::Input::Neutral
        } else {
            read_input()
        };
        // The press that dismissed a window stays swallowed until released —
        // otherwise the engine (whose press latch reset during the Neutral
        // starvation) reads the still-held key as fresh and reopens the NPC.
        if self.accept_blocked {
            if matches!(input, psiv_core::Input::Action) {
                input = psiv_core::Input::Neutral;
            } else {
                self.accept_blocked = false;
            }
        }
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        runtime.set_field_suspended(false);
        let events = runtime.tick(input);
        let stepped = self.process_events(events);
        self.service_dialogue_actions();
        // A landing tick with the key still held is mid-stride, not rest:
        // without this, the idle frame flashes for one tick every step (the
        // cartridge's animation free-runs and never sees such a gap).
        let walking = {
            let state = self.runtime.as_ref().map(|rt| rt.state());
            state.is_some_and(|s| s.is_stepping()) || (stepped && input.direction().is_some())
        };
        if !self.battle_presentation_active() {
            self.sync_visuals(walking);
        }
    }
}

impl Field {
    /// Loads the current map's PNG and rebuilds NPC sprites.
    fn load_map_visuals(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let id = runtime.map_id().0;
        // Surface every gap the effect layer knows about — silence here
        // would read as "fully patched" when it is not.
        let fx = runtime.map_effects();
        if fx.unresolved_layout_writes > 0 {
            godot_warn!(
                "map {id:#05x}: {} layout write(s) active but unresolved - collision/visual patch pending pack support",
                fx.unresolved_layout_writes
            );
        }
        if fx.undecoded_entries > 0 {
            godot_print!(
                "map {id:#05x}: {} undecoded effect entr(ies) - possibly incompletely patched",
                fx.undecoded_entries
            );
        }
        if fx.variant.is_some() {
            godot_print!("map {id:#05x}: layout variant active");
        }

        // Active layout_writes blit their 32px patch tiles over the baked
        // PNG — the doors MapDataManager opens have to LOOK open, not just
        // walk open. Collected before the image loads to keep borrows flat.
        let blits: Vec<(u32, u32, u32)> = fx.patch_blits.clone();
        let atlas = runtime
            .map_record()
            .and_then(|r| r.patch_tiles.clone())
            .filter(|_| !blits.is_empty());

        match runtime.map_png().map(str::to_owned) {
            Some(name) => {
                let path = format!("{}/{name}", self.pack_dir);
                match Image::load_from_file(&GString::from(path.as_str())) {
                    Some(mut image) => {
                        if let Some(tiles) = &atlas {
                            let atlas_path = format!("{}/{}", self.pack_dir, tiles.png);
                            match Image::load_from_file(&GString::from(atlas_path.as_str())) {
                                Some(atlas_img) => {
                                    let edge = tiles.tile_pixels as i32;
                                    for &(cx, cy, index) in &blits {
                                        let Some(tile) =
                                            tiles.tiles.iter().find(|t| t.index == index)
                                        else {
                                            godot_error!("patch tile {index} missing from atlas");
                                            continue;
                                        };
                                        image.blit_rect(
                                            &atlas_img,
                                            Rect2i::new(
                                                Vector2i::new(tile.x as i32, 0),
                                                Vector2i::new(edge, edge),
                                            ),
                                            Vector2i::new(cx as i32 * edge, cy as i32 * edge),
                                        );
                                    }
                                    godot_print!(
                                        "map {id:#05x}: {} patch tile(s) applied",
                                        blits.len()
                                    );
                                }
                                None => godot_error!("could not load patch atlas {atlas_path}"),
                            }
                        }
                        if let Some(texture) = ImageTexture::create_from_image(&image)
                            && let Some(sprite) = self.map_sprite.as_mut()
                        {
                            sprite.set_texture(&texture);
                            // A scene's InitVramAndCram may have blanked the
                            // field; a map redraw is what restores it.
                            sprite.set_visible(true);
                        }
                    }
                    None => godot_error!("could not load map image {path}"),
                }
            }
            None => godot_error!("map {id:#05x} has no png declared in the pack"),
        }

        // The priority overlay: absent on the 22 maps with no priority tiles.
        let over = runtime.map_png_over().map(str::to_owned);
        if let Some(sprite) = self.overlay_sprite.as_mut() {
            match over {
                Some(name) => {
                    let path = format!("{}/{name}", self.pack_dir);
                    match Image::load_from_file(&GString::from(path.as_str())) {
                        Some(mut image) => {
                            // The overlay atlas mirrors the base one: same
                            // indices, above-sprites pixels only.
                            if let Some(tiles) = &atlas
                                && let Some(over_png) = &tiles.png_over
                            {
                                let atlas_path = format!("{}/{over_png}", self.pack_dir);
                                if let Some(atlas_img) =
                                    Image::load_from_file(&GString::from(atlas_path.as_str()))
                                {
                                    let edge = tiles.tile_pixels as i32;
                                    for &(cx, cy, index) in &blits {
                                        if let Some(tile) =
                                            tiles.tiles.iter().find(|t| t.index == index)
                                        {
                                            image.blit_rect(
                                                &atlas_img,
                                                Rect2i::new(
                                                    Vector2i::new(tile.x as i32, 0),
                                                    Vector2i::new(edge, edge),
                                                ),
                                                Vector2i::new(cx as i32 * edge, cy as i32 * edge),
                                            );
                                        }
                                    }
                                } else {
                                    godot_error!("could not load overlay atlas {atlas_path}");
                                }
                            }
                            match ImageTexture::create_from_image(&image) {
                                Some(texture) => {
                                    sprite.set_texture(&texture);
                                    sprite.set_visible(true);
                                }
                                None => godot_error!("could not texture overlay {path}"),
                            }
                        }
                        None => godot_error!("could not load overlay {path}"),
                    }
                }
                None => sprite.set_visible(false),
            }
        }

        for NpcNode { node, .. } in &mut self.npc_nodes {
            node.queue_free();
        }
        self.npc_nodes.clear();

        // Gather NPC draw info first; borrowing data and adding children at
        // the same time fights the base borrow.
        struct NpcDraw {
            index: usize,
            sheet: String,
            idle: String,
            // Object pixel coordinates, not cells: 85 retail objects sit on
            // half-cells (8px-scaled words), so x/y_pixels are authoritative.
            x: i32,
            y: i32,
        }
        let mut draws: Vec<NpcDraw> = Vec::new();
        if let Some(record) = runtime.map_record() {
            for (index, npc) in record.npcs.iter().enumerate() {
                // Invisible triggers (sprite_reason set) still block in the
                // engine, exactly like the cartridge's invisible objects, but
                // draw nothing. Despawned objects (engine `active` false —
                // the single source of truth) draw nothing either, which is
                // what keeps Alys's old self from resurrecting on rebuild.
                if !runtime.map().npcs().get(index).is_none_or(|n| n.active) {
                    continue;
                }
                let Some(sprite) = &npc.sprite else { continue };
                draws.push(NpcDraw {
                    index,
                    sheet: sprite.sheet.clone(),
                    idle: sprite.idle_sequence.clone(),
                    x: npc.x_pixels as i32,
                    y: npc.y_pixels as i32,
                });
            }
            let missing: Vec<String> = draws
                .iter()
                .filter(|d| !self.sheet_views.contains_key(&d.sheet))
                .map(|d| d.sheet.clone())
                .collect();
            for sheet_id in missing {
                if let Some(sheet) = runtime.data().sheet(&sheet_id) {
                    if let Some(view) = SheetView::build(&self.pack_dir, sheet) {
                        self.sheet_views.insert(sheet_id, view);
                    } else {
                        godot_error!("sheet {sheet_id} png failed to load");
                    }
                }
            }
        }

        for draw in draws {
            let Some(view) = self.sheet_views.get(&draw.sheet) else {
                continue;
            };
            let live = self
                .runtime
                .as_ref()
                .and_then(|rt| rt.map().npcs().get(draw.index))
                .map(|npc| (npc.cell, (i32::from(npc.offset.x), i32::from(npc.offset.y))));
            let (base, spawn) = live.map_or(((draw.x, draw.y), (0, 0)), |(cell, offset)| {
                (
                    npc_pixel_position(cell, offset),
                    (i32::from(cell.x), i32::from(cell.y)),
                )
            });
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(5);
            let frame = camp_receipt_frame(draw.index, &draw.sheet)
                .unwrap_or_else(|| view.frame_at(&draw.idle, 0));
            view.apply(&mut node, frame);
            // "Draw a frame at (object_x - origin_x, object_y - origin_y) and
            // it lands exactly where the VDP would put it."
            node.set_position(Vector2::new(
                (base.0 - view.origin_x) as f32,
                (base.1 - view.origin_y + view.frame_height) as f32,
            ));
            self.base_mut().add_child(&node);
            self.npc_nodes.push(NpcNode {
                node,
                sheet: draw.sheet,
                idle: draw.idle,
                index: draw.index,
                base,
                spawn,
            });
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
        let sizes = [
            (
                top_left,
                Vector2::new(view.x, (frame_top_left.y - top_left.y).max(0.0)),
            ),
            (
                Vector2::new(top_left.x, frame_bottom_right.y),
                Vector2::new(
                    view.x,
                    (top_left.y + view.y - frame_bottom_right.y).max(0.0),
                ),
            ),
            (
                Vector2::new(top_left.x, frame_top_left.y),
                Vector2::new(
                    (frame_top_left.x - top_left.x).max(0.0),
                    BATTLE_FRAME_HEIGHT,
                ),
            ),
            (
                Vector2::new(frame_bottom_right.x, frame_top_left.y),
                Vector2::new(
                    (top_left.x + view.x - frame_bottom_right.x).max(0.0),
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
    /// refreshes follower sheets. Called on PartyChanged.
    fn refresh_party_sheets(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let leader = runtime.game().party_slot(0).map(|c| c.0).unwrap_or(0);
        if leader != self.leader_char {
            self.leader_char = leader;
            let view = runtime
                .data()
                .party_sheet(leader as usize)
                .and_then(|sheet| SheetView::build(&self.pack_dir, sheet));
            if view.is_some() {
                self.party_view = view;
                godot_print!("party leader is now sheet {leader}");
            }
        }
    }
}
