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

use psiv_core::{Direction, Input};

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

    /// The field-mode input this pad resolves to, in the cartridge's order.
    ///
    /// The talk button wins over a direction: `FieldControls_GetInput`
    /// (`ps4.asm:114889`) reads `ButtonSpeak` out of `Joypad_Pressed` and hands
    /// the frame to `FieldRoutine_Interaction`, which the engine models by
    /// latching the press and spending it when the party comes to rest
    /// (`psiv-core/src/field.rs`, `FieldState::tick`). It is the held level that
    /// resolves here, not the edge, so a press held across frames is one
    /// interaction — and so the dismiss latch, which compares this answer to
    /// [`Input::Action`], keeps swallowing while the key is still down.
    ///
    /// The d-pad is `FieldObj_GetInput` (`ps4.asm:93212`), which hands
    /// `FieldObj_Move` (`ps4.asm:93352`) the raw four-bit mask and lets
    /// `FieldObj_MovementsTbl` (`ps4.asm:93675`) choose. Its first sixteen
    /// entries are the whole rule:
    ///
    /// | mask | buttons | direction |
    /// | --- | --- | --- |
    /// | `$01` | Up | Up |
    /// | `$02` | Down | Down |
    /// | `$03` | Up+Down | — |
    /// | `$04`, `$05`, `$06` | Left, with Up and/or Down | Left |
    /// | `$07` | Up+Down+Left | — |
    /// | `$08`, `$09`, `$0A` | Right, with Up and/or Down | Right |
    /// | `$0B` | Up+Down+Right | — |
    /// | `$0C`..`$0F` | Left+Right | — |
    ///
    /// So an opposing pair cancels the whole mask — including a perfectly good
    /// horizontal or vertical held with it — and a held horizontal otherwise
    /// beats a vertical. That is not "Up first", which is what the shell's own
    /// `read_input` did before this resolution moved here.
    #[must_use]
    pub const fn field_input(self) -> Input {
        if self.held(Button::Speak) {
            return Input::Action;
        }
        let (up, down) = (self.held(Button::Up), self.held(Button::Down));
        let (left, right) = (self.held(Button::Left), self.held(Button::Right));
        // `$03`, `$07`, `$0B`, `$0F` and every `$0C` to `$0F`: an opposing pair
        // is a mask the cartridge does nothing for — not even a facing change,
        // since the table leaves its facing byte at `$FF`.
        if (up && down) || (left && right) {
            return Input::Neutral;
        }
        if left {
            return Input::Direction(Direction::Left);
        }
        if right {
            return Input::Direction(Direction::Right);
        }
        if up {
            return Input::Direction(Direction::Up);
        }
        if down {
            return Input::Direction(Direction::Down);
        }
        Input::Neutral
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
    use psiv_core::{Direction, Input};

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

    /// Every one of the sixteen d-pad masks, against `FieldObj_MovementsTbl`
    /// (`ps4.asm:93675`): the entries with zero step durations and a `$FF`
    /// facing byte are the masks the cartridge does nothing for. This is the
    /// table, not a preference — a menu that reorders these is a fidelity bug.
    #[test]
    fn the_cartridges_mask_table_decides_every_direction() {
        use Direction::{Down, Left, Right, Up};
        let table: [(u8, Option<Direction>); 16] = [
            (0x00, None),
            (0x01, Some(Up)),
            (0x02, Some(Down)),
            (0x03, None),
            (0x04, Some(Left)),
            (0x05, Some(Left)),
            (0x06, Some(Left)),
            (0x07, None),
            (0x08, Some(Right)),
            (0x09, Some(Right)),
            (0x0A, Some(Right)),
            (0x0B, None),
            (0x0C, None),
            (0x0D, None),
            (0x0E, None),
            (0x0F, None),
        ];
        for (mask, expected) in table {
            let resolved = Pad::from_bits(mask).field_input();
            let expected = expected.map_or(Input::Neutral, Input::Direction);
            assert_eq!(resolved, expected, "mask {mask:#04x}");
        }
    }

    /// The two ends of the table that a "first button wins" rule gets wrong:
    /// a horizontal beats a vertical, and an opposing pair cancels the mask
    /// even when a single direction is held with it.
    #[test]
    fn a_horizontal_beats_a_vertical_and_an_opposing_pair_cancels() {
        let up_left = Pad::of(&[Button::Up, Button::Left]);
        assert_eq!(up_left.field_input(), Input::Direction(Direction::Left));
        assert_eq!(
            Pad::of(&[Button::Down, Button::Right]).field_input(),
            Input::Direction(Direction::Right)
        );
        assert_eq!(
            Pad::of(&[Button::Up, Button::Down, Button::Left]).field_input(),
            Input::Neutral,
            "$07 is an entry the table leaves empty"
        );
        assert_eq!(
            Pad::of(&[Button::Left, Button::Right]).field_input(),
            Input::Neutral
        );
    }

    /// The talk button is not a direction and not a tie-break: it takes the
    /// frame, and it is read from the held set, not the press edge.
    #[test]
    fn the_talk_button_takes_the_frame_from_any_direction() {
        let both = Pad::of(&[Button::Speak, Button::Down]);
        assert_eq!(both.field_input(), Input::Action);
        assert_eq!(both.field_input(), both.with(Button::Up).field_input());
        assert_eq!(Pad::new(Button::Speak).field_input(), Input::Action);
        // Cancel, Camp and Start are not field input: the engine's `Input` has
        // no notion of them, and the shell reads Cancel for the camp itself.
        for button in [Button::Cancel, Button::Camp, Button::Start] {
            assert_eq!(Pad::new(button).field_input(), Input::Neutral);
        }
    }
}
