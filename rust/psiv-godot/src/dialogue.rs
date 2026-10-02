//! The dialogue window: the cartridge's message box, drawn from the pack.
//!
//! The node draws and nothing else. Every rule — the byte walk, the open
//! animation's frame count, the typewriter clock, the choices, the flags —
//! lives in `psiv-runtime`'s dialogue runner, which hands this node a
//! [`DialogueView`] once per frame. The node keeps the art: nine frame tiles
//! laid out by `window.json`'s geometry rule, the pack's glyph strip at 32
//! characters on two lines, the portrait each entry asks for, and the
//! byte-verified retail scroll-arrow sprite (not a guessed triangle; its
//! screen position is the text-side `scroll_arrow` record).
//!
//! Nothing here invents presentation either: the box sits where
//! `text_window.rect` says (272x48 at (24, 160) of the Genesis's 320x224
//! frame), and the scene-dialogue portrait applies the measured event-mode
//! delta rather than a second coordinate system.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{INode2D, Image, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_data::{CHARS_PER_LINE, DialogueSet, LINES_PER_WINDOW, Role};
use psiv_runtime::DialogueView;

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

/// The message box. `Field` owns one, hands it the runtime's view every frame,
/// and the runtime owns everything else.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct DialogueWindow {
    base: Base<Node2D>,
    view: Option<WindowView>,
    portraits: HashMap<u8, Gd<ImageTexture>>,
    pack_dir: String,
    /// What the runtime says to draw this frame; `None` with no window up.
    snapshot: Option<DialogueView>,
    choice_view: Option<choice::ChoiceView>,
}

#[godot_api]
impl INode2D for DialogueWindow {
    fn init(base: Base<Node2D>) -> Self {
        DialogueWindow {
            base,
            view: None,
            portraits: HashMap::new(),
            pack_dir: String::new(),
            snapshot: None,
            choice_view: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(Z_INDEX);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn physics_process(&mut self, _delta: f64) {
        if self.snapshot.is_none() {
            return;
        }
        self.place();
        self.base_mut().queue_redraw();
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
    /// Hands the window the pack's art. Until this is called it can only
    /// complain.
    pub fn configure(&mut self, pack_dir: &str, set: &DialogueSet) {
        self.pack_dir = pack_dir.to_owned();
        self.choice_view = choice::ChoiceView::build(pack_dir, set);
        match WindowView::build(pack_dir, set) {
            Some(view) => self.view = Some(view),
            None => godot_error!("dialogue: window or font art failed to load from {pack_dir}"),
        }
    }

    /// Takes this frame's runtime view. `None` hides the window.
    pub fn set_view(&mut self, snapshot: Option<DialogueView>) {
        if let Some(portrait) = snapshot.as_ref().and_then(|view| view.portrait) {
            self.cache_portrait(portrait);
        }
        let visible = snapshot.is_some();
        self.snapshot = snapshot;
        self.base_mut().set_visible(visible);
        self.base_mut().queue_redraw();
    }

    /// Whether a window is on screen. The shell gates input on the runtime's
    /// own view; this is only here so the node and the runtime cannot drift.
    #[must_use]
    #[allow(dead_code)] // For a renderer-side assertion in a later node.
    pub fn is_open(&self) -> bool {
        self.snapshot.is_some()
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
        let snapshot = self.snapshot.as_ref()?;
        let (full_cells, height_cells) = view.cells();
        let width_cells = snapshot.open_cells.clamp(0, full_cells);
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
            let mut budget = snapshot.revealed;
            'lines: for (row, line) in snapshot.lines.iter().enumerate().take(LINES_PER_WINDOW) {
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

            if let Some(id) = snapshot.portrait
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
                            + if snapshot.scene_dialogue && snapshot.cutscene_portrait {
                                RETAIL_SCENE_PORTRAIT_OFFSET
                            } else {
                                Vector2::ZERO
                            },
                        view.portrait_size,
                    ),
                    src: Rect2::new(Vector2::ZERO, view.portrait_size),
                });
            }

            if snapshot.waiting && snapshot.revealed >= snapshot.total {
                arrow = Some(Quad {
                    texture: view.arrow.clone(),
                    dest: Rect2::new(view.arrow_offset, view.arrow_size),
                    src: Rect2::new(Vector2::ZERO, view.arrow_size),
                });
            }
        }

        if let Some(cursor) = snapshot
            .choice
            .as_ref()
            .filter(|choice| choice.ready)
            .map(|choice| choice.cursor)
            && let Some(choice) = self.choice_view.as_ref()
        {
            quads.extend(choice.quads(view, cursor));
        }
        Some(DrawList { fill, quads, arrow })
    }

    /// Loads the portraits an entry needs. Called when a view asks for one, so
    /// the 39 PNGs are never all in memory at once.
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
