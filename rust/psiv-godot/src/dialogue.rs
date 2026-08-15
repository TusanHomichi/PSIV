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
//! (272x48 at (24, 160) of the Genesis's 320x224 frame). The two things the
//! pack does not answer -- how fast retail draws a character, and what the
//! scroll arrow's art is -- are not guessed: text appears at once and the
//! waiting indicator is an obvious placeholder triangle. Both are open
//! questions for the emulator oracle.
//!
//! `TextFlow` is game logic living in the presentation crate, which is the
//! wrong side of the line in docs/RUNTIME_DESIGN.md. It is here because
//! `psiv-core` has no dialogue module yet; when one lands, this half moves
//! there unchanged and the node keeps calling it.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::{
    CHARS_PER_LINE, Ctrl, DialogueEntry, DialogueSet, LINES_PER_WINDOW, PORTRAIT_HIDE, PageEnd,
    Role, Segment,
};

/// The Genesis's visible frame. The pack states window positions in it, so a
/// wider viewport keeps the box's margins rather than its coordinates.
const SCREEN: (f32, f32) = (320.0, 224.0);

/// Above the field, above the party, above anything a later overlay adds.
const Z_INDEX: i32 = 1000;

// ---------------------------------------------------------------------------
// The text loop
// ---------------------------------------------------------------------------

/// What opening an entry produced.
pub enum Opening {
    /// A `$FA` preamble was taken: reopen at this entry instead.
    Jump(u16),
    /// A window to show, driven by this flow.
    Window(Box<TextFlow>),
    /// The preamble was `$F6`: the entry fires an event and shows nothing.
    Event(u16),
    /// The entry has nothing to say -- an empty entry, which the cartridge's
    /// dense ids are full of.
    Silent,
}

/// `RunText_CharacterLoop`, one page at a time.
///
/// The rules are the routine's, not a reimplementation of them: the column
/// counter wraps at 32 and breaks the line by itself, the line counter is one
/// bit wide so a full second line waits for input, `$FC` is swallowed when it
/// lands exactly on a wrap, and the interrupt looks at the byte it stopped on
/// before it shows an arrow. `psiv_tools.dialogue_pack.paginate` is the same
/// walk done offline; the tests hold this against it entry by entry.
pub struct TextFlow {
    segments: Vec<Segment>,
    /// Next segment to run.
    index: usize,
    /// Where in the current text run to resume, when a page filled mid-run.
    char_offset: usize,
    lines: Vec<String>,
    line: usize,
    column: usize,
    portrait: Option<u8>,
    /// Frames left on a `$F9`.
    hold: u16,
    /// Set while a finished page is on screen.
    stop: Option<PageEnd>,
    /// No segments left to run.
    done: bool,
    /// The window is gone.
    closed: bool,
    /// What v1 could not act on, for the caller to print. Never silently
    /// dropped: the event system does not exist yet and this is the record of
    /// everything that will need it.
    log: Vec<String>,
    /// The event-flag bank at open time, for mid-message `$FA`.
    flags: Vec<bool>,
    /// A taken mid-message `$FA` jump target; the window rebuilds the flow.
    jump: Option<u16>,
    /// A mid-message `$F6`; the window forwards it to the runtime.
    event: Option<u16>,
    /// This flow's entry id — `$FA` branch targets are relative to it.
    entry_id: u16,
}

impl TextFlow {
    /// Walks the preamble the interaction code eats and starts the message.
    ///
    /// `entry := $FA* ( $F6 event | $F3? text... )`. The `$FA` run is followed
    /// on its not-set branch -- event flags do not exist yet -- and every skip
    /// is logged.
    #[must_use]
    #[allow(dead_code)] // The flag-free form, kept for tests and future callers.
    pub fn open(entry: &DialogueEntry) -> Opening {
        TextFlow::open_with_flags(entry, &[])
    }

    /// [`TextFlow::open`], consulting the live event-flag bank so `$FA`
    /// chains take their set branches (this is how the town reacts to the
    /// story, and how the principal's entry 0 routes to his briefing).
    #[must_use]
    pub fn open_with_flags(entry: &DialogueEntry, flags: &[bool]) -> Opening {
        let mut log = Vec::new();
        let mut index = 0;
        while let Some(Segment::Control(ctrl @ Ctrl::FlagCheck { .. })) = entry.segments.get(index)
        {
            if let Ctrl::FlagCheck {
                flag, then_entry, ..
            } = ctrl
            {
                if flags.get(*flag as usize).copied().unwrap_or(false) {
                    // Branch targets are RELATIVE: GetOffsetByID counts
                    // forward from the current entry. (Absolute worked for
                    // the principal only because his chain starts at 0.)
                    return Opening::Jump(entry.id + *then_entry);
                }
                log.push(format!(
                    "flag_check: flag {flag} -> entry {then_entry}, not taken"
                ));
            }
            index += 1;
        }
        if let Some(Segment::Control(Ctrl::Event { id, .. })) = entry.segments.get(index) {
            return Opening::Event(*id);
        }
        if let Some(Segment::Control(Ctrl::KeepNpcFacing { .. })) = entry.segments.get(index) {
            log.push("keep_npc_facing: the NPC keeps its facing (not modelled yet)".to_owned());
            index += 1;
        }

        let mut flow = TextFlow {
            segments: entry.segments[index..].to_vec(),
            index: 0,
            char_offset: 0,
            lines: vec![String::new()],
            line: 0,
            column: 0,
            portrait: None,
            hold: 0,
            stop: None,
            done: false,
            closed: false,
            log,
            flags: flags.to_vec(),
            jump: None,
            event: None,
            entry_id: entry.id,
        };
        flow.pump();
        if flow.done && flow.stop.is_none() {
            return Opening::Silent;
        }
        Opening::Window(Box::new(flow))
    }

    /// The one or two lines currently on screen.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The portrait id currently shown, if any.
    #[must_use]
    pub fn portrait(&self) -> Option<u8> {
        self.portrait
    }

    /// How the page on screen ended, if a page is finished.
    #[must_use]
    pub fn page_end(&self) -> Option<PageEnd> {
        self.stop
    }

    /// Whether the arrow is up: the page is full and more text follows.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        matches!(
            self.stop,
            Some(PageEnd::Full | PageEnd::Wait | PageEnd::Close)
        )
    }

    /// Whether the window is still showing anything.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.closed
    }

    /// Whether a `$F9` delay is running.
    #[must_use]
    #[allow(dead_code)] // Field will want this when scenes gate on hold state.
    pub fn is_holding(&self) -> bool {
        self.hold > 0
    }

    /// One engine tick: runs down a `$F9` delay. Returns whether anything
    /// changed.
    pub fn tick(&mut self) -> bool {
        if self.hold == 0 {
            return false;
        }
        self.hold -= 1;
        if self.hold > 0 {
            return false;
        }
        self.pump();
        true
    }

    /// The accept press. Clears a finished page and runs on, or closes the
    /// window when the message is over.
    pub fn advance(&mut self) {
        match self.stop {
            None => {}
            Some(PageEnd::End | PageEnd::Choice) => {
                self.stop = None;
                self.closed = true;
            }
            Some(_) => {
                self.stop = None;
                self.lines = vec![String::new()];
                self.line = 0;
                self.column = 0;
                if self.done {
                    self.closed = true;
                } else {
                    self.pump();
                }
            }
        }
    }

    /// A taken mid-message jump, once.
    pub fn take_jump(&mut self) -> Option<u16> {
        self.jump.take()
    }

    /// A mid-message `$F6` event, once.
    pub fn take_event(&mut self) -> Option<u16> {
        self.event.take()
    }

    /// Everything the flow could not act on since the last call.
    pub fn drain_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// The code byte `offset` segments ahead, or `None` when that is text or
    /// past the end -- which is the same lookahead the routine does, since the
    /// entry's own `$FF` terminator is not in the list.
    fn peek(&self, offset: usize) -> Option<u8> {
        match self.segments.get(self.index + offset) {
            Some(Segment::Control(ctrl)) => Some(ctrl.code()),
            _ => None,
        }
    }

    fn close(&mut self, end: PageEnd) {
        self.stop = Some(end);
    }

    /// `TextCtrlCode_Interrupt`. Returns whether the message ended here.
    ///
    /// It looks at the byte it stopped on before showing anything: the end of
    /// the entry and `$F7` terminate without an arrow, and a following `$FD`
    /// is swallowed so one page does not wait twice.
    fn interrupt(&mut self, waited: PageEnd) -> bool {
        if self.index >= self.segments.len() {
            self.close(PageEnd::End);
            self.done = true;
            return true;
        }
        match self.peek(0) {
            Some(0xF7) => {
                self.index += 1;
                self.close(PageEnd::Close);
            }
            Some(0xFD) => {
                self.index += 1;
                self.close(waited);
            }
            _ => self.close(waited),
        }
        false
    }

    /// Runs segments until a page is finished, a delay starts, or the entry
    /// ends.
    fn pump(&mut self) {
        while self.index < self.segments.len() {
            let segment = self.segments[self.index].clone();
            self.index += 1;
            let stop = match segment {
                Segment::Text(run) => self.run_text(&run),
                Segment::Control(ctrl) => self.run_control(&ctrl),
            };
            if stop {
                return;
            }
        }
        if self.lines.iter().any(|line| !line.is_empty()) {
            self.close(PageEnd::End);
        }
        self.done = true;
    }

    /// Writes a text run into the window, breaking and paging as it goes.
    /// Returns whether pumping stops here.
    fn run_text(&mut self, run: &str) -> bool {
        let chars: Vec<char> = run.chars().collect();
        let start = std::mem::take(&mut self.char_offset);
        for position in start..chars.len() {
            self.lines[self.line].push(chars[position]);
            self.column += 1;
            if self.column < CHARS_PER_LINE {
                continue;
            }
            self.column = 0;
            // The routine peeks at the next *byte*, which is the next segment
            // only when the run ends on this character.
            let at_run_end = position + 1 == chars.len();
            if at_run_end && self.peek(0) == Some(0xF5) {
                self.index += 1;
                self.log_choice();
                self.close(PageEnd::Choice);
                self.done = true;
                return true;
            }
            self.line += 1;
            if at_run_end && self.peek(0) == Some(0xFC) {
                self.index += 1;
            }
            if !self.line.is_multiple_of(LINES_PER_WINDOW) {
                self.lines.push(String::new());
            } else if at_run_end {
                self.interrupt(PageEnd::Full);
                return true;
            } else {
                // More glyphs of this run follow, so the interrupt has nothing
                // to swallow: the page just fills. Resume mid-run.
                self.close(PageEnd::Full);
                self.index -= 1;
                self.char_offset = position + 1;
                return true;
            }
        }
        false
    }

    /// Runs one control code. Returns whether pumping stops here.
    fn run_control(&mut self, ctrl: &Ctrl) -> bool {
        match ctrl {
            Ctrl::Newline { .. } => {
                self.column = 0;
                self.line += 1;
                while self.lines.len() <= self.line {
                    // $FC does not check the two-line limit. Retail data never
                    // does this; a pack that did would be a data bug, and the
                    // window would show it rather than hide it.
                    self.lines.push(String::new());
                }
                false
            }
            Ctrl::Wait { .. } => {
                self.interrupt(PageEnd::Wait);
                true
            }
            Ctrl::Close { .. } => {
                self.close(PageEnd::Close);
                true
            }
            Ctrl::YesNo { .. } => {
                self.log_choice();
                self.close(PageEnd::Choice);
                self.done = true;
                true
            }
            Ctrl::Portrait { id, position, .. } => {
                self.portrait = (*id != PORTRAIT_HIDE).then_some(*id);
                if let Some(slot) = position {
                    self.log.push(format!(
                        "portrait: talk slot {slot} ignored (talk mode is later)"
                    ));
                }
                false
            }
            Ctrl::Delay { frames, .. } => {
                self.hold = *frames;
                true
            }
            Ctrl::Action {
                action,
                action_id,
                sound,
                panel,
                flag,
                ..
            } => {
                self.log.push(format!(
                    "action: {action:?} (#{action_id}) skipped; sound {sound:?}, panel {panel:?}, \
                     flag {flag:?}"
                ));
                false
            }
            Ctrl::FlagCheck {
                flag, then_entry, ..
            } => {
                if self.flags.get(*flag as usize).copied().unwrap_or(false) {
                    self.jump = Some(self.entry_id + *then_entry);
                    self.done = true;
                    self.close(PageEnd::End);
                    return true;
                }
                self.log.push(format!(
                    "flag_check: flag {flag} -> entry {then_entry}, not taken"
                ));
                false
            }
            Ctrl::Event { id, .. } => {
                self.event = Some(*id);
                self.done = true;
                self.close(PageEnd::End);
                true
            }
            Ctrl::KeepNpcFacing { .. } => {
                // Mid-message this reaches TextCtrlCode_Null, which is an rts:
                // the message ends here.
                self.log
                    .push("keep_npc_facing mid-message: ends the message (rts)".to_owned());
                self.close(PageEnd::End);
                self.done = true;
                true
            }
        }
    }

    fn log_choice(&mut self) {
        self.log.push(
            "yes_no: no choice window in v1, the message ends here (both branches skipped)"
                .to_owned(),
        );
    }
}

// ---------------------------------------------------------------------------
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
    border: f32,
    /// The whole message box, frame included.
    box_size: Vector2,
    /// Where the portrait goes, relative to the box's top-left corner.
    portrait_offset: Vector2,
    portrait_size: Vector2,
    /// Where the scroll arrow goes, relative to the box's top-left corner.
    arrow_offset: Vector2,
    /// Margins of the box inside the Genesis frame, kept when the viewport is
    /// wider or taller than 320x224.
    bottom_margin: f32,
    /// Cells the frame grows per animation step.
    step_cells: i32,
    /// Colour index $F of `Pal_Init_Line_3`: what the glyphs are drawn in, and
    /// the only colour this module picks for itself (the arrow placeholder).
    text_color: Color,
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
        let text_color = set
            .window
            .palette
            .colors
            .get(15)
            .map_or(Color::WHITE, |rgb| {
                Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)
            });

        Some(WindowView {
            tiles,
            font,
            glyph_at,
            glyph: Vector2::new(text.glyph_width as f32, text.glyph_height as f32),
            cell: set.window.geometry.cell_pixels as f32,
            border: set.window.geometry.border_cells as f32,
            box_size: Vector2::new(text.rect.width as f32, text.rect.height as f32),
            portrait_offset: Vector2::new(
                (portrait.rect.x - text.rect.x) as f32,
                (portrait.rect.y - text.rect.y) as f32,
            ),
            portrait_size: Vector2::new(portrait.rect.width as f32, portrait.rect.height as f32),
            arrow_offset: Vector2::new(
                (arrow.screen_x - text.rect.x) as f32,
                (arrow.screen_y - text.rect.y) as f32,
            ),
            bottom_margin: SCREEN.1 - (text.rect.y + text.rect.height) as f32,
            step_cells: set.window.geometry.open_animation.step_cells as i32,
            text_color,
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
    /// The scroll-arrow placeholder, when the page is waiting.
    arrow: Option<PackedVector2Array>,
    /// Colour index $F: what glyphs are drawn in, and the arrow with them.
    text_color: Color,
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
            open_cells: 0,
            event_flags: Vec::new(),
            pending_event: None,
            current_tree: 0,
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
            // The typewriter: one glyph per 3 frames, measured off hardware.
            let total = self.flow.as_ref().map_or(0, |flow| {
                flow.lines().iter().map(|l| l.chars().count()).sum()
            });
            if self.revealed < total {
                self.reveal_tick += 1;
                if self.reveal_tick >= 3 {
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
        if let Some(points) = list.arrow {
            self.base_mut()
                .draw_colored_polygon(&points, list.text_color);
        }
    }
}

impl DialogueWindow {
    /// Hands the window the pack. Until this is called it can only complain.
    pub fn configure(&mut self, pack_dir: &str, set: DialogueSet) {
        self.pack_dir = pack_dir.to_owned();
        match WindowView::build(pack_dir, &set) {
            Some(view) => self.view = Some(view),
            None => godot_error!("dialogue: window or font art failed to load from {pack_dir}"),
        }
        self.set = Some(set);
    }

    /// Opens an NPC's line: the map's dialogue tree (1-based) and the object's
    /// `dialogue_id`. Returns whether a window actually opened.
    pub fn open_dialogue(&mut self, tree: u8, dialogue_id: u16) -> bool {
        let Some(set) = self.set.as_ref() else {
            godot_error!("dialogue: no pack loaded; call configure() first");
            return false;
        };
        // Follow `$FA` preamble jumps against the live flags, bounded so a
        // cyclic chain (a data bug) cannot hang.
        self.current_tree = tree;
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

    /// A `$F6` event the dialogue fired, once. Field starts the scene.
    pub fn take_pending_event(&mut self) -> Option<u16> {
        self.pending_event.take()
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

    /// The accept press.
    pub fn advance(&mut self) {
        if self.is_opening() {
            return;
        }
        let mut reopen = false;
        let mut closed = false;
        if let Some(flow) = self.flow.as_mut() {
            reopen = flow.page_end() == Some(PageEnd::Close);
            let total: usize = flow.lines().iter().map(|l| l.chars().count()).sum();
            if self.revealed < total {
                // A press mid-typewriter completes the page instead of
                // advancing it. ASSUMPTION pending an oracle tape (the
                // common idiom; retail's behavior here is untested).
                self.revealed = total;
                self.drain_log();
                self.sync_portrait();
                self.base_mut().queue_redraw();
                return;
            }
            flow.advance();
            self.revealed = 0;
            self.reveal_tick = 0;
            closed = !flow.is_open();
        }
        self.drain_log();
        if closed {
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

    /// Whether the arrow is up and the window is waiting to be advanced.
    #[must_use]
    #[allow(dead_code)] // For a renderer that styles the waiting state itself.
    pub fn is_waiting(&self) -> bool {
        self.flow.as_ref().is_some_and(TextFlow::is_waiting)
    }

    fn is_opening(&self) -> bool {
        self.view
            .as_ref()
            .is_some_and(|view| self.open_cells < view.cells().0)
    }

    /// Applies mid-message `$FA` jumps and `$F6` events the flow raised.
    fn service_flow_signals(&mut self) {
        let (jump, event) = match self.flow.as_mut() {
            Some(flow) => (flow.take_jump(), flow.take_event()),
            None => (None, None),
        };
        if let Some(event) = event {
            self.pending_event = Some(event);
            self.close();
        }
        if let Some(next) = jump {
            let tree = self.current_tree;
            self.close();
            self.open_dialogue(tree, next);
        }
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
    /// the Genesis frame, generalised to whatever viewport the renderer has.
    /// The box keeps its distance from the bottom edge and stays centred, at
    /// the world's own integer scale.
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
        let position = Vector2::new(
            (top_left.x + (visible.x - size.x) / 2.0).floor(),
            (top_left.y + visible.y - margin - size.y).floor(),
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
            let origin = Vector2::new(view.border * cell, view.border * cell);
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
                                    row as f32 * view.glyph.y,
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
                quads.push(Quad {
                    texture: texture.clone(),
                    dest: Rect2::new(view.portrait_offset, view.portrait_size),
                    src: Rect2::new(Vector2::ZERO, view.portrait_size),
                });
            }

            let total: usize = flow.lines().iter().map(|l| l.chars().count()).sum();
            if flow.is_waiting() && self.revealed >= total {
                arrow = Some(waiting_indicator(view.arrow_offset, cell));
            }
        }

        Some(DrawList {
            fill,
            quads,
            arrow,
            text_color: view.text_color,
        })
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

/// The placeholder for the scroll arrow: a triangle in the text colour at the
/// sprite's own position. The arrow is a hardware sprite whose art is not in
/// the pack, so this is deliberately not a reconstruction of it.
fn waiting_indicator(at: Vector2, cell: f32) -> PackedVector2Array {
    let half = cell / 2.0;
    PackedVector2Array::from(&[
        at,
        at + Vector2::new(cell, 0.0),
        at + Vector2::new(half, half),
    ])
}

#[cfg(test)]
mod tests;
