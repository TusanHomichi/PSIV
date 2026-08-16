"""Battle presentation art: enemy sprites and character battle poses.

Static enemy bodies and party members are composed into plane buffers and drawn
with `PlaneMapToRAM`, so that base pipeline is the one `psiv_tools.planes`
already models: a tile bank, a grid of pattern-name words, and a CRAM line.
Enemy animation pieces are separately managed by the retail `Enemy_Sprites`
path and emitted by `psiv_tools.battle_enemy_overlays` as dynamic tile
replacements over that body.

## Enemies

Three parallel tables, all indexed by `fighter_id` (= the 0-based enemy id, so
`EnemyID_Helex = 0`). None of the binding lives in the 48-byte enemy record.

    loc_27F3AE   20 bytes  the graphics record (below)
    loc_27FFA2   22 bytes  eleven palette words
    EnemySpriteMappingsOffs  the dynamic tile-replacement piece lists

The graphics record is:

    +$00 word  VRAM allocation, in patterns
    +$02 long  ArtNem #1, decompressed to VRAM by `loc_7BFA`
    +$06 long  ArtNem #2, decompressed to RAM and used by the piece lists
    +$0A long  ArtNem #3, decompressed to RAM for the sprite-piece system
    +$0E long  the body plane mapping, Enigma
    +$12 byte  half-width in cells
    +$13 byte  height in cells

Three facts about that record were established against the cartridge rather
than assumed, because each one is easy to get wrong:

- **The +$00 word is a VRAM reservation, not a tile count.** `loc_7BFA` does
  `lsl.w #5,d2 / add.w d2,($FFFFEE54).w` -- it advances the running VRAM
  allocator. It equals ArtNem #1's own header count for only 49 of the 153
  enemies, and for 14 enemies it is smaller than the largest pattern the
  mapping references, so it cannot bound anything.
- **The mapping addresses all three art blobs concatenated, in the order
  art#3, art#1, art#2.** Art #1 alone covers only 102 of the 153 mappings. Of
  the six possible orderings, this one alone puts a blank pattern at index 0
  for 141 of the 153 enemies -- index 0 is the empty cell in every mapping --
  and it leaves the fewest cells unresolved by a wide margin (303, against 483
  for the next best). Art #3 is the small shared blob two enemies at a time
  have in common, which is consistent with it being loaded first.
- **The width byte is a half-width.** `loc_10ED4` does `add.w d1,d1` before
  calling `PlaneMapToRAM`, so the drawn grid is `2 * width` by `height`, and
  the mapping supplies exactly that many words. That is presumably because 69
  of the 153 are exact mirrors, drawing the right half as the left half with
  the H-flip bit set -- but the other 84 are not, so the halves are read as
  ordinary cells and the symmetry is only ever reported, never assumed.

Two enemies -- ProfoundDarkness2 and ProfoundDarkness3 -- do not have an
Enigma stream in the +$0E field at all. See `RAW_MAPPING_ENEMY_IDS`.

The body mapping is not the whole enemy. `loc_7C56` walks
`EnemySpriteMappingsOffs` into per-enemy dynamic tile replacements, animated
from Art #2 and copied into the Art #3 staging bank. The overlay decoder lives
in `psiv_tools.battle_enemy_overlays`; this module attaches its strict,
metadata-only result to each enemy so the body census and the replacement
census cannot drift apart.

## Characters

`CharBattleLongRangePLCOffs` is eleven self-relative words indexed by
`fighter_id - 1` (character ids are 1-based). Each block is:

    +$00 word  VRAM/DMA allocation in bytes
    +$02 long  uncompressed art, DMA'd straight in by `loc_8244`
    +$06...    one long per pose, an Enigma mapping

with a second art pointer allowed mid-list; it applies to the poses after it,
which is how Demi and Wren carry their gun art. Poses are indexed by their
position in the list -- `movea.l (a0,d0.w),a0` with `d0 = 4 * pose` -- so pose
0 is the idle pose for every character.

Characters draw as a fixed 6x6 grid: `loc_86D6` loads `moveq #6,d1 / moveq
#6,d2`. Their per-slot plane buffers are 36 words apart (`loc_9A80`).

## Palettes

Both are proven from the loaders, not guessed.

- An enemy occupies **CRAM line 1 or line 2**, chosen per battle slot from bit
  7 of its `Enemy_Positions` entry, not from anything about the enemy. The tail
  of `loc_10ED4` writes index 1 = `$EEE`, index 2 = `$000`, then copies the
  eleven table words into indices 3..13. Indices 14 and 15 are UI colours set
  once by `loc_7634`.
- Characters share **CRAM line 3**: every character mapping is decompressed
  with a base of `$E000`, which is the priority bit plus palette line 3.
  `loc_7634` fills indices 1..15 from `loc_76A4`, selected by `Vehicle_Index`.

Decoded pixels never leave this module as data; `extract_battle_art` returns
metadata only. `export_battle_art_pngs` writes images, and its output is Sega
artwork that must stay out of version control.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Sequence

from . import planes
from .enigma import EnigmaError
from .enigma import decompress as enigma_decompress
from .gfx import COLORS_PER_LINE, GRAYSCALE_RAMP, RGB, decode_palette, decode_tile, palette_rgb
from .nemesis import TILE_SIZE
from .nemesis import decompress as nemesis_decompress
from .nemesis import read_header as nemesis_header
from .symbols import ENEMY_SYMBOLS

# ---------------------------------------------------------------------------
# Enemies
# ---------------------------------------------------------------------------
ENEMY_COUNT = 153

ENEMY_ART_TABLE: dict[str, Any] = {
    "label": "loc_27F3AE",
    "rom_offset": 0x27F3AE,
    "entry_count": ENEMY_COUNT,
    "entry_size": 20,
}
ENEMY_PALETTE_TABLE: dict[str, Any] = {
    "label": "loc_27FFA2",
    "rom_offset": 0x27FFA2,
    "entry_count": ENEMY_COUNT,
    "entry_size": 22,
}

ENEMY_ART_FIELDS = 3  # +$02, +$06, +$0A

# Field indices in the order the mapping addresses them: art #3, then #1, #2.
# See the module docstring for the evidence; `test_battle_art` re-derives it.
ENEMY_ART_BANK_ORDER = (2, 0, 1)

BLANK_PATTERN = bytes(64)
ENEMY_PALETTE_COLORS = 11
ENEMY_PALETTE_FIRST_INDEX = 3
ENEMY_CRAM_LINES = (1, 2)

# `loc_10ED4`: move.w #$EEE,(a1)+ / clr.w (a1)+ before the eleven-word copy.
ENEMY_FIXED_COLORS: dict[int, int] = {1: 0x0EEE, 2: 0x0000}

# ProfoundDarkness2 and ProfoundDarkness3 point +$0E at a plain array of
# `2 * width * height` pattern-name words instead of an Enigma stream. The
# bytes are unambiguous: the Enigma header would declare zero inline bits,
# which `EniDecomp` cannot act on, while reading the field as raw words yields
# a well-formed mapping whose right half is the left half H-flipped. Exactly
# two enemies do this and `enemy_mapping` fails if a third ever appears.
RAW_MAPPING_ENEMY_IDS = frozenset({0x86, 0x87})

# ---------------------------------------------------------------------------
# Characters
# ---------------------------------------------------------------------------
CHARACTER_COUNT = 11

CHARACTER_PLC_TABLE: dict[str, Any] = {
    "label": "CharBattleLongRangePLCOffs",
    "rom_offset": 0x00825E,
    "entry_count": CHARACTER_COUNT,
    # The eleventh block's own length; the table has no terminator, so the end
    # of the last block is the one bound that cannot be read from the table.
    "rom_end_exclusive": 0x00836A,
}

# `fighter_id - 1` order, which is the order the PLC offset table is in.
CHARACTER_SYMBOLS: tuple[str, ...] = (
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika",
    "Demi", "Wren", "Raja", "Kyra", "Seth",
)

CHARACTER_COLUMNS = 6  # loc_86D6: moveq #6,d1
CHARACTER_ROWS = 6     # loc_86D6: moveq #6,d2
CHARACTER_SLOT_WORDS = CHARACTER_COLUMNS * CHARACTER_ROWS  # loc_9A80 stride $48
CHARACTER_CRAM_LINE = 3
CHARACTER_BASE_WORD = 0xE000  # loc_822C: addi.w #$E000,d0 -- priority + line 3

CHARACTER_PALETTE_TABLE: dict[str, Any] = {
    "label": "loc_76A4",
    "rom_offset": 0x0076A4,
    "entry_count": 4,  # on foot, then three vehicles
    "entry_size": 30,
}
CHARACTER_PALETTE_COLORS = 15
CHARACTER_PALETTE_FIRST_INDEX = 1

# `loc_7634` writes these as instruction immediates rather than from a table.
# The addresses are asserted before the values are trusted.
UI_COLOR_SITES: tuple[tuple[int, int, int, int], ...] = (
    # rom_offset, cram_line, colour index, value
    (0x007672, 0, 15, 0x02CE),
    (0x007678, 1, 15, 0x0CC4),
    (0x00767E, 2, 15, 0x062E),
)
UI_COLOR_14 = 0x0600  # move.w #$600,d0 then written to lines 0, 1 and 2

MOVE_W_IMMEDIATE_TO_SHORT = 0x31FC  # move.w #imm,(xxx).w
PALETTE_TABLE_BUFFER = 0xFB00  # Palette_Table_Buffer, as a short address


class BattleArtError(ValueError):
    pass


def _slice(data: bytes, offset: int, size: int, what: str) -> bytes:
    if offset < 0 or offset + size > len(data):
        raise BattleArtError(
            f"{what} at 0x{offset:06X} (+{size}) runs past the end of the ROM"
        )
    return data[offset:offset + size]


def _word(data: bytes, offset: int) -> int:
    return int.from_bytes(_slice(data, offset, 2, "Word"), "big")


def _long(data: bytes, offset: int) -> int:
    return int.from_bytes(_slice(data, offset, 4, "Pointer"), "big")


# ---------------------------------------------------------------------------
# Nemesis art, with the adjacency bound the surrounding tables provide
# ---------------------------------------------------------------------------
def _art_record(data: bytes, offset: int, label: str, bound: int | None) -> tuple[bytes, dict[str, Any]]:
    header = nemesis_header(data, offset)
    decompressed, consumed = nemesis_decompress(data, offset)
    if len(decompressed) != header.decompressed_size:
        raise BattleArtError(
            f"{label} at 0x{offset:06X}: decoded {len(decompressed)} bytes but the "
            f"header declares {header.tile_count} patterns"
        )
    # NemDecomp reads one byte of lookahead it may never use, so a stream may
    # legitimately consume one byte more than the gap to whatever follows it.
    if bound is not None and consumed > bound + 1:
        raise BattleArtError(
            f"{label} at 0x{offset:06X}: consumed {consumed} bytes and ran into the "
            f"next blob, which starts {bound} bytes in"
        )
    return decompressed, {
        "label": label,
        "rom_offset": f"0x{offset:06X}",
        "compression": "nemesis",
        "consumed": consumed,
        "tile_count": header.tile_count,
        "xor_mode": header.xor_mode,
        "decompressed_size": len(decompressed),
        "decompressed_sha256": hashlib.sha256(decompressed).hexdigest(),
    }


def enemy_records(data: bytes) -> list[dict[str, Any]]:
    """Read the 153 graphics records without decompressing anything."""
    base = ENEMY_ART_TABLE["rom_offset"]
    stride = ENEMY_ART_TABLE["entry_size"]
    records = []
    for enemy_id in range(ENEMY_COUNT):
        offset = base + enemy_id * stride
        records.append({
            "id": enemy_id,
            "symbol": ENEMY_SYMBOLS[enemy_id] if enemy_id < len(ENEMY_SYMBOLS) else None,
            "record_offset": offset,
            "vram_patterns": _word(data, offset),
            "art_offsets": [_long(data, offset + 2 + 4 * i) for i in range(ENEMY_ART_FIELDS)],
            "mapping_offset": _long(data, offset + 14),
            "half_width_cells": data[offset + 18],
            "height_cells": data[offset + 19],
        })
    return records


def enemy_art_bounds(records: Sequence[dict[str, Any]], rom_size: int) -> dict[int, int]:
    """Distance from each art or mapping offset to the next one along.

    The three art blobs and the mapping are stored interleaved in the same
    banks, so together they bracket each other and no external length source is
    needed to bound a stream.
    """
    landmarks = sorted(
        {offset for record in records for offset in record["art_offsets"]}
        | {record["mapping_offset"] for record in records}
    )
    return {
        offset: (landmarks[i + 1] if i + 1 < len(landmarks) else rom_size) - offset
        for i, offset in enumerate(landmarks)
    }


def build_tile_bank(
    data: bytes, record: dict[str, Any], bounds: dict[int, int] | None = None
) -> tuple[list[bytes], list[dict[str, Any]]]:
    """Decompress an enemy's three art blobs into one bank.

    The blobs are concatenated in `ENEMY_ART_BANK_ORDER`, which is the order
    the body mapping addresses them in, not the order the record lists them.
    `sources` stays in record-field order and each entry says where its blob
    landed in the bank.
    """
    tiles: list[bytes] = []
    sources: list[dict[str, Any]] = [{} for _ in record["art_offsets"]]
    for field in ENEMY_ART_BANK_ORDER:
        offset = record["art_offsets"][field]
        label = f"ArtNem_{record['symbol'] or record['id']}Battle{field + 1}"
        decompressed, source = _art_record(
            data, offset, label, None if bounds is None else bounds.get(offset)
        )
        source["field"] = f"+0x{2 + 4 * field:02X}"
        source["first_pattern"] = len(tiles)
        sources[field] = source
        tiles.extend(
            decode_tile(decompressed[i:i + TILE_SIZE])
            for i in range(0, len(decompressed), TILE_SIZE)
        )
    return tiles, sources


def enemy_mapping(data: bytes, record: dict[str, Any]) -> tuple[list[int], dict[str, Any]]:
    """Read an enemy's body mapping. Returns (pattern-name words, metadata)."""
    offset = record["mapping_offset"]
    columns = 2 * record["half_width_cells"]
    expected = columns * record["height_cells"]
    raw = record["id"] in RAW_MAPPING_ENEMY_IDS

    if raw:
        words = [_word(data, offset + 2 * i) for i in range(expected)]
        consumed = 2 * expected
    else:
        try:
            words, consumed = enigma_decompress(data, offset, max_words=4096)
        except EnigmaError as exc:
            raise BattleArtError(
                f"enemy 0x{record['id']:02X} ({record['symbol']}): the mapping at "
                f"0x{offset:06X} is not an Enigma stream ({exc}). Only "
                f"{sorted(RAW_MAPPING_ENEMY_IDS)} are known to store a raw word "
                "array there; a new one needs its own proof before it is trusted."
            ) from exc

    if len(words) != expected:
        raise BattleArtError(
            f"enemy 0x{record['id']:02X} ({record['symbol']}): mapping at "
            f"0x{offset:06X} decoded {len(words)} words but the record's "
            f"{record['half_width_cells']}x{record['height_cells']} half-size "
            f"means {columns}x{record['height_cells']} = {expected} cells"
        )
    return words, {
        "rom_offset": f"0x{offset:06X}",
        "format": "raw_words" if raw else "enigma",
        "consumed": consumed,
        "columns": columns,
        "rows": record["height_cells"],
        "cell_count": expected,
    }


def enemy_palette_words(data: bytes, enemy_id: int) -> list[int]:
    offset = ENEMY_PALETTE_TABLE["rom_offset"] + enemy_id * ENEMY_PALETTE_TABLE["entry_size"]
    raw = _slice(data, offset, ENEMY_PALETTE_COLORS * 2, f"enemy palette 0x{enemy_id:02X}")
    return [int.from_bytes(raw[i:i + 2], "big") for i in range(0, len(raw), 2)]


def enemy_cram_line(data: bytes, enemy_id: int, line: int = 1) -> list[int]:
    """The sixteen CRAM words an enemy's line holds once battle init is done."""
    if line not in ENEMY_CRAM_LINES:
        raise BattleArtError(f"enemies occupy CRAM line 1 or 2, not {line}")
    colors = [0x0000] * COLORS_PER_LINE
    for index, value in ENEMY_FIXED_COLORS.items():
        colors[index] = value
    words = enemy_palette_words(data, enemy_id)
    for i, value in enumerate(words):
        colors[ENEMY_PALETTE_FIRST_INDEX + i] = value
    colors[14] = UI_COLOR_14
    colors[15] = next(v for _, l, i, v in UI_COLOR_SITES if l == line and i == 15)
    return colors


# ---------------------------------------------------------------------------
# Characters
# ---------------------------------------------------------------------------
def character_blocks(data: bytes) -> list[tuple[int, int]]:
    """The (start, end) of each PLC block, from the self-relative offsets."""
    base = CHARACTER_PLC_TABLE["rom_offset"]
    starts = [base + _word(data, base + 2 * i) for i in range(CHARACTER_COUNT)]
    if starts != sorted(starts):
        raise BattleArtError(
            f"{CHARACTER_PLC_TABLE['label']} offsets are not ascending; the block "
            "boundaries cannot be derived from them"
        )
    ends = starts[1:] + [CHARACTER_PLC_TABLE["rom_end_exclusive"]]
    return list(zip(starts, ends))


def _is_mapping(data: bytes, offset: int) -> bool:
    """Tell a pose mapping from an art pointer inside a PLC block.

    A block is a flat list of longs with no tag, so the two are separated the
    only way the cartridge itself separates them: an Enigma stream decodes and
    a run of raw pattern data does not. The bound is generous on purpose -- it
    only has to exclude art, and art is not a valid stream at all.
    """
    try:
        words, _ = enigma_decompress(data, offset, max_words=2048)
    except EnigmaError:
        return False
    return 1 <= len(words) <= 512


def character_records(data: bytes) -> list[dict[str, Any]]:
    """Decode the eleven PLC blocks into characters with an ordered pose list."""
    records = []
    for index, (start, end) in enumerate(character_blocks(data)):
        if (end - start - 2) % 4:
            raise BattleArtError(
                f"PLC block at 0x{start:06X} is {end - start} bytes, which is not a "
                "size word followed by whole longs"
            )
        longs = [_long(data, start + 2 + 4 * i) for i in range((end - start - 2) // 4)]
        if not longs:
            raise BattleArtError(f"PLC block at 0x{start:06X} has no art pointer")

        art_offsets: list[int] = [longs[0]]
        poses: list[dict[str, Any]] = []
        current = longs[0]
        for value in longs[1:]:
            if _is_mapping(data, value):
                poses.append({
                    "pose": len(poses),
                    "mapping_offset": value,
                    "art_offset": current,
                })
            else:
                current = value
                art_offsets.append(value)

        records.append({
            "id": index + 1,  # fighter ids are 1-based
            "symbol": CHARACTER_SYMBOLS[index],
            "plc_offset": start,
            "plc_end_exclusive": end,
            "vram_bytes": _word(data, start),
            "art_offsets": art_offsets,
            "poses": poses,
        })
    return records


def character_art_bank(data: bytes, art_offset: int, patterns: int) -> list[bytes]:
    """Read `patterns` uncompressed 8x8 tiles from `art_offset`."""
    raw = _slice(data, art_offset, patterns * TILE_SIZE, f"character art 0x{art_offset:06X}")
    return [decode_tile(raw[i:i + TILE_SIZE]) for i in range(0, len(raw), TILE_SIZE)]


def character_art_extents(data: bytes, records: Sequence[dict[str, Any]]) -> dict[int, int]:
    """How many patterns each character art blob actually holds.

    Uncompressed art carries no length. The poses that use a blob do: the
    highest pattern any of them names is the last one the blob has to contain.
    That reproduces every documented blob length exactly, so it is used as the
    extent rather than the PLC's size word, which is a VRAM allocation and
    over-reserves.
    """
    extents: dict[int, int] = {}
    for record in records:
        for pose in record["poses"]:
            words, _ = enigma_decompress(data, pose["mapping_offset"], max_words=2048)
            highest = max(word & 0x07FF for word in words) + 1
            offset = pose["art_offset"]
            extents[offset] = max(extents.get(offset, 0), highest)
    return extents


def character_cram_line(data: bytes, vehicle_index: int = 0) -> list[int]:
    """CRAM line 3 as `loc_7634` leaves it. Index 0 is never written."""
    table = CHARACTER_PALETTE_TABLE
    if not 0 <= vehicle_index < table["entry_count"]:
        raise BattleArtError(
            f"Vehicle_Index {vehicle_index} is outside the {table['entry_count']}-entry "
            f"{table['label']} table"
        )
    offset = table["rom_offset"] + vehicle_index * table["entry_size"]
    raw = _slice(data, offset, CHARACTER_PALETTE_COLORS * 2, table["label"])
    words = [int.from_bytes(raw[i:i + 2], "big") for i in range(0, len(raw), 2)]
    return [0x0000] + words


def verify_ui_colors(data: bytes) -> list[dict[str, Any]]:
    """Check the palette immediates `loc_7634` writes are where we think.

    These three colours come from instruction operands rather than a table, so
    the instruction is decoded and its destination checked before the value is
    believed.
    """
    sites = []
    for offset, line, index, value in UI_COLOR_SITES:
        opcode = _word(data, offset)
        immediate = _word(data, offset + 2)
        destination = _word(data, offset + 4)
        expected = PALETTE_TABLE_BUFFER + line * COLORS_PER_LINE * 2 + index * 2
        if opcode != MOVE_W_IMMEDIATE_TO_SHORT or immediate != value or destination != expected:
            raise BattleArtError(
                f"0x{offset:06X}: expected `move.w #${value:04X},"
                f"(Palette_Table_Buffer+${line * 32 + index * 2:02X})` but found opcode "
                f"0x{opcode:04X} immediate 0x{immediate:04X} destination 0x{destination:04X}"
            )
        sites.append({
            "rom_offset": f"0x{offset:06X}",
            "cram_line": line,
            "color_index": index,
            "value": f"0x{value:04X}",
        })
    return sites


# ---------------------------------------------------------------------------
# CRAM images
# ---------------------------------------------------------------------------
def _rgb_line(words: Sequence[int]) -> list[RGB]:
    return palette_rgb(decode_palette(b"".join(w.to_bytes(2, "big") for w in words)))


def battle_cram(
    data: bytes, enemy_id: int | None = None, enemy_line: int = 1, vehicle_index: int = 0
) -> list[RGB]:
    """A 64-colour CRAM image of a battle: line 3 characters, line 1/2 enemy."""
    lines: list[list[RGB]] = [list(GRAYSCALE_RAMP) for _ in range(planes.CRAM_LINES)]
    lines[CHARACTER_CRAM_LINE] = _rgb_line(character_cram_line(data, vehicle_index))
    if enemy_id is not None:
        lines[enemy_line] = _rgb_line(enemy_cram_line(data, enemy_id, enemy_line))
    return [color for line in lines for color in line]


# ---------------------------------------------------------------------------
# Extraction
# ---------------------------------------------------------------------------
def extract_enemy_art(data: bytes) -> dict[str, Any]:
    records = enemy_records(data)
    bounds = enemy_art_bounds(records, len(data))
    art_cache: dict[int, dict[str, Any]] = {}
    entries = []
    formats: dict[str, int] = {}

    for record in records:
        tiles, sources = build_tile_bank(data, record, bounds)
        for source in sources:
            art_cache.setdefault(int(source["rom_offset"], 16), source)
        words, mapping = enemy_mapping(data, record)
        formats[mapping["format"]] = formats.get(mapping["format"], 0) + 1
        highest = max(word & 0x07FF for word in words) + 1
        if highest > len(tiles):
            raise BattleArtError(
                f"enemy 0x{record['id']:02X} ({record['symbol']}): mapping names "
                f"pattern {highest - 1} but the three art blobs supply {len(tiles)}"
            )
        # Cells the body mapping fills with a pattern the static bank leaves
        # blank. Those are the ones the animated sprite pieces cover.
        holes = sum(
            1 for word in words
            if (word & 0x07FF) and tiles[word & 0x07FF] == BLANK_PATTERN
        )
        entries.append({
            "id": record["id"],
            "symbol": record["symbol"],
            "record_offset": f"0x{record['record_offset']:06X}",
            "vram_patterns": record["vram_patterns"],
            "half_width_cells": record["half_width_cells"],
            "height_cells": record["height_cells"],
            "size_cells": [mapping["columns"], mapping["rows"]],
            "size_pixels": [mapping["columns"] * 8, mapping["rows"] * 8],
            "art": sources,
            "bank_patterns": len(tiles),
            "patterns_used": highest,
            "body_holes": holes,
            "body_complete": holes == 0,
            "mapping": mapping,
            "palette": {
                "rom_offset": f"0x{ENEMY_PALETTE_TABLE['rom_offset'] + record['id'] * 22:06X}",
                "cram_lines": list(ENEMY_CRAM_LINES),
                "first_index": ENEMY_PALETTE_FIRST_INDEX,
                "color_count": ENEMY_PALETTE_COLORS,
                "colors": decode_palette(
                    b"".join(w.to_bytes(2, "big") for w in enemy_palette_words(data, record["id"]))
                ),
            },
        })

    # Keep the body and dynamic-tile censuses in one extraction result.  The
    # import is local because the overlay module reuses this module's body
    # decoders; at call time this module is fully initialized.
    from .battle_enemy_overlays import decode_enemy_overlay_records, overlay_payload

    overlay_provenance, overlay_records = decode_enemy_overlay_records(
        data, records, bounds
    )
    overlay = overlay_payload(overlay_provenance, overlay_records)
    overlays_by_id = {entry["id"]: entry for entry in overlay["enemies"]}
    for entry in entries:
        entry["overlay"] = overlays_by_id[entry["id"]]

    return {
        "art_table": {
            "label": ENEMY_ART_TABLE["label"],
            "rom_offset": f"0x{ENEMY_ART_TABLE['rom_offset']:06X}",
            "rom_end_exclusive": f"0x{ENEMY_ART_TABLE['rom_offset'] + ENEMY_COUNT * 20:06X}",
            "entry_count": ENEMY_COUNT,
            "entry_size": 20,
        },
        "palette_table": {
            "label": ENEMY_PALETTE_TABLE["label"],
            "rom_offset": f"0x{ENEMY_PALETTE_TABLE['rom_offset']:06X}",
            "rom_end_exclusive": f"0x{ENEMY_PALETTE_TABLE['rom_offset'] + ENEMY_COUNT * 22:06X}",
            "entry_count": ENEMY_COUNT,
            "entry_size": 22,
        },
        "distinct_art_blobs": len(art_cache),
        "mapping_formats": formats,
        "raw_mapping_enemy_ids": sorted(RAW_MAPPING_ENEMY_IDS),
        "total_art_patterns": sum(s["tile_count"] for s in art_cache.values()),
        "art_bank_order": [f + 1 for f in ENEMY_ART_BANK_ORDER],
        "bodies_complete": sum(1 for e in entries if e["body_complete"]),
        "total_body_holes": sum(e["body_holes"] for e in entries),
        "overlay": {
            "table": overlay["table"],
            "provenance": overlay["provenance"],
            "coverage": overlay["coverage"],
            "piece_count": overlay["piece_count"],
            "enabled_piece_count": overlay["enabled_piece_count"],
            "distinct_mapping_blocks": overlay["distinct_mapping_blocks"],
        },
        "enemies": entries,
    }


def extract_character_art(data: bytes) -> dict[str, Any]:
    records = character_records(data)
    extents = character_art_extents(data, records)
    entries = []
    for record in records:
        poses = []
        for pose in record["poses"]:
            words, consumed = enigma_decompress(data, pose["mapping_offset"], max_words=2048)
            poses.append({
                "pose": pose["pose"],
                "mapping_rom_offset": f"0x{pose['mapping_offset']:06X}",
                "mapping_consumed": consumed,
                "cell_count": len(words),
                "fits_slot": len(words) <= CHARACTER_SLOT_WORDS,
                "art_rom_offset": f"0x{pose['art_offset']:06X}",
                "patterns_used": max(word & 0x07FF for word in words) + 1,
            })
        art = []
        for offset in record["art_offsets"]:
            patterns = extents[offset]
            raw = _slice(data, offset, patterns * TILE_SIZE, "character art")
            art.append({
                "rom_offset": f"0x{offset:06X}",
                "compression": None,
                "pattern_count": patterns,
                "size": patterns * TILE_SIZE,
                "rom_end_exclusive": f"0x{offset + patterns * TILE_SIZE:06X}",
                "sha256": hashlib.sha256(raw).hexdigest(),
            })
        entries.append({
            "id": record["id"],
            "symbol": record["symbol"],
            "plc_rom_offset": f"0x{record['plc_offset']:06X}",
            "vram_bytes": record["vram_bytes"],
            "art": art,
            "pose_count": len(poses),
            "poses": poses,
        })

    return {
        "plc_table": {
            "label": CHARACTER_PLC_TABLE["label"],
            "rom_offset": f"0x{CHARACTER_PLC_TABLE['rom_offset']:06X}",
            "entry_count": CHARACTER_COUNT,
        },
        "palette_table": {
            "label": CHARACTER_PALETTE_TABLE["label"],
            "rom_offset": f"0x{CHARACTER_PALETTE_TABLE['rom_offset']:06X}",
            "entry_count": CHARACTER_PALETTE_TABLE["entry_count"],
            "entry_size": CHARACTER_PALETTE_TABLE["entry_size"],
            "cram_line": CHARACTER_CRAM_LINE,
            "first_index": CHARACTER_PALETTE_FIRST_INDEX,
            "color_count": CHARACTER_PALETTE_COLORS,
            "colors": decode_palette(
                b"".join(w.to_bytes(2, "big") for w in character_cram_line(data)[1:])
            ),
        },
        "grid_cells": [CHARACTER_COLUMNS, CHARACTER_ROWS],
        "slot_words": CHARACTER_SLOT_WORDS,
        "total_poses": sum(len(r["poses"]) for r in records),
        "distinct_art_blobs": len(extents),
        "characters": entries,
    }


def extract_battle_art(data: bytes) -> dict[str, Any]:
    """Metadata for every battle actor. No pixel data is returned."""
    return {
        "ui_colors": verify_ui_colors(data),
        "enemies": extract_enemy_art(data),
        "characters": extract_character_art(data),
    }


# ---------------------------------------------------------------------------
# PNG export (Sega pixels: keep the output directory out of version control)
# ---------------------------------------------------------------------------
def render_enemy(data: bytes, record: dict[str, Any], line: int = 1, bounds=None) -> bytes:
    tiles, _ = build_tile_bank(data, record, bounds)
    words, mapping = enemy_mapping(data, record)
    base = line << 13  # $2000 for line 1, $4000 for line 2
    cells = planes.decode_cells([(word + base) & 0xFFFF for word in words])
    return planes.render(
        cells,
        mapping["columns"],
        tiles,
        battle_cram(data, record["id"], line),
        transparent_backdrop=True,
    )


def render_character_pose(
    data: bytes, record: dict[str, Any], pose: dict[str, Any], patterns: int
) -> bytes:
    tiles = character_art_bank(data, pose["art_offset"], patterns)
    words, _ = enigma_decompress(data, pose["mapping_offset"], max_words=2048)
    # The buffer a pose is decompressed into is 36 words; anything past that is
    # never drawn, so the render stops where the hardware stops.
    words = [(word + CHARACTER_BASE_WORD) & 0xFFFF for word in words[:CHARACTER_SLOT_WORDS]]
    cells = planes.decode_cells(words)
    return planes.render(
        cells, CHARACTER_COLUMNS, tiles, battle_cram(data), transparent_backdrop=True
    )


def _safe_name(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


def export_battle_art_pngs(data: bytes, out_dir: str | Path) -> list[dict[str, Any]]:
    """Write one PNG per enemy and per character pose."""
    directory = Path(out_dir)
    (directory / "enemies").mkdir(parents=True, exist_ok=True)
    (directory / "characters").mkdir(parents=True, exist_ok=True)
    written: list[dict[str, Any]] = []

    records = enemy_records(data)
    bounds = enemy_art_bounds(records, len(data))
    for record in records:
        name = f"{record['id']:03d}_{_safe_name(record['symbol'] or 'unknown')}"
        path = directory / "enemies" / f"{name}.png"
        image = render_enemy(data, record, 1, bounds)
        path.write_bytes(image)
        written.append({
            "kind": "enemy",
            "id": record["id"],
            "symbol": record["symbol"],
            "path": str(path),
            "palette": "loc_27FFA2 line 1",
            "bytes": len(image),
        })

    characters = character_records(data)
    extents = character_art_extents(data, characters)
    for record in characters:
        for pose in record["poses"]:
            name = f"{record['symbol']}_pose{pose['pose']}"
            path = directory / "characters" / f"{name}.png"
            image = render_character_pose(
                data, record, pose, extents[pose["art_offset"]]
            )
            path.write_bytes(image)
            written.append({
                "kind": "character",
                "id": record["id"],
                "symbol": record["symbol"],
                "pose": pose["pose"],
                "path": str(path),
                "palette": "loc_76A4 line 3",
                "bytes": len(image),
            })
    return written
