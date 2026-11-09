//! `dialogue/trees.json`: the port of `extract_dialogue` in
//! `psiv_tools/text.py`, of `psiv_tools/dialogue_pack/flow.py` (control
//! segments, `RunText_CharacterLoop` as pagination, the system messages) and
//! `selection.py`, and of `window_json` and `interaction_json` in
//! `window.py`. Those modules' docs give the reasons.

use sha2::{Digest, Sha256};

use super::{DIALOGUE_CRAM_LINE, FORMAT_VERSION, refuse, slice};
use crate::decode::{Call, Decodes};
use crate::json::Json;
use crate::text::{
    DIALOGUE, DIALOGUE_BLOB_ALIGNMENT, DIALOGUE_REGION_START, WINDOW, decode_string,
    dialogue_tree_specs, split_terminated, trim_tree_alignment,
};
use crate::{PackError, png};

/// `TALK_DIALOGUE_TREES`: trees whose `$F4` carries a second operand.
const TALK_TREES: [usize; 2] = [31, 32];
const CHARS_PER_LINE: usize = 32;
const LINES_PER_WINDOW: usize = 2;
const TILE_PIXELS: usize = 8;

/// `CTRL_NAMES` and `CTRL_NOTES`: the retail meaning of each control code.
const CONTROLS: [(u8, &str, &str); 16] = [
    (
        0xF0,
        "null",
        "TextCtrlCode_Null is an rts: it ends the message",
    ),
    (
        0xF1,
        "null",
        "TextCtrlCode_Null is an rts: it ends the message",
    ),
    (0xF2, "action", "TextActionsOffs sub-dispatch; see `action`"),
    (
        0xF3,
        "keep_npc_facing",
        "preamble only: the NPC does not turn to face the player",
    ),
    (
        0xF4,
        "portrait",
        "load portrait `id` into VRAM $55C and show it; id 0 hides it",
    ),
    (
        0xF5,
        "yes_no",
        "yes/no window; the chosen operand is an entry id in this tree",
    ),
    (
        0xF6,
        "event",
        "preamble only: fire event `id` instead of showing the message",
    ),
    (
        0xF7,
        "close",
        "close the window and yield; the caller resumes after this byte",
    ),
    (
        0xF8,
        "null",
        "TextCtrlCode_Null is an rts: it ends the message",
    ),
    (0xF9, "delay", "wait `frames` frames"),
    (
        0xFA,
        "flag_check",
        "if event flag `flag` is set, continue at entry `then_entry`",
    ),
    (
        0xFB,
        "null",
        "not a retail code: the jump table sends it to TextCtrlCode_Null",
    ),
    (
        0xFC,
        "newline",
        "newline; does not check the two-line limit",
    ),
    (
        0xFD,
        "wait",
        "show the scroll arrow, wait, clear the window, start at line 0",
    ),
    (0xFE, "terminate", "end the message"),
    (
        0xFF,
        "terminate",
        "end the message; also the entry delimiter",
    ),
];

/// `WinTiles_PlayerNothingMsg`: one line per playable character.
const SYSTEM_MESSAGES: usize = 0x1FE660;
const SYSTEM_MESSAGE_COUNT: usize = 11;
const SYSTEM_MESSAGES_LABEL: &str = "WinTiles_PlayerNothingMsg";
const SYSTEM_MESSAGES_PAD: u8 = 1;
/// `Event_GettingOffVehicle`'s `lea (CannotGetOffString).l, a0`.
const CANNOT_GET_OFF_LEA: usize = 0x06BFEC;
const CANNOT_GET_OFF_LABEL: &str = "CannotGetOffString";
const LEA_ABS_A0: [u8; 2] = [0x41, 0xF9];
/// `DIALOGUE_PORTRAIT_SYMBOLS` 1..=11: the system messages' speakers.
const SPEAKERS: [&str; 11] = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth",
];

/// `Interaction_DisplayDialogue`: its opcode runs, pinned by length and
/// digest, and the data between them (the cutoff, the override offset and
/// the table pointer) read from the ROM.
const SELECTION: usize = 0x058844;
const SELECTION_PINS: [(usize, usize, &str); 4] = [
    (
        0,
        18,
        "bd22f58d82d598811b289763156ba3ff71d70e5a1c38e29a90e0687c1a6213e1",
    ),
    (
        20,
        4,
        "d83cc5cb75686878810ffa58c0682b59c3a1eacc6584d3fbcf49f6f496f04735",
    ),
    (
        26,
        2,
        "df8a7fada9461dc56be0853d003ebb3bc2a4436093c0bdaf84c4ed8c64471417",
    ),
    (
        32,
        14,
        "b11de8a6ab939268cfda45b94a49f7eabb3f0572d01b9ba1180e07f276954130",
    ),
];
const WORLD_COUNT: usize = 6;

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn sha256(bytes: &[u8]) -> String {
    png::hex(&Sha256::digest(bytes))
}

fn string<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    match value.get(key) {
        Some(Json::Str(s)) => Some(s),
        _ => None,
    }
}

fn list(value: &Json, key: &str) -> Vec<Json> {
    match value.get(key) {
        Some(Json::Array(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn operands(segment: &Json) -> Vec<i128> {
    list(segment, "operands")
        .iter()
        .map(|o| match o {
            Json::Int(n) => *n,
            _ => 0,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Segments
// ---------------------------------------------------------------------------

/// `control_segment`: a decoded control record as a typed renderer segment.
fn control_segment(segment: &Json) -> Result<Json, PackError> {
    let code_text = string(segment, "ctrl").unwrap_or_default();
    let code = u8::from_str_radix(code_text.trim_start_matches("0x"), 16).unwrap_or(0);
    if code == 0xFB {
        return refuse(
            "control code $FB occurs in the script, but the retail jump table sends $FB to \
             TextCtrlCode_Null (an rts) and the retail interaction preamble has no $FB branch; \
             psiv_tools.text decodes it with the Grand Cross handler's three operand bytes, \
             which would be wrong here"
                .into(),
        );
    }
    let name = CONTROLS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map_or("", |(_, name, _)| name);
    let ops = operands(segment);
    let op = |i: usize| Json::Int(ops.get(i).copied().unwrap_or(0));
    let mut out = Json::object()
        .with("ctrl", name)
        .with("code", code_text)
        .with(
            "operands",
            Json::Array(ops.iter().map(|&n| Json::Int(n)).collect()),
        );
    match code {
        0xF2 => {
            let action = string(segment, "action").unwrap_or_default().to_owned();
            out.insert("action", action.as_str());
            out.insert(
                "action_id",
                segment.get("action_id").cloned().unwrap_or(Json::Null),
            );
            match action.as_str() {
                "load_panel" => out.insert(
                    "panel",
                    Json::Int(
                        (ops.first().copied().unwrap_or(0) << 8) | ops.get(1).copied().unwrap_or(0),
                    ),
                ),
                "load_sound" | "load_sound_2" => out.insert("sound", op(0)),
                "set_event_flag" => out.insert("flag", op(0)),
                _ => {}
            }
        }
        0xF4 => {
            out.insert(
                "id",
                segment.get("portrait_id").cloned().unwrap_or(Json::Null),
            );
            out.insert(
                "position",
                segment
                    .get("portrait_location")
                    .cloned()
                    .unwrap_or(Json::Null),
            );
        }
        0xF5 => {
            out.insert("yes_entry", op(0));
            out.insert("no_entry", op(1));
        }
        0xF6 => out.insert(
            "id",
            Json::Int((ops.first().copied().unwrap_or(0) << 8) | ops.get(1).copied().unwrap_or(0)),
        ),
        0xF9 => {
            // `subq.b #1,d7` then `dbf`: a zero operand waits 256 frames.
            let frames = ops.first().copied().unwrap_or(0);
            out.insert(
                "frames",
                Json::Int(if frames == 0 { 0x100 } else { frames }),
            );
        }
        0xFA => {
            out.insert("scope", "event_flag");
            out.insert("flag", op(0));
            out.insert("then_entry", op(1));
        }
        _ => {}
    }
    Ok(out)
}

/// `entry_segments`.
fn entry_segments(segments: &[Json]) -> Result<Vec<Json>, PackError> {
    segments
        .iter()
        .map(|s| {
            if s.get("text").is_some() {
                Ok(s.clone())
            } else {
                control_segment(s)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

fn ctrl(segment: Option<&Json>) -> Option<&str> {
    segment
        .filter(|s| s.get("text").is_none())
        .and_then(|s| string(s, "ctrl"))
}

/// `split_preamble`: what the interaction code eats, and whether it is an
/// event.
fn split_preamble(segments: &[Json]) -> (usize, bool) {
    let mut index = 0;
    while ctrl(segments.get(index)) == Some("0xFA") {
        index += 1;
    }
    match ctrl(segments.get(index)) {
        Some("0xF6") => (index + 1, true),
        Some("0xF3") => (index + 1, false),
        _ => (index, false),
    }
}

/// `paginate`'s state: the pages so far and the window being filled.
struct Pager<'a> {
    segments: &'a [Json],
    index: usize,
    pages: Vec<Json>,
    lines: Vec<String>,
    line: usize,
    column: usize,
}

impl Pager<'_> {
    fn close(&mut self, end: &str) {
        let lines = std::mem::replace(&mut self.lines, vec![String::new()]);
        self.pages
            .push(Json::object().with("lines", lines).with("end", end));
        self.line = 0;
        self.column = 0;
    }

    fn peek(&self) -> Option<&str> {
        ctrl(self.segments.get(self.index))
    }

    /// `TextCtrlCode_Interrupt`: returns whether the message ended here.
    fn interrupt(&mut self, waited: &str) -> bool {
        if self.index >= self.segments.len() {
            self.close("end");
            return true;
        }
        match self.peek() {
            Some("0xF7") => {
                self.index += 1;
                self.close("close");
                return false;
            }
            Some("0xFD") => self.index += 1,
            _ => {}
        }
        self.close(waited);
        false
    }
}

/// `paginate`: one entry split into windows exactly as
/// `RunText_CharacterLoop` does.
fn paginate(segments: &[Json]) -> Vec<Json> {
    let (start, is_event) = split_preamble(segments);
    if is_event {
        return Vec::new();
    }
    let mut p = Pager {
        segments,
        index: start,
        pages: Vec::new(),
        lines: vec![String::new()],
        line: 0,
        column: 0,
    };
    while p.index < segments.len() {
        let segment = &segments[p.index];
        p.index += 1;
        if let Some(Json::Str(run)) = segment.get("text") {
            let chars: Vec<char> = run.chars().collect();
            for (position, c) in chars.iter().enumerate() {
                p.lines[p.line].push(*c);
                p.column += 1;
                if p.column < CHARS_PER_LINE {
                    continue;
                }
                p.column = 0;
                let at_run_end = position == chars.len() - 1;
                if at_run_end && p.peek() == Some("0xF5") {
                    p.index += 1;
                    p.close("choice");
                    return p.pages;
                }
                p.line += 1;
                if at_run_end && p.peek() == Some("0xFC") {
                    p.index += 1;
                }
                if !p.line.is_multiple_of(LINES_PER_WINDOW) {
                    p.lines.push(String::new());
                } else if !at_run_end {
                    p.close("full");
                } else if p.interrupt("full") {
                    return p.pages;
                }
            }
            continue;
        }
        match string(segment, "ctrl").unwrap_or_default() {
            "0xFC" => {
                p.column = 0;
                p.line += 1;
                while p.lines.len() <= p.line {
                    p.lines.push(String::new());
                }
            }
            "0xFD" => {
                if p.interrupt("wait") {
                    return p.pages;
                }
            }
            "0xF7" => p.close("close"),
            "0xFE" | "0xFF" => {
                p.close("end");
                return p.pages;
            }
            "0xF5" => {
                p.close("choice");
                return p.pages;
            }
            "0xF0" | "0xF1" | "0xF3" | "0xF8" | "0xFB" => {
                p.close("end");
                return p.pages;
            }
            _ => {}
        }
    }
    if p.lines.iter().any(|l| !l.is_empty()) {
        let lines = std::mem::take(&mut p.lines);
        p.pages
            .push(Json::object().with("lines", lines).with("end", "end"));
    }
    p.pages
}

/// One decoded string as an entry: text, raw bytes, typed segments, pages.
fn entry(decoded: &Json) -> Result<(Json, Vec<Json>), PackError> {
    let raw = list(decoded, "segments");
    Ok((
        Json::object()
            .with("text", decoded.get("text").cloned().unwrap_or(Json::Null))
            .with(
                "raw_hex",
                decoded.get("raw_hex").cloned().unwrap_or(Json::Null),
            )
            .with("segments", entry_segments(&raw)?)
            .with("pages", paginate(&raw)),
        raw,
    ))
}

// ---------------------------------------------------------------------------
// Trees
// ---------------------------------------------------------------------------

/// `extract_dialogue` and `tree_json`: every tree's runtime record, and the
/// region they fill.
fn trees(rom: &[u8], decodes: &Decodes) -> Result<(Vec<Json>, usize, Json), PackError> {
    let mut trees = Vec::new();
    let mut total = 0;
    let mut region_end = DIALOGUE_REGION_START;
    for spec in dialogue_tree_specs(rom, decodes)? {
        let label = format!("DialogueTree{}", spec.tree);
        let decoded = decodes.get(Call::Kosinski { offset: spec.start })?;
        let decompressed = decoded.bytes();
        let (block, _) = trim_tree_alignment(&decompressed, &label)?;
        let compressed = slice(rom, spec.start, spec.end - spec.start, &label)?;
        let talk = TALK_TREES.contains(&spec.tree);
        let operand_bytes = if talk { 2 } else { 1 };
        let mut entries = Vec::new();
        for (index, (block_offset, payload)) in
            split_terminated(block, 0xFF)?.into_iter().enumerate()
        {
            let decoded = decode_string(payload, DIALOGUE, operand_bytes)?;
            let (mut record, _) = entry(&decoded)?;
            record.insert("id", index);
            record.insert("block_offset", format!("0x{block_offset:04X}"));
            entries.push(record);
        }
        total += entries.len();
        region_end = spec.end;
        trees.push(
            Json::object()
                .with("tree", spec.tree)
                .with("label", label.as_str())
                .with("rom_offset", hex6(spec.start))
                .with("rom_end_exclusive", hex6(spec.end))
                .with("compression", "kosinski")
                .with("compressed_size", compressed.len())
                .with("compressed_sha256", sha256(compressed))
                .with("decompressed_size", decompressed.len())
                .with("portrait_operand_bytes", operand_bytes)
                .with("is_talk_tree", talk)
                .with("entry_count", entries.len())
                .with("entries", entries),
        );
    }
    let region = Json::object()
        .with("start", hex6(DIALOGUE_REGION_START))
        .with("end_exclusive", hex6(region_end))
        .with("tree_count", trees.len())
        .with("blob_alignment", DIALOGUE_BLOB_ALIGNMENT);
    Ok((trees, total, region))
}

// ---------------------------------------------------------------------------
// System messages
// ---------------------------------------------------------------------------

fn terminator(rom: &[u8], from: usize) -> Option<usize> {
    rom.get(from..)?
        .iter()
        .position(|&b| b == 0xFF)
        .map(|i| from + i)
}

/// `cannot_get_off`: the mounted party's refusal, as a one-page entry.
fn cannot_get_off(rom: &[u8]) -> Result<Json, PackError> {
    let lea = slice(rom, CANNOT_GET_OFF_LEA, 6, CANNOT_GET_OFF_LABEL)?;
    if lea[..2] != LEA_ABS_A0 {
        return refuse(format!(
            "{CANNOT_GET_OFF_LABEL}: no `lea (abs).l, a0` at 0x{CANNOT_GET_OFF_LEA:06X}"
        ));
    }
    let offset = u32::from_be_bytes([lea[2], lea[3], lea[4], lea[5]]) as usize;
    let end = terminator(rom, offset).filter(|&e| e - offset <= 32);
    let Some(end) = end else {
        return refuse(format!(
            "{CANNOT_GET_OFF_LABEL} at 0x{offset:06X} has no terminator within a line"
        ));
    };
    let decoded = decode_string(&rom[offset..end], WINDOW, 1)?;
    let (record, _) = entry(&decoded)?;
    if list(&record, "segments")
        .iter()
        .any(|s| s.get("text").is_none())
    {
        return refuse(format!("{CANNOT_GET_OFF_LABEL} holds a control code"));
    }
    let mut out = Json::object()
        .with("id", 0u8)
        .with("label", CANNOT_GET_OFF_LABEL)
        .with("rom_offset", hex6(offset))
        .with("charset", WINDOW);
    for key in ["text", "raw_hex", "segments", "pages"] {
        out.insert(key, record.get(key).cloned().unwrap_or(Json::Null));
    }
    Ok(out)
}

/// `system_messages`: the eleven "nothing here" lines, in an entry's shape.
fn system_messages(rom: &[u8]) -> Result<Json, PackError> {
    let mut offset = SYSTEM_MESSAGES;
    let mut messages = Vec::new();
    for (index, speaker) in SPEAKERS.iter().enumerate().take(SYSTEM_MESSAGE_COUNT) {
        let Some(end) = terminator(rom, offset) else {
            return refuse(format!(
                "{SYSTEM_MESSAGES_LABEL} message {index} at 0x{offset:06X} has no $FF terminator"
            ));
        };
        let decoded = decode_string(&rom[offset..end], DIALOGUE, 1)?;
        let (record, _) = entry(&decoded)?;
        let first_portrait = list(&record, "segments")
            .iter()
            .find(|s| string(s, "ctrl") == Some("portrait"))
            .and_then(|s| s.get("id").cloned());
        if first_portrait != Some(Json::Int(index as i128 + 1)) {
            return refuse(format!(
                "{SYSTEM_MESSAGES_LABEL} message {index} at 0x{offset:06X} does not open with \
                 portrait {}",
                index + 1
            ));
        }
        let mut message = Json::object()
            .with("id", index)
            .with("character_id", index)
            .with("portrait", index + 1)
            .with("portrait_symbol", *speaker)
            .with("rom_offset", hex6(offset));
        for key in ["text", "raw_hex", "segments", "pages"] {
            message.insert(key, record.get(key).cloned().unwrap_or(Json::Null));
        }
        messages.push(message);
        offset = end + 1;
    }
    let block = slice(
        rom,
        SYSTEM_MESSAGES,
        offset - SYSTEM_MESSAGES,
        SYSTEM_MESSAGES_LABEL,
    )?;
    Ok(Json::object()
        .with("label", SYSTEM_MESSAGES_LABEL)
        .with("rom_offset", hex6(SYSTEM_MESSAGES))
        .with("rom_end_exclusive", hex6(offset))
        .with("compression", Json::Null)
        .with("size_bytes", block.len())
        .with("sha256", sha256(block))
        .with("count", messages.len())
        .with("charset", DIALOGUE)
        .with(
            "selector",
            Json::object()
                .with("routine", "Interaction_DoPlayerNothingMsg")
                .with("rom_offset", "0x058CE8")
                .with("ram", "Current_Party_Slots ($FFFFF40A)")
                .with(
                    "addressing",
                    "GetOffsetByID, counting $FF terminators from the label",
                )
                .with("note", "the id is the party leader's character id, 0..10"),
        )
        .with("alignment_pad_bytes", SYSTEM_MESSAGES_PAD)
        .with(
            "note",
            "Uncompressed, unlike the 43 trees, and not part of any of them: these are the lines \
             the leader says when Talk finds nothing. They are not counted in the census, which \
             is about the trees. The records after this block (cabinets, graves, flowers) each \
             have their own label and pointer and are not in this id space.",
        )
        .with("messages", messages)
        .with("cannot_get_off", cannot_get_off(rom)?))
}

// ---------------------------------------------------------------------------
// World selection, window and interaction
// ---------------------------------------------------------------------------

/// `extract_selection`: the live-world table and the first world's cutoff.
fn selection(rom: &[u8], decodes: &Decodes) -> Result<Json, PackError> {
    let code = slice(rom, SELECTION, 46, "Interaction_DisplayDialogue")?;
    for (at, len, digest) in SELECTION_PINS {
        if sha256(&code[at..at + len]) != digest {
            return refuse(format!(
                "Interaction_DisplayDialogue opcode mismatch at 0x{:06X}",
                SELECTION + at
            ));
        }
    }
    let word = |at: usize| usize::from(u16::from_be_bytes([code[at], code[at + 1]]));
    let (cutoff, override_offset) = (word(18), word(24));
    if cutoff > 0xFF || !override_offset.is_multiple_of(4) || override_offset < WORLD_COUNT * 4 {
        return refuse("invalid byte cutoff or world-tree override offset".into());
    }
    let table = u32::from_be_bytes([code[28], code[29], code[30], code[31]]) as usize;
    let pointers = slice(rom, table, override_offset + 4, "WorldDialogueTreePtrs")?;
    let specs = dialogue_tree_specs(rom, decodes)?;
    let tree_at = |at: usize| -> Result<usize, PackError> {
        let pointer = u32::from_be_bytes([
            pointers[at],
            pointers[at + 1],
            pointers[at + 2],
            pointers[at + 3],
        ]) as usize;
        specs
            .iter()
            .find(|s| s.start == pointer)
            .map(|s| s.tree)
            .ok_or_else(|| {
                PackError(format!(
                    "world dialogue pointer 0x{pointer:06X} is not a tree"
                ))
            })
    };
    Ok(Json::object()
        .with("routine_offset", hex6(SELECTION))
        .with("table_offset", hex6(table))
        .with(
            "world_trees",
            (0..WORLD_COUNT)
                .map(|i| tree_at(i * 4))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .with(
            "first_world_override",
            Json::object()
                .with("entry_from", cutoff)
                .with("tree", tree_at(override_offset)?),
        ))
}

/// `window_json`: what a renderer needs to lay the window out.
fn window(scroll_arrow: &Json) -> Json {
    Json::object()
        .with("chars_per_line", CHARS_PER_LINE)
        .with("lines_per_window", LINES_PER_WINDOW)
        .with("glyph_width", 8u8)
        .with("glyph_height", 16u8)
        .with("tile_width", 32u8)
        .with("tile_height", 4u8)
        .with("screen_tile_x", 4u8)
        .with("screen_tile_y", 0x15u8)
        .with(
            "rect",
            Json::object()
                .with("x", 4 * TILE_PIXELS)
                .with("y", 0x15 * TILE_PIXELS)
                .with("width", 32 * TILE_PIXELS)
                .with("height", 4 * TILE_PIXELS),
        )
        .with("vram_tile", "0x580")
        .with("vram_address", "0xB000")
        .with("cram_line", DIALOGUE_CRAM_LINE)
        .with("palette", "Pal_Init_Line_3")
        .with("text_color_index", 0xFu8)
        .with("background_color_index", 0xEu8)
        .with("scroll_arrow", scroll_arrow.clone())
        .with(
            "page_endings",
            vec!["full", "wait", "close", "choice", "end"],
        )
        .with("chrome", "dialogue/window.json")
        .with(
            "note",
            "A glyph is 8x16 and the window holds 32 of them on each of two lines. The column \
             counter wraps at 32 (andi.w #$1F,d1), which swallows an explicit $FC landing \
             exactly there, and the line counter is masked to one bit (andi.w #1,d2), which is \
             what makes a full second line wait for input. $FC itself is not masked: a third \
             line would be written past the window's tiles.",
        )
}

/// `interaction_json`.
fn interaction() -> Json {
    Json::object()
        .with("grammar", "entry := $FA* ( $F6 event | $F3? text... )")
        .with("routine", "Interaction_ProcessDialogueTree")
        .with("rom_offset", "0x05887A")
        .with(
            "note",
            "The interaction code walks the preamble before RunText sees the entry: $FA follows \
             an event flag to another entry by id, $F6 fires an event instead of showing the \
             message, and $F3 suppresses Interaction_UpdateObj so the NPC keeps its facing. \
             $F6's operand is read as a word here, which is what fixes its length; \
             TextCtrlCode_Event skips only one byte and no retail data reaches it. The retail \
             scanner has no $FB branch.",
        )
        .with(
            "entry_addressing",
            "GetDialogueByID counts $FF bytes from the tree's start, so entry ids are dense and \
             include the empty entries.",
        )
}

/// `dialogue/trees.json`.
pub fn write(rom: &[u8], root: &std::path::Path, scroll_arrow: &Json) -> Result<(), PackError> {
    let decodes = Decodes::new(rom);
    let (trees, total, region) = trees(rom, &decodes)?;
    let mut controls = Json::object();
    for (code, name, note) in CONTROLS {
        controls.insert(
            format!("0x{code:02X}"),
            Json::object().with("ctrl", name).with("note", note),
        );
    }
    let payload = Json::object()
        .with("format_version", FORMAT_VERSION)
        .with("kind", "dialogue_trees")
        .with("region", region)
        .with("tree_count", trees.len())
        .with("entry_count", total)
        .with("charset", DIALOGUE)
        .with("window", window(scroll_arrow))
        .with("interaction", interaction())
        .with("control_codes", controls)
        .with("font", "dialogue/font.json")
        .with("chrome", "dialogue/window.json")
        .with("system_messages", system_messages(rom)?)
        .with("world_interaction", selection(rom, &decodes)?)
        .with("portraits", "dialogue/portraits.json")
        .with("trees", trees);
    crate::write_json(root, "dialogue/trees.json", &payload).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Json {
        Json::object().with("text", s)
    }

    fn code(c: &str) -> Json {
        Json::object().with("ctrl", c)
    }

    fn pages(segments: &[Json]) -> Vec<(Vec<String>, String)> {
        paginate(segments)
            .iter()
            .map(|p| {
                let v = serde_json::Value::from(p);
                (
                    v["lines"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|l| l.as_str().unwrap().to_owned())
                        .collect(),
                    v["end"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn two_full_lines_fill_a_window_and_the_rest_ends_the_entry() {
        let long = "a".repeat(64) + "b";
        let got = pages(&[text(&long)]);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1, "full");
        assert_eq!(got[0].0, vec!["a".repeat(32), "a".repeat(32)]);
        assert_eq!(got[1], (vec!["b".to_owned()], "end".to_owned()));
    }

    #[test]
    fn a_preamble_event_shows_nothing_and_a_wait_swallows_nothing_after_it() {
        assert!(pages(&[code("0xFA"), code("0xF6"), text("x")]).is_empty());
        let got = pages(&[code("0xF3"), text("hi"), code("0xFD"), text("yo")]);
        assert_eq!(got[0], (vec!["hi".to_owned()], "wait".to_owned()));
        assert_eq!(got[1], (vec!["yo".to_owned()], "end".to_owned()));
    }

    #[test]
    fn a_full_line_swallows_a_newline_landing_on_it() {
        let got = pages(&[text(&"a".repeat(32)), code("0xFC"), text("b")]);
        assert_eq!(got[0].0, vec!["a".repeat(32), "b".to_owned()]);
    }

    #[test]
    fn a_wait_of_zero_frames_is_256_and_fb_is_refused() {
        let seg = Json::object()
            .with("ctrl", "0xF9")
            .with("operands", vec![0u8]);
        let out = serde_json::Value::from(&control_segment(&seg).unwrap());
        assert_eq!(out["frames"], 256);
        assert!(control_segment(&code("0xFB")).is_err());
    }
}
