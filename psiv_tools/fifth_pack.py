"""The fifth member's table: what `Event_PickingFifthCharacter` reads from the ROM.

`Event_PickingFifthCharacter` (`$071E66`, `ps4.asm:151681`) keeps two tables
behind its code. `loc_72182` has one eight-byte row per friend waiting at the
Mota Spaceport, in the order of their objects from `$FFFFC440`: where they stand
(X and Y words), their character id, the first of their dialogue lines, the
`EventFlag_*Picked` flag set while they are the fifth, and an art offset.
`loc_721B8` lists the five character records the routine refills, as RAM
addresses. Both are the cartridge's data, so they belong to the pack, not to
the source: the scene (`psiv-core`, `scenes/fifth_character.rs`) takes them
from here (`docs/scenes/181_PickingFifthCharacter.md`).
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

FIFTH_DIRECTORY = "fifth"
FIFTH_NAME = "fifth/fifth.json"
FORMAT_VERSION = 1

#: `EventPtrs[$68]`'s routine: its first instruction is `jsr (CalcPartyNumber).l`.
EVENT_START = 0x071E66
CALC_PARTY_NUMBER = 0x05A6A4
#: `lea (loc_721B8).l, a0` before the refill loop, and `lea (loc_72182).l, a0`
#: before the scan for the row ahead of the leader.
RECORDS_LEA = 0x071EA6
ROWS_LEA = 0x071EEC
ROWS = 5
ROW_BYTES = 8
#: `Character_Stats`: record N is `$80` bytes on.
CHARACTER_STATS = 0xFFFFF500
RECORD_BYTES = 0x80


class FifthError(ValueError):
    """The ROM does not hold the routine's tables where the US image does."""


def _word(rom: bytes, offset: int) -> int:
    return int.from_bytes(rom[offset:offset + 2], "big")


def _long(rom: bytes, offset: int) -> int:
    return int.from_bytes(rom[offset:offset + 4], "big")


def _lea_a0(rom: bytes, at: int, what: str) -> int:
    if rom[at:at + 2] != b"\x41\xf9":
        raise FifthError(f"no `lea (...).l, a0` naming {what} at 0x{at:06X}")
    return _long(rom, at + 2)


def extract_fifth(rom: bytes) -> dict[str, Any]:
    """The JSON record, each table found through the instruction that names it."""
    if rom[EVENT_START:EVENT_START + 2] != b"\x4e\xb9" or _long(rom, EVENT_START + 2) != CALC_PARTY_NUMBER:
        raise FifthError("Event_PickingFifthCharacter does not open with jsr CalcPartyNumber")
    rows_at = _lea_a0(rom, ROWS_LEA, "loc_72182")
    records_at = _lea_a0(rom, RECORDS_LEA, "loc_721B8")
    rows = []
    for row in range(ROWS):
        at = rows_at + row * ROW_BYTES
        rows.append({
            "x": _word(rom, at),
            "y": _word(rom, at + 2),
            "character": rom[at + 4],
            "first_line": rom[at + 5],
            "picked_flag": rom[at + 6],
        })
    records = []
    for row in range(ROWS):
        address = _long(rom, records_at + row * 4)
        index, rest = divmod(address - CHARACTER_STATS, RECORD_BYTES)
        if rest or not 0 <= index < 16:
            raise FifthError(f"loc_721B8 entry {row} is not a character record: 0x{address:08X}")
        records.append(index)
    return {
        "format_version": FORMAT_VERSION,
        "kind": "fifth",
        "rom_sha256": hashlib.sha256(rom).hexdigest(),
        "rows": rows,
        "records": records,
        "provenance": {
            "event": f"0x{EVENT_START:06X}",
            "rows": f"0x{rows_at:06X}",
            "records": f"0x{records_at:06X}",
        },
    }


def emit_fifth(rom: bytes, directory: Path) -> dict[str, str]:
    """Writes the record and returns its manifest entry."""
    record = extract_fifth(rom)
    path = directory / FIFTH_NAME
    path.parent.mkdir(parents=True, exist_ok=True)
    data = (json.dumps(record, indent=2, sort_keys=True) + "\n").encode()
    path.write_bytes(data)
    return {"path": FIFTH_NAME, "sha256": hashlib.sha256(data).hexdigest()}
