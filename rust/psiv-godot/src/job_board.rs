//! The Hunter's Guild list window.
//!
//! [`JobBoardScreen`] draws a [`JobBoardView`] and nothing else; the rows, the
//! cursor and when the window closes are the session's
//! (`psiv-runtime/src/session/job_board.rs`). The window art and font are the
//! shared set the camp draws with. A row in `$A000` or `$E000` is the same font
//! tiles on another colour line (`psiv_tools/guild_pack.py`), so the font is
//! recoloured once per line.

use godot::classes::{INode2D, ImageTexture, Node2D};
use godot::prelude::*;

use psiv_core::JobColour;
use psiv_data::{DialogueSet, GuildBoard, GuildLine};
use psiv_runtime::JobBoardView;

use crate::Field;
use crate::camp::chrome::{CampChrome, Quad};

/// `WinTiles_CursorBox`, and the solid pattern `loc_69A32` and the red cursor
/// draw over it (`destination.rs`).
const CURSOR_BOX_PATTERN: u16 = 0x6E7;
const SELECTED_PATTERN: u16 = 0x6E8;

/// The board node. `Field` owns it and hands it the session's view each frame.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub(crate) struct JobBoardScreen {
    base: Base<Node2D>,
    chrome: Option<CampChrome>,
    guild: Option<GuildBoard>,
    done_font: Option<Gd<ImageTexture>>,
    closed_font: Option<Gd<ImageTexture>>,
    view: Option<JobBoardView>,
}

#[godot_api]
impl INode2D for JobBoardScreen {
    fn init(base: Base<Node2D>) -> Self {
        JobBoardScreen {
            base,
            chrome: None,
            guild: None,
            done_font: None,
            closed_font: None,
            view: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_z_index(1000);
        self.base_mut().set_z_as_relative(false);
        self.base_mut().set_visible(false);
    }

    fn draw(&mut self) {
        for quad in self.quads() {
            self.base_mut()
                .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
        }
    }
}

fn recolour(chrome: &CampChrome, from: &GuildLine, to: &GuildLine) -> Option<Gd<ImageTexture>> {
    chrome.recoloured_font(from, to)
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
        if let (Some(chrome), Some(guild)) = (self.chrome.as_ref(), guild.as_ref()) {
            self.done_font = recolour(chrome, &guild.font_line, &guild.done_line);
            self.closed_font = recolour(chrome, &guild.font_line, &guild.closed_line);
        }
        self.guild = guild;
    }

    /// Shows the session's board, or hides the node when there is none.
    pub(crate) fn sync(&mut self, view: Option<&JobBoardView>, center: Vector2) {
        if view != self.view.as_ref() {
            self.view = view.cloned();
            self.base_mut().queue_redraw();
        }
        let visible = view.is_some_and(|view| view.columns > 0);
        self.base_mut().set_visible(visible);
        if visible {
            self.base_mut()
                .set_position(center - Vector2::new(160.0, 112.0));
        }
    }

    fn quads(&self) -> Vec<Quad> {
        let (Some(chrome), Some(guild), Some(view)) = (
            self.chrome.as_ref(),
            self.guild.as_ref(),
            self.view.as_ref(),
        ) else {
            return Vec::new();
        };
        let window = guild.window;
        let (x, y) = (i32::from(window.x), i32::from(window.y));
        let width = i32::from(window.width);
        let columns = i32::from(view.columns);
        let mut quads = chrome.frame_cells(
            x + (width - columns) / 2,
            y,
            columns,
            i32::from(window.height),
        );
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
            quads.extend(chrome.window_word(pattern, (x + 1, line)));
            let text = if shown.pending {
                guild.pending.as_str()
            } else {
                guild.names[row].as_str()
            };
            let cell = (x + 3, line);
            let font = match shown.colour {
                JobColour::Open => None,
                JobColour::Done => self.done_font.as_ref(),
                JobColour::Closed => self.closed_font.as_ref(),
            };
            quads.extend(match font {
                Some(font) => chrome.text_in(font, text, cell),
                None => chrome.text(text, cell),
            });
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
