//! The Godot presentation layer: a thin gdext bridge over `psiv-runtime`.
//!
//! This crate owns pixels and input, zero game rules. Floats are legal here —
//! they exist only between the engine's integer state and the screen.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{
    Camera2D, INode2D, Image, ImageTexture, Input, Node2D, ProjectSettings, Sprite2D,
};
use godot::prelude::*;

use psiv_core::{Cell, Direction, StepFrames, WarpTrigger};
use psiv_data::{GameData, Sheet};
use psiv_runtime::{Runtime, RuntimeEvent};

struct PsivExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PsivExtension {}

const CELL_PIXELS: f32 = 16.0;
/// Piata, one cell below the academy doors — the golden test's spawn.
const SPAWN_MAP: u16 = 0x010;
const SPAWN_CELL: (u16, u16) = (31, 8);

/// A sheet made drawable: its texture plus the geometry and sequences the
/// pack declares. Copied out of `psiv-data` so nodes never borrow `GameData`.
struct SheetView {
    texture: Gd<ImageTexture>,
    frame_width: i32,
    frame_height: i32,
    origin_x: i32,
    origin_y: i32,
    /// name -> (frames as (index, duration_ticks), total duration)
    sequences: BTreeMap<String, (Vec<(i32, u32)>, u32)>,
}

impl SheetView {
    fn build(pack_dir: &str, sheet: &Sheet) -> Option<SheetView> {
        let path = format!("{pack_dir}/{}", sheet.png);
        let image = Image::load_from_file(&GString::from(path.as_str()))?;
        let texture = ImageTexture::create_from_image(&image)?;
        let mut sequences = BTreeMap::new();
        for (name, sequence) in &sheet.sequences {
            let frames: Vec<(i32, u32)> = sequence
                .frames
                .iter()
                .map(|f| (f.index as i32, f.duration_ticks))
                .collect();
            let total: u32 = frames.iter().map(|(_, d)| d).sum();
            sequences.insert(name.clone(), (frames, total.max(1)));
        }
        Some(SheetView {
            texture,
            frame_width: sheet.frame_width as i32,
            frame_height: sheet.frame_height as i32,
            origin_x: sheet.origin_x,
            origin_y: sheet.origin_y,
            sequences,
        })
    }

    /// The strip frame index for `sequence` at animation tick `tick`.
    fn frame_at(&self, sequence: &str, tick: u64) -> i32 {
        let Some((frames, total)) = self.sequences.get(sequence) else {
            return 0;
        };
        let mut remaining = (tick % u64::from(*total)) as u32;
        for (index, duration) in frames {
            if remaining < *duration {
                return *index;
            }
            remaining -= duration;
        }
        frames.last().map_or(0, |(index, _)| *index)
    }

    /// Configures a sprite node to show one frame of this strip.
    fn apply(&self, sprite: &mut Gd<Sprite2D>, frame: i32) {
        sprite.set_texture(&self.texture);
        sprite.set_region_enabled(true);
        sprite.set_region_rect(Rect2::new(
            Vector2::new((frame * self.frame_width) as f32, 0.0),
            Vector2::new(self.frame_width as f32, self.frame_height as f32),
        ));
    }

    /// Where the frame's top-left goes for an entity occupying `cell`.
    ///
    /// The cartridge's character position sits one cell above the occupied
    /// cell (the standing-cell shift), and `origin` is where that position
    /// lands inside the frame.
    fn draw_pos(&self, cell: Cell, offset: (i32, i32)) -> Vector2 {
        let x = f32::from(cell.x) * CELL_PIXELS - self.origin_x as f32 + offset.0 as f32;
        let y = (f32::from(cell.y) - 1.0) * CELL_PIXELS - self.origin_y as f32 + offset.1 as f32;
        Vector2::new(x, y)
    }
}

fn sequence_name(kind: &str, facing: Direction) -> String {
    let dir = match facing {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    };
    format!("{kind}_{dir}")
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
    /// (node, sheet id, sequence names) per visible NPC on the current map.
    npc_nodes: Vec<(Gd<Sprite2D>, String, String, String)>,
    sheet_views: HashMap<String, SheetView>,
    camera: Option<Gd<Camera2D>>,
    anim_tick: u64,
    /// The party's active sequence and the tick it started, so animation
    /// phase restarts at frame 0 on a sequence change — matching
    /// `FieldObj_Move`'s reset-to-frame-0 rather than free-phase modulo.
    party_sequence: String,
    party_seq_start: u64,
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
            npc_nodes: Vec::new(),
            sheet_views: HashMap::new(),
            camera: None,
            anim_tick: 0,
            party_sequence: String::new(),
            party_seq_start: 0,
        }
    }

    fn ready(&mut self) {
        self.pack_dir = ProjectSettings::singleton()
            .globalize_path("res://../runtime-pack")
            .to_string();
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

        let mut overlay_sprite = Sprite2D::new_alloc();
        overlay_sprite.set_centered(false);
        overlay_sprite.set_z_index(20);
        self.base_mut().add_child(&overlay_sprite);
        self.overlay_sprite = Some(overlay_sprite);

        let mut party = Sprite2D::new_alloc();
        party.set_centered(false);
        party.set_z_index(10);
        self.base_mut().add_child(&party);
        self.party = Some(party);

        let mut camera = Camera2D::new_alloc();
        camera.set_zoom(Vector2::new(3.0, 3.0));
        self.base_mut().add_child(&camera);
        camera.make_current();
        self.camera = Some(camera);

        self.runtime = Some(runtime);
        self.load_map_visuals();
        self.sync_visuals(false);
        godot_print!(
            "PSIV field ready: map {:#05x}, party at ({}, {})",
            SPAWN_MAP,
            SPAWN_CELL.0,
            SPAWN_CELL.1
        );
    }

    fn physics_process(&mut self, _delta: f64) {
        self.anim_tick += 1;
        let input = read_input();
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        let events = runtime.tick(input);
        let mut stepped = false;
        for event in events {
            match event {
                RuntimeEvent::StepCompleted { .. } => stepped = true,
                RuntimeEvent::MapChanged { map, trigger } => {
                    let kind = match trigger {
                        WarpTrigger::MapChange => "doorway",
                        WarpTrigger::NormalGround => "ground",
                    };
                    godot_print!("map change ({kind}) -> {:#05x}", map.0);
                    self.load_map_visuals();
                }
                RuntimeEvent::UnpackedTarget { map } => {
                    godot_error!("transition target {:#05x} is not in the pack", map.0);
                }
                RuntimeEvent::WarpUnmapped { cell } => {
                    godot_error!("type-1 cell with no doorway record at {cell:?}");
                }
                // Dialogue windows land with the dialogue slice; until then
                // the interaction layer reports to the console.
                RuntimeEvent::Interact { npc_index, cell } => {
                    godot_print!("talk: npc {npc_index} at {cell:?}");
                }
                RuntimeEvent::InteractNothing { .. } => {
                    godot_print!("talk: nothing here");
                }
            }
        }
        // A landing tick with the key still held is mid-stride, not rest:
        // without this, the idle frame flashes for one tick every step (the
        // cartridge's animation free-runs and never sees such a gap).
        let walking = {
            let state = self.runtime.as_ref().map(|rt| rt.state());
            state.is_some_and(|s| s.is_stepping()) || (stepped && input.direction().is_some())
        };
        self.sync_visuals(walking);
    }
}

impl Field {
    /// Loads the current map's PNG and rebuilds NPC sprites.
    fn load_map_visuals(&mut self) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let id = runtime.map_id().0;

        match runtime.map_png().map(str::to_owned) {
            Some(name) => {
                let path = format!("{}/{name}", self.pack_dir);
                match Image::load_from_file(&GString::from(path.as_str())) {
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
            None => godot_error!("map {id:#05x} has no png declared in the pack"),
        }

        // The priority overlay: absent on the 22 maps with no priority tiles.
        let over = runtime.map_png_over().map(str::to_owned);
        if let Some(sprite) = self.overlay_sprite.as_mut() {
            match over {
                Some(name) => {
                    let path = format!("{}/{name}", self.pack_dir);
                    match Image::load_from_file(&GString::from(path.as_str()))
                        .and_then(|image| ImageTexture::create_from_image(&image))
                    {
                        Some(texture) => {
                            sprite.set_texture(&texture);
                            sprite.set_visible(true);
                        }
                        None => godot_error!("could not load overlay {path}"),
                    }
                }
                None => sprite.set_visible(false),
            }
        }

        for (node, ..) in &mut self.npc_nodes {
            node.queue_free();
        }
        self.npc_nodes.clear();

        // Gather NPC draw info first; borrowing data and adding children at
        // the same time fights the base borrow.
        struct NpcDraw {
            sheet: String,
            idle: String,
            walk: String,
            // Object pixel coordinates, not cells: 85 retail objects sit on
            // half-cells (8px-scaled words), so x/y_pixels are authoritative.
            x: i32,
            y: i32,
        }
        let mut draws: Vec<NpcDraw> = Vec::new();
        if let Some(record) = runtime.map_record() {
            for npc in &record.npcs {
                // Invisible triggers (sprite_reason set) still block in the
                // engine, exactly like the cartridge's invisible objects, but
                // draw nothing.
                let Some(sprite) = &npc.sprite else { continue };
                draws.push(NpcDraw {
                    sheet: sprite.sheet.clone(),
                    idle: sprite.idle_sequence.clone(),
                    walk: sprite.walk_sequence.clone(),
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
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(5);
            let frame = view.frame_at(&draw.idle, 0);
            view.apply(&mut node, frame);
            // "Draw a frame at (object_x - origin_x, object_y - origin_y) and
            // it lands exactly where the VDP would put it."
            node.set_position(Vector2::new(
                (draw.x - view.origin_x) as f32,
                (draw.y - view.origin_y) as f32,
            ));
            self.base_mut().add_child(&node);
            self.npc_nodes.push((node, draw.sheet, draw.idle, draw.walk));
        }
    }

    /// Places and animates the party sprite, animates NPCs, moves the camera.
    fn sync_visuals(&mut self, walking: bool) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let state = runtime.state();
        let cell = state.cell();
        let offset = state.render_offset_16ths();
        let kind = if walking { "walk" } else { "idle" };
        let sequence = sequence_name(kind, state.facing());
        if sequence != self.party_sequence {
            self.party_sequence = sequence;
            self.party_seq_start = self.anim_tick;
        }

        if let (Some(party), Some(view)) = (self.party.as_mut(), self.party_view.as_ref()) {
            let frame = view.frame_at(&self.party_sequence, self.anim_tick - self.party_seq_start);
            view.apply(party, frame);
            party.set_position(view.draw_pos(cell, offset));
        }

        for (node, sheet_id, idle, _walk) in &mut self.npc_nodes {
            if let Some(view) = self.sheet_views.get(sheet_id) {
                let frame = view.frame_at(idle, self.anim_tick);
                view.apply(node, frame);
            }
        }

        if let Some(camera) = self.camera.as_mut() {
            let center = Vector2::new(
                f32::from(cell.x) * CELL_PIXELS + offset.0 as f32 + CELL_PIXELS / 2.0,
                f32::from(cell.y) * CELL_PIXELS + offset.1 as f32 + CELL_PIXELS / 2.0,
            );
            camera.set_position(center);
        }
    }
}

/// One input per tick. Confirm wins over movement — the cartridge reads them
/// on separate paths and the talk takes the frame; a still-held direction
/// simply walks on the next tick. The engine edge-detects Action internally.
fn read_input() -> psiv_core::Input {
    let input = Input::singleton();
    if input.is_action_pressed("ui_accept") {
        return psiv_core::Input::Action;
    }
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
