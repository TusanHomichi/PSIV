//! Godot-side battle composition and event playback.
//!
//! This node is deliberately a dumb presentation surface. It never rolls,
//! subtracts HP, chooses a target, or awards anything. The field hands it the
//! engine's [`BattleEvent`] timeline, and the field hands its one Tier-1 menu
//! choice back to `Runtime::battle_round`.

use std::collections::{BTreeMap, VecDeque};

use godot::classes::{INode2D, Image, ImageTexture, Input, Node2D, Sprite2D};
use godot::prelude::*;

use psiv_core::battle::{BattleEvent, FighterId, Outcome, RoundOrders};
use psiv_data::DialogueSet;
use psiv_runtime::{BattleAnimationEvent, BattleTimeline};

use super::art::BattleArt;
use super::attack::{EnemySprite, enemy_sprite_origin};
use super::chrome::{BattleChrome, WindowRect};
use super::enemy_overlay::EnemyAnimation;
use super::layout::{append_status_quads, tile_dest, transient_column};
use super::sfx::{BattleSoundRequests, QueuedBattleEvent, queue_timeline};
use super::state::{ActiveEvent, DamageDraw, FinishRequest, MessageKind};
use super::status;
use super::timeline::{self, Beat};
use super::vehicle::texture as vehicle_texture;
use super::vehicle_ui::{self, SkillSlot};
use super::{BattleSetup, EnemyPlacement};

pub(super) use super::state::PartyStatus;

/// Retail `$FFFFEE66`: `12 * (Battle_Speed + 1)`, clamped to 0..4.
pub(crate) const fn battle_dwell_frames(speed: u16) -> u16 {
    let speed = if speed > 4 { 4 } else { speed };
    12 * (speed + 1)
}

pub(crate) const BATTLE_DWELL_FRAMES: u16 = battle_dwell_frames(2);

/// Plane cells are 8x8 pixels, per `docs/BATTLE_GEOMETRY.md` §1.
pub(super) const BATTLE_CELL_PIXELS: i32 = 8;

/// Party columns in fighter-id order. The retail layout is center-out, so the
/// visible order is slot 4, 2, 1, 3, 5. These are the §3 table verbatim.
pub(crate) const PARTY_COLUMNS: [i32; 5] = [17, 11, 23, 5, 29];
/// Party art starts at plane row 15 / screen y120, and is 6x6 cells (§3).
const PARTY_ROW_Y: f32 = 120.0;

/// §4 places enemy damage three columns left of the enemy's position byte.
const ENEMY_DAMAGE_COLUMN_OFFSET: i32 = 3;
const ENEMY_DAMAGE_Y: f32 = 40.0;
const PARTY_DAMAGE_Y: f32 = 144.0;
const DAMAGE_WIDTH: f32 = 40.0;
const DAMAGE_HEIGHT: f32 = 16.0;

/// The authentic Genesis battle frame, `BATTLE_GEOMETRY.md` §1.
pub(crate) const BATTLE_FRAME_WIDTH: f32 = 320.0;
pub(crate) const BATTLE_FRAME_HEIGHT: f32 = 224.0;
const BATTLE_BACKGROUND_WIDTH: i32 = 512;
const BATTLE_BACKGROUND_HEIGHT: i32 = 192;

/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=enemy_name_window`, `pixel_rect`.
pub(super) const ENEMY_NAME_RECT: WindowRect = WindowRect {
    x: 16.0,
    y: 8.0,
    w: 96.0,
    h: 24.0,
};
/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.text_runs` run
/// `text=ZORAN BULT`, cell `(3,2)`, pixel `(24,16)`. The same interior origin
/// is used for the runtime enemy name in every command-idle/return capture.
pub(super) const ENEMY_NAME_TEXT_RECT: WindowRect = WindowRect {
    x: 24.0,
    y: 16.0,
    w: 80.0,
    h: 8.0,
};

/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=command_window`, `pixel_rect`.
pub(super) const COMMAND_RECT: WindowRect = WindowRect {
    x: 24.0,
    y: 40.0,
    w: 64.0,
    h: 56.0,
};
/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.text_runs` runs
/// `COMD`, `MACR`, and `RUN`, beginning at cell `(6,6)` / pixel `(48,48)`;
/// the decoded line pitch is 16 pixels.
pub(super) const COMMAND_TEXT_RECT: WindowRect = WindowRect {
    x: 48.0,
    y: 48.0,
    w: 40.0,
    h: 48.0,
};

/// `oracle/layouts/battle_victory.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=victory_window`, `pixel_rect`.
pub(super) const VICTORY_RECT: WindowRect = WindowRect {
    x: 80.0,
    y: 128.0,
    w: 160.0,
    h: 40.0,
};
/// `oracle/layouts/battle_victory.json`: `planes.plane_a.text_runs` run
/// `Victory!`, cell `(11,17)` / pixel `(88,136)`.
pub(super) const VICTORY_TEXT_RECT: WindowRect = WindowRect {
    x: 88.0,
    y: 136.0,
    w: 144.0,
    h: 8.0,
};

/// Non-capture narration uses the decoded transient/wide placements; attack
/// and effect captures have no message rectangle.
const TRANSIENT_MESSAGE_RECT: WindowRect = WindowRect {
    x: 88.0,
    y: 144.0,
    w: 96.0,
    h: 24.0,
};
const LONG_TRANSIENT_MESSAGE_RECT: WindowRect = WindowRect {
    x: 88.0,
    y: 144.0,
    w: 144.0,
    h: 24.0,
};
const WIDE_MESSAGE_RECT: WindowRect = WindowRect {
    x: 80.0,
    y: 128.0,
    w: 160.0,
    h: 40.0,
};
/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=status_strip`, `pixel_rect`.
pub(super) const STATUS_RECT: WindowRect = WindowRect {
    x: 16.0,
    y: 168.0,
    w: 288.0,
    h: 48.0,
};
/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// `status_strip.interior` begins at cell `(3,22)` and the decoded tile runs
/// place pane starts at cells 2, 9, 16, 23, and 30 (56-pixel pitch).
pub(super) const STATUS_PANE_START_CELLS: [i32; 5] = [2, 9, 16, 23, 30];
pub(super) const STATUS_NAME_Y: f32 = 176.0;
pub(super) const STATUS_HP_Y: f32 = 192.0;
pub(super) const STATUS_TP_Y: f32 = 200.0;

/// `oracle/layouts/battle_command_idle.json`: selected/disabled command cursor
/// tile runs at row 6/8/10, local command cell x=1 (screen x=32).
const COMMAND_CURSOR_WORDS: [u16; 3] = [0x6e8, 0x6e7, 0x6e7];
/// `oracle/layouts/battle_command_idle.json`: separator special cells at
/// global columns 9,16,23,30, rows 21..26, with patterns 0x6f4/0x6f5 and the
/// bottom vertical flip. These holes belong to one status window, not five.
pub(super) const STATUS_SEPARATOR_COLUMNS: [i32; 4] = [9, 16, 23, 30];

struct PartySprite {
    fighter: FighterId,
    node: Gd<Sprite2D>,
    idle: Gd<ImageTexture>,
    attack: Option<Gd<ImageTexture>>,
}

/// The battle presentation node owned by [`super::super::Field`].
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct BattleScreen {
    base: Base<Node2D>,
    pack_dir: String,
    art: Option<BattleArt>,
    chrome: Option<BattleChrome>,
    background: Option<Gd<Sprite2D>>,
    enemy_nodes: Vec<EnemySprite>,
    party_nodes: Vec<PartySprite>,
    enemy_textures: BTreeMap<(u16, u8), Gd<ImageTexture>>,
    enemy_positions: BTreeMap<u8, u8>,
    names: BTreeMap<u8, String>,
    character_names: BTreeMap<u8, String>,
    party_status: Vec<PartyStatus>,
    events: VecDeque<QueuedBattleEvent>,
    sound_requests: BattleSoundRequests,
    current: Option<ActiveEvent>,
    message: String,
    message_kind: MessageKind,
    damage: Option<DamageDraw>,
    command_open: bool,
    cursor: usize,
    vehicle_index: Option<u16>,
    vehicle_skills: Vec<SkillSlot>,
    skill_open: bool,
    skill_cursor: usize,
    finish_outcome: Option<Outcome>,
    reward_each: u16,
    reward_meseta: u16,
    transient_column: i32,
    finish_request: Option<FinishRequest>,
    close_when_idle: bool,
    close_ready: bool,
}

#[godot_api]
impl INode2D for BattleScreen {
    fn init(base: Base<Node2D>) -> Self {
        BattleScreen {
            base,
            pack_dir: String::new(),
            art: None,
            chrome: None,
            background: None,
            enemy_nodes: Vec::new(),
            party_nodes: Vec::new(),
            enemy_textures: BTreeMap::new(),
            enemy_positions: BTreeMap::new(),
            names: BTreeMap::new(),
            character_names: BTreeMap::new(),
            party_status: Vec::new(),
            events: VecDeque::new(),
            sound_requests: BattleSoundRequests::default(),
            current: None,
            message: String::new(),
            message_kind: MessageKind::None,
            damage: None,
            command_open: false,
            cursor: 0,
            vehicle_index: None,
            vehicle_skills: Vec::new(),
            skill_open: false,
            skill_cursor: 0,
            finish_outcome: None,
            reward_each: 0,
            reward_meseta: 0,
            transient_column: 11,
            finish_request: None,
            close_when_idle: false,
            close_ready: false,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(100);
        self.base_mut().set_visible(false);
        // The black stage backing must sit BELOW the art children: a
        // CanvasItem's own draw() paints at its z, and the art nodes use
        // negative relative z — a draw_rect here covered the entire stage
        // (found via the self-screenshot loop). A ColorRect child at z -30
        // backs everything instead.
        let mut backing = godot::classes::ColorRect::new_alloc();
        backing.set_position(Vector2::ZERO);
        backing.set_size(Vector2::new(BATTLE_FRAME_WIDTH, BATTLE_FRAME_HEIGHT));
        backing.set_color(Color::BLACK);
        backing.set_z_index(-30);
        self.base_mut().add_child(&backing);
    }

    fn draw(&mut self) {
        let Some(chrome) = self.chrome.as_ref() else {
            return;
        };
        let cell = chrome.cell;
        let palette = chrome.palette;
        let mut quads = Vec::new();
        if self.command_open
            && !self.skill_open
            && let Some(frame) = chrome.frame(ENEMY_NAME_RECT)
        {
            quads.extend(frame);
            let name = self
                .enemy_positions
                .keys()
                .next()
                .and_then(|fighter| self.names.get(fighter))
                .map_or("", String::as_str);
            quads.extend(chrome.text(name, ENEMY_NAME_TEXT_RECT));
        }
        if self.command_open
            && !self.skill_open
            && let Some(frame) = chrome.frame(COMMAND_RECT)
        {
            quads.extend(frame);
            let labels = if self.vehicle_index.is_some() {
                "ATTAC\nOPTIN\nRUN"
            } else {
                "COMD\nMACR\nRUN"
            };
            quads.extend(chrome.text_with_pitch(labels, COMMAND_TEXT_RECT, 16.0));
        }
        if self.command_open && self.skill_open {
            vehicle_ui::draw(chrome, &self.vehicle_skills, self.skill_cursor, &mut quads);
        }
        match self.message_kind {
            MessageKind::Transient | MessageKind::Wide if !self.message.is_empty() => {
                let rect =
                    if self.message_kind == MessageKind::Transient && self.message == "DEFENSE" {
                        WindowRect {
                            x: self.transient_column as f32 * cell,
                            ..TRANSIENT_MESSAGE_RECT
                        }
                    } else if self.message_kind == MessageKind::Transient {
                        LONG_TRANSIENT_MESSAGE_RECT
                    } else {
                        WIDE_MESSAGE_RECT
                    };
                if let Some(frame) = chrome.frame(rect) {
                    quads.extend(frame);
                    quads.extend(chrome.text(&self.message, rect.inset(cell)));
                }
            }
            MessageKind::Victory => {
                quads.extend(chrome.victory_quads(VICTORY_RECT, VICTORY_TEXT_RECT, None));
            }
            MessageKind::VictoryRewards => {
                quads.extend(chrome.victory_quads(
                    VICTORY_RECT,
                    VICTORY_TEXT_RECT,
                    Some((self.reward_each, self.reward_meseta)),
                ));
            }
            MessageKind::None | MessageKind::Transient | MessageKind::Wide => {}
        }
        append_status_quads(chrome, &self.party_status, &mut quads);
        if let Some(damage) = self.damage
            && let Some(column) = self.damage_column(damage.target)
        {
            // §4: enemy damage is row 5, party damage row 18; each block is 5x2.
            let rect = WindowRect {
                x: column as f32 * BATTLE_CELL_PIXELS as f32,
                y: if damage.target.side() == psiv_core::battle::Side::Enemy {
                    ENEMY_DAMAGE_Y
                } else {
                    PARTY_DAMAGE_Y
                },
                w: DAMAGE_WIDTH,
                h: DAMAGE_HEIGHT,
            };
            quads.extend(chrome.damage_quads(damage.amount, rect));
        }
        if self.command_open && !self.skill_open {
            for (row, pattern) in COMMAND_CURSOR_WORDS.into_iter().enumerate() {
                if let Some(quad) =
                    chrome.window_word(pattern, false, false, tile_dest(4, 6 + row as i32 * 2))
                {
                    // `0x6e8` is the selected command; `0x6e7` is the retail
                    // blue disabled/unselected form for the other rows.
                    quads.push(quad);
                }
            }
        }
        for quad in quads {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        let icon_pixels = status::status_pixels(palette, &self.party_status);
        for (position, color) in icon_pixels {
            let points = PackedVector2Array::from(&[
                position,
                Vector2::new(position.x + 1.0, position.y),
                Vector2::new(position.x + 1.0, position.y + 1.0),
                Vector2::new(position.x, position.y + 1.0),
            ]);
            self.base_mut().draw_colored_polygon(&points, color);
        }
    }
}

impl BattleScreen {
    /// Loads the battle art and the dialogue chrome/font conventions.
    pub(crate) fn configure(&mut self, pack_dir: &str) {
        self.pack_dir = pack_dir.to_owned();
        self.art = match BattleArt::load(pack_dir) {
            Ok(art) => Some(art),
            Err(error) => {
                godot_error!("battle art failed to load from {pack_dir}: {error}");
                None
            }
        };
        match DialogueSet::load(std::path::Path::new(pack_dir)) {
            Ok(set) => {
                self.chrome = BattleChrome::build(pack_dir, &set);
                if self.chrome.is_none() {
                    godot_error!("battle: dialogue chrome/font art failed to load");
                }
            }
            Err(error) => godot_error!("battle: dialogue pack failed to load: {error}"),
        }
    }

    /// Rebuilds the field for one random encounter and starts its event tape.
    pub(crate) fn begin(&mut self, setup: BattleSetup, timeline: BattleTimeline) {
        self.clear_sprites();
        self.enemy_positions.clear();
        self.names.clear();
        self.character_names.clear();
        self.party_status.clear();
        self.events.clear();
        self.sound_requests.clear();
        self.current = None;
        self.finish_outcome = None;
        self.finish_request = None;
        self.close_when_idle = false;
        self.close_ready = false;
        self.command_open = false;
        self.cursor = 0;
        self.vehicle_index = setup.vehicle_index;
        self.vehicle_skills = setup
            .party
            .first()
            .map(vehicle_ui::slots_for)
            .unwrap_or_default();
        self.skill_open = false;
        self.skill_cursor = 0;
        self.reward_each = 0;
        self.reward_meseta = 0;
        self.transient_column = 11;
        self.message.clear();
        self.message_kind = MessageKind::None;
        self.damage = None;
        self.build_background(&setup);
        self.build_enemies(&setup.enemies, setup.enemy_animation_phase_ticks);
        self.build_party(&setup);
        self.events.extend(queue_timeline(timeline));
        self.base_mut().set_visible(true);
        self.start_next_event();
        self.base_mut().queue_redraw();
    }

    /// Reads the retail command menu. The field calls Runtime after this
    /// returns; this node never resolves a round itself.
    pub(crate) fn take_command(&mut self) -> Option<RoundOrders> {
        if !self.command_open
            || self.current.is_some()
            || !self.events.is_empty()
            || self.finish_outcome.is_some()
            || self.finish_request.is_some()
        {
            return None;
        }
        let input = Input::singleton();
        if self.skill_open {
            if input.is_action_just_pressed("ui_cancel") {
                self.skill_open = false;
                self.skill_cursor = 0;
                self.base_mut().queue_redraw();
                return None;
            }
            if input.is_action_just_pressed("ui_up") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), -2);
                self.base_mut().queue_redraw();
            }
            if input.is_action_just_pressed("ui_down") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), 2);
                self.base_mut().queue_redraw();
            }
            if input.is_action_just_pressed("ui_left") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), -1);
                self.base_mut().queue_redraw();
            }
            if input.is_action_just_pressed("ui_right") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), 1);
                self.base_mut().queue_redraw();
            }
            if !input.is_action_just_pressed("ui_accept") {
                return None;
            }
            let skill = vehicle_ui::selected(&self.vehicle_skills, self.skill_cursor)?;
            self.vehicle_skills[self.skill_cursor].current -= 1;
            self.skill_open = false;
            self.command_open = false;
            self.base_mut().queue_redraw();
            return Some(RoundOrders::Commands(vec![
                psiv_core::battle::Command::VehicleSkill(skill),
            ]));
        }
        if input.is_action_just_pressed("ui_up") || input.is_action_just_pressed("ui_left") {
            self.cursor = self.cursor.saturating_sub(1);
            self.base_mut().queue_redraw();
        }
        if input.is_action_just_pressed("ui_down") || input.is_action_just_pressed("ui_right") {
            self.cursor = (self.cursor + 1).min(2);
            self.base_mut().queue_redraw();
        }
        if !input.is_action_just_pressed("ui_accept") {
            return None;
        }
        if self.cursor == 1 {
            if self.vehicle_index.is_some() {
                self.skill_open = true;
                self.skill_cursor = 0;
            }
            // MACR is visible for ordinary party parity; macro execution is
            // Tier 3. A vehicle's middle entry opens the retail OPTIN list.
            self.base_mut().queue_redraw();
            return None;
        }
        self.command_open = false;
        self.base_mut().queue_redraw();
        Some(if self.cursor == 0 {
            RoundOrders::attack_all()
        } else {
            RoundOrders::Run
        })
    }

    /// Advances one retail-default dwell frame and starts the next event when
    /// the prior visual beat is complete.
    pub(crate) fn advance_frame(&mut self) {
        for enemy in &mut self.enemy_nodes {
            if let Some(animation) = enemy.animation.as_mut()
                && let Some(texture) = animation.advance()
            {
                enemy.node.set_texture(&texture);
            }
            enemy.advance_attack();
        }
        if let Some(active) = self.current.as_mut() {
            if active.wait_for_confirm {
                if !Input::singleton().is_action_just_pressed("ui_accept") {
                    return;
                }
                active.remaining = 1;
            }
            if active.remaining > 1 {
                active.remaining -= 1;
                return;
            }
            self.current = None;
            self.damage = None;
            self.restore_party_pose();
            self.start_next_event();
            self.base_mut().queue_redraw();
            return;
        }
        self.start_next_event();
    }

    pub(crate) fn enqueue_timeline(&mut self, timeline: BattleTimeline) {
        self.events.extend(queue_timeline(timeline));
        if self.current.is_none() {
            self.start_next_event();
        }
        self.base_mut().queue_redraw();
    }

    pub(crate) fn enqueue_events(&mut self, events: Vec<BattleEvent>) {
        self.enqueue_timeline(BattleTimeline {
            events,
            sounds: Vec::new(),
            animations: Vec::new(),
        });
    }

    pub(crate) fn take_sound_requests(&mut self) -> Vec<u8> {
        self.sound_requests.take()
    }

    pub(crate) fn take_finish_request(&mut self) -> Option<FinishRequest> {
        self.finish_request.take()
    }

    pub(crate) fn set_close_when_idle(&mut self) {
        self.close_when_idle = true;
        if self.current.is_none() && self.events.is_empty() {
            self.start_next_event();
        }
    }

    pub(crate) fn take_close_ready(&mut self) -> bool {
        std::mem::take(&mut self.close_ready)
    }

    pub(crate) fn fail_round(&mut self, error: &str) {
        godot_error!("battle round failed: {error}");
        self.message = "Battle error!".into();
        self.message_kind = MessageKind::Wide;
        self.current = None;
        self.events.clear();
        self.sound_requests.clear();
        self.finish_outcome = None;
        self.finish_request = None;
        self.command_open = true;
        self.base_mut().queue_redraw();
    }

    fn clear_sprites(&mut self) {
        if let Some(mut background) = self.background.take() {
            background.queue_free();
        }
        for mut enemy in self.enemy_nodes.drain(..) {
            enemy.node.queue_free();
        }
        for mut party in self.party_nodes.drain(..) {
            party.node.queue_free();
        }
    }

    fn build_background(&mut self, setup: &BattleSetup) {
        let Some(art) = self.art.as_ref() else {
            return;
        };
        let selected = art
            .background_path(
                setup.event_battle,
                setup.map_id,
                setup.motavia_terrain,
                setup.dark_force_2,
                setup.vehicle_index.is_some(),
            )
            .or_else(|| {
                godot_error!(
                    "battle background selection has no asset for map {:#05x}; using background 0 provisionally",
                    setup.map_id
                );
                art.background_path_by_index(0)
            });
        let mut node = Sprite2D::new_alloc();
        node.set_centered(false);
        node.set_z_index(-20);
        // The §1 background is 512x192 at world origin; the 320x224 camera
        // frame shows its left 320 pixels and leaves the bottom UI strip clear.
        node.set_position(Vector2::ZERO);
        if let Some(path) = selected {
            let full = format!("{}/{}", self.pack_dir, path);
            match Image::load_from_file(&GString::from(full.as_str())) {
                Some(image) => {
                    if image.get_width() != BATTLE_BACKGROUND_WIDTH
                        || image.get_height() != BATTLE_BACKGROUND_HEIGHT
                    {
                        godot_error!(
                            "battle background {full} is {}x{}, expected {}x{}",
                            image.get_width(),
                            image.get_height(),
                            BATTLE_BACKGROUND_WIDTH,
                            BATTLE_BACKGROUND_HEIGHT
                        );
                    }
                    match ImageTexture::create_from_image(&image) {
                        Some(texture) => node.set_texture(&texture),
                        None => godot_error!("battle background texture creation failed: {full}"),
                    }
                }
                None => godot_error!("battle background image failed to load: {full}"),
            }
        }
        self.base_mut().add_child(&node);
        self.background = Some(node);
    }

    fn build_enemies(&mut self, enemies: &[EnemyPlacement], animation_phase_ticks: usize) {
        if self.art.is_none() {
            return;
        }
        for enemy in enemies {
            let Some(fighter) = FighterId::new(enemy.fighter_id) else {
                godot_error!(
                    "battle formation has invalid enemy fighter id {}",
                    enemy.fighter_id
                );
                continue;
            };
            let (width, height) = self
                .art
                .as_ref()
                .and_then(|art| art.enemy_size(enemy.enemy_id))
                .unwrap_or((0, 0));
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(-10);
            let (x, y) = enemy_sprite_origin(enemy.position, width, height);
            node.set_position(Vector2::new(x as f32, y as f32));
            let mut animation = self.art.as_ref().and_then(|art| {
                art.enemy_animation(
                    &self.pack_dir,
                    enemy.enemy_id,
                    enemy.position,
                    animation_phase_ticks,
                )
            });
            let line = super::art::enemy_cram_line(enemy.position);
            let key = (enemy.enemy_id, line);
            let texture = if let Some(current) = animation.as_ref().map(EnemyAnimation::texture) {
                Some(current)
            } else {
                self.enemy_textures.get(&key).cloned().or_else(|| {
                    let texture = self.art.as_ref().and_then(|art| {
                        art.enemy_texture(&self.pack_dir, enemy.enemy_id, enemy.position)
                    });
                    if let Some(texture) = texture.as_ref() {
                        self.enemy_textures.insert(key, texture.clone());
                    } else {
                        godot_error!(
                            "battle enemy {} body failed to load for fighter {}",
                            enemy.enemy_id,
                            enemy.fighter_id
                        );
                    }
                    texture
                })
            };
            if let Some(texture) = texture {
                node.set_texture(&texture);
            }
            self.base_mut().add_child(&node);
            self.enemy_positions
                .insert(enemy.fighter_id, enemy.position);
            self.names.insert(enemy.fighter_id, enemy.name.clone());
            self.enemy_nodes.push(EnemySprite {
                fighter,
                node,
                animation: animation.take(),
                attack: None,
            });
        }
    }

    fn build_party(&mut self, setup: &BattleSetup) {
        let party = &setup.party;
        let vehicle_surface = setup.vehicle_index.is_some();
        self.party_status = party
            .iter()
            .map(|member| PartyStatus {
                fighter: member.fighter_id,
                character: member.character,
                name: member.name.clone(),
                hp: member.hp,
                tp: member.tp,
            })
            .collect();
        self.party_status.sort_by_key(|member| member.character);
        if self.art.is_none() && !vehicle_surface {
            return;
        }
        for member in party {
            let Some(fighter) = FighterId::new(member.fighter_id) else {
                godot_error!("battle party has invalid fighter id {}", member.fighter_id);
                continue;
            };
            let idle = setup
                .vehicle_png
                .as_deref()
                .zip(setup.vehicle_frame)
                .and_then(|(path, (width, height))| {
                    vehicle_texture(&self.pack_dir, path, width, height)
                })
                .or_else(|| {
                    self.art
                        .as_ref()
                        .and_then(|art| art.character_texture(&self.pack_dir, member.character, 0))
                });
            let Some(idle) = idle else {
                godot_error!(
                    "battle character {} idle pose failed to load for fighter {}",
                    member.character,
                    member.fighter_id
                );
                continue;
            };
            let attack = if vehicle_surface {
                None
            } else {
                self.art
                    .as_ref()
                    .and_then(|art| art.character_texture(&self.pack_dir, member.character, 1))
            };
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(-10);
            let column = member
                .fighter_id
                .checked_sub(1)
                .and_then(|index| PARTY_COLUMNS.get(index as usize).copied())
                .unwrap_or(0);
            node.set_position(Vector2::new(
                column as f32 * BATTLE_CELL_PIXELS as f32,
                PARTY_ROW_Y,
            ));
            node.set_texture(&idle);
            self.base_mut().add_child(&node);
            self.names.insert(member.fighter_id, member.name.clone());
            self.character_names
                .insert(member.character, member.name.clone());
            self.party_nodes.push(PartySprite {
                fighter,
                node,
                idle,
                attack,
            });
        }
    }

    fn start_next_event(&mut self) {
        if self.current.is_some() {
            return;
        }
        let Some(queued) = self.events.pop_front() else {
            if let Some(outcome) = self.finish_outcome.take() {
                self.finish_request = Some(FinishRequest {
                    outcome,
                    reward_each: self.reward_each,
                });
                self.command_open = false;
            } else if self.close_when_idle {
                self.close_ready = true;
                self.command_open = false;
            } else {
                self.command_open = true;
                self.message.clear();
                self.message_kind = MessageKind::None;
            }
            return;
        };
        self.sound_requests.extend(queued.sounds);
        self.start_enemy_animations(&queued.animations);
        let event = queued.event;

        self.update_live_party_hp(&event);
        let narration = timeline::narration(&event, &self.names, &self.character_names);
        if narration.line == "Unhandled battle event." {
            godot_error!("battle renderer: unhandled BattleEvent: {event:?}");
        }
        if let BattleEvent::UnsupportedAbility { actor, ability } = &event {
            godot_error!(
                "battle renderer: engine emitted unsupported ability {ability} for fighter {}",
                actor.get()
            );
        }
        if let BattleEvent::VehicleSkillEffectUnavailable { actor, skill } = &event {
            godot_error!(
                "battle renderer: vehicle skill {skill} for fighter {} has no effect dispatcher",
                actor.get()
            );
        }
        if let BattleEvent::Rewarded {
            experience_each, ..
        } = &event
        {
            self.reward_each = *experience_each;
        }
        if let BattleEvent::Rewarded { meseta, .. } = &event {
            self.reward_meseta = *meseta;
        }
        self.message = narration.line;
        self.transient_column = 11;
        self.message_kind = match narration.beat {
            Beat::Start => MessageKind::None,
            Beat::End(Outcome::Escaped) | Beat::Defense(_) => MessageKind::Transient,
            Beat::End(Outcome::Victory) if matches!(&event, BattleEvent::Ended { .. }) => {
                self.message.clear();
                MessageKind::VictoryRewards
            }
            Beat::End(_) | Beat::LevelUp => MessageKind::Wide,
            Beat::Reward => {
                self.message = "Victory!".into();
                MessageKind::Victory
            }
            // Attack/effect captures decode only the status strip here.
            Beat::Attack(_) | Beat::Damage { .. } | Beat::Hide(_) => MessageKind::None,
            Beat::None if self.message.is_empty() => MessageKind::None,
            Beat::None => MessageKind::Transient,
        };
        if let BattleEvent::Died { fighter } = &event
            && fighter.side() == psiv_core::battle::Side::Party
            && self.party_defeated()
        {
            self.message_kind = MessageKind::Wide;
        }
        if let Beat::Defense(actor) = narration.beat {
            self.transient_column = transient_column(actor);
        }
        if matches!(&event, BattleEvent::Escaped) {
            self.transient_column = 11;
        }
        self.damage = None;
        self.restore_party_pose();
        match narration.beat {
            Beat::Attack(actor) => self.set_party_pose(actor, true),
            Beat::Damage {
                target,
                amount: Some(amount),
                ..
            } => {
                self.damage = Some(DamageDraw { target, amount });
            }
            Beat::Hide(fighter) => self.hide_fighter(fighter),
            Beat::End(outcome) => self.finish_outcome = Some(outcome),
            Beat::None
            | Beat::Start
            | Beat::Defense(_)
            | Beat::Reward
            | Beat::LevelUp
            | Beat::Damage { .. } => {}
        }
        self.current = Some(ActiveEvent {
            remaining: if self.message_kind == MessageKind::Wide
                && self.message.ends_with("defeated...!")
            {
                0x78
            } else {
                BATTLE_DWELL_FRAMES
            },
            wait_for_confirm: timeline::waits_for_confirm(narration.beat),
        });
        self.command_open = false;
    }

    fn update_live_party_hp(&mut self, event: &BattleEvent) {
        let BattleEvent::Resolved {
            target,
            remaining_hp,
            ..
        } = event
        else {
            return;
        };
        if target.side() == psiv_core::battle::Side::Party
            && let Some(member) = self
                .party_status
                .iter_mut()
                .find(|member| member.fighter == target.get())
        {
            member.hp = *remaining_hp;
        }
    }

    fn restore_party_pose(&mut self) {
        for party in &mut self.party_nodes {
            party.node.set_texture(&party.idle);
        }
    }

    fn set_party_pose(&mut self, fighter: FighterId, attack: bool) {
        if fighter.side() != psiv_core::battle::Side::Party {
            return;
        }
        let Some(party) = self
            .party_nodes
            .iter_mut()
            .find(|party| party.fighter == fighter)
        else {
            return;
        };
        if attack {
            if let Some(texture) = party.attack.as_ref() {
                party.node.set_texture(texture);
            }
        } else {
            party.node.set_texture(&party.idle);
        }
    }

    fn hide_fighter(&mut self, fighter: FighterId) {
        if fighter.side() != psiv_core::battle::Side::Enemy {
            return;
        }
        if let Some(enemy) = self
            .enemy_nodes
            .iter_mut()
            .find(|enemy| enemy.fighter == fighter)
        {
            enemy.clear_attack();
            enemy.node.set_visible(false);
        }
    }

    fn start_enemy_animations(&mut self, animations: &[BattleAnimationEvent]) {
        for animation in animations {
            if animation.actor.side() != psiv_core::battle::Side::Enemy {
                continue;
            }
            if let Some(enemy) = self
                .enemy_nodes
                .iter_mut()
                .find(|enemy| enemy.fighter == animation.actor)
            {
                enemy.begin_attack(animation);
            }
        }
    }

    fn party_defeated(&self) -> bool {
        !self.party_status.is_empty() && self.party_status.iter().all(|member| member.hp == 0)
    }

    fn damage_column(&self, target: FighterId) -> Option<i32> {
        if target.side() == psiv_core::battle::Side::Enemy {
            return self
                .enemy_positions
                .get(&target.get())
                .map(|position| i32::from(position & 0x7f) - ENEMY_DAMAGE_COLUMN_OFFSET);
        }
        PARTY_COLUMNS
            .get(target.get().checked_sub(1)? as usize)
            .copied()
    }
}
