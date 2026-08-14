//! The Godot presentation layer: a thin gdext bridge over `psiv-runtime`.
//!
//! This crate owns pixels and input, zero game rules. Floats are legal here —
//! they exist only between the engine's integer state and the screen.

use godot::classes::{
    Camera2D, ColorRect, INode2D, Image, ImageTexture, Input, Node2D, ProjectSettings, Sprite2D,
};
use godot::global::godot_error;
use godot::prelude::*;

use psiv_core::{Cell, Direction, StepFrames, WarpTrigger};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};

struct PsivExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PsivExtension {}

const CELL_PIXELS: f32 = 16.0;
/// Piata, one cell below the academy doors — the golden test's spawn.
const SPAWN_MAP: u16 = 0x010;
const SPAWN_CELL: (u16, u16) = (31, 8);

/// The field scene: map picture, placeholder party marker, placeholder NPCs.
#[derive(GodotClass)]
#[class(base=Node2D)]
struct Field {
    base: Base<Node2D>,
    runtime: Option<Runtime>,
    map_sprite: Option<Gd<Sprite2D>>,
    party: Option<Gd<ColorRect>>,
    npc_nodes: Vec<Gd<ColorRect>>,
    camera: Option<Gd<Camera2D>>,
}

#[godot_api]
impl INode2D for Field {
    fn init(base: Base<Node2D>) -> Self {
        Field {
            base,
            runtime: None,
            map_sprite: None,
            party: None,
            npc_nodes: Vec::new(),
            camera: None,
        }
    }

    fn ready(&mut self) {
        let pack_dir = ProjectSettings::singleton()
            .globalize_path("res://../runtime-pack")
            .to_string();
        let data = match GameData::load(std::path::Path::new(&pack_dir)) {
            Ok(data) => data,
            Err(e) => {
                godot_error!("runtime pack failed to load from {pack_dir}: {e}");
                return;
            }
        };
        let runtime = match Runtime::new(
            data,
            SPAWN_MAP,
            Cell::new(SPAWN_CELL.0, SPAWN_CELL.1),
            Direction::Up,
            StepFrames::default(),
        ) {
            Ok(rt) => rt,
            Err(e) => {
                godot_error!("runtime failed to start: {e}");
                return;
            }
        };

        let mut map_sprite = Sprite2D::new_alloc();
        map_sprite.set_centered(false);
        self.base_mut().add_child(&map_sprite);
        self.map_sprite = Some(map_sprite);

        let mut party = ColorRect::new_alloc();
        party.set_size(Vector2::new(CELL_PIXELS, CELL_PIXELS));
        party.set_color(Color::from_rgb(0.95, 0.35, 0.2));
        party.set_z_index(10);
        self.base_mut().add_child(&party);
        self.party = Some(party);

        let mut camera = Camera2D::new_alloc();
        camera.set_zoom(Vector2::new(3.0, 3.0));
        self.base_mut().add_child(&camera);
        camera.make_current();
        self.camera = Some(camera);

        self.runtime = Some(runtime);
        self.load_map_visuals(&pack_dir);
        self.sync_positions();
        godot_print!(
            "PSIV field ready: map {:#05x}, party at ({}, {})",
            SPAWN_MAP,
            SPAWN_CELL.0,
            SPAWN_CELL.1
        );
    }

    fn physics_process(&mut self, _delta: f64) {
        let input = read_input();
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        let events = runtime.tick(input);
        for event in events {
            match event {
                RuntimeEvent::StepCompleted { .. } => {}
                RuntimeEvent::MapChanged { map, trigger } => {
                    let kind = match trigger {
                        WarpTrigger::MapChange => "doorway",
                        WarpTrigger::NormalGround => "ground",
                    };
                    godot_print!("map change ({kind}) -> {:#05x}", map.0);
                    let pack_dir = ProjectSettings::singleton()
                        .globalize_path("res://../runtime-pack")
                        .to_string();
                    self.load_map_visuals(&pack_dir);
                }
                RuntimeEvent::UnpackedTarget { map } => {
                    godot_error!("transition target {:#05x} is not in the pack", map.0);
                }
                RuntimeEvent::WarpUnmapped { cell } => {
                    godot_error!("type-1 cell with no doorway record at {cell:?}");
                }
            }
        }
        self.sync_positions();
    }
}

impl Field {
    /// Loads the current map's PNG and rebuilds NPC placeholders.
    fn load_map_visuals(&mut self, pack_dir: &str) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let id = runtime.map_id().0;

        // The manifest names each map's PNG; find it by id prefix.
        let maps_dir = format!("{pack_dir}/maps");
        let prefix = format!("{id:03x}_");
        let png = std::fs::read_dir(&maps_dir).ok().and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|name| name.starts_with(&prefix) && name.ends_with(".png"))
        });
        match png {
            Some(name) => {
                let path = format!("{maps_dir}/{name}");
                let image = Image::load_from_file(&GString::from(path.as_str()));
                match image {
                    Some(image) => {
                        if let Some(texture) = ImageTexture::create_from_image(&image) {
                            if let Some(sprite) = self.map_sprite.as_mut() {
                                sprite.set_texture(&texture);
                            }
                        }
                    }
                    None => godot_error!("could not load map image {path}"),
                }
            }
            None => godot_error!("no PNG for map {id:#05x} in {maps_dir}"),
        }

        // NPC placeholders: rebuild per map.
        for npc in &mut self.npc_nodes {
            npc.queue_free();
        }
        self.npc_nodes.clear();
        let npcs: Vec<(u16, u16)> = runtime
            .map()
            .npcs()
            .iter()
            .map(|n| (n.cell.x, n.cell.y))
            .collect();
        for (x, y) in npcs {
            let mut rect = ColorRect::new_alloc();
            rect.set_size(Vector2::new(CELL_PIXELS, CELL_PIXELS));
            rect.set_color(Color::from_rgb(0.2, 0.5, 0.95));
            rect.set_position(Vector2::new(
                f32::from(x) * CELL_PIXELS,
                f32::from(y) * CELL_PIXELS,
            ));
            rect.set_z_index(5);
            self.base_mut().add_child(&rect);
            self.npc_nodes.push(rect);
        }
    }

    /// Places the party marker and camera at the engine's position.
    fn sync_positions(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let state = runtime.state();
        let cell = state.cell();
        let (ox, oy) = state.render_offset_16ths();
        // Sixteenths of a cell are pixels at 1x, so this is integer-exact.
        #[allow(clippy::cast_precision_loss)]
        let pos = Vector2::new(
            f32::from(cell.x) * CELL_PIXELS + ox as f32,
            f32::from(cell.y) * CELL_PIXELS + oy as f32,
        );
        if let Some(party) = self.party.as_mut() {
            party.set_position(pos);
        }
        if let Some(camera) = self.camera.as_mut() {
            camera.set_position(pos + Vector2::new(CELL_PIXELS / 2.0, CELL_PIXELS / 2.0));
        }
    }
}

/// One direction per tick; the engine has no diagonals, so order breaks ties.
fn read_input() -> psiv_core::Input {
    let input = Input::singleton();
    let held = [
        ("ui_up", Direction::Up),
        ("ui_down", Direction::Down),
        ("ui_left", Direction::Left),
        ("ui_right", Direction::Right),
    ]
    .into_iter()
    .find(|(action, _)| input.is_action_pressed(*action));
    match held {
        Some((_, dir)) => psiv_core::Input::Direction(dir),
        None => psiv_core::Input::Neutral,
    }
}
