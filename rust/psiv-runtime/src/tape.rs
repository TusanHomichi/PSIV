//! The pad tape: every frame's joypad byte of one run, so it can be replayed.
//!
//! A run is a pure function of its starting point and the pad byte of every
//! frame ([`crate::Session::frame`] takes nothing else), so the tape is
//! the whole record. Campaign and the native Godot feed share this codec.
//!
//! # Format (version 1)
//!
//! Plain text, one record per line, so a tape diffs and greps:
//!
//! ```text
//! psiv-tape 1
//! start new-game
//! frames 123456
//! pads
//! 00 120
//! 20 1
//! 00 3
//! ```
//!
//! * line 1 is the magic and the version;
//! * `start new-game` begins at power-on (the title's START: the new-game
//!   initialiser and the opening event); `start save <fnv64>` begins from a
//!   slot file whose bytes hash to that FNV-1a 64 value (hex), which the
//!   replayer checks before it plays;
//! * `frames N` is the number of frames the runs add up to;
//! * every line after `pads` is `<hex byte> <count>`: that joypad byte, held
//!   for that many consecutive frames. The byte is the cartridge's own bit
//!   order ([`crate::Pad::bits`]: Up 0x01, Down 0x02, Left 0x04, Right
//!   0x08, Cancel 0x10, Speak 0x20, Camp 0x40, Start 0x80).

use std::fmt;

/// Where a tape's first frame starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapeStart {
    /// Power-on: the title's START and the opening event.
    NewGame,
    /// A slot file, named by the FNV-1a 64 hash of its bytes.
    Save {
        /// FNV-1a 64 of the slot file.
        hash: u64,
    },
}

/// A recorded run's pads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tape {
    /// Where frame 0 starts.
    pub start: TapeStart,
    /// One joypad byte per frame.
    pub pads: Vec<u8>,
}

/// Why a tape could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeError(pub String);

impl fmt::Display for TapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad tape: {}", self.0)
    }
}

impl std::error::Error for TapeError {}

/// FNV-1a 64 over `bytes`: the hash a tape names its starting save with.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl Tape {
    /// An empty tape from `start`.
    #[must_use]
    pub fn new(start: TapeStart) -> Tape {
        Tape {
            start,
            pads: Vec::new(),
        }
    }

    /// The tape's text form.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("psiv-tape 1\n");
        match &self.start {
            TapeStart::NewGame => out.push_str("start new-game\n"),
            TapeStart::Save { hash } => out.push_str(&format!("start save {hash:016x}\n")),
        }
        out.push_str(&format!("frames {}\npads\n", self.pads.len()));
        let mut at = 0;
        while at < self.pads.len() {
            let byte = self.pads[at];
            let run = self.pads[at..].iter().take_while(|b| **b == byte).count();
            out.push_str(&format!("{byte:02x} {run}\n"));
            at += run;
        }
        out
    }

    /// Reads a tape from its text form.
    ///
    /// # Errors
    ///
    /// [`TapeError`] for a wrong magic, a bad header line, a malformed run or
    /// a frame count that does not match the runs.
    pub fn parse(text: &str) -> Result<Tape, TapeError> {
        let mut lines = text.lines();
        let bad = |what: &str| TapeError(what.to_owned());
        if lines.next() != Some("psiv-tape 1") {
            return Err(bad("first line is not `psiv-tape 1`"));
        }
        let start_line = lines.next().ok_or_else(|| bad("missing start line"))?;
        let start = match start_line.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["start", "new-game"] => TapeStart::NewGame,
            ["start", "save", hash] => TapeStart::Save {
                hash: u64::from_str_radix(hash, 16).map_err(|_| bad("bad save hash"))?,
            },
            _ => return Err(bad("bad start line")),
        };
        let frames: usize = lines
            .next()
            .and_then(|l| l.strip_prefix("frames "))
            .and_then(|n| n.trim().parse().ok())
            .ok_or_else(|| bad("bad frames line"))?;
        if lines.next() != Some("pads") {
            return Err(bad("missing `pads` line"));
        }
        // Do not reserve an untrusted header's count before checking its runs.
        let mut pads = Vec::new();
        for line in lines {
            let (byte, count) = line
                .split_once(' ')
                .ok_or_else(|| bad("run line is not `<hex> <count>`"))?;
            let byte = u8::from_str_radix(byte, 16).map_err(|_| bad("bad pad byte"))?;
            let count: usize = count.parse().map_err(|_| bad("bad run count"))?;
            if count == 0 || count > frames.saturating_sub(pads.len()) {
                return Err(bad("run count exceeds frames or is zero"));
            }
            pads.extend(std::iter::repeat_n(byte, count));
        }
        if pads.len() != frames {
            return Err(TapeError(format!(
                "header says {frames} frames, the runs hold {}",
                pads.len()
            )));
        }
        Ok(Tape { start, pads })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tape_round_trips_through_its_text_form() {
        let tape = Tape {
            start: TapeStart::Save { hash: 0xdead_beef },
            pads: vec![0, 0, 0, 0x20, 0, 0x01, 0x01],
        };
        let text = tape.render();
        assert!(text.contains("00 3\n20 1\n00 1\n01 2\n"));
        assert_eq!(Tape::parse(&text), Ok(tape));
        let new_game = Tape::new(TapeStart::NewGame);
        assert_eq!(Tape::parse(&new_game.render()), Ok(new_game));
    }

    #[test]
    fn a_damaged_tape_is_refused() {
        let tape = Tape {
            start: TapeStart::NewGame,
            pads: vec![1, 1, 2],
        };
        let text = tape.render();
        assert!(Tape::parse(&text.replace("frames 3", "frames 4")).is_err());
        assert!(Tape::parse(&text.replace("psiv-tape 1", "psiv-tape 2")).is_err());
        assert!(Tape::parse(&text.replace("01 2", "zz 2")).is_err());
        assert!(Tape::parse(&text.replace("01 2", "01 0")).is_err());
        assert!(Tape::parse(&text.replace("frames 3", "frames 18446744073709551615")).is_err());
        assert!(Tape::parse("").is_err());
    }

    #[test]
    fn fnv_matches_its_published_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
