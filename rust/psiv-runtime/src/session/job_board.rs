//! The Hunter's Guild job board: the list window and its cursor.
//!
//! `Event_HuntersGuild` (`ps4.asm:150995`) keeps one text window up from its
//! welcome to its goodbye, opens the list beside it and reads the pad in three
//! places. The scene (`psiv-core`, `scenes/hunters_guild.rs`) keeps the order
//! and the flags; this mode holds the window and owns the pad at those three
//! places, each a blocking [`JobBoardStep`]:
//!
//! ```text
//! Hold    DoMapUpdate_JoypadLoop(#$3B): 60 frames, or a press of
//!         Cancel, Speak, Camp or Start (`ps4.asm:121041`)
//! Choose  .selectjobloop: Cancel, Speak or Camp ends it, on `loc_69A32`'s
//!         solid box; otherwise Win_UpdateCursorUpDown moves the cursor,
//!         wrapping, with SFXID_MovingCursor, and the red cursor blinks
//! Linger  .terminateguildwindowloop: the same loop, with the cursor object
//!         gone (`clr.w (a4)`), so a move only sounds
//! ```
//!
//! While the mode is up the dialogue window keeps its last text with no press
//! to close it (`Runtime::dialogue_board_text`), since `RunText` ends
//! at its terminator and the routine destroys the window itself.

use psiv_core::{Input, JobBoardStep, JobRow, job_row};

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::{Frame, FrameMode, Session};

/// `SFXID_MovingCursor`.
const SFX_MOVING_CURSOR: u8 = 0xF2;
/// `SFXID_Selection`, written after a pick that is not Cancel.
const SFX_SELECTION: u8 = 0xF3;
/// `move.w #$3B, d0` / `DoMapUpdate_JoypadLoop`, a `dbf` loop.
const HOLD_FRAMES: u16 = 60;
const JOBS: u8 = 8;
/// `FieldObj_RedCursor`'s timers, as the destination menu reads them.
const CURSOR_START_TIMER: i16 = 0x19;
const CURSOR_FLIP_TIMER: i16 = 0x0F;
const CURSOR_INVISIBLE_EXTRA: i16 = 0x0A;
/// `Window_Draw`'s first pass uploads two columns, each later one four.
const FIRST_COLUMNS: u8 = 2;
const STEP_COLUMNS: u8 = 4;

/// Everything the list window draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobBoardView {
    /// Columns of the list window uploaded; zero before it opens.
    pub columns: u8,
    /// The rows are written once the window is whole.
    pub rows: Vec<JobRow>,
    /// `Window_Option_Index_2`.
    pub cursor: u8,
    /// The red cursor object is up and in its visible half.
    pub cursor_lit: bool,
    /// The row `loc_69A32` stamped solid, until the boxes are redrawn.
    pub stamped: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The scene runs: text is typing, or nothing is waiting.
    Idle,
    Hold(u16),
    Opening,
    Choose,
    Linger,
}

#[derive(Clone, Debug)]
pub(crate) struct JobBoardMode {
    view: JobBoardView,
    phase: Phase,
    cursor_timer: i16,
    cursor_visible: bool,
    width: u8,
    /// A Hold began on this frame, whose tick the text window had suspended.
    hold_entered: bool,
}

impl JobBoardMode {
    fn new(runtime: &Runtime) -> JobBoardMode {
        let width = runtime
            .data()
            .guild()
            .map_or(FIRST_COLUMNS, |guild| guild.window.width);
        JobBoardMode {
            view: JobBoardView {
                columns: 0,
                rows: Vec::new(),
                cursor: 0,
                cursor_lit: false,
                stamped: None,
            },
            phase: Phase::Idle,
            cursor_timer: CURSOR_START_TIMER,
            cursor_visible: false,
            width,
            hold_entered: false,
        }
    }

    fn start_cursor(&mut self) {
        self.cursor_timer = CURSOR_START_TIMER;
        self.cursor_visible = true;
    }

    /// `RedCursor_Main`, as `Win_UpdateCursorUpDown` runs it first.
    fn update_cursor(&mut self) {
        self.cursor_timer -= 1;
        if self.cursor_timer < 0 {
            self.cursor_timer = CURSOR_FLIP_TIMER;
            self.cursor_visible = !self.cursor_visible;
            if !self.cursor_visible {
                self.cursor_timer += CURSOR_INVISIBLE_EXTRA;
            }
        }
    }

    /// Up then Down, each testing its own edge. A move shows the cursor.
    fn move_cursor(&mut self, pressed: Pad) -> Option<u8> {
        let mut sound = None;
        if pressed.held(Button::Up) {
            self.view.cursor = (self.view.cursor + JOBS - 1) % JOBS;
            sound = Some(SFX_MOVING_CURSOR);
            self.start_cursor();
        }
        if pressed.held(Button::Down) {
            self.view.cursor = (self.view.cursor + 1) % JOBS;
            sound = Some(SFX_MOVING_CURSOR);
            self.start_cursor();
        }
        sound
    }

    /// One frame; the answer for the scene once this phase is over.
    fn frame(&mut self, pressed: Pad) -> (Option<JobAnswer>, Option<u8>) {
        let any = |buttons: &[Button]| buttons.iter().any(|&button| pressed.held(button));
        match self.phase {
            Phase::Idle => (None, None),
            Phase::Hold(left) => {
                let pressed = any(&[Button::Cancel, Button::Speak, Button::Camp, Button::Start]);
                if pressed || left <= 1 {
                    (Some(JobAnswer::Done), None)
                } else {
                    self.phase = Phase::Hold(left - 1);
                    (None, None)
                }
            }
            Phase::Opening => {
                self.view.columns = if self.view.columns == 0 {
                    FIRST_COLUMNS
                } else {
                    self.view.columns.saturating_add(STEP_COLUMNS)
                }
                .min(self.width);
                if self.view.columns >= self.width {
                    (Some(JobAnswer::Done), None)
                } else {
                    (None, None)
                }
            }
            Phase::Choose => {
                if any(&[Button::Cancel, Button::Speak, Button::Camp]) {
                    self.view.stamped = Some(self.view.cursor);
                    self.cursor_visible = false;
                    // `btst #ButtonCancel` decides; Cancel plays no selection.
                    return if pressed.held(Button::Cancel) {
                        (Some(JobAnswer::Picked(None)), None)
                    } else {
                        (
                            Some(JobAnswer::Picked(Some(self.view.cursor))),
                            Some(SFX_SELECTION),
                        )
                    };
                }
                self.update_cursor();
                (None, self.move_cursor(pressed))
            }
            Phase::Linger => {
                if any(&[Button::Cancel, Button::Speak, Button::Camp]) {
                    return (Some(JobAnswer::Done), None);
                }
                let sound = self.move_cursor(pressed);
                self.cursor_visible = false;
                (None, sound)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobAnswer {
    Done,
    Picked(Option<u8>),
}

impl Session {
    /// Drops the board and its retained text: the session is leaving the scene.
    pub(super) fn reset_job_board(&mut self) {
        self.job_board = None;
        self.runtime.dialogue_board_text(false);
    }

    /// A scene's [`psiv_core::SceneOp::JobBoard`].
    pub(super) fn job_board_step(&mut self, step: JobBoardStep) {
        match step {
            JobBoardStep::Begin => {
                self.job_board = Some(JobBoardMode::new(&self.runtime));
                self.runtime.dialogue_board_text(true);
            }
            JobBoardStep::Close => {
                self.job_board = None;
                self.runtime.dialogue_board_text(false);
                self.runtime.set_field_suspended(false);
            }
            JobBoardStep::ShowList => {
                let flags: Vec<JobRow> = self.runtime.guild_jobs().map_or_else(Vec::new, |jobs| {
                    (0..usize::from(JOBS))
                        .map(|row| {
                            job_row(&jobs, row, |flag| {
                                self.runtime
                                    .game()
                                    .is_set(psiv_core::Flag::event(u16::from(flag)))
                            })
                        })
                        .collect()
                });
                if let Some(mode) = self.job_board.as_mut() {
                    mode.view.rows = flags;
                    mode.phase = Phase::Opening;
                }
            }
            JobBoardStep::Boxes => {
                if let Some(mode) = self.job_board.as_mut() {
                    mode.view.stamped = None;
                    mode.cursor_visible = false;
                }
            }
            JobBoardStep::Hold | JobBoardStep::Choose | JobBoardStep::Linger => {
                if let Some(mode) = self.job_board.as_mut() {
                    mode.phase = match step {
                        JobBoardStep::Hold => {
                            mode.hold_entered = true;
                            Phase::Hold(HOLD_FRAMES)
                        }
                        JobBoardStep::Choose => {
                            mode.start_cursor();
                            mode.view.cursor_lit = true;
                            Phase::Choose
                        }
                        _ => Phase::Linger,
                    };
                }
            }
        }
    }

    /// Whether the mode owns this frame's pad: a text window that is still
    /// typing or waiting on a press keeps it first.
    pub(super) fn job_board_waiting(&self) -> bool {
        !self.runtime.dialogue_open()
            && self
                .job_board
                .as_ref()
                .is_some_and(|mode| mode.phase != Phase::Idle)
    }

    /// The end of the frame a Hold began on, its first iteration: the loop
    /// tests the pad first, so a press this frame leaves with no update, and
    /// otherwise this frame runs the update the text window's suspension kept
    /// from the tick. The other 59 run on the board's own frames.
    pub(super) fn job_board_settle(&mut self, pressed: Pad) {
        let Some(mode) = self.job_board.as_mut() else {
            return;
        };
        if !std::mem::take(&mut mode.hold_entered) {
            return;
        }
        let leaving = [Button::Cancel, Button::Speak, Button::Camp, Button::Start]
            .iter()
            .any(|&button| pressed.held(button));
        if leaving {
            mode.phase = Phase::Hold(1);
        } else if self.runtime.field_suspended() {
            self.runtime.run_map_updates();
        }
    }

    /// One frame of the board.
    pub(super) fn job_board_frame(&mut self, pressed: Pad) -> Frame {
        let mode = self.job_board.as_mut().expect("a board frame has a mode");
        let (answer, sound) = mode.frame(pressed);
        mode.view.cursor_lit = mode.phase == Phase::Choose && mode.cursor_visible;
        let answered = answer.is_some();
        let holding = matches!(mode.phase, Phase::Hold(_));
        if let Some(answer) = answer {
            mode.phase = Phase::Idle;
            mode.view.cursor_lit = false;
            self.runtime.answer_job_board(match answer {
                JobAnswer::Done => psiv_core::SceneInput::JobBoardDone,
                JobAnswer::Picked(row) => psiv_core::SceneInput::JobBoardPicked(row),
            });
        }
        // The scene is blocked on the board and the field parked behind the
        // windows; the tick spends the frame, and on the frame the board
        // answers lets the scene take it. The Hold is the one wait that runs
        // map updates (`DoMapUpdate_JoypadLoop` calls `RunMapUpdates`, the
        // other loops only `DMAPlane_A_VInt`): the scene's update gate is shut
        // by a suspension, so it stays open there.
        self.runtime.set_field_suspended(!answered && !holding);
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::Field,
            events,
            routed,
            sound,
            ..Frame::default()
        }
    }

    /// The list window, while the guild's scene holds it.
    #[must_use]
    pub fn job_board_view(&self) -> Option<&JobBoardView> {
        self.job_board.as_ref().map(|mode| &mode.view)
    }
}
