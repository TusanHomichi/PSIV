//! The retail dialogue runner: the message box, its clock and its signals.
//!
//! The cartridge's dialogue is a byte walker ([`TextFlow`]) plus a window that
//! owns the clock: the box opens over nine frames, one glyph appears every
//! three frames (one per frame while Speak is held), a press advances a
//! finished page and is swallowed mid-typewriter or mid-animation, `$F7`
//! suspends the window while the scene moves its actors, and `$F2` actions are
//! released when the typewriter reaches their byte position.
//!
//! A `$F4` costs frames too ([`PortraitChange`]): decompressing a portrait's
//! art takes two lag frames (`PORTRAIT_LOAD_LAG_FRAMES`), VInts that find
//! the 68000 still busy. A lag frame runs nothing: no input, no field tick, no
//! `Main_Frame_Count` and no RNG step ([`DialogueRunner::lag_frame`]).
//!
//! That window lives here now, not in the renderer. It reads the live
//! [`GameState`] at every branch, writes `$F2` event flags directly, and turns
//! everything the presentation layer must do into [`DialogueSignal`]s. What
//! the shell gets back is a [`DialogueView`] snapshot to draw.
//!
//! # The two halves of a frame
//!
//! The cartridge's window node ran *after* the field node every frame, and the
//! field kept ticking while the window was up. The runner keeps that split,
//! so the shell calls it twice per frame with the same pad:
//!
//! ```text
//! frame N   input(pad)     the player's press, the answer, the close
//!           ...            the shell runs the field and its events
//!           window()       $F2 actions, the box, the typewriter, the signals
//! ```
//!
//! Nothing in the second half touches the field, so the RNG stream and every
//! frame count stay where they were; swapping the halves would move a `$F2`
//! flag write across a field tick, which is a behavior change and not a
//! refactor.

mod actions;
mod choice;
pub(crate) mod close;
pub mod glue;
mod open;
mod selection;
#[cfg(test)]
mod selection_tests;
mod stop_byte;
mod text3;
mod text_flow;
mod view;

#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_corpus;

pub(crate) use text_flow::ELSYDEON_BROKEN_FRAMES;
pub use text_flow::{DialogueAction, Opening, PortraitChange, TextFlow};
pub use view::{DialogueChoiceView, DialogueView};

use crate::pad::{Button, Pad};
use choice::ChoiceState;
use psiv_core::GameState;
use psiv_data::{DialogueSet, PageEnd};
use std::sync::Arc;

/// Something the dialogue did that the shell has to present.
///
/// The list a frame produces is ordered: signals are appended as the frame
/// walks, and the shell drains them once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogueSignal {
    /// The window shut. `suspended` says retail's `$F7` saved a text cursor
    /// for the scene (the scene hears `dialogue_closed`) rather than the
    /// message running out (`dialogue_ended`). The runtime has already
    /// delivered that acknowledgement; this is the presentation side — the
    /// panel planes a scene dialogue leaves behind.
    Closed {
        /// Whether an `$F7` cursor is waiting to be resumed.
        suspended: bool,
    },
    /// The player answered a yes/no prompt. The scene has the answer already;
    /// this is the log line.
    ChoiceAnswered(bool),
    /// A retail `$F2` action released at its byte position. `SetEventFlag`
    /// has already been written to the game state; the rest are presentation.
    Action(DialogueAction),
    /// A diagnostic message from the flow: a skipped flag check, an ignored
    /// portrait slot, a control with no payload.
    Log(String),
    /// A defect: a missing entry, a broken jump chain, a resume with no saved
    /// cursor. The shell reports these as errors.
    Fault(String),
}

/// The dialogue window's state machine: the flow, its clock and its signals.
pub struct DialogueRunner {
    /// The pack: the trees the flow walks, and the window geometry the open
    /// animation counts in. Shared with the loaded [`GameData`] it came from,
    /// so a runtime hands the same set to its window instead of copying it.
    ///
    /// [`GameData`]: psiv_data::GameData
    set: Option<Arc<DialogueSet>>,
    /// The window's stream, while one is on screen.
    flow: Option<TextFlow>,
    /// Retail's saved text address, including its original tree binding. Map
    /// changes between dialogue chunks must not redirect the cursor.
    suspended: Option<(u8, TextFlow)>,
    /// The yes/no cursor and the answer rules.
    choice: ChoiceState,
    /// Frame width in cells while the box is opening; the full width once it
    /// is open.
    open_cells: i32,
    /// Glyphs revealed on the current page: retail draws one character every
    /// 3 frames (oracle: logs/03_npc_talk.csv, writes to Win_Tile_Buffer on a
    /// strict 3-frame cadence — 20 chars/second); `reveal_tick` counts frames
    /// toward the next one.
    revealed: usize,
    reveal_tick: u8,
    /// A typewriter step that came due at a `$F4` whose frames had not run:
    /// it draws its glyph first thing after them, whatever the pad then.
    step_due: bool,
    /// This frame's pad: the typewriter reads its Speak level.
    pad: Pad,
    /// Last frame's pad, for the presses.
    previous: Pad,
    /// A `$F6` the flow fired. The shell drains it on the *next* frame, which
    /// is when the window node's pending event became visible to the field.
    event: Option<u16>,
    /// The tree of the currently open dialogue, for mid-message jumps.
    tree: u8,
    /// Scene dialogue selects `WinGroup_Event` for its portrait window;
    /// ordinary talk keeps `WinGroup_Dialogue`. A scene pause is preserved
    /// regardless of which window the original flags select.
    scene_dialogue: bool,
    cutscene_portrait: bool,
    /// Signals produced since the last drain, in order.
    signals: Vec<DialogueSignal>,
    /// The guild board's text mode (`session/job_board.rs`): a window closes
    /// by itself at its terminator and its last page stays drawn.
    board_text: bool,
    /// The open scene window is one its routine leaves on screen when the
    /// text ends (`Runtime::open_scene_dialogue`): its last page is held.
    hold_last_page: bool,
    /// A window's last page, still drawn after its text ended: the guild
    /// board's, until the next text replaces it, or a scene window's, until
    /// the scene destroys it ([`DialogueRunner::release_held_page`]) or the
    /// next text replaces it.
    held_page: Option<DialogueView>,
    /// The portrait the routine's first text put up; later texts leave it.
    board_portrait: Option<u8>,
    /// The byte under the saved dialogue address after the last stop
    /// (`stop_byte.rs`).
    stop_byte: Option<u8>,
    /// The byte two before it, the last the text ran (`stop_byte.rs`).
    last_byte: Option<u8>,
    /// Whether the typewriter has reached the flow's first pending `$F4`
    /// ([`TextFlow::pending_portrait`]) and its frames have started.
    portrait_reached: bool,
    /// Its destroy frames left to run.
    destroy_left: u8,
    /// Its lag frames, waiting for the destroy frames.
    lag_left: u8,
    /// The pad of the typewriter step that waits behind its frames.
    deferred_pad: Option<Pad>,
    /// Lag frames left ([`DialogueRunner::lag_frame`]).
    lag: u8,
    /// A finished `$F4`'s glyph and what the plane shows until the glyph
    /// after it is drawn: nothing after a load, the old portrait after
    /// `$F4 0`.
    portrait_plane: Option<(usize, Option<u8>)>,
    /// The window runs `RunText3` (`text3.rs`): no pad, no arrow, its own
    /// page clock.
    text3: bool,
    /// The page's `RunText3` wait: frames left, once its last glyph is drawn.
    text3_wait: Option<u16>,
    /// Retail `Panel_Num`, shared by scene and dialogue panel operations.
    panels: u8,
    /// A panel load waiting for its measured decompression lag.
    loading_panel: Option<DialogueAction>,
}

impl Default for DialogueRunner {
    fn default() -> Self {
        DialogueRunner::new()
    }
}

impl DialogueRunner {
    /// A runner with no pack and no window.
    #[must_use]
    pub(crate) fn new() -> DialogueRunner {
        DialogueRunner {
            set: None,
            flow: None,
            suspended: None,
            board_text: false,
            hold_last_page: false,
            held_page: None,
            board_portrait: None,
            choice: ChoiceState::default(),
            open_cells: 0,
            revealed: 0,
            reveal_tick: 0,
            step_due: false,
            pad: Pad::NEUTRAL,
            previous: Pad::NEUTRAL,
            event: None,
            tree: 0,
            scene_dialogue: false,
            cutscene_portrait: false,
            signals: Vec::new(),
            stop_byte: None,
            last_byte: None,
            portrait_reached: false,
            destroy_left: 0,
            lag_left: 0,
            deferred_pad: None,
            lag: 0,
            portrait_plane: None,
            text3: false,
            text3_wait: None,
            panels: 0,
            loading_panel: None,
        }
    }

    /// A runner whose window resolves entries through `set`.
    #[must_use]
    pub(crate) fn with_pack(set: Arc<DialogueSet>) -> DialogueRunner {
        let mut runner = DialogueRunner::new();
        runner.set_pack(set);
        runner
    }

    /// Hands the runner the dialogue pack it resolves entries through.
    pub(crate) fn set_pack(&mut self, set: Arc<DialogueSet>) {
        self.set = Some(set);
    }

    /// The loaded dialogue pack, for the renderer's art and the pack's own
    /// tree table.
    #[must_use]
    pub(crate) fn pack(&self) -> Option<&DialogueSet> {
        self.set.as_deref()
    }

    /// The guild board's text mode: see `Runtime::dialogue_board_text`.
    pub(crate) fn set_board_text(&mut self, on: bool) {
        self.board_text = on;
        self.held_page = None;
        self.board_portrait = None;
    }

    /// The open scene window's routine leaves it on screen when its text
    /// ends, so its last page stays drawn until the scene destroys it.
    pub(crate) fn hold_page_at_end(&mut self) {
        self.hold_last_page = self.scene_dialogue && self.flow.is_some();
    }

    /// The scene destroyed the window its text left up (`Window_Destroy`).
    pub(crate) fn release_held_page(&mut self) {
        self.held_page = None;
    }

    /// The last page as it stays on screen once its text has ended: drawn,
    /// with no arrow, no prompt and nothing a press would advance.
    fn last_page(&self) -> Option<DialogueView> {
        self.view().map(|page| DialogueView {
            waiting: false,
            dismissable: false,
            choice: None,
            ..page
        })
    }

    /// Whether a window is on screen. The shell gates its input on this.
    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        self.flow.is_some()
    }

    /// Whether this frame is a lag frame: its VInt finds `VInt_Flag` clear
    /// (`ps4.asm:548-549`), so it skips `ReadJoypads`, `UpdateRNGSeed` and
    /// `Main_Frame_Count` (`ps4.asm:614-624`), while the 68000 is still
    /// decompressing a portrait or a panel. Nothing of the frame runs.
    #[must_use]
    pub(crate) fn lag_frame(&self) -> bool {
        self.lag > 0
    }

    /// A `$F6` the flow fired, once.
    pub(crate) fn take_event(&mut self) -> Option<u16> {
        self.event.take()
    }

    /// The signals produced since the last call.
    pub(crate) fn drain_signals(&mut self) -> Vec<DialogueSignal> {
        std::mem::take(&mut self.signals)
    }

    /// What the renderer draws this frame, or `None` with no window up.
    #[must_use]
    pub(crate) fn view(&self) -> Option<DialogueView> {
        let Some(flow) = self.flow.as_ref() else {
            return self.held_page.clone();
        };
        Some(DialogueView {
            lines: flow.lines().to_vec(),
            revealed: self.revealed,
            total: flow.visible_chars(),
            portrait: self.shown_portrait(flow),
            page_end: flow.page_end(),
            // `RunText3` draws no arrow and reads no press.
            waiting: flow.is_waiting() && !self.text3,
            dismissable: self.is_dismissable() && !self.text3,
            open_cells: self.open_cells,
            choice: flow.has_choice().then_some(DialogueChoiceView {
                cursor: self.choice.cursor(),
                ready: self.choice_ready(),
            }),
            tree: self.tree,
            scene_dialogue: self.scene_dialogue,
            cutscene_portrait: self.cutscene_portrait,
        })
    }

    /// The input half of a frame.
    ///
    /// The shell calls this every frame, window or not: the pad is latched
    /// either way, so the press that opened a window does not read as a fresh
    /// press on the frame after, and a press made while the field owned input
    /// was spent where it happened. `notice_up` is retail's one exception to
    /// accept-only dismissal: cancel closes a field-status window.
    pub(crate) fn input(&mut self, game: &mut GameState, pad: Pad, notice_up: bool) {
        if self.lag_frame() {
            // No `ReadJoypads` on a lag frame: a press is read, as a press,
            // on the first frame after it.
            return;
        }
        let pressed = pad.pressed(self.previous);
        self.pad = pad;
        self.previous = pad;
        if self.flow.is_none() {
            return;
        }
        // The `$F4`s the typewriter has reached run before the frame reads
        // the page: a `$F4 0` finishes at once, and a load or swap starts in
        // this frame's phase, under a prompt as on any page.
        self.portrait_work(Some(pad), false, game);
        if self.flow.as_ref().is_some_and(TextFlow::has_jump_or_event) {
            // The walk resumed into a `$FA` jump or a `$F6`: the window half
            // services it, as it does any other.
            return;
        }
        if self.board_text
            && self.is_dismissable()
            && self.flow.as_ref().and_then(TextFlow::page_end) == Some(PageEnd::End)
        {
            // `RunText` returns at the terminator; the window stays for the
            // next text, and the guild's routine destroys it itself.
            self.retain_board_portrait();
            self.held_page = self.view();
            self.close_window();
            self.detect_close();
            return;
        }
        if self.text3_input(game) {
            // `RunText3` reads no pad: its own clock turns the page.
        } else if self.flow.as_ref().is_some_and(TextFlow::has_choice) {
            // Choice input owns the prompt, including while it finishes
            // typing; merely advancing the text never picks an answer.
            if !self.choice_ready() {
                return;
            }
            self.choice.move_cursor(pressed);
            if let Some(yes) = self.choice.answer(pressed) {
                self.answer_choice(yes, game);
            }
        } else if pressed.held(Button::Speak) || (notice_up && pressed.held(Button::Cancel)) {
            self.advance(game);
        }
        // A `$F4` the typewriter reached (or a page the press opened on one):
        // its frames start now, and a lag frame starts with this one.
        self.portrait_work(Some(pad), false, game);
        self.detect_close();
    }

    /// The window's own half of a frame: `$F2` actions, the open animation,
    /// the typewriter and the flow's signals.
    ///
    /// The shell calls it after the field has run and the frame's events have
    /// been applied — the point the window node's own `physics_process` held
    /// before the window moved here.
    pub(crate) fn window(&mut self, game: &mut GameState) {
        if self.lag_frame() {
            self.lag -= 1;
            return;
        }
        // Keep the old panel planes through decompression; deliver the
        // completed load before resuming the byte loop.
        if let Some(action) = self.loading_panel.take() {
            self.signals.push(DialogueSignal::Action(action));
        }
        // New actions launch after this tick: their first VInt is this one.
        let action_was_holding = self.flow.as_ref().is_some_and(TextFlow::action_holding);
        if let Some(flow) = self.flow.as_mut() {
            flow.tick_action(game);
        }
        self.pump_actions(game);
        if self.flow.is_none() || self.action_holding() || self.lag_frame() {
            return;
        }
        if self.open_cells < self.full_cells() {
            // `loc_686BC` (`ps4.asm:139790`) widens the box by the pack's
            // step_cells (two: a cell each side) a pass, and render bit 1
            // `bchg`s bit 2 so only every other pass, the first included,
            // waits for a VInt: the frames show 2, 6, 10 ... cells (tape 43,
            // frames 8203-8211).
            let pass = self.step_cells();
            self.open_cells = if self.open_cells == 0 {
                pass
            } else {
                self.open_cells + 2 * pass
            }
            .min(self.full_cells());
            // A `$F4` at the head of the page runs once the box is open: its
            // frames follow this one, before the typewriter's first.
            self.portrait_work(None, false, game);
        } else if self.portrait_work(Some(self.pad), true, game) {
            // The typewriter's step waits behind the portrait's frames.
        } else {
            self.retain_board_portrait();
            if let Some(flow) = self.flow.as_mut()
                && self.revealed >= flow.visible_chars()
                && !action_was_holding
            {
                // Delays only run once the box is open; the cartridge's
                // animation holds the text loop the same way. A `$F9` is a
                // byte of the text loop, so it starts once the glyphs before
                // it are drawn, not with the page.
                flow.tick(game);
            }
            // A control after a delay runs as that delay returns. Waiting
            // until the next window tick would spend its VInt twice.
            self.pump_actions(game);
            if self.flow.is_none() || self.action_holding() || self.lag_frame() {
                self.drain_flow_log();
                return;
            }
            // A delay that ran out into a `$F4` the typewriter is at: a
            // `$F4 0` finishes before the step, and a load or swap starts as
            // one reached by typing does. The step, if due, waits for it.
            self.portrait_work(None, false, game);
            let pad = self.deferred_pad.take().unwrap_or(self.pad);
            self.reveal(pad);
            self.settle_portrait_plane();
        }
        self.text3_tick();
        self.drain_flow_log();
        self.service_flow(game);
    }

    /// Runs the `$F4`s the typewriter has reached, in stream order: each
    /// one's destroy frames, counted, then its lag frames. The typewriter
    /// step that reached the first (with `pad`, the frame's Speak level)
    /// waits behind all of them, so a change adds exactly its own frames to
    /// the page.
    ///
    /// The input half and the opening box start a change (`spend` false): a
    /// change with no destroy frame makes this frame its first lag frame in
    /// the input half, and the next one after the box opens. The window half
    /// (`spend` true) spends destroy frames, and returns whether this frame
    /// belongs to the portraits, so the typewriter takes no step.
    fn portrait_work(&mut self, pad: Option<Pad>, spend: bool, game: &GameState) -> bool {
        loop {
            let Some(change) = self
                .flow
                .as_ref()
                .and_then(TextFlow::pending_portrait)
                .copied()
            else {
                return false;
            };
            if self.portrait_reached {
                if self.lag > 0 {
                    return true;
                }
                if self.destroy_left > 0 {
                    if !spend {
                        return false;
                    }
                    // `$06A1B4`: a counted frame, the old portrait gone and
                    // no glyph. Its last one starts the lag.
                    self.destroy_left -= 1;
                    if self.destroy_left == 0 {
                        self.lag = std::mem::take(&mut self.lag_left);
                    }
                    return true;
                }
                // Its frames are spent. A destroy's `DMAPlane_A_VInt` left
                // the plane without a portrait; with none, it still shows
                // what it did. The new one comes with the glyph after it, and
                // the next `$F4` may follow at once.
                let plane = if change.destroy_frames > 0 {
                    None
                } else {
                    self.displayed_before(&change)
                };
                self.finish_portrait(change.at, plane, game);
                continue;
            }
            if self.is_opening() || self.revealed < change.at {
                return false;
            }
            if !change.has_frames() {
                // `$F4 0` restores the plane with no frame of its own: what
                // the screen shows goes with the next glyph.
                let plane = self.displayed_before(&change);
                self.finish_portrait(change.at, plane, game);
                continue;
            }
            self.portrait_reached = true;
            self.deferred_pad = self.deferred_pad.or(pad);
            self.destroy_left = change.destroy_frames;
            self.lag_left = change.lag_frames;
            if self.destroy_left > 0 {
                continue;
            }
            // In the input half this frame is the first lag frame; in the
            // window half the next one is, and this one waits.
            self.lag = std::mem::take(&mut self.lag_left);
            return spend;
        }
    }

    /// Drops the flow's first pending `$F4`; the plane shows `plane` until
    /// the glyph after `at` is drawn.
    fn finish_portrait(&mut self, at: usize, plane: Option<u8>, game: &GameState) {
        if let Some(flow) = self.flow.as_mut() {
            flow.finish_portrait(game);
        }
        self.portrait_reached = false;
        self.destroy_left = 0;
        self.lag_left = 0;
        self.portrait_plane = Some((at, plane));
    }

    /// Whether a `$F4` is still to run or running: the page cannot be
    /// advanced, answered or closed before its portrait is up.
    fn portrait_pending(&self) -> bool {
        self.lag > 0
            || self
                .flow
                .as_ref()
                .is_some_and(|flow| flow.pending_portrait().is_some())
    }

    /// The finished `$F4`'s plane, while the glyph after it is not drawn.
    fn portrait_plane(&self) -> Option<Option<u8>> {
        let total = self.flow.as_ref().map_or(0, TextFlow::visible_chars);
        self.portrait_plane
            .filter(|&(at, _)| self.revealed <= at && self.revealed < total)
            .map(|(_, plane)| plane)
    }

    /// What the screen shows as `change` runs: a finished `$F4` at the same
    /// glyph left its plane there, with no `DMAPlane_A_VInt` since; otherwise
    /// the portrait before it.
    fn displayed_before(&self, change: &PortraitChange) -> Option<u8> {
        self.portrait_plane().unwrap_or(change.previous)
    }

    /// The guild board keeps the portrait its first text put up, once it is
    /// on screen.
    fn retain_board_portrait(&mut self) {
        if self.board_text && !self.portrait_pending() && self.portrait_plane().is_none() {
            self.board_portrait = self
                .flow
                .as_ref()
                .and_then(TextFlow::portrait)
                .or(self.board_portrait);
        }
    }

    /// Forgets the plane once the glyph after its `$F4` is drawn, or the
    /// page has no glyph after it.
    fn settle_portrait_plane(&mut self) {
        if self.portrait_plane().is_none() {
            self.portrait_plane = None;
        }
    }

    fn clear_portrait_change(&mut self) {
        self.step_due = false;
        self.portrait_reached = false;
        self.destroy_left = 0;
        self.lag_left = 0;
        self.deferred_pad = None;
        self.portrait_plane = None;
    }

    /// The portrait the window draws: the old one until the typewriter
    /// reaches a change, none while its frames run, then the new one with
    /// the glyph after it.
    fn shown_portrait(&self, flow: &TextFlow) -> Option<u8> {
        let reached = flow.pending_portrait().filter(|_| self.portrait_reached);
        let portrait = if let Some(change) = reached {
            // A swap's destroy frame clears the plane; a load with nothing
            // up sends nothing until its glyph.
            if change.destroy_frames > 0 {
                None
            } else {
                self.displayed_before(change)
            }
        } else if let Some(plane) = self.portrait_plane() {
            plane
        } else if let Some(change) = flow.pending_portrait() {
            change.previous
        } else {
            flow.portrait()
        };
        portrait.or(self.board_portrait)
    }

    fn start(&mut self, opening: Opening, who: &str) -> bool {
        match opening {
            Opening::Jump(next) => {
                self.fault(format!("{who}: unresolved jump to {next} reached start()"));
                false
            }
            Opening::Event(event) => {
                self.log(format!("{who} fires event {event:#x}"));
                self.event = Some(event);
                false
            }
            Opening::Silent => {
                self.log(format!("{who} is empty; nothing to show"));
                false
            }
            Opening::Window(flow) => {
                self.flow = Some(*flow);
                let replaced = self.held_page.take().is_some();
                self.open_cells = if self.board_text && replaced {
                    self.full_cells()
                } else {
                    0
                };
                self.revealed = 0;
                self.reveal_tick = 0;
                self.text3_wait = None;
                self.clear_portrait_change();
                self.choice.reset();
                self.drain_flow_log();
                true
            }
        }
    }

    /// The accept press. Clears a finished page and runs on, or closes the
    /// window when the message is over.
    fn advance(&mut self, game: &mut GameState) {
        if self.is_opening() {
            return;
        }
        // The page as it was drawn, before the advance moves the flow on.
        let last_page = if self.hold_last_page {
            self.last_page()
        } else {
            None
        };
        let Some(flow) = self.flow.as_mut() else {
            return;
        };
        if !flow.is_dismissable() {
            return;
        }
        if self.lag > 0 || flow.pending_portrait().is_some() {
            // `TextCtrlCode_Portrait` runs before the routine reads the page
            // end: a press cannot leave the page before its portrait is up.
            return;
        }
        let reopen = flow.page_end() == Some(PageEnd::Close);
        if self.revealed < flow.visible_chars() {
            // A press mid-typewriter neither completes the page nor advances
            // it: retail accelerates only while Speak is HELD (the window
            // half's cadence), and a tap adds exactly its held frames (oracle
            // tapes 13/15 — the earlier complete-the-page assumption was
            // wrong). Swallow the press.
            return;
        }
        let suspended = self.scene_dialogue && flow.pause_for_scene();
        if !suspended {
            flow.advance(game);
        }
        self.revealed = 0;
        self.reveal_tick = 0;
        self.step_due = false;
        self.portrait_plane = None;
        self.drain_flow_log();
        if self.flow.as_ref().is_some_and(TextFlow::is_open) {
            if reopen {
                // $F7 destroys the window; the next page opens a new one, so
                // the animation runs again.
                self.open_cells = 0;
                self.revealed = 0;
                self.reveal_tick = 0;
                self.step_due = false;
            }
            return;
        }
        if suspended {
            self.suspended = self.flow.take().map(|flow| (self.tree, flow));
        }
        if last_page.is_some() {
            self.held_page = last_page;
        }
        self.close_window();
    }

    /// The player's answer to a yes/no prompt.
    fn answer_choice(&mut self, yes: bool, game: &mut GameState) {
        self.signals.push(DialogueSignal::ChoiceAnswered(yes));
        if self.choice.standalone() {
            // The scene owns this prompt: the window closes on the answer and
            // no flow continues.
            self.close_window();
            self.choice.reset();
            return;
        }
        if let Some(flow) = self.flow.as_mut() {
            flow.answer_choice(yes, game);
        }
        self.revealed = 0;
        self.reveal_tick = 0;
        self.step_due = false;
        self.portrait_plane = None;
        self.choice.reset();
        self.service_flow(game);
        self.drain_flow_log();
    }

    /// The close acknowledgement, once, on the frame the window shut inside
    /// the input half. A window that shuts while servicing a jump or an event
    /// (the window half) is deliberately silent: the node behaved the same
    /// way, and the scene's `DialogueClosed` reached it through the close
    /// that came before.
    fn detect_close(&mut self) {
        if self.flow.is_some() {
            return;
        }
        self.signals.push(DialogueSignal::Closed {
            suspended: self.suspended.is_some(),
        });
    }

    /// Applies mid-message `$FA` jumps and `$F6` events the flow raised.
    fn service_flow(&mut self, game: &GameState) {
        for _ in 0..16 {
            let (jump, event) = match self.flow.as_mut() {
                Some(flow) => (flow.take_jump(), flow.take_event()),
                None => (None, None),
            };
            if let Some(event) = event {
                self.event = Some(event);
                self.close_window();
                return;
            }
            let Some(next) = jump else {
                return;
            };
            let entry = self.set.as_ref().and_then(|set| set.entry(self.tree, next));
            match (self.flow.as_mut(), entry) {
                (Some(flow), Some(entry)) => flow.continue_at(entry, game),
                _ => {
                    let tree = self.tree;
                    self.fault(format!("missing branch entry {next} in tree {tree}"));
                    self.close_window();
                    return;
                }
            }
        }
        self.fault("in-stream branch chain too deep".to_owned());
        self.close_window();
    }

    /// The typewriter: one glyph per 3 frames released, one per frame while
    /// Speak is held — hold-to-accelerate, measured off hardware (oracle tapes
    /// 13/15: 39 draws in a 40-frame held span vs 13 released; an early press
    /// inside the open animation is dropped, which [`DialogueRunner::advance`]
    /// already models). Page advance is EDGE-triggered and acceleration is
    /// LEVEL-driven — measured independently (tape 16): a held button never
    /// advances a finished page (58 chars at 1/frame, then a dead stop with
    /// the button down), but a fresh press with the hold maintained starts the
    /// next page already accelerated. Reading the held state per tick here
    /// gives exactly that pairing.
    ///
    /// The typewriter never draws past a `$F4` whose frames have not run:
    /// a step due there waits, and is the first after them.
    fn reveal(&mut self, pad: Pad) {
        let Some(flow) = self.flow.as_ref() else {
            return;
        };
        let total = flow.visible_chars();
        if self.revealed >= total {
            return;
        }
        let limit = flow.pending_portrait().map_or(total, |change| change.at);
        if self.step_due {
            if self.revealed < limit {
                self.step_due = false;
                self.reveal_tick = 0;
                self.revealed += 1;
            }
            return;
        }
        // `RunText3` draws a glyph every frame and reads no pad.
        let cadence = if self.text3 || pad.held(Button::Speak) {
            1
        } else {
            3
        };
        self.reveal_tick += 1;
        if self.reveal_tick >= cadence {
            self.reveal_tick = 0;
            if self.revealed >= limit {
                self.step_due = true;
                return;
            }
            self.revealed += 1;
        }
    }

    /// Whether the box has finished opening: a press and the choice cursor
    /// both wait for it.
    fn is_opening(&self) -> bool {
        self.open_cells < self.full_cells()
    }

    /// Whether the yes/no prompt is on screen, fully typed and answerable.
    fn choice_ready(&self) -> bool {
        !self.is_opening()
            && !self.portrait_pending()
            && self
                .flow
                .as_ref()
                .is_some_and(|flow| flow.has_choice() && self.revealed >= flow.visible_chars())
    }

    /// Whether an accept press would advance or dismiss the page now.
    fn is_dismissable(&self) -> bool {
        // The flow reaches its page end before the typewriter has revealed
        // the glyphs; a dismissal must not start until the page is actually
        // on screen, its portrait included.
        !self.is_opening()
            && !self.portrait_pending()
            && self
                .flow
                .as_ref()
                .is_some_and(|flow| self.revealed >= flow.visible_chars() && flow.is_dismissable())
    }

    /// The box's full width in cells, from the pack's window geometry.
    fn full_cells(&self) -> i32 {
        let Some(window) = self.set.as_ref().map(|set| &set.window) else {
            return 0;
        };
        let cell = i32::try_from(window.geometry.cell_pixels).unwrap_or(i32::MAX);
        if cell == 0 {
            return 0;
        }
        window.text_window.rect.width / cell
    }

    /// Cells one `loc_686BC` pass widens the box by: the pack's step_cells.
    fn step_cells(&self) -> i32 {
        self.set
            .as_ref()
            .map_or(1, |set| {
                i32::try_from(set.window.geometry.open_animation.step_cells).unwrap_or(1)
            })
            .max(1)
    }

    fn drain_flow_log(&mut self) {
        let lines = self
            .flow
            .as_mut()
            .map(TextFlow::drain_log)
            .unwrap_or_default();
        for line in lines {
            self.signals.push(DialogueSignal::Log(line));
        }
    }

    fn log(&mut self, line: String) {
        self.signals.push(DialogueSignal::Log(line));
    }

    fn fault(&mut self, line: String) {
        self.signals.push(DialogueSignal::Fault(line));
    }
}
