//! The Godot presentation layer: a thin gdext bridge over `psiv-runtime`.
//!
//! This crate owns pixels and input, zero game rules. Floats are legal here —
//! they exist only between the engine's integer state and the screen.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{
    Camera2D, INode2D, Image, ImageTexture, Input, Node2D, ProjectSettings, Sprite2D,
};
use godot::prelude::*;

mod dialogue;
use dialogue::DialogueWindow;

use psiv_core::{Cell, Direction, StepFrames, WarpTrigger};
use psiv_data::{GameData, Sheet};
use psiv_runtime::{Runtime, RuntimeEvent};

struct PsivExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PsivExtension {}

const CELL_PIXELS: f32 = 16.0;
/// Fallback spawn when the pack predates game-start extraction: Piata, one
/// cell below the academy doors (the golden test's spawn).
const FALLBACK_SPAWN_MAP: u16 = 0x010;
const FALLBACK_SPAWN_CELL: (u16, u16) = (31, 8);

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
        // Drawn above the node origin so the origin is the feet line and
        // y-sort orders characters the way the hardware did.
        sprite.set_offset(Vector2::new(0.0, -(self.frame_height as f32)));
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
        // Node origin sits at the feet; apply() draws the frame above it.
        Vector2::new(x, y + self.frame_height as f32)
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
    /// (node, sheet id, idle sequence, walk sequence, map-order npc index)
    /// per visible NPC on the current map.
    npc_nodes: Vec<(Gd<Sprite2D>, String, String, String, usize)>,
    /// Follower sprites (party members after the leader), created on demand.
    /// (node, active sequence, sequence start tick) per follower.
    follower_nodes: Vec<(Gd<Sprite2D>, String, u64)>,
    sheet_views: HashMap<String, SheetView>,
    camera: Option<Gd<Camera2D>>,
    dialogue: Option<Gd<DialogueWindow>>,
    anim_tick: u64,
    /// Set while a dialogue is open and until accept is released after it
    /// closes — the press that dismisses a window must not immediately
    /// re-open it (the engine's own press latch resets while we starve it
    /// with Neutral, so the still-held key would read as a fresh press).
    accept_blocked: bool,
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
            follower_nodes: Vec::new(),
            sheet_views: HashMap::new(),
            camera: None,
            dialogue: None,
            anim_tick: 0,
            accept_blocked: false,
            party_sequence: String::new(),
            party_seq_start: 0,
        }
    }

    fn ready(&mut self) {
        // Characters sort by their feet line, like the hardware's sprite
        // ordering: standing north of an NPC puts you behind them.
        self.base_mut().set_y_sort_enabled(true);
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
        let runtime = match Runtime::new(
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
        party.set_z_index(5);
        self.base_mut().add_child(&party);
        self.party = Some(party);

        let mut camera = Camera2D::new_alloc();
        camera.set_zoom(Vector2::new(3.0, 3.0));
        self.base_mut().add_child(&camera);
        camera.make_current();
        self.camera = Some(camera);

        let mut window = DialogueWindow::new_alloc();
        match psiv_data::DialogueSet::load(std::path::Path::new(&self.pack_dir)) {
            Ok(set) => window.bind_mut().configure(&self.pack_dir, set),
            Err(e) => godot_error!("dialogue pack failed to load: {e}"),
        }
        window.set_z_index(30);
        self.base_mut().add_child(&window);
        self.dialogue = Some(window);

        self.runtime = Some(runtime);
        self.load_map_visuals();
        self.sync_visuals(false);
        godot_print!(
            "PSIV field ready: map {:#05x}, party at ({}, {})",
            spawn_map,
            spawn_cell.x,
            spawn_cell.y
        );
    }

    fn physics_process(&mut self, _delta: f64) {
        self.anim_tick += 1;

        // An open dialogue owns the input: accept advances the window, the
        // engine gets Neutral (the cartridge swaps Game_Mode_Routine to
        // FieldRoutine_Interaction; we model it by starving the field of
        // input, per the engine's documented non-modal contract).
        if self.dialogue.as_ref().is_some_and(|w| w.bind().is_open()) {
            self.accept_blocked = true;
            if Input::singleton().is_action_just_pressed("ui_accept")
                && let Some(window) = self.dialogue.as_mut()
            {
                window.bind_mut().advance();
            }
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.tick(psiv_core::Input::Neutral);
            }
            self.sync_visuals(false);
            return;
        }

        let mut input = read_input();
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
                RuntimeEvent::Interact {
                    npc_index,
                    cell,
                    reach,
                } => {
                    if matches!(reach, psiv_core::InteractReach::AcrossCounter) {
                        // Shop-vs-dialogue splits on the shop-location table;
                        // until the shop UI exists, counters open dialogue,
                        // which is also the correct behavior for desks (the
                        // principal is not in the shop table).
                        godot_print!(
                            "counter reach at {cell:?} (shop-table check pending shop UI)"
                        );
                    }
                    let binding = self.runtime.as_ref().and_then(|rt| {
                        let record = rt.map_record()?;
                        // Trees are 1-based; 0 means the map binds none.
                        let tree = match record.dialogue_tree {
                            0 => return None,
                            tree => tree,
                        };
                        let id = record.npcs.get(npc_index)?.dialogue_id;
                        Some((tree, id))
                    });
                    match binding {
                        Some((tree, id)) => {
                            let opened = self
                                .dialogue
                                .as_mut()
                                .is_some_and(|w| w.bind_mut().open_dialogue(tree, id));
                            if opened {
                                // NPCs turn to face the speaker; the \$F3
                                // control code exists precisely to suppress
                                // this, which proves it is the default.
                                let toward = self
                                    .runtime
                                    .as_ref()
                                    .map(|rt| rt.state().facing().opposite());
                                if let Some(toward) = toward {
                                    let name = sequence_name("idle", toward);
                                    for (_, _, idle, _, index) in &mut self.npc_nodes {
                                        if *index == npc_index {
                                            *idle = name.clone();
                                        }
                                    }
                                }
                            }
                        }
                        None => godot_print!(
                            "talk: npc {npc_index} at {cell:?} has no dialogue binding"
                        ),
                    }
                }
                RuntimeEvent::InteractNothing { .. } => {
                    // The cartridge answers with the leader's own "Nothing
                    // here" line — one per character. Slot 0 (Chaz) until
                    // game-start state supplies the real leader.
                    if let Some(window) = self.dialogue.as_mut() {
                        window.bind_mut().open_nothing_here(0);
                    }
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
                        if let Some(texture) = ImageTexture::create_from_image(&image)
                            && let Some(sprite) = self.map_sprite.as_mut()
                        {
                            sprite.set_texture(&texture);
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
            index: usize,
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
            for (index, npc) in record.npcs.iter().enumerate() {
                // Invisible triggers (sprite_reason set) still block in the
                // engine, exactly like the cartridge's invisible objects, but
                // draw nothing.
                let Some(sprite) = &npc.sprite else { continue };
                draws.push(NpcDraw {
                    index,
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
                (draw.y - view.origin_y + view.frame_height) as f32,
            ));
            self.base_mut().add_child(&node);
            self.npc_nodes
                .push((node, draw.sheet, draw.idle, draw.walk, draw.index));
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

        // Followers: members()[1..] walk the leader's vacated cells. Which
        // character occupies which slot comes from game-start state; until it
        // lands, slot index selects the party sheet directly (no followers
        // exist yet, so this is forward wiring, not a guess shipped).
        struct FollowerDraw {
            sheet_id: String,
            kind: &'static str,
            facing: Direction,
            cell: Cell,
            offset: (i32, i32),
        }
        let mut fdraws: Vec<Option<FollowerDraw>> = Vec::new();
        {
            let members = runtime.members();
            for (slot, member) in members.iter().enumerate().skip(1) {
                let sheet_id = runtime.data().party_sheet(slot).map(|s| s.id.clone());
                fdraws.push(sheet_id.map(|sheet_id| FollowerDraw {
                    sheet_id,
                    kind: if member.is_stepping { "walk" } else { "idle" },
                    facing: member.facing,
                    cell: member.cell,
                    offset: member.render_offset_16ths,
                }));
            }
            let missing: Vec<String> = fdraws
                .iter()
                .flatten()
                .filter(|d| !self.sheet_views.contains_key(&d.sheet_id))
                .map(|d| d.sheet_id.clone())
                .collect();
            for sheet_id in missing {
                if let Some(sheet) = runtime.data().sheet(&sheet_id)
                    && let Some(view) = SheetView::build(&self.pack_dir, sheet)
                {
                    self.sheet_views.insert(sheet_id, view);
                }
            }
        }
        while self.follower_nodes.len() < fdraws.len() {
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(5);
            self.base_mut().add_child(&node);
            self.follower_nodes.push((node, String::new(), 0));
        }
        for (idx, draw) in fdraws.iter().enumerate() {
            let Some(draw) = draw else { continue };
            let Some(view) = self.sheet_views.get(&draw.sheet_id) else {
                continue;
            };
            let (node, seq, start) = &mut self.follower_nodes[idx];
            let name = sequence_name(draw.kind, draw.facing);
            if *seq != name {
                *seq = name;
                *start = self.anim_tick;
            }
            let frame = view.frame_at(seq, self.anim_tick - *start);
            view.apply(node, frame);
            node.set_position(view.draw_pos(draw.cell, draw.offset));
        }

        for (node, sheet_id, idle, _walk, _index) in &mut self.npc_nodes {
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
