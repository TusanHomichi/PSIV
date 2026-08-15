import hashlib
import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.core import read_rom
from psiv_tools.dialogue_pack import (
    BACKGROUND_COLOR_INDEX,
    CHARS_PER_LINE,
    CTRL_NAMES,
    DIALOGUE_FORMAT_VERSION,
    DIALOGUE_WINDOW_INDEX,
    FONT_JSON_NAME,
    FONT_PNG_NAME,
    FONT_ROM_OFFSET,
    FONT_SIZE,
    GLYPH_BYTES,
    GLYPH_COUNT,
    GLYPH_HEIGHT,
    GLYPH_WIDTH,
    LINES_PER_WINDOW,
    MENU_FONT_TILES,
    MENU_FONT_VRAM_TILE,
    PAGE_ENDINGS,
    PORTRAITS_DIRECTORY,
    PORTRAITS_NAME,
    SIGNATURES,
    SYSTEM_MESSAGE_COUNT,
    SYSTEM_MESSAGES_LABEL,
    TEXT_COLOR_INDEX,
    TILE_PIXELS,
    TREES_NAME,
    WINDOW_ART_TILES,
    WINDOW_ART_VRAM_TILE,
    WINDOW_JSON_NAME,
    WINDOW_PNG_NAME,
    WINDOW_TILE_HEIGHT,
    WINDOW_TILE_WIDTH,
    WINDOW_TILE_X,
    WINDOW_TILE_Y,
    Census,
    DialoguePackError,
    check_signatures,
    check_window_fits_the_text,
    control_segment,
    dialogue_palette,
    emit_dialogue,
    font_strip,
    frame_roles,
    glyph_bitmaps,
    paginate,
    pattern_word,
    split_preamble,
    window_records,
)
from psiv_tools.gfx import PORTRAIT_TILES
from psiv_tools.pack import PACK_FORMAT_VERSION
from psiv_tools.text import DIALOGUE_CHARSET, extract_dialogue

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

# `DialogueTree1` entry 4, the Piata academy student. Two full lines, then a
# window the retail loop pages to on its own because the second line lands
# exactly on the 32-character wrap.
PINNED_TREE = 1
PINNED_ENTRY = 4

# `ArtNem_ChazDialPortrait`, decompressed and composed six tiles across.
CHAZ_PORTRAIT_SHA = "a0f4230b0e22abaee6b16a61f21656d196bdaef5cea94c2c37797fa998b77495"


def parse_png_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Re-parse a PNG, verifying every chunk CRC."""
    if data[:8] != png.PNG_SIGNATURE:
        raise AssertionError("missing PNG signature")
    chunks = []
    pos = 8
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload = data[pos + 8:pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length:pos + 12 + length])
        expected = zlib.crc32(kind + payload) & 0xFFFFFFFF
        if crc != expected:
            raise AssertionError(f"bad CRC on {kind!r}: {crc:08X} != {expected:08X}")
        chunks.append((kind, payload))
        pos += 12 + length
    return chunks


def png_pixels(data: bytes) -> tuple[int, int, list[bytes], dict[bytes, bytes]]:
    """Width, height, unfiltered rows and the chunk payloads of an indexed PNG."""
    chunks = dict(parse_png_chunks(data))
    width, height, depth, colour = struct.unpack(">IIBB", chunks[b"IHDR"][:10])
    if (depth, colour) != (8, png.COLOR_TYPE_INDEXED):
        raise AssertionError(f"expected 8-bit indexed, got depth {depth} type {colour}")
    raw = zlib.decompress(b"".join(
        payload for kind, payload in parse_png_chunks(data) if kind == b"IDAT"
    ))
    rows = []
    for y in range(height):
        start = y * (width + 1)
        if raw[start]:
            raise AssertionError(f"row {y} uses filter {raw[start]}, not 0")
        rows.append(raw[start + 1:start + 1 + width])
    return width, height, rows, chunks


def text(run: str) -> dict:
    return {"text": run}


def ctrl(code: int, *operands: int) -> dict:
    return {"ctrl": f"0x{code:02X}", "name": CTRL_NAMES[code], "operands": list(operands)}


def lines_of(pages) -> list[list[str]]:
    return [page["lines"] for page in pages]


def endings_of(pages) -> list[str]:
    return [page["end"] for page in pages]


# ---------------------------------------------------------------------------
# The window model. None of this needs a ROM.
# ---------------------------------------------------------------------------
class TestPaginate(unittest.TestCase):
    """`RunText_CharacterLoop`, rule by rule."""

    def test_a_short_message_is_one_window(self):
        pages = paginate([text("Hello"), ctrl(0xFC), text("there")])
        self.assertEqual(lines_of(pages), [["Hello", "there"]])
        self.assertEqual(endings_of(pages), ["end"])

    def test_the_column_counter_wraps_at_32(self):
        pages = paginate([text("A" * 40)])
        self.assertEqual(lines_of(pages), [["A" * 32, "A" * 8]])

    def test_two_full_lines_page_on_their_own(self):
        """`andi.w #1,d2` reaching zero jumps into TextCtrlCode_Interrupt."""
        pages = paginate([text("A" * 64 + "B")])
        self.assertEqual(lines_of(pages), [["A" * 32, "A" * 32], ["B"]])
        self.assertEqual(endings_of(pages), ["full", "end"])

    def test_a_newline_landing_on_a_full_line_is_swallowed(self):
        """`cmpi.b #$FC,(a0)` right after the wrap, so this is one line break."""
        pages = paginate([text("A" * 32), ctrl(0xFC), text("B")])
        self.assertEqual(lines_of(pages), [["A" * 32, "B"]])

    def test_a_wait_landing_on_a_full_window_is_swallowed(self):
        """TextCtrlCode_Interrupt skips a $FD it stopped on, so the window
        waits once rather than waiting again on an empty box."""
        pages = paginate([text("A" * 64), ctrl(0xFD), text("B")])
        self.assertEqual(lines_of(pages), [["A" * 32, "A" * 32], ["B"]])
        self.assertEqual(endings_of(pages), ["full", "end"])

    def test_a_wait_before_the_terminator_does_not_wait(self):
        """The byte after the last segment is the entry's own $FF, which
        TextCtrlCode_Interrupt swallows on its way to TextCtrlCode_Terminate."""
        pages = paginate([text("Bye"), ctrl(0xFD)])
        self.assertEqual(lines_of(pages), [["Bye"]])
        self.assertEqual(endings_of(pages), ["end"])

    def test_two_waits_in_a_row_are_one_wait(self):
        pages = paginate([text("A"), ctrl(0xFD), ctrl(0xFD), text("B")])
        self.assertEqual(lines_of(pages), [["A"], ["B"]])
        self.assertEqual(endings_of(pages), ["wait", "end"])

    def test_close_yields_and_the_message_resumes(self):
        """$F7 saves the text pointer and returns; the caller runs the rest of
        the entry in a fresh window."""
        pages = paginate([text("A"), ctrl(0xF7), text("B"), ctrl(0xFD)])
        self.assertEqual(lines_of(pages), [["A"], ["B"]])
        self.assertEqual(endings_of(pages), ["close", "end"])

    def test_a_wait_followed_by_close_is_a_close(self):
        pages = paginate([text("A"), ctrl(0xFD), ctrl(0xF7), text("B")])
        self.assertEqual(endings_of(pages), ["close", "end"])

    def test_a_choice_ends_the_entry(self):
        """TextCtrlCode_YesNo jumps to another entry by id, so nothing after
        the two operand bytes is reachable."""
        pages = paginate([text("Well?"), ctrl(0xF5, 3, 4), text("unreachable")])
        self.assertEqual(lines_of(pages), [["Well?"]])
        self.assertEqual(endings_of(pages), ["choice"])

    def test_a_choice_on_a_full_line_is_reached_without_a_line_break(self):
        pages = paginate([text("A" * 32), ctrl(0xF5, 1, 2)])
        self.assertEqual(lines_of(pages), [["A" * 32]])
        self.assertEqual(endings_of(pages), ["choice"])

    def test_newline_is_not_masked(self):
        """$FC increments the line counter without the two-line mask, so a
        third line is a real -- if never used -- possibility."""
        pages = paginate([text("A"), ctrl(0xFC), text("B"), ctrl(0xFC), text("C")])
        self.assertEqual(lines_of(pages), [["A", "B", "C"]])

    def test_the_preamble_is_not_displayed(self):
        pages = paginate([ctrl(0xFA, 9, 2), ctrl(0xF3), text("Hi")])
        self.assertEqual(lines_of(pages), [["Hi"]])

    def test_an_event_entry_shows_nothing(self):
        self.assertEqual(paginate([ctrl(0xFA, 9, 2), ctrl(0xF6, 0, 12)]), [])

    def test_a_null_code_in_the_text_ends_the_message(self):
        """TextCtrlCode_Null is an `rts`: `$F0`, `$F1`, `$F3`, `$F8` and `$FB`
        all return from RunText if the character loop meets one."""
        pages = paginate([text("A"), ctrl(0xF8), text("B")])
        self.assertEqual(lines_of(pages), [["A"]])
        self.assertEqual(endings_of(pages), ["end"])

    def test_control_codes_do_not_take_up_columns(self):
        pages = paginate([text("A" * 31), ctrl(0xF4, 3), text("BC")])
        self.assertEqual(lines_of(pages), [["A" * 31 + "B", "C"]])


class TestSplitPreamble(unittest.TestCase):
    def test_flag_checks_then_text(self):
        self.assertEqual(
            split_preamble([ctrl(0xFA, 1, 2), ctrl(0xFA, 3, 4), text("Hi")]), (2, False)
        )

    def test_flag_checks_then_event(self):
        self.assertEqual(split_preamble([ctrl(0xFA, 1, 2), ctrl(0xF6, 0, 5)]), (2, True))

    def test_keep_facing_is_eaten_too(self):
        self.assertEqual(split_preamble([ctrl(0xF3), text("Hi")]), (1, False))

    def test_plain_text_has_no_preamble(self):
        self.assertEqual(split_preamble([text("Hi")]), (0, False))


class TestPatternWord(unittest.TestCase):
    """The nine words `loc_68704` writes are three tiles and their flips."""

    def test_a_word_splits_into_priority_palette_flips_and_tile(self):
        self.assertEqual(pattern_word(0xC6E9), {
            "word": "0xC6E9", "priority": True, "cram_line": 2,
            "flip_v": False, "flip_h": False, "vram_tile": 0x6E9,
        })

    def test_the_flipped_corners_are_the_same_tile(self):
        corners = [pattern_word(w) for w in (0xC6E9, 0xCEE9, 0xD6E9, 0xDEE9)]
        self.assertEqual({corner["vram_tile"] for corner in corners}, {0x6E9})
        self.assertEqual(
            [(c["flip_h"], c["flip_v"]) for c in corners],
            [(False, False), (True, False), (False, True), (True, True)],
        )

    def test_the_frame_is_three_tiles_and_a_fill(self):
        roles = frame_roles()
        self.assertEqual(len(roles), 9)
        self.assertEqual({role["tile"] for role in roles.values()}, {0, 105, 106, 115})
        self.assertEqual(roles["fill"]["tile"], 0)
        self.assertEqual(roles["edge_right"]["tile"], roles["edge_left"]["tile"])
        self.assertTrue(roles["edge_right"]["flip_h"])
        for name, role in roles.items():
            self.assertEqual(role["cram_line"], 2, name)
            self.assertTrue(role["priority"], name)
            self.assertEqual(role["x"], role["tile"] * TILE_PIXELS, name)
            self.assertLess(role["tile"], WINDOW_ART_TILES, name)

    def test_the_frame_tiles_dodge_the_font_loaded_over_the_blob(self):
        """`Title_ArtPtrs` loads `ArtNem_Font` at $681, so blob indices
        1..87 are overwritten for the whole game. Every frame role is outside
        that span, which is why the window still has a border."""
        first = MENU_FONT_VRAM_TILE - WINDOW_ART_VRAM_TILE
        overwritten = range(first, first + MENU_FONT_TILES)
        for name, role in frame_roles().items():
            self.assertNotIn(role["tile"], overwritten, name)


class TestControlSegment(unittest.TestCase):
    def test_portrait_carries_its_id_and_slot(self):
        segment = dict(ctrl(0xF4, 6, 2), portrait_id=6, portrait_location=2)
        self.assertEqual(control_segment(segment), {
            "ctrl": "portrait", "code": "0xF4", "operands": [6, 2],
            "id": 6, "position": 2,
        })

    def test_a_portrait_outside_the_talk_trees_has_no_slot(self):
        segment = dict(ctrl(0xF4, 6), portrait_id=6)
        self.assertIsNone(control_segment(segment)["position"])

    def test_yes_no_names_both_targets(self):
        out = control_segment(ctrl(0xF5, 7, 8))
        self.assertEqual((out["ctrl"], out["yes_entry"], out["no_entry"]), ("yes_no", 7, 8))

    def test_event_operands_are_a_word(self):
        self.assertEqual(control_segment(ctrl(0xF6, 1, 0x23))["id"], 0x123)

    def test_flag_check_names_its_branch(self):
        out = control_segment(ctrl(0xFA, 0xDA, 3))
        self.assertEqual(
            (out["ctrl"], out["scope"], out["flag"], out["then_entry"]),
            ("flag_check", "event_flag", 0xDA, 3),
        )

    def test_a_zero_delay_is_256_frames(self):
        """`subq.b #1,d7` then `dbf`, so a zero operand wraps rather than
        waiting for nothing."""
        self.assertEqual(control_segment(ctrl(0xF9, 0))["frames"], 0x100)
        self.assertEqual(control_segment(ctrl(0xF9, 59))["frames"], 59)

    def test_actions_decode_their_own_operands(self):
        panel = dict(ctrl(0xF2, 0, 35), action_id=0, action="load_panel")
        self.assertEqual(control_segment(panel)["panel"], 35)
        sound = dict(ctrl(0xF2, 0xFE), action_id=3, action="load_sound")
        self.assertEqual(control_segment(sound)["sound"], 0xFE)

    def test_the_extended_flag_check_is_not_a_retail_code(self):
        """`TextCtrlCodesJmpTbl` sends $FB to TextCtrlCode_Null and the retail
        interaction preamble has no $FB branch, so `psiv_tools.text`'s
        three-operand reading would be wrong for this cartridge. Nothing in the
        retail script uses it; if that ever changes, this fails loudly."""
        with self.assertRaises(DialoguePackError):
            control_segment(ctrl(0xFB, 1, 2, 3))

    def test_every_control_code_has_a_name(self):
        self.assertEqual(sorted(CTRL_NAMES), list(range(0xF0, 0x100)))


# ---------------------------------------------------------------------------
# The emitted pack.
# ---------------------------------------------------------------------------
@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestDialoguePack(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._tmp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._tmp.name)
        cls.fragment = emit_dialogue(cls.data, cls.root)
        cls.trees = json.loads((cls.root / TREES_NAME).read_text())
        cls.font = json.loads((cls.root / FONT_JSON_NAME).read_text())
        cls.window = json.loads((cls.root / WINDOW_JSON_NAME).read_text())
        cls.portraits = json.loads((cls.root / PORTRAITS_NAME).read_text())

    @classmethod
    def tearDownClass(cls):
        cls._tmp.cleanup()

    # -- provenance ---------------------------------------------------------
    def test_the_format_version_tracks_the_pack(self):
        self.assertEqual(DIALOGUE_FORMAT_VERSION, PACK_FORMAT_VERSION)
        for payload in (self.trees, self.font, self.window, self.portraits):
            self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)

    def test_every_transcribed_routine_is_checked_against_the_image(self):
        checked = check_signatures(self.data)
        self.assertEqual(len(checked), len(SIGNATURES))
        self.assertEqual(
            {entry["name"] for entry in checked}, set(SIGNATURES),
        )
        self.assertEqual(self.fragment["signatures"], checked)

    def test_a_changed_routine_is_refused(self):
        broken = bytearray(self.data)
        offset, _, _ = SIGNATURES["chars_per_line"]
        broken[offset + 3] ^= 0xFF  # the andi.w immediate
        with self.assertRaises(DialoguePackError):
            check_signatures(bytes(broken))

    def test_the_window_palette_is_pal_init_line_3(self):
        palette = dialogue_palette(self.data)
        self.assertEqual(len(palette), 16)
        # Index $E is the window's blue and index $F the text's white; every
        # glyph is drawn out of exactly these two.
        self.assertEqual(palette[TEXT_COLOR_INDEX], (255, 255, 255))
        self.assertEqual(palette[BACKGROUND_COLOR_INDEX], (0, 0x24, 0x6D))
        self.assertEqual(
            self.font["palette"]["colors"][BACKGROUND_COLOR_INDEX], [0, 0x24, 0x6D]
        )
        self.assertEqual(self.font["palette"]["cram_line"], 2)
        self.assertEqual(self.portraits["palette"]["cram_line"], 2)

    # -- trees --------------------------------------------------------------
    def test_the_trees_match_the_decoder(self):
        decoded = extract_dialogue(self.data)
        self.assertEqual(self.trees["tree_count"], 43)
        self.assertEqual(len(self.trees["trees"]), 43)
        self.assertEqual(self.trees["entry_count"], decoded["total_entries"])
        for emitted, source in zip(self.trees["trees"], decoded["trees"]):
            self.assertEqual(emitted["tree"], source["tree"])
            self.assertEqual(emitted["entry_count"], source["entry_count"])
            self.assertEqual(len(emitted["entries"]), source["entry_count"])
            self.assertEqual(
                [entry["id"] for entry in emitted["entries"]],
                list(range(source["entry_count"])),
            )
            self.assertEqual(
                [entry["raw_hex"] for entry in emitted["entries"]],
                [entry["raw_hex"] for entry in source["entries"]],
            )

    def test_only_the_talk_trees_carry_a_portrait_slot(self):
        for tree in self.trees["trees"]:
            talk = tree["tree"] in (31, 32)
            self.assertEqual(tree["is_talk_tree"], talk)
            self.assertEqual(tree["portrait_operand_bytes"], 2 if talk else 1)
            for entry in tree["entries"]:
                for segment in entry["segments"]:
                    if segment.get("ctrl") == "portrait":
                        self.assertEqual(segment["position"] is not None, talk)

    def test_a_pinned_entry(self):
        entry = self.entry(PINNED_TREE, PINNED_ENTRY)
        self.assertEqual(entry["text"], (
            "About a month ago, monsters\n"
            "began to appear in the basement.\n"
            "I'm so frightened, I can't\n"
            "even think about my research!"
        ))
        self.assertEqual([segment.get("ctrl") for segment in entry["segments"]], [
            "flag_check", "flag_check", "flag_check", None, "newline", None,
            "wait", None, "newline", None,
        ])
        self.assertEqual(entry["segments"][0], {
            "ctrl": "flag_check", "code": "0xFA", "operands": [0xDA, 3],
            "scope": "event_flag", "flag": 0xDA, "then_entry": 3,
        })
        # The second line lands exactly on the 32-character wrap, so the
        # window pages itself and the $FD that follows is swallowed.
        self.assertEqual(entry["pages"], [
            {"lines": ["About a month ago, monsters",
                       "began to appear in the basement."], "end": "full"},
            {"lines": ["I'm so frightened, I can't",
                       "even think about my research!"], "end": "end"},
        ])

    def test_a_pinned_portrait_conversation(self):
        """`DialogueTree5` entry 14, Hahn introducing Saya: `$F7` closes the
        window between scenes and the portrait changes with the speaker."""
        entry = self.entry(5, 14)
        portraits = [
            segment["id"] for segment in entry["segments"]
            if segment.get("ctrl") == "portrait"
        ]
        self.assertEqual(portraits[:6], [3, 12, 3, 3, 2, 12])
        self.assertEqual(entry["pages"][0], {"lines": ["Saya!"], "end": "close"})
        self.assertEqual(entry["pages"][1], {
            "lines": ["Hahn! You've come home!", "I'm so happy!"], "end": "wait",
        })

    def test_no_line_is_wider_than_the_window(self):
        for tree in self.trees["trees"]:
            for entry in tree["entries"]:
                for page in entry["pages"]:
                    self.assertLessEqual(len(page["lines"]), LINES_PER_WINDOW, entry)
                    for line in page["lines"]:
                        self.assertLessEqual(len(line), CHARS_PER_LINE, entry)

    def test_the_window_metrics_are_the_proven_ones(self):
        window = self.trees["window"]
        self.assertEqual(window["chars_per_line"], 32)
        self.assertEqual(window["lines_per_window"], 2)
        self.assertEqual([window["glyph_width"], window["glyph_height"]], [8, 16])
        self.assertEqual(window["rect"], {"x": 32, "y": 168, "width": 256, "height": 32})
        self.assertEqual(window["vram_tile"], "0x580")
        self.assertEqual(window["cram_line"], 2)
        self.assertEqual(self.fragment["window"], window)

    # -- font ---------------------------------------------------------------
    def test_the_font_is_eighty_8x16_glyphs(self):
        self.assertEqual(GLYPH_COUNT, FONT_SIZE // GLYPH_BYTES)
        self.assertEqual(len(self.font["glyphs"]), GLYPH_COUNT)
        image = (self.root / FONT_PNG_NAME).read_bytes()
        width, height, rows, chunks = png_pixels(image)
        self.assertEqual((width, height), (GLYPH_COUNT * GLYPH_WIDTH, GLYPH_HEIGHT))
        self.assertEqual(
            hashlib.sha256(image).hexdigest(), self.font["png_sha256"]
        )
        # Two colours and nothing else: `ParseText` only ever writes $E or $F.
        self.assertEqual({value for row in rows for value in row}, {0xE, 0xF})
        # tRNS covers the background index so glyphs composite over a fill.
        self.assertEqual(chunks[b"tRNS"][BACKGROUND_COLOR_INDEX], 0)

    def test_the_glyph_map_indexes_by_the_text_byte(self):
        for glyph in self.font["glyphs"]:
            self.assertEqual(glyph["x"], glyph["byte"] * GLYPH_WIDTH)
            self.assertEqual(glyph["char"], DIALOGUE_CHARSET.get(glyph["byte"]))
        self.assertEqual(self.font["by_char"]["A"], 1)
        self.assertEqual(self.font["by_char"]["a"], 27)
        self.assertEqual(self.font["by_char"]["0"], 64)
        self.assertEqual(self.font["by_char"][" "], 0)
        self.assertEqual(self.font["by_char"]["."], 0x35)
        self.assertEqual(self.font["unmapped_glyphs"], [0x4E, 0x4F])

    def test_the_rendered_a_is_a_capital_a(self):
        """The cheapest proof that the 1bpp 8x16 reading is the right one: the
        glyph the charset calls 'A' draws a capital A, and it draws it in the
        two colours `ParseText` uses rather than in raw indices."""
        _, _, rows, _ = png_pixels((self.root / FONT_PNG_NAME).read_bytes())
        rect = next(g for g in self.font["glyphs"] if g["char"] == "A")
        drawn = [
            "".join(
                "#" if row[rect["x"] + x] == TEXT_COLOR_INDEX else "."
                for x in range(GLYPH_WIDTH)
            )
            for row in rows
        ]
        self.assertEqual(drawn, [
            "........",
            "........",
            "........",
            "...#....",
            "..###...",
            ".##.##..",
            "##...##.",
            "##...##.",
            "##...##.",
            "#######.",
            "##...##.",
            "##...##.",
            "##...##.",
            "........",
            "........",
            "........",
        ])

    def test_the_strip_is_the_font_bytes_expanded(self):
        """Every pixel of the strip is one bit of `Art_DialogueFont`."""
        raw = self.data[FONT_ROM_OFFSET:FONT_ROM_OFFSET + FONT_SIZE]
        glyphs = glyph_bitmaps(self.data)
        width, height, pixels = font_strip(glyphs)
        _, _, rows, _ = png_pixels((self.root / FONT_PNG_NAME).read_bytes())
        self.assertEqual(b"".join(rows), pixels)
        for index in (1, 27, 0x35, 0x40):
            for y in range(GLYPH_HEIGHT):
                byte = raw[index * GLYPH_BYTES + y]
                self.assertEqual(
                    rows[y][index * GLYPH_WIDTH:(index + 1) * GLYPH_WIDTH],
                    bytes(
                        TEXT_COLOR_INDEX if byte & (1 << (7 - x)) else BACKGROUND_COLOR_INDEX
                        for x in range(GLYPH_WIDTH)
                    ),
                )

    def test_four_charset_bytes_have_no_glyph_at_all(self):
        """`[`, `(`, `)` and `=` are in `script/charset.asm` and blank in the
        font. The script never uses any of them."""
        blank = [g["byte"] for g in self.font["glyphs"] if g["blank"]]
        self.assertEqual(blank, [0x00, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F])
        census = self.fragment["census"]["blank_glyphs_in_charset"]
        self.assertEqual([entry["char"] for entry in census], ["[", "(", ")", "="])
        self.assertTrue(all(entry["used"] is False for entry in census))

    # -- system messages ----------------------------------------------------
    def test_the_nothing_here_lines_are_emitted(self):
        """`WinTiles_PlayerNothingMsg`: eleven uncompressed messages, one per
        party leader, addressed by `Current_Party_Slots` through the same
        `GetOffsetByID` a dialogue tree uses."""
        block = self.trees["system_messages"]
        self.assertEqual(block["label"], SYSTEM_MESSAGES_LABEL)
        self.assertEqual(block["rom_offset"], "0x1FE660")
        self.assertEqual(block["rom_end_exclusive"], "0x1FE7E5")
        self.assertIsNone(block["compression"])
        self.assertEqual(block["size_bytes"], 389)
        self.assertEqual(
            block["sha256"],
            hashlib.sha256(self.data[0x1FE660:0x1FE7E5]).hexdigest(),
        )
        self.assertEqual(block["count"], SYSTEM_MESSAGE_COUNT)
        self.assertEqual(len(block["messages"]), SYSTEM_MESSAGE_COUNT)
        self.assertEqual(block["selector"]["ram"], "Current_Party_Slots ($FFFFF40A)")
        self.assertEqual(
            self.fragment["trees"]["system_message_count"], SYSTEM_MESSAGE_COUNT
        )

    def test_each_leader_says_their_own_line_with_their_own_portrait(self):
        messages = self.trees["system_messages"]["messages"]
        self.assertEqual([m["id"] for m in messages], list(range(11)))
        self.assertEqual([m["portrait"] for m in messages], list(range(1, 12)))
        self.assertEqual([m["portrait_symbol"] for m in messages], [
            "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren",
            "Raja", "Kyra", "Seth",
        ])
        for message in messages:
            first = message["segments"][0]
            self.assertEqual(first["ctrl"], "portrait")
            self.assertEqual(first["id"], message["portrait"])

    def test_pinned_system_messages(self):
        messages = self.trees["system_messages"]["messages"]
        self.assertEqual(
            messages[0]["text"], "There's nothing interesting\nhere."
        )
        self.assertEqual(
            messages[0]["raw_hex"],
            self.data[0x1FE660:0x1FE683].hex(),
        )
        self.assertEqual(messages[7]["text"], "No irregularities ahead.")
        self.assertEqual(
            messages[10]["text"], "There isn't\nanything strange here."
        )

    def test_system_messages_paginate_like_entries(self):
        for message in self.trees["system_messages"]["messages"]:
            self.assertEqual(len(message["pages"]), 1, message["id"])
            for page in message["pages"]:
                self.assertIn(page["end"], PAGE_ENDINGS)
                self.assertLessEqual(len(page["lines"]), LINES_PER_WINDOW)
                for line in page["lines"]:
                    self.assertLessEqual(len(line), CHARS_PER_LINE)
            self.assertEqual(
                "\n".join(message["pages"][0]["lines"]), message["text"]
            )

    def test_the_records_after_the_block_are_left_out(self):
        """An `even` pad byte separates the eleven from the examine messages
        that follow, and those carry their own labels and pointers, so they are
        not in this id space and the pack does not pretend otherwise."""
        block = self.trees["system_messages"]
        end = int(block["rom_end_exclusive"], 16)
        self.assertEqual(self.data[end - 1], 0xFF)     # the eleventh terminator
        self.assertEqual(self.data[end], 0x00)         # the `even` pad
        self.assertEqual(block["alignment_pad_bytes"], 1)
        self.assertEqual(self.data[end + 1:end + 3].hex(), "f401")  # loc_1FE7E6

    # -- window chrome ------------------------------------------------------
    def test_the_chrome_strip_is_the_whole_window_blob(self):
        image = (self.root / WINDOW_PNG_NAME).read_bytes()
        width, height, rows, chunks = png_pixels(image)
        self.assertEqual((width, height), (WINDOW_ART_TILES * TILE_PIXELS, TILE_PIXELS))
        self.assertEqual(hashlib.sha256(image).hexdigest(), self.window["png_sha256"])
        self.assertEqual(
            self.window["png_sha256"], self.fragment["chrome"]["png_sha256"]
        )
        # Not one pixel of the window art is index 0, so there is nothing to
        # mark transparent and the strip carries no tRNS.
        self.assertNotIn(b"tRNS", chunks)
        self.assertNotIn(0, {value for row in rows for value in row})
        self.assertEqual(self.window["source"]["vram_tile"], "0x680")
        self.assertEqual(self.window["source"]["tile_count"], WINDOW_ART_TILES)

    def test_the_fill_tile_is_the_window_blue(self):
        """`loc_68704` fills the interior with `$C680`, blob tile 0, and that
        tile is a solid block of index $E -- the same colour
        `FillTextBackground` paints the text area with."""
        _, _, rows, chunks = png_pixels((self.root / WINDOW_PNG_NAME).read_bytes())
        fill = self.window["roles"]["fill"]
        self.assertEqual(fill["tile"], 0)
        pixels = {
            row[fill["x"] + x] for row in rows for x in range(TILE_PIXELS)
        }
        self.assertEqual(pixels, {BACKGROUND_COLOR_INDEX})
        palette = chunks[b"PLTE"]
        index = BACKGROUND_COLOR_INDEX * 3
        self.assertEqual(tuple(palette[index:index + 3]), (0, 0x24, 0x6D))
        self.assertEqual(self.window["palette"]["fill_index"], BACKGROUND_COLOR_INDEX)
        self.assertEqual(self.window["palette"]["cram_line"], 2)

    def test_the_border_tiles_look_like_a_border(self):
        """A corner has its outline on the top and left and nothing on the
        outside; an edge is a band across the whole tile. Cheap, and it is what
        catches a role map pointing at the wrong tile."""
        _, _, rows, _ = png_pixels((self.root / WINDOW_PNG_NAME).read_bytes())
        roles = self.window["roles"]

        def tile(role):
            x = roles[role]["x"]
            return [
                "".join(
                    "." if row[x + column] == BACKGROUND_COLOR_INDEX else "#"
                    for column in range(TILE_PIXELS)
                )
                for row in rows
            ]

        corner = tile("corner_top_left")
        self.assertEqual(corner[0], "........")   # nothing above the corner
        self.assertEqual(corner[1], ".#######")   # the top run starts one in
        self.assertTrue(all(row[0] == "." for row in corner))
        edge = tile("edge_top")
        self.assertEqual(edge[0], "........")
        self.assertEqual(edge[1], "########")     # a band across the whole cell
        self.assertEqual(edge[4], "........")
        left = tile("edge_left")
        self.assertEqual({row for row in left}, {".####..."})

    def test_the_window_records_are_the_cartridges(self):
        records = window_records(self.data)
        self.assertEqual([r["name"] for r in records], [
            "controls", "dialogue", "portrait", "yes_no",
        ])
        self.assertEqual(self.window["windows"], records)
        box = records[DIALOGUE_WINDOW_INDEX]
        self.assertEqual(box["record_offset"], "0x069388")
        self.assertEqual(
            (box["width_cells"], box["height_cells"], box["x_cell"], box["y_cell"]),
            (34, 6, 3, 20),
        )
        self.assertEqual(box["rect"], {"x": 24, "y": 160, "width": 272, "height": 48})
        portrait = records[2]
        self.assertEqual(portrait["rect"], {"x": 40, "y": 104, "width": 48, "height": 48})

    def test_the_box_interior_is_exactly_the_text_area(self):
        """The window record and `TextBufferToPlane`'s immediates are separate
        pieces of the cartridge that describe the same rectangle, so the
        emitter checks them against each other rather than trusting either."""
        records = window_records(self.data)
        self.assertEqual(check_window_fits_the_text(records)["interior"], {
            "x_cell": WINDOW_TILE_X, "y_cell": WINDOW_TILE_Y,
            "width_cells": WINDOW_TILE_WIDTH, "height_cells": WINDOW_TILE_HEIGHT,
        })
        moved = [dict(record) for record in records]
        moved[DIALOGUE_WINDOW_INDEX] = dict(
            moved[DIALOGUE_WINDOW_INDEX],
            interior={"x_cell": 4, "y_cell": 21, "width_cells": 30, "height_cells": 4},
        )
        with self.assertRaises(DialoguePackError):
            check_window_fits_the_text(moved)

    def test_the_geometry_rule_is_a_one_cell_border(self):
        geometry = self.window["geometry"]
        self.assertEqual(geometry["border_cells"], 1)
        self.assertFalse(geometry["shadow"])
        self.assertEqual(geometry["open_animation"]["step_cells"], 2)
        self.assertEqual(geometry["open_animation"]["from"], "center")

    # -- portraits ----------------------------------------------------------
    def test_every_portrait_is_written(self):
        self.assertEqual(self.portraits["count"], 39)
        self.assertEqual(self.portraits["distinct_art"], 36)
        self.assertEqual(
            [entry["id"] for entry in self.portraits["portraits"]], list(range(1, 40))
        )
        for entry in self.portraits["portraits"]:
            image = (self.root / entry["png"]).read_bytes()
            self.assertEqual(hashlib.sha256(image).hexdigest(), entry["png_sha256"])
            width, height, _, chunks = png_pixels(image)
            self.assertEqual((width, height), (48, 48))
            self.assertEqual(chunks[b"tRNS"][0], 0)
            self.assertEqual(entry["art"]["tile_count"], PORTRAIT_TILES)
            self.assertTrue(entry["png"].startswith(PORTRAITS_DIRECTORY))

    def test_chaz_is_pinned(self):
        chaz = self.portraits["portraits"][0]
        self.assertEqual(chaz["symbol"], "Chaz")
        self.assertEqual(chaz["png"], f"{PORTRAITS_DIRECTORY}/01_Chaz.png")
        self.assertEqual(chaz["png_sha256"], CHAZ_PORTRAIT_SHA)
        self.assertEqual(chaz["art"]["rom_offset"], "0x29BE5C")
        self.assertIsNone(chaz["duplicate_of"])

    def test_the_repeated_portraits_share_their_art(self):
        """Six ids point at three blobs; the files are byte-identical and each
        second id says which one it repeats."""
        by_id = {entry["id"]: entry for entry in self.portraits["portraits"]}
        for first, second in ((22, 23), (24, 25), (30, 31)):
            self.assertEqual(by_id[second]["duplicate_of"], first)
            self.assertEqual(by_id[second]["png_sha256"], by_id[first]["png_sha256"])
        self.assertEqual(
            sum(1 for e in self.portraits["portraits"] if e["duplicate_of"] is None), 36
        )

    def test_every_portrait_the_script_names_exists(self):
        """The id space `$F4` uses and the id space the table defines are the
        same one, plus id 0, which means hide the portrait window."""
        known = {entry["id"] for entry in self.portraits["portraits"]}
        used = set()
        for tree in self.trees["trees"]:
            for entry in tree["entries"]:
                for segment in entry["segments"]:
                    if segment.get("ctrl") == "portrait":
                        used.add(segment["id"])
        self.assertTrue(used - {0} <= known, sorted(used - {0} - known))
        self.assertIn(0, used)
        census = self.fragment["census"]
        self.assertEqual(census["portrait_ids_outside_table"], [])
        self.assertEqual(census["portrait_ids_unused"], [18, 22, 24, 30])
        self.assertEqual(
            {int(key) for key in census["portrait_ids"]}, used
        )

    def test_the_talk_trees_use_three_portrait_slots(self):
        """`mulu.w #$C,d5` -- the Talk command's second operand moves the
        portrait twelve tiles right per slot, and the script uses 0, 1 and 2."""
        self.assertEqual(
            sorted(int(key) for key in self.fragment["census"]["portrait_positions"]),
            [0, 1, 2],
        )
        self.assertEqual(self.portraits["geometry"]["talk_slot_stride_tiles"], 12)

    # -- census -------------------------------------------------------------
    def test_the_census_reports_what_the_script_uses(self):
        census = self.fragment["census"]
        self.assertEqual(census["entries"], self.trees["entry_count"])
        self.assertEqual(sorted(census["control_codes"]), [
            "0xF2", "0xF3", "0xF4", "0xF5", "0xF6", "0xF7", "0xF9", "0xFA",
            "0xFC", "0xFD",
        ])
        self.assertEqual(sorted(census["unused_control_codes"]), [
            "0xF0", "0xF1", "0xF8", "0xFB", "0xFE", "0xFF",
        ])
        self.assertEqual(census["longest_line"], CHARS_PER_LINE)
        self.assertEqual(census["lines_past_the_window"], [])
        self.assertEqual(census["preamble"]["outside_preamble"], [])
        self.assertEqual(census["preamble"]["events"], 69)
        self.assertEqual(census["preamble"]["keep_npc_facing"], 33)
        # 1,098 of the 2,736 entries are empty: `GetDialogueByID` counts $FF
        # bytes, so a hole in the id space is a pair of adjacent terminators.
        self.assertEqual(census["empty_entries"], 1098)
        self.assertEqual(
            [entry["char"] for entry in census["unused_charset_bytes"]],
            ["X", "*", "<", ">", "%", "6", "7", "9", "[", "(", ")", "="],
        )

    def test_the_census_counts_every_page_ending(self):
        census = self.fragment["census"]
        self.assertEqual(sorted(census["page_endings"]), [
            "choice", "close", "end", "full", "wait",
        ])
        self.assertEqual(
            census["pages"], sum(census["page_endings"].values())
        )

    # -- the fragment -------------------------------------------------------
    def test_the_fragment_names_files_that_exist_and_match(self):
        fragment = self.fragment
        for path, sha in (
            (fragment["trees"]["path"], fragment["trees"]["sha256"]),
            (fragment["font"]["path"], fragment["font"]["sha256"]),
            (fragment["font"]["png"], fragment["font"]["png_sha256"]),
            (fragment["chrome"]["path"], fragment["chrome"]["sha256"]),
            (fragment["chrome"]["png"], fragment["chrome"]["png_sha256"]),
            (fragment["portraits"]["path"], fragment["portraits"]["sha256"]),
        ):
            data = (self.root / path).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), sha, path)
        self.assertEqual(fragment["trees"]["tree_count"], 43)
        self.assertEqual(fragment["portraits"]["count"], 39)
        written = sorted(
            str(path.relative_to(self.root))
            for path in self.root.rglob("*") if path.is_file()
        )
        self.assertEqual(len(written), 6 + 39)
        self.assertIn(TREES_NAME, written)

    def test_rebuilding_is_byte_identical(self):
        with tempfile.TemporaryDirectory() as other:
            root = Path(other)
            fragment = emit_dialogue(self.data, root)
            self.assertEqual(fragment, self.fragment)
            for path in sorted(self.root.rglob("*")):
                if path.is_file():
                    name = path.relative_to(self.root)
                    self.assertEqual(
                        (root / name).read_bytes(), path.read_bytes(), str(name)
                    )

    def entry(self, tree: int, entry_id: int) -> dict:
        record = next(t for t in self.trees["trees"] if t["tree"] == tree)
        return record["entries"][entry_id]


class TestCensus(unittest.TestCase):
    def test_an_empty_census_is_still_shaped_like_one(self):
        census = Census().to_json((1, 2), ())
        self.assertEqual(census["entries"], 0)
        self.assertEqual(census["portrait_ids_unused"], [1, 2])
        self.assertEqual(len(census["unused_charset_bytes"]), len(DIALOGUE_CHARSET))


if __name__ == "__main__":
    unittest.main()
