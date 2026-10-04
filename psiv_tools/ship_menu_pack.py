"""The ship's destination screen: the planetary map behind the menu windows.

`loc_63BC4` (`ps4.asm:133499`) draws the screen the destination menu stands
on in three steps, all of which this module replays from the cartridge:

* `loc_63F00` (`:133745-133756`) decompresses two Nemesis art blobs to VRAM
  tiles `$100` and `$1EB`;
* `loc_63F2C` (`:133758-133911`) blits two Enigma plane mappings (the frame
  and orbits to Plane A, the picture's second layer to Plane B), loads the
  32-word palette `loc_1DE158`, and then, for each world the mask leaves
  out, draws a *cover* over that world's marker (`$FFFFED42` bits 5, 3 and 2:
  Rykros, Kuran, the Air Castle);
* `loc_64144` (`:133894-133925`) re-uploads the palette every frame and
  blinks the cursor row's marker by writing `$EEE` into one entry.

The windows, text and cursor are not here: they are the shared window art the
dialogue pack already carries, drawn by the shell at the rectangles in
`windows` below (`WinGroup_Event`, `ps4.asm:141150`). What this module emits
is everything the screen needs that only the ROM knows: the eight static
backgrounds (one per cover combination) as CRAM-indexed bytes, the palette,
the window records and the strings.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from . import gfx, planes
from .enigma import decompress as enigma_decompress
from .nemesis import decompress as nemesis_decompress
from .text import decode_name

SHIP_MENU_DIRECTORY = "ship_menu"
SHIP_MENU_NAME = "ship_menu/ship_menu.json"
FORMAT_VERSION = 1

SCREEN_WIDTH = 320
SCREEN_HEIGHT = 224
CELLS_WIDE = 40
CELLS_HIGH = 28
PLANE_STRIDE = 64

#: `loc_63F00`: Nemesis art and the VRAM tile each blob is loaded at.
ART = ((0x1D7FAE, 0x100), (0x1D8A0E, 0x1EB))
#: `loc_63F2C`: the two full pictures, both 26 x 21 cells at screen cell (2,2).
PICTURE_A = {"offset": 0x1DDB88, "base": 0x100}
PICTURE_B = {"offset": 0x1DDC2A, "base": 0x2100}
PICTURE_ORIGIN = (2, 2)
PICTURE_SIZE = (26, 21)
#: `loc_1DE158`, 32 words copied into `Palette_Table_Buffer` (CRAM lines 0, 1).
PALETTE_OFFSET = 0x1DE158
PALETTE_WORDS = 32

#: The covers: (rom offset, base word, cell x, cell y, cols, rows), drawn into
#: Plane A. `RYKROS` runs when the Rykros marker is not shown (`:133781-133872`),
#: `KURAN` when mask bit 3 is clear (`:133880-133893`), `AIR_CASTLE` when bit 2
#: is clear (`:133895-133911`).
RYKROS_MAPS = (
    (0x1DDBFE, 0x100, 11, 8, 1, 2),
    (0x1DDC06, 0x100, 19, 13, 2, 2),
    (0x1DDC20, 0x100, 12, 5, 2, 2),
)
#: Single words written after the Rykros maps: (cell x, cell y, word).
RYKROS_WORDS = (
    (13, 4, 0x21EB),
    (11, 7, 0x21EB),
    (11, 10, 0x21EB),
    (25, 7, 0x21EB),
    (23, 10, 0x21EB),
    (26, 6, 0x21EC),
    (22, 11, 0x21EC),
)
KURAN_MAP = (0x1DDC12, 0x100, 6, 12, 3, 2)
AIR_CASTLE_MAP = (0x1DDBF2, 0x100, 15, 12, 2, 2)

#: Cover bits of a background index.
COVER_RYKROS = 1
COVER_KURAN = 2
COVER_AIR_CASTLE = 4

#: `WinGroup_Event` (`ps4.asm:141150`): eight-byte records, the window ids this
#: screen creates are 5 (the prompt) and `5 + rows` (the list).
WINDOW_IDS = (5, 6, 7, 8, 9, 10)

#: Strings: `loc_2AAA46`, `loc_2AAA60`, `loc_2AAA7A` (`ps4.asm:340113-340139`,
#: the retail English branch).
PROMPT_OFFSET = 0x2AAA46
SUFFIX_OFFSET = 0x2AAA60
NAMES_OFFSET = 0x2AAA7A
WORLD_NAMES = 6


class ShipMenuError(ValueError):
    """The ROM does not hold what the menu's loader expects."""


def _vram_tiles(rom: bytes) -> dict[int, bytes]:
    """VRAM tile number to 64 pixel indices, for the two art blobs."""
    tiles: dict[int, bytes] = {}
    for offset, first in ART:
        data, _ = nemesis_decompress(rom, offset)
        for index, tile in enumerate(gfx.decode_tiles(data)):
            tiles[first + index] = tile
    return tiles


def _map_words(rom: bytes, offset: int, base: int, count: int) -> list[int]:
    words, _ = enigma_decompress(rom, offset, base, max_words=planes.PLANE_BUFFER_WORDS)
    if len(words) < count:
        raise ShipMenuError(
            f"mapping at 0x{offset:06X} decoded {len(words)} cells, wanted {count}"
        )
    return words[:count]


def _blit(plane: list[int], words: list[int], x: int, y: int, cols: int, rows: int) -> None:
    """`PlaneMapToRAM`: `cols` words a row, `rows` rows, from cell (x, y)."""
    for row in range(rows):
        for col in range(cols):
            plane[((y + row) % 32) * PLANE_STRIDE + (x + col) % PLANE_STRIDE] = words[
                row * cols + col
            ]


def build_planes(rom: bytes, covers: int) -> tuple[list[int], list[int]]:
    """Plane A and Plane B as `loc_63F2C` leaves them for a cover set."""
    plane_a = [0] * (PLANE_STRIDE * 32)
    plane_b = [0] * (PLANE_STRIDE * 32)
    cols, rows = PICTURE_SIZE
    count = cols * rows
    _blit(
        plane_a,
        _map_words(rom, PICTURE_A["offset"], PICTURE_A["base"], count),
        *PICTURE_ORIGIN,
        cols,
        rows,
    )
    _blit(
        plane_b,
        _map_words(rom, PICTURE_B["offset"], PICTURE_B["base"], count),
        *PICTURE_ORIGIN,
        cols,
        rows,
    )
    if covers & COVER_RYKROS:
        for offset, base, x, y, w, h in RYKROS_MAPS:
            _blit(plane_a, _map_words(rom, offset, base, w * h), x, y, w, h)
        for x, y, word in RYKROS_WORDS:
            plane_a[y * PLANE_STRIDE + x] = word
    for flag, (offset, base, x, y, w, h) in (
        (COVER_KURAN, KURAN_MAP),
        (COVER_AIR_CASTLE, AIR_CASTLE_MAP),
    ):
        if covers & flag:
            _blit(plane_a, _map_words(rom, offset, base, w * h), x, y, w, h)
    return plane_a, plane_b


def compose_background(rom: bytes, covers: int, tiles: dict[int, bytes]) -> bytes:
    """The 320x224 picture as global CRAM indices, backdrop (0) transparent.

    The VDP's order at equal priority is Plane A over Plane B; a high-priority
    cell beats a low one on either plane. Neither plane here sets priority, but
    the rule is applied as the hardware applies it.
    """
    plane_a, plane_b = build_planes(rom, covers)
    out = bytearray(SCREEN_WIDTH * SCREEN_HEIGHT)
    for cy in range(CELLS_HIGH):
        for cx in range(CELLS_WIDE):
            cells = []
            for plane in (plane_a, plane_b):
                word = plane[cy * PLANE_STRIDE + cx]
                cell = planes.decode_cell(word)
                art = tiles.get(cell.tile)
                if art is None:
                    if word:
                        raise ShipMenuError(
                            f"cell ({cx},{cy}) names VRAM tile 0x{cell.tile:03X}, not loaded"
                        )
                    art = bytes(64)
                cells.append((cell, planes._oriented(art, cell.h_flip, cell.v_flip)))
            for y in range(8):
                for x in range(8):
                    chosen = 0
                    best = -1
                    for rank, (cell, art) in enumerate(cells):
                        pixel = art[y * 8 + x]
                        if not pixel:
                            continue
                        # Higher priority wins; plane A (rank 0) wins ties.
                        score = (2 if cell.priority else 0) + (1 if rank == 0 else 0)
                        if score > best:
                            best = score
                            chosen = cell.palette_line * 16 + pixel
                    out[(cy * 8 + y) * SCREEN_WIDTH + cx * 8 + x] = chosen
    return bytes(out)


def _palette(rom: bytes) -> list[dict[str, Any]]:
    raw = rom[PALETTE_OFFSET:PALETTE_OFFSET + PALETTE_WORDS * 2]
    colors = gfx.decode_palette(raw)
    rgb = gfx.palette_rgb(colors)
    return [
        {"word": f"0x{(raw[i * 2] << 8) | raw[i * 2 + 1]:04X}", "rgb": list(rgb[i])}
        for i in range(PALETTE_WORDS)
    ]


def _color(word: int) -> list[int]:
    return list(gfx.palette_rgb(gfx.decode_palette(word.to_bytes(2, "big")))[0])


#: The first records of `WinGroup_Event` (`ps4.asm:141150-141170`), which
#: follows `WinGroup_Dialogue` (`0x069380`) and is told apart from its look-alikes
#: by the whole run.
WIN_GROUP_EVENT_HEAD = tuple(
    bytes.fromhex(word)
    for word in (
        "19040715",
        "21050314",
        "0505050d",
        "06041b0e",
        "0505030e",
        "19040715",
        "0a021a09",
    )
)
WIN_GROUP_EVENT = 0x0693A0


def _windows(rom: bytes) -> list[dict[str, Any]]:
    """`WinGroup_Event` records for the window ids this screen creates."""
    head = [rom[WIN_GROUP_EVENT + i * 8:WIN_GROUP_EVENT + i * 8 + 4] for i in range(7)]
    if tuple(head) != WIN_GROUP_EVENT_HEAD:
        raise ShipMenuError("WinGroup_Event is not at 0x0693A0")
    records = []
    for window_id in WINDOW_IDS:
        offset = WIN_GROUP_EVENT + window_id * 8
        width, height, x, y = rom[offset:offset + 4]
        records.append(
            {
                "id": window_id,
                "width": width + 1,
                "height": height + 1,
                "x": x,
                "y": y,
                "rom_offset": f"0x{offset:06X}",
            }
        )
    return records


def _strings(rom: bytes) -> dict[str, Any]:
    def until(offset: int, end: int) -> bytes:
        stop = rom.index(bytes([end]), offset)
        return rom[offset:stop]

    names = []
    cursor = NAMES_OFFSET
    for _ in range(WORLD_NAMES):
        raw = until(cursor, 0xFE)
        names.append(decode_name(raw))
        cursor += len(raw) + 1
    suffix = until(SUFFIX_OFFSET, 0xFF)
    first, _, second = suffix.partition(b"\xfc")
    return {
        "prompt": decode_name(until(PROMPT_OFFSET, 0xFE)),
        "confirm_suffix": [decode_name(first), decode_name(second)],
        "names": names,
    }


def extract_ship_menu(rom: bytes) -> tuple[dict[str, Any], dict[int, bytes]]:
    """The JSON record and the eight cover-combination backgrounds."""
    tiles = _vram_tiles(rom)
    backgrounds = {covers: compose_background(rom, covers, tiles) for covers in range(8)}
    record = {
        "format_version": FORMAT_VERSION,
        "kind": "ship_menu",
        "rom_sha256": hashlib.sha256(rom).hexdigest(),
        "screen": {"width": SCREEN_WIDTH, "height": SCREEN_HEIGHT},
        "palette": _palette(rom),
        "colors": {"highlight": _color(0x0EEE), "dark": _color(0x0400)},
        "windows": _windows(rom),
        **_strings(rom),
        "backgrounds": [
            {
                "covers": covers,
                "file": f"{SHIP_MENU_DIRECTORY}/background_{covers}.idx",
                "sha256": hashlib.sha256(data).hexdigest(),
            }
            for covers, data in sorted(backgrounds.items())
        ],
        "provenance": {
            "loader": "loc_63F00, loc_63F2C, loc_64144 (ps4.asm:133745-133925)",
            "art": [{"rom_offset": f"0x{o:06X}", "vram_tile": f"0x{t:03X}"} for o, t in ART],
            "palette": f"0x{PALETTE_OFFSET:06X}",
        },
    }
    return record, backgrounds


def emit_ship_menu(rom: bytes, directory: Path) -> dict[str, str]:
    """Write the screen into `directory` and return its manifest entry."""
    record, backgrounds = extract_ship_menu(rom)
    folder = directory / SHIP_MENU_DIRECTORY
    folder.mkdir(parents=True, exist_ok=True)
    for covers, data in backgrounds.items():
        (folder / f"background_{covers}.idx").write_bytes(data)
    payload = (json.dumps(record, indent=2, sort_keys=True) + "\n").encode()
    (directory / SHIP_MENU_NAME).write_bytes(payload)
    return {"path": SHIP_MENU_NAME, "sha256": hashlib.sha256(payload).hexdigest()}
