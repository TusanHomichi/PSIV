import re
import unittest
from pathlib import Path

from psiv_tools.core import (
    CHARACTER_NAMES,
    PROFESSIONS,
    TABLES,
    TECHNIQUE_NAMES,
    read_rom,
)
from psiv_tools.text import (
    CONTROL_CODES,
    DIALOGUE,
    DIALOGUE_CHARSET,
    DIALOGUE_REGION_START,
    DIALOGUE_TREE_COUNT,
    NAME_TABLES,
    TALK_DIALOGUE_TREES,
    TEXT_ACTIONS,
    WINDOW,
    WINDOW_CHARSET,
    TextError,
    decode_name,
    decode_string,
    extract_dialogue,
    extract_names,
    split_terminated,
    trim_tree_alignment,
)

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
DISASM = ROOT / "reference" / "ps4disasm"
SCRIPT_DIR = DISASM / "script"

CHARSET_SOURCES = {
    WINDOW: DISASM / "general" / "tables" / "wincharset.asm",
    DIALOGUE: SCRIPT_DIR / "charset.asm",
}

# Dialogue trees whose retail bytes are *not* reproduced by the disassembly's
# "dialogue N.asm" source. Each is a defect in the reference tree, not in the
# decoder, and each is pinned here so a future clone that fixes one makes this
# test fail loudly instead of quietly widening the exception list.
#
#   17  the fork rewrote the Alys/Chaz opening; 84 separate edits
#   19  ROM has a trailing space at 0x0E53 the source drops
#   29  ROM has a trailing space at 0x03F7 the source drops
#   31  source has an extra $02 portrait-location byte at 0x0668
#   34  ROM has a $FB control byte at 0x119E the source drops
#
# 19/29/31/34 additionally carry one spurious 0x00 late in the source, which
# is why each of them still assembles to the retail byte count.
ORACLE_MISMATCHES = {17, 19, 29, 31, 34}

# Retail values for the options the disassembly's sources are conditional on.
ORACLE_SYMBOLS = {"skip_opening": 0}

CHARSET_RANGE = re.compile(r"^\s*charset\s+'(.)'\s*,\s*'(.)'\s*,\s*(\$[0-9A-Fa-f]+|\d+)\s*$")
CHARSET_SINGLE = re.compile(r"^\s*charset\s+'(.)'\s*,\s*(\$[0-9A-Fa-f]+|\d+)\s*$")
CHARSET_SINGLE_NUM = re.compile(r"^\s*charset\s+(\$[0-9A-Fa-f]+)\s*,\s*(\$[0-9A-Fa-f]+|\d+)\s*$")


def _number(token: str) -> int:
    token = token.strip()
    return int(token[1:], 16) if token.startswith("$") else int(token)


def read_charset(path: Path) -> dict[int, str]:
    """Re-derive a charset from the disassembly's own `charset` directives.

    Only the three directive forms the assembler uses are accepted; anything
    else means the file grew a construct this parser would silently ignore, so
    it fails instead.
    """
    table: dict[int, str] = {}
    for lineno, raw in enumerate(path.read_text(encoding="iso-8859-1").splitlines(), start=1):
        line = raw.rstrip()
        if not line.strip():
            continue
        match = CHARSET_RANGE.match(line)
        if match:
            base = _number(match.group(3))
            for index, code in enumerate(range(ord(match.group(1)), ord(match.group(2)) + 1)):
                table[base + index] = chr(code)
            continue
        match = CHARSET_SINGLE.match(line)
        if match:
            table[_number(match.group(2))] = match.group(1)
            continue
        match = CHARSET_SINGLE_NUM.match(line)
        if match:
            table[_number(match.group(2))] = chr(_number(match.group(1)))
            continue
        raise AssertionError(f"{path.name}:{lineno}: unrecognised charset directive {line!r}")
    return table


DC_B = re.compile(r"^\s*dc\.b\s+(.*)$")
DC_W = re.compile(r"^\s*dc\.w\s+(\$[0-9A-Fa-f]+|\d+)\s*(;.*)?$")
IF = re.compile(r"^\s*if\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)\s*$")
ELSE = re.compile(r"^\s*else\s*$")
ENDIF = re.compile(r"^\s*endif\s*$")
HEX_DIGITS = set("0123456789abcdefABCDEF")


def assemble_dialogue_source(path: Path, charset: dict[str, int], symbols: dict[str, int]) -> bytes:
    """Assemble one "dialogue N.asm" data listing into raw tree bytes.

    Nothing here is a general assembler: only `dc.b`, `dc.w` and a flat
    `if/else/endif` on a numeric option are accepted, and an unmappable
    character or an unexpected directive raises rather than being coerced --
    `compress_script.py` silently substitutes a space for unknown characters,
    and an oracle that does the same could hide a charset error.
    """
    out = bytearray()
    condition: list[tuple[bool, bool]] = []

    for lineno, raw in enumerate(path.read_text(encoding="iso-8859-1").splitlines(), start=1):
        line = raw.rstrip()
        match = IF.match(line)
        if match:
            active = all(a for a, _ in condition) and symbols[match.group(1)] == int(match.group(2))
            condition.append((active, active))
            continue
        if ELSE.match(line):
            _, taken = condition[-1]
            outer = all(a for a, _ in condition[:-1])
            condition[-1] = (outer and not taken, True)
            continue
        if ENDIF.match(line):
            condition.pop()
            continue
        if not all(a for a, _ in condition):
            continue

        match = DC_W.match(line)
        if match:
            out += _number(match.group(1)).to_bytes(2, "big")
            continue
        match = DC_B.match(line)
        if match:
            out += _assemble_dc_b(match.group(1), charset, path, lineno)
            continue
        if line.split(";", 1)[0].strip():
            raise AssertionError(f"{path.name}:{lineno}: unexpected directive {line!r}")

    if condition:
        raise AssertionError(f"{path.name}: unterminated if")
    if len(out) % 2:
        out.append(0)  # compress_script.py pads the tree to an even length
    return bytes(out)


def _assemble_dc_b(operands: str, charset: dict[str, int], path: Path, lineno: int) -> bytes:
    out = bytearray()
    index = 0
    while index < len(operands):
        char = operands[index]
        if char in " \t,":
            index += 1
        elif char == ";":
            break
        elif char == '"':
            index += 1
            while index < len(operands):
                if operands[index] == '"':
                    if operands[index + 1:index + 2] == '"':
                        out.append(charset['"'])
                        index += 2
                        continue
                    index += 1
                    break
                if operands[index] not in charset:
                    raise AssertionError(
                        f"{path.name}:{lineno}: {operands[index]!r} is not in the charset"
                    )
                out.append(charset[operands[index]])
                index += 1
        elif char == "$":
            end = index + 1
            while end < len(operands) and operands[end] in HEX_DIGITS:
                end += 1
            out.append(int(operands[index + 1:end], 16))
            index = end
        elif char.isdigit():
            end = index
            while end < len(operands) and operands[end].isdigit():
                end += 1
            out.append(int(operands[index:end]))
            index = end
        else:
            raise AssertionError(f"{path.name}:{lineno}: unexpected operand at {operands[index:]!r}")
    return bytes(out)


class TestCharsetTables(unittest.TestCase):
    """Charset facts that need neither the ROM nor the disassembly."""

    def test_the_two_charsets_disagree_where_it_matters(self):
        """Same uppercase, different lowercase and digits.

        This is the whole reason each name table records which font it uses:
        a table decoded with the wrong charset looks right until its first
        lowercase letter or digit.
        """
        for code in range(1, 27):
            self.assertEqual(WINDOW_CHARSET[code], DIALOGUE_CHARSET[code])
        self.assertEqual(WINDOW_CHARSET[0], " ")
        self.assertEqual(DIALOGUE_CHARSET[0], " ")
        self.assertEqual(WINDOW_CHARSET[27], "0")
        self.assertEqual(DIALOGUE_CHARSET[27], "a")
        self.assertEqual(WINDOW_CHARSET[57], "a")
        self.assertEqual(DIALOGUE_CHARSET[64], "0")

    def test_dialogue_charset_covers_every_value_below_the_control_range(self):
        """0x00..0x4D with no holes; 0x4E..0xEF are simply unused."""
        self.assertEqual(sorted(DIALOGUE_CHARSET), list(range(0, 78)))

    def test_window_charset_holes_are_real(self):
        """The window font leaves 0x25..0x30 and 0x35..0x38 undefined."""
        self.assertEqual(max(WINDOW_CHARSET), 0x56)
        for code in [*range(0x25, 0x31), *range(0x35, 0x39)]:
            self.assertNotIn(code, WINDOW_CHARSET)

    def test_control_code_table_is_complete(self):
        self.assertEqual(sorted(CONTROL_CODES), list(range(0xF0, 0x100)))
        self.assertEqual(CONTROL_CODES[0xFC]["name"], "newline")
        self.assertEqual(CONTROL_CODES[0xFF]["name"], "terminate")
        # $F2 and $F4 are the only two whose length is not a constant.
        variable = [code for code, spec in CONTROL_CODES.items() if spec["operand_bytes"] is None]
        self.assertEqual(variable, [0xF2, 0xF4])

    def test_grand_cross_only_text_actions_are_excluded(self):
        """TextActionsOffs entries $D and $E are behind `if grand_cross=1`."""
        self.assertEqual(sorted(TEXT_ACTIONS), list(range(0x0, 0xD)))


@unittest.skipUnless(DISASM.is_dir(), f"disassembly oracle not present at {DISASM}")
class TestCharsetsAgainstDisassembly(unittest.TestCase):
    """Both tables are transcriptions, so re-parse the sources and compare."""

    def test_window_charset_matches_wincharset_asm(self):
        self.assertEqual(read_charset(CHARSET_SOURCES[WINDOW]), WINDOW_CHARSET)

    def test_dialogue_charset_matches_script_charset_asm(self):
        self.assertEqual(read_charset(CHARSET_SOURCES[DIALOGUE]), DIALOGUE_CHARSET)


class TestDecodeString(unittest.TestCase):
    """Decoder unit tests. No ROM, no disassembly."""

    def test_decode_name_uses_the_window_font(self):
        self.assertEqual(decode_name(bytes([8, 5, 12, 5, 24])), "HELEX")
        self.assertEqual(decode_name(bytes([3, 0x40, 0x39, 0x52])), "Chaz")

    def test_the_same_bytes_decode_differently_per_charset(self):
        encoded = bytes([1, 27, 64])
        self.assertEqual(decode_name(encoded, charset=WINDOW), "A0h")
        self.assertEqual(decode_name(encoded, charset=DIALOGUE), "Aa0")

    def test_line_breaks_become_newlines_and_stay_in_segments(self):
        decoded = decode_string(bytes([1, 0xFC, 2, 0xFD, 3]))
        self.assertEqual(decoded["text"], "A\nB\nC")
        self.assertEqual(
            [s.get("name", s.get("text")) for s in decoded["segments"]],
            ["A", "newline", "B", "wait_for_input", "C"],
        )

    def test_control_operands_are_preserved(self):
        decoded = decode_string(bytes([0xFA, 0xDA, 0x03, 1]))
        self.assertEqual(decoded["segments"][0], {
            "ctrl": "0xFA", "name": "event_flag_check",
            "operands": [0xDA, 0x03], "length": 3,
        })
        self.assertEqual(decoded["text"], "A")

    def test_extended_event_check_takes_three_operand_bytes(self):
        decoded = decode_string(bytes([0xFB, 0x01, 0x02, 0x03, 1]))
        self.assertEqual(decoded["segments"][0]["operands"], [1, 2, 3])
        self.assertEqual(decoded["text"], "A")

    def test_portrait_operand_count_changes_the_text(self):
        """$F4 reads a second operand only when Game_Mode_Routine is 4."""
        encoded = bytes([0xF4, 0x03, 0x02, 1])
        self.assertEqual(decode_string(encoded)["text"], "BA")
        talk = decode_string(encoded, portrait_operand_bytes=2)
        self.assertEqual(talk["text"], "A")
        self.assertEqual(talk["segments"][0]["portrait_id"], 3)
        self.assertEqual(talk["segments"][0]["portrait_location"], 2)

    def test_action_operand_length_follows_the_action(self):
        load_panel = decode_string(bytes([0xF2, 0x00, 0x12, 0x34, 1]))
        self.assertEqual(load_panel["segments"][0]["action"], "load_panel")
        self.assertEqual(load_panel["segments"][0]["operands"], [0x12, 0x34])
        self.assertEqual(load_panel["text"], "A")

        pause = decode_string(bytes([0xF2, 0x08, 1]))
        self.assertEqual(pause["segments"][0]["action"], "pause_music")
        self.assertEqual(pause["segments"][0]["operands"], [])
        self.assertEqual(pause["text"], "A")

    def test_unmapped_glyph_byte_raises(self):
        """0x4E is past the end of the dialogue font; 0x25 is a window hole."""
        with self.assertRaises(TextError):
            decode_string(bytes([0x4E]))
        with self.assertRaises(TextError):
            decode_name(bytes([0x25]), charset=WINDOW)

    def test_non_retail_action_raises(self):
        """Action $D exists only in the Grand Cross fork."""
        with self.assertRaises(TextError):
            decode_string(bytes([0xF2, 0x0D, 0x00, 0x00]))

    def test_truncated_operand_raises(self):
        with self.assertRaises(TextError):
            decode_string(bytes([0xFA, 0x01]))

    def test_split_terminated_rejects_a_dangling_tail(self):
        self.assertEqual(
            split_terminated(bytes([1, 0xFF, 2, 3, 0xFF]), 0xFF),
            [(0, b"\x01"), (2, b"\x02\x03")],
        )
        with self.assertRaises(TextError):
            split_terminated(bytes([1, 0xFF, 2]), 0xFF)

    def test_alignment_pad_is_trimmed_only_in_its_documented_shape(self):
        self.assertEqual(trim_tree_alignment(b"\x01\xFF", "t"), (b"\x01\xFF", 0))
        self.assertEqual(trim_tree_alignment(b"\x01\xFF\x00", "t"), (b"\x01\xFF", 1))
        with self.assertRaises(TextError):
            trim_tree_alignment(b"\x01\xFF\x00\x00", "t")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestNamesFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.names = extract_names(cls.data)

    def test_signatures_pass(self):
        self.assertTrue(all(v["ok"] for v in self.names["validation"]))

    def test_table_counts(self):
        counts = {name: len(self.names[name]) for name in
                  [spec["name"] for spec in NAME_TABLES]}
        self.assertEqual(counts, {
            "character_names": 11,
            "profession_names": 8,
            "enemy_names": 153,
            "enemy_skill_names": 112,
            "combo_names": 14,
            "item_names": 160,
            "technique_names": 40,
            "skill_names": 54,
            "place_names": 54,
            "item_names_dialogue": 160,
        })

    def test_counts_line_up_with_the_record_tables(self):
        """A name list that disagrees with its record table is a bad offset."""
        self.assertEqual(len(self.names["enemy_names"]), TABLES["enemies"]["count"])
        self.assertEqual(len(self.names["enemy_skill_names"]), TABLES["enemy_skills"]["count"])
        self.assertEqual(len(self.names["item_names"]), TABLES["items"]["count"])
        self.assertEqual(len(self.names["technique_names"]), TABLES["techniques"]["count"])
        self.assertEqual(len(self.names["skill_names"]), TABLES["skills"]["count"])
        self.assertEqual(len(self.names["character_names"]), TABLES["characters"]["count"])
        # ComboNames has no entry for combo 0 ("None"), so it is one short of
        # the 15-record combo table and starts at id 1.
        self.assertEqual(len(self.names["combo_names"]), TABLES["combos"]["count"] - 1)
        self.assertEqual(self.names["combo_names"][0]["id"], 1)

    def test_id_conventions_match_the_record_extractors(self):
        """Enemies and characters are 0-based; items and abilities are 1-based."""
        self.assertEqual(self.names["enemy_names"][0]["id"], 0)
        self.assertEqual(self.names["enemy_names"][-1]["id"], 152)
        self.assertEqual(self.names["character_names"][0]["id"], 0)
        self.assertEqual(self.names["item_names"][0]["id"], 1)
        self.assertEqual(self.names["item_names"][-1]["id"], 160)
        self.assertEqual(self.names["technique_names"][0]["id"], 1)
        self.assertEqual(self.names["enemy_skill_names"][0]["id"], 1)

    def test_pinned_names(self):
        def named(table, entry_id):
            return next(e["name"] for e in self.names[table] if e["id"] == entry_id)

        self.assertEqual(named("enemy_names", 0), "HELEX")
        self.assertEqual(named("enemy_names", 1), "MONSTERFLY")
        self.assertEqual(named("enemy_names", 152), "ZIO")
        self.assertEqual(named("enemy_skill_names", 1), "NOTHING")
        self.assertEqual(named("enemy_skill_names", 112), "BLACK WAVE")
        self.assertEqual(named("item_names", 1), "DAGGER")
        self.assertEqual(named("item_names", 0x80), "ANTIDOTE")
        self.assertEqual(named("item_names", 160), "MAHLAYRING")
        self.assertEqual(named("technique_names", 1), "FOI")
        self.assertEqual(named("technique_names", 40), "HINAS")
        self.assertEqual(named("skill_names", 1), "CROSSCUT")
        self.assertEqual(named("skill_names", 54), "RECOVER")
        self.assertEqual(named("combo_names", 1), "PARADINBLW")
        self.assertEqual(named("combo_names", 14), "DESTRUCT")
        self.assertEqual(named("place_names", 0), "PIATA")
        self.assertEqual(named("place_names", 53), "RAPPY CAVE")
        self.assertEqual(named("character_names", 0), "Chaz")
        self.assertEqual(named("profession_names", 0), "HUNTER")

    def test_provenance_on_every_entry(self):
        for spec in NAME_TABLES:
            table = self.names["tables"][spec["name"]]
            with self.subTest(spec["name"]):
                for entry in table["entries"]:
                    offset = int(entry["rom_offset"], 16)
                    raw = bytes.fromhex(entry["raw_hex"])
                    self.assertEqual(self.data[offset:offset + len(raw)], raw)
                    self.assertEqual(raw[-1], spec["terminator"])
                    self.assertEqual(len(raw), len(entry["name"]) + 1)

    def test_tables_are_contiguous_where_the_rom_says_they_are(self):
        """EnemySkillNames butts directly onto the enemy record table."""
        self.assertEqual(
            self.names["tables"]["enemy_skill_names"]["rom_end_exclusive"],
            f"0x{TABLES['enemies']['offset']:06X}",
        )
        self.assertEqual(
            self.names["tables"]["enemy_names"]["rom_end_exclusive"],
            self.names["tables"]["enemy_skill_names"]["rom_offset"],
        )

    def test_the_two_item_name_tables_agree(self):
        """`InventoryNames` (menu font) and `InventoryNames2` (dialogue font).

        Two tables, two different encodings, same 160 strings -- which is a
        strong check that both charsets are right.
        """
        window = [e["name"] for e in self.names["item_names"]]
        dialogue = [e["name"] for e in self.names["item_names_dialogue"]]
        self.assertEqual(window, dialogue)
        self.assertNotEqual(
            self.names["item_names"][0]["raw_hex"],
            self.names["item_names_dialogue"][0]["raw_hex"],
        )

    def test_character_and_technique_names_match_the_existing_lists(self):
        """core's hand-entered lists were right; now they are ROM-backed."""
        self.assertEqual([e["name"] for e in self.names["character_names"]], CHARACTER_NAMES)
        self.assertEqual(
            [e["name"] for e in self.names["profession_names"]],
            [p.upper() for p in PROFESSIONS],
        )
        self.assertEqual(
            [e["name"] for e in self.names["technique_names"]],
            [t.upper() for t in TECHNIQUE_NAMES],
        )

    def test_vehicle_names_come_from_the_inventory_table(self):
        """`ItemID_LandRover = $96`, `ItemID_IceDigger = $97`."""
        self.assertEqual(
            [(v["id"], v["item_id"], v["name"]) for v in self.names["vehicle_names"]],
            [(1, 0x96, "LAND-ROVER"), (2, 0x97, "ICE-DIGGER"), (3, 0x98, "HYDROFOIL")],
        )

    def test_wrong_offset_is_rejected_rather_than_decoded(self):
        shifted = bytearray(self.data)
        shifted[0x280D42] = 0x00
        with self.assertRaises(TextError):
            extract_names(bytes(shifted))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestDialogueFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.dialogue = extract_dialogue(cls.data)
        cls.trees = {t["tree"]: t for t in cls.dialogue["trees"]}

    def test_region_shape(self):
        region = self.dialogue["region"]
        self.assertEqual(region["start"], f"0x{DIALOGUE_REGION_START:06X}")
        self.assertEqual(region["tree_count"], DIALOGUE_TREE_COUNT)
        self.assertEqual(region["end_exclusive"], "0x1FE655")
        self.assertEqual(self.dialogue["total_entries"], 2736)

    def test_blobs_are_back_to_back_and_zero_padded(self):
        """Each tree ends where the decompressor stops; the next begins at the
        following 16-byte boundary, and the gap is zeros."""
        offset = DIALOGUE_REGION_START
        for tree in range(1, DIALOGUE_TREE_COUNT + 1):
            record = self.trees[tree]
            with self.subTest(tree=tree):
                self.assertEqual(record["rom_offset"], f"0x{offset:06X}")
                end = int(record["rom_end_exclusive"], 16)
                self.assertEqual(end - offset, record["compressed_size"])
                self.assertEqual(record["padding_bytes"], (-end) % 16)
                self.assertEqual(
                    self.data[end:end + record["padding_bytes"]],
                    bytes(record["padding_bytes"]),
                )
                offset = end + record["padding_bytes"]

    def test_uncompressed_text_follows_the_region(self):
        """`WinTiles_PlayerNothingMsg` sits immediately past the last tree,
        stored as plain dialogue-charset bytes rather than compressed."""
        end = int(self.dialogue["region"]["end_exclusive"], 16)
        start = end + (-end) % 16
        self.assertEqual(start, 0x1FE660)
        message = decode_string(self.data[start + 2:start + 0x22])
        self.assertTrue(message["text"].startswith("There's nothing interesting"))

    def test_pinned_first_entry(self):
        first = self.trees[1]["entries"][0]
        self.assertEqual(first["id"], 0)
        self.assertEqual(first["block_offset"], "0x0000")
        self.assertEqual(
            first["text"],
            "Are you a hunter?\nAre you here to exterminate\nthe monsters?",
        )
        self.assertEqual(
            [s["ctrl"] for s in first["segments"] if "ctrl" in s],
            ["0xFA", "0xFA", "0xFA", "0xFD", "0xFC"],
        )

    def test_only_the_talk_trees_use_the_two_byte_portrait_form(self):
        """The Talk command is the only caller running with
        Game_Mode_Routine = 4, and `Win_TalkDialogue` picks tree 31 for talk
        ids below $1A and tree 32 above. The data confirms it: in those two
        trees every $F4 is followed by a location byte, and in the other 41 the
        1-byte reading is the only one that leaves no unmapped bytes."""
        talk = [t["tree"] for t in self.dialogue["trees"] if t["portrait_operand_bytes"] == 2]
        self.assertEqual(talk, list(TALK_DIALOGUE_TREES))
        self.assertEqual(self.trees[31]["entry_count"], 26)  # talk ids 0..$19
        for tree in TALK_DIALOGUE_TREES:
            for entry in self.trees[tree]["entries"]:
                for segment in entry["segments"]:
                    if segment.get("ctrl") == "0xF4":
                        self.assertIn(segment["portrait_location"], (0, 1, 2))

    def test_every_byte_is_accounted_for(self):
        """Segments must reproduce their entry's raw bytes exactly.

        This is the check that the operand lengths are right: a wrong length
        desynchronises the parse and either raises on an unmapped byte or
        changes the reconstructed length.
        """
        for tree in self.dialogue["trees"]:
            with self.subTest(tree=tree["tree"]):
                for entry in tree["entries"]:
                    raw = bytes.fromhex(entry["raw_hex"])
                    rebuilt = sum(
                        len(segment["text"]) if "text" in segment else segment["length"]
                        for segment in entry["segments"]
                    )
                    self.assertEqual(rebuilt, len(raw))

    def test_control_codes_seen_in_retail_data(self):
        """$F0, $F1, $F8, $FB and $FE are defined but never used by the US
        script; $FE only terminates the menu-font name tables."""
        seen = {
            segment["ctrl"]
            for tree in self.dialogue["trees"]
            for entry in tree["entries"]
            for segment in entry["segments"]
            if "ctrl" in segment
        }
        self.assertEqual(sorted(seen), [
            "0xF2", "0xF3", "0xF4", "0xF5", "0xF6", "0xF7", "0xF9", "0xFA", "0xFC", "0xFD",
        ])

    def test_text_action_ids_seen_in_retail_data(self):
        actions = {
            segment["action_id"]
            for tree in self.dialogue["trees"]
            for entry in tree["entries"]
            for segment in entry["segments"]
            if segment.get("ctrl") == "0xF2"
        }
        self.assertEqual(sorted(actions), [0, 1, 2, 3, 4, 6, 7, 8, 9, 0xA, 0xB, 0xC])

    def test_provenance_is_carried(self):
        """A decompressed entry has no ROM offset of its own, so the blob's
        range and hash live on the tree, exactly as formations.py does it."""
        tree = self.trees[1]
        self.assertEqual(tree["label"], "DialogueTree1")
        self.assertEqual(tree["rom_offset"], "0x1DF600")
        self.assertEqual(tree["compression"], "kosinski")
        self.assertEqual(len(tree["compressed_sha256"]), 64)
        self.assertEqual(tree["decompressed_size"], 5854)
        self.assertEqual(tree["alignment_pad_bytes"], 1)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(SCRIPT_DIR.is_dir(), f"disassembly oracle not present at {SCRIPT_DIR}")
class TestDialogueOracle(unittest.TestCase):
    """Round-trip the decompressed trees against the disassembly's sources."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        charset = {char: code for code, char in read_charset(CHARSET_SOURCES[DIALOGUE]).items()}
        cls.sources = {
            tree: assemble_dialogue_source(
                SCRIPT_DIR / f"dialogue {tree}.asm", charset, ORACLE_SYMBOLS
            )
            for tree in range(1, DIALOGUE_TREE_COUNT + 1)
        }
        cls.blobs = {}
        offset = DIALOGUE_REGION_START
        for tree in range(1, DIALOGUE_TREE_COUNT + 1):
            from psiv_tools.kosinski import decompress

            block, consumed = decompress(cls.data, offset)
            cls.blobs[tree] = block
            offset = offset + consumed
            offset += (-offset) % 16

    def test_thirty_eight_trees_match_byte_for_byte(self):
        matching = [t for t in self.sources if self.blobs[t] == self.sources[t]]
        self.assertEqual(len(matching), DIALOGUE_TREE_COUNT - len(ORACLE_MISMATCHES))
        for tree in matching:
            with self.subTest(tree=tree):
                self.assertEqual(self.blobs[tree], self.sources[tree])

    def test_the_known_mismatches_are_exactly_the_documented_ones(self):
        mismatched = {t for t in self.sources if self.blobs[t] != self.sources[t]}
        self.assertEqual(mismatched, ORACLE_MISMATCHES)

    def test_source_defects_are_single_byte_slips_except_tree_17(self):
        """Four of the five differ from retail by two single-byte edits.

        Tree 17's source was rewritten wholesale by the fork, so it gets no
        byte-level claim beyond "not the retail script".
        """
        import difflib

        for tree in sorted(ORACLE_MISMATCHES - {17}):
            with self.subTest(tree=tree):
                rom_block, source = self.blobs[tree], self.sources[tree]
                self.assertEqual(len(rom_block), len(source))
                matcher = difflib.SequenceMatcher(None, rom_block, source, autojunk=False)
                edits = [op for op in matcher.get_opcodes() if op[0] != "equal"]
                self.assertEqual(len(edits), 2)
                self.assertTrue(all(i2 - i1 <= 1 and j2 - j1 <= 1 for _, i1, i2, j1, j2 in edits))

        # The one control byte among those slips: the source drops a $FB.
        self.assertEqual(self.blobs[34][0x119E], 0xFB)
        self.assertNotEqual(self.sources[34][0x119E], 0xFB)

    def test_tree_17_is_a_rewrite_not_a_slip(self):
        self.assertNotEqual(len(self.blobs[17]), len(self.sources[17]))


if __name__ == "__main__":
    unittest.main()
