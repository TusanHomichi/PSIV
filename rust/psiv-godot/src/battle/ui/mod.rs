//! Godot-side battle composition: the node, its sprites and its clock.
//!
//! This node is deliberately a dumb presentation surface. It never rolls,
//! subtracts HP, chooses a target, or awards anything. It draws the runtime's
//! [`BattleView`] — the command window, the narration page, the status strip,
//! the damage block — and advances the art clocks that have no game meaning
//! (the enemy overlay animation and the decoded attack layers).
//!
//! The split from `psiv-godot/src/battle/`'s single 1,000-line file:
//!
//! - `build.rs` builds the background, enemy and party sprites for a setup;
//! - `draw.rs` composes one frame of plane-A quads from the view;
//! - this file owns the node, its art, and the two clocks.

mod build;
mod draw;

use std::collections::BTreeMap;

use godot::classes::{INode2D, ImageTexture, Node2D, Sprite2D};
use godot::prelude::*;

use psiv_core::battle::FighterId;
use psiv_data::DialogueSet;
use psiv_runtime::{BattleAnimationEvent, BattleView, CommandMenuView, MenuView, PartyStatus};

use super::BattleSetup;
use super::art::BattleArt;
use super::attack::EnemySprite;
use super::chrome::BattleChrome;

/// Plane cells are 8x8 pixels, per `docs/battle/BATTLE_GEOMETRY.md` §1.
pub(super) const BATTLE_CELL_PIXELS: i32 = 8;

/// Party columns in fighter-id order. The retail layout is center-out, so the
/// visible order is slot 4, 2, 1, 3, 5. These are the §3 table verbatim.
pub(crate) const PARTY_COLUMNS: [i32; 5] = [17, 11, 23, 5, 29];
/// Party art starts at plane row 15 / screen y120, and is 6x6 cells (§3).
const PARTY_ROW_Y: f32 = 120.0;

/// §4 places enemy damage three columns left of the enemy's position byte.
const ENEMY_DAMAGE_COLUMN_OFFSET: i32 = 3;
pub(super) const ENEMY_DAMAGE_Y: f32 = 40.0;
pub(super) const PARTY_DAMAGE_Y: f32 = 144.0;
pub(super) const DAMAGE_WIDTH: f32 = 40.0;
pub(super) const DAMAGE_HEIGHT: f32 = 16.0;

/// The authentic Genesis battle frame, `docs/battle/BATTLE_GEOMETRY.md` §1.
pub(crate) const BATTLE_FRAME_WIDTH: f32 = 320.0;
pub(crate) const BATTLE_FRAME_HEIGHT: f32 = 224.0;
const BATTLE_BACKGROUND_WIDTH: i32 = 512;
const BATTLE_BACKGROUND_HEIGHT: i32 = 192;

/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=enemy_name_window`, `pixel_rect`.
pub(super) const ENEMY_NAME_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 16.0,
    y: 8.0,
    w: 96.0,
    h: 24.0,
};
/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.text_runs` run
/// `text=ZORAN BULT`, cell `(3,2)`, pixel `(24,16)`. The same interior origin
/// is used for the runtime enemy name in every command-idle/return capture.
pub(super) const ENEMY_NAME_TEXT_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 24.0,
    y: 16.0,
    w: 80.0,
    h: 8.0,
};

/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=command_window`, `pixel_rect`.
pub(super) const COMMAND_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 24.0,
    y: 40.0,
    w: 64.0,
    h: 56.0,
};
/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.text_runs` runs
/// `COMD`, `MACR`, and `RUN`, beginning at cell `(6,6)` / pixel `(48,48)`;
/// the decoded line pitch is 16 pixels.
pub(super) const COMMAND_TEXT_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 48.0,
    y: 48.0,
    w: 40.0,
    h: 48.0,
};

/// `oracle/layouts/battle_victory.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=victory_window`, `pixel_rect`.
pub(super) const VICTORY_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 80.0,
    y: 128.0,
    w: 160.0,
    h: 40.0,
};
/// `oracle/layouts/battle_victory.json`: `planes.plane_a.text_runs` run
/// `Victory!`, cell `(11,17)` / pixel `(88,136)`.
pub(super) const VICTORY_TEXT_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 88.0,
    y: 136.0,
    w: 144.0,
    h: 8.0,
};

/// Non-capture narration uses the decoded transient/wide placements; attack
/// and effect captures have no message rectangle.
pub(super) const TRANSIENT_MESSAGE_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 88.0,
    y: 144.0,
    w: 96.0,
    h: 24.0,
};
pub(super) const LONG_TRANSIENT_MESSAGE_RECT: super::chrome::WindowRect =
    super::chrome::WindowRect {
        x: 88.0,
        y: 144.0,
        w: 144.0,
        h: 24.0,
    };
pub(super) const WIDE_MESSAGE_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
    x: 80.0,
    y: 128.0,
    w: 160.0,
    h: 40.0,
};

/// `oracle/layouts/battle_command_idle.json`: `planes.plane_a.chrome_rectangles`
/// entry `kind=status_strip`, `pixel_rect`.
pub(super) const STATUS_RECT: super::chrome::WindowRect = super::chrome::WindowRect {
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
pub(super) const COMMAND_CURSOR_WORDS: [u16; 3] = [0x6e8, 0x6e7, 0x6e7];
/// `oracle/layouts/battle_command_idle.json`: separator special cells at
/// global columns 9,16,23,30, rows 21..26, with patterns 0x6f4/0x6f5 and the
/// bottom vertical flip. These holes belong to one status window, not five.
pub(super) const STATUS_SEPARATOR_COLUMNS: [i32; 4] = [9, 16, 23, 30];

pub(super) struct PartySprite {
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
    /// The command icon and cursor art, built as the planes ask for it.
    tiles: super::tiles::CommandTiles,
    background: Option<Gd<Sprite2D>>,
    enemy_nodes: Vec<EnemySprite>,
    party_nodes: Vec<PartySprite>,
    enemy_textures: BTreeMap<(u16, u8), Gd<ImageTexture>>,
    enemy_positions: BTreeMap<u8, u8>,
    vehicle_index: Option<u16>,
    /// What the runtime says this frame; the node draws it and decides nothing.
    view: Option<BattleView>,
}

#[godot_api]
impl INode2D for BattleScreen {
    fn init(base: Base<Node2D>) -> Self {
        BattleScreen {
            base,
            pack_dir: String::new(),
            art: None,
            chrome: None,
            tiles: super::tiles::CommandTiles::default(),
            background: None,
            enemy_nodes: Vec::new(),
            party_nodes: Vec::new(),
            enemy_textures: BTreeMap::new(),
            enemy_positions: BTreeMap::new(),
            vehicle_index: None,
            view: None,
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
        draw::compose(self);
    }
}

impl BattleScreen {
    /// Loads the battle art and the dialogue chrome/font conventions.
    pub(crate) fn configure(&mut self, pack_dir: &str, command_art: psiv_data::CommandUiArt) {
        self.pack_dir = pack_dir.to_owned();
        self.tiles = super::tiles::CommandTiles::new(command_art);
        self.art = match BattleArt::load(pack_dir) {
            Ok(art) => Some(art),
            Err(error) => {
                godot_error!("battle art failed to load from {pack_dir}: {error}");
                None
            }
        };
        match DialogueSet::load(std::path::Path::new(pack_dir)) {
            Ok(set) => {
                let ink = self
                    .art
                    .as_ref()
                    .map_or([Color::BLACK; 4], |art| art.line_ink());
                self.chrome = BattleChrome::build(pack_dir, &set, ink);
                if self.chrome.is_none() {
                    godot_error!("battle: dialogue chrome/font art failed to load");
                }
            }
            Err(error) => godot_error!("battle: dialogue pack failed to load: {error}"),
        }
    }

    /// Rebuilds the art for one battle and shows the node.
    ///
    /// The runtime has already started the battle and produced this frame's
    /// view; [`BattleScreen::set_view`] hands it over immediately after.
    pub(crate) fn begin(&mut self, setup: BattleSetup) {
        self.clear_sprites();
        self.enemy_positions.clear();
        self.vehicle_index = setup.vehicle_index;
        self.view = None;
        self.build_background(&setup);
        self.build_enemies(&setup.enemies, setup.enemy_animation_phase_ticks);
        self.build_party(&setup);
        self.base_mut().set_visible(true);
        self.base_mut().queue_redraw();
    }

    /// Takes this frame's view: applies the visibility and pose cues it
    /// carries, and hands the node what to draw.
    pub(crate) fn set_view(&mut self, view: BattleView) {
        for status in &view.enemies {
            if let Some(position) = status.position
                && self.enemy_nodes.iter().any(|enemy| {
                    enemy.fighter.get() == status.fighter && enemy.enemy_id != status.enemy_id
                })
            {
                self.reseat_enemy(status.fighter, status.enemy_id, position);
            }
            if let Some(enemy) = self
                .enemy_nodes
                .iter_mut()
                .find(|enemy| enemy.fighter.get() == status.fighter)
            {
                enemy.node.set_visible(status.visible);
            }
        }
        // The party's own sprites are never hidden by a battle, so a revive
        // needs nothing here: the node was never taken away.
        // The runtime says which party bodies the plane holds: the cartridge
        // clears the party rows as a per-character window opens
        // (`Battle_OpenCharComd`, `ps4.asm:2099-2101`: `loc_E4C` clears,
        // `loc_E64` restores the saved enemy rows, `loc_EB6` draws one body)
        // and a body returns as its owner acts.
        for party in &mut self.party_nodes {
            party.node.set_visible(
                view.shown
                    .as_ref()
                    .is_none_or(|shown| shown.contains(&party.fighter.get())),
            );
        }
        self.restore_party_pose();
        for actor in &view.poses {
            self.set_party_pose(*actor, true);
        }
        for animation in &view.animations {
            self.start_enemy_animation(animation);
        }
        self.view = Some(view);
        self.base_mut().queue_redraw();
    }

    /// The first enemy's overlay clocks, for the capture log: the oracle's
    /// `Enemy_Sprites` words say the same thing about the cartridge.
    pub(crate) fn overlay_clocks(&self) -> Vec<(usize, u8)> {
        self.enemy_nodes
            .first()
            .and_then(|enemy| enemy.animation.as_ref())
            .map(super::enemy_overlay::EnemyAnimation::clocks)
            .unwrap_or_default()
    }

    /// The frame's view, once the runtime has produced one.
    pub(crate) fn view(&self) -> Option<&BattleView> {
        self.view.as_ref()
    }

    /// Advances the art clocks that carry no game state: the enemy overlay
    /// animation and any live attack layer.
    pub(crate) fn advance_animation(&mut self) {
        for enemy in &mut self.enemy_nodes {
            if let Some(animation) = enemy.animation.as_mut()
                && let Some(texture) = animation.advance()
            {
                enemy.node.set_texture(&texture);
            }
            enemy.advance_attack();
        }
    }

    /// Machine-readable summary of the command surface, for the native input
    /// drivers (`PSIV_DEBUG_ROUTE=1`, `Field::debug_play_state`).
    ///
    /// The shape is the one the drivers have always read: `ready`, `finishing`,
    /// `message`, the COMD cursor as `cursor`, and `menu` — `null` on the main
    /// options, the command window's own lists otherwise.
    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        let view = self.view.as_ref();
        let message = view.map(|view| view.message.as_str()).unwrap_or("");
        let finishing = view.is_some_and(|view| view.finishing);
        let ready = view.is_some_and(|view| view.ready);
        let cursor = view.map_or(0, |view| view.cursor);
        let menu = view.and_then(|view| view.menu.as_ref()).and_then(menu_json);
        serde_json::json!({
            "ready": ready,
            "finishing": finishing,
            "message": message,
            "cursor": cursor,
            "menu": menu,
        })
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

    fn start_enemy_animation(&mut self, animation: &BattleAnimationEvent) {
        if animation.actor.side() != psiv_core::battle::Side::Enemy {
            return;
        }
        if let Some(enemy) = self
            .enemy_nodes
            .iter_mut()
            .find(|enemy| enemy.fighter == animation.actor)
        {
            enemy.begin_attack(animation);
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

/// The debug probe's `menu` object, spelled the way the native drivers parse
/// it.
///
/// `null` unless the per-character command window is open — that is the
/// contract the drivers' own loops read (`tools/native/native_battle_recovery.gd`,
/// `native_alshline.gd`, `native_bioplant.gd`: a null `menu` with `ready` true
/// is the COMD/MACR/RUN window, and they press accept to open it). `page` keeps
/// Rust's own `Debug` spelling of the runtime's page enum (`"Actions"`,
/// `"Targets(Technique(31))"`), which is what those drivers compare against.
fn menu_json(menu: &MenuView) -> Option<serde_json::Value> {
    match menu {
        MenuView::Top { .. } | MenuView::VehicleSkills { .. } => None,
        MenuView::Commands(commands) => Some(commands_json(commands)),
    }
}

fn commands_json(view: &CommandMenuView) -> serde_json::Value {
    serde_json::json!({
        "title": view.title,
        "rows": view.rows.iter().map(|row| serde_json::json!([row.label, row.enabled])).collect::<Vec<_>>(),
        "cursor": view.cursor,
        "page": format!("{:?}", view.page),
        "actor": view.actor,
        "character": view.character,
        "party": part_json(&view.party),
        "enemies": view.enemies,
        "techniques": view.techniques.iter().map(|technique| serde_json::json!({
            "id": technique.id, "name": technique.name, "cost": technique.cost,
            "available": technique.available,
        })).collect::<Vec<_>>(),
        "skills": view.skills.iter().map(|skill| serde_json::json!({
            "id": skill.id, "name": skill.name, "remaining": skill.remaining,
            "available": skill.available,
        })).collect::<Vec<_>>(),
        "targets": view.targets,
    })
}

fn part_json(party: &[PartyStatus]) -> Vec<serde_json::Value> {
    party
        .iter()
        .map(|member| {
            serde_json::json!({
                "id": member.fighter, "name": member.name, "hp": member.hp,
                "max_hp": member.max_hp, "tp": member.tp, "status": member.status,
            })
        })
        .collect()
}
