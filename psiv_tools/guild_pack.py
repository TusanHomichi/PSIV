"""The Hunter's Guild job board: what `Event_HuntersGuild` draws from the ROM.

`Event_HuntersGuild` (`$0718E6`, `ps4.asm:150995`) keeps its quest table, money
table and job names behind its own code, and draws the list window with three
colour lines: `$C000` (line 2, `Pal_Init_Line_3`, the window font's own),
`$A000` (line 1) for a finished job and `$E000` (line 3) for a job that is not
open. Lines 1 and 3 are `HuntersGuild`'s map palette, so the colours belong to
the pack, not to the source.

The rows and the money are the only copy the game reads: the scene's rules
(`psiv-core`, `scenes/hunters_guild.rs`) take them from here.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from . import gfx
from .ship_menu_pack import WIN_GROUP_EVENT, ShipMenuError
from .text import decode_name

GUILD_DIRECTORY = "guild"
GUILD_NAME = "guild/guild.json"
FORMAT_VERSION = 1

#: The event's first instruction is `lea (HuntersGuildData).l, a0`.
EVENT_START = 0x0718E6
#: `HuntersGuildData`: eight rows of eight bytes.
DATA = 0x071D74
ROWS = 8
ROW_BYTES = 8
#: `HuntersGuildMoneyTbl`, thousands of meseta, directly after the rows.
MONEY = DATA + ROWS * ROW_BYTES
#: `HuntersGuildTextOffset`: eight words relative to itself.
NAME_OFFSETS = MONEY + ROWS
#: `lea (GuildText_ListingPending).l, a0` in the list loop, and its target.
PENDING_LEA = 0x071AE2
PENDING = 0x071E46
#: `move.w #$B, d0` / `Window_Create`: the list window's `WinGroup_Event` record.
WINDOW_ID = 0x0B
#: `HuntersGuild`'s map palette blob: line 0, then lines 1 and 3
#: (`psiv_tools/layouts.py`), named by the map record's own pointer.
MAP_PALETTE_POINTER = 0x148BA4
MAP_PALETTE = 0x12F510
LINE_1 = MAP_PALETTE + 0x20
LINE_3 = MAP_PALETTE + 0x40
#: `Pal_Init_Line_3`, copied into line 2: the colours the font's PNG is drawn in.
PAL_INIT_LINE_3 = 0x296300
LINE_WORDS = 16


class GuildError(ShipMenuError):
    """The ROM does not hold the guild's data where the US image does."""


def _word(rom: bytes, offset: int) -> int:
    return int.from_bytes(rom[offset:offset + 2], "big")


def _long(rom: bytes, offset: int) -> int:
    return int.from_bytes(rom[offset:offset + 4], "big")


def _line(rom: bytes, offset: int) -> list[list[int]]:
    data = rom[offset:offset + LINE_WORDS * 2]
    return [list(colour) for colour in gfx.palette_rgb(gfx.decode_palette(data))]


def _string(rom: bytes, offset: int) -> str:
    end = rom.find(b"\xfe", offset, offset + 32)
    if end < 0:
        raise GuildError(f"no $FE ends the job name at 0x{offset:06X}")
    return decode_name(rom[offset:end])


def extract_guild(rom: bytes) -> dict[str, Any]:
    """The JSON record, after checking each anchor against the code that names it."""
    if rom[EVENT_START:EVENT_START + 2] != b"\x41\xf9" or _long(rom, EVENT_START + 2) != DATA:
        raise GuildError("Event_HuntersGuild does not open with its table's lea")
    if rom[PENDING_LEA:PENDING_LEA + 2] != b"\x41\xf9" or _long(rom, PENDING_LEA + 2) != PENDING:
        raise GuildError("the list loop does not name GuildText_ListingPending")
    if _long(rom, MAP_PALETTE_POINTER) != MAP_PALETTE:
        raise GuildError("HuntersGuild's map record does not name its palette")
    window = WIN_GROUP_EVENT + WINDOW_ID * 8
    width, height, x, y = rom[window:window + 4]
    return {
        "format_version": FORMAT_VERSION,
        "kind": "guild",
        "rom_sha256": hashlib.sha256(rom).hexdigest(),
        "window": {
            "id": WINDOW_ID,
            "width": width + 1,
            "height": height + 1,
            "x": x,
            "y": y,
            "rom_offset": f"0x{window:06X}",
        },
        "names": [
            _string(rom, NAME_OFFSETS + _word(rom, NAME_OFFSETS + row * 2)) for row in range(ROWS)
        ],
        "pending": _string(rom, PENDING),
        "rows": [list(rom[DATA + row * ROW_BYTES:DATA + (row + 1) * ROW_BYTES]) for row in range(ROWS)],
        "money": list(rom[MONEY:MONEY + ROWS]),
        "font_line": _line(rom, PAL_INIT_LINE_3),
        "done_line": _line(rom, LINE_1),
        "closed_line": _line(rom, LINE_3),
        "provenance": {
            "event": f"0x{EVENT_START:06X}",
            "rows": f"0x{DATA:06X}",
            "money": f"0x{MONEY:06X}",
            "name_offsets": f"0x{NAME_OFFSETS:06X}",
            "pending": f"0x{PENDING:06X}",
            "map_palette": f"0x{MAP_PALETTE:06X}",
            "font_line": f"0x{PAL_INIT_LINE_3:06X}",
        },
    }


def emit_guild(rom: bytes, directory: Path) -> dict[str, str]:
    """Writes the record and returns its manifest entry."""
    record = extract_guild(rom)
    path = directory / GUILD_NAME
    path.parent.mkdir(parents=True, exist_ok=True)
    data = (json.dumps(record, indent=2, sort_keys=True) + "\n").encode()
    path.write_bytes(data)
    return {"path": GUILD_NAME, "sha256": hashlib.sha256(data).hexdigest()}
