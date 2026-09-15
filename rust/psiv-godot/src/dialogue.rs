//! The dialogue window: the cartridge's message box, assembled from the pack.
//!
//! Two halves, deliberately separated. [`TextFlow`] is the cartridge's text
//! loop with no engine types in it at all -- `RunText_CharacterLoop`'s column
//! and line counters, its wrap, its interrupt, its page ends -- so it can be
//! tested against the extractor's own pagination for all 2,736 retail entries.
//! [`DialogueWindow`] is the Node2D that draws whatever the flow is showing:
//! frame tiles, glyphs, portrait, and the animation that opens the box.
//!
//! Nothing here invents presentation. The frame is nine tiles laid out by
//! `window.json`'s geometry rule, the text is the pack's glyph strip at 32
//! characters on two lines, and the box sits where `text_window.rect` says
//! (272x48 at (24, 160) of the Genesis's 320x224 frame). The waiting sprite
//! is the pack's byte-verified retail `ArtNem_Font` slice, not a guessed
//! triangle; its screen position remains the text-side `scroll_arrow` record.
//!
//! `TextFlow` is game logic living in the presentation crate, which is the
//! wrong side of the line in docs/RUNTIME_DESIGN.md. It is here because
//! `psiv-core` has no dialogue module yet; when one lands, this half moves
//! there unchanged and the node keeps calling it.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::{CHARS_PER_LINE, DialogueEntry, DialogueSet, LINES_PER_WINDOW, PageEnd, Role};

/// The Genesis's visible frame. The pack states window positions in it, so a
/// wider viewport keeps the box's margins rather than its coordinates.
const SCREEN: (f32, f32) = (320.0, 224.0);

/// `TextBufferToPlane` writes the 32x4 text map at screen tile (4,21).
const RETAIL_TEXT_ORIGIN: Vector2 = Vector2::new(8.0, 8.0);
const RETAIL_TEXT_LINE_PITCH: f32 = 16.0;
/// `WinGroup_Event` record 2 is at (3,14); normal dialogue uses (5,13).
/// The pack stores the common talk rect, so scene dialogue applies this
/// measured event-mode delta at draw time.
const RETAIL_SCENE_PORTRAIT_OFFSET: Vector2 = Vector2::new(-16.0, 8.0);

/// Above the field, above the party, above anything a later overlay adds.
const Z_INDEX: i32 = 1000;

mod choice;
mod text_flow;
pub use text_flow::{DialogueAction, Opening, TextFlow};

// The pack, made drawable
// ---------------------------------------------------------------------------

/// The dialogue pack's art and metrics, copied out of [`DialogueSet`] so the
/// node never borrows it -- the same shape as `SheetView` in `lib.rs`.
struct WindowView {
    /// The nine roles, pre-flipped: the plane word's H/V flips are baked in at
    /// load, so drawing is one blit per cell with no flip flags to get wrong.
    tiles: BTreeMap<&'static str, Gd<ImageTexture>>,
    font: Gd<ImageTexture>,
    /// Character -> the top-left of its cell in the strip.
    glyph_at: HashMap<char, Vector2>,
    glyph: Vector2,
    cell: f32,
    /// The whole message box, frame included.
    box_size: Vector2,
    /// Retail `TextBufferToPlane` origin and the two 8x16 glyph row pitch.
    text_origin: Vector2,
    text_line_pitch: f32,
    /// Where the portrait goes, relative to the box's top-left corner.
    portrait_offset: Vector2,
    portrait_size: Vector2,
    /// Where the scroll arrow goes, relative to the box's top-left corner.
    arrow_offset: Vector2,
    /// The extracted 2x1 hardware sprite.
    arrow: Gd<ImageTexture>,
    arrow_size: Vector2,
    /// Margins of the box inside the Genesis frame, kept when the viewport is
    /// wider or taller than 320x224.
    bottom_margin: f32,
    /// Cells the frame grows per animation step.
    step_cells: i32,
    /// Portrait id -> its PNG, pack-root-relative.
    portrait_pngs: BTreeMap<u8, String>,
}

impl WindowView {
    fn build(pack_dir: &str, set: &DialogueSet) -> Option<WindowView> {
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

        let text = &set.window.text_window;
        let portrait = &set.window.portrait_window;
        let arrow = &set.trees.window.scroll_arrow;
        let arrow_texture = ImageTexture::create_from_image(&load_image(pack_dir, &arrow.png)?)?;

        Some(WindowView {
            tiles,
            font,
            glyph_at,
            glyph: Vector2::new(text.glyph_width as f32, text.glyph_height as f32),
            cell: set.window.geometry.cell_pixels as f32,
            box_size: Vector2::new(text.rect.width as f32, text.rect.height as f32),
            text_origin: RETAIL_TEXT_ORIGIN,
            text_line_pitch: RETAIL_TEXT_LINE_PITCH,
            portrait_offset: Vector2::new(
                (portrait.rect.x - text.rect.x) as f32,
                (portrait.rect.y - text.rect.y) as f32,
            ),
            portrait_size: Vector2::new(portrait.rect.width as f32, portrait.rect.height as f32),
            arrow_offset: Vector2::new(
                (arrow.screen_x - text.rect.x) as f32,
                (arrow.screen_y - text.rect.y) as f32,
            ),
            arrow: arrow_texture,
            arrow_size: Vector2::new(arrow.width as f32, arrow.height as f32),
            bottom_margin: SCREEN.1 - (text.rect.y + text.rect.height) as f32,
            step_cells: set.window.geometry.open_animation.step_cells as i32,
            portrait_pngs: set
                .portraits
                .portraits
                .iter()
                .map(|p| (p.id, p.png.clone()))
                .collect(),
        })
    }

    fn tile(&self, role: Role) -> Option<&Gd<ImageTexture>> {
        self.tiles.get(role.as_str())
    }

    /// The box in cells, frame included.
    fn cells(&self) -> (i32, i32) {
        (
            (self.box_size.x / self.cell) as i32,
            (self.box_size.y / self.cell) as i32,
        )
    }
}

/// One blit: a texture, where it goes, and which part of it to use.
struct Quad {
    texture: Gd<ImageTexture>,
    dest: Rect2,
    src: Rect2,
}

/// One frame of the window, in local coordinates with (0, 0) at the box's
/// top-left corner. Built before anything touches the base node, because gdext
/// will not lend out the base while `self` is borrowed.
struct DrawList {
    /// The interior, tiled with the fill cell. Drawn first: the glyph strip is
    /// transparent where the cartridge fills colour index $E.
    fill: Option<(Gd<ImageTexture>, Rect2)>,
    /// Frame cells, then glyphs, then the portrait.
    quads: Vec<Quad>,
    /// The retail scroll-arrow sprite, when the page is waiting.
    arrow: Option<Quad>,
}

// ---------------------------------------------------------------------------
// The node
// ---------------------------------------------------------------------------

/// The message box. `Field` owns one, opens it on an interaction, and feeds it
/// the accept press.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct DialogueWindow {
    base: Base<Node2D>,
    view: Option<WindowView>,
    set: Option<DialogueSet>,
    portraits: HashMap<u8, Gd<ImageTexture>>,
    pack_dir: String,
    flow: Option<TextFlow>,
    /// Retail's saved text address, including its original tree binding.
    /// Map changes between dialogue chunks must not redirect the cursor.
    suspended: Option<(u8, TextFlow)>,
    choice_view: Option<choice::ChoiceView>,
    choice_cursor: usize,
    pending_choice: Option<bool>,
    standalone_choice: bool,
    /// Frame width in cells while the box is opening; equals the full width
    /// once it is open.
    open_cells: i32,
    /// The live event-flag bank, copied in by Field before each open so the
    /// `$FA` chains take their real branches.
    event_flags: Vec<bool>,
    /// A `$F6` the dialogue fired; Field forwards it to the runtime.
    pending_event: Option<u16>,
    /// The tree of the currently open dialogue, for mid-message jumps.
    current_tree: u8,
    /// Scene dialogue selects `WinGroup_Event` for its portrait window;
    /// ordinary talk keeps `WinGroup_Dialogue`.
    scene_dialogue: bool,
    cutscene_portrait: bool,
    /// Glyphs revealed on the current page. Retail draws one character every
    /// 3 frames (oracle: logs/03_npc_talk.csv, writes to Win_Tile_Buffer on a
    /// strict 3-frame cadence — 20 chars/second); this counts revealed glyphs
    /// and `reveal_tick` counts frames toward the next one.
    revealed: usize,
    reveal_tick: u8,
}

#[godot_api]
impl INode2D for DialogueWindow {
    fn init(base: Base<Node2D>) -> Self {
        DialogueWindow {
            base,
            view: None,
            set: None,
            portraits: HashMap::new(),
            pack_dir: String::new(),
            flow: None,
            suspended: None,
            choice_view: None,
            choice_cursor: 0,
            pending_choice: None,
            standalone_choice: false,
            open_cells: 0,
            event_flags: Vec::new(),
            pending_event: None,
            current_tree: 0,
            scene_dialogue: false,
            cutscene_portrait: false,
            revealed: 0,
            reveal_tick: 0,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(Z_INDEX);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn physics_process(&mut self, _delta: f64) {
        if self.flow.is_none() {
            return;
        }
        self.place();
        let full = self.view.as_ref().map_or(0, |view| view.cells().0);
        let mut redraw = false;
        if self.open_cells < full {
            // Oracle: the box opens in 9 frames; the pack's step_cells is
            // per SIDE, so the width grows by twice that each frame.
            let step = self.view.as_ref().map_or(2, |view| view.step_cells) * 2;
            self.open_cells = (self.open_cells + step).min(full);
            redraw = true;
        } else {
            if let Some(flow) = self.flow.as_mut() {
                // Delays only run once the box is open; the cartridge's
                // animation holds the text loop the same way.
                redraw = flow.tick();
            }
            // The typewriter: one glyph per 3 frames released, one per frame
            // while Speak is held — hold-to-accelerate, measured off hardware
            // (oracle tapes 13/15: 39 draws in a 40-frame held span vs 13
            // released; an early press inside the open animation is dropped,
            // which the swallow above already models). Page advance is
            // EDGE-triggered and acceleration is LEVEL-driven — measured
            // independently (tape 16): a held button never advances a
            // finished page (58 chars at 1/frame, then a dead stop with the
            // button down), but a fresh press with the hold maintained
            // starts the next page already accelerated. Reading the held
            // state per tick here gives exactly that pairing.
            let total = self.flow.as_ref().map_or(0, |flow| {
                flow.lines().iter().map(|l| l.chars().count()).sum()
            });
            if self.revealed < total {
                let cadence = if godot::classes::Input::singleton().is_action_pressed("ui_accept") {
                    1
                } else {
                    3
                };
                self.reveal_tick += 1;
                if self.reveal_tick >= cadence {
                    self.reveal_tick = 0;
                    self.revealed += 1;
                    redraw = true;
                }
            }
        }
        self.drain_log();
        self.service_flow_signals();
        if redraw {
            self.sync_portrait();
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let Some(list) = self.draw_list() else {
            return;
        };
        if let Some((texture, rect)) = list.fill {
            self.base_mut().draw_texture_rect(&texture, rect, true);
        }
        for quad in list.quads {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
        if let Some(quad) = list.arrow {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
    }
}

impl DialogueWindow {
    /// Hands the window the pack. Until this is called it can only complain.
    pub fn configure(&mut self, pack_dir: &str, set: DialogueSet) {
        self.pack_dir = pack_dir.to_owned();
        self.choice_view = choice::ChoiceView::build(pack_dir, &set);
        match WindowView::build(pack_dir, &set) {
            Some(view) => self.view = Some(view),
            None => godot_error!("dialogue: window or font art failed to load from {pack_dir}"),
        }
        self.set = Some(set);
    }

    /// Opens an NPC's line: the map's dialogue tree (1-based) and the object's
    /// `dialogue_id`. Returns whether a window actually opened.
    pub fn open_dialogue(&mut self, tree: u8, dialogue_id: u16) -> bool {
        self.cutscene_portrait = false;
        self.open_dialogue_with_mode(tree, dialogue_id, false)
    }

    /// Opens a scene-owned line, preserving F7 pauses regardless of whether
    /// the original flags select the field or panel portrait position.
    pub fn open_scene_dialogue(&mut self, tree: u8, dialogue_id: u16, panel_layout: bool) -> bool {
        self.cutscene_portrait = panel_layout;
        self.open_dialogue_with_mode(tree, dialogue_id, true)
    }

    fn open_dialogue_with_mode(
        &mut self,
        tree: u8,
        dialogue_id: u16,
        scene_dialogue: bool,
    ) -> bool {
        self.suspended = None;
        self.standalone_choice = false;
        let Some(set) = self.set.as_ref() else {
            godot_error!("dialogue: no pack loaded; call configure() first");
            return false;
        };
        // Follow `$FA` preamble jumps against the live flags, bounded so a
        // cyclic chain (a data bug) cannot hang.
        self.current_tree = tree;
        self.scene_dialogue = scene_dialogue;
        let mut id = dialogue_id;
        for _ in 0..16 {
            let Some(entry) = set.entry(tree, id) else {
                godot_error!("dialogue: tree {tree} has no entry {id}");
                return false;
            };
            match TextFlow::open_with_flags(entry, &self.event_flags) {
                Opening::Jump(next) => id = next,
                opening => return self.start(opening, &format!("tree {tree} entry {id}")),
            }
        }
        godot_error!("dialogue: tree {tree} entry {dialogue_id}: preamble jump chain too deep");
        false
    }

    /// Opens an entry the caller already resolved.
    pub fn open(&mut self, entry: &DialogueEntry) -> bool {
        self.suspended = None;
        self.scene_dialogue = false;
        let opening = TextFlow::open_with_flags(entry, &self.event_flags);
        if let Opening::Jump(next) = opening {
            // System messages never jump; a jump here means a caller fed a
            // tree entry through the pre-resolved path.
            godot_error!("dialogue: pre-resolved entry {} jumps to {next}", entry.id);
            return false;
        }
        self.start(opening, &format!("entry {}", entry.id))
    }

    /// Field hands in the current event-flag bank before opening dialogue.
    pub fn set_event_flags(&mut self, flags: Vec<bool>) {
        self.event_flags = flags;
    }

    /// Resume the scene's saved stream after its movement/presentation ops.
    pub fn resume_scene_dialogue(&mut self, panel_layout: bool) -> bool {
        self.cutscene_portrait = panel_layout;
        let Some((tree, flow)) = self.suspended.take() else {
            godot_error!("dialogue: scene resume has no saved cursor");
            return false;
        };
        self.current_tree = tree;
        self.scene_dialogue = true;
        match flow.resume_scene(&self.event_flags) {
            Opening::Jump(entry) => self.open_scene_dialogue(tree, entry, self.cutscene_portrait),
            opening => self.start(opening, &format!("resumed tree {tree}")),
        }
    }

    /// A `$F6` event the dialogue fired, once. Field starts the scene.
    pub fn take_pending_event(&mut self) -> Option<u16> {
        self.pending_event.take()
    }

    /// Returns the next embedded action only after the preceding glyphs have
    /// been revealed. This is the shell-side timing gate for retail `$F2`.
    pub fn take_ready_action(&mut self) -> Option<DialogueAction> {
        let ready = self
            .flow
            .as_ref()
            .is_some_and(|flow| flow.action_ready(self.revealed));
        ready
            .then(|| self.flow.as_mut().and_then(TextFlow::take_pending_action))
            .flatten()
    }

    /// Lets the pure text loop continue after Field has applied one action.
    pub fn resume_after_action(&mut self) {
        if let Some(flow) = self.flow.as_mut() {
            flow.resume_after_action();
        }
        self.service_flow_signals();
        self.sync_portrait();
        self.base_mut().queue_redraw();
    }

    /// Applies a flag written by an embedded action to both the shell's live
    /// bank and the currently running flow.
    pub fn set_event_flag(&mut self, flag: u8) {
        let index = usize::from(flag);
        if self.event_flags.len() <= index {
            self.event_flags.resize(index + 1, false);
        }
        self.event_flags[index] = true;
        if let Some(flow) = self.flow.as_mut() {
            flow.set_event_flag(flag);
        }
    }

    /// The leader's "Nothing here" line (one per character slot).
    pub fn open_nothing_here(&mut self, character_slot: usize) -> bool {
        let Some(set) = self.set.as_ref() else {
            godot_error!("dialogue: no pack loaded; call configure() first");
            return false;
        };
        let Some(entry) = set.nothing_here(character_slot).cloned() else {
            godot_print!("dialogue: pack carries no system messages");
            return false;
        };
        self.open(&entry)
    }

    /// Displays a static field-status window using the normal frame and font.
    pub(crate) fn open_status(&mut self, lines: &[String]) -> bool {
        let mut segments = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i != 0 {
                segments.push(psiv_data::Segment::Control(psiv_data::Ctrl::Newline {
                    code: 0xFC,
                    operands: Vec::new(),
                }));
            }
            segments.push(psiv_data::Segment::Text(line.clone()));
        }
        let entry = DialogueEntry {
            id: 0,
            text: lines.join("\n"),
            segments,
            pages: Vec::new(),
        };
        let opened = self.open(&entry);
        // These windows load tile strings at once, without the dialogue typewriter.
        self.revealed = lines.iter().map(|line| line.chars().count()).sum();
        opened
    }

    /// The accept press.
    pub fn advance(&mut self) {
        if self.is_opening() {
            return;
        }
        let mut reopen = false;
        let mut closed = false;
        let mut suspended = false;
        if let Some(flow) = self.flow.as_mut() {
            reopen = flow.page_end() == Some(PageEnd::Close);
            let total: usize = flow.lines().iter().map(|l| l.chars().count()).sum();
            if self.revealed < total {
                // A press mid-typewriter neither completes the page nor
                // advances it: retail accelerates only while Speak is HELD
                // (tick's cadence), and a tap adds exactly its held frames
                // (oracle tapes 13/15 — the earlier complete-the-page
                // assumption was wrong). Swallow the press.
                return;
            }
            if flow.page_end().is_some()
                && std::env::var("PSIV_DEBUG_INPUT").is_ok_and(|value| value == "1")
            {
                godot_print!("dialogue page {:?}: {:?}", flow.page_end(), flow.lines());
            }
            suspended = self.scene_dialogue && flow.pause_for_scene();
            if !suspended {
                flow.advance();
            }
            self.revealed = 0;
            self.reveal_tick = 0;
            closed = !flow.is_open();
        }
        self.drain_log();
        if closed {
            if suspended {
                self.suspended = self.flow.take().map(|flow| (self.current_tree, flow));
            }
            self.close();
            return;
        }
        self.sync_portrait();
        if reopen {
            // $F7 destroys the window; the next page opens a new one, so the
            // animation runs again.
            self.open_cells = 0;
            self.revealed = 0;
            self.reveal_tick = 0;
        }
        self.base_mut().queue_redraw();
    }

    /// Whether a window is on screen. `Field` gates input on this.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.flow.is_some()
    }

    /// Read-only rendered page observation for native input routes.
    pub(crate) fn debug_page(&self) -> Option<serde_json::Value> {
        self.flow.as_ref().map(|flow| {
            serde_json::json!({
                "tree": self.current_tree,
                "lines": flow.lines(),
                "end": format!("{:?}", flow.page_end()),
                "ready": self.is_dismissable(),
            })
        })
    }

    pub(crate) fn has_suspended_scene_dialogue(&self) -> bool {
        self.suspended.is_some()
    }

    /// Whether the arrow is up and the window is waiting to be advanced.
    #[must_use]
    #[allow(dead_code)] // For a renderer that styles the waiting state itself.
    pub fn is_waiting(&self) -> bool {
        self.flow.as_ref().is_some_and(TextFlow::is_waiting)
    }

    /// Whether an accept press would advance or dismiss the current page,
    /// including the entry's final `End` page. The retail-pace harness uses
    /// this; the arrow keeps using [`DialogueWindow::is_waiting`].
    pub fn is_dismissable(&self) -> bool {
        // The flow reaches its page end before the typewriter has revealed
        // the glyphs; the retail-pace hold must not start (or spam no-op
        // advances, each of which adds accelerated frames) until the page is
        // actually on screen.
        let total: usize = self.flow.as_ref().map_or(0, |flow| {
            flow.lines().iter().map(|l| l.chars().count()).sum()
        });
        self.revealed >= total
            && !self.is_opening()
            && self.flow.as_ref().is_some_and(TextFlow::is_dismissable)
    }

    fn is_opening(&self) -> bool {
        self.view
            .as_ref()
            .is_some_and(|view| self.open_cells < view.cells().0)
    }

    /// Applies mid-message `$FA` jumps and `$F6` events the flow raised.
    fn service_flow_signals(&mut self) {
        for _ in 0..16 {
            let (jump, event) = match self.flow.as_mut() {
                Some(flow) => (flow.take_jump(), flow.take_event()),
                None => (None, None),
            };
            if let Some(event) = event {
                self.pending_event = Some(event);
                self.close();
                return;
            }
            let Some(next) = jump else {
                return;
            };
            let entry = self
                .set
                .as_ref()
                .and_then(|set| set.entry(self.current_tree, next));
            match (self.flow.as_mut(), entry) {
                (Some(flow), Some(entry)) => flow.continue_at(entry),
                _ => {
                    godot_error!(
                        "dialogue: missing branch entry {next} in tree {}",
                        self.current_tree
                    );
                    self.close();
                    return;
                }
            }
        }
        godot_error!("dialogue: in-stream branch chain too deep");
        self.close();
    }

    fn start(&mut self, opening: Opening, who: &str) -> bool {
        match opening {
            Opening::Jump(next) => {
                godot_error!("dialogue: {who}: unresolved jump to {next} reached start()");
                false
            }
            Opening::Event(event) => {
                godot_print!("dialogue: {who} fires event {event:#x}");
                self.pending_event = Some(event);
                false
            }
            Opening::Silent => {
                godot_print!("dialogue: {who} is empty; nothing to show");
                false
            }
            Opening::Window(flow) => {
                self.flow = Some(*flow);
                self.open_cells = 0;
                self.revealed = 0;
                self.reveal_tick = 0;
                self.choice_cursor = 0;
                self.drain_log();
                self.sync_portrait();
                self.place();
                self.base_mut().set_visible(true);
                self.base_mut().queue_redraw();
                true
            }
        }
    }

    fn close(&mut self) {
        self.flow = None;
        self.open_cells = 0;
        self.scene_dialogue = false;
        self.base_mut().set_visible(false);
        self.base_mut().queue_redraw();
    }

    /// Loads whatever portrait the flow is asking for.
    fn sync_portrait(&mut self) {
        if let Some(id) = self.flow.as_ref().and_then(TextFlow::portrait) {
            self.cache_portrait(id);
        }
    }

    fn drain_log(&mut self) {
        let lines = self
            .flow
            .as_mut()
            .map(TextFlow::drain_log)
            .unwrap_or_default();
        for line in lines {
            godot_print!("dialogue: {line}");
        }
    }

    /// Puts the node where the box belongs on screen: the pack's position in
    /// the Genesis frame. The 320x224 retail surface is centred in whatever
    /// viewport the renderer has (the same rule the cutscene panel layer
    /// uses), and the box sits at its retail coordinates inside that surface
    /// — anchoring to the viewport instead drifts the box downward whenever
    /// the viewport is taller than the 3x surface, which the oracle pairs
    /// caught as a ~21-pixel error.
    fn place(&mut self) {
        let Some(size) = self.view.as_ref().map(|view| view.box_size) else {
            return;
        };
        let margin = self.view.as_ref().map_or(0.0, |view| view.bottom_margin);
        let viewport = self.base().get_viewport_rect();
        let canvas = self.base().get_canvas_transform().affine_inverse();
        let top_left = canvas * viewport.position;
        let bottom_right = canvas * (viewport.position + viewport.size);
        let visible = bottom_right - top_left;
        let surface_top = top_left.y + (visible.y - SCREEN.1) / 2.0;
        // The window plane carries the same one-pixel horizontal origin
        // residue the scene panels do (oracle receipt: the settled
        // MeetingRika window matches at a +1 X shift, rows 176..208
        // dropping 62→13 RMSE; Y needs none).
        let position = Vector2::new(
            (top_left.x + (visible.x - size.x) / 2.0).floor() + 1.0,
            (surface_top + SCREEN.1 - margin - size.y).floor(),
        );
        self.base_mut().set_position(position);
    }

    /// Everything to draw this frame.
    fn draw_list(&self) -> Option<DrawList> {
        let view = self.view.as_ref()?;
        let flow = self.flow.as_ref()?;
        let (full_cells, height_cells) = view.cells();
        let width_cells = self.open_cells.clamp(0, full_cells);
        if width_cells < 2 {
            return None;
        }
        let cell = view.cell;
        // The frame grows from the centre, so a partial box is inset by half
        // the cells it is still missing.
        let left = ((full_cells - width_cells) as f32 / 2.0).floor() * cell;

        let mut quads = Vec::new();
        let mut blit = |texture: &Gd<ImageTexture>, x: f32, y: f32| {
            quads.push(Quad {
                texture: texture.clone(),
                dest: Rect2::new(Vector2::new(x, y), Vector2::new(cell, cell)),
                src: Rect2::new(Vector2::ZERO, Vector2::new(cell, cell)),
            });
        };

        // corner, W-2 edge, corner; H-2 rows of edge, fill, edge; and again.
        let bottom = (height_cells - 1) as f32 * cell;
        let right = (width_cells - 1) as f32 * cell + left;
        blit(view.tile(Role::CornerTopLeft)?, left, 0.0);
        blit(view.tile(Role::CornerTopRight)?, right, 0.0);
        blit(view.tile(Role::CornerBottomLeft)?, left, bottom);
        blit(view.tile(Role::CornerBottomRight)?, right, bottom);
        for column in 1..width_cells - 1 {
            let x = left + column as f32 * cell;
            blit(view.tile(Role::EdgeTop)?, x, 0.0);
            blit(view.tile(Role::EdgeBottom)?, x, bottom);
        }
        for row in 1..height_cells - 1 {
            let y = row as f32 * cell;
            blit(view.tile(Role::EdgeLeft)?, left, y);
            blit(view.tile(Role::EdgeRight)?, right, y);
        }

        let interior = Rect2::new(
            Vector2::new(left + cell, cell),
            Vector2::new(
                (width_cells - 2) as f32 * cell,
                (height_cells - 2) as f32 * cell,
            ),
        );
        let fill = view
            .tile(Role::Fill)
            .map(|texture| (texture.clone(), interior));

        let mut arrow = None;
        if width_cells == full_cells {
            // These are the retail `TextBufferToPlane` coordinates, not a
            // generic centre-in-window guess: screen tile (4,21), 8x16
            // glyphs, and a 16-pixel line pitch.
            let origin = view.text_origin;
            let mut budget = self.revealed;
            'lines: for (row, line) in flow.lines().iter().enumerate().take(LINES_PER_WINDOW) {
                for (column, ch) in line.chars().enumerate().take(CHARS_PER_LINE) {
                    if budget == 0 {
                        break 'lines;
                    }
                    budget -= 1;
                    let Some(at) = view.glyph_at.get(&ch) else {
                        // Load-time validation proves every retail character
                        // has a glyph, so this is a pack defect if it happens.
                        godot_error!("dialogue: no glyph for {ch:?}");
                        continue;
                    };
                    quads.push(Quad {
                        texture: view.font.clone(),
                        dest: Rect2::new(
                            origin
                                + Vector2::new(
                                    column as f32 * view.glyph.x,
                                    row as f32 * view.text_line_pitch,
                                ),
                            view.glyph,
                        ),
                        src: Rect2::new(*at, view.glyph),
                    });
                }
            }

            if let Some(id) = flow.portrait()
                && let Some(texture) = self.portraits.get(&id)
            {
                // The portrait art covers its own frame completely: the PNGs
                // carry the border, so no chrome is drawn under them.
                // The portrait rides the window plane: the box's own +1 X
                // origin already applies through the node position, and
                // adding the scene-plane (1,1) residue on top double-shifts
                // it (frame-7250 receipt: correlation wants exactly (-1,-1)
                // back).
                quads.push(Quad {
                    texture: texture.clone(),
                    dest: Rect2::new(
                        view.portrait_offset
                            + if self.scene_dialogue && self.cutscene_portrait {
                                RETAIL_SCENE_PORTRAIT_OFFSET
                            } else {
                                Vector2::ZERO
                            },
                        view.portrait_size,
                    ),
                    src: Rect2::new(Vector2::ZERO, view.portrait_size),
                });
            }

            let total: usize = flow.lines().iter().map(|l| l.chars().count()).sum();
            if flow.is_waiting() && self.revealed >= total {
                arrow = Some(Quad {
                    texture: view.arrow.clone(),
                    dest: Rect2::new(view.arrow_offset, view.arrow_size),
                    src: Rect2::new(Vector2::ZERO, view.arrow_size),
                });
            }
        }

        if self.choice_ready()
            && let Some(choice) = self.choice_view.as_ref()
        {
            quads.extend(choice.quads(view, self.choice_cursor));
        }
        Some(DrawList { fill, quads, arrow })
    }

    /// Loads the portraits an entry needs. Called when a window opens, so the
    /// 39 PNGs are never all in memory at once.
    fn cache_portrait(&mut self, id: u8) {
        if self.portraits.contains_key(&id) {
            return;
        }
        let Some(png) = self
            .view
            .as_ref()
            .and_then(|view| view.portrait_pngs.get(&id))
            .cloned()
        else {
            godot_error!("dialogue: portrait {id} is not in the pack");
            return;
        };
        match load_image(&self.pack_dir, &png)
            .and_then(|image| ImageTexture::create_from_image(&image))
        {
            Some(texture) => {
                self.portraits.insert(id, texture);
            }
            None => godot_error!(
                "dialogue: portrait art {}/{png} failed to load",
                self.pack_dir
            ),
        }
    }
}

/// Loads one of the pack's PNGs. The pack names its own files; this never
/// invents a filename.
fn load_image(pack_dir: &str, png: &str) -> Option<Gd<Image>> {
    let path = format!("{pack_dir}/{png}");
    Image::load_from_file(&GString::from(path.as_str()))
}

#[cfg(test)]
mod tests;
