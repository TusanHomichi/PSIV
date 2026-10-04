//! The ship's destination menu: a session mode the scene hands the frame to.
//!
//! `Cutscene_InsideSpaceship` (`ps4.asm:155427`) is a jump into `loc_63BC4`
//! (`:133499-133726`), and `Cutscene_SpaceshipSabotage` carries a copy of the
//! same menu (`:155431-155560`). The routine builds a list of worlds from a flag
//! table and `World_Index` (`scenes/flight.rs` in `psiv-core` holds the table
//! and the list rule), draws two windows on a planetary map, and loops on the
//! joypad until Cancel, Speak or Camp is pressed (`:133604-133615`):
//!
//! ```text
//! no button      Win_UpdateCursorUpDown moves Window_Option_Index, wrapping,
//!                with SFXID_MovingCursor (`:141702-141731`); the red cursor
//!                object blinks (`RedCursor_Main`); `loc_64144` re-uploads the
//!                palette and lights the cursor row's marker on frames where
//!                `Main_Frame_Count` bit 3 is set (`:133894-133925`)
//! Cancel         the windows are destroyed and the scene resumes on its own
//!                cancel leg, nothing written (`loc_63E5E`, `:133692-133726`)
//! Speak or Camp  SFXID_Selection, the prompt window is cleared and typed over
//!                with "<NAME> will be / the destination." at `Message_Speed`
//!                (three frames a character, one while a button is held,
//!                `:141913-141917`), then a 60-frame wait, then `World_Index`
//!                is written from the row (`:133677`) and the flight starts
//! ```
//!
//! The scene blocks on `SceneOp::DestinationMenu` while this mode owns the
//! pad; the answer reaches it through `Runtime::answer_destination`, which
//! writes `World_Index` for a confirm and nothing for a Cancel. The window the
//! shell draws is the [`DestinationView`].
//!
//! The teardown after the answer takes frames in which the cartridge reads no
//! pad: 55 after a Cancel and 25 after the confirm wait, measured on the oracle
//! (`oracle/tapes/35_ship_destination_menu.tape`), during which the view's
//! phase is [`DestinationPhase::Closing`].
//!
//! Not modelled: the windows' open animation (`Window_Draw`, `:139785-139836`),
//! during which the cartridge does not read the pad, and the `Message_Speed`
//! option (the default of 2 is used; `ps4.asm:88668`).

use psiv_core::{DestinationMask, Flag, Input, destination_mask_byte, destination_worlds};
use psiv_data::{COVER_AIR_CASTLE, COVER_KURAN, COVER_RYKROS, ShipPalette};

use crate::Runtime;
use crate::pad::{Button, Pad};

use super::{Frame, FrameMode, Session};

/// `SFXID_MovingCursor` (`ps4.constants.asm:1038`).
const SFX_MOVING_CURSOR: u8 = 0xF2;
/// `SFXID_Selection` (`:1039`).
const SFX_SELECTION: u8 = 0xF3;
/// `Sound_StopSpcSFX` (`:1057`), written once the message is typed
/// (`ps4.asm:133795`).
const SOUND_STOP_SPC_SFX: u8 = 0xFD;
/// `EventFlag_Rykros` (`ps4.constants.asm:1636`).
const FLAG_RYKROS: u16 = 0xD0;
/// Mask bit 5: the Rykros row (`btst #5, ($FFFFED42).w`, `ps4.asm:133887`).
const MASK_RYKROS: u8 = 0x20;
/// Mask bit 3: Kuran (`:133898`).
const MASK_KURAN: u8 = 0x08;
/// Mask bit 2: the Air Castle (`:133914`).
const MASK_AIR_CASTLE: u8 = 0x04;
/// The default `Message_Speed` (`ps4.asm:88668`): the loop after each typed
/// character runs `d0 + 1` frames.
const MESSAGE_SPEED: u8 = 2;
/// The wait after the message, `moveq #$3B, d0` and a `dbf` loop
/// (`ps4.asm:133797-133803`).
const CONFIRM_WAIT_FRAMES: u16 = 60;
/// Frames from a Cancel press to the scene's return, measured on the oracle
/// (tape 35, Speak/B at frame 7401: the map reload begins at 7456): the two
/// `Window_Destroy` calls, `PalFadeOut_ClrSpriteTbl`, `Panel_DestroyAll` and
/// `Map_LoadChunks` (`ps4.asm:133692-133710`). The cartridge reads no pad.
const CANCEL_CLOSE_FRAMES: u16 = 55;
/// Frames from the end of the confirm wait to the takeoff's map load, measured
/// on the same tape (a Zelan row: 173 frames from the press, of which the 29
/// typed characters take 87 and the wait 60; Motavia's 31 characters: 179):
/// the same teardown, `ps4.asm:133805-133833`.
const CONFIRM_CLOSE_FRAMES: u16 = 25;
/// `FieldObj_RedCursor`'s timers (`ps4.asm:141817-141843`): the first update
/// leaves `$19`, a flip reloads `$F` and an invisible stretch adds `$A`.
const CURSOR_START_TIMER: i16 = 0x19;
const CURSOR_FLIP_TIMER: i16 = 0x0F;
const CURSOR_INVISIBLE_EXTRA: i16 = 0x0A;

/// Where the menu is in its run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationPhase {
    /// The list is up and the pad moves the cursor.
    Choosing,
    /// A row was confirmed: the message is typing, then the wait runs. The
    /// cursor box is stamped solid (`loc_69A32` writes `$C6E8`).
    Confirmed,
    /// The answer is given and the cartridge is tearing the windows down and
    /// fading out; it reads no pad. The shell keeps drawing what it drew.
    Closing,
}

/// Everything the destination screen draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationView {
    /// The worlds listed, top to bottom: `World_Index` values.
    pub rows: Vec<u8>,
    /// The cursor row (`Window_Option_Index`).
    pub cursor: usize,
    /// Choosing or confirmed.
    pub phase: DestinationPhase,
    /// The row mask (`$FFFFED42`).
    pub mask: u8,
    /// `EventFlag_Rykros`: with mask bit 5 it decides whether the Rykros
    /// marker is covered.
    pub rykros_known: bool,
    /// `Main_Frame_Count` bit 3 on the frame the view was made: the marker of
    /// the cursor row is lit.
    pub blink: bool,
    /// Whether the red cursor object is in its visible half.
    pub cursor_visible: bool,
    /// The confirm message typed so far, lines joined with `\n`. Empty while
    /// choosing.
    pub typed: String,
}

impl DestinationView {
    /// Whether the Rykros marker is covered: `loc_63F2C` skips the cover only
    /// when the flag is set and mask bit 5 names the row (`ps4.asm:133781-133787`).
    #[must_use]
    pub fn rykros_covered(&self) -> bool {
        !(self.rykros_known && self.mask & MASK_RYKROS != 0)
    }

    /// The pack background to draw: one cover bit per marker the mask hides.
    #[must_use]
    pub fn covers(&self) -> usize {
        let mut covers = 0;
        if self.rykros_covered() {
            covers |= COVER_RYKROS;
        }
        if self.mask & MASK_KURAN == 0 {
            covers |= COVER_KURAN;
        }
        if self.mask & MASK_AIR_CASTLE == 0 {
            covers |= COVER_AIR_CASTLE;
        }
        covers
    }

    /// The palette state `loc_64144` leaves (`ps4.asm:133894-133925`).
    #[must_use]
    pub fn palette(&self) -> ShipPalette {
        ShipPalette {
            dark: self.rykros_covered(),
            highlight_world: self
                .blink
                .then(|| self.rows.get(self.cursor).copied())
                .flatten(),
        }
    }

    /// The world under the cursor.
    #[must_use]
    pub fn selected_world(&self) -> Option<u8> {
        self.rows.get(self.cursor).copied()
    }
}

/// What a frame of the menu decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DestinationOutcome {
    /// Still up.
    Open,
    /// A row was confirmed and its wait is over.
    Chosen(u8),
    /// Cancel.
    Cancelled,
}

/// The mode's state: the view and the counters the view does not show.
#[derive(Clone, Debug)]
pub(crate) struct DestinationMode {
    view: DestinationView,
    /// The cursor object's timer.
    cursor_timer: i16,
    /// The whole confirm message, `\n` for `$FC`.
    message: Vec<char>,
    /// Characters typed so far.
    typed: usize,
    /// Frames left of the current character's wait.
    char_wait: u8,
    /// Frames left of the 60-frame wait, once the text is done.
    tail: Option<u16>,
    /// The teardown after the answer: frames left, and the world confirmed
    /// (`None` for a Cancel).
    closing: Option<(u16, Option<u8>)>,
}

impl DestinationMode {
    /// Builds the list as `loc_63BC4` does (`ps4.asm:133499-133535`).
    pub(crate) fn open(runtime: &Runtime, mask: DestinationMask) -> DestinationMode {
        let mask = destination_mask_byte(mask, |flag| runtime.game().is_set(flag));
        let rows = destination_worlds(mask, runtime.world_index());
        let rykros_known = runtime.game().is_set(Flag::event(FLAG_RYKROS));
        DestinationMode {
            view: DestinationView {
                rows,
                cursor: 0,
                phase: DestinationPhase::Choosing,
                mask,
                rykros_known,
                blink: false,
                cursor_visible: true,
                typed: String::new(),
            },
            cursor_timer: CURSOR_START_TIMER,
            message: Vec::new(),
            typed: 0,
            char_wait: 0,
            tail: None,
            closing: None,
        }
    }

    pub(crate) fn view(&self) -> &DestinationView {
        &self.view
    }

    /// One frame. `pad` is the held byte, `pressed` its new edges; the sound is
    /// the effect the frame asked for.
    pub(crate) fn frame(
        &mut self,
        runtime: &Runtime,
        pad: Pad,
        pressed: Pad,
    ) -> (DestinationOutcome, Option<u8>) {
        match self.view.phase {
            DestinationPhase::Choosing => self.choose(runtime, pressed),
            DestinationPhase::Confirmed => (self.confirmed_frame(pad), None),
            DestinationPhase::Closing => (self.closing_frame(), None),
        }
    }

    fn choose(&mut self, runtime: &Runtime, pressed: Pad) -> (DestinationOutcome, Option<u8>) {
        let any_of = |buttons: &[Button]| buttons.iter().any(|&button| pressed.held(button));
        if any_of(&[Button::Cancel, Button::Speak, Button::Camp]) {
            // `btst #4` tests Cancel first (`ps4.asm:133617`).
            if pressed.held(Button::Cancel) {
                self.view.phase = DestinationPhase::Closing;
                self.closing = Some((CANCEL_CLOSE_FRAMES, None));
                return (DestinationOutcome::Open, None);
            }
            // With no row there is nothing to confirm: the cartridge would
            // read past its row list. Only Cancel leaves such a menu.
            let Some(world) = self.view.selected_world() else {
                return (DestinationOutcome::Open, None);
            };
            self.view.phase = DestinationPhase::Confirmed;
            self.message = confirm_message(runtime, world);
            return (DestinationOutcome::Open, Some(SFX_SELECTION));
        }
        // `Win_UpdateCursorUpDown` runs the cursor object first (`loc_69F3C`),
        // then reads Up and Down.
        self.update_cursor_object();
        // `loc_64144` re-uploads the palette and reads `Main_Frame_Count`
        // only on frames the loop did not exit: once a button is pressed the
        // marker stays as that last upload left it.
        self.view.blink = runtime.main_frame_count() & 8 != 0;
        let count = self.view.rows.len();
        let mut sound = None;
        if count > 0 {
            if pressed.held(Button::Up) {
                self.view.cursor = (self.view.cursor + count - 1) % count;
                sound = Some(SFX_MOVING_CURSOR);
                self.reset_cursor_object();
            }
            if pressed.held(Button::Down) {
                self.view.cursor = (self.view.cursor + 1) % count;
                sound = Some(SFX_MOVING_CURSOR);
                self.reset_cursor_object();
            }
        }
        (DestinationOutcome::Open, sound)
    }

    /// `RedCursor_Main` (`ps4.asm:141836-141843`).
    fn update_cursor_object(&mut self) {
        self.cursor_timer -= 1;
        if self.cursor_timer < 0 {
            self.cursor_timer = CURSOR_FLIP_TIMER;
            self.view.cursor_visible = !self.view.cursor_visible;
            if !self.view.cursor_visible {
                self.cursor_timer += CURSOR_INVISIBLE_EXTRA;
            }
        }
    }

    /// The Up/Down handlers' `move.w #$19, $1C(a4); bclr #1, $2(a4)`.
    fn reset_cursor_object(&mut self) {
        self.cursor_timer = CURSOR_START_TIMER;
        self.view.cursor_visible = true;
    }

    /// The typed message and the wait behind it.
    fn confirmed_frame(&mut self, pad: Pad) -> DestinationOutcome {
        if let Some(left) = self.tail {
            if left <= 1 {
                self.view.phase = DestinationPhase::Closing;
                self.closing = Some((CONFIRM_CLOSE_FRAMES, self.view.selected_world()));
                return DestinationOutcome::Open;
            }
            self.tail = Some(left - 1);
            return DestinationOutcome::Open;
        }
        if self.char_wait == 0 {
            if self.typed >= self.message.len() {
                // `$FF` ends the text; `Sound_StopSpcSFX`, then the wait.
                self.tail = Some(CONFIRM_WAIT_FRAMES);
                return DestinationOutcome::Open;
            }
            self.type_next_character(pad);
        }
        self.char_wait = self.char_wait.saturating_sub(1);
        DestinationOutcome::Open
    }

    /// The teardown: nothing is read, and the answer lands when it ends.
    fn closing_frame(&mut self) -> DestinationOutcome {
        let Some((left, answer)) = self.closing else {
            return DestinationOutcome::Cancelled;
        };
        let left = left.saturating_sub(1);
        if left > 0 {
            self.closing = Some((left, answer));
            return DestinationOutcome::Open;
        }
        answer.map_or(DestinationOutcome::Cancelled, DestinationOutcome::Chosen)
    }

    /// Writes one character, as `LoadWindowTiles` does, and starts its wait.
    /// A newline (`$FC`) costs no frames.
    fn type_next_character(&mut self, pad: Pad) {
        let held_mask = [Button::Cancel, Button::Speak, Button::Camp, Button::Start];
        while let Some(&character) = self.message.get(self.typed) {
            self.typed += 1;
            self.view.typed.push(character);
            if character == '\n' {
                continue;
            }
            // `Joypad_Held & (Cancel|Speak|Camp|Start)` is read once per
            // character: held, the loop runs once; otherwise `Message_Speed + 1`.
            let held = held_mask.iter().any(|&button| pad.held(button));
            self.char_wait = if held { 1 } else { MESSAGE_SPEED + 1 };
            return;
        }
    }
}

/// "NAME will be" and "the destination.": `loc_2AAA7A`'s name then
/// `loc_2AAA60` (`ps4.asm:133778-133793`), the two lines `$FC` apart. A pack
/// without the screen types nothing.
fn confirm_message(runtime: &Runtime, world: u8) -> Vec<char> {
    let Some(menu) = runtime.data().ship_menu() else {
        return Vec::new();
    };
    let Some(name) = menu.name(world) else {
        return Vec::new();
    };
    let mut text = String::from(name);
    for (index, line) in menu.confirm_suffix().iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(line);
    }
    text.chars().collect()
}

impl Session {
    /// Opens the menu for a scene's `SceneOp::DestinationMenu`. A mode is
    /// already up only if a scene asked twice, which the runner cannot do: it
    /// blocks on the first request.
    pub(super) fn open_destination(&mut self, mask: DestinationMask) {
        self.destination = Some(DestinationMode::open(&self.runtime, mask));
    }

    /// One frame of the destination menu.
    pub(crate) fn destination_frame(&mut self, pad: Pad, pressed: Pad) -> Frame {
        let mode = self
            .destination
            .as_mut()
            .expect("a destination frame has a mode");
        let (outcome, sound) = mode.frame(&self.runtime, pad, pressed);
        let sound = sound.or_else(|| {
            // `Sound_StopSpcSFX` goes out once, on the frame the wait begins.
            (mode.tail == Some(CONFIRM_WAIT_FRAMES)).then_some(SOUND_STOP_SPC_SFX)
        });
        match outcome {
            DestinationOutcome::Open => {}
            DestinationOutcome::Chosen(world) => {
                self.destination = None;
                self.runtime.answer_destination(Some(world));
            }
            DestinationOutcome::Cancelled => {
                self.destination = None;
                self.runtime.answer_destination(None);
            }
        }
        // The scene is blocked on the menu, so the field is parked as it is
        // behind any window; the tick spends the frame (and, on the frame the
        // menu answers, lets the scene take its answer).
        self.runtime.set_field_suspended(self.destination.is_some());
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::Destination,
            events,
            routed,
            sound,
            ..Frame::default()
        }
    }

    /// The destination menu's window, while it is up.
    #[must_use]
    pub fn destination_view(&self) -> Option<&DestinationView> {
        self.destination.as_ref().map(DestinationMode::view)
    }
}
