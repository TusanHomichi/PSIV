//! The cartridge's joypad byte.
//!
//! `Pad` is the runtime's input: one byte per frame, exactly the eight bits
//! the cartridge reads out of the control port. The bit order is the
//! disassembly's, not the tape's:
//!
//! ```text
//! ButtonUp = 0  ButtonDown = 1  ButtonLeft  = 2  ButtonRight = 3
//! ButtonCancel = 4  ButtonSpeak = 5  ButtonCamp = 6  ButtonStart = 7
//! ```
//!
//! (`reference/ps4disasm/ps4.constants.asm:1877-1884`.) The tape spelling in
//! [`psiv_core::replay`] is a different mapping on purpose — Genesis **A** is
//! `ButtonCamp`, **B** is `ButtonCancel`, **C** is `ButtonSpeak` — and this
//! type is the hardware side of that pair, so a tape bit and a pad bit are
//! deliberately not the same number.
//!
//! The cartridge keeps two words: `Joypad_Held`, this frame's state, and
//! `Joypad_Pressed`, the buttons that went down since the previous read. The
//! field walks on the held bits (`FieldObj_GetInput` masks them down to the
//! d-pad) while the talk button comes out of the pressed word
//! (`FieldControls_GetInput`); the menus are edge-driven the same way. Keeping
//! last frame's pad and asking for [`Pad::pressed`] is that pair: the caller
//! owns the previous frame, the button set is this frame's edges.
//!
//! `Pad` is small, `Copy` and free of engine types: it is the only input type
//! a runtime session ever takes.

use core::fmt;

/// One of the pad's eight buttons, in the cartridge's bit order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Button {
    /// D-pad up, bit 0.
    Up,
    /// D-pad down, bit 1.
    Down,
    /// D-pad left, bit 2.
    Left,
    /// D-pad right, bit 3.
    Right,
    /// Genesis B, bit 4: retail's cancel — the yes/no window's direct NO.
    Cancel,
    /// Genesis C, bit 5: retail's talk/confirm button.
    Speak,
    /// Genesis A, bit 6.
    Camp,
    /// Start, bit 7.
    Start,
}

impl Button {
    /// Every button, in bit order.
    pub const ALL: [Button; 8] = [
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::Cancel,
        Button::Speak,
        Button::Camp,
        Button::Start,
    ];

    /// The bit this button occupies in the joypad byte.
    #[must_use]
    pub const fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// The disassembly's constant name, for logs and evidence.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Button::Up => "ButtonUp",
            Button::Down => "ButtonDown",
            Button::Left => "ButtonLeft",
            Button::Right => "ButtonRight",
            Button::Cancel => "ButtonCancel",
            Button::Speak => "ButtonSpeak",
            Button::Camp => "ButtonCamp",
            Button::Start => "ButtonStart",
        }
    }
}

impl fmt::Display for Button {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// The joypad byte: which buttons are held this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Pad(u8);

impl Pad {
    /// Nothing held.
    pub const NEUTRAL: Pad = Pad(0);

    /// A pad with `button` held.
    #[must_use]
    pub const fn new(button: Button) -> Pad {
        Pad(button.bit())
    }

    /// The pad holding every button in `buttons`.
    #[must_use]
    pub const fn of(buttons: &[Button]) -> Pad {
        let mut bits = 0;
        let mut index = 0;
        while index < buttons.len() {
            bits |= buttons[index].bit();
            index += 1;
        }
        Pad(bits)
    }

    /// The raw byte, in the cartridge's bit order.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// A pad from a raw byte, keeping only the eight button bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Pad {
        Pad(bits)
    }

    /// `Joypad_Held`: whether `button` is down this frame.
    #[must_use]
    pub const fn held(self, button: Button) -> bool {
        self.0 & button.bit() != 0
    }

    /// This pad plus `button`. Genesis keys repeat: a held button stays held.
    #[must_use]
    pub const fn with(self, button: Button) -> Pad {
        Pad(self.0 | button.bit())
    }

    /// This pad without `button`.
    #[must_use]
    pub const fn without(self, button: Button) -> Pad {
        Pad(self.0 & !button.bit())
    }

    /// Whether any button is down.
    #[must_use]
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    /// `Joypad_Pressed`: the buttons that went down between `previous` and
    /// this pad. A button that was already held is not pressed again — the
    /// edge the cartridge's menus and interactions act on.
    #[must_use]
    pub const fn pressed(self, previous: Pad) -> Pad {
        Pad(self.0 & !previous.0)
    }

    /// Whether `button` went down this frame.
    #[must_use]
    pub const fn is_pressed(self, previous: Pad, button: Button) -> bool {
        self.pressed(previous).held(button)
    }
}

impl fmt::Display for Pad {
    /// The held buttons by name, or `neutral`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return formatter.write_str("neutral");
        }
        let mut first = true;
        for button in Button::ALL {
            if !self.held(button) {
                continue;
            }
            if !first {
                formatter.write_str("|")?;
            }
            first = false;
            formatter.write_str(button.name())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, Pad};

    #[test]
    fn every_button_keeps_its_disassembly_bit() {
        assert_eq!(Button::Up.bit(), 0x01);
        assert_eq!(Button::Down.bit(), 0x02);
        assert_eq!(Button::Left.bit(), 0x04);
        assert_eq!(Button::Right.bit(), 0x08);
        assert_eq!(Button::Cancel.bit(), 0x10);
        assert_eq!(Button::Speak.bit(), 0x20);
        assert_eq!(Button::Camp.bit(), 0x40);
        assert_eq!(Button::Start.bit(), 0x80);
    }

    #[test]
    fn held_state_is_the_raw_byte_and_neutral_is_empty() {
        assert_eq!(Pad::NEUTRAL.bits(), 0);
        assert!(!Pad::NEUTRAL.any());
        let pad = Pad::new(Button::Speak).with(Button::Up).with(Button::Speak);
        assert_eq!(pad.bits(), 0x21);
        assert!(pad.held(Button::Speak));
        assert!(pad.held(Button::Up));
        assert!(!pad.held(Button::Cancel));
        assert_eq!(Pad::from_bits(pad.bits()), pad);
        assert_eq!(Pad::of(&[Button::Speak, Button::Up]), pad);
    }

    #[test]
    fn the_pressed_set_is_the_held_set_minus_the_previous_frame() {
        let up = Pad::new(Button::Up);
        let up_and_speak = up.with(Button::Speak);
        // A fresh press on a previously neutral pad is pressed.
        assert_eq!(up.pressed(Pad::NEUTRAL).bits(), up.bits());
        assert!(up.is_pressed(Pad::NEUTRAL, Button::Up));
        // Holding a button across frames is not a second press.
        assert_eq!(up.pressed(up), Pad::NEUTRAL);
        assert!(!up.is_pressed(up, Button::Up));
        // A second button going down while the first is held is one edge.
        assert_eq!(
            up_and_speak.pressed(up),
            Pad::new(Button::Speak),
            "only the newly down button is pressed"
        );
        assert!(up_and_speak.is_pressed(up, Button::Speak));
        assert!(!up_and_speak.is_pressed(up, Button::Up));
        // A release is not a press either.
        assert_eq!(Pad::NEUTRAL.pressed(up_and_speak), Pad::NEUTRAL);
    }

    #[test]
    fn a_pad_prints_its_held_buttons_by_name() {
        assert_eq!(Pad::NEUTRAL.to_string(), "neutral");
        assert_eq!(
            Pad::of(&[Button::Speak, Button::Right]).to_string(),
            "ButtonRight|ButtonSpeak"
        );
        assert_eq!(Button::Cancel.to_string(), "ButtonCancel");
    }
}
