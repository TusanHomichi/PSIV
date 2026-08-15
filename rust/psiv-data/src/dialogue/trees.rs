//! `dialogue/trees.json`: the decoded dialogue trees.
//!
//! One entry is a segment stream -- the byte stream regrouped into text runs
//! and control codes -- plus the pages the cartridge's text loop would show.
//! The control-code census is closed: see [`Ctrl`].

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use super::{PixelRect, hex_byte};

// ---------------------------------------------------------------------------
// trees.json
// ---------------------------------------------------------------------------

/// `dialogue/trees.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TreeFile {
    /// Pack format; checked against [`DIALOGUE_FORMAT_VERSION`].
    pub format_version: u32,
    /// Declared tree count, validated against `trees.len()`.
    pub tree_count: u32,
    /// Declared entry count across all trees.
    pub entry_count: u32,
    /// The trees, in tree order (`tree` is 1-based and dense).
    pub trees: Vec<DialogueTree>,
    /// The window metrics the text loop itself implies, as the decoder read
    /// them out of `RunText_CharacterLoop`. Cross-checked against
    /// `window.json` by the renderer, not here: this is the text side.
    pub window: TreeWindow,
}

/// One Kosinski tree.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DialogueTree {
    /// 1-based tree number, the value a map record's `dialogue_tree` holds.
    pub tree: u8,
    /// Disassembly label, such as `DialogueTree1`.
    pub label: String,
    /// Talk-mode trees take a second `$F4` operand: the portrait slot.
    pub is_talk_tree: bool,
    /// 1 for field trees, 2 for talk trees.
    pub portrait_operand_bytes: u8,
    /// Declared entry count, validated against `entries.len()`.
    pub entry_count: u32,
    /// Entries in id order, ids dense from 0.
    pub entries: Vec<DialogueEntry>,
}

impl DialogueTree {
    /// One entry by id. Ids are dense, so this is an index.
    #[must_use]
    pub fn entry(&self, id: u16) -> Option<&DialogueEntry> {
        self.entries.get(usize::from(id))
    }
}

/// One entry: what an NPC says, from the byte after the previous `$FF` to the
/// next one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DialogueEntry {
    /// Entry id within its tree.
    pub id: u16,
    /// The decoded text with `$FC` newlines as `\n` and control codes
    /// dropped; for logs and searching, never for layout.
    pub text: String,
    /// The byte stream regrouped: text runs and control codes in order.
    pub segments: Vec<Segment>,
    /// What the cartridge's text loop would show, window by window. The
    /// renderer derives the same thing from `segments` at runtime; this is the
    /// extractor's own answer, which makes it the oracle the runtime is tested
    /// against.
    pub pages: Vec<Page>,
}

/// One windowful of text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Page {
    /// One or two lines, already broken exactly as the cartridge breaks them.
    pub lines: Vec<String>,
    /// Why the page ended.
    pub end: PageEnd,
}

/// Why a page ended -- which is also what the player has to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageEnd {
    /// Both lines filled with text still to come: the arrow appears and the
    /// window waits, exactly as `$FD` does.
    Full,
    /// `$FD`: show the arrow, wait, clear, restart at line 0.
    Wait,
    /// `$F7`: the caller resumes after this byte.
    Close,
    /// `$F5`: a yes/no choice follows.
    Choice,
    /// The entry ran out ($FE/$FF, or a null code).
    End,
}

/// The window as the text loop sees it (`trees.json`'s `window` block).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TreeWindow {
    /// 32.
    pub chars_per_line: u32,
    /// 2.
    pub lines_per_window: u32,
    /// 8.
    pub glyph_width: u32,
    /// 16.
    pub glyph_height: u32,
    /// The text area itself, without the frame, in Genesis screen pixels.
    pub rect: PixelRect,
    /// Where the scroll arrow sprite sits when a page waits.
    pub scroll_arrow: ScrollArrow,
}

/// The scroll-arrow sprite's screen position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ScrollArrow {
    /// Screen x, in Genesis pixels.
    pub screen_x: i32,
    /// Screen y, in Genesis pixels.
    pub screen_y: i32,
}

// ---------------------------------------------------------------------------
// Segments
// ---------------------------------------------------------------------------

/// `$F4` id 0: hide the portrait window.
pub const PORTRAIT_HIDE: u8 = 0;

/// One piece of an entry: a run of glyphs, or one control code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// A run of characters, flushed at every control code.
    Text(String),
    /// One control code with its operands.
    Control(Ctrl),
}

impl<'de> Deserialize<'de> for Segment {
    /// Text segments are `{"text": ...}` and control segments are tagged by
    /// `ctrl`. Hand-written rather than `#[serde(untagged)]` so an unknown
    /// control name reports itself instead of "data did not match any variant".
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(text) = value.get("text") {
            if value.as_object().is_some_and(|map| map.len() > 1) {
                return Err(D::Error::custom(
                    "a text segment carries nothing but `text`",
                ));
            }
            let text = text
                .as_str()
                .ok_or_else(|| D::Error::custom("segment `text` must be a string"))?;
            return Ok(Segment::Text(text.to_owned()));
        }
        serde_json::from_value(value)
            .map(Segment::Control)
            .map_err(D::Error::custom)
    }
}

/// Every control code the retail trees contain, as
/// `TextCtrlCodesJmpTbl` dispatches them.
///
/// The census is closed on purpose: `$F0`, `$F1`, `$F8` and `$FB` are
/// `TextCtrlCode_Null` and `$FE`/`$FF` terminate, so the decoder ends the entry
/// on them and never emits a segment for one. A name outside this list means
/// the pack was written by a decoder this build does not understand, and the
/// load fails rather than guessing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "ctrl", rename_all = "snake_case", deny_unknown_fields)]
pub enum Ctrl {
    /// `$F2`: `TextActionsOffs` sub-dispatch.
    Action {
        /// Raw code byte, `0xF2`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// The bytes that followed it.
        operands: Vec<u8>,
        /// Which action, by name.
        action: ActionKind,
        /// The `TextActionsOffs` index.
        action_id: u8,
        /// Sound id, for the two sound actions.
        #[serde(default)]
        sound: Option<u8>,
        /// Panel id, for `load_panel`. A word: `$F2 00` takes two operand
        /// bytes and retail data reaches 397.
        #[serde(default)]
        panel: Option<u16>,
        /// Event flag, for `set_event_flag`.
        #[serde(default)]
        flag: Option<u8>,
    },
    /// `$F3`: preamble only -- the NPC does not turn to face the player.
    KeepNpcFacing {
        /// Raw code byte, `0xF3`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// Always empty.
        operands: Vec<u8>,
    },
    /// `$F4`: show portrait `id`; id 0 hides the portrait window.
    Portrait {
        /// Raw code byte, `0xF4`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// One byte, or two in a talk tree.
        operands: Vec<u8>,
        /// Portrait id into `DialoguePortraitArtPtrs`.
        id: u8,
        /// Talk-mode slot; shifts the portrait right by 12 tiles per slot.
        /// `None` outside talk trees.
        #[serde(default)]
        position: Option<u8>,
    },
    /// `$F5`: yes/no window; each answer is an entry id in this tree.
    YesNo {
        /// Raw code byte, `0xF5`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// The two entry ids.
        operands: Vec<u8>,
        /// Entry to continue at on yes.
        yes_entry: u16,
        /// Entry to continue at on no.
        no_entry: u16,
    },
    /// `$F6`: preamble only -- fire event `id` instead of showing anything.
    Event {
        /// Raw code byte, `0xF6`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// The event id as a word.
        operands: Vec<u8>,
        /// Event id.
        id: u16,
    },
    /// `$F7`: close the window; the caller resumes after this byte.
    Close {
        /// Raw code byte, `0xF7`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// Always empty.
        operands: Vec<u8>,
    },
    /// `$F9`: hold for `frames` frames.
    Delay {
        /// Raw code byte, `0xF9`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// One byte.
        operands: Vec<u8>,
        /// Frames to hold.
        frames: u16,
    },
    /// `$FA`: if the event flag is set, continue at `then_entry`.
    FlagCheck {
        /// Raw code byte, `0xFA`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// Flag then target entry.
        operands: Vec<u8>,
        /// Event flag number.
        flag: u8,
        /// What kind of flag it is; always `event_flag` in retail data.
        scope: FlagScope,
        /// Entry to continue at when the flag is set.
        then_entry: u16,
    },
    /// `$FC`: newline. Does not check the two-line limit.
    Newline {
        /// Raw code byte, `0xFC`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// Always empty.
        operands: Vec<u8>,
    },
    /// `$FD`: show the arrow, wait for input, clear, restart at line 0.
    Wait {
        /// Raw code byte, `0xFD`.
        #[serde(deserialize_with = "hex_byte")]
        code: u8,
        /// Always empty.
        operands: Vec<u8>,
    },
}

impl Ctrl {
    /// The code byte the pack recorded.
    #[must_use]
    pub const fn code(&self) -> u8 {
        match self {
            Ctrl::Action { code, .. }
            | Ctrl::KeepNpcFacing { code, .. }
            | Ctrl::Portrait { code, .. }
            | Ctrl::YesNo { code, .. }
            | Ctrl::Event { code, .. }
            | Ctrl::Close { code, .. }
            | Ctrl::Delay { code, .. }
            | Ctrl::FlagCheck { code, .. }
            | Ctrl::Newline { code, .. }
            | Ctrl::Wait { code, .. } => *code,
        }
    }

    /// The code byte `TextCtrlCodesJmpTbl` reaches this handler from.
    #[must_use]
    pub const fn expected_code(&self) -> u8 {
        match self {
            Ctrl::Action { .. } => 0xF2,
            Ctrl::KeepNpcFacing { .. } => 0xF3,
            Ctrl::Portrait { .. } => 0xF4,
            Ctrl::YesNo { .. } => 0xF5,
            Ctrl::Event { .. } => 0xF6,
            Ctrl::Close { .. } => 0xF7,
            Ctrl::Delay { .. } => 0xF9,
            Ctrl::FlagCheck { .. } => 0xFA,
            Ctrl::Newline { .. } => 0xFC,
            Ctrl::Wait { .. } => 0xFD,
        }
    }

    /// The pack's name for this code, for messages and logs.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Ctrl::Action { .. } => "action",
            Ctrl::KeepNpcFacing { .. } => "keep_npc_facing",
            Ctrl::Portrait { .. } => "portrait",
            Ctrl::YesNo { .. } => "yes_no",
            Ctrl::Event { .. } => "event",
            Ctrl::Close { .. } => "close",
            Ctrl::Delay { .. } => "delay",
            Ctrl::FlagCheck { .. } => "flag_check",
            Ctrl::Newline { .. } => "newline",
            Ctrl::Wait { .. } => "wait",
        }
    }
}

/// What a `$F2` action does, by `TextActionsOffs` index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// 0: load cutscene panel `panel`.
    LoadPanel,
    /// 1: destroy the most recent panel.
    DestroyLastPanel,
    /// 2: destroy every panel.
    DestroyAllPanels,
    /// 3: play sound `sound`.
    LoadSound,
    /// 4: the second sound entry point.
    #[serde(rename = "load_sound_2")]
    LoadSound2,
    /// 6: reload the palette.
    UpdatePalette,
    /// 7: Zio's eyes turn red.
    ZioEyesRed,
    /// 8: pause the music.
    PauseMusic,
    /// 9: resume the music.
    ResumeMusic,
    /// 10: the sabotage alarm's red palette.
    SabotageAlarmRedPalette,
    /// 11: set event flag `flag`.
    SetEventFlag,
    /// 12: Elsydeon breaks.
    ElsydeonBroken,
}

/// What a `$FA` check reads. Retail data only ever checks event flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagScope {
    /// The event flag array.
    EventFlag,
}
