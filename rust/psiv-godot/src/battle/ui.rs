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

use super::art::BattleArt;
use super::chrome::{BattleChrome, WindowRect};
use super::layout::{append_status_quads, tile_dest};
use super::timeline::{self, Beat};
use super::{BattleSetup, EnemyPlacement, PartyPlacement};

/// `Battle_Speed == 2`: 12 * (speed + 1) frames, per
/// `docs/BATTLE_GEOMETRY.md` §5. Runtime has no speed setting API yet, so this
/// is the documented retail default rather than a hidden presentation guess.
pub(crate) const BATTLE_DWELL_FRAMES: u16 = 36;

/// Plane cells are 8x8 pixels, per `docs/BATTLE_GEOMETRY.md` §1.
pub(super) const BATTLE_CELL_PIXELS: i32 = 8;

/// Party columns in fighter-id order. The retail layout is center-out, so the
/// visible order is slot 4, 2, 1, 3, 5. These are the §3 table verbatim.
pub(crate) const PARTY_COLUMNS: [i32; 5] = [17, 11, 23, 5, 29];
/// Party art starts at plane row 15 / screen y120, and is 6x6 cells (§3).
const PARTY_ROW_Y: f32 = 120.0;

/// The enemy decoder anchors art at row 15 and grows it upward (§2).
const ENEMY_BASELINE_ROW: i32 = 15;

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

/// The transient and wide message rectangles are retained for non-capture
/// event narration. Their placements are the corresponding records in
/// `docs/BATTLE_GEOMETRY.md`; attack/followup captures deliberately contain no
/// message rectangle and therefore suppress these during those beats.
const TRANSIENT_MESSAGE_RECT: WindowRect = WindowRect {
    x: 88.0,
    y: 144.0,
    w: 96.0,
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

/// Converts the formation position byte and the art record's dimensions into
/// the sprite's top-left pixel. `docs/BATTLE_GEOMETRY.md` §2 makes the byte the
/// art's bottom-right column, not its left edge; both axes use the 8-pixel
/// plane cell.
pub(super) fn enemy_sprite_origin(position: u8, width_cells: u16, height_cells: u16) -> (i32, i32) {
    let column = i32::from(position & 0x7F);
    (
        (column - i32::from(width_cells)) * BATTLE_CELL_PIXELS,
        (ENEMY_BASELINE_ROW - i32::from(height_cells)) * BATTLE_CELL_PIXELS,
    )
}

struct EnemySprite {
    fighter: FighterId,
    node: Gd<Sprite2D>,
}

struct PartySprite {
    fighter: FighterId,
    node: Gd<Sprite2D>,
    idle: Gd<ImageTexture>,
    attack: Option<Gd<ImageTexture>>,
}

struct ActiveEvent {
    remaining: u16,
}

/// A request for the field to call the runtime epilogue after playback.
#[derive(Clone, Copy)]
pub(crate) struct FinishRequest {
    pub(crate) outcome: Outcome,
    pub(crate) reward_each: u16,
}

#[derive(Clone, Copy)]
struct DamageDraw {
    target: FighterId,
    amount: u16,
    critical: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    None,
    Transient,
    Wide,
}

#[derive(Clone)]
pub(super) struct PartyStatus {
    pub(super) fighter: u8,
    pub(super) character: u8,
    pub(super) name: String,
    pub(super) hp: u16,
    pub(super) tp: u16,
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
    events: VecDeque<BattleEvent>,
    current: Option<ActiveEvent>,
    message: String,
    message_kind: MessageKind,
    damage: Option<DamageDraw>,
    command_open: bool,
    cursor: usize,
    finish_outcome: Option<Outcome>,
    reward_each: u16,
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
            current: None,
            message: String::new(),
            message_kind: MessageKind::None,
            damage: None,
            command_open: false,
            cursor: 0,
            finish_outcome: None,
            reward_each: 0,
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
            && let Some(frame) = chrome.frame(COMMAND_RECT)
        {
            quads.extend(frame);
            quads.extend(chrome.text_with_pitch("COMD\nMACR\nRUN", COMMAND_TEXT_RECT, 16.0));
        }
        if self.message_kind != MessageKind::None && !self.message.is_empty() {
            let rect = match self.message_kind {
                MessageKind::Transient => TRANSIENT_MESSAGE_RECT,
                MessageKind::Wide => WIDE_MESSAGE_RECT,
                MessageKind::None => unreachable!(),
            };
            let text_rect = if self.message == "Victory!" {
                VICTORY_TEXT_RECT
            } else {
                rect.inset(cell)
            };
            let frame_rect = if self.message == "Victory!" {
                VICTORY_RECT
            } else {
                rect
            };
            if let Some(frame) = chrome.frame(frame_rect) {
                quads.extend(frame);
                quads.extend(chrome.text(&self.message, text_rect));
            }
        }
        append_status_quads(chrome, &self.party_status, &mut quads);
        if let Some(damage) = self.damage
            && let Some(column) = self.damage_column(damage.target)
        {
            // `docs/BATTLE_GEOMETRY.md` §4: enemy damage is row 5 / y40,
            // party damage is row 18 / y144, and the digit block is 5x2 cells.
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
            let label = if damage.critical {
                format!("{}!", damage.amount)
            } else {
                damage.amount.to_string()
            };
            quads.extend(chrome.text(&label, rect));
        }
        if self.command_open {
            for (row, pattern) in COMMAND_CURSOR_WORDS.into_iter().enumerate() {
                if let Some(quad) =
                    chrome.window_word(pattern, false, false, tile_dest(4, 6 + row as i32 * 2))
                {
                    // `0x6e8` is selected COMD; `0x6e7` is the retail blue
                    // disabled/unselected form for MACR and RUN.
                    quads.push(quad);
                }
            }
        }
        for quad in quads {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        self.draw_status_icons(palette);
    }
}

impl BattleScreen {
    fn draw_pixel(&mut self, x: f32, y: f32, color: Color) {
        let points = PackedVector2Array::from(&[
            Vector2::new(x, y),
            Vector2::new(x + 1.0, y),
            Vector2::new(x + 1.0, y + 1.0),
            Vector2::new(x, y + 1.0),
        ]);
        self.base_mut().draw_colored_polygon(&points, color);
    }

    fn draw_pattern(&mut self, origin: Vector2, rows: &[&str], palette: [Color; 16]) {
        for (row, line) in rows.iter().enumerate() {
            for (column, symbol) in line.chars().enumerate() {
                let color = match symbol {
                    'B' => palette[14],
                    'W' => palette[15],
                    'K' => palette[0],
                    'L' => palette[2],
                    'R' => palette[13],
                    'P' => palette[12],
                    'Y' => palette[11],
                    'q' => palette[6],
                    'o' => palette[5],
                    _ => continue,
                };
                self.draw_pixel(origin.x + column as f32, origin.y + row as f32, color);
            }
        }
    }

    fn draw_status_icons(&mut self, palette: [Color; 16]) {
        const QUESTION: [&str; 16] = [
            "BWWWWWWWWWWWWWWB",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKqoYYYYoqKKKW",
            "WKKKoYqKKqYoqKKW",
            "WKKKYYKKKKYYqKKW",
            "WKKKoYKKKKYYqKKW",
            "WKKKKKKqoYYqKKKW",
            "WKKKKKKYYoqKKKKW",
            "WKKKKKKYYqKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKYYqKKKKKW",
            "WKKKKKKYYqKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "BWWWWWWWWWWWWWWB",
        ];
        const BLANK: [&str; 16] = [
            "BWWWWWWWWWWWWWWB",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "WKKKKKKKKKKKKKKW",
            "BWWWWWWWWWWWWWWB",
        ];
        for (pane, start) in STATUS_PANE_START_CELLS.iter().copied().enumerate() {
            let member = pane > 0 && pane < 4 && self.party_status.get(pane - 1).is_some();
            if pane == 0 || pane == 4 || member {
                // `oracle/layouts/battle_command_idle.json` places the outer
                // icon words at cells 7/8 and the party icon words at
                // 14/15, 21/22, and 28/29; all are 16x16 at row 22.
                let origin = Vector2::new(
                    (start + 5) as f32 * BATTLE_CELL_PIXELS as f32,
                    STATUS_NAME_Y,
                );
                self.draw_pattern(origin, if member { &QUESTION } else { &BLANK }, palette);
            }
        }
    }

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
    pub(crate) fn begin(&mut self, setup: BattleSetup, events: Vec<BattleEvent>) {
        self.clear_sprites();
        self.enemy_positions.clear();
        self.names.clear();
        self.character_names.clear();
        self.party_status.clear();
        self.events.clear();
        self.current = None;
        self.finish_outcome = None;
        self.finish_request = None;
        self.close_when_idle = false;
        self.close_ready = false;
        self.command_open = false;
        self.cursor = 0;
        self.reward_each = 0;
        self.message.clear();
        self.message_kind = MessageKind::None;
        self.damage = None;

        self.build_background(&setup);
        self.build_enemies(&setup.enemies);
        self.build_party(&setup.party);
        self.events.extend(events);
        self.base_mut().set_visible(true);
        self.start_next_event();
        self.base_mut().queue_redraw();
    }

    /// Reads the Tier-1 COMD/RUN menu. The field calls Runtime after this
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
            // MACR is visible for retail parity; macro execution is Tier 3.
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
        if let Some(active) = self.current.as_mut() {
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

    pub(crate) fn enqueue_events(&mut self, events: Vec<BattleEvent>) {
        self.events.extend(events);
        if self.current.is_none() {
            self.start_next_event();
        }
        self.base_mut().queue_redraw();
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

    fn build_enemies(&mut self, enemies: &[EnemyPlacement]) {
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
            let line = super::art::enemy_cram_line(enemy.position);
            let key = (enemy.enemy_id, line);
            let texture = self.enemy_textures.get(&key).cloned().or_else(|| {
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
            });
            if let Some(texture) = texture {
                node.set_texture(&texture);
            }
            self.base_mut().add_child(&node);
            self.enemy_positions
                .insert(enemy.fighter_id, enemy.position);
            self.names.insert(enemy.fighter_id, enemy.name.clone());
            self.enemy_nodes.push(EnemySprite { fighter, node });
        }
    }

    fn build_party(&mut self, party: &[PartyPlacement]) {
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
        if self.art.is_none() {
            return;
        }
        for member in party {
            let Some(fighter) = FighterId::new(member.fighter_id) else {
                godot_error!("battle party has invalid fighter id {}", member.fighter_id);
                continue;
            };
            let Some(idle) = self
                .art
                .as_ref()
                .and_then(|art| art.character_texture(&self.pack_dir, member.character, 0))
            else {
                godot_error!(
                    "battle character {} idle pose failed to load for fighter {}",
                    member.character,
                    member.fighter_id
                );
                continue;
            };
            let attack = self
                .art
                .as_ref()
                .and_then(|art| art.character_texture(&self.pack_dir, member.character, 1));
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
        let Some(event) = self.events.pop_front() else {
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

        self.update_live_party_hp(&event);
        let narration = timeline::narration(&event, &self.names, &self.character_names);
        if narration.line == "Unhandled battle event." {
            godot_error!("battle renderer: unhandled BattleEvent: {event:?}");
        }
        if let BattleEvent::UnsupportedAbility { actor, ability } = event {
            godot_error!(
                "battle renderer: engine emitted unsupported ability {ability} for fighter {}",
                actor.get()
            );
        }
        if let BattleEvent::Rewarded {
            experience_each, ..
        } = event
        {
            self.reward_each = experience_each;
        }
        self.message = narration.line;
        self.message_kind = match narration.beat {
            Beat::Start => MessageKind::None,
            Beat::End(_) | Beat::Reward | Beat::LevelUp => MessageKind::Wide,
            // `oracle/layouts/battle_attack_effect.json` and
            // `battle_followup.json` decode only the status strip during the
            // attack/effect interval. The retail transient message is absent
            // there, so these beats carry narration for logs but no window.
            Beat::Attack(_) | Beat::Damage { .. } | Beat::Hide(_) => MessageKind::None,
            Beat::None => MessageKind::Transient,
        };
        self.damage = None;
        self.restore_party_pose();
        match narration.beat {
            Beat::Attack(actor) => self.set_party_pose(actor, true),
            Beat::Damage {
                target,
                amount: Some(amount),
                critical,
            } => {
                self.damage = Some(DamageDraw {
                    target,
                    amount,
                    critical,
                });
            }
            Beat::Hide(fighter) => self.hide_fighter(fighter),
            Beat::End(outcome) => self.finish_outcome = Some(outcome),
            Beat::None | Beat::Start | Beat::Reward | Beat::LevelUp | Beat::Damage { .. } => {}
        }
        self.current = Some(ActiveEvent {
            remaining: BATTLE_DWELL_FRAMES,
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
            enemy.node.set_visible(false);
        }
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

#[cfg(test)]
mod tests {
    use super::enemy_sprite_origin;

    #[test]
    fn enemy_position_byte_is_a_bottom_right_anchor() {
        assert_eq!(enemy_sprite_origin(20, 10, 10), (80, 40));
        assert_eq!(enemy_sprite_origin(137, 6, 6), (24, 72));
        assert_eq!(enemy_sprite_origin(159, 6, 6), (200, 72));
    }

    #[test]
    fn enemy_palette_bit_does_not_move_the_body() {
        assert_eq!(enemy_sprite_origin(20 | 0x80, 10, 10), (80, 40));
    }
}
