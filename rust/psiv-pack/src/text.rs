//! PSIV text: the two charsets, the control codes, strings and the name
//! tables. The port of `psiv_tools/text.py`, which owns the citations (the
//! charset files, `TextCtrlCodesJmpTbl`, `TextActionsOffs`, each table's
//! offsets and signature). Its records are built as the Python builds them,
//! so the pack JSON matches byte for byte; the dialogue trees (Kosinski)
//! follow the decoders slice.

use std::sync::OnceLock;

use crate::json::Json;

pub const WINDOW: &str = "window";
pub const DIALOGUE: &str = "dialogue";
pub const CONTROL_BASE: u8 = 0xF0;
/// `$FC` ends a line and `$FD` waits for input: both are newlines in `text`.
pub const LINE_BREAKS: [u8; 2] = [0xFC, 0xFD];
pub const TERMINATORS: [u8; 2] = [0xFE, 0xFF];

/// Text the decoders refuse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextError(pub String);

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TextError {}

impl From<TextError> for crate::PackError {
    fn from(error: TextError) -> Self {
        crate::PackError(error.0)
    }
}

fn error(message: String) -> TextError {
    TextError(message)
}

/// One charset: the glyph for each byte below the control range.
pub struct Charset {
    glyphs: [Option<char>; 256],
    size: usize,
}

impl Charset {
    fn build(spans: &[(char, char, u8)], singles: &[(u8, char)]) -> Self {
        let mut glyphs = [None; 256];
        let mut size = 0;
        let mut set = |value: u8, glyph: char| {
            assert!(
                glyphs[usize::from(value)].is_none(),
                "charset value 0x{value:02X} assigned twice"
            );
            glyphs[usize::from(value)] = Some(glyph);
            size += 1;
        };
        for &(first, last, base) in spans {
            for (i, c) in (first..=last).enumerate() {
                set(base + i as u8, c);
            }
        }
        for &(value, glyph) in singles {
            set(value, glyph);
        }
        Charset { glyphs, size }
    }

    pub fn glyph(&self, byte: u8) -> Option<char> {
        self.glyphs[usize::from(byte)]
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

/// `general/tables/wincharset.asm`, directive for directive; $56 is the
/// middle dot.
pub fn window_charset() -> &'static Charset {
    static TABLE: OnceLock<Charset> = OnceLock::new();
    TABLE.get_or_init(|| {
        Charset::build(
            &[('A', 'Z', 1), ('a', 'z', 57), ('0', '9', 27)],
            &[
                (0, ' '),
                (0x31, '-'),
                (0x32, '!'),
                (0x33, '?'),
                (0x34, ':'),
                (0x53, '.'),
                (0x54, '\''),
                (0x55, ','),
                (0x56, '\u{B7}'),
            ],
        )
    })
}

/// `script/charset.asm`, directive for directive; $3D/$3E are decoded
/// literally as `<` and `>`, as the disassembly writes them.
pub fn dialogue_charset() -> &'static Charset {
    static TABLE: OnceLock<Charset> = OnceLock::new();
    TABLE.get_or_init(|| {
        Charset::build(
            &[('A', 'Z', 1), ('a', 'z', 27), ('0', '9', 64)],
            &[
                (0, ' '),
                (0x35, '.'),
                (0x36, '\''),
                (0x37, ','),
                (0x38, '*'),
                (0x39, ':'),
                (0x3A, '!'),
                (0x3B, '?'),
                (0x3C, '-'),
                (0x3D, '<'),
                (0x3E, '>'),
                (0x3F, '%'),
                (74, '['),
                (75, '('),
                (76, ')'),
                (77, '='),
            ],
        )
    })
}

pub fn charset(name: &str) -> Result<&'static Charset, TextError> {
    match name {
        WINDOW => Ok(window_charset()),
        DIALOGUE => Ok(dialogue_charset()),
        other => Err(error(format!(
            "Unknown charset '{other}'; expected one of ['dialogue', 'window']"
        ))),
    }
}

/// One `TextCtrlCodesJmpTbl` entry: name, operand bytes (None where not a
/// constant), the routine.
pub struct ControlCode {
    pub name: &'static str,
    pub operand_bytes: Option<u8>,
    pub routine: &'static str,
}

/// `TextCtrlCodesJmpTbl`, indexed by code - $F0.
pub const CONTROL_CODES: [ControlCode; 16] = [
    ControlCode {
        name: "null",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Null",
    },
    ControlCode {
        name: "null",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Null",
    },
    ControlCode {
        name: "action",
        operand_bytes: None,
        routine: "TextCtrlCode_Actions",
    },
    ControlCode {
        name: "keep_npc_facing",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Null",
    },
    ControlCode {
        name: "portrait",
        operand_bytes: None,
        routine: "TextCtrlCode_Portrait",
    },
    ControlCode {
        name: "yes_no",
        operand_bytes: Some(2),
        routine: "TextCtrlCode_YesNo",
    },
    ControlCode {
        name: "event",
        operand_bytes: Some(2),
        routine: "TextCtrlCode_Event",
    },
    ControlCode {
        name: "close_window",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Terminate3",
    },
    ControlCode {
        name: "null",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Null",
    },
    ControlCode {
        name: "delay",
        operand_bytes: Some(1),
        routine: "TextCtrlCode_Delay",
    },
    ControlCode {
        name: "event_flag_check",
        operand_bytes: Some(2),
        routine: "TextCtrlCode_CheckEventFlag",
    },
    ControlCode {
        name: "extended_event_flag_check",
        operand_bytes: Some(3),
        routine: "TextCtrlCode_CheckExtendedEventFlag",
    },
    ControlCode {
        name: "newline",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Newline",
    },
    ControlCode {
        name: "wait_for_input",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Interrupt",
    },
    ControlCode {
        name: "terminate",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Terminate2",
    },
    ControlCode {
        name: "terminate",
        operand_bytes: Some(0),
        routine: "TextCtrlCode_Terminate",
    },
];

/// `TextActionsOffs`, the $F2 sub-dispatch: name and operand bytes. $D and
/// $E exist only under `grand_cross=1`, so they are not retail codes.
pub const TEXT_ACTIONS: [(&str, u8); 13] = [
    ("load_panel", 2),
    ("destroy_last_panel", 0),
    ("destroy_all_panels", 0),
    ("load_sound", 1),
    ("load_sound_2", 1),
    ("null", 0),
    ("update_palette", 0),
    ("zio_eyes_red", 0),
    ("pause_music", 0),
    ("resume_music", 0),
    ("sabotage_alarm_red_palette", 0),
    ("set_event_flag", 1),
    ("elsydeon_broken", 0),
];

fn hex(bytes: &[u8]) -> String {
    crate::png::hex(bytes)
}

/// `_control_length`: the control code at `index`, its total length (code,
/// $F2's action byte, operands) and its segment record.
fn control(
    data: &[u8],
    index: usize,
    portrait_operand_bytes: u8,
) -> Result<(usize, Json), TextError> {
    let code = data[index];
    let spec = &CONTROL_CODES[usize::from(code - CONTROL_BASE)];
    let mut segment = Json::object()
        .with("ctrl", format!("0x{code:02X}"))
        .with("name", spec.name);
    let mut operand_start = index + 1;
    let length = match code {
        0xF2 => {
            let action = *data
                .get(operand_start)
                .ok_or_else(|| error(format!("$F2 at offset {index} has no action byte")))?;
            let (name, operands) = TEXT_ACTIONS.get(usize::from(action)).ok_or_else(|| {
                error(format!(
                    "$F2 at offset {index} selects action 0x{action:02X}, which is not one of \
                     the retail entries of TextActionsOffs"
                ))
            })?;
            operand_start += 1;
            segment.insert("action_id", action);
            segment.insert("action", *name);
            2 + usize::from(*operands)
        }
        0xF4 => 1 + usize::from(portrait_operand_bytes),
        _ => 1 + usize::from(spec.operand_bytes.expect("a constant operand count")),
    };
    if index + length > data.len() {
        return Err(error(format!(
            "control code 0x{code:02X} at offset {index} needs {} more bytes but only {} remain",
            length - 1,
            data.len() - index - 1
        )));
    }
    let operands = &data[operand_start..index + length];
    segment.insert("operands", operands.to_vec());
    if code == 0xF4 {
        segment.insert("portrait_id", operands[0]);
        if portrait_operand_bytes == 2 {
            segment.insert("portrait_location", operands[1]);
        }
    }
    segment.insert("length", length);
    Ok((length, segment))
}

/// `decode_string`: `{"text", "segments", "raw_hex"}`. `text` flattens
/// glyphs and turns $FC/$FD into newlines; `segments` keeps every run and
/// every control record. A byte the charset lacks is an error, not a `?`.
pub fn decode_string(
    data: &[u8],
    charset_name: &str,
    portrait_operand_bytes: u8,
) -> Result<Json, TextError> {
    let table = charset(charset_name)?;
    let mut segments: Vec<Json> = Vec::new();
    let mut text = String::new();
    let mut run = String::new();
    let mut index = 0;
    while index < data.len() {
        let byte = data[index];
        if byte < CONTROL_BASE {
            let glyph = table.glyph(byte).ok_or_else(|| {
                error(format!(
                    "byte 0x{byte:02X} at offset {index} is not in the {charset_name} charset \
                     and is below the 0x{CONTROL_BASE:02X} control range"
                ))
            })?;
            run.push(glyph);
            text.push(glyph);
            index += 1;
            continue;
        }
        let (length, segment) = control(data, index, portrait_operand_bytes)?;
        if !run.is_empty() {
            segments.push(Json::object().with("text", std::mem::take(&mut run)));
        }
        segments.push(segment);
        if LINE_BREAKS.contains(&byte) {
            text.push('\n');
        }
        index += length;
    }
    if !run.is_empty() {
        segments.push(Json::object().with("text", run));
    }
    Ok(Json::object()
        .with("text", text)
        .with("segments", segments)
        .with("raw_hex", hex(data)))
}

/// `decode_name`: a name-table entry, glyphs only.
pub fn decode_name(data: &[u8], charset_name: &str) -> Result<String, TextError> {
    let table = charset(charset_name)?;
    data.iter()
        .enumerate()
        .map(|(index, &byte)| {
            table.glyph(byte).ok_or_else(|| {
                error(format!(
                    "name byte 0x{byte:02X} at offset {index} is not in the {charset_name} charset"
                ))
            })
        })
        .collect()
}

/// `split_terminated`: `(offset in block, payload)` per entry, counting
/// terminators byte by byte as `GetDialogueByID` does; a trailing
/// unterminated remainder is an error.
pub fn split_terminated(block: &[u8], terminator: u8) -> Result<Vec<(usize, &[u8])>, TextError> {
    let mut entries = Vec::new();
    let mut start = 0;
    for (index, &byte) in block.iter().enumerate() {
        if byte == terminator {
            entries.push((start, &block[start..index]));
            start = index + 1;
        }
    }
    if start != block.len() {
        return Err(error(format!(
            "block of {} bytes ends with {} bytes after the last 0x{terminator:02X} terminator",
            block.len(),
            block.len() - start
        )));
    }
    Ok(entries)
}

/// One name table: `text.py`'s `NAME_TABLES` entry and its signature.
pub struct NameTable {
    pub name: &'static str,
    pub label: &'static str,
    pub start: usize,
    pub end: usize,
    pub count: usize,
    pub terminator: u8,
    pub charset: &'static str,
    pub id_base: usize,
    /// The first entry, encoded: checked before anything is decoded.
    pub signature: &'static str,
}

// One row of `NAME_TABLES`, in the Python dict's field order.
#[allow(clippy::too_many_arguments)]
const fn table(
    name: &'static str,
    label: &'static str,
    start: usize,
    end: usize,
    count: usize,
    terminator: u8,
    charset: &'static str,
    id_base: usize,
    signature: &'static str,
) -> NameTable {
    NameTable {
        name,
        label,
        start,
        end,
        count,
        terminator,
        charset,
        id_base,
        signature,
    }
}

/// `NAME_TABLES` with `NAME_TABLE_SIGNATURES`, in the Python order.
pub const NAME_TABLES: [NameTable; 10] = [
    table(
        "character_names",
        "(character names)",
        0x280CD0,
        0x280D07,
        11,
        0xFF,
        WINDOW,
        0,
        "03403952ff",
    ),
    table(
        "profession_names",
        "(profession names)",
        0x280D08,
        0x280D42,
        8,
        0xFF,
        WINDOW,
        0,
        "08150e140512ff",
    ),
    table(
        "enemy_names",
        "EnemyNames",
        0x280D42,
        0x2812E4,
        153,
        0xFF,
        WINDOW,
        0,
        "08050c0518ff",
    ),
    table(
        "enemy_skill_names",
        "EnemySkillNames",
        0x2812E4,
        0x2816BC,
        112,
        0xFF,
        WINDOW,
        1,
        "0e0f1408090e07ff",
    ),
    table(
        "combo_names",
        "ComboNames",
        0x28554C,
        0x2855E1,
        14,
        0xFF,
        WINDOW,
        1,
        "1001120104090e020c17ff",
    ),
    table(
        "item_names",
        "InventoryNames",
        0x2AAFB4,
        0x2AB622,
        160,
        0xFE,
        WINDOW,
        1,
        "040107070512fe",
    ),
    table(
        "technique_names",
        "TechniqueNames",
        0x2AB622,
        0x2AB701,
        40,
        0xFE,
        WINDOW,
        1,
        "060f09fe",
    ),
    table(
        "skill_names",
        "SkillNames",
        0x2AB702,
        0x2AB8A1,
        54,
        0xFE,
        WINDOW,
        1,
        "03120f1313031514fe",
    ),
    table(
        "place_names",
        "PlaceNames",
        0x2AB8A2,
        0x2ABA70,
        54,
        0xFE,
        WINDOW,
        0,
        "1009011401fe",
    ),
    table(
        "item_names_dialogue",
        "InventoryNames2",
        0x2ABA70,
        0x2AC0DE,
        160,
        0xFF,
        DIALOGUE,
        1,
        "040107070512ff",
    ),
];

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

/// `extract_name_table`.
pub fn extract_name_table(data: &[u8], spec: &NameTable) -> Result<Json, TextError> {
    let block = data.get(spec.start..spec.end).ok_or_else(|| {
        error(format!(
            "{} at 0x{:06X} runs past end of ROM",
            spec.label, spec.start
        ))
    })?;
    let entries = split_terminated(block, spec.terminator)?;
    if entries.len() != spec.count {
        return Err(error(format!(
            "{} at 0x{:06X}: found {} entries between 0x{:06X} and 0x{:06X}, expected {}",
            spec.label,
            spec.start,
            entries.len(),
            spec.start,
            spec.end,
            spec.count
        )));
    }
    let mut records = Vec::with_capacity(entries.len());
    for (index, (block_offset, payload)) in entries.into_iter().enumerate() {
        let offset = spec.start + block_offset;
        records.push(
            Json::object()
                .with("id", spec.id_base + index)
                .with("name", decode_name(payload, spec.charset)?)
                .with("rom_offset", format!("0x{offset:06X}"))
                .with("raw_hex", hex(&data[offset..offset + payload.len() + 1])),
        );
    }
    Ok(Json::object()
        .with("label", spec.label)
        .with("rom_offset", format!("0x{:06X}", spec.start))
        .with("rom_end_exclusive", format!("0x{:06X}", spec.end))
        .with("charset", spec.charset)
        .with("terminator", format!("0x{:02X}", spec.terminator))
        .with("id_base", spec.id_base)
        .with("count", records.len())
        .with("entries", records))
}

/// `validate_name_tables`: each table's first encoded entry, and whether it
/// matches.
pub fn validate_name_tables(data: &[u8]) -> Vec<(bool, Json)> {
    NAME_TABLES
        .iter()
        .map(|spec| {
            let expected = unhex(spec.signature);
            let end = (spec.start + expected.len()).min(data.len());
            let actual = data.get(spec.start.min(end)..end).unwrap_or_default();
            let ok = actual == expected.as_slice();
            (
                ok,
                Json::object()
                    .with("label", spec.label)
                    .with("table", spec.name)
                    .with("offset", format!("0x{:06X}", spec.start))
                    .with("ok", ok)
                    .with("expected_hex", hex(&expected))
                    .with("actual_hex", hex(actual)),
            )
        })
        .collect()
}

/// `FIRST_VEHICLE_ITEM_ID`: the vehicles are inventory entries $96..$98.
pub const FIRST_VEHICLE_ITEM_ID: usize = 0x96;

fn field<'a>(record: &'a Json, key: &str) -> Option<&'a Json> {
    let Json::Object(pairs) = record else {
        return None;
    };
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// `vehicle_names`, from the inventory table's entries.
pub fn vehicle_names(item_entries: &[Json]) -> Result<Vec<Json>, TextError> {
    (1..=3usize)
        .map(|vehicle_id| {
            let item_id = FIRST_VEHICLE_ITEM_ID + vehicle_id - 1;
            let item = item_entries
                .iter()
                .find(|entry| field(entry, "id") == Some(&Json::from(item_id)))
                .ok_or_else(|| {
                    error(format!(
                        "vehicle {vehicle_id} expects item id 0x{item_id:02X}"
                    ))
                })?;
            let copy = |key: &str| field(item, key).cloned().unwrap_or(Json::Null);
            Ok(Json::object()
                .with("id", vehicle_id)
                .with("name", copy("name"))
                .with("item_id", item_id)
                .with("rom_offset", copy("rom_offset"))
                .with("raw_hex", copy("raw_hex"))
                .with("source", "InventoryNames"))
        })
        .collect()
}

/// `extract_names`: every localized name table, after their signatures.
pub fn extract_names(data: &[u8]) -> Result<Json, TextError> {
    let validations = validate_name_tables(data);
    let bad: Vec<&str> = NAME_TABLES
        .iter()
        .zip(&validations)
        .filter(|(_, (ok, _))| !ok)
        .map(|(spec, _)| spec.label)
        .collect();
    if !bad.is_empty() {
        return Err(error(format!(
            "Name table signatures do not match: {}",
            bad.join(", ")
        )));
    }
    let mut tables = Json::object();
    let mut entries: Vec<(&str, Json)> = Vec::new();
    let mut item_entries = Vec::new();
    for spec in &NAME_TABLES {
        let table = extract_name_table(data, spec)?;
        let listed = field(&table, "entries").cloned().unwrap_or(Json::Null);
        if spec.name == "item_names"
            && let Json::Array(items) = &listed
        {
            item_entries = items.clone();
        }
        entries.push((spec.name, listed));
        tables.insert(spec.name, table);
    }
    let charsets = Json::object()
        .with(
            WINDOW,
            Json::object()
                .with("source", "general/tables/wincharset.asm")
                .with("size", window_charset().size()),
        )
        .with(
            DIALOGUE,
            Json::object()
                .with("source", "script/charset.asm")
                .with("size", dialogue_charset().size()),
        );
    let mut names = Json::object()
        .with(
            "validation",
            validations
                .into_iter()
                .map(|(_, record)| record)
                .collect::<Vec<_>>(),
        )
        .with("charsets", charsets)
        .with("tables", tables)
        .with("vehicle_names", vehicle_names(&item_entries)?);
    for (name, listed) in entries {
        names.insert(name, listed);
    }
    Ok(names)
}

const DIALOGUE_REGION_START: usize = 0x1DF600;
const DIALOGUE_TREE_COUNT: usize = 43;
const DIALOGUE_BLOB_ALIGNMENT: usize = 16;

/// One Kosinski dialogue tree of the region.
pub struct TreeSpec {
    pub tree: usize,
    pub start: usize,
    pub end: usize,
    pub padding: usize,
}

/// `dialogue_tree_specs`: walk the region's 43 back-to-back streams. Each
/// next blob starts on a 16-byte boundary, across padding that must be zero.
pub fn dialogue_tree_specs(
    data: &[u8],
    decodes: &crate::decode::Decodes,
) -> Result<Vec<TreeSpec>, crate::PackError> {
    let mut specs = Vec::with_capacity(DIALOGUE_TREE_COUNT);
    let mut offset = DIALOGUE_REGION_START;
    for tree in 1..=DIALOGUE_TREE_COUNT {
        let decoded = decodes.get(crate::decode::Call::Kosinski { offset })?;
        let end = offset + decoded.consumed();
        let padding = end.next_multiple_of(DIALOGUE_BLOB_ALIGNMENT) - end;
        if data[end..end + padding].iter().any(|byte| *byte != 0) {
            return Err(crate::PackError(format!(
                "DialogueTree{tree} at 0x{offset:06X} is followed by non-zero padding at \
                 0x{end:06X}"
            )));
        }
        specs.push(TreeSpec {
            tree,
            start: offset,
            end,
            padding,
        });
        offset = end + padding;
    }
    Ok(specs)
}

/// `trim_tree_alignment`: drop the single zero the compressor appends to an
/// odd-length tree (`...$FF $00`); anything else after the last terminator
/// is an error. Returns the block and the bytes dropped.
pub fn trim_tree_alignment<'a>(
    block: &'a [u8],
    label: &str,
) -> Result<(&'a [u8], usize), TextError> {
    match block {
        [] | [.., 0xFF] => Ok((block, 0)),
        [.., 0xFF, 0x00] => Ok((&block[..block.len() - 1], 1)),
        [.., last] => Err(error(format!(
            "{label} decompresses to {} bytes ending 0x{last:02X}, which is neither a terminator \
             nor the single-zero alignment pad",
            block.len()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::dumps;

    #[test]
    fn charsets_hold_their_directives() {
        assert_eq!(window_charset().size(), 71);
        assert_eq!(dialogue_charset().size(), 78);
        assert_eq!(window_charset().glyph(57), Some('a'));
        assert_eq!(dialogue_charset().glyph(27), Some('a'));
        assert_eq!(window_charset().glyph(0x56), Some('\u{B7}'));
        assert_eq!(dialogue_charset().glyph(0x56), None);
        assert!(charset("klingon").is_err());
    }

    #[test]
    fn strings_keep_every_segment() {
        // "Hi" $FC "!" $F2 $03 $07 (load_sound 7) $F4 $05 (portrait 5) $FD
        let data = [0x08, 0x23, 0xFC, 0x3A, 0xF2, 0x03, 0x07, 0xF4, 0x05, 0xFD];
        let decoded = decode_string(&data, DIALOGUE, 1).unwrap();
        let text = dumps(&decoded, true);
        assert!(text.contains("\"text\": \"Hi\\n!\\n\""), "{text}");
        assert!(text.contains("\"action\": \"load_sound\""), "{text}");
        assert!(text.contains("\"portrait_id\": 5"), "{text}");
        assert!(!text.contains("portrait_location"), "{text}");
        assert!(
            text.contains("\"raw_hex\": \"0823fc3af20307f405fd\""),
            "{text}"
        );
        let talk = dumps(
            &decode_string(&[0xF4, 0x05, 0x01], DIALOGUE, 2).unwrap(),
            true,
        );
        assert!(talk.contains("\"portrait_location\": 1"), "{talk}");
    }

    #[test]
    fn strings_refuse_what_python_refuses() {
        assert!(decode_string(&[0x60], DIALOGUE, 1).is_err(), "not a glyph");
        assert!(
            decode_string(&[0xF2], DIALOGUE, 1).is_err(),
            "no action byte"
        );
        assert!(
            decode_string(&[0xF2, 0x0D], DIALOGUE, 1).is_err(),
            "grand cross action"
        );
        assert!(
            decode_string(&[0xF5, 0x00], DIALOGUE, 1).is_err(),
            "short operands"
        );
        assert!(decode_name(&[0x01, 0xF0], WINDOW).is_err());
        assert_eq!(
            decode_name(&[0x03, 0x40, 0x39, 0x52], WINDOW).unwrap(),
            "Chaz"
        );
    }

    #[test]
    fn terminated_blocks_and_alignment() {
        let entries = split_terminated(&[1, 0xFF, 0xFF, 2, 0xFF], 0xFF).unwrap();
        assert_eq!(
            entries,
            vec![(0, &[1u8][..]), (2, &[][..]), (3, &[2u8][..])]
        );
        assert!(split_terminated(&[1, 0xFF, 2], 0xFF).is_err());
        assert_eq!(trim_tree_alignment(&[1, 0xFF], "t").unwrap().1, 0);
        assert_eq!(
            trim_tree_alignment(&[1, 0xFF, 0], "t").unwrap(),
            (&[1u8, 0xFF][..], 1)
        );
        assert!(trim_tree_alignment(&[1, 0xFF, 0, 0], "t").is_err());
    }

    #[test]
    fn a_wrong_image_fails_its_signatures() {
        let error = extract_names(&vec![0; 0x2AC0DE]).unwrap_err();
        assert!(
            error
                .0
                .starts_with("Name table signatures do not match: (character names)")
        );
    }
}
