"""Mega Drive tile art and palettes, decoded from the retail ROM.

Three formats meet here:

- **Nemesis-compressed art**, decompressed by `psiv_tools.nemesis`, which is
  what `NemDecomp` is called on throughout `ps4.asm`.
- **Uncompressed art**, copied straight to VRAM (`Art_DialogueFont` is the
  example this module carries).
- **Palettes** in Mega Drive CRAM format, three bits per channel.

## Tile format

A Mega Drive pattern is 32 bytes: eight rows of one longword, two pixels per
byte, **most significant nybble first**. This is not an assumption. It is what
`Nem_PCD_WritePixel` does when it assembles a row:

    lsl.l   #4, d4      ; shift the row up by a nybble
    or.b    d1, d4      ; and drop the new palette index into the low nybble

The first pixel decoded ends up in the highest nybble of the longword, so the
high nybble of each byte is the left pixel. (It is a packed format, not a
planar one; the pixels of a row are contiguous.)

## CRAM format

A colour is one big-endian word laid out as `%0000 BBB0 GGG0 RRR0`: three bits
per channel at bit positions 1-3, 5-7 and 9-11. The remaining bits are ignored
by the VDP.

Three bits are widened to eight by **bit replication** (`v << 5 | v << 2 |
v >> 1`). The alternative in common use is `v * 36`, which is simpler but tops
out at 252, so pure white never reaches 0xFF and a round trip through an image
editor cannot reproduce the source value. Replication maps 0 to 0 and 7 to
255, is monotone, and inverts exactly as `v = c >> 5`, so nothing is lost.
`levels` keeps the original 0-7 triple alongside the widened value regardless.

## Provenance

Every offset below was located in the retail ROM and is asserted before use.
The pointer tables are read from the cartridge itself rather than hard-coded
per entry, so the ROM stays the authority on which art belongs to which
portrait or battle background.

Decoded pixels are never returned by `extract_graphics`; only metadata is,
because metadata is safe to commit and Sega's artwork is not. Use
`export_art_pngs` to write images somewhere ignored, such as `generated/`.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Iterable, Sequence

from . import png
from .nemesis import TILE_SIZE, decompress, read_header

TILE_WIDTH = 8
TILE_HEIGHT = 8
COLORS_PER_LINE = 16
PALETTE_LINE_SIZE = COLORS_PER_LINE * 2

# %0000BBB0GGG0RRR0 -- everything outside this mask is ignored by the VDP.
CRAM_DEFINED_MASK = 0x0EEE

RGB = tuple[int, int, int]


class GraphicsError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Nemesis art with a documented compressed length.
#
# Each of these is referenced directly by code (rather than through a pointer
# table) and its length is the length of the matching `binclude` payload in the
# public disassembly. Every one of those payloads occurs exactly once in the
# retail image, which is how the offsets here were established; the extractor
# re-proves them by requiring the decompressor to stop at the documented end.
# ---------------------------------------------------------------------------
NEMESIS_ART: list[dict[str, Any]] = [
    {"label": "ArtNem_SegaLogo", "rom_offset": 0x0411D0, "compressed_size": 1004},
    {"label": "ArtNem_TitlePSTitle", "rom_offset": 0x1CC47A, "compressed_size": 2608},
    {"label": "ArtNem_TitleTheEndOfTheMillennium", "rom_offset": 0x1CCEAA, "compressed_size": 1020},
    {"label": "ArtNem_PressStartButton", "rom_offset": 0x1CD2A6, "compressed_size": 202},
    {"label": "ArtNem_TitleCopyrightText", "rom_offset": 0x1CD370, "compressed_size": 242},
    {"label": "ArtNem_TitleBackground", "rom_offset": 0x1CD462, "compressed_size": 7568},
    {"label": "ArtNem_GameStartMotaBG", "rom_offset": 0x1CF1F2, "compressed_size": 5680},
    {"label": "ArtNem_TitleScrollingTextBG", "rom_offset": 0x1D0822, "compressed_size": 6260},
    {"label": "ArtNem_SaveBlockGlowingCursor", "rom_offset": 0x1D2096, "compressed_size": 92},
    # Loaded together by Title_ArtPtrs at VRAM tiles $680 and $7C0, while
    # loc_42C90 copies Pal_Init_Line_3 into Palette_Line_3, so both render
    # against that line.
    {
        "label": "ArtNem_WindowTiles",
        "rom_offset": 0x2A2B7E,
        "compressed_size": 1212,
        "palette": {"label": "Pal_Init_Line_3", "line": 0},
    },
    {
        "label": "ArtNem_Font",
        "rom_offset": 0x2A303A,
        "compressed_size": 790,
        "palette": {"label": "Pal_Init_Line_3", "line": 0},
    },
    {"label": "ArtNem_CreditFont", "rom_offset": 0x2A3350, "compressed_size": 498},
]

# Uncompressed art, copied to VRAM without a decompressor.
RAW_ART: list[dict[str, Any]] = [
    {
        "label": "Art_DialogueFont",
        "rom_offset": 0x2A3542,
        "size": 1280,
        "palette": {"label": "Pal_Init_Line_3", "line": 0},
    },
]

# ---------------------------------------------------------------------------
# Pointer tables read out of the cartridge.
# ---------------------------------------------------------------------------
DIALOGUE_PORTRAIT_TABLE: dict[str, Any] = {
    "label": "DialoguePortraitArtPtrs",
    "rom_offset": 0x06A4B0,
    "entry_count": 0x28,
    "vram_tile": 0x55C,
}

# Every dialogue portrait decodes to 36 patterns, and laying them out
# row-major six across produces the 48x48 picture, so the art is stored in
# reading order rather than the column-major order sprite mappings often use.
PORTRAIT_TILES = 36
PORTRAIT_COLUMNS = 6

# The disassembly appends seven shopkeeper portraits at $28-$2E. Those
# pointers are not in the retail table: index $27 (Sekreas) is the last
# pointer, and the longwords after it are unrelated data. The shopkeeper
# portrait blobs do exist in the ROM and are reached from the shop tables near
# 0x068000 instead. Reading only 0x28 entries is therefore deliberate.
DIALOGUE_PORTRAIT_SYMBOLS: tuple[str | None, ...] = (
    None, "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi",
    "Wren", "Raja", "Kyra", "Seth", "Saya", "Holt", "Principal", "Dorin",
    "Pana", "HntGuildReceptionist", "Baker", "Zio", "Juza", "Gyuna", "Esper", "Esper",
    "EsperChief", "EsperChief", "GumbiousPriest", "GumbiousBishop", "Lashiec",
    "XeAThoul", "XeAThoul2", "XeAThoul2", "FortuneTeller", "DElmLars",
    "AlysWounded", "ReFaze", "MissingStudent", "Tallas", "DyingBoy", "Sekreas",
)

BATTLE_BG_TABLE: dict[str, Any] = {
    "label": "BattleBGArtPtrs",
    "rom_offset": 0x006ED4,
    "entry_count": 0x20,
    "entry_size": 12,  # art, plane mapping, palette
}

# `loc_6C3C` clears colour 0 of the palette buffer and then copies $D words to
# Palette_Table_Buffer+2, so a battle-background palette is 13 colours landing
# in line 0 at index 1, with index 0 forced to black.
BATTLE_BG_PALETTE_COLORS = 13
BATTLE_BG_PALETTE_FIRST_INDEX = 1

# Several backgrounds share one art blob under different palettes, so the art
# and the palette have different names in the disassembly. Keeping both apart
# matters: index $06 is `Pal_BioPlantBattleBG` over `ArtNem_PowerPlantsBattleBG`.
BATTLE_BG_PALETTE_SYMBOLS: tuple[str, ...] = (
    "MotaDesert", "MotaBeach", "MotaGrass", "MotaSea", "Dezo1", "Dezo2",
    "BioPlant", "Wreckage", "PlateSys", "ClimCenter", "WeaponPlant",
    "VahalFort", "PlateSysF1", "AcademyBasement", "LadeaTower", "ZioFort",
    "GaruberkTw", "TonoeBasement", "AirCastle", "MonsenAndIslandCave",
    "MonsenAndIslandCave", "Rykros", "Zema", "Reshel", "CourageTw",
    "StrengthTw", "AngerTw", "TheEdge", "ElsydeonCave", "Passageway",
    "CarnivorousTrees", "DarkForce1",
)

BATTLE_BG_ART_SYMBOLS: tuple[str, ...] = (
    "MotaDesert", "MotaBeach", "MotaGrass", "MotaSea", "Dezo", "Dezo",
    "PowerPlants", "PowerPlants", "PowerPlants", "PowerPlants", "PowerPlants",
    "PowerPlants", "PowerPlants", "AcademyBasement", "LadeaTower",
    "ZioFortAirCastle", "GaruberkTw", "TonoeBasement", "ZioFortAirCastle",
    "MonsenCavePassageway", "IslandAndElsydeonCave", "Rykros", "Zema",
    "Reshel", "RykrosTowers", "RykrosTowers", "RykrosTowers", "TheEdge",
    "IslandAndElsydeonCave", "MonsenCavePassageway", "CarnivorousTrees",
    "DarkForce1",
)

# ---------------------------------------------------------------------------
# Palettes.
# ---------------------------------------------------------------------------
PALETTES: list[dict[str, Any]] = [
    {
        "label": "Pal_Init",
        "rom_offset": 0x09F2BC,
        "line_count": 4,
        "note": "initial CRAM image; only line 3 (index 2) holds colours",
    },
    {
        "label": "Pal_Init_Line_3",
        "rom_offset": 0x296300,
        "line_count": 1,
        "note": "copied into Palette_Line_3 by loc_42C90; the window/font line",
    },
    {"label": "Pal_TitleScreen", "rom_offset": 0x1D29BC, "line_count": 4},
    {"label": "Pal_TitleScrollingText", "rom_offset": 0x1D2A3C, "line_count": 4},
    {"label": "Pal_TitleCharPortraits", "rom_offset": 0x2F4994, "line_count": 4},
]

# Pal_Init_Line_3 is a byte-for-byte duplicate of line index 2 of Pal_Init, in
# the same way the character level tables are duplicated. Verified, not assumed.
PAL_INIT_MIRRORED_LINE = 2

# Fallback for art whose CRAM line the disassembly does not pin down. The
# palette line a tile uses is carried by the Enigma-compressed plane mapping,
# not by the art, so guessing one would be worse than showing raw indices.
GRAYSCALE_RAMP: tuple[RGB, ...] = tuple(
    (v, v, v) for v in (0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255)
)


# ---------------------------------------------------------------------------
# Colour decoding
# ---------------------------------------------------------------------------
def expand_channel(level: int) -> int:
    """Widen a 3-bit CRAM channel to 8 bits by replicating its bits."""
    if not 0 <= level <= 7:
        raise GraphicsError(f"CRAM channel level {level} is outside 0..7")
    return (level << 5) | (level << 2) | (level >> 1)


def decode_color(word: int) -> dict[str, Any]:
    """Decode one CRAM word, `%0000BBB0GGG0RRR0`."""
    if not 0 <= word <= 0xFFFF:
        raise GraphicsError(f"CRAM word 0x{word:X} is not a 16-bit value")
    red = (word >> 1) & 7
    green = (word >> 5) & 7
    blue = (word >> 9) & 7
    rgb = (expand_channel(red), expand_channel(green), expand_channel(blue))
    entry: dict[str, Any] = {
        "raw": f"0x{word:04X}",
        "levels": {"r": red, "g": green, "b": blue},
        "rgb": list(rgb),
        "hex": "#{:02X}{:02X}{:02X}".format(*rgb),
    }
    if word & ~CRAM_DEFINED_MASK & 0xFFFF:
        # The VDP ignores these bits; flag them rather than silently dropping
        # them, since a set bit here usually means the offset is wrong.
        entry["unused_bits_set"] = True
    return entry


def decode_palette(raw: bytes) -> list[dict[str, Any]]:
    """Decode a run of big-endian CRAM words."""
    if len(raw) % 2:
        raise GraphicsError(f"Palette data is {len(raw)} bytes, which is not a whole number of words")
    return [decode_color((raw[i] << 8) | raw[i + 1]) for i in range(0, len(raw), 2)]


def palette_rgb(colors: Iterable[dict[str, Any]]) -> list[RGB]:
    return [tuple(c["rgb"]) for c in colors]  # type: ignore[misc]


# ---------------------------------------------------------------------------
# Tile decoding
# ---------------------------------------------------------------------------
def decode_tile(chunk: bytes) -> bytes:
    """Expand one 32-byte pattern into 64 palette indices, row-major."""
    if len(chunk) != TILE_SIZE:
        raise GraphicsError(f"A pattern is {TILE_SIZE} bytes, got {len(chunk)}")
    out = bytearray(TILE_WIDTH * TILE_HEIGHT)
    for i, byte in enumerate(chunk):
        out[i * 2] = byte >> 4
        out[i * 2 + 1] = byte & 0x0F
    return bytes(out)


def decode_tiles(data: bytes) -> list[bytes]:
    if len(data) % TILE_SIZE:
        raise GraphicsError(
            f"Art is {len(data)} bytes, which is not a whole number of "
            f"{TILE_SIZE}-byte patterns"
        )
    return [decode_tile(data[i:i + TILE_SIZE]) for i in range(0, len(data), TILE_SIZE)]


def compose_sheet(tiles: Sequence[bytes], columns: int = 16) -> tuple[int, int, bytes]:
    """Lay tiles out left to right, top to bottom. Returns (w, h, indices)."""
    if columns <= 0:
        raise GraphicsError(f"Sheet needs at least one column, got {columns}")
    if not tiles:
        raise GraphicsError("Cannot compose a sheet from zero tiles")
    rows = (len(tiles) + columns - 1) // columns
    width = columns * TILE_WIDTH
    height = rows * TILE_HEIGHT
    pixels = bytearray(width * height)
    for index, tile in enumerate(tiles):
        ox = (index % columns) * TILE_WIDTH
        oy = (index // columns) * TILE_HEIGHT
        for y in range(TILE_HEIGHT):
            start = (oy + y) * width + ox
            pixels[start:start + TILE_WIDTH] = tile[y * TILE_WIDTH:(y + 1) * TILE_WIDTH]
    return width, height, bytes(pixels)


def render_sheet(
    tiles: Sequence[bytes],
    colors: Sequence[RGB] = GRAYSCALE_RAMP,
    columns: int = 16,
    transparent_index: int | None = 0,
) -> bytes:
    """Render tiles to an indexed PNG. Index 0 is the Mega Drive's transparent
    colour for sprites, so it is made transparent unless told otherwise."""
    width, height, pixels = compose_sheet(tiles, columns)
    palette = list(colors)
    if len(palette) < COLORS_PER_LINE:
        palette += [(0, 0, 0)] * (COLORS_PER_LINE - len(palette))
    transparent = () if transparent_index is None else (transparent_index,)
    return png.encode_indexed(width, height, pixels, palette, transparent)


# ---------------------------------------------------------------------------
# ROM reads
# ---------------------------------------------------------------------------
def _slice(data: bytes, offset: int, size: int, what: str) -> bytes:
    if offset < 0 or offset + size > len(data):
        raise GraphicsError(f"{what} at 0x{offset:06X} (+{size}) runs past the end of the ROM")
    return data[offset:offset + size]


def read_long(data: bytes, offset: int) -> int:
    return int.from_bytes(_slice(data, offset, 4, "Pointer"), "big")


def decompress_art(
    data: bytes,
    offset: int,
    label: str,
    compressed_size: int | None = None,
    max_compressed_size: int | None = None,
) -> tuple[bytes, dict[str, Any]]:
    """Decompress one Nemesis blob and check it against everything known.

    The header's tile count independently predicts the output length, so a
    decode that terminates cleanly at the right size and inside the right span
    of ROM has agreed with the cartridge three separate ways.

    `compressed_size` is an exact documented length; `max_compressed_size` is
    an upper bound derived from where the next blob starts. `NemDecomp` refills
    its bit window before writing the pixels that emptied it, so it may read
    one byte it never uses and `consumed` is allowed to exceed the stream by
    exactly one.
    """
    header = read_header(data, offset)
    decompressed, consumed = decompress(data, offset)

    if len(decompressed) != header.decompressed_size:
        raise GraphicsError(
            f"{label} at 0x{offset:06X}: decoded {len(decompressed)} bytes but the "
            f"header declares {header.tile_count} patterns "
            f"({header.decompressed_size} bytes)"
        )
    if compressed_size is not None and consumed - compressed_size not in (0, 1):
        raise GraphicsError(
            f"{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes, "
            f"but the documented stream is {compressed_size} bytes (a trailing "
            "lookahead byte would allow at most one more)"
        )
    if max_compressed_size is not None and consumed > max_compressed_size + 1:
        raise GraphicsError(
            f"{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes and "
            f"ran into the next blob, which starts {max_compressed_size} bytes in"
        )

    size = compressed_size if compressed_size is not None else consumed
    record: dict[str, Any] = {
        "label": label,
        "rom_offset": f"0x{offset:06X}",
        "compression": "nemesis",
        "compressed_size": size,
        "rom_end_exclusive": f"0x{offset + size:06X}",
        "consumed": consumed,
        "trailing_lookahead_byte": consumed == size + 1,
        "tile_count": header.tile_count,
        "xor_mode": header.xor_mode,
        "header_word": f"0x{header.raw:04X}",
        "decompressed_size": len(decompressed),
        "decompressed_sha256": hashlib.sha256(decompressed).hexdigest(),
        "compressed_sha256": hashlib.sha256(_slice(data, offset, size, label)).hexdigest(),
    }
    return decompressed, record


def read_pointer_table(data: bytes, offset: int, count: int, stride: int = 4) -> list[int]:
    return [read_long(data, offset + i * stride) for i in range(count)]


def _gap_bounds(offsets: Iterable[int], rom_size: int, wanted: Iterable[int] | None = None) -> dict[int, int]:
    """For each wanted offset, how far it is to the next known address.

    `offsets` is every address the surrounding pointer table names, which may
    include neighbours that are not themselves art. The more of them there are,
    the tighter the bound; `wanted` defaults to all of them.
    """
    landmarks = sorted({o for o in offsets if o})
    targets = sorted({o for o in (wanted if wanted is not None else landmarks) if o})
    bounds = {}
    for offset in targets:
        following = next((o for o in landmarks if o > offset), rom_size)
        bounds[offset] = following - offset
    return bounds


# ---------------------------------------------------------------------------
# Extraction
# ---------------------------------------------------------------------------
def extract_palettes(data: bytes) -> dict[str, Any]:
    """Decode the named palettes and verify the Pal_Init_Line_3 duplication."""
    entries = []
    by_label: dict[str, list[list[dict[str, Any]]]] = {}
    for spec in PALETTES:
        offset = spec["rom_offset"]
        size = spec["line_count"] * PALETTE_LINE_SIZE
        raw = _slice(data, offset, size, spec["label"])
        lines = [
            decode_palette(raw[i * PALETTE_LINE_SIZE:(i + 1) * PALETTE_LINE_SIZE])
            for i in range(spec["line_count"])
        ]
        by_label[spec["label"]] = lines
        entry: dict[str, Any] = {
            "label": spec["label"],
            "rom_offset": f"0x{offset:06X}",
            "rom_end_exclusive": f"0x{offset + size:06X}",
            "line_count": spec["line_count"],
            "colors_per_line": COLORS_PER_LINE,
            "raw_hex": raw.hex(),
            "lines": [{"line": i, "colors": colors} for i, colors in enumerate(lines)],
        }
        if "note" in spec:
            entry["note"] = spec["note"]
        entries.append(entry)

    init = next(s for s in PALETTES if s["label"] == "Pal_Init")
    mirror = next(s for s in PALETTES if s["label"] == "Pal_Init_Line_3")
    init_line = _slice(
        data,
        init["rom_offset"] + PAL_INIT_MIRRORED_LINE * PALETTE_LINE_SIZE,
        PALETTE_LINE_SIZE,
        "Pal_Init",
    )
    mirror_line = _slice(data, mirror["rom_offset"], PALETTE_LINE_SIZE, "Pal_Init_Line_3")
    if init_line != mirror_line:
        raise GraphicsError(
            "Pal_Init_Line_3 at 0x{:06X} is not a copy of Pal_Init line index {} at "
            "0x{:06X}; one of the two offsets is wrong".format(
                mirror["rom_offset"], PAL_INIT_MIRRORED_LINE, init["rom_offset"]
            )
        )

    return {
        "format": "megadrive_cram",
        "color_word_layout": "%0000BBB0GGG0RRR0, big-endian",
        "channel_expansion": "3-bit to 8-bit by bit replication (v<<5 | v<<2 | v>>1)",
        "pal_init_line_3_mirrors_pal_init_line": PAL_INIT_MIRRORED_LINE,
        "palettes": entries,
        "_lines_by_label": by_label,
    }


def extract_dialogue_portraits(data: bytes) -> dict[str, Any]:
    spec = DIALOGUE_PORTRAIT_TABLE
    pointers = read_pointer_table(data, spec["rom_offset"], spec["entry_count"])
    bounds = _gap_bounds(pointers, len(data))

    cache: dict[int, dict[str, Any]] = {}
    entries = []
    for index, pointer in enumerate(pointers):
        symbol = DIALOGUE_PORTRAIT_SYMBOLS[index] if index < len(DIALOGUE_PORTRAIT_SYMBOLS) else None
        if pointer == 0:
            # Index $00 is a null pointer; the caller treats 0 as "no portrait".
            entries.append({"index": index, "symbol": symbol, "art": None})
            continue
        if pointer not in cache:
            label = f"ArtNem_{symbol}DialPortrait" if symbol else f"ArtNem_DialPortrait_{index:02X}"
            _, record = decompress_art(
                data, pointer, label, max_compressed_size=bounds[pointer]
            )
            cache[pointer] = record
        entries.append({"index": index, "symbol": symbol, "art": cache[pointer]})

    return {
        "label": spec["label"],
        "table_rom_offset": f"0x{spec['rom_offset']:06X}",
        "entry_count": spec["entry_count"],
        "vram_tile": f"0x{spec['vram_tile']:03X}",
        "distinct_art_blobs": len(cache),
        "entries": entries,
    }


def extract_battle_backgrounds(data: bytes) -> dict[str, Any]:
    spec = BATTLE_BG_TABLE
    base, count, stride = spec["rom_offset"], spec["entry_count"], spec["entry_size"]
    art_pointers = [read_long(data, base + i * stride) for i in range(count)]
    mapping_pointers = [read_long(data, base + i * stride + 4) for i in range(count)]
    # Each background's Enigma plane mapping is stored immediately after its
    # art, so the two pointer columns together bound every art blob exactly.
    # That makes the compressed length a ROM-internal fact: nothing outside the
    # cartridge is needed to know where a battle background ends.
    bounds = _gap_bounds(art_pointers + mapping_pointers, len(data), art_pointers)

    cache: dict[int, dict[str, Any]] = {}
    entries = []
    for index in range(count):
        offset = base + index * stride
        art_ptr = read_long(data, offset)
        mapping_ptr = read_long(data, offset + 4)
        palette_ptr = read_long(data, offset + 8)
        art_symbol = BATTLE_BG_ART_SYMBOLS[index] if index < len(BATTLE_BG_ART_SYMBOLS) else None
        symbol = (
            BATTLE_BG_PALETTE_SYMBOLS[index] if index < len(BATTLE_BG_PALETTE_SYMBOLS) else None
        )
        if art_ptr not in cache:
            _, record = decompress_art(
                data,
                art_ptr,
                f"ArtNem_{art_symbol}BattleBG" if art_symbol else f"ArtNem_BattleBG_{index:02X}",
                compressed_size=bounds[art_ptr],
            )
            cache[art_ptr] = record
        raw = _slice(
            data, palette_ptr, BATTLE_BG_PALETTE_COLORS * 2, f"battle palette {index:02X}"
        )
        entries.append({
            "index": index,
            "symbol": symbol,
            "art_symbol": art_symbol,
            "art": cache[art_ptr],
            "plane_mapping": {
                "label": f"MapEni_{art_symbol}BattleBG" if art_symbol else None,
                "rom_offset": f"0x{mapping_ptr:06X}",
                "compression": "enigma",
                "decoded": False,
            },
            "palette": {
                "label": f"Pal_{symbol}BattleBG" if symbol else None,
                "rom_offset": f"0x{palette_ptr:06X}",
                "cram_line": 0,
                "first_index": BATTLE_BG_PALETTE_FIRST_INDEX,
                "color_count": BATTLE_BG_PALETTE_COLORS,
                "index_0_forced_black": True,
                "raw_hex": raw.hex(),
                "colors": decode_palette(raw),
            },
        })

    return {
        "label": spec["label"],
        "table_rom_offset": f"0x{base:06X}",
        "entry_count": count,
        "entry_size": stride,
        "distinct_art_blobs": len(cache),
        "palette_note": (
            "loc_6C3C clears colour 0 and copies 13 words to Palette_Table_Buffer+2, "
            "so a battle palette occupies line 0 indices 1-13"
        ),
        "entries": entries,
    }


def extract_graphics(data: bytes) -> dict[str, Any]:
    """Decode every located art blob and palette; return metadata only.

    No pixel data is returned. Rendered images go through `export_art_pngs`,
    which writes to a caller-chosen directory that must not be committed.
    """
    palettes = extract_palettes(data)
    palettes.pop("_lines_by_label", None)

    nemesis_art = []
    for spec in NEMESIS_ART:
        _, record = decompress_art(
            data, spec["rom_offset"], spec["label"], compressed_size=spec["compressed_size"]
        )
        if "palette" in spec:
            record["palette"] = spec["palette"]
        nemesis_art.append(record)

    raw_art = []
    for spec in RAW_ART:
        raw = _slice(data, spec["rom_offset"], spec["size"], spec["label"])
        if spec["size"] % TILE_SIZE:
            raise GraphicsError(
                f"{spec['label']} is {spec['size']} bytes, not a whole number of patterns"
            )
        record = {
            "label": spec["label"],
            "rom_offset": f"0x{spec['rom_offset']:06X}",
            "rom_end_exclusive": f"0x{spec['rom_offset'] + spec['size']:06X}",
            "compression": None,
            "size": spec["size"],
            "tile_count": spec["size"] // TILE_SIZE,
            "sha256": hashlib.sha256(raw).hexdigest(),
        }
        if "palette" in spec:
            record["palette"] = spec["palette"]
        raw_art.append(record)

    portraits = extract_dialogue_portraits(data)
    battle = extract_battle_backgrounds(data)

    total_tiles = (
        sum(a["tile_count"] for a in nemesis_art)
        + sum(a["tile_count"] for a in raw_art)
        + sum(e["art"]["tile_count"] for e in portraits["entries"] if e["art"])
        + sum(e["art"]["tile_count"] for e in battle["entries"])
    )
    return {
        "tile_format": {
            "bytes_per_tile": TILE_SIZE,
            "size": [TILE_WIDTH, TILE_HEIGHT],
            "bits_per_pixel": 4,
            "packing": "two pixels per byte, high nybble is the left pixel",
        },
        "palettes": palettes,
        "nemesis_art": nemesis_art,
        "raw_art": raw_art,
        "dialogue_portraits": portraits,
        "battle_backgrounds": battle,
        "total_tiles_decoded": total_tiles,
    }


# ---------------------------------------------------------------------------
# PNG export (never committed; write under generated/ or a temp directory)
# ---------------------------------------------------------------------------
def _palette_for(data: bytes, spec: dict[str, Any] | None) -> list[RGB]:
    if not spec:
        return list(GRAYSCALE_RAMP)
    palette = next((p for p in PALETTES if p["label"] == spec["label"]), None)
    if palette is None:
        raise GraphicsError(f"Unknown palette {spec['label']!r}")
    offset = palette["rom_offset"] + spec.get("line", 0) * PALETTE_LINE_SIZE
    raw = _slice(data, offset, PALETTE_LINE_SIZE, spec["label"])
    return palette_rgb(decode_palette(raw))


def _safe_name(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


def export_art_pngs(data: bytes, out_dir: str | Path, columns: int = 16) -> list[dict[str, Any]]:
    """Write one PNG per located art blob. Returns what was written.

    The output directory holds decoded Sega artwork and must stay out of
    version control.
    """
    directory = Path(out_dir)
    directory.mkdir(parents=True, exist_ok=True)
    written: list[dict[str, Any]] = []

    def emit(
        label: str,
        tiles: Sequence[bytes],
        palette: dict[str, Any] | None,
        colors: Sequence[RGB] | None = None,
        palette_name: str | None = None,
        sheet_columns: int | None = None,
    ) -> None:
        across = sheet_columns or columns
        rgb = list(colors) if colors is not None else _palette_for(data, palette)
        image = render_sheet(tiles, rgb, across)
        path = directory / f"{_safe_name(label)}.png"
        path.write_bytes(image)
        if palette_name is None:
            palette_name = palette["label"] if palette else "grayscale index ramp"
        written.append({
            "label": label,
            "path": str(path),
            "tile_count": len(tiles),
            "columns": across,
            "palette": palette_name,
            "bytes": len(image),
        })

    for spec in NEMESIS_ART:
        decompressed, _ = decompress_art(
            data, spec["rom_offset"], spec["label"], compressed_size=spec["compressed_size"]
        )
        emit(spec["label"], decode_tiles(decompressed), spec.get("palette"))

    for spec in RAW_ART:
        raw = _slice(data, spec["rom_offset"], spec["size"], spec["label"])
        emit(spec["label"], decode_tiles(raw), spec.get("palette"))

    portraits = extract_dialogue_portraits(data)
    seen: set[str] = set()
    for entry in portraits["entries"]:
        art = entry["art"]
        if art is None or art["label"] in seen:
            continue
        seen.add(art["label"])
        decompressed, _ = decompress_art(data, int(art["rom_offset"], 16), art["label"])
        tiles = decode_tiles(decompressed)
        across = PORTRAIT_COLUMNS if len(tiles) == PORTRAIT_TILES else None
        emit(art["label"], tiles, None, sheet_columns=across)

    battle = extract_battle_backgrounds(data)
    for entry in battle["entries"]:
        art = entry["art"]
        label = f"{art['label']}_{entry['symbol']}"
        decompressed, _ = decompress_art(data, int(art["rom_offset"], 16), art["label"])
        # Colour 0 is cleared by the loader, then 13 colours land at index 1.
        colors = [(0, 0, 0)] + palette_rgb(entry["palette"]["colors"])
        colors += [(0, 0, 0)] * (COLORS_PER_LINE - len(colors))
        emit(label, decode_tiles(decompressed), None, colors, entry["palette"]["label"])

    return written
