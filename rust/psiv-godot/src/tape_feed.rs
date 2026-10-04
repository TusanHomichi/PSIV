//! One Godot-facing view of the runtime's tape codec. The native driver gets
//! bytes and string metadata; it never parses the text or computes FNV itself.

use godot::classes::{IRefCounted, RefCounted};
use godot::prelude::*;
use psiv_runtime::tape::{Tape, TapeStart, fnv1a64};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub(crate) struct TapeFeed {
    base: Base<RefCounted>,
    tape: Option<Tape>,
    error: String,
}

#[godot_api]
impl IRefCounted for TapeFeed {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            tape: None,
            error: String::new(),
        }
    }
}

#[godot_api]
impl TapeFeed {
    /// Parse the source file once, with the same codec campaign replay uses.
    #[func]
    fn open_tape(&mut self, path: GString) -> bool {
        self.tape = None;
        self.error.clear();
        let path = path.to_string();
        match std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| Tape::parse(&text).map_err(|error| error.to_string()))
        {
            Ok(tape) => {
                self.tape = Some(tape);
                true
            }
            Err(error) => {
                self.error = format!("{path}: {error}");
                false
            }
        }
    }

    #[func]
    fn last_error(&self) -> GString {
        GString::from(self.error.as_str())
    }

    #[func]
    fn pad_bytes(&self) -> PackedByteArray {
        self.tape
            .as_ref()
            .map(|tape| PackedByteArray::from(tape.pads.clone()))
            .unwrap_or_default()
    }

    #[func]
    fn start_kind(&self) -> GString {
        match self.tape.as_ref().map(|tape| &tape.start) {
            Some(TapeStart::NewGame) => GString::from("new-game"),
            Some(TapeStart::Save { .. }) => GString::from("save"),
            None => GString::new(),
        }
    }

    #[func]
    fn frame_count_decimal(&self) -> GString {
        self.tape
            .as_ref()
            .map(|tape| GString::from(tape.pads.len().to_string().as_str()))
            .unwrap_or_default()
    }

    #[func]
    fn save_hash_hex(&self) -> GString {
        match self.tape.as_ref().map(|tape| &tape.start) {
            Some(TapeStart::Save { hash }) => GString::from(format!("{hash:016x}").as_str()),
            _ => GString::new(),
        }
    }

    /// The caller verifies the source bytes before copying them into an
    /// isolated slot. This is the same FNV function campaign replay uses.
    #[func]
    fn matches_source_save(&mut self, bytes: PackedByteArray) -> bool {
        self.error.clear();
        let Some(Tape {
            start: TapeStart::Save { hash },
            ..
        }) = &self.tape
        else {
            self.error = "tape does not start from a save".into();
            return false;
        };
        let actual = fnv1a64(bytes.as_slice());
        if actual != *hash {
            self.error =
                format!("source save FNV mismatch: expected {hash:016x}, got {actual:016x}");
            return false;
        }
        true
    }
}
