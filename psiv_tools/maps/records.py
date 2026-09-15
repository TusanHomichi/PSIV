"""Field maps: the 417-entry pointer table and the per-map record grammar.

`GameMode_LoadFieldMap` (`ps4.asm`) is the whole specification. It indexes
`FieldMapPtrs` by `Field_Map_Index`, loads the resulting long into `a0`, and
then hands `a0` to a fixed sequence of subroutines, each of which advances it
past its own section. A map record therefore has no length field and no
per-section offsets: it is a stream that can only be read by replaying the
loader. Everything below is transcribed from those subroutines, in the order
the loader calls them.

    byte    Map_General_Var
    byte    music id (0 = leave the current track playing)
    ...     tilesets                    loc_519D2
    ...     sprite art (Nemesis)        loc_51A1A
    ...     sprite art (Kosinski)       loc_51A5C
    (nothing)                           loc_51A7A -- vehicle art, no map data
    4 bytes FG/BG row and column sizes
    ...     camera/scroll setup         loc_51AB2
    ...     chunk pointers              Map_LoadChunks
    ...     map updates, $FF            GoPast_FF_Terminator
    ...     transitions, $FFFF          GoPast_FFFF_Terminator
    ...     transitions 2, $FFFF        GoPast_FFFF_Terminator
    (nothing)                           loc_53546 -- party placement
    ...     objects                     LoadMapObjects
    ...     treasure chests             LoadTreasureChests
    ...     tile animations             LoadTileAnimations
    (nothing)                           loc_53854 -- camera clamping
    long    FG layout pointer           loc_539E2   (world maps: absent)
    long    BG layout pointer           loc_53A04   (world maps: absent)
    long    dialogue tree pointer       Map_DialogueTreesToRAM
    ...     interaction areas, $FFFF    GoPast_FFFF_Terminator
    ...     events, $FF                 GoPast_FF_Terminator
    long    palette pointer             loc_53F14
    4 bytes poison / random battles / town teleport / dungeon teleport
    ...     MapDataManager indexes, $FFFF

Three of the loader's steps consume nothing at all -- `loc_51A7A` reads
`Vehicle_Index` and its own pointer table, `loc_53546` places the party from
RAM, and `loc_53854` clamps the camera -- so they are named here only to keep
the sequence checkable against the asm.

`loc_539E2` and `loc_53A04` are the one genuine branch in the grammar. Both
test `Field_Map_Index & $FFFE` and skip the read entirely when it is zero,
which is true for exactly the two world maps (`MapID_Motavia = 0`,
`MapID_Dezolis = 1`); those two build their layouts from the fixed tables at
`loc_107DC2` / `loc_10BE02` instead. Every other option- and revision-gated
branch in `GameMode_LoadFieldMap` (`bugfixes`, `dialogue_uncompressed`) only
touches RAM, so the retail grammar and this clone's grammar are the same
stream either way.

Sections terminate in one of three ways and the distinction is load-bearing:

* `tst.w (a0)` + `bmi` -- any *negative* word ends the list (tilesets,
  Nemesis sprites, objects, chests, tile animations). $FFFF is what retail
  actually stores in all 361 real records, but $FFFE would work too, and in
  the Nemesis sprite list $FFFE means something else entirely (see below).
* `cmpi.w #$FFFF` -- only $FFFF ends the list (Kosinski sprites, chunks,
  transitions, interaction areas, MapDataManager).
* `cmpi.b #$FF` -- byte lists (map updates, events).

`LoadMapObjects` and `LoadTreasureChests` have a second path: when
`Map_Load_Flags` says the objects are already in RAM they call
`GoPast_FFFF_Terminator` instead of parsing. That word scan and the record
walk only agree if no record contains $FFFF at an even offset and the
terminator is exactly $FFFF, so both are checked here rather than assumed.

This module decodes one record at a time. Walking the whole pointer table is
`psiv_tools.maps.extract_maps`, and the encounter tables a map binds to live
in `psiv_tools.maps.encounters`.
"""

from __future__ import annotations

from typing import Any

from ..symbols import FIELD_OBJECT_SYMBOLS, MAP_SYMBOLS, MUSIC_ID_BASE, MUSIC_SYMBOLS

# FieldMapPtrs: 417 longs of absolute 68000 addresses. The ROM is mapped at 0
# so the pointers are already retail file offsets. The table ends exactly
# where its own first entry points, which is the boundary proof.
FIELD_MAP_PTRS = 0x100000
MAP_COUNT = 417
POINTER_SIZE = 4

# PtrMap_Null* all point at ErrorTrap. Entries 3..$F are the first run of
# them, so the 16-long prefix pins both the address and the null convention.
ERROR_TRAP = 0x000200
FIELD_MAP_PTRS_SIGNATURE = bytes.fromhex(
    "001006840010fea20011966400000200000002000000020000000200000002000000"
    "020000000200000002000000020000000200000002000000020000000200"
)

WORD_TERMINATOR = 0xFFFF
BYTE_TERMINATOR = 0xFF
# loc_51A1A only: not a terminator but a "decompress to RAM instead of VRAM"
# marker introducing a 6-byte body.
SPRITE_TO_RAM_MARKER = 0xFFFE

TRANSITION_SIZE = 10
OBJECT_SIZE = 10
INTERACTION_SIZE = 10
CHEST_SIZE = 6
TILE_ANIMATION_SIZE = 2

# Palette data pointed at by loc_53F14: 16 longs for CRAM lines 0-1, then 8
# more longs for line 3. Line 2 is copied from Pal_Init_Line_3 and is not part
# of the map's own blob.
PALETTE_WORDS = 48
PALETTE_SIZE = PALETTE_WORDS * 2
PALETTE_LINES = (0, 1, 3)
PALETTE_LINE_FROM_PAL_INIT = 2

# Jump-table sizes, counted from the disassembly. Every index a map record
# stores has to land inside its table or the game jumps into the middle of
# some other routine, so these are hard bounds, not sanity ranges.
MAP_UPDATE_ROUTINES = 0x40  # MapUpdateJmpTbl
EVENT_ROUTINES = 0x80  # RunEventsJmpTbl (retail; grand_cross replaces it)
MAP_DATA_MANAGER_ROUTINES = 0xA0  # MapDataManagerJmpTbl
FIELD_OBJECT_STRIDE = 4  # object ids are byte offsets into FieldObjectsJmpTbl
# InteractionRoutines, as this clone assembles it. Entry 7 is the grand_cross
# addition; retail maps never select anything above 5, which the tests pin.
INTERACTION_ROUTINES = 8

# XYRangeJmpTbl: shared by map transitions and interaction areas. Each entry
# turns the record's (x, y) into a rectangle and tests the player against it.
XY_RANGES = (
    "Null", "XYPlus40", "XYPlus20", "XYExact", "XLower", "XHigher", "YLower",
    "YHigher", "XYLowerWithPlayerY", "XPlus20_YPlus10", "XPlus10_YPlus60",
    "XPlus40_YPlus20", "XPlus10_YPlus20", "XPlus60_YPlus10", "XPlus40_YPlus10",
)

# `Interaction_ChkMapAreas` stores byte 6 into Interaction_Event_Type.
INTERACTION_FLAG_TYPES = {0: "story", 1: "chest", 2: "temporary"}

# `Interaction_GetEvent` indexes this 32-word table with the area's byte 9.
# The high bit keeps the same cutscene-vs-event meaning as Event_Index.  It is
# emitted with the runtime area rather than re-created in the Rust bridge.
INTERACTION_EVENT_INDEXES = (
    0x0000, 0x0029, 0x002D, 0x0035, 0x0036, 0x0039, 0x003A, 0x0042,
    0x0008, 0x0066, 0x801E, 0x006B, 0x006C, 0x006D, 0x006E, 0x006F,
    0x0000, 0x0000, 0x0000, 0x0013, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x001B, 0x001C, 0x0000, 0x0000, 0x0000,
)

# Chest byte 1 -> Found_Item_Type. Zero means byte 3 is an item id; non-zero
# means byte 3 is a meseta count, rendered by loc_66BEE as the number followed
# by the string at loc_2AAACA, which decodes to "00 meseta procured!" -- so
# the stored byte is hundreds of meseta.
CHEST_MESETA_MULTIPLIER = 100


class MapError(ValueError):
    pass


def be16(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset:offset + 2], "big")


def be32(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset:offset + 4], "big")


def _hex(value: int, digits: int = 6) -> str:
    return f"0x{value:0{digits}X}"


def _music(value: int) -> dict[str, Any]:
    """Music id 0 means "keep playing whatever is playing"."""
    index = value - MUSIC_ID_BASE
    return {
        "id": value,
        "id_hex": f"0x{value:02X}",
        "symbol": MUSIC_SYMBOLS[index] if 0 <= index < len(MUSIC_SYMBOLS) else None,
        "changes_music": value != 0,
    }


def _map_reference(map_id: int) -> dict[str, Any]:
    return {
        "id": map_id,
        "id_hex": f"0x{map_id:03X}",
        "symbol": MAP_SYMBOLS[map_id] if 0 <= map_id < len(MAP_SYMBOLS) else None,
    }


def _xy_range(value: int) -> dict[str, Any]:
    return {
        "id": value,
        "name": XY_RANGES[value] if 0 <= value < len(XY_RANGES) else None,
    }


class _Cursor:
    """A replay of the loader's `a0`, with provenance for every step."""

    def __init__(self, data: bytes, offset: int, map_id: int, symbol: str):
        self.data = data
        self.offset = offset
        self.map_id = map_id
        self.symbol = symbol

    def fail(self, message: str) -> None:
        raise MapError(
            f"map 0x{self.map_id:03X} ({self.symbol}) at {_hex(self.offset)}: {message}"
        )

    def _check(self, size: int) -> None:
        if self.offset + size > len(self.data):
            self.fail(f"record runs past the end of the ROM reading {size} bytes")

    def byte(self) -> int:
        self._check(1)
        value = self.data[self.offset]
        self.offset += 1
        return value

    def word(self) -> int:
        self._check(2)
        value = be16(self.data, self.offset)
        self.offset += 2
        return value

    def long(self) -> int:
        self._check(4)
        value = be32(self.data, self.offset)
        self.offset += 4
        return value

    def peek_word(self) -> int:
        self._check(2)
        return be16(self.data, self.offset)

    def peek_byte(self) -> int:
        self._check(1)
        return self.data[self.offset]

    def skip(self, size: int) -> bytes:
        self._check(size)
        raw = self.data[self.offset:self.offset + size]
        self.offset += size
        return raw

    def hex_between(self, start: int, end: int) -> str:
        return self.data[start:end].hex()

    def section(self, start: int, **extra: Any) -> dict[str, Any]:
        return {
            "rom_offset": _hex(start),
            "rom_end_exclusive": _hex(self.offset),
            "size_bytes": self.offset - start,
            "raw_hex": self.hex_between(start, self.offset),
            **extra,
        }


def _pointer(cursor: _Cursor, value: int, what: str) -> int:
    if not 0 < value < len(cursor.data):
        cursor.fail(f"{what} pointer {_hex(value)} is outside the ROM")
    return value


# ------------------------------------------------------------------- sections


def _read_word_long_list(
    cursor: _Cursor,
    what: str,
    key: str,
    compression: str,
    *,
    negative_terminator: bool,
    sign_extend: bool = False,
) -> dict[str, Any]:
    """A list of (word, pointer long) pairs -- loc_519D2 and loc_51A5C.

    loc_519D2 ends on any negative word and its word is a VRAM tile number.
    loc_51A5C ends only on $FFFF and sign-extends its word into `$FFFF____`,
    so the value names a work-RAM destination rather than video memory. The
    two properties are independent even though retail only pairs them one way.
    """
    start = cursor.offset
    entries = []
    while True:
        head = cursor.peek_word()
        if head == WORD_TERMINATOR or (negative_terminator and head >= 0x8000):
            break
        entry_start = cursor.offset
        word = cursor.word()
        pointer = _pointer(cursor, cursor.long(), what)
        entries.append({
            "index": len(entries),
            "rom_offset": _hex(entry_start),
            key: f"0x{0xFFFF0000 | word:08X}" if sign_extend else word,
            "source_offset": _hex(pointer),
            "compression": compression,
            "raw_hex": cursor.hex_between(entry_start, cursor.offset),
        })
    terminator = cursor.word()
    return cursor.section(
        start,
        terminator=f"0x{terminator:04X}",
        entry_count=len(entries),
        entries=entries,
    )


def _read_nemesis_sprites(cursor: _Cursor) -> dict[str, Any]:
    """loc_51A1A: Nemesis sprite art.

    A leading $FFFE switches the destination from VRAM to RAM and is followed
    by the same (tile number, art pointer) body, so it costs 8 bytes rather
    than 6 and does *not* end the list. Any other negative word does.
    """
    start = cursor.offset
    entries = []
    while True:
        entry_start = cursor.offset
        head = cursor.peek_word()
        if head == SPRITE_TO_RAM_MARKER:
            cursor.word()
            destination = "ram"
        else:
            cursor.word()
            if head >= 0x8000:
                terminator = head
                break
            destination = "vram"
        tile_number = cursor.word() if destination == "ram" else head
        art = _pointer(cursor, cursor.long(), "sprite art")
        entries.append({
            "index": len(entries),
            "rom_offset": _hex(entry_start),
            "destination": destination,
            "tile_number": tile_number,
            "source_offset": _hex(art),
            "compression": "nemesis",
            "raw_hex": cursor.hex_between(entry_start, cursor.offset),
        })
    return cursor.section(
        start,
        terminator=f"0x{terminator:04X}",
        entry_count=len(entries),
        entries=entries,
    )


def _read_scroll(cursor: _Cursor) -> dict[str, Any]:
    """loc_51AB2: one byte, one skipped byte, then two conditional blocks.

    Each of the two words is stored as a byte and only when that byte is zero
    does the routine read the four longs that follow it. That makes the
    section 6, 14 or 22 bytes long, and getting the condition backwards
    desynchronises everything after it.
    """
    start = cursor.offset
    mode = cursor.byte()
    padding = cursor.byte()
    result: dict[str, Any] = {"mode": mode, "padding_byte": f"0x{padding:02X}"}
    for plane in ("fg", "bg"):
        word = cursor.word()
        result[f"{plane}_scroll_mode"] = word & 0xFF
        result[f"{plane}_scroll_word"] = f"0x{word:04X}"
        if word & 0xFF:
            result[f"{plane}_step_counters"] = None
        else:
            result[f"{plane}_step_counters"] = {
                "x": f"0x{cursor.long():08X}",
                "y": f"0x{cursor.long():08X}",
            }
    return cursor.section(start, **result)


def _read_chunks(cursor: _Cursor) -> dict[str, Any]:
    """Map_LoadChunks: Kosinski chunk-table pointers until $FFFF.

    The compressed chunk tables themselves are a separate problem; only the
    pointers and this section's extent belong to the record grammar.
    """
    start = cursor.offset
    pointers = []
    while cursor.peek_word() != WORD_TERMINATOR:
        pointers.append(_hex(_pointer(cursor, cursor.long(), "chunk table")))
    cursor.word()
    return cursor.section(
        start,
        terminator=f"0x{WORD_TERMINATOR:04X}",
        pointer_count=len(pointers),
        compression="kosinski",
        pointers=pointers,
    )


def _read_index_list(cursor: _Cursor, width: int, limit: int, what: str) -> dict[str, Any]:
    """A terminated list of jump-table indexes.

    Map updates and events are byte lists ended by `$FF`; MapDataManager is a
    word list ended by `$FFFF`. The index has to land inside its jump table or
    the game jumps into the middle of an unrelated routine, so an out-of-range
    value is a grammar violation, not a curiosity.
    """
    start = cursor.offset
    peek, read = (cursor.peek_byte, cursor.byte) if width == 1 else (cursor.peek_word, cursor.word)
    terminator = BYTE_TERMINATOR if width == 1 else WORD_TERMINATOR
    ids = []
    while peek() != terminator:
        value = read()
        if value >= limit:
            cursor.fail(
                f"{what} index 0x{value:0{width * 2}X} is outside the "
                f"{limit}-entry jump table"
            )
        ids.append(value)
    read()
    return cursor.section(
        start,
        terminator=f"0x{terminator:0{width * 2}X}",
        count=len(ids),
        ids=ids,
    )


def _scan_to_word_terminator(cursor: _Cursor, offset: int) -> int:
    """`GoPast_FFFF_Terminator`, byte-for-byte: step words until $FFFF.

    Run from a section's first byte this is exactly what the loader does when
    it skips a section instead of parsing it, so its answer can be compared
    against a structural record walk over the same bytes. The returned offset
    is the terminator's own address.
    """
    while True:
        if offset + 2 > len(cursor.data):
            cursor.fail("section is not $FFFF-terminated before the end of the ROM")
        if be16(cursor.data, offset) == WORD_TERMINATOR:
            return offset
        offset += 2


def _read_fixed_records(
    cursor: _Cursor,
    size: int,
    what: str,
    decode,
    *,
    negative_terminator: bool,
    cross_check: bool = True,
) -> dict[str, Any]:
    """Walk fixed-size records up to a terminator, then cross-check the scan.

    `negative_terminator` selects between the two terminator tests the loader
    actually uses. `cross_check` replays `GoPast_FFFF_Terminator` from the
    section's first byte: objects and treasure chests take that path instead
    of parsing when `Map_Load_Flags` says they are already in RAM, so the two
    have to land on the same terminator -- which they only do if no record
    holds $FFFF at an even offset and the terminator is exactly $FFFF. Tile
    animations have no such alternative path and would be over-constrained by
    the check, since the loader accepts any negative word to end them.
    """
    start = cursor.offset
    entries = []
    while True:
        head = cursor.peek_word()
        if head == WORD_TERMINATOR or (negative_terminator and head >= 0x8000):
            break
        entry_start = cursor.offset
        raw = cursor.skip(size)
        entries.append({
            "index": len(entries),
            "rom_offset": _hex(entry_start),
            **decode(raw),
            "raw_hex": raw.hex(),
        })
    if cross_check:
        scanned = _scan_to_word_terminator(cursor, start)
        if scanned != cursor.offset:
            cursor.fail(
                f"{what}: the record walk ends at {_hex(cursor.offset)} but "
                f"GoPast_FFFF_Terminator stops at {_hex(scanned)}; the loader's "
                "two paths through this section disagree"
            )
    terminator = cursor.word()
    return cursor.section(
        start,
        terminator=f"0x{terminator:04X}",
        record_size=size,
        count=len(entries),
        entries=entries,
    )


def _decode_transition(raw: bytes) -> dict[str, Any]:
    """DoMapTransitionData, 10 bytes.

    Source coordinates are tile bytes scaled by 16. Destination coordinates go
    through `Map_Start_X_Pos` (byte * 2) and are scaled by 8 when the party is
    placed, so both ends land on the same 16-pixel grid.
    """
    return {
        "source": {"x_tile": raw[0], "y_tile": raw[1], "x": raw[0] * 16, "y": raw[1] * 16},
        "range": _xy_range(int.from_bytes(raw[2:4], "big")),
        "target": _map_reference(int.from_bytes(raw[4:6], "big")),
        "destination": {"x_tile": raw[6], "y_tile": raw[7], "x": raw[6] * 16, "y": raw[7] * 16},
        "facing_dir": raw[8],
        "character_alignment": raw[9],
    }


def _decode_object(raw: bytes) -> dict[str, Any]:
    """LoadMapObjects, 10 bytes. Coordinates are tile words scaled by 8."""
    object_id = int.from_bytes(raw[0:2], "big")
    index = object_id // FIELD_OBJECT_STRIDE
    return {
        "object_id": object_id,
        "symbol": (
            FIELD_OBJECT_SYMBOLS[index]
            if object_id % FIELD_OBJECT_STRIDE == 0 and index < len(FIELD_OBJECT_SYMBOLS)
            else None
        ),
        "facing_dir": raw[2],
        "dialogue_id": raw[3],
        "art_tile": int.from_bytes(raw[4:6], "big"),
        "x_tile": int.from_bytes(raw[6:8], "big"),
        "y_tile": int.from_bytes(raw[8:10], "big"),
        "x": int.from_bytes(raw[6:8], "big") * 8,
        "y": int.from_bytes(raw[8:10], "big") * 8,
    }


def _decode_chest(raw: bytes) -> dict[str, Any]:
    """LoadTreasureChests, 6 bytes. Coordinates are tile bytes scaled by 16."""
    is_meseta = bool(raw[1])
    return {
        "white_chest": bool(raw[0]),
        "object_symbol": "WhiteTreasureChest" if raw[0] else "TreasureChest",
        "contents_type": "meseta" if is_meseta else "item",
        "item_id": None if is_meseta else raw[3],
        "meseta": raw[3] * CHEST_MESETA_MULTIPLIER if is_meseta else None,
        "chest_flag": raw[2],
        "x_tile": raw[4],
        "y_tile": raw[5],
        "x": raw[4] * 16,
        "y": raw[5] * 16,
    }


def _decode_interaction(raw: bytes) -> dict[str, Any]:
    """Interaction_ChkMapAreas, 10 bytes. Coordinates are tile words * 8."""
    flag_type = raw[6]
    interaction_type = raw[8]
    return {
        "x_tile": int.from_bytes(raw[0:2], "big"),
        "y_tile": int.from_bytes(raw[2:4], "big"),
        "x": int.from_bytes(raw[0:2], "big") * 8,
        "y": int.from_bytes(raw[2:4], "big") * 8,
        "range": _xy_range(int.from_bytes(raw[4:6], "big")),
        "flag_type": {"id": flag_type, "name": INTERACTION_FLAG_TYPES.get(flag_type)},
        "flag": raw[7],
        "interaction_type": interaction_type,
        # `Interaction_ChkMapAreas` returns byte 9 in d4 and what it means
        # depends on the routine byte 8 selected: a dialogue id for
        # Interaction_DisplayDialogue, an event index for Interaction_GetEvent,
        # and so on. It is deliberately not over-interpreted here.
        "parameter": raw[9],
    }


def _decode_tile_animation(raw: bytes) -> dict[str, Any]:
    """LoadTileAnimations, 2 bytes: animation id and its update rate."""
    return {"animation_id": raw[0], "parameter": raw[1]}


def _read_palette(cursor: _Cursor) -> dict[str, Any]:
    """loc_53F14: one pointer to 48 CRAM words for lines 0, 1 and 3."""
    start = cursor.offset
    pointer = _pointer(cursor, cursor.long(), "palette")
    if pointer + PALETTE_SIZE > len(cursor.data):
        cursor.fail(f"palette at {_hex(pointer)} runs past the end of the ROM")
    raw = cursor.data[pointer:pointer + PALETTE_SIZE]
    lines = {}
    for slot, line in enumerate(PALETTE_LINES):
        chunk = raw[slot * 32:(slot + 1) * 32]
        lines[str(line)] = {
            "rom_offset": _hex(pointer + slot * 32),
            "raw_hex": chunk.hex(),
        }
    return cursor.section(
        start,
        pointer=_hex(pointer),
        palette_size_bytes=PALETTE_SIZE,
        lines_from_map=list(PALETTE_LINES),
        line_from_pal_init_3=PALETTE_LINE_FROM_PAL_INIT,
        lines=lines,
    )


# ------------------------------------------------------------------ map record


def _walk_map(
    data: bytes,
    map_id: int,
    pointer: int,
    dialogue_trees: dict[int, dict[str, Any]],
) -> dict[str, Any]:
    symbol = MAP_SYMBOLS[map_id]
    cursor = _Cursor(data, pointer, map_id, symbol)

    general_var = cursor.byte()
    music = _music(cursor.byte())
    tilesets = _read_word_long_list(
        cursor, "tileset art", "vram_tile", "kosinski", negative_terminator=True
    )
    sprites = _read_nemesis_sprites(cursor)
    sprite_data = _read_word_long_list(
        cursor, "sprite data", "ram_address", "kosinski",
        negative_terminator=False, sign_extend=True,
    )

    dimensions_start = cursor.offset
    sizes = [cursor.byte() for _ in range(4)]
    dimensions = cursor.section(dimensions_start, **dict(zip(
        ("fg_row_size", "fg_column_size", "bg_row_size", "bg_column_size"), sizes
    )))

    scroll = _read_scroll(cursor)
    chunks = _read_chunks(cursor)
    map_updates = _read_index_list(cursor, 1, MAP_UPDATE_ROUTINES, "map update")
    transitions = _read_fixed_records(
        cursor, TRANSITION_SIZE, "transitions", _decode_transition,
        negative_terminator=False,
    )
    transitions_2 = _read_fixed_records(
        cursor, TRANSITION_SIZE, "transitions 2", _decode_transition,
        negative_terminator=False,
    )
    objects = _read_fixed_records(
        cursor, OBJECT_SIZE, "objects", _decode_object, negative_terminator=True,
    )
    chests = _read_fixed_records(
        cursor, CHEST_SIZE, "treasure chests", _decode_chest, negative_terminator=True,
    )
    tile_animations = _read_fixed_records(
        cursor, TILE_ANIMATION_SIZE, "tile animations", _decode_tile_animation,
        negative_terminator=True, cross_check=False,
    )

    layout_start = cursor.offset
    if map_id & 0xFFFE:
        fg_layout = _pointer(cursor, cursor.long(), "FG layout")
        bg_layout = _pointer(cursor, cursor.long(), "BG layout")
        layout = cursor.section(
            layout_start,
            present=True,
            fg_offset=_hex(fg_layout),
            bg_offset=_hex(bg_layout),
            compression="kosinski",
        )
    else:
        # loc_539E2/loc_53A04 skip the read entirely for MapID 0 and 1 and
        # build both planes from loc_107DC2 / loc_10BE02 instead.
        layout = cursor.section(
            layout_start,
            present=False,
            reason=(
                "loc_539E2/loc_53A04 test Field_Map_Index & $FFFE and take the "
                "world-map path, which reads no bytes from the record"
            ),
        )

    dialogue_start = cursor.offset
    dialogue_pointer = _pointer(cursor, cursor.long(), "dialogue tree")
    tree = dialogue_trees.get(dialogue_pointer)
    if tree is None:
        cursor.fail(
            f"dialogue pointer {_hex(dialogue_pointer)} does not match the start "
            "of any of the 43 Kosinski dialogue trees"
        )
    dialogue = cursor.section(
        dialogue_start,
        pointer=_hex(dialogue_pointer),
        compression="kosinski",
        **tree,
    )

    interaction_areas = _read_fixed_records(
        cursor, INTERACTION_SIZE, "interaction areas", _decode_interaction,
        negative_terminator=False,
    )
    for entry in interaction_areas["entries"]:
        if entry["interaction_type"] >= INTERACTION_ROUTINES:
            cursor.fail(
                f"interaction area at {entry['rom_offset']} selects routine "
                f"{entry['interaction_type']}, outside InteractionRoutines"
            )
    events = _read_index_list(cursor, 1, EVENT_ROUTINES, "event")
    palette = _read_palette(cursor)

    flags_start = cursor.offset
    poison = cursor.byte()
    random_battles = cursor.byte()
    town_teleport = cursor.byte()
    dungeon_teleport = cursor.byte()
    flags = cursor.section(
        flags_start,
        poison=poison,
        random_battles=random_battles,
        town_teleport=town_teleport,
        # `bmi` skips the store: a negative byte inherits the current exit.
        # Valley Maze uses this to remember which entrance HINAS returns to.
        dungeon_teleport_index=None if dungeon_teleport & 0x80 else dungeon_teleport,
        dungeon_teleport_raw=f"0x{dungeon_teleport:02X}",
    )

    map_data_manager = _read_index_list(
        cursor, 2, MAP_DATA_MANAGER_ROUTINES, "MapDataManager"
    )

    return {
        **_map_reference(map_id),
        "is_null": False,
        "pointer_offset": _hex(FIELD_MAP_PTRS + map_id * POINTER_SIZE),
        "pointer": _hex(pointer),
        "rom_offset": _hex(pointer),
        "rom_end_exclusive": _hex(cursor.offset),
        "size_bytes": cursor.offset - pointer,
        "general_var": general_var,
        "music": music,
        "tilesets": tilesets,
        "sprites": sprites,
        "sprite_data": sprite_data,
        "dimensions": dimensions,
        "scroll": scroll,
        "chunks": chunks,
        "map_updates": map_updates,
        "transitions": transitions,
        "transitions_2": transitions_2,
        "objects": objects,
        "treasure_chests": chests,
        "tile_animations": tile_animations,
        "layout": layout,
        "dialogue": dialogue,
        "interaction_areas": interaction_areas,
        "events": events,
        "palette": palette,
        "flags": flags,
        "map_data_manager": map_data_manager,
        "raw_hex": data[pointer:cursor.offset].hex(),
    }


# ------------------------------------------------------------- pointer table


def _dialogue_tree_index(data: bytes) -> dict[int, dict[str, Any]]:
    """Map each dialogue tree's ROM offset to its extracted tree identity."""
    from ..text import dialogue_tree_specs

    return {
        spec["start"]: {"tree": spec["tree"], "label": spec["label"]}
        for spec in dialogue_tree_specs(data)
    }


def _check_pointer_table(data: bytes) -> dict[str, Any]:
    start = FIELD_MAP_PTRS
    end = start + MAP_COUNT * POINTER_SIZE
    actual = data[start:start + len(FIELD_MAP_PTRS_SIGNATURE)]
    if actual != FIELD_MAP_PTRS_SIGNATURE:
        raise MapError(
            f"FieldMapPtrs signature mismatch at {_hex(start)}: expected "
            f"{FIELD_MAP_PTRS_SIGNATURE.hex()}, got {actual.hex()}"
        )
    if data.count(FIELD_MAP_PTRS_SIGNATURE) != 1:
        raise MapError(
            f"FieldMapPtrs signature occurs {data.count(FIELD_MAP_PTRS_SIGNATURE)} "
            "times in this ROM; it must be unique for the offset to be self-verifying"
        )
    first = be32(data, start)
    if first != end:
        raise MapError(
            f"FieldMapPtrs is {MAP_COUNT} longs ending at {_hex(end)}, but its "
            f"first entry (Map_Motavia) is {_hex(first)}; the table is supposed "
            "to be flush against the first map record"
        )
    return {
        "label": "FieldMapPtrs",
        "rom_offset": _hex(start),
        "rom_end_exclusive": _hex(end),
        "entry_size": POINTER_SIZE,
        "count": MAP_COUNT,
        "signature_hex": FIELD_MAP_PTRS_SIGNATURE.hex(),
        "error_trap": _hex(ERROR_TRAP),
        "indexing": (
            "Field_Map_Index * 4; the long is an absolute 68000 address and the "
            "ROM maps at 0, so it is also the retail file offset"
        ),
        "flush_against_first_record": _hex(first),
    }
