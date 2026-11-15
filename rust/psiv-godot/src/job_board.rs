//! The Hunter's Guild list window.
//!
//! [`JobBoardScreen`] draws a [`JobBoardView`] and nothing else; the rows, the
//! cursor and when the window closes are the session's
//! (`psiv-runtime/src/session/job_board.rs`). The window art and font are the
//! shared set the camp draws with. `Event_HuntersGuild` writes a row's name
//! with `$C000` while the job is open, `$A000` once it is collected and
//! `$E000` while it is not open: the same font tiles on CRAM lines 2, 1 and 3
//! (`psiv_tools/guild_pack.py`). So the screen draws through three
//! [`WindowLayer`]s, one per line, and the shell gives each its line's
//! material (`window_palette::WindowNode`): the frame, the cursor boxes and
//! the open names on `Window`, the collected names on `Done`, the others on
//! `Closed`.

use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

use psiv_core::JobColour;
use psiv_data::{DialogueSet, GuildBoard};
use psiv_runtime::JobBoardView;

use crate::Field;
use crate::camp::chrome::{CampChrome, Quad};
use crate::window_layer::WindowLayer;
use crate::window_palette::WindowNode;

/// `WinTiles_CursorBox`, and the solid pattern `loc_69A32` and the red cursor
/// draw over it (`destination.rs`).
const CURSOR_BOX_PATTERN: u16 = 0x6E7;
const SELECTED_PATTERN: u16 = 0x6E8;

/// The board's quads, by the layer that draws them.
#[derive(Default)]
struct BoardQuads {
    window: Vec<Quad>,
    done: Vec<Quad>,
    closed: Vec<Quad>,
}

/// The board's three layers, made on first use.
struct Layers {
    window: Gd<WindowLayer>,
    done: Gd<WindowLayer>,
    closed: Gd<WindowLayer>,
}

/// The board node. `Field` owns it and hands it the session's view each frame.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct JobBoardScreen {
    base: Base<Node2D>,
    chrome: Option<CampChrome>,
    guild: Option<GuildBoard>,
    view: Option<JobBoardView>,
    layers: Option<Layers>,
}

#[godot_api]
impl INode2D for JobBoardScreen {
    fn init(base: Base<Node2D>) -> Self {
        JobBoardScreen {
            base,
            chrome: None,
            guild: None,
            view: None,
            layers: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(1000);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
        self.layers();
    }
}

impl JobBoardScreen {
    /// Loads the shared window art and keeps the pack's board.
    pub(crate) fn configure(
        &mut self,
        pack_dir: &str,
        set: &DialogueSet,
        guild: Option<GuildBoard>,
    ) {
        self.chrome = CampChrome::build(pack_dir, set);
        self.guild = guild;
    }

    fn layers(&mut self) -> &mut Layers {
        if self.layers.is_none() {
            let layers = Layers {
                window: WindowLayer::named("Window"),
                done: WindowLayer::named("Done"),
                closed: WindowLayer::named("Closed"),
            };
            for layer in [&layers.window, &layers.done, &layers.closed] {
                self.base_mut().add_child(layer);
            }
            self.layers = Some(layers);
        }
        self.layers.as_mut().expect("made above")
    }

    /// The layer that draws `node`'s art; the window layer for a node that
    /// is not the board's.
    pub(crate) fn layer(&mut self, node: WindowNode) -> Gd<WindowLayer> {
        let layers = self.layers();
        match node {
            WindowNode::JobDone => layers.done.clone(),
            WindowNode::JobClosed => layers.closed.clone(),
            _ => layers.window.clone(),
        }
    }

    /// Shows the session's board, or hides the node when there is none.
    pub(crate) fn sync(&mut self, view: Option<&JobBoardView>, center: Vector2) {
        if view != self.view.as_ref() {
            self.view = view.cloned();
            let quads = self.quads();
            let layers = self.layers();
            layers.window.bind_mut().show(quads.window);
            layers.done.bind_mut().show(quads.done);
            layers.closed.bind_mut().show(quads.closed);
        }
        let visible = view.is_some_and(|view| view.columns > 0);
        self.base_mut().set_visible(visible);
        if visible {
            self.base_mut()
                .set_position(center - Vector2::new(160.0, 112.0));
        }
    }

    fn quads(&self) -> BoardQuads {
        let (Some(chrome), Some(guild), Some(view)) = (
            self.chrome.as_ref(),
            self.guild.as_ref(),
            self.view.as_ref(),
        ) else {
            return BoardQuads::default();
        };
        let window = guild.window;
        let (x, y) = (i32::from(window.x), i32::from(window.y));
        let width = i32::from(window.width);
        let columns = i32::from(view.columns);
        let mut quads = BoardQuads {
            window: chrome.frame_cells(
                x + (width - columns) / 2,
                y,
                columns,
                i32::from(window.height),
            ),
            ..BoardQuads::default()
        };
        if columns < width {
            return quads;
        }
        for (row, shown) in view.rows.iter().enumerate() {
            let line = y + 1 + row as i32 * 2;
            let lit =
                view.stamped == Some(row as u8) || (view.cursor_lit && view.cursor == row as u8);
            let pattern = if lit {
                SELECTED_PATTERN
            } else {
                CURSOR_BOX_PATTERN
            };
            quads
                .window
                .extend(chrome.window_word(pattern, (x + 1, line)));
            let text = if shown.pending {
                guild.pending.as_str()
            } else {
                guild.names[row].as_str()
            };
            let (glyphs, words) = chrome.text_parts(text, (x + 3, line));
            quads.window.extend(words);
            match shown.colour {
                JobColour::Open => quads.window.extend(glyphs),
                JobColour::Done => quads.done.extend(glyphs),
                JobColour::Closed => quads.closed.extend(glyphs),
            }
        }
        quads
    }
}

impl Field {
    /// Hands the board node what the session shows, over the camera's frame.
    pub(crate) fn present_job_board(&mut self) {
        let view = self
            .session
            .as_ref()
            .and_then(|session| session.job_board_view())
            .cloned();
        let center = self
            .camera
            .as_ref()
            .map_or(Vector2::ZERO, |camera| camera.get_position());
        if let Some(screen) = self.job_board.as_mut() {
            screen.bind_mut().sync(view.as_ref(), center);
        }
    }
}
