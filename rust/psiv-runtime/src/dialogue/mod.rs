//! The retail dialogue runner: the message box, its clock and its signals.
//!
//! The cartridge's dialogue is a byte walker ([`TextFlow`]) plus a window that
//! owns the clock: the box opens over nine frames, one glyph appears every
//! three frames (one per frame while Speak is held), a press advances a
//! finished page and is swallowed mid-typewriter or mid-animation, `$F7`
//! suspends the window while the scene moves its actors, and `$F2` actions are
//! released when the typewriter reaches their byte position.
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

mod choice;
pub mod glue;
mod open;
mod text_flow;
mod view;

#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_corpus;

pub use text_flow::{DialogueAction, Opening, TextFlow};
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
            choice: ChoiceState::default(),
            open_cells: 0,
            revealed: 0,
            reveal_tick: 0,
            pad: Pad::NEUTRAL,
            previous: Pad::NEUTRAL,
            event: None,
            tree: 0,
            scene_dialogue: false,
            cutscene_portrait: false,
            signals: Vec::new(),
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

    /// Whether a window is on screen. The shell gates its input on this.
    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        self.flow.is_some()
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
        let flow = self.flow.as_ref()?;
        Some(DialogueView {
            lines: flow.lines().to_vec(),
            revealed: self.revealed,
            total: flow.visible_chars(),
            portrait: flow.portrait(),
            page_end: flow.page_end(),
            waiting: flow.is_waiting(),
            dismissable: self.is_dismissable(),
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
        let pressed = pad.pressed(self.previous);
        self.pad = pad;
        self.previous = pad;
        if self.flow.is_none() {
            return;
        }
        if self.flow.as_ref().is_some_and(TextFlow::has_choice) {
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
        self.detect_close();
    }

    /// The window's own half of a frame: `$F2` actions, the open animation,
    /// the typewriter and the flow's signals.
    ///
    /// The shell calls it after the field has run and the frame's events have
    /// been applied — the point the window node's own `physics_process` held
    /// before the window moved here.
    pub(crate) fn window(&mut self, game: &mut GameState) {
        self.pump_actions(game);
        if self.flow.is_none() {
            return;
        }
        if self.open_cells < self.full_cells() {
            // Oracle: the box opens in 9 frames; the pack's step_cells is
            // per SIDE, so the width grows by twice that each frame.
            let step = self.step_cells();
            self.open_cells = (self.open_cells + step).min(self.full_cells());
        } else {
            if let Some(flow) = self.flow.as_mut() {
                // Delays only run once the box is open; the cartridge's
                // animation holds the text loop the same way.
                flow.tick(game);
            }
            self.reveal();
        }
        self.drain_flow_log();
        self.service_flow(game);
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
                self.open_cells = 0;
                self.revealed = 0;
                self.reveal_tick = 0;
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
        let Some(flow) = self.flow.as_mut() else {
            return;
        };
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
        self.drain_flow_log();
        if self.flow.as_ref().is_some_and(TextFlow::is_open) {
            if reopen {
                // $F7 destroys the window; the next page opens a new one, so
                // the animation runs again.
                self.open_cells = 0;
                self.revealed = 0;
                self.reveal_tick = 0;
            }
            return;
        }
        if suspended {
            self.suspended = self.flow.take().map(|flow| (self.tree, flow));
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

    /// Releases the next `$F2` whose byte position the typewriter has reached,
    /// applying `SetEventFlag` to the live game state.
    ///
    /// Consecutive actions run back-to-back before the next character
    /// iteration, exactly as retail does; the byte stream only moves forward,
    /// so the loop ends at the entry's last action.
    fn pump_actions(&mut self, game: &mut GameState) {
        loop {
            let ready = self
                .flow
                .as_ref()
                .is_some_and(|flow| flow.action_ready(self.revealed));
            if !ready {
                return;
            }
            let Some(action) = self.flow.as_mut().and_then(TextFlow::take_pending_action) else {
                return;
            };
            if let DialogueAction::SetEventFlag(flag) = action {
                // The write lands before the text loop resumes, so a `$FA`
                // later in the same entry reads it.
                let _ = game.set(psiv_core::Flag::event(u16::from(flag)));
            }
            self.signals.push(DialogueSignal::Action(action));
            if let Some(flow) = self.flow.as_mut() {
                flow.resume_after_action(game);
            }
            self.service_flow(game);
            self.drain_flow_log();
        }
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
    fn reveal(&mut self) {
        let total = self.flow.as_ref().map_or(0, TextFlow::visible_chars);
        if self.revealed >= total {
            return;
        }
        let cadence = if self.pad.held(Button::Speak) { 1 } else { 3 };
        self.reveal_tick += 1;
        if self.reveal_tick >= cadence {
            self.reveal_tick = 0;
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
            && self
                .flow
                .as_ref()
                .is_some_and(|flow| flow.has_choice() && self.revealed >= flow.visible_chars())
    }

    /// Whether an accept press would advance or dismiss the page now.
    fn is_dismissable(&self) -> bool {
        // The flow reaches its page end before the typewriter has revealed
        // the glyphs; a dismissal must not start until the page is actually
        // on screen.
        !self.is_opening()
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

    /// Cells the box grows per animation step, per frame.
    fn step_cells(&self) -> i32 {
        self.set
            .as_ref()
            .map_or(1, |set| {
                i32::try_from(set.window.geometry.open_animation.step_cells).unwrap_or(1)
            })
            .max(1)
            * 2
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
