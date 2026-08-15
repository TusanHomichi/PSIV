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
use psiv_data::{DialogueSet, Role};

use super::art::BattleArt;
use super::timeline::{self, Beat};
use super::{BattleSetup, EnemyPlacement, PartyPlacement};

/// `Battle_Speed == 2`: 12 * (speed + 1) frames. Runtime has no speed setting
/// API yet, so this is the documented retail default rather than a hidden
/// presentation guess.
pub(crate) const BATTLE_DWELL_FRAMES: u16 = 36;

/// Party columns in fighter-id order. The retail layout is center-out, so the
/// visible order is slot 4, 2, 1, 3, 5.
pub(crate) const PARTY_COLUMNS: [i32; 5] = [17, 11, 23, 5, 29];

/// Byte-pinned upper battle window used for the one-line event narration.
const NARRATION_RECT: WindowRect = WindowRect {
    x: 40.0,
    y: 72.0,
    w: 120.0,
    h: 48.0,
};

/// The retail main options rectangle was not pinned in BATTLE_GEOMETRY.md.
/// Tier 1 temporarily draws COMD/RUN in the known small-list rectangle. Keep
/// this named and visible so it cannot be mistaken for byte-verified layout.
const PROVISIONAL_COMMAND_RECT: WindowRect = WindowRect {
    x: 248.0,
    y: 120.0,
    w: 56.0,
    // Deliberately 7x6 rather than the byte-pinned small-list 7x5: the
    // retail main-options rect is unresolved, and two 8x16 glyph rows need
    // the extra cell to keep COMD and RUN inside the frame.
    h: 48.0,
};

#[derive(Clone, Copy)]
struct WindowRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl WindowRect {
    fn inset(self, amount: f32) -> WindowRect {
        WindowRect {
            x: self.x + amount,
            y: self.y + amount,
            w: (self.w - amount * 2.0).max(0.0),
            h: (self.h - amount * 2.0).max(0.0),
        }
    }
}

struct Quad {
    texture: Gd<ImageTexture>,
    dest: Rect2,
    src: Rect2,
}

struct BattleChrome {
    tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    font: Gd<ImageTexture>,
    glyph_at: BTreeMap<char, Vector2>,
    glyph: Vector2,
    cell: f32,
    text_color: Color,
}

impl BattleChrome {
    /// Uses the same pack assets and role flips as `dialogue.rs`, without
    /// borrowing the dialogue renderer or changing its >1k-line module.
    fn build(pack_dir: &str, set: &DialogueSet) -> Option<BattleChrome> {
        let strip = load_image(pack_dir, &set.window.png)?;
        let mut tiles = BTreeMap::new();
        for role in Role::ALL {
            let tile = set.window.role(role)?;
            let region = Rect2i::new(
                Vector2i::new(tile.x, tile.y),
                Vector2i::new(tile.width as i32, tile.height as i32),
            );
            let mut cell = strip.get_region(region)?;
            if tile.flip_h {
                cell.flip_x();
            }
            if tile.flip_v {
                cell.flip_y();
            }
            tiles.insert(role.as_str(), ImageTexture::create_from_image(&cell)?);
        }

        let font = ImageTexture::create_from_image(&load_image(pack_dir, &set.font.png)?)?;
        let glyph_at = set
            .font
            .by_char
            .iter()
            .filter_map(|(ch, byte)| {
                let glyph = set.font.glyphs.iter().find(|g| g.byte == *byte)?;
                Some((*ch, Vector2::new(glyph.x as f32, glyph.y as f32)))
            })
            .collect();
        let text_color = set
            .window
            .palette
            .colors
            .get(15)
            .map_or(Color::WHITE, |rgb| {
                Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)
            });

        Some(BattleChrome {
            tiles,
            font,
            glyph_at,
            glyph: Vector2::new(set.font.glyph.width as f32, set.font.glyph.height as f32),
            cell: set.window.geometry.cell_pixels as f32,
            text_color,
        })
    }

    fn tile(&self, name: &'static str) -> Option<Gd<ImageTexture>> {
        self.tiles.get(name).cloned()
    }

    fn frame(&self, rect: WindowRect) -> Option<Vec<Quad>> {
        let cols = (rect.w / self.cell).round() as i32;
        let rows = (rect.h / self.cell).round() as i32;
        if cols < 2 || rows < 2 {
            return None;
        }
        let mut quads = Vec::new();
        let at = |x: i32, y: i32| {
            Rect2::new(
                Vector2::new(rect.x + x as f32 * self.cell, rect.y + y as f32 * self.cell),
                Vector2::new(self.cell, self.cell),
            )
        };
        let src = Rect2::new(Vector2::ZERO, Vector2::new(self.cell, self.cell));
        let mut push = |name: &'static str, x: i32, y: i32| {
            if let Some(texture) = self.tile(name) {
                quads.push(Quad {
                    texture,
                    dest: at(x, y),
                    src,
                });
            }
        };

        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                push("fill", x, y);
            }
        }
        for x in 1..cols - 1 {
            push("edge_top", x, 0);
            push("edge_bottom", x, rows - 1);
        }
        for y in 1..rows - 1 {
            push("edge_left", 0, y);
            push("edge_right", cols - 1, y);
        }
        push("corner_top_left", 0, 0);
        push("corner_top_right", cols - 1, 0);
        push("corner_bottom_left", 0, rows - 1);
        push("corner_bottom_right", cols - 1, rows - 1);
        Some(quads)
    }

    fn text(&self, text: &str, rect: WindowRect) -> Vec<Quad> {
        let cols = (rect.w / self.glyph.x.max(1.0)).floor() as usize;
        let rows = (rect.h / self.glyph.y.max(1.0)).floor() as usize;
        if cols == 0 || rows == 0 {
            return Vec::new();
        }
        let mut quads = Vec::new();
        let mut col = 0usize;
        let mut row = 0usize;
        for ch in text.chars() {
            if ch == '\n' {
                col = 0;
                row += 1;
                continue;
            }
            if col >= cols {
                col = 0;
                row += 1;
            }
            if row >= rows {
                break;
            }
            let Some(source) = self.glyph_at.get(&ch).or_else(|| self.glyph_at.get(&'?')) else {
                col += 1;
                continue;
            };
            quads.push(Quad {
                texture: self.font.clone(),
                dest: Rect2::new(
                    Vector2::new(
                        rect.x + col as f32 * self.glyph.x,
                        rect.y + row as f32 * self.glyph.y,
                    ),
                    self.glyph,
                ),
                src: Rect2::new(*source, self.glyph),
            });
            col += 1;
        }
        quads
    }
}

fn load_image(pack_dir: &str, name: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{name}");
    Image::load_from_file(&GString::from(path.as_str()))
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
    events: VecDeque<BattleEvent>,
    current: Option<ActiveEvent>,
    message: String,
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
            events: VecDeque::new(),
            current: None,
            message: String::new(),
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
    }

    fn draw(&mut self) {
        let Some(chrome) = self.chrome.as_ref() else {
            return;
        };
        let cell = chrome.cell;
        let glyph_height = chrome.glyph.y;
        let text_color = chrome.text_color;
        let mut quads = Vec::new();
        if let Some(frame) = chrome.frame(NARRATION_RECT) {
            quads.extend(frame);
            quads.extend(chrome.text(&self.message, NARRATION_RECT.inset(cell)));
        }
        if self.command_open
            && let Some(frame) = chrome.frame(PROVISIONAL_COMMAND_RECT)
        {
            quads.extend(frame);
            let text_rect = PROVISIONAL_COMMAND_RECT.inset(cell);
            quads.extend(chrome.text("COMD\nRUN", text_rect));
        }
        if let Some(damage) = self.damage
            && let Some(column) = self.damage_column(damage.target)
        {
            let rect = WindowRect {
                x: column as f32 * 8.0,
                y: if damage.target.side() == psiv_core::battle::Side::Enemy {
                    40.0
                } else {
                    144.0
                },
                w: 40.0,
                h: 16.0,
            };
            let label = if damage.critical {
                format!("{}!", damage.amount)
            } else {
                damage.amount.to_string()
            };
            quads.extend(chrome.text(&label, rect));
        }
        for quad in quads {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        if self.command_open {
            let y = PROVISIONAL_COMMAND_RECT.y + cell + self.cursor as f32 * glyph_height;
            let x = PROVISIONAL_COMMAND_RECT.x + cell * 0.35;
            let points = PackedVector2Array::from(&[
                Vector2::new(x, y + 2.0),
                Vector2::new(x + 5.0, y + 6.0),
                Vector2::new(x, y + 10.0),
            ]);
            self.base_mut().draw_colored_polygon(&points, text_color);
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
    pub(crate) fn begin(&mut self, setup: BattleSetup, events: Vec<BattleEvent>) {
        self.clear_sprites();
        self.enemy_positions.clear();
        self.names.clear();
        self.character_names.clear();
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
            self.cursor = (self.cursor + 1).min(1);
            self.base_mut().queue_redraw();
        }
        if !input.is_action_just_pressed("ui_accept") {
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
        if let Some(path) = selected {
            let full = format!("{}/{}", self.pack_dir, path);
            match Image::load_from_file(&GString::from(full.as_str())) {
                Some(image) => match ImageTexture::create_from_image(&image) {
                    Some(texture) => node.set_texture(&texture),
                    None => godot_error!("battle background texture creation failed: {full}"),
                },
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
            node.set_position(Vector2::new(
                ((enemy.position & 0x7f) as i32 - width as i32) as f32 * 8.0,
                (15 - height as i32) as f32 * 8.0,
            ));
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
        if self.art.is_none() {
            return;
        }
        for (index, member) in party.iter().enumerate() {
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
            let column = PARTY_COLUMNS.get(index).copied().unwrap_or(0);
            node.set_position(Vector2::new(column as f32 * 8.0, 120.0));
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
            }
            return;
        };

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
                .map(|position| i32::from(position & 0x7f) - 3);
        }
        PARTY_COLUMNS
            .get(target.get().checked_sub(1)? as usize)
            .copied()
    }
}
