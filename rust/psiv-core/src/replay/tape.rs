//! The oracle's input tapes: what was held, for how long, and where the marks
//! are.
//!
//! Pure text in, frames out. Nothing here knows about the engine — a tape is
//! the *input* side of a replay, and keeping it free of engine types is what
//! lets tape parsing be tested without building a map.

use core::fmt;

use crate::field::Input;
use crate::geom::Direction;

/// The four directions plus the four Mega Drive buttons, as a tape spells them.
///
/// The mapping is not the obvious one and the oracle's own notes call it a
/// trap: Genesis **C** is `ButtonSpeak` (talk), **B** is `ButtonCancel` (inert
/// in field control), **A** is `ButtonCamp`. Bit order here is the tape's
/// letter order `UDLRABCS`, not the joypad's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buttons(u8);

impl Buttons {
    /// Nothing held.
    pub const NONE: Buttons = Buttons(0);

    const UP: u8 = 1 << 0;
    const DOWN: u8 = 1 << 1;
    const LEFT: u8 = 1 << 2;
    const RIGHT: u8 = 1 << 3;
    /// Genesis A — `ButtonCamp`.
    const A: u8 = 1 << 4;
    /// Genesis B — `ButtonCancel`, inert in field control.
    const B: u8 = 1 << 5;
    /// Genesis C — `ButtonSpeak`, the talk button.
    const C: u8 = 1 << 6;
    const START: u8 = 1 << 7;

    /// Parses a tape's button field: `.` for nothing, else letters from
    /// `UDLRABCS`.
    ///
    /// # Errors
    ///
    /// [`TapeError::BadButton`] for any other character.
    pub fn parse(text: &str) -> Result<Buttons, TapeError> {
        if text == "." {
            return Ok(Buttons::NONE);
        }
        let mut bits = 0;
        for ch in text.chars() {
            bits |= match ch {
                'U' => Buttons::UP,
                'D' => Buttons::DOWN,
                'L' => Buttons::LEFT,
                'R' => Buttons::RIGHT,
                'A' => Buttons::A,
                'B' => Buttons::B,
                'C' => Buttons::C,
                'S' => Buttons::START,
                other => return Err(TapeError::BadButton(other)),
            };
        }
        Ok(Buttons(bits))
    }

    /// The tape spelling, for round-tripping into a log's `buttons` column.
    #[must_use]
    pub fn to_tape(self) -> String {
        if self.0 == 0 {
            return ".".to_string();
        }
        let mut out = String::new();
        for (bit, ch) in [
            (Buttons::UP, 'U'),
            (Buttons::DOWN, 'D'),
            (Buttons::LEFT, 'L'),
            (Buttons::RIGHT, 'R'),
            (Buttons::A, 'A'),
            (Buttons::B, 'B'),
            (Buttons::C, 'C'),
            (Buttons::START, 'S'),
        ] {
            if self.0 & bit != 0 {
                out.push(ch);
            }
        }
        out
    }

    /// The direction held, if any.
    ///
    /// A tape may in principle hold two; the engine takes one per tick, so the
    /// order here is a declared tie-break rather than a discovered rule. No
    /// retail field tape holds two at once.
    #[must_use]
    pub const fn direction(self) -> Option<Direction> {
        if self.0 & Buttons::UP != 0 {
            Some(Direction::Up)
        } else if self.0 & Buttons::DOWN != 0 {
            Some(Direction::Down)
        } else if self.0 & Buttons::LEFT != 0 {
            Some(Direction::Left)
        } else if self.0 & Buttons::RIGHT != 0 {
            Some(Direction::Right)
        } else {
            None
        }
    }

    /// Whether the talk button is held. Genesis **C**, not B.
    #[must_use]
    pub const fn talk(self) -> bool {
        self.0 & Buttons::C != 0
    }

    /// The engine input for this frame.
    ///
    /// Talk wins over a direction, matching the cartridge: the two are read
    /// through different paths and `FieldControls_GetInput` hands the frame to
    /// the interaction routine before the movement code runs.
    #[must_use]
    pub const fn to_input(self) -> Input {
        if self.talk() {
            return Input::Action;
        }
        match self.direction() {
            Some(dir) => Input::Direction(dir),
            None => Input::Neutral,
        }
    }
}

/// Why a tape would not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapeError {
    /// A button letter outside `UDLRABCS`.
    BadButton(char),
    /// A frame count that was not a positive integer.
    BadFrameCount(String),
    /// A line with too few fields.
    Malformed(String),
    /// `end` without a `repeat`, or a `repeat` inside a `repeat`.
    BadRepeat(String),
    /// A `repeat` block that never closed.
    UnclosedRepeat,
}

impl fmt::Display for TapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapeError::BadButton(ch) => write!(f, "{ch:?} is not one of UDLRABCS"),
            TapeError::BadFrameCount(text) => write!(f, "{text:?} is not a frame count"),
            TapeError::Malformed(line) => write!(f, "malformed tape line: {line:?}"),
            TapeError::BadRepeat(line) => write!(f, "bad repeat structure at {line:?}"),
            TapeError::UnclosedRepeat => write!(f, "a repeat block was never closed"),
        }
    }
}

impl core::error::Error for TapeError {}

/// One step: a button state held for some frames, optionally labelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeStep {
    /// How many consecutive frames, at least 1.
    pub frames: u32,
    /// What is held.
    pub buttons: Buttons,
    /// A label, recorded on the step's first frame.
    pub mark: Option<String>,
}

/// One frame of a replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeFrame {
    /// 1-based, counting from power-on exactly as the oracle's logs do.
    pub number: u32,
    /// The mark, on a step's first frame only.
    pub mark: Option<String>,
    /// What is held.
    pub buttons: Buttons,
}

/// A parsed tape.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tape {
    steps: Vec<TapeStep>,
}

impl Tape {
    /// Parses tape text.
    ///
    /// Blank lines and `#` comments are ignored; `repeat <n>` / `end` blocks
    /// expand in place and do not nest.
    ///
    /// # Errors
    ///
    /// See [`TapeError`].
    pub fn parse(text: &str) -> Result<Tape, TapeError> {
        let mut steps = Vec::new();
        let mut repeat: Option<(u32, Vec<TapeStep>)> = None;

        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            if let Some(count) = line.strip_prefix("repeat ") {
                if repeat.is_some() {
                    return Err(TapeError::BadRepeat(line.to_string()));
                }
                let count = count
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| TapeError::BadFrameCount(count.trim().to_string()))?;
                repeat = Some((count, Vec::new()));
                continue;
            }
            if line == "end" {
                let Some((count, body)) = repeat.take() else {
                    return Err(TapeError::BadRepeat(line.to_string()));
                };
                for _ in 0..count {
                    steps.extend(body.iter().cloned());
                }
                continue;
            }

            let mut fields = line.split_whitespace();
            let (Some(frames), Some(buttons)) = (fields.next(), fields.next()) else {
                return Err(TapeError::Malformed(line.to_string()));
            };
            let frames = frames
                .parse::<u32>()
                .map_err(|_| TapeError::BadFrameCount(frames.to_string()))?;
            if frames == 0 {
                return Err(TapeError::BadFrameCount(frames.to_string()));
            }
            let step = TapeStep {
                frames,
                buttons: Buttons::parse(buttons)?,
                mark: fields.next().map(str::to_string),
            };
            match repeat.as_mut() {
                Some((_, body)) => body.push(step),
                None => steps.push(step),
            }
        }

        if repeat.is_some() {
            return Err(TapeError::UnclosedRepeat);
        }
        Ok(Tape { steps })
    }

    /// The steps, expanded.
    #[must_use]
    pub fn steps(&self) -> &[TapeStep] {
        &self.steps
    }

    /// Total frames.
    #[must_use]
    pub fn frame_count(&self) -> u32 {
        self.steps.iter().map(|step| step.frames).sum()
    }

    /// Every frame, numbered from 1 as the oracle numbers `retro_run()` calls.
    #[must_use]
    pub fn frames(&self) -> Vec<TapeFrame> {
        let mut out = Vec::with_capacity(self.frame_count() as usize);
        let mut number = 1;
        for step in &self.steps {
            for index in 0..step.frames {
                out.push(TapeFrame {
                    number,
                    mark: if index == 0 { step.mark.clone() } else { None },
                    buttons: step.buttons,
                });
                number += 1;
            }
        }
        out
    }

    /// The frame a mark lands on.
    #[must_use]
    pub fn mark_frame(&self, mark: &str) -> Option<u32> {
        let mut number = 1;
        for step in &self.steps {
            if step.mark.as_deref() == Some(mark) {
                return Some(number);
            }
            number += step.frames;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Input;
    use crate::geom::Direction;

    #[test]
    fn buttons_parse_and_round_trip() {
        assert_eq!(Buttons::parse(".").unwrap(), Buttons::NONE);
        assert_eq!(Buttons::parse(".").unwrap().to_tape(), ".");
        let held = Buttons::parse("DC").unwrap();
        assert_eq!(held.direction(), Some(Direction::Down));
        assert!(held.talk());
        assert_eq!(held.to_tape(), "DC");
        assert_eq!(Buttons::parse("X"), Err(TapeError::BadButton('X')));
    }

    #[test]
    fn genesis_c_is_the_talk_button_not_b() {
        // The oracle's own notes call this a trap: C is ButtonSpeak, B is
        // ButtonCancel and does nothing in field control.
        assert_eq!(Buttons::parse("C").unwrap().to_input(), Input::Action);
        assert_eq!(Buttons::parse("B").unwrap().to_input(), Input::Neutral);
        assert_eq!(Buttons::parse("A").unwrap().to_input(), Input::Neutral);
    }

    #[test]
    fn talk_wins_over_a_direction() {
        assert_eq!(Buttons::parse("RC").unwrap().to_input(), Input::Action);
        assert_eq!(
            Buttons::parse("R").unwrap().to_input(),
            Input::Direction(Direction::Right)
        );
    }

    #[test]
    fn a_tape_expands_repeat_blocks() {
        let tape = Tape::parse(
            "# a comment\n\
             2 . start\n\
             repeat 3\n\
             1 D\n\
             2 .\n\
             end\n\
             1 R done\n",
        )
        .unwrap();

        assert_eq!(tape.steps().len(), 1 + 3 * 2 + 1);
        assert_eq!(tape.frame_count(), 2 + 3 * 3 + 1);
        assert_eq!(tape.mark_frame("start"), Some(1));
        assert_eq!(tape.mark_frame("done"), Some(12));
        assert_eq!(tape.mark_frame("nope"), None);
    }

    #[test]
    fn frames_number_from_one_and_carry_marks_on_the_first_frame_only() {
        let tape = Tape::parse("3 D walk\n1 .\n").unwrap();
        let frames = tape.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].number, 1);
        assert_eq!(frames[0].mark.as_deref(), Some("walk"));
        assert_eq!(frames[1].mark, None, "only the first frame is marked");
        assert_eq!(frames[3].buttons, Buttons::NONE);
    }

    #[test]
    fn malformed_tapes_are_rejected() {
        assert!(matches!(
            Tape::parse("0 D\n"),
            Err(TapeError::BadFrameCount(_))
        ));
        assert!(matches!(Tape::parse("D\n"), Err(TapeError::Malformed(_))));
        assert!(matches!(Tape::parse("end\n"), Err(TapeError::BadRepeat(_))));
        assert!(matches!(
            Tape::parse("repeat 2\n1 .\n"),
            Err(TapeError::UnclosedRepeat)
        ));
        assert!(matches!(
            Tape::parse("repeat 2\nrepeat 2\n1 .\nend\nend\n"),
            Err(TapeError::BadRepeat(_))
        ));
    }
}
