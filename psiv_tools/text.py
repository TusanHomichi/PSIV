"""PSIV text: character sets, name tables, and the compressed dialogue trees.

The cartridge does not store text as ASCII. It stores font-tile indices, and it
uses *two different* index assignments depending on which font the text is
drawn with:

    window charset    `general/tables/wincharset.asm`
                      'A'-'Z' = 1.., '0'-'9' = 27.., 'a'-'z' = 57..
                      used by every menu/battle name table

    dialogue charset  `script/charset.asm`
                      'A'-'Z' = 1.., 'a'-'z' = 27.., '0'-'9' = 64..
                      used by the dialogue trees and by `InventoryNames2`

The two agree only on `' '` and `'A'-'Z'`, so a name table decoded with the
wrong table looks correct until the first lowercase letter or digit. Which one
applies is not a guess: `ps4.asm` includes one charset file or the other
immediately above each table, and every table below records which.

Bytes `$F0-$FF` are control codes, never glyphs: `RunText_CharacterLoop`
compares each byte against `$F0` and, at or above it, jumps through
`TextCtrlCodesJmpTbl` indexed by the low nibble. `CONTROL_CODES` is a
transcription of that 16-entry table, and each code's operand length is taken
from the routine it dispatches to rather than from prose.

Dialogue lives in 43 Kosinski-compressed blobs (`DialogueTreesToRAM` calls
`KosDecomp` on the tree address). Inside a decompressed tree, entry N is found
by counting `$FF` bytes from the start -- `GetDialogueByID` does exactly that,
byte by byte, which is the same addressing scheme the battle formations use.
"""

from __future__ import annotations

import hashlib
from typing import Any, Iterable

from .kosinski import decompress

WINDOW = "window"
DIALOGUE = "dialogue"

CONTROL_BASE = 0xF0


class TextError(ValueError):
    pass


def _charset(pairs: Iterable[tuple[int, str]]) -> dict[int, str]:
    table: dict[int, str] = {}
    for value, char in pairs:
        if value in table:
            raise TextError(f"charset value 0x{value:02X} assigned twice")
        table[value] = char
    return table


def _span(first: str, last: str, base: int) -> list[tuple[int, str]]:
    """One `charset 'a', 'z', base` directive."""
    return [(base + i, chr(c)) for i, c in enumerate(range(ord(first), ord(last) + 1))]


# `general/tables/wincharset.asm`, directive for directive. $56 is the
# middle dot (0xB7 in the iso-8859-1 source file).
WINDOW_CHARSET: dict[int, str] = _charset([
    *_span("A", "Z", 1),
    *_span("a", "z", 57),
    *_span("0", "9", 27),
    (0, " "),
    (0x31, "-"), (0x32, "!"), (0x33, "?"), (0x34, ":"),
    (0x53, "."), (0x54, "'"), (0x55, ","), (0x56, "·"),
])

# `script/charset.asm`, directive for directive. This one covers 0x00..0x4D
# with no holes. $3D/$3E are written as '<' and '>' in the disassembly source
# but the font draws them as opening and closing double quotation marks; they
# are decoded literally here so the decode stays a transcription.
DIALOGUE_CHARSET: dict[int, str] = _charset([
    *_span("A", "Z", 1),
    *_span("a", "z", 27),
    *_span("0", "9", 64),
    (0, " "),
    (0x35, "."), (0x36, "'"), (0x37, ","), (0x38, "*"), (0x39, ":"),
    (0x3A, "!"), (0x3B, "?"), (0x3C, "-"), (0x3D, "<"), (0x3E, ">"),
    (0x3F, "%"), (74, "["), (75, "("), (76, ")"), (77, "="),
])

CHARSETS = {WINDOW: WINDOW_CHARSET, DIALOGUE: DIALOGUE_CHARSET}

# `TextCtrlCodesJmpTbl` in ps4.asm, one entry per code, plus the number of
# operand bytes the target routine consumes. Where the count is None the
# length is not a constant and is resolved separately.
CONTROL_CODES: dict[int, dict[str, Any]] = {
    0xF0: {"name": "null", "operand_bytes": 0, "routine": "TextCtrlCode_Null"},
    0xF1: {"name": "null", "operand_bytes": 0, "routine": "TextCtrlCode_Null"},
    0xF2: {"name": "action", "operand_bytes": None, "routine": "TextCtrlCode_Actions"},
    0xF3: {"name": "keep_npc_facing", "operand_bytes": 0, "routine": "TextCtrlCode_Null"},
    0xF4: {"name": "portrait", "operand_bytes": None, "routine": "TextCtrlCode_Portrait"},
    0xF5: {"name": "yes_no", "operand_bytes": 2, "routine": "TextCtrlCode_YesNo"},
    0xF6: {"name": "event", "operand_bytes": 2, "routine": "TextCtrlCode_Event"},
    0xF7: {"name": "close_window", "operand_bytes": 0, "routine": "TextCtrlCode_Terminate3"},
    0xF8: {"name": "null", "operand_bytes": 0, "routine": "TextCtrlCode_Null"},
    0xF9: {"name": "delay", "operand_bytes": 1, "routine": "TextCtrlCode_Delay"},
    0xFA: {"name": "event_flag_check", "operand_bytes": 2, "routine": "TextCtrlCode_CheckEventFlag"},
    0xFB: {"name": "extended_event_flag_check", "operand_bytes": 3, "routine": "TextCtrlCode_CheckExtendedEventFlag"},
    0xFC: {"name": "newline", "operand_bytes": 0, "routine": "TextCtrlCode_Newline"},
    0xFD: {"name": "wait_for_input", "operand_bytes": 0, "routine": "TextCtrlCode_Interrupt"},
    0xFE: {"name": "terminate", "operand_bytes": 0, "routine": "TextCtrlCode_Terminate2"},
    0xFF: {"name": "terminate", "operand_bytes": 0, "routine": "TextCtrlCode_Terminate"},
}

# `TextActionsOffs`: the sub-dispatch for $F2. The operand count is how many
# bytes past the action byte the handler advances a0. Entries $D and $E of that
# table only exist under `grand_cross=1` and are therefore not retail codes.
TEXT_ACTIONS: dict[int, dict[str, Any]] = {
    0x0: {"name": "load_panel", "operand_bytes": 2},
    0x1: {"name": "destroy_last_panel", "operand_bytes": 0},
    0x2: {"name": "destroy_all_panels", "operand_bytes": 0},
    0x3: {"name": "load_sound", "operand_bytes": 1},
    0x4: {"name": "load_sound_2", "operand_bytes": 1},
    0x5: {"name": "null", "operand_bytes": 0},
    0x6: {"name": "update_palette", "operand_bytes": 0},
    0x7: {"name": "zio_eyes_red", "operand_bytes": 0},
    0x8: {"name": "pause_music", "operand_bytes": 0},
    0x9: {"name": "resume_music", "operand_bytes": 0},
    0xA: {"name": "sabotage_alarm_red_palette", "operand_bytes": 0},
    0xB: {"name": "set_event_flag", "operand_bytes": 1},
    0xC: {"name": "elsydeon_broken", "operand_bytes": 0},
}

TERMINATORS = (0xFE, 0xFF)
# $FC ends a line, $FD stops for input before continuing. Both become newlines
# in the flattened `text` field; `segments` keeps them apart.
LINE_BREAKS = (0xFC, 0xFD)


# ---------------------------------------------------------------------------
# Name tables
#
# Every offset below was located by assembling the English (revision > 0)
# variant out of ps4.asm through the charset named in its `charset` field and
# then requiring a unique match in the retail image, the same discipline the
# formation blobs use. Ranges are end-exclusive.
#
# `id_base` matches the id convention the existing extractors already use, so
# these lists line up 1:1 with `core.extract_*` output: enemies and characters
# are 0-based, everything else is 1-based (see formations.py).
# ---------------------------------------------------------------------------
NAME_TABLES: list[dict[str, Any]] = [
    {"name": "character_names", "label": "(character names)", "start": 0x280CD0,
     "end": 0x280D07, "count": 11, "terminator": 0xFF, "charset": WINDOW, "id_base": 0},
    {"name": "profession_names", "label": "(profession names)", "start": 0x280D08,
     "end": 0x280D42, "count": 8, "terminator": 0xFF, "charset": WINDOW, "id_base": 0},
    {"name": "enemy_names", "label": "EnemyNames", "start": 0x280D42,
     "end": 0x2812E4, "count": 153, "terminator": 0xFF, "charset": WINDOW, "id_base": 0},
    {"name": "enemy_skill_names", "label": "EnemySkillNames", "start": 0x2812E4,
     "end": 0x2816BC, "count": 112, "terminator": 0xFF, "charset": WINDOW, "id_base": 1},
    {"name": "combo_names", "label": "ComboNames", "start": 0x28554C,
     "end": 0x2855E1, "count": 14, "terminator": 0xFF, "charset": WINDOW, "id_base": 1},
    {"name": "item_names", "label": "InventoryNames", "start": 0x2AAFB4,
     "end": 0x2AB622, "count": 160, "terminator": 0xFE, "charset": WINDOW, "id_base": 1},
    {"name": "technique_names", "label": "TechniqueNames", "start": 0x2AB622,
     "end": 0x2AB701, "count": 40, "terminator": 0xFE, "charset": WINDOW, "id_base": 1},
    {"name": "skill_names", "label": "SkillNames", "start": 0x2AB702,
     "end": 0x2AB8A1, "count": 54, "terminator": 0xFE, "charset": WINDOW, "id_base": 1},
    {"name": "place_names", "label": "PlaceNames", "start": 0x2AB8A2,
     "end": 0x2ABA70, "count": 54, "terminator": 0xFE, "charset": WINDOW, "id_base": 0},
    {"name": "item_names_dialogue", "label": "InventoryNames2", "start": 0x2ABA70,
     "end": 0x2AC0DE, "count": 160, "terminator": 0xFF, "charset": DIALOGUE, "id_base": 1},
]

# First entry of each table, encoded. Checked before anything is decoded so a
# wrong build cannot silently produce plausible-looking names.
NAME_TABLE_SIGNATURES: dict[str, str] = {
    "character_names": "03403952ff",          # "Chaz"
    "profession_names": "08150e140512ff",     # "HUNTER"
    "enemy_names": "08050c0518ff",            # "HELEX"
    "enemy_skill_names": "0e0f1408090e07ff",  # "NOTHING"
    "combo_names": "1001120104090e020c17ff",  # "PARADINBLW"
    "item_names": "040107070512fe",           # "DAGGER"
    "technique_names": "060f09fe",            # "FOI"
    "skill_names": "03120f1313031514fe",      # "CROSSCUT"
    "place_names": "1009011401fe",            # "PIATA"
    "item_names_dialogue": "040107070512ff",  # "DAGGER"
}


# ---------------------------------------------------------------------------
# Dialogue trees
#
# `DialogueTree1` sits at a $100 boundary and the 43 blobs follow one another,
# each padded to a 16-byte boundary with zeros (the padding `compress_script.py`
# documents). Only the start of each blob is recorded: the end is whatever the
# decompressor consumes, and `extract_dialogue` requires the next blob to begin
# at the next 16-byte boundary after it, which turns the whole chain into one
# continuous length check.
#
# `portrait_operand_bytes` is 2 for exactly trees 31 and 32 and 1 everywhere
# else. `TextCtrlCode_Portrait` reads a second operand byte only when
# `Game_Mode_Routine` is 4, and `Win_TalkDialogue` -- the Talk command -- is
# the only caller that runs in that mode; it selects DialogueTree31 for talk
# ids below $1A and DialogueTree32 at or above it. The data agrees exactly:
# in trees 31 and 32 all 84 and 89 $F4 codes are followed by a byte in 0..2,
# and across the other 41 trees the one-operand reading is the only one that
# consumes every blob without hitting a byte outside the font.
# ---------------------------------------------------------------------------
DIALOGUE_REGION_START = 0x1DF600
DIALOGUE_TREE_COUNT = 43
DIALOGUE_BLOB_ALIGNMENT = 16
TALK_DIALOGUE_TREES = (31, 32)


def _charset_table(charset: str) -> dict[int, str]:
    try:
        return CHARSETS[charset]
    except KeyError:
        raise TextError(f"Unknown charset {charset!r}; expected one of {sorted(CHARSETS)}")


def _control_length(data: bytes, index: int, portrait_operand_bytes: int) -> tuple[int, dict[str, Any]]:
    """Return `(total_length, segment)` for the control code at `index`."""
    code = data[index]
    spec = CONTROL_CODES[code]
    segment: dict[str, Any] = {"ctrl": f"0x{code:02X}", "name": spec["name"]}

    operand_start = index + 1
    if code == 0xF2:
        if operand_start >= len(data):
            raise TextError(f"$F2 at offset {index} has no action byte")
        action = data[operand_start]
        action_spec = TEXT_ACTIONS.get(action)
        if action_spec is None:
            raise TextError(
                f"$F2 at offset {index} selects action 0x{action:02X}, which is not "
                "one of the retail entries of TextActionsOffs"
            )
        operand_start += 1
        length = 2 + action_spec["operand_bytes"]
        segment["action_id"] = action
        segment["action"] = action_spec["name"]
    elif code == 0xF4:
        length = 1 + portrait_operand_bytes
    else:
        length = 1 + spec["operand_bytes"]

    if index + length > len(data):
        raise TextError(
            f"control code 0x{code:02X} at offset {index} needs {length - 1} more "
            f"bytes but only {len(data) - index - 1} remain"
        )

    operands = list(data[operand_start:index + length])
    segment["operands"] = operands
    if code == 0xF4:
        segment["portrait_id"] = operands[0]
        if portrait_operand_bytes == 2:
            segment["portrait_location"] = operands[1]
    # `length` counts the code byte itself, plus $F2's action byte, plus the
    # operands, so summing it across segments reconstructs the entry's length.
    segment["length"] = length
    return length, segment


def decode_string(
    data: bytes,
    *,
    charset: str = DIALOGUE,
    portrait_operand_bytes: int = 1,
) -> dict[str, Any]:
    """Decode one encoded string into text plus an ordered segment list.

    `text` is the readable flattening: glyphs as characters, `$FC` and `$FD`
    as newlines, every other control code contributing nothing. `segments`
    loses nothing -- each element is either `{"text": ...}` or a control record
    carrying the raw code, its name and its operand bytes -- so a byte that the
    decoder cannot name still appears as itself rather than being dropped.

    Raises `TextError` on any byte the charset does not define. Unmapped glyph
    bytes are a decode failure, not a "?" to paper over: all 43 retail dialogue
    trees and all ten name tables parse with zero of them.
    """
    table = _charset_table(charset)
    segments: list[dict[str, Any]] = []
    text: list[str] = []
    run: list[str] = []
    index = 0

    def flush() -> None:
        if run:
            segments.append({"text": "".join(run)})
            run.clear()

    while index < len(data):
        byte = data[index]
        if byte < CONTROL_BASE:
            char = table.get(byte)
            if char is None:
                raise TextError(
                    f"byte 0x{byte:02X} at offset {index} is not in the {charset} "
                    f"charset and is below the 0x{CONTROL_BASE:02X} control range"
                )
            run.append(char)
            text.append(char)
            index += 1
            continue

        length, segment = _control_length(data, index, portrait_operand_bytes)
        flush()
        segments.append(segment)
        if byte in LINE_BREAKS:
            text.append("\n")
        index += length

    flush()
    return {
        "text": "".join(text),
        "segments": segments,
        "raw_hex": data.hex(),
    }


def decode_name(data: bytes, *, charset: str = WINDOW) -> str:
    """Decode a name-table entry, which must be glyphs only."""
    table = _charset_table(charset)
    out = []
    for index, byte in enumerate(data):
        char = table.get(byte)
        if char is None:
            raise TextError(
                f"name byte 0x{byte:02X} at offset {index} is not in the {charset} charset"
            )
        out.append(char)
    return "".join(out)


def split_terminated(block: bytes, terminator: int) -> list[tuple[int, bytes]]:
    """Split on `terminator`, returning `(offset_within_block, payload)` pairs.

    The game locates entry N by counting terminator bytes one at a time
    (`GetDialogueByID`), so this scan is byte-by-byte too and a trailing
    unterminated remainder is an error rather than a final entry.
    """
    entries: list[tuple[int, bytes]] = []
    start = 0
    for index, byte in enumerate(block):
        if byte == terminator:
            entries.append((start, block[start:index]))
            start = index + 1
    if start != len(block):
        raise TextError(
            f"block of {len(block)} bytes ends with {len(block) - start} bytes after "
            f"the last 0x{terminator:02X} terminator"
        )
    return entries


def extract_name_table(data: bytes, spec: dict[str, Any]) -> dict[str, Any]:
    start, end = spec["start"], spec["end"]
    block = data[start:end]
    if len(block) != end - start:
        raise TextError(f"{spec['label']} at 0x{start:06X} runs past end of ROM")

    entries = split_terminated(block, spec["terminator"])
    if len(entries) != spec["count"]:
        raise TextError(
            f"{spec['label']} at 0x{start:06X}: found {len(entries)} entries between "
            f"0x{start:06X} and 0x{end:06X}, expected {spec['count']}"
        )

    records = []
    for index, (block_offset, payload) in enumerate(entries):
        offset = start + block_offset
        records.append({
            "id": spec["id_base"] + index,
            "name": decode_name(payload, charset=spec["charset"]),
            "rom_offset": f"0x{offset:06X}",
            "raw_hex": data[offset:offset + len(payload) + 1].hex(),
        })
    return {
        "label": spec["label"],
        "rom_offset": f"0x{start:06X}",
        "rom_end_exclusive": f"0x{end:06X}",
        "charset": spec["charset"],
        "terminator": f"0x{spec['terminator']:02X}",
        "id_base": spec["id_base"],
        "count": len(records),
        "entries": records,
    }


def validate_name_tables(data: bytes) -> list[dict[str, Any]]:
    """Check the first encoded entry of every table before decoding anything."""
    results = []
    for spec in NAME_TABLES:
        expected = bytes.fromhex(NAME_TABLE_SIGNATURES[spec["name"]])
        actual = data[spec["start"]:spec["start"] + len(expected)]
        results.append({
            "label": spec["label"],
            "table": spec["name"],
            "offset": f"0x{spec['start']:06X}",
            "ok": actual == expected,
            "expected_hex": expected.hex(),
            "actual_hex": actual.hex(),
        })
    return results


# The three vehicles have no name table of their own: they are inventory
# entries. `ps4.constants.asm` pins two of the three ids -- `ItemID_LandRover
# = $96` and `ItemID_IceDigger = $97` -- and the Hydrofoil follows at $98, in
# the same order as `VehicleID_LandRover = 1`, `..IceDigger = 2`,
# `..Hydrofoil = 3`.
FIRST_VEHICLE_ITEM_ID = 0x96


def vehicle_names(item_entries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Vehicle display names, taken from the inventory table they live in."""
    by_id = {entry["id"]: entry for entry in item_entries}
    out = []
    for vehicle_id in (1, 2, 3):
        item_id = FIRST_VEHICLE_ITEM_ID + vehicle_id - 1
        item = by_id.get(item_id)
        if item is None:
            raise TextError(f"vehicle {vehicle_id} expects item id 0x{item_id:02X}")
        out.append({
            "id": vehicle_id,
            "name": item["name"],
            "item_id": item_id,
            "rom_offset": item["rom_offset"],
            "raw_hex": item["raw_hex"],
            "source": "InventoryNames",
        })
    return out


def extract_names(data: bytes) -> dict[str, Any]:
    """Decode every localized name table out of the retail image."""
    validations = validate_name_tables(data)
    bad = [v["label"] for v in validations if not v["ok"]]
    if bad:
        raise TextError("Name table signatures do not match: " + ", ".join(bad))

    tables = {spec["name"]: extract_name_table(data, spec) for spec in NAME_TABLES}
    entries = {name: table["entries"] for name, table in tables.items()}
    return {
        "validation": validations,
        "charsets": {
            WINDOW: {"source": "general/tables/wincharset.asm", "size": len(WINDOW_CHARSET)},
            DIALOGUE: {"source": "script/charset.asm", "size": len(DIALOGUE_CHARSET)},
        },
        "tables": tables,
        "vehicle_names": vehicle_names(entries["item_names"]),
        **entries,
    }


def dialogue_tree_specs(data: bytes) -> list[dict[str, Any]]:
    """Walk the dialogue region, one Kosinski blob per tree.

    Each blob's length is whatever `KosDecomp` consumes; the next blob must
    begin at the following 16-byte boundary and every padding byte between
    them must be zero. Chained across all 43 trees that is a single check
    that the region really is 43 back-to-back streams.
    """
    specs = []
    offset = DIALOGUE_REGION_START
    for tree in range(1, DIALOGUE_TREE_COUNT + 1):
        decompressed, consumed = decompress(data, offset)
        end = offset + consumed
        padding = (-end) % DIALOGUE_BLOB_ALIGNMENT
        if any(data[end:end + padding]):
            raise TextError(
                f"DialogueTree{tree} at 0x{offset:06X} is followed by non-zero "
                f"padding at 0x{end:06X}"
            )
        specs.append({
            "tree": tree,
            "label": f"DialogueTree{tree}",
            "start": offset,
            "end": end,
            "padding": padding,
            "decompressed": decompressed,
        })
        offset = end + padding
    return specs


def trim_tree_alignment(block: bytes, label: str) -> tuple[bytes, int]:
    """Drop the even-length pad byte some decompressed trees end with.

    `compress_script.py` appends a single zero when the uncompressed tree has
    an odd length, so 20 of the 43 retail trees decompress to `...$FF $00`.
    Only that exact shape is accepted; anything else after the final
    terminator is a decode failure.
    """
    if not block or block[-1] == 0xFF:
        return block, 0
    if len(block) >= 2 and block[-1] == 0x00 and block[-2] == 0xFF:
        return block[:-1], 1
    raise TextError(
        f"{label} decompresses to {len(block)} bytes ending 0x{block[-1]:02X}, "
        "which is neither a terminator nor the single-zero alignment pad"
    )


def extract_dialogue(data: bytes) -> dict[str, Any]:
    """Decompress and decode all 43 dialogue trees."""
    trees = []
    total_entries = 0
    for spec in dialogue_tree_specs(data):
        start, end = spec["start"], spec["end"]
        block, alignment_pad = trim_tree_alignment(spec["decompressed"], spec["label"])
        compressed = data[start:end]
        portrait_operand_bytes = 2 if spec["tree"] in TALK_DIALOGUE_TREES else 1

        source = {
            "label": spec["label"],
            "rom_offset": f"0x{start:06X}",
            "rom_end_exclusive": f"0x{end:06X}",
            "compression": "kosinski",
            "compressed_size": len(compressed),
            "compressed_sha256": hashlib.sha256(compressed).hexdigest(),
            "decompressed_size": len(spec["decompressed"]),
            "alignment_pad_bytes": alignment_pad,
            "padding_bytes": spec["padding"],
        }

        entries = []
        for index, (block_offset, payload) in enumerate(split_terminated(block, 0xFF)):
            decoded = decode_string(
                payload,
                charset=DIALOGUE,
                portrait_operand_bytes=portrait_operand_bytes,
            )
            entries.append({
                "id": index,
                "block_offset": f"0x{block_offset:04X}",
                **decoded,
            })
        total_entries += len(entries)

        trees.append({
            "tree": spec["tree"],
            "portrait_operand_bytes": portrait_operand_bytes,
            "is_talk_tree": spec["tree"] in TALK_DIALOGUE_TREES,
            "entry_count": len(entries),
            **source,
            "entries": entries,
        })

    last = trees[-1]
    return {
        "region": {
            "start": f"0x{DIALOGUE_REGION_START:06X}",
            "end_exclusive": last["rom_end_exclusive"],
            "tree_count": len(trees),
            "blob_alignment": DIALOGUE_BLOB_ALIGNMENT,
        },
        "control_codes": {
            f"0x{code:02X}": dict(spec) for code, spec in sorted(CONTROL_CODES.items())
        },
        "text_actions": {f"0x{k:X}": v for k, v in sorted(TEXT_ACTIONS.items())},
        "total_entries": total_entries,
        "trees": trees,
    }


def extract_text(data: bytes) -> dict[str, Any]:
    """Everything this module owns, in one record."""
    return {"names": extract_names(data), "dialogue": extract_dialogue(data)}
