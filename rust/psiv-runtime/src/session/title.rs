//! The title: retail's front door, one frame at a time.
//!
//! `TitleRoutinePtrs` (`ps4.asm:86212`) chains the front door's routines —
//! the fading text, the Press Start prompt, the portraits, the scrolling text
//! and finally `TitleRoutine_PickOption` (`ps4.asm:87455`), the option window
//! this mode mirrors. The shell draws the phases; every decision is here:
//! which window is up, what the cursor does, which slot may be continued, and
//! what an accepted row *means*.
//!
//! # Buttons
//!
//! The cartridge's option windows confirm on
//! `ButtonSpeak_Mask|ButtonCamp_Mask|ButtonStart_Mask` and read the press edge
//! out of `Joypad_Pressed` (`TitleRoutine_PickOption`, `ps4.asm:87512-87513`;
//! `Title_NoSavedData`, `ps4.asm:87569-87570`; the CONTINUE slot list at
//! `ps4.asm:87622-87623`; ERASE DATA's list at `ps4.asm:87782-87783`). This
//! port reads the *held* pad and keeps its own latch, which is the same edge
//! for a key that goes down once, and it accepts the same three buttons. The
//! cursor is an Up/Down wrap: `moveq #2, d0` over a three-row window
//! (`ps4.asm:87527-87529`), `moveq #0, d0` over the single START row
//! (`ps4.asm:87572-87574`), `moveq #1, d0` over the two-row ARE YOU SURE?
//! window (`ps4.asm:87918-87920`).
//!
//! # Slot rules
//!
//! `$FFFFED59` is the presence bitmap `loc_4339A` fills by inspecting each of
//! the three slots (`ps4.asm:87474-87477`). CONTINUE's confirm refuses the
//! selected row when its bit is *set* (the slot is empty): it clears
//! `Joypad_Held` and re-enters the list with no state change at all
//! (`loc_43678`, `ps4.asm:87668-87674`), and a slot index that masks to 3 falls
//! back to START instead of loading (`ps4.asm:87710-87716`). ERASE DATA walks
//! the same list (`loc_437F8`, `ps4.asm:87746`; its own empty-row refusal
//! `ps4.asm:87828-87833`), opens the ARE YOU SURE? windows (`ps4.asm:87908`)
//! and erases through `loc_64DC0` (`ps4.asm:87937`) only when the cursor sits
//! on the first row; every other answer backs out to the slot list
//! (`ps4.asm:87925-87927`, `loc_43AF8` at `ps4.asm:87956`).
//!
//! # What this port does differently
//!
//! One fidelity fix, cited and tested: the option rows confirm on
//! `ButtonCamp` as well as `ButtonSpeak` and `ButtonStart`, which the port's
//! earlier held-pad read had dropped. The rest is recorded, not silently
//! corrected, because the certification captures were taken with it:
//!
//! - The Press Start hold is the retail `#$233` (563) frames
//!   (`TitleRoutine_PressStartButton`, `ps4.asm:86926`), and this port opens
//!   the option window when it expires — where the cartridge moves to
//!   `TitleRoutine_CharPortraits` (`Game_Mode_Routine = 8`, `ps4.asm:86943`)
//!   and reaches the option window only through the attract sequence. The
//!   port has no portraits or scrolling-text phase.
//! - Every phase accepts the option window's three buttons; the cartridge's
//!   Press Start hold leaves only on `ButtonStart` (`ps4.asm:86937`).
//! - The Sega and reveal phases advance on a press, which no routine in
//!   `TitleRoutine_FadingText` (`ps4.asm:86220-86387`) reads.
//! - ARE YOU SURE? opens on YES; the cartridge initializes
//!   `Window_Option_Index_3` to 1, the other row (`ps4.asm:87908`).
//! - The port is silent here: the cartridge plays `SFXID_Selection` on the
//!   erase and `SFXID_EnemyKilled` as its payload goes (`ps4.asm:87928`,
//!   `87934`).
//! - Cancel on the option window opens the cartridge's sound test
//!   (`ps4.asm:87517-87526`); this port ignores Cancel.

use psiv_core::{Direction, Input};

use crate::pad::{Button, Pad};

use super::{Frame, FrameMode, Mode, Session};

/// `TitleRoutine_PressStartButton`'s own countdown: `#$233` frames
/// (`ps4.asm:86926`) in the USA (non-`revision=0`) build.
pub(crate) const PRESS_START_HOLD_TICKS: u32 = 563;

/// Oracle tape 25: the Sega logo holds through frame 200. The retail fade
/// routine runs a frame-counted script and no routine in
/// `TitleRoutine_FadingText` reads the pad, so the hold is presentation timing
/// this port keeps as state: a press during it is swallowed by the phase
/// change, not by the fade.
const SEGA_HOLD_TICKS: u32 = 200;

/// The decoded title mappings settle by roughly frame 450; the transfer is
/// the reveal's own 300-frame script (tape 25).
const TITLE_REVEAL_TICKS: u32 = 299;

/// The phases of the front door, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlePhase {
    /// The Sega logo's fade and hold.
    Sega,
    /// The background transfer outline, before the title art arrives.
    Reveal,
    /// The settled logo and subtitle with the Press Start prompt.
    PressStart,
    /// The option window: CONTINUE, START, ERASE DATA — or START alone.
    Menu,
    /// CONTINUE's slot list.
    Slots,
    /// ERASE DATA's slot list.
    EraseSlots,
    /// ERASE DATA's ARE YOU SURE? window.
    EraseConfirm,
}

/// Which of the title's windows the phase shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleWindow {
    /// No window: the art phases.
    None,
    /// The single-row START window of a save-less card.
    NoSave,
    /// CONTINUE, START, ERASE DATA.
    SaveOptions,
    /// The three slot rows, for CONTINUE or ERASE DATA.
    Slots,
    /// ARE YOU SURE? with YES and NO.
    EraseConfirm,
}

/// What the shell draws this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitleView {
    /// The phase.
    pub phase: TitlePhase,
    /// The window the phase shows.
    pub window: TitleWindow,
    /// The row the cursor is on.
    pub cursor: usize,
    /// The three visible slots: `true` where a valid save sits.
    pub slots: [bool; 3],
    /// Frames this phase has run: the shell's press-start palette and its
    /// replay samples count from it.
    pub ticks: u32,
    /// Frames since the title appeared, phase changes included.
    pub elapsed: u32,
}

/// What an accepted row asked the session to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TitleAction {
    /// The frame only moved the cursor or a phase.
    None,
    /// START: build the retail initializer's new game.
    Start,
    /// CONTINUE the populated slot.
    Continue(usize),
    /// ERASE DATA the populated slot.
    Erase(usize),
}

/// The title flow's state.
pub(crate) struct TitleMode {
    phase: TitlePhase,
    ticks: u32,
    elapsed: u32,
    cursor: usize,
    slots: [bool; 3],
    /// The slot ERASE DATA is asking about.
    erase_slot: usize,
    accept_down: bool,
    direction_down: bool,
    /// The debug harness's switch (`PSIV_DEBUG_TITLE_AUTOSTART`): walk the
    /// phases with synthetic accepts and select START.
    autostart: bool,
}

impl TitleMode {
    /// The title over `slots`, at its first frame.
    pub(crate) fn new(slots: [bool; 3], autostart: bool) -> TitleMode {
        TitleMode {
            phase: TitlePhase::Sega,
            ticks: 0,
            elapsed: 0,
            cursor: 0,
            slots,
            erase_slot: 0,
            accept_down: false,
            direction_down: false,
            autostart,
        }
    }

    pub(crate) fn view(&self) -> TitleView {
        TitleView {
            phase: self.phase,
            window: self.window(),
            cursor: self.cursor,
            slots: self.slots,
            ticks: self.ticks,
            elapsed: self.elapsed,
        }
    }

    /// The window the phase shows. `Title_NoSavedData` (`ps4.asm:87542`)
    /// creates the one-row window when no slot holds a save.
    fn window(&self) -> TitleWindow {
        match self.phase {
            TitlePhase::Menu if self.slots.iter().any(|slot| *slot) => TitleWindow::SaveOptions,
            TitlePhase::Menu => TitleWindow::NoSave,
            TitlePhase::Slots | TitlePhase::EraseSlots => TitleWindow::Slots,
            TitlePhase::EraseConfirm => TitleWindow::EraseConfirm,
            TitlePhase::Sega | TitlePhase::Reveal | TitlePhase::PressStart => TitleWindow::None,
        }
    }

    /// One title frame.
    pub(crate) fn frame(&mut self, pad: Pad) -> TitleAction {
        self.ticks = self.ticks.saturating_add(1);
        self.elapsed = self.elapsed.saturating_add(1);
        // Debug-only fix-loop selector, same family as PSIV_DEBUG_EVENT: walk
        // the title phases with synthetic accepts and select START, so the
        // new-game handoff is testable without an input device. The shell sets
        // the switch; the runtime never reads the environment.
        if self.autostart
            && self.ticks > 10
            && matches!(
                self.phase,
                TitlePhase::Sega | TitlePhase::Reveal | TitlePhase::PressStart | TitlePhase::Menu
            )
        {
            if matches!(self.phase, TitlePhase::Menu) {
                self.cursor = if self.slots.iter().any(|slot| *slot) {
                    1
                } else {
                    0
                };
            }
            return self.accept();
        }
        // The option window's three buttons, held or pressed: this port reads
        // the held pad and latches, which is the same edge once.
        let accept_down =
            pad.held(Button::Speak) || pad.held(Button::Camp) || pad.held(Button::Start);
        let pressed = accept_down && !self.accept_down;
        self.accept_down = accept_down;
        // An accept row is not also a cursor move: the cartridge's confirm
        // branch runs before its cursor routine on the same `Joypad_Pressed`.
        let direction = if accept_down {
            None
        } else {
            match pad.field_input() {
                Input::Direction(direction) => Some(direction),
                _ => None,
            }
        };
        let direction_pressed = direction.is_some() && !self.direction_down;
        self.direction_down = direction.is_some();
        if direction_pressed && let Some(direction) = direction {
            self.move_menu(direction);
        }
        let choice = if pressed {
            self.accept()
        } else {
            TitleAction::None
        };
        if choice == TitleAction::None {
            match self.phase {
                TitlePhase::Sega if self.ticks > SEGA_HOLD_TICKS => {
                    self.phase = TitlePhase::Reveal;
                    self.ticks = 0;
                }
                TitlePhase::Reveal if self.ticks > TITLE_REVEAL_TICKS => {
                    self.phase = TitlePhase::PressStart;
                    self.ticks = 0;
                }
                TitlePhase::PressStart if self.ticks >= PRESS_START_HOLD_TICKS => {
                    self.phase = TitlePhase::Menu;
                    self.ticks = 0;
                }
                _ => {}
            }
        }
        choice
    }

    /// Moves the cursor over the phase's rows.
    fn move_menu(&mut self, direction: Direction) {
        let count = match self.phase {
            TitlePhase::Menu if self.slots.iter().any(|slot| *slot) => 3,
            TitlePhase::Menu => 1,
            TitlePhase::Slots | TitlePhase::EraseSlots => 3,
            TitlePhase::EraseConfirm => 2,
            _ => return,
        };
        self.cursor = match direction {
            Direction::Up if self.cursor == 0 => count - 1,
            Direction::Up => self.cursor - 1,
            Direction::Down => (self.cursor + 1) % count,
            _ => self.cursor,
        };
    }

    /// An accepted row.
    fn accept(&mut self) -> TitleAction {
        match self.phase {
            TitlePhase::Sega => {
                self.phase = TitlePhase::Reveal;
                self.ticks = 0;
            }
            TitlePhase::Reveal => {
                self.phase = TitlePhase::PressStart;
                self.ticks = 0;
            }
            TitlePhase::PressStart => {
                self.phase = TitlePhase::Menu;
                self.ticks = 0;
            }
            TitlePhase::Menu if !self.slots.iter().any(|slot| *slot) => {
                return TitleAction::Start;
            }
            TitlePhase::Menu if self.cursor == 0 => {
                self.phase = TitlePhase::Slots;
                self.cursor = self.slots.iter().position(|slot| *slot).unwrap_or(0);
            }
            TitlePhase::Menu if self.cursor == 1 => return TitleAction::Start,
            TitlePhase::Menu => {
                self.phase = TitlePhase::EraseSlots;
                self.cursor = self.slots.iter().position(|slot| *slot).unwrap_or(0);
            }
            // An empty row is refused with no state change: the cartridge
            // clears the held pad and re-enters the list (`ps4.asm:87678`).
            TitlePhase::Slots if self.slots.get(self.cursor).copied().unwrap_or(false) => {
                return TitleAction::Continue(self.cursor);
            }
            TitlePhase::Slots => {}
            TitlePhase::EraseSlots if self.slots.get(self.cursor).copied().unwrap_or(false) => {
                self.erase_slot = self.cursor;
                self.phase = TitlePhase::EraseConfirm;
                self.cursor = 0;
            }
            TitlePhase::EraseSlots => {}
            // YES erases; NO (`loc_43AF8`) backs out to the slot list.
            TitlePhase::EraseConfirm if self.cursor == 0 => {
                return TitleAction::Erase(self.erase_slot);
            }
            TitlePhase::EraseConfirm => {
                self.phase = TitlePhase::EraseSlots;
                self.cursor = self.erase_slot;
            }
        }
        TitleAction::None
    }

    /// The slot's payload was zeroed: its row leaves the list and the option
    /// window comes back, as `loc_64DC0`'s return through `loc_4335C` does.
    pub(crate) fn finish_erase(&mut self, slot: usize, erased: bool) {
        if erased {
            if let Some(row) = self.slots.get_mut(slot) {
                *row = false;
            }
            self.phase = TitlePhase::Menu;
            self.cursor = 0;
            self.ticks = 0;
        } else {
            self.phase = TitlePhase::EraseSlots;
            self.cursor = slot;
        }
    }
}

/// What a title frame asked the shell to do, after the session did the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleFrame {
    /// What to draw. `None` on the frame START or CONTINUE handed the session
    /// to the field: the front door is over and its nodes come down.
    pub view: Option<TitleView>,
    /// The game this frame entered, when START or CONTINUE did.
    pub entered: Option<TitleEntry>,
    /// The erase this frame ran, when ERASE DATA did.
    pub erased: Option<TitleErase>,
    /// Why a row could not complete, for the shell's log. The title stays up.
    pub failure: Option<TitleFailure>,
}

/// The game a title frame entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleEntry {
    /// START: the retail initializer rebuilt the runtime. `event_started` is
    /// whether the opening's scene is running.
    Started {
        /// Whether Event_GameStart's scene started.
        event_started: bool,
    },
    /// CONTINUE: the slot loaded and validated.
    Continued {
        /// The visible slot, zero-based.
        slot: usize,
    },
}

/// The erase a title frame ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleErase {
    /// The slot's physical payload was zeroed; its row left the list.
    Erased {
        /// The visible slot, zero-based.
        slot: usize,
        /// The file that was rewritten.
        path: String,
    },
    /// The erase failed; the title is back on the slot list with the cursor on
    /// that slot.
    Failed {
        /// The visible slot, zero-based.
        slot: usize,
        /// The store's own message.
        error: String,
    },
}

/// A front-door row that could not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleFailure {
    /// START could not build the retail initializer's runtime.
    NewGame(String),
    /// CONTINUE could not load or validate the slot.
    Continue {
        /// The visible slot, zero-based.
        slot: usize,
        /// The validator's or the store's message.
        error: String,
    },
}

impl Session {
    /// Puts the session at retail's front door and returns its first view.
    ///
    /// The shell calls this where the cartridge's `MainGameProgram` enters the
    /// title sequence: on a power-on boot, and after the fade a defeat leaves
    /// behind. The title owns every frame from here until START or CONTINUE
    /// enters the game.
    pub fn start_title(&mut self) -> TitleView {
        let slots = self.save_slots();
        let autostart = self.title_autostart;
        self.mode = Mode::Title(Box::new(TitleMode::new(slots, autostart)));
        match &self.mode {
            Mode::Title(mode) => mode.view(),
            _ => unreachable!("the title was just installed"),
        }
    }

    /// The title's view while it owns the session's frames.
    ///
    /// `None` once START or CONTINUE handed the frame to the field, and on a
    /// defeat's last frame, where [`TitleFrame::view`] carries the view the
    /// shell is about to draw.
    #[must_use]
    pub fn title(&self) -> Option<TitleView> {
        match &self.mode {
            Mode::Title(mode) => Some(mode.view()),
            _ => None,
        }
    }

    /// Whether a defeat's fade owns the frames.
    ///
    /// The shell's battle stage keeps its last page on screen through the fade
    /// — only the frame that restores the title takes it down — so it asks
    /// this, not the screen's own visibility, before it drives another battle
    /// frame (`end_battle_presentation` is the other half of the same rule).
    #[must_use]
    pub fn game_over_active(&self) -> bool {
        matches!(self.mode, Mode::GameOver(_))
    }

    /// One frame of the front door.
    ///
    /// START, CONTINUE and ERASE DATA complete here, inside the frame the
    /// player chose them on: the session builds the new game or loads the slot
    /// with no round trip through the shell, and the shell learns what happened
    /// from [`TitleFrame`].
    pub(crate) fn title_frame(&mut self, pad: Pad) -> Frame {
        let action = match &mut self.mode {
            Mode::Title(mode) => mode.frame(pad),
            _ => unreachable!("title_frame runs only in title mode"),
        };
        let mut entered = None;
        let mut erased = None;
        let mut failure = None;
        match action {
            TitleAction::None => {}
            TitleAction::Erase(slot) => match self.erase_slot(slot) {
                Ok(path) => {
                    if let Mode::Title(mode) = &mut self.mode {
                        mode.finish_erase(slot, true);
                    }
                    erased = Some(TitleErase::Erased {
                        slot,
                        path: path.display().to_string(),
                    });
                }
                Err(error) => {
                    if let Mode::Title(mode) = &mut self.mode {
                        mode.finish_erase(slot, false);
                    }
                    erased = Some(TitleErase::Failed {
                        slot,
                        error: error.to_string(),
                    });
                }
            },
            TitleAction::Start => match self.start_new_game() {
                Ok(event_started) => entered = Some(TitleEntry::Started { event_started }),
                Err(error) => failure = Some(TitleFailure::NewGame(error)),
            },
            TitleAction::Continue(slot) => match self.continue_save(slot) {
                Ok(()) => entered = Some(TitleEntry::Continued { slot }),
                Err(error) => failure = Some(TitleFailure::Continue { slot, error }),
            },
        }
        // A completed choice hands the next frame to the field: the title's own
        // view is gone by then, and the shell reads the choice instead. An
        // erase stays on the front door, so its frame still carries the new
        // window the row left behind.
        let view = match &self.mode {
            Mode::Title(mode) => Some(mode.view()),
            _ => None,
        };
        Frame {
            mode: FrameMode::Title,
            title: Some(TitleFrame {
                view,
                entered,
                erased,
                failure,
            }),
            ..Frame::default()
        }
    }
}

#[cfg(test)]
mod title_tests;
