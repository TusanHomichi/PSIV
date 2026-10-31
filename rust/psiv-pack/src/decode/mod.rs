//! The cartridge's three decompressors, and the table that decodes each
//! stream once per build.
//!
//! Each decoder is a transcription of its Python counterpart, which owns the
//! retail citations (`psiv_tools/nemesis.py`, `kosinski.py`, `enigma.py`).
//! The full gate's parity step decodes every stream a Python full build
//! decodes (the census, `rust/psiv-pack/tests/fixtures/decode_census.json`)
//! with both and compares each output and consumed length.
//!
//! [`Decodes`] is `psiv_tools/decode_cache.py`'s scope: sections share it,
//! so a stream several sections read (an enemy's art and its palette
//! variants) is decoded once. It lives for one build and is keyed by the
//! decoder and its arguments; the input is always the build's one ROM.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub mod enigma;
pub mod kosinski;
pub mod nemesis;

/// A stream a decoder refuses; the message names the offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeError {}

impl From<DecodeError> for crate::PackError {
    fn from(error: DecodeError) -> Self {
        crate::PackError(error.0)
    }
}

/// One decoder call, as the census and the table key it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Call {
    Nemesis {
        offset: usize,
    },
    Kosinski {
        offset: usize,
    },
    Enigma {
        offset: usize,
        base_tile: u16,
        max_words: Option<usize>,
    },
}

/// What a call decoded: bytes (Nemesis, Kosinski) or words (Enigma), and
/// the consumed length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded {
    Bytes(Vec<u8>, usize),
    Words(Vec<u16>, usize),
}

impl Decoded {
    /// The output as the 68000 left it in memory (Enigma's words big-endian).
    pub fn bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        match self {
            Decoded::Bytes(bytes, _) => bytes.into(),
            Decoded::Words(words, _) => enigma::words_to_bytes(words).into(),
        }
    }

    pub fn consumed(&self) -> usize {
        match self {
            Decoded::Bytes(_, consumed) | Decoded::Words(_, consumed) => *consumed,
        }
    }
}

/// Run one call against `rom`, uncached.
pub fn run(rom: &[u8], call: Call) -> Result<Decoded, DecodeError> {
    Ok(match call {
        Call::Nemesis { offset } => {
            let (bytes, consumed) = nemesis::decompress(rom, offset)?;
            Decoded::Bytes(bytes, consumed)
        }
        Call::Kosinski { offset } => {
            let (bytes, consumed) = kosinski::decompress(rom, offset)?;
            Decoded::Bytes(bytes, consumed)
        }
        Call::Enigma {
            offset,
            base_tile,
            max_words,
        } => {
            let (words, consumed) = enigma::decompress(rom, offset, base_tile, max_words)?;
            Decoded::Words(words, consumed)
        }
    })
}

type Slot = Arc<OnceLock<Result<Arc<Decoded>, DecodeError>>>;

/// One build's decodes: each call runs once, whichever section asks first,
/// and every other asker (on any thread) waits for and shares its result.
pub struct Decodes<'rom> {
    rom: &'rom [u8],
    slots: Mutex<HashMap<Call, Slot>>,
}

impl<'rom> Decodes<'rom> {
    pub fn new(rom: &'rom [u8]) -> Self {
        Decodes {
            rom,
            slots: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, call: Call) -> Result<Arc<Decoded>, DecodeError> {
        let slot = {
            let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            Arc::clone(slots.entry(call).or_default())
        };
        slot.get_or_init(|| run(self.rom, call).map(Arc::new))
            .clone()
    }

    /// How many distinct calls have been asked for.
    pub fn len(&self) -> usize {
        self.slots.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_decodes_once_and_is_shared() {
        // A Kosinski stream: two literals, then the terminator.
        let rom = [0x83, 0x00, b'a', b'b', 0xFE, 0x00, 0xF8, 0x00];
        let decodes = Decodes::new(&rom);
        let call = Call::Kosinski { offset: 0 };
        let results: Vec<Arc<Decoded>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| decodes.get(call).unwrap()))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(results.iter().all(|r| Arc::ptr_eq(r, &results[0])));
        assert_eq!(results[0].bytes().as_ref(), b"abab");
        assert_eq!(decodes.len(), 1);
        // A failing call fails for every asker, the same way.
        let bad = Call::Kosinski { offset: 7 };
        assert_eq!(decodes.get(bad), decodes.get(bad));
        assert!(decodes.get(bad).is_err());
    }
}
