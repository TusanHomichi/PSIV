//! What the renderer draws: one frame's snapshot of the window.
//!
//! The view is read-only and owned: the shell takes it once per frame and
//! draws it. Nothing here mutates game state, and nothing the renderer does
//! can change what the dialogue does next.

use psiv_data::PageEnd;

/// The yes/no window, when one is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogueChoiceView {
    /// Row the cursor is on: 0 = YES, 1 = NO.
    pub cursor: usize,
    /// Whether the prompt is fully typed and an answer would be accepted.
    pub ready: bool,
}

/// Everything a renderer needs to draw the message box this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueView {
    /// The one or two lines currently on screen.
    pub lines: Vec<String>,
    /// Glyphs revealed on the current page: draw this many, in reading order.
    pub revealed: usize,
    /// Glyphs the current page holds in total.
    pub total: usize,
    /// The portrait id currently shown, if any.
    pub portrait: Option<u8>,
    /// How the page on screen ended, when it has.
    pub page_end: Option<PageEnd>,
    /// Whether the retail scroll arrow is up: a full page with more to come.
    pub waiting: bool,
    /// Whether an accept press would advance or dismiss this page now: the
    /// page is fully revealed, the box has finished opening, and the flow is
    /// not sitting on a choice.
    pub dismissable: bool,
    /// Width of the partially opened box in cells; the full width once it is
    /// open. The renderer clamps it to the pack's window width.
    pub open_cells: i32,
    /// The yes/no window while the flow is asking one.
    pub choice: Option<DialogueChoiceView>,
    /// The dialogue tree the window is reading.
    pub tree: u8,
    /// Scene dialogue selects `WinGroup_Event` for its portrait window;
    /// ordinary talk keeps `WinGroup_Dialogue`.
    pub scene_dialogue: bool,
    /// A panel cutscene places its portrait with the measured event-mode
    /// delta; the renderer adds it to the pack's talk position.
    pub cutscene_portrait: bool,
}
